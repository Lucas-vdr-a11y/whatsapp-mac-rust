//! Channels (newsletters) and status updates.

use whatsapp_rust::wacore_binary::JidExt as _;
use whatsapp_rust::waproto::whatsapp as wa;
use whatsapp_rust::{NewsletterRole, StatusSendOptions};

use crate::client::{WaClient, from_upstream, to_upstream};
use crate::error::{CoreError, Result};
use crate::types::{ChatSummary, Jid};

impl WaClient {
    /// Follow a channel from its invite link.
    ///
    /// Only the public share-URL shape is accepted
    /// (`https://whatsapp.com/channel/<code>`, with or without `www.`);
    /// anything else fails with [`CoreError::InvalidInput`] before touching the
    /// network. The invite code is resolved to a channel JID first and that JID
    /// is then joined, mirroring the WhatsApp Web / Baileys two-step flow.
    pub async fn follow_channel(&self, invite_url: &str) -> Result<Jid> {
        let invite_code = channel_invite_code(invite_url).ok_or_else(|| {
            CoreError::InvalidInput(format!("not a WhatsApp channel invite link: {invite_url}"))
        })?;

        let client = self.client().await?;
        let newsletter = client.newsletter();
        let metadata = newsletter
            .get_metadata_by_invite(invite_code)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;
        let joined = newsletter
            .join(&metadata.jid)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;

        Ok(from_upstream(&joined.jid))
    }

    /// Unfollow (leave) a channel.
    pub async fn unfollow_channel(&self, chat_id: &Jid) -> Result<()> {
        let jid = to_upstream(chat_id)?;
        if !jid.is_newsletter() {
            return Err(CoreError::InvalidInput(format!(
                "not a channel JID: {chat_id}"
            )));
        }

        let client = self.client().await?;
        client
            .newsletter()
            .leave(&jid)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))
    }

    /// Send a text post to a channel we administer.
    ///
    /// Posting requires the owner/admin role; the upstream metadata's viewer
    /// role is checked first and a non-admin post fails with
    /// [`CoreError::Protocol`]. The post itself goes out as a plaintext SMAX
    /// stanza (channels are not end-to-end encrypted) and is recorded in the
    /// local store like any other message.
    pub async fn send_channel_message(&self, chat_id: &Jid, text: &str) -> Result<()> {
        let jid = to_upstream(chat_id)?;
        if !jid.is_newsletter() {
            return Err(CoreError::InvalidInput(format!(
                "not a channel JID: {chat_id}"
            )));
        }
        let text = text.trim();
        if text.is_empty() {
            return Err(CoreError::InvalidInput("message text is empty".into()));
        }

        let client = self.client().await?;
        let metadata = client
            .newsletter()
            .get_metadata(&jid)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;
        if let Some(role) = metadata.role.as_ref()
            && !matches!(role, NewsletterRole::Owner | NewsletterRole::Admin)
        {
            return Err(CoreError::Protocol(format!(
                "cannot post to channel {chat_id}: viewer role is {role:?}, owner or admin required"
            )));
        }

        // `send_text` detects the newsletter JID and uses the upstream
        // plaintext path; it also stores and publishes the local echo.
        self.send_text(chat_id, text).await.map(|_| ())
    }

    /// Post a plain-text status update.
    ///
    /// Privacy defaults to `contacts` (`StatusSendOptions::default()`), and the
    /// recipients are the account's known private (1:1) chats from the local
    /// store — the closest stand-in for an address book until contact sync
    /// lands. `background_argb` is the 0xAARRGGBB background colour.
    pub async fn post_status_text(&self, text: &str, background_argb: u32) -> Result<()> {
        let text = text.trim();
        if text.is_empty() {
            return Err(CoreError::InvalidInput("status text is empty".into()));
        }

        let recipients = status_recipients(&self.store().list_chats()?);
        if recipients.is_empty() {
            return Err(CoreError::InvalidInput(
                "no private chats available to receive a status update".into(),
            ));
        }

        let client = self.client().await?;
        client
            .status()
            .send_text(
                text,
                background_argb,
                wa::message::extended_text_message::FontType::SYSTEM,
                &recipients,
                StatusSendOptions::default(),
            )
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;
        Ok(())
    }
}

/// Extract the invite code from a WhatsApp channel share URL.
///
/// Accepted shapes:
/// - `https://whatsapp.com/channel/<code>`
/// - `https://www.whatsapp.com/channel/<code>`
///
/// Query strings, fragments and a trailing slash are ignored.
fn channel_invite_code(invite_url: &str) -> Option<&str> {
    let invite_url = invite_url.trim();
    let (scheme, remainder) = invite_url.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return None;
    }

    let (host, path) = remainder.split_once('/')?;
    if !host.eq_ignore_ascii_case("whatsapp.com") && !host.eq_ignore_ascii_case("www.whatsapp.com")
    {
        return None;
    }

    let code = path.strip_prefix("channel/")?;
    let code = code.split(['/', '?', '#']).next().unwrap_or(code);
    if code.is_empty() { None } else { Some(code) }
}

/// True for 1:1 chats, which are the only valid status recipients.
fn is_status_recipient(jid: &Jid) -> bool {
    matches!(jid.server(), Some("s.whatsapp.net" | "lid" | "c.us"))
}

/// Upstream user JIDs for the private chats in `chats`.
fn status_recipients(chats: &[ChatSummary]) -> Vec<whatsapp_rust::Jid> {
    chats
        .iter()
        .filter(|chat| is_status_recipient(&chat.id))
        .filter_map(|chat| to_upstream(&chat.id).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chat(id: &str) -> ChatSummary {
        ChatSummary {
            id: Jid::new(id),
            name: String::new(),
            last_message_preview: None,
            last_activity_ts: 0,
            unread_count: 0,
            muted: false,
            pinned: false,
            is_group: false,
            is_archived: false,
        }
    }

    #[test]
    fn parses_channel_invite_links() {
        assert_eq!(
            channel_invite_code("https://whatsapp.com/channel/0029VbABC123"),
            Some("0029VbABC123")
        );
        assert_eq!(
            channel_invite_code("https://www.whatsapp.com/channel/0029VbABC123/"),
            Some("0029VbABC123")
        );
        assert_eq!(
            channel_invite_code("  https://whatsapp.com/channel/0029VbABC123?ref=share  "),
            Some("0029VbABC123")
        );
    }

    #[test]
    fn rejects_non_channel_links() {
        assert_eq!(
            channel_invite_code("https://chat.whatsapp.com/AbCdEfGh"),
            None
        );
        assert_eq!(channel_invite_code("https://whatsapp.com/"), None);
        assert_eq!(channel_invite_code("https://whatsapp.com/channel/"), None);
        assert_eq!(channel_invite_code("whatsapp.com/channel/abc"), None);
        assert_eq!(channel_invite_code(""), None);
    }

    #[test]
    fn status_recipients_include_only_private_chats() {
        let chats = [
            chat("alice@s.whatsapp.net"),
            chat("999@lid"),
            chat("123-456@g.us"),
            chat("120363000000000001@newsletter"),
            chat("status@broadcast"),
        ];

        let recipients = status_recipients(&chats);

        assert_eq!(recipients.len(), 2);
        assert_eq!(recipients[0].to_string(), "alice@s.whatsapp.net");
        assert_eq!(recipients[1].to_string(), "999@lid");
    }
}
