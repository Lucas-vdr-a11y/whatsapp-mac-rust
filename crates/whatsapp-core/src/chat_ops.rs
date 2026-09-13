//! Chat lifecycle and poll/event operations.
//!
//! Extends [`WaClient`] with the chat-scoped command surface: deleting and
//! clearing chats, disappearing-message timers, mention sends, forwarding,
//! pinning messages, polls and events. It lives apart from `client.rs` so it
//! can evolve independently, like `actions.rs` does for message actions.
//!
//! Local persistence keeps metadata, not protobuf bodies (see `store.rs`), so
//! operations that reference an existing message (pin, poll vote) rebuild its
//! key from the stored row; forwarding is limited to text because there is no
//! stored body to relay for media.
//!
//! Poll votes are encrypted with the poll creation message's `message_secret`.
//! Secrets minted by [`WaClient::create_poll`] in this process are remembered
//! in memory; otherwise the upstream message-secret store is consulted (it
//! holds captures when upstream's secret persistence is enabled, which is the
//! default).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use whatsapp_rust::send::PinDuration;
use whatsapp_rust::wacore::proto_helpers::MessageBuilderExt;
use whatsapp_rust::wacore::types::message::AddressingMode;
use whatsapp_rust::waproto::whatsapp as wa;
use whatsapp_rust::{Client, EventCreationParams};

use crate::client::{WaClient, to_upstream};
use crate::error::{CoreError, Result};
use crate::events::CoreEvent;
use crate::store::Store;
use crate::types::{Jid, Message, MessageKind, MessageStatus};

/// Fallback sender id for own messages before the account JID is known.
/// Mirrors the constant in `client.rs` (private to that module).
const ME_PLACEHOLDER: &str = "me";

/// Maximum number of characters in a chat-list preview.
const PREVIEW_MAX: usize = 120;

/// WhatsApp polls accept 2-12 options.
const POLL_MIN_OPTIONS: usize = 2;
const POLL_MAX_OPTIONS: usize = 12;

/// The pin durations WhatsApp exposes, keyed by their day count.
const PIN_DAY_CHOICES: [u32; 3] = [1, 7, 30];

impl WaClient {
    /// Delete a chat for this account (syncd `deleteChat`).
    ///
    /// Downloaded media is kept (`delete_media = false`); there is no separate
    /// flag in the command surface. The local chat/message rows are not removed
    /// here: [`Store`] has no delete helper yet, so the protocol action is the
    /// source of truth until one lands in `store.rs`.
    pub async fn delete_chat(&self, chat_id: &Jid) -> Result<()> {
        let client = self.client().await?;
        let jid = to_upstream(chat_id)?;
        client
            .chat_actions()
            .delete_chat(&jid, false, None)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;
        Ok(())
    }

    /// Clear a chat's messages while keeping the chat (syncd `clearChat`).
    ///
    /// Starred messages and downloaded media are kept (`false`/`false`), the
    /// conservative default WhatsApp Web offers. Local rows are not touched
    /// until `Store` grows a clear helper (same as [`WaClient::delete_chat`]).
    pub async fn clear_chat(&self, chat_id: &Jid) -> Result<()> {
        let client = self.client().await?;
        let jid = to_upstream(chat_id)?;
        client
            .chat_actions()
            .clear_chat(&jid, false, false, None)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;
        Ok(())
    }

    /// Turn disappearing messages on for a chat (`seconds` = 0 disables).
    ///
    /// 1:1 chats send an `EPHEMERAL_SETTING` protocol message; groups use the
    /// group-metadata IQ (`Groups::set_ephemeral`). Other chat kinds (status,
    /// broadcast, newsletter) are rejected by the upstream layer.
    pub async fn set_disappearing_timer(&self, chat_id: &Jid, seconds: u32) -> Result<()> {
        let client = self.client().await?;
        let jid = to_upstream(chat_id)?;
        if chat_id.is_group() {
            client
                .groups()
                .set_ephemeral(&jid, seconds)
                .await
                .map_err(|error| CoreError::Protocol(error.to_string()))
        } else {
            client
                .set_chat_disappearing_timer(jid, seconds)
                .await
                .map(|_| ())
                .map_err(|error| CoreError::Protocol(error.to_string()))
        }
    }

