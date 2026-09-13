//! Message-level actions: quoting replies, reactions, edits, revokes, stars.
//!
//! This module extends [`WaClient`] with the message-action surface. It is
//! implemented separately from `client.rs` so it can evolve independently.
//!
//! Group message keys need the original author: WhatsApp references an
//! incoming group message by `(chat, id, from_me, participant)` and rejects a
//! key without a `participant` when the message is not ours. The author comes
//! from the [`Store`](crate::store::Store), which is also the only source for
//! the quoted body (we persist metadata, not protobufs).

use whatsapp_rust::send::RevokeType;
use whatsapp_rust::wacore::proto_helpers::{MessageBuilderExt, build_quote_context_with_info};
use whatsapp_rust::waproto::whatsapp as wa;

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

impl WaClient {
    /// Send a text message that quotes an existing message.
    pub async fn send_text_quoting(
        &self,
        chat_id: &Jid,
        text: &str,
        quoted_message_id: &str,
    ) -> Result<Message> {
        let text = text.trim();
        if text.is_empty() {
            return Err(CoreError::InvalidInput("message text is empty".into()));
        }

        let store = self.store();
        let quoted = store
            .find_message(quoted_message_id)?
            .ok_or_else(|| message_not_found(quoted_message_id))?;

        let client = self.client().await?;
        let to = to_upstream(chat_id)?;
        let own_jid = self.own_jid();
        let inputs = quote_inputs(&quoted, own_jid.as_ref())?;
        let context = build_quote_context_with_info(
            &inputs.message_id,
            &inputs.sender,
            &inputs.quoted_chat,
            &to,
            &inputs.body,
        );

        let sent = client
            .send_message(&to, wa::Message::text_with_context(text, context))
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;

        let message = Message {
            id: sent.message_id,
            chat_id: chat_id.clone(),
            sender_id: own_jid.unwrap_or_else(|| Jid::new(ME_PLACEHOLDER)),
            from_me: true,
            timestamp: now_unix(),
            kind: MessageKind::Text,
            text: Some(text.to_owned()),
            status: MessageStatus::Sent,
        };

        // Chat row first: the message references it.
        store.record_message_activity(
            chat_id,
            &text_preview(text),
            message.timestamp,
            None,
            false,
        )?;
        store.upsert_message(&message)?;
        self.emit(CoreEvent::Message(message.clone()));
        Ok(message)
    }

    /// Send (or clear, with an empty emoji) a reaction to a message.
    pub async fn send_reaction(
        &self,
        chat_id: &Jid,
        message_id: &str,
        emoji: &str,
        from_me: bool,
    ) -> Result<()> {
        let client = self.client().await?;
        let to = to_upstream(chat_id)?;
        let stored = lookup_group_sender(&self.store(), chat_id, message_id, from_me)?;
        let participant = group_participant(chat_id, stored.as_ref(), from_me)?;
        let key = message_key(chat_id, message_id, from_me, participant);

        client
            .send_reaction(&to, key, emoji)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;
        Ok(())
    }

    /// Edit a message this account sent.
    pub async fn edit_message(
        &self,
        chat_id: &Jid,
        message_id: &str,
        new_text: &str,
    ) -> Result<()> {
        let text = new_text.trim();
        if text.is_empty() {
            return Err(CoreError::InvalidInput(
                "edited message text is empty".into(),
            ));
        }

        let store = self.store();
        let mut stored = store
            .find_message(message_id)?
            .ok_or_else(|| message_not_found(message_id))?;
        if !stored.from_me {
            return Err(CoreError::InvalidInput(
                "only messages sent by this account can be edited".into(),
            ));
        }
        if stored.chat_id != *chat_id {
            return Err(CoreError::InvalidInput(
                "the message does not belong to this chat".into(),
            ));
        }

        let client = self.client().await?;
        let to = to_upstream(chat_id)?;
        client
            .edit_message(&to, message_id, wa::Message::text(text))
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;

        // `upsert_message` refreshes the text on conflict; nothing else moved.
        stored.text = Some(text.to_owned());
        store.upsert_message(&stored)?;
        Ok(())
    }

