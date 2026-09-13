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
    pub fn is_group(&self) -> bool {
        self.server() == Some("g.us")
    }

    /// True for the status broadcast feed.
    pub fn is_status(&self) -> bool {
        self.server() == Some("broadcast")
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