    /// Send a text message that @-mentions the given JIDs.
    ///
    /// Mentions are attached through `context_info.mentioned_jid` (WA Web's
    /// `GenerateMentionedJids`), so the message is sent as an
    /// `extendedTextMessage`. The local echo is persisted and emitted exactly
    /// like [`WaClient::send_text`] in `client.rs`.
    pub async fn send_text_with_mentions(
        &self,
        chat_id: &Jid,
        text: &str,
        mentions: &[Jid],
    ) -> Result<Message> {
        let text = text.trim();
        if text.is_empty() {
            return Err(CoreError::InvalidInput("message text is empty".into()));
        }
        let context = mentions_context(mentions)?;

        let client = self.client().await?;
        let to = to_upstream(chat_id)?;
        let sent = client
            .send_message(&to, wa::Message::text_with_context(text, context))
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;

        let message = self.outgoing_message(
            sent.message_id,
            chat_id,
            MessageKind::Text,
            Some(text.to_owned()),
        );
        self.persist_and_emit(&message, &text_preview(text))?;
        Ok(message)
    }

    /// Forward a stored message to another chat.
    ///
    /// The original is looked up in the [`Store`]; only text messages can be
    /// reconstructed, because the store keeps metadata rather than protocol
    /// bodies. The forwarded copy is marked as forwarded by the upstream send
    /// path (`prepare_for_forward`), then persisted and emitted like a normal
    /// outgoing message.
    pub async fn forward_message(
        &self,
        to_chat: &Jid,
        from_chat: &Jid,
        message_id: &str,
    ) -> Result<Message> {
        let store = self.store();
        let original = store
            .find_message(message_id)?
            .ok_or_else(|| message_not_found(message_id))?;
        if original.chat_id != *from_chat {
            return Err(CoreError::InvalidInput(
                "the message does not belong to this chat".into(),
            ));
        }
        if original.kind != MessageKind::Text {
            return Err(CoreError::InvalidInput(
                "only text messages can be forwarded from the local store".into(),
            ));
        }
        let text = original.text.clone().unwrap_or_default();
        if text.is_empty() {
            return Err(CoreError::InvalidInput(
                "the stored message has no text to forward".into(),
            ));
        }

        let client = self.client().await?;
        let to = to_upstream(to_chat)?;
        let sent = client
            .forward_message(&to, &wa::Message::text(&text))
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;

        let message = self.outgoing_message(
            sent.message_id,
            to_chat,
            MessageKind::Text,
            Some(text.clone()),
        );
        self.persist_and_emit(&message, &text_preview(&text))?;
        Ok(message)
    }