    /// Delete a message, for everyone or just locally.
    pub async fn revoke_message(
        &self,
        chat_id: &Jid,
        message_id: &str,
        for_everyone: bool,
    ) -> Result<()> {
        let store = self.store();
        let stored = store
            .find_message(message_id)?
            .ok_or_else(|| message_not_found(message_id))?;
        let client = self.client().await?;
        let to = to_upstream(chat_id)?;

        if for_everyone {
            client
                .revoke_message(&to, message_id, revoke_type(chat_id, &stored)?)
                .await
                .map_err(|error| CoreError::Protocol(error.to_string()))?;

            // Keep the row as a tombstone. `upsert_message` refreshes the text
            // on conflict; it does not write `kind` (see `store.rs`), so the
            // persisted kind stays at its previous value while the in-memory
            // row carries `Unsupported`.
            let mut tombstone = stored;
            tombstone.text = None;
            tombstone.kind = MessageKind::Unsupported;
            store.upsert_message(&tombstone)?;
        } else {
            let from_me = stored.from_me;
            let participant = group_participant(chat_id, Some(&stored), from_me)?;
            client
                .chat_actions()
                .delete_message_for_me(
                    &to,
                    participant.as_ref(),
                    message_id,
                    from_me,
                    false,
                    delete_timestamp(&stored),
                )
                .await
                .map_err(|error| CoreError::Protocol(error.to_string()))?;
        }
        Ok(())
    }

    /// Star or unstar a message.
    pub async fn star_message(
        &self,
        chat_id: &Jid,
        message_id: &str,
        from_me: bool,
        star: bool,
    ) -> Result<()> {
        let client = self.client().await?;
        let to = to_upstream(chat_id)?;
        let stored = lookup_group_sender(&self.store(), chat_id, message_id, from_me)?;
        let participant = group_participant(chat_id, stored.as_ref(), from_me)?;

        let actions = client.chat_actions();
        let result = if star {
            actions
                .star_message(&to, participant.as_ref(), message_id, from_me)
                .await
        } else {
            actions
                .unstar_message(&to, participant.as_ref(), message_id, from_me)
                .await
        };
        result.map_err(|error| CoreError::Protocol(error.to_string()))?;
        // The schema has no `starred` column, so the app-state mutation above
        // is the only persistence: the flag syncs across our devices but is not
        // stored in the local `messages` table.
        Ok(())
    }
}

/// Inputs resolved from a stored message for [`build_quote_context_with_info`].
#[derive(Debug)]
struct QuoteInputs {
    message_id: String,
    sender: whatsapp_rust::Jid,
    quoted_chat: whatsapp_rust::Jid,
    body: wa::Message,
}

/// Resolve quote-context inputs from a stored message.
///
/// The store keeps metadata, not protobufs, so `body` is a best-effort
/// reconstruction: plain text round-trips exactly, everything else degrades to
/// an empty body (recipients render the quote without its content preview).
fn quote_inputs(quoted: &Message, own_jid: Option<&Jid>) -> Result<QuoteInputs> {
    Ok(QuoteInputs {
        message_id: quoted.id.clone(),
        sender: quote_sender(quoted, own_jid)?,
        quoted_chat: to_upstream(&quoted.chat_id)?,
        body: quoted_body(quoted),
    })
}

/// Author JID for a quote context.
///
/// Own rows may carry the `me` placeholder (history sync, sends before the
/// account JID was known); the account JID is preferred when we have it.
fn quote_sender(quoted: &Message, own_jid: Option<&Jid>) -> Result<whatsapp_rust::Jid> {
    if quoted.from_me
        && let Some(own_jid) = own_jid
    {
        return to_upstream(own_jid);
    }
    if quoted.sender_id.as_str() == ME_PLACEHOLDER {
        return Err(CoreError::Protocol(
            "cannot quote this message before the account JID is known".into(),
        ));
    }
    to_upstream(&quoted.sender_id)
}

/// Reconstruct the quotable body of a stored message.
fn quoted_body(quoted: &Message) -> wa::Message {
    match quoted.kind {
        MessageKind::Text => quoted
            .text
            .as_deref()
            .map(wa::Message::text)
            .unwrap_or_default(),
        _ => wa::Message::default(),
    }
}

/// Build the target [`wa::MessageKey`] of a message in `chat`.
///
/// `participant` is the original author for group messages we did not send and
/// `None` otherwise (the `from_me` flag then identifies the author).
fn message_key(
    chat_id: &Jid,
    message_id: &str,
    from_me: bool,
    participant: Option<whatsapp_rust::Jid>,
) -> wa::MessageKey {
    wa::MessageKey {
        remote_jid: Some(chat_id.as_str().to_owned()),
        from_me: Some(from_me),
        id: Some(message_id.to_owned()),
        participant: participant.map(|jid| jid.to_string()),
    }
}

/// Resolve the `participant` field of a group message key.
///
/// WhatsApp requires the original author for group messages that are not ours;
/// 1:1 keys and our own messages identify the author through `from_me` alone.
fn group_participant(
    chat_id: &Jid,
    stored: Option<&Message>,
    from_me: bool,
) -> Result<Option<whatsapp_rust::Jid>> {
    if !chat_id.is_group() || from_me {
        return Ok(None);
    }
    let sender = stored.map(|message| &message.sender_id).ok_or_else(|| {
        CoreError::Protocol(
            "cannot resolve the original sender of a group message that is not stored".into(),
        )
    })?;
    to_upstream(sender).map(Some)
}

