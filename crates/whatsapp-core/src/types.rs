//! Domain model shared between the core, the Tauri host and the UI.
//!
//! These types are serialized over IPC and stored in SQLite, so changes to
//! them are migrations. Keep additions backward compatible.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A WhatsApp address. Examples:
///
/// - `15551234567@s.whatsapp.net` — a person
/// - `1234567890-1234567890@g.us` — a group
/// - `status@broadcast` — the status feed
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Jid(pub String);

impl Jid {
    /// Wrap a raw JID string.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The raw string form.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The user part, e.g. the phone number or group id.
    pub fn user(&self) -> &str {
        self.0
            .split_once('@')
            .map_or(self.0.as_str(), |(user, _)| user)
    }

    /// The server part, e.g. `s.whatsapp.net` or `g.us`.
    pub fn server(&self) -> Option<&str> {
        self.0.split_once('@').map(|(_, server)| server)
    }

    /// True for group chats.
    ///
    /// Groups live on `g.us`, but LID-addressed groups surface as `@lid`
    /// JIDs whose user part sits in the group-id numbering space (see
    /// [`Jid::is_group_lid_form`]). Treating those as direct chats created
    /// parallel, misclassified rows beside the `@g.us` ones (audit S7).
    pub fn is_group(&self) -> bool {
        match self.server() {
            Some("g.us") => true,
            Some("lid") => self.is_group_lid_form(),
            _ => false,
        }
    }

    /// True for `@lid` JIDs in the group-id numbering space: all digits and
    /// at least 18 characters, the same ids `@g.us` uses (e.g. the 18-digit
    /// `120363…` form). Direct-chat LIDs are phone-derived and much shorter
    /// (15-16 digits at most), so the length check keeps them direct chats.
    pub fn is_group_lid_form(&self) -> bool {
        let user = self.user();
        user.len() >= 18 && !user.bytes().any(|byte| !byte.is_ascii_digit())
    }

    /// True for the status broadcast feed.
    pub fn is_status(&self) -> bool {
        self.server() == Some("broadcast")
    }

    /// True for a linked-identity address (`@lid`).
    pub fn is_lid(&self) -> bool {
        self.server() == Some("lid")
    }

    /// True when `name` is empty or just this JID's user part (a phone or
    /// LID number), i.e. not a real contact or group name.
    pub fn name_is_placeholder(&self, name: &str) -> bool {
        let trimmed = name.trim();
        trimmed.is_empty() || trimmed == self.user()
    }
}

impl fmt::Display for Jid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for Jid {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for Jid {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<Jid> for String {
    fn from(value: Jid) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_group_servers() {
        assert!(Jid::new("120363404062663307@g.us").is_group());
        assert!(!Jid::new("31657632048@s.whatsapp.net").is_group());
        assert!(!Jid::new("status@broadcast").is_group());
    }

    #[test]
    fn classifies_group_lid_forms_as_groups() {
        // Group-id numbering space: 18+ digits, digits only.
        assert!(Jid::new("120363404062663307@lid").is_group());
        assert!(Jid::new("120363404062663307@lid").is_group_lid_form());
        // Phone-derived direct-chat LIDs stay direct.
        assert!(!Jid::new("14083231338501@lid").is_group());
        assert!(!Jid::new("254970750308491@lid").is_group());
        assert!(!Jid::new("254970750308491@lid").is_group_lid_form());
        // 17 digits is still in direct space; the boundary is 18.
        assert!(!Jid::new("99999999999999999@lid").is_group());
        // A hyphenated legacy id can never be a group LID form.
        assert!(!Jid::new("31647820621-1537097334@lid").is_group());
    }

    #[test]
    fn lid_and_status_classifiers_keep_working() {
        assert!(Jid::new("31657632048@lid").is_lid());
        assert!(Jid::new("status@broadcast").is_status());
        assert_eq!(
            Jid::new("120363404062663307@lid").user(),
            "120363404062663307"
        );
    }
}

/// One row in the chat list.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSummary {
    /// Chat JID.
    pub id: Jid,
    /// Display name (contact name, group subject, …). Empty until resolved.
    pub name: String,
    /// Preview of the most recent message, already formatted for display.
    pub last_message_preview: Option<String>,
    /// Unix seconds of the most recent activity.
    pub last_activity_ts: u64,
    /// Unread message count.
    pub unread_count: u32,
    /// True when muted.
    pub muted: bool,
    /// True when pinned to the top of the list.
    pub pinned: bool,
    /// True for group chats.
    pub is_group: bool,
    /// True when archived by the user.
    pub is_archived: bool,
    /// Kind of the newest message, when known. Lets the UI render a
    /// localized preview label instead of parsing the preview string.
    #[serde(default)]
    pub last_message_kind: Option<MessageKind>,
    /// True when the newest message was sent by this account; drives the
    /// tick prefix the official client shows in the chat list.
    #[serde(default)]
    pub last_from_me: bool,
    /// Delivery status of the newest message. Blue double checks when read,
    /// grey otherwise — same as the official chat list.
    #[serde(default)]
    pub last_status: Option<MessageStatus>,
}

/// What a message contains. The UI switches on this enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MessageKind {
    /// Plain text.
    Text,
    /// Image, possibly with caption.
    Image,
    /// Video, possibly with caption.
    Video,
    /// Audio file.
    Audio,
    /// Recorded voice note.
    VoiceNote,
    /// Arbitrary document.
    Document,
    /// Sticker.
    Sticker,
    /// Animated GIF.
    Gif,
    /// Location or live location.
    Location,
    /// Shared contact card.
    Contact,
    /// Poll.
    Poll,
    /// Group/chat event (joins, subject changes, …).
    System,
    /// A message type this build does not render yet.
    Unsupported,
}

/// Lifecycle of a message we sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MessageStatus {
    /// Queued locally, not yet acknowledged by the server.
    Pending,
    /// Accepted by the server (single tick).
    Sent,
    /// Delivered to all recipient devices (double tick).
    Delivered,
    /// Read by at least one recipient (blue ticks).
    Read,
    /// Voice note listened to.
    Played,
    /// Permanently failed.
    Failed,
}

/// A chat message.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    /// Protocol message id.
    pub id: String,
    /// Chat this message belongs to.
    pub chat_id: Jid,
    /// Author of the message (equals [`Message::chat_id`] in 1:1 chats).
    pub sender_id: Jid,
    /// True when this account sent the message.
    pub from_me: bool,
    /// Unix seconds.
    pub timestamp: u64,
    /// Content type.
    pub kind: MessageKind,
    /// Text body or caption when applicable.
    pub text: Option<String>,
    /// Delivery state for outgoing messages.
    pub status: MessageStatus,
    /// True when the payload arrived in a view-once envelope
    /// (`viewOnceMessage` / `viewOnceMessageV2` / `viewOnceMessageV2Extension`,
    /// possibly nested under `deviceSentMessage`/`ephemeralMessage`) or carries
    /// the inline `view_once` flag on modern payloads.
    #[serde(default)]
    pub view_once: bool,
}