    /// Pin a message in a chat for all participants (`days` = 1, 7 or 30).
    ///
    /// Group messages from others need the original author in the message key,
    /// so the store is consulted for the sender (mirrors `actions.rs`).
    pub async fn pin_message(
        &self,
        chat_id: &Jid,
        message_id: &str,
        days: u32,
        from_me: bool,
    ) -> Result<()> {
        let duration = pin_duration_from_days(days)?;
        let key = target_message_key(&self.store(), chat_id, message_id, from_me)?;
        let client = self.client().await?;
        let to = to_upstream(chat_id)?;
        client
            .pin_message(to, key, duration)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))
    }

    /// Unpin a message previously pinned in a chat.
    pub async fn unpin_message(
        &self,
        chat_id: &Jid,
        message_id: &str,
        from_me: bool,
    ) -> Result<()> {
        let key = target_message_key(&self.store(), chat_id, message_id, from_me)?;
        let client = self.client().await?;
        let to = to_upstream(chat_id)?;
        client
            .unpin_message(to, key)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))
    }

    /// Create a poll and return its message id.
    ///
    /// `selectable_count` is how many options a voter may pick; upstream
    /// validates the option count (2-12), duplicate names, and that the count
    /// is within `1..=options.len()` (single-select gets the v3 body, multi the
    /// v1 body). The returned id is also the key for [`WaClient::vote_poll`].
    pub async fn create_poll(
        &self,
        chat_id: &Jid,
        question: &str,
        options: &[String],
        selectable_count: u32,
    ) -> Result<String> {
        let question = question.trim();
        validate_poll(question, options, selectable_count)?;

        let client = self.client().await?;
        let to = to_upstream(chat_id)?;
        let (sent, secret) = client
            .polls()
            .create(to, question, options, selectable_count)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;

        let message_id = sent.message_id;
        remember_poll_secret(&message_id, &secret);

        let message = self.outgoing_message(
            message_id.clone(),
            chat_id,
            MessageKind::Poll,
            Some(question.to_owned()),
        );
        self.persist_and_emit(&message, &text_preview(question))?;
        Ok(message_id)
    }

    /// Vote on a poll (an empty `option_names` clears the previous vote).
    ///
    /// The poll's `message_secret` is needed to encrypt the vote. It is taken
    /// from this process's poll-creation cache when available, otherwise from
    /// the upstream message-secret store. The creator JID is derived from the
    /// stored poll row; for our own polls the exact sender identity used on the
    /// wire is preferred (from the secret store) and otherwise guessed from the
    /// chat's addressing mode.
    pub async fn vote_poll(
        &self,
        chat_id: &Jid,
        poll_message_id: &str,
        option_names: &[String],
    ) -> Result<()> {
        validate_vote_options(option_names)?;

        let store = self.store();
        let stored = store
            .find_message(poll_message_id)?
            .ok_or_else(|| message_not_found(poll_message_id))?;
        if stored.chat_id != *chat_id {
            return Err(CoreError::InvalidInput(
                "the poll does not belong to this chat".into(),
            ));
        }

        let client = self.client().await?;
        let remembered = remembered_poll_secret(poll_message_id);
        // Outbound lookups run even when the secret is cached so the exact
        // sender identity used on the wire is recovered; inbound lookups only
        // run when the cache missed.
        let from_store = if !stored.from_me && remembered.is_some() {
            None
        } else {
            poll_secret_from_store(&client, chat_id, &stored, poll_message_id).await?
        };

        let secret = remembered
            .or_else(|| from_store.as_ref().map(|(secret, _)| secret.clone()))
            .ok_or_else(|| {
                CoreError::Protocol(format!(
                    "no message secret stored for poll {poll_message_id}; \
                     the poll must have been created or received on this device"
                ))
            })?;

        let creator = if !stored.from_me {
            to_upstream(&stored.sender_id)?
        } else if let Some((_, Some(sender))) = from_store {
            sender
        } else {
            own_poll_creator(&client, chat_id).await?
        };

        let to = to_upstream(chat_id)?;
        client
            .polls()
            .vote(to, poll_message_id, &creator, &secret, option_names)
            .await
            .map(|_| ())
            .map_err(|error| CoreError::Protocol(error.to_string()))
    }

    /// Create an event and return its message id.
    ///
    /// `start_ts` is Unix seconds; `0` means "no start time". Upstream requires
    /// a non-empty name. The event's per-message secret is not retained here
    /// (RSVP responses are not part of this surface), so only the event itself
    /// can be sent, not responded to.
    pub async fn create_event(
        &self,
        chat_id: &Jid,
        name: &str,
        description: Option<&str>,
        start_ts: u64,
    ) -> Result<String> {
        let name = name.trim();
        if name.is_empty() {
            return Err(CoreError::InvalidInput("event name is empty".into()));
        }
        let params = EventCreationParams {
            name: name.to_owned(),
            description: description.map(str::to_owned),
            start_time: event_start_time(start_ts)?,
            ..Default::default()
        };

        let client = self.client().await?;
        let to = to_upstream(chat_id)?;
        let (sent, _secret) = client
            .events()
            .create(to, params)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;

        let message = self.outgoing_message(
            sent.message_id.clone(),
            chat_id,
            MessageKind::System,
            Some(name.to_owned()),
        );
        self.persist_and_emit(&message, &text_preview(name))?;
        Ok(sent.message_id)
    }

    /// Build the local echo row for an outgoing message.
    fn outgoing_message(
        &self,
        id: String,
        chat_id: &Jid,
        kind: MessageKind,
        text: Option<String>,
    ) -> Message {
        Message {
            id,
            chat_id: chat_id.clone(),
            sender_id: self.own_jid().unwrap_or_else(|| Jid::new(ME_PLACEHOLDER)),
            from_me: true,
            timestamp: now_unix(),
            kind,
            text,
            status: MessageStatus::Sent,
            view_once: false,
        }
    }

    /// Persist the local echo (chat row first) and publish it on the bus,
    /// mirroring `WaClient::send_text` in `client.rs`.
    fn persist_and_emit(&self, message: &Message, preview: &str) -> Result<()> {
        let store = self.store();
        store.record_message_activity(&message.chat_id, preview, message.timestamp, None, false)?;
        store.upsert_message(message)?;
        self.emit(CoreEvent::Message(message.clone()));
        Ok(())
    }
}