/// Load the target message when the key needs its author, saving a store read
/// for 1:1 chats and our own messages.
fn lookup_group_sender(
    store: &Store,
    chat_id: &Jid,
    message_id: &str,
    from_me: bool,
) -> Result<Option<Message>> {
    if chat_id.is_group() && !from_me {
        store.find_message(message_id)
    } else {
        Ok(None)
    }
}

/// Pick the revoke flavour for a stored message.
///
/// Our own messages use a sender revoke; group messages from others use an
/// admin revoke with the original author as participant.
fn revoke_type(chat_id: &Jid, stored: &Message) -> Result<RevokeType> {
    if chat_id.is_group() && !stored.from_me {
        Ok(RevokeType::Admin {
            original_sender: to_upstream(&stored.sender_id)?,
        })
    } else {
        Ok(RevokeType::Sender)
    }
}

/// `DeleteMessageForMeAction` carries epoch milliseconds; our rows store
/// seconds.
fn delete_timestamp(message: &Message) -> Option<i64> {
    i64::try_from(message.timestamp)
        .ok()
        .map(|seconds| seconds.saturating_mul(1000))
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
        }
    }

    fn group_message() -> Message {
        Message {
            chat_id: Jid::new("120363000000000001@g.us"),
            ..stored_message()
        }
    }

    fn upstream_jid(value: &str) -> whatsapp_rust::Jid {
        value.parse().expect("test JID parses")
    }

    #[test]
    fn message_key_has_all_fields_for_a_direct_chat() {
        let key = message_key(&Jid::new("5511999@s.whatsapp.net"), "MSG1", false, None);

        assert_eq!(key.remote_jid.as_deref(), Some("5511999@s.whatsapp.net"));
        assert_eq!(key.id.as_deref(), Some("MSG1"));
        assert_eq!(key.from_me, Some(false));
        assert_eq!(key.participant, None);
    }

    #[test]
    fn message_key_carries_the_participant_for_group_messages_from_others() {
        let key = message_key(
            &Jid::new("120363000000000001@g.us"),
            "MSG2",
            false,
            Some(upstream_jid("5511888@s.whatsapp.net")),
        );

        assert_eq!(key.remote_jid.as_deref(), Some("120363000000000001@g.us"));
        assert_eq!(key.id.as_deref(), Some("MSG2"));
        assert_eq!(key.from_me, Some(false));
        assert_eq!(key.participant.as_deref(), Some("5511888@s.whatsapp.net"));
    }

    #[test]
    fn message_key_omits_the_participant_for_our_own_group_messages() {
        let key = message_key(&Jid::new("120363000000000001@g.us"), "MSG3", true, None);

        assert_eq!(key.from_me, Some(true));
        assert_eq!(key.participant, None);
    }

    #[test]
    fn participant_is_none_for_direct_chats_and_our_own_messages() {
        let direct = stored_message();
        assert_eq!(
            group_participant(&direct.chat_id, Some(&direct), false).unwrap(),
            None
        );

        let group = group_message();
        assert_eq!(
            group_participant(&group.chat_id, Some(&group), true).unwrap(),
            None
        );
    }

    #[test]
    fn participant_is_the_sender_for_group_messages_from_others() {
        let message = group_message();
        let participant = group_participant(&message.chat_id, Some(&message), false)
            .unwrap()
            .expect("incoming group messages carry a participant");

        assert_eq!(participant.to_string(), "5511999@s.whatsapp.net");
    }

    #[test]
    fn participant_requires_a_stored_message_in_groups() {
        let error = group_participant(&Jid::new("120363000000000001@g.us"), None, false)
            .expect_err("a group key without an author must not be built");

        assert!(matches!(error, CoreError::Protocol(_)));
    }

    #[test]
    fn lookup_group_sender_only_reads_the_store_when_needed() {
        let store = Store::open_in_memory().expect("open store");
        let message = group_message();
        store
            .record_message_activity(&message.chat_id, "hello", message.timestamp, None, false)
            .expect("chat row");
        store.upsert_message(&message).expect("message row");

        // Incoming group message: the store supplies the author.
        let found =
            lookup_group_sender(&store, &message.chat_id, "MSG1", false).expect("lookup succeeds");
        assert_eq!(
            found.expect("stored").sender_id.as_str(),
            "5511999@s.whatsapp.net"
        );

        // Our own message: no lookup needed.
        assert!(
            lookup_group_sender(&store, &message.chat_id, "MSG1", true)
                .unwrap()
                .is_none()
        );

        // Direct chat: no lookup needed either.
        assert!(
            lookup_group_sender(&store, &stored_message().chat_id, "MSG1", false)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn quote_inputs_capture_id_sender_chat_and_body() {
        let quoted = stored_message();
        let inputs = quote_inputs(&quoted, None).expect("quote inputs");

        assert_eq!(inputs.message_id, "MSG1");
        assert_eq!(inputs.sender.to_string(), "5511999@s.whatsapp.net");
        assert_eq!(inputs.quoted_chat.to_string(), "5511999@s.whatsapp.net");
        assert_eq!(inputs.body.conversation.as_deref(), Some("hello"));
    }

    #[test]
    fn quote_context_wires_the_stored_metadata() {
        let quoted = stored_message();
        let inputs = quote_inputs(&quoted, None).expect("quote inputs");
        let target = upstream_jid("5511999@s.whatsapp.net");
        let context = build_quote_context_with_info(
            &inputs.message_id,
            &inputs.sender,
            &inputs.quoted_chat,
            &target,
            &inputs.body,
        );

        assert_eq!(context.stanza_id.as_deref(), Some("MSG1"));
        assert_eq!(
            context.participant.as_deref(),
            Some("5511999@s.whatsapp.net")
        );
        // Same-chat reply: WA Web omits `remote_jid`.
        assert_eq!(context.remote_jid, None);
        let body = context.quoted_message.into_option().expect("quoted body");
        assert_eq!(body.conversation.as_deref(), Some("hello"));
    }

    #[test]
    fn quote_context_marks_a_cross_chat_quote() {
        let quoted = stored_message();
        let inputs = quote_inputs(&quoted, None).expect("quote inputs");
        let target = upstream_jid("5521888@s.whatsapp.net");
        let context = build_quote_context_with_info(
            &inputs.message_id,
            &inputs.sender,
            &inputs.quoted_chat,
            &target,
            &inputs.body,
        );

        assert_eq!(
            context.remote_jid.as_deref(),
            Some("5511999@s.whatsapp.net")
        );
    }

    #[test]
    fn quote_sender_prefers_the_account_jid_for_own_messages() {
        let mut own = stored_message();
        own.from_me = true;
        own.sender_id = Jid::new(ME_PLACEHOLDER);

        let inputs =
            quote_inputs(&own, Some(&Jid::new("5511000@s.whatsapp.net"))).expect("quote inputs");

        assert_eq!(inputs.sender.to_string(), "5511000@s.whatsapp.net");
    }

    #[test]
    fn quote_sender_keeps_a_real_sender_without_an_account_jid() {
        let mut own = stored_message();
        own.from_me = true;

        let inputs = quote_inputs(&own, None).expect("quote inputs");

        assert_eq!(inputs.sender.to_string(), "5511999@s.whatsapp.net");
    }

    #[test]
    fn quote_sender_rejects_the_placeholder_without_an_account_jid() {
        let mut own = stored_message();
        own.from_me = true;
        own.sender_id = Jid::new(ME_PLACEHOLDER);

        let error = quote_inputs(&own, None).expect_err("no way to attribute the quote");
        assert!(matches!(error, CoreError::Protocol(_)));
    }

    #[test]
    fn quoted_body_is_empty_for_non_text_messages() {
        let mut image = stored_message();
        image.kind = MessageKind::Image;
        image.text = Some("a caption".to_owned());

        let body = quoted_body(&image);
        assert!(body.conversation.is_none());
        assert!(body.extended_text_message.is_unset());
    }

    #[test]
    fn revoke_type_uses_admin_for_group_messages_from_others() {
        let message = group_message();
        match revoke_type(&message.chat_id, &message).expect("revoke type") {
            RevokeType::Admin { original_sender } => {
                assert_eq!(original_sender.to_string(), "5511999@s.whatsapp.net");
            }
            other => panic!("expected an admin revoke, got {other:?}"),
        }
    }

    #[test]
    fn revoke_type_uses_sender_for_our_own_or_direct_messages() {
        let own = Message {
            from_me: true,
            ..group_message()
        };
        assert_eq!(
            revoke_type(&own.chat_id, &own).expect("revoke type"),
            RevokeType::Sender
        );

        let incoming = stored_message();
        assert_eq!(
            revoke_type(&incoming.chat_id, &incoming).expect("revoke type"),
            RevokeType::Sender
        );
    }

    #[test]
    fn delete_timestamp_converts_seconds_to_milliseconds() {
        assert_eq!(delete_timestamp(&stored_message()), Some(1_700_000_000_000));

        let absurd = Message {
            timestamp: u64::MAX,
            ..stored_message()
        };
        assert_eq!(delete_timestamp(&absurd), None);
    }

    #[test]
    fn text_preview_is_truncated_to_the_chat_list_budget() {
        assert_eq!(text_preview("hello"), "hello");
        assert_eq!(text_preview(&"x".repeat(500)).chars().count(), PREVIEW_MAX);
    }
}