/// Poll secrets minted by this process, keyed by poll message id.
///
/// The upstream store keys secrets by `(chat, sender, message id)`, which needs
/// the exact sender identity; keeping the secret under the message id alone
/// makes a vote immediately after creation race-free.
fn poll_secrets() -> &'static Mutex<HashMap<String, Vec<u8>>> {
    static SECRETS: OnceLock<Mutex<HashMap<String, Vec<u8>>>> = OnceLock::new();
    SECRETS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn remember_poll_secret(message_id: &str, secret: &[u8]) {
    if let Ok(mut secrets) = poll_secrets().lock() {
        secrets.insert(message_id.to_owned(), secret.to_vec());
    }
}

fn remembered_poll_secret(message_id: &str) -> Option<Vec<u8>> {
    poll_secrets()
        .lock()
        .ok()
        .and_then(|secrets| secrets.get(message_id).cloned())
}

/// Look a poll secret up in the upstream message-secret store.
///
/// Returns the secret plus, for our own polls, the sender identity it was
/// stored under (the identity used on the wire, which the vote encryption must
/// reproduce).
async fn poll_secret_from_store(
    client: &Client,
    chat_id: &Jid,
    stored: &Message,
    poll_message_id: &str,
) -> Result<Option<(Vec<u8>, Option<whatsapp_rust::Jid>)>> {
    let backend = client.persistence_manager().backend();
    let chat = to_upstream(chat_id)?.to_non_ad_string();
    let candidates: Vec<whatsapp_rust::Jid> = if stored.from_me {
        // Outbound secrets are keyed by our own sender identity: PN for normal
        // chats, LID for bot chats (and LID-addressing groups).
        [client.pn(), client.lid()].into_iter().flatten().collect()
    } else {
        vec![to_upstream(&stored.sender_id)?]
    };

    for sender in candidates {
        let sender = sender.to_non_ad();
        if let Some(secret) = backend
            .get_msg_secret(&chat, &sender.to_non_ad_string(), poll_message_id)
            .await
            .map_err(|error| CoreError::Storage(error.to_string()))?
        {
            return Ok(Some((secret, stored.from_me.then_some(sender))));
        }
    }
    Ok(None)
}

/// Own base JID to use as the poll creator in `chat`.
///
/// Mirrors WhatsApp Web's `getMeUserLidOrJidForChat`: LID for LID-addressed
/// chats (and bot chats), the group's addressing mode for groups, PN otherwise,
/// falling back to whichever identity is known.
async fn own_poll_creator(client: &Client, chat_id: &Jid) -> Result<whatsapp_rust::Jid> {
    let prefer_lid = match chat_id.server() {
        Some("lid" | "bot") => true,
        Some("g.us") => {
            let group = to_upstream(chat_id)?;
            client
                .groups()
                .query_info(&group)
                .await
                .map(|info| info.addressing_mode == AddressingMode::Lid)
                .unwrap_or(false)
        }
        _ => false,
    };
    let (preferred, fallback) = if prefer_lid {
        (client.lid(), client.pn())
    } else {
        (client.pn(), client.lid())
    };
    preferred
        .or(fallback)
        .map(|jid| jid.to_non_ad())
        .ok_or_else(|| CoreError::Protocol("the account JID is not known yet".into()))
}

/// Build the mention context for an outgoing text message.
///
/// Mentions are stored as user JIDs (device suffix stripped), matching WhatsApp
/// Web's `mentionedJidList`.
fn mentions_context(mentions: &[Jid]) -> Result<wa::ContextInfo> {
    if mentions.is_empty() {
        return Err(CoreError::InvalidInput(
            "at least one mention is required".into(),
        ));
    }
    let mut mentioned_jid = Vec::with_capacity(mentions.len());
    for mention in mentions {
        mentioned_jid.push(to_upstream(mention)?.to_non_ad_string());
    }
    Ok(wa::ContextInfo {
        mentioned_jid,
        ..Default::default()
    })
}

/// Build the target [`wa::MessageKey`] of a message in `chat`.
///
/// `participant` is the original author for group messages we did not send and
/// `None` otherwise (the `from_me` flag then identifies the author), matching
/// `actions.rs`.
fn target_message_key(
    store: &Store,
    chat_id: &Jid,
    message_id: &str,
    from_me: bool,
) -> Result<wa::MessageKey> {
    let participant = if chat_id.is_group() && !from_me {
        let stored = store
            .find_message(message_id)?
            .ok_or_else(|| message_not_found(message_id))?;
        Some(to_upstream(&stored.sender_id)?)
    } else {
        None
    };
    Ok(wa::MessageKey {
        remote_jid: Some(chat_id.as_str().to_owned()),
        from_me: Some(from_me),
        id: Some(message_id.to_owned()),
        participant: participant.map(|jid| jid.to_string()),
    })
}

/// Map the command's day count onto WhatsApp's pin durations.
fn pin_duration_from_days(days: u32) -> Result<PinDuration> {
    match days {
        1 => Ok(PinDuration::Hours24),
        7 => Ok(PinDuration::Days7),
        30 => Ok(PinDuration::Days30),
        other => Err(CoreError::InvalidInput(format!(
            "pin duration must be one of {PIN_DAY_CHOICES:?} days, got {other}"
        ))),
    }
}

/// Validate a poll definition before it reaches the protocol layer.
fn validate_poll(question: &str, options: &[String], selectable_count: u32) -> Result<()> {
    if question.trim().is_empty() {
        return Err(CoreError::InvalidInput("poll question is empty".into()));
    }
    if !(POLL_MIN_OPTIONS..=POLL_MAX_OPTIONS).contains(&options.len()) {
        return Err(CoreError::InvalidInput(format!(
            "polls need between {POLL_MIN_OPTIONS} and {POLL_MAX_OPTIONS} options, got {}",
            options.len()
        )));
    }
    if options.iter().any(|option| option.trim().is_empty()) {
        return Err(CoreError::InvalidInput(
            "poll options must not be empty".into(),
        ));
    }
    if selectable_count == 0 || selectable_count as usize > options.len() {
        return Err(CoreError::InvalidInput(format!(
            "selectable_count must be between 1 and {} (got {selectable_count})",
            options.len()
        )));
    }
    let unique: std::collections::HashSet<&str> = options.iter().map(String::as_str).collect();
    if unique.len() != options.len() {
        return Err(CoreError::InvalidInput(
            "poll options must be unique".into(),
        ));
    }
    Ok(())
}

/// Validate a vote selection. An empty list clears the previous vote.
fn validate_vote_options(option_names: &[String]) -> Result<()> {
    if option_names.iter().any(|name| name.trim().is_empty()) {
        return Err(CoreError::InvalidInput(
            "poll option names must not be empty".into(),
        ));
    }
    Ok(())
}

/// Event start time: `0` means "not set"; anything outside `i64` is invalid.
fn event_start_time(start_ts: u64) -> Result<Option<i64>> {
    if start_ts == 0 {
        return Ok(None);
    }
    i64::try_from(start_ts)
        .map(Some)
        .map_err(|_| CoreError::InvalidInput("event start time is out of range".into()))
}

fn message_not_found(message_id: &str) -> CoreError {
    CoreError::Protocol(format!("message {message_id} not found in the local store"))
}

/// Short, human-readable chat-list preview (mirrors `client.rs`).
fn text_preview(text: &str) -> String {
    text.chars().take(PREVIEW_MAX).collect()
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{MessageKind, MessageStatus};

    fn stored_message() -> Message {
        Message {
            id: "MSG1".to_owned(),
            chat_id: Jid::new("5511999@s.whatsapp.net"),
            sender_id: Jid::new("5511999@s.whatsapp.net"),
            from_me: false,
            timestamp: 1_700_000_000,
            kind: MessageKind::Text,
            text: Some("hello".to_owned()),
            status: MessageStatus::Delivered,
            view_once: false,
        }
    }

    fn group_message() -> Message {
        Message {
            chat_id: Jid::new("120363000000000001@g.us"),
            ..stored_message()
        }
    }

    fn options(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn pin_duration_accepts_only_whatsapp_day_choices() {
        assert_eq!(pin_duration_from_days(1).unwrap(), PinDuration::Hours24);
        assert_eq!(pin_duration_from_days(7).unwrap(), PinDuration::Days7);
        assert_eq!(pin_duration_from_days(30).unwrap(), PinDuration::Days30);

        for unsupported in [0, 2, 3, 24, 31, u32::MAX] {
            let error = pin_duration_from_days(unsupported)
                .expect_err("unsupported day counts must be rejected");
            assert!(matches!(error, CoreError::InvalidInput(_)));
        }
    }

    #[test]
    fn validate_poll_accepts_a_well_formed_poll() {
        let poll_options = options(&["A", "B", "C"]);
        assert!(validate_poll("Question?", &poll_options, 2).is_ok());
        assert!(validate_poll("Question?", &poll_options, 1).is_ok());
        assert!(validate_poll("Question?", &poll_options, 3).is_ok());
    }

    #[test]
    fn validate_poll_rejects_bad_questions() {
        let poll_options = options(&["A", "B"]);
        assert!(validate_poll("", &poll_options, 1).is_err());
        assert!(validate_poll("   ", &poll_options, 1).is_err());
    }

    #[test]
    fn validate_poll_rejects_bad_option_counts() {
        assert!(matches!(
            validate_poll("Q", &options(&["A"]), 1),
            Err(CoreError::InvalidInput(_))
        ));
        let thirteen = options(&[
            "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13",
        ]);
        assert!(validate_poll("Q", &thirteen, 1).is_err());
        assert!(validate_poll("Q", &options(&["A", " "]), 1).is_err());
    }

    #[test]
    fn validate_poll_rejects_bad_selectable_counts() {
        let poll_options = options(&["A", "B"]);
        assert!(validate_poll("Q?", &poll_options, 0).is_err());
        assert!(validate_poll("Q?", &poll_options, 3).is_err());
    }

    #[test]
    fn validate_poll_rejects_duplicate_options() {
        let error = validate_poll("Q?", &options(&["A", "A"]), 1)
            .expect_err("duplicate names must be rejected");
        assert!(matches!(error, CoreError::InvalidInput(_)));
    }

    #[test]
    fn validate_vote_options_allows_clear_but_not_blanks() {
        assert!(validate_vote_options(&[]).is_ok());
        assert!(validate_vote_options(&options(&["A", "B"])).is_ok());
        assert!(validate_vote_options(&options(&["A", " "])).is_err());
    }

    #[test]
    fn mentions_context_requires_at_least_one_mention() {
        let error = mentions_context(&[]).expect_err("an empty mention list is a caller error");
        assert!(matches!(error, CoreError::InvalidInput(_)));
    }

    #[test]
    fn mentions_context_strips_device_suffixes() {
        let context = mentions_context(&[
            Jid::new("5511999@s.whatsapp.net"),
            Jid::new("5511888:3@s.whatsapp.net"),
        ])
        .expect("valid mentions");

        assert_eq!(
            context.mentioned_jid,
            vec![
                "5511999@s.whatsapp.net".to_owned(),
                "5511888@s.whatsapp.net".to_owned(),
            ]
        );
    }

    #[test]
    fn mentions_context_rejects_malformed_jids() {
        assert!(mentions_context(&[Jid::new("not a jid")]).is_err());
    }

    #[test]
    fn target_message_key_carries_the_participant_for_group_messages_from_others() {
        let store = Store::open_in_memory().expect("open store");
        let message = group_message();
        store
            .record_message_activity(&message.chat_id, "hello", message.timestamp, None, false)
            .expect("chat row");
        store.upsert_message(&message).expect("message row");

        let key = target_message_key(&store, &message.chat_id, "MSG1", false).expect("key");
        assert_eq!(key.remote_jid.as_deref(), Some("120363000000000001@g.us"));
        assert_eq!(key.id.as_deref(), Some("MSG1"));
        assert_eq!(key.from_me, Some(false));
        assert_eq!(key.participant.as_deref(), Some("5511999@s.whatsapp.net"));
    }

    #[test]
    fn target_message_key_requires_a_stored_sender_for_incoming_group_messages() {
        let store = Store::open_in_memory().expect("open store");
        let error = target_message_key(
            &store,
            &Jid::new("120363000000000001@g.us"),
            "MISSING",
            false,
        )
        .expect_err("a group key without an author must not be built");
        assert!(matches!(error, CoreError::Protocol(_)));
    }

    #[test]
    fn target_message_key_skips_the_store_for_own_and_direct_messages() {
        let store = Store::open_in_memory().expect("open store");

        // Our own group message: `from_me` identifies the author.
        let own = target_message_key(&store, &Jid::new("120363000000000001@g.us"), "MSG1", true)
            .expect("key");
        assert_eq!(own.participant, None);
        assert_eq!(own.from_me, Some(true));

        // Direct chat: no participant either way.
        let direct = target_message_key(&store, &Jid::new("5511999@s.whatsapp.net"), "MSG1", false)
            .expect("key");
        assert_eq!(direct.participant, None);
    }

    #[test]
    fn event_start_time_maps_zero_to_none() {
        assert_eq!(event_start_time(0).unwrap(), None);
        assert_eq!(
            event_start_time(1_700_000_000).unwrap(),
            Some(1_700_000_000)
        );
    }

    #[test]
    fn event_start_time_rejects_overflow() {
        let error = event_start_time(u64::MAX).expect_err("must not wrap into i64");
        assert!(matches!(error, CoreError::InvalidInput(_)));
    }

    #[test]
    fn poll_secrets_round_trip_by_message_id() {
        let id = "POLL-ROUND-TRIP";
        assert_eq!(remembered_poll_secret(id), None);

        remember_poll_secret(id, &[7u8; 32]);
        assert_eq!(remembered_poll_secret(id), Some(vec![7u8; 32]));

        assert_eq!(remembered_poll_secret("POLL-OTHER"), None);
    }

    #[test]
    fn text_preview_is_truncated_to_the_chat_list_budget() {
        assert_eq!(text_preview("hello"), "hello");
        assert_eq!(text_preview(&"x".repeat(500)).chars().count(), PREVIEW_MAX);
    }

    #[test]
    fn stored_message_fixture_is_self_consistent() {
        // Guards the fixtures the key tests rely on.
        let message = stored_message();
        assert!(!message.from_me);
        assert_eq!(message.kind, MessageKind::Text);
        assert_eq!(message.chat_id, message.sender_id);
    }
}
