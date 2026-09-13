//! Status updates ("stories"): receive, list and view contacts' updates.
//!
//! # How statuses arrive
//!
//! A status update is an ordinary message sent to the `status@broadcast` JID:
//! `chat_id` is `status@broadcast` and `sender_id` is the contact who posted
//! it. Nothing else about the envelope is special, so ingest is a matter of
//! classifying the inbound [`Message`] (see [`is_status_chat`],
//! [`is_status_sender`] and [`status_update_from_message`]) and persisting a
//! compact row for the UI's 24-hour list.
//!
//! # Wiring (the parent's follow-up)
//!
//! `store.rs` owns the SQLite connection and its schema migrations, so the
//! command surface programs against [`StatusStore`]. Everything else is ready;
//! the follow-up is mechanical:
//!
//! 1. `store.rs`: bump [`crate::store::SCHEMA_VERSION`] to `6` and run
//!    `include_str!("schema/006_statuses.sql")` when `user_version < 6`. Then
//!    append the [`StatusStore`] implementation whose exact body is documented
//!    on the trait (it uses this module's `Store`-independent helpers).
//! 2. `client.rs`: call [`import_status_message`] from `handle_inbound_message`
//!    right after the raw protobuf is stored, for example:
//!
//!    ```ignore
//!    if let Err(error) = crate::statuses::import_status_message(
//!        store,
//!        &message,
//!        Some(&context.message.encode_to_vec()),
//!    ) {
//!        tracing::warn!(%error, "failed to store status update");
//!    }
//!    ```
//!
//!    (`store` is the `&Store` the handler already holds; `use
//!    crate::statuses::{StatusStore as _};` brings the trait methods into
//!    scope. [`WaClient::import_status`] is the same logic for callers that
//!    hold a client handle.)
//! 3. `lib.rs`: `pub mod statuses;` and, for the Tauri host,
//!    `pub use statuses::StatusUpdate;`.
//!
//! Until step 1 lands, [`WaClient::import_status`], [`WaClient::statuses`] and
//! [`WaClient::mark_status_viewed`] fail to compile against the missing
//! `impl StatusStore for Store`; the documented body below resolves it.

use serde::{Deserialize, Serialize};
use whatsapp_rust::buffa::Message as _;
use whatsapp_rust::proto_helpers::MessageExt;
use whatsapp_rust::waproto::whatsapp as wa;

use crate::client::WaClient;
use crate::error::Result;
use crate::media::MediaStore as _;
use crate::types::{Jid, Message, MessageKind};

/// JID of the status broadcast feed itself.
pub const STATUS_BROADCAST_JID: &str = "status@broadcast";

/// How long a status update stays visible, in seconds (WhatsApp's 24 h TTL).
pub const STATUS_TTL_SECS: u64 = 86_400;

/// True when `jid` is the status broadcast feed (`status@broadcast`), i.e. the
/// chat an update was posted to. This is stricter than
/// [`Jid::is_status`], which matches any `@broadcast` server.
pub fn is_status_chat(jid: &Jid) -> bool {
    jid.as_str() == STATUS_BROADCAST_JID
}

/// True when `jid` may be the *author* of a status update. The broadcast feed
/// itself is the destination, never the sender, so it is excluded.
pub fn is_status_sender(jid: &Jid) -> bool {
    !is_status_chat(jid)
}

/// Coarse kind of a status update, serialized lower-case for the UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatusKind {
    /// Plain text on a colored background.
    Text,
    /// Photo, possibly with a caption.
    Image,
    /// Video, possibly with a caption.
    Video,
    /// Recorded voice note.
    Voice,
    /// A kind this build does not render yet (audio, stickers, documents, …).
    Unknown,
}

impl StatusKind {
    /// Stable string used in the `statuses.kind` column and over IPC.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Image => "image",
            Self::Video => "video",
            Self::Voice => "voice",
            Self::Unknown => "unknown",
        }
    }

    /// Parse a value stored by [`StatusKind::as_str`]; unknown strings map to
    /// [`StatusKind::Unknown`] so a future build's rows never break the list.
    pub fn from_stored(value: &str) -> Self {
        match value {
            "text" => Self::Text,
            "image" => Self::Image,
            "video" => Self::Video,
            "voice" => Self::Voice,
            _ => Self::Unknown,
        }
    }
}

/// Map the message classifier's kind onto the status kind.
pub fn status_kind(kind: MessageKind) -> StatusKind {
    match kind {
        MessageKind::Text => StatusKind::Text,
        MessageKind::Image => StatusKind::Image,
        MessageKind::Video | MessageKind::Gif => StatusKind::Video,
        MessageKind::VoiceNote => StatusKind::Voice,
        _ => StatusKind::Unknown,
    }
}

/// One status update as the UI consumes it. `expires_at` is derived from
/// `timestamp` ([`status_expires_at`]) and never stored separately.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusUpdate {
    /// Protocol message id.
    pub id: String,
    /// Contact who posted the update (`status@broadcast` never appears here).
    pub sender: Jid,
    /// Unix seconds when the update was posted.
    pub timestamp: u64,
    /// Content kind.
    pub kind: StatusKind,
    /// Text body or caption when applicable.
    pub text: Option<String>,
    /// 0xAARRGGBB background color of a text status.
    pub background_argb: Option<u32>,
    /// Unix seconds when the update drops out of the 24 h window.
    pub expires_at: u64,
    /// Local read state; backs the subtle ring change in the list.
    pub viewed: bool,
}

/// Unix seconds at which `timestamp` falls out of the 24 h window.
pub fn status_expires_at(timestamp: u64) -> u64 {
    timestamp.saturating_add(STATUS_TTL_SECS)
}

/// Map an inbound message onto a status update, or `None` when it is not one.
///
/// A status is a message posted to `status@broadcast` by a contact. Messages
/// this account sent are skipped: they belong to "My status", not the Recent
/// list (and the broadcast feed itself can never be the author). The
/// background color is not part of [`Message`]; `raw_proto` — the serialized
/// `wa::Message`, when the caller has it — fills it in via
/// [`status_background_argb`].
pub fn status_update_from_message(
    message: &Message,
    raw_proto: Option<&[u8]>,
) -> Option<StatusUpdate> {
    if !is_status_chat(&message.chat_id)
        || !is_status_sender(&message.sender_id)
        || message.from_me
    {
        return None;
    }

    Some(StatusUpdate {
        id: message.id.clone(),
        sender: message.sender_id.clone(),
        timestamp: message.timestamp,
        kind: status_kind(message.kind),
        text: message.text.clone(),
        background_argb: status_background_argb(raw_proto),
        expires_at: status_expires_at(message.timestamp),
        viewed: false,
    })
}

/// Background color of a text status, decoded from its serialized protobuf.
/// `None` when the payload has no `extendedTextMessage.background_argb` (or the
/// protobuf is unreadable).
pub fn status_background_argb(raw_proto: Option<&[u8]>) -> Option<u32> {
    let raw_proto = raw_proto?;
    let message = wa::Message::decode_from_slice(raw_proto).ok()?;
    message
        .get_base_message()
        .extended_text_message
        .as_option()
        .and_then(|extended| extended.background_argb)
}

/// The persistence surface statuses need.
///
/// `store.rs` owns the SQLite connection and its migrations, so the command
/// surface programs against this trait. The exact implementation (uses
/// `Store`'s private `lock()` / `storage_error()` / `as_i64()` helpers):
///
/// ```ignore
/// // store.rs, appended next to the `impl MediaStore for Store` block:
/// use crate::statuses::{STATUS_TTL_SECS, StatusKind, StatusStore, StatusUpdate};
///
/// impl StatusStore for Store {
///     fn upsert_status(&self, update: &StatusUpdate) -> Result<()> {
///         let connection = self.lock()?;
///         connection
///             .execute(
///                 "INSERT INTO statuses (
///                      id, sender, timestamp, kind, text, background_argb, viewed
///                  ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
///                  ON CONFLICT(id) DO UPDATE SET
///                      sender = excluded.sender,
///                      timestamp = excluded.timestamp,
///                      kind = excluded.kind,
///                      text = excluded.text,
///                      background_argb = excluded.background_argb",
///                 params![
///                     &update.id,
///                     update.sender.as_str(),
///                     as_i64(update.timestamp),
///                     update.kind.as_str(),
///                     update.text.as_deref(),
///                     update.background_argb.map(|value| value as i64),
///                     i64::from(update.viewed),
///                 ],
///             )
///             .map_err(storage_error)?;
///         Ok(())
///     }
///
///     fn list_statuses(&self, now: u64) -> Result<Vec<StatusUpdate>> {
///         let connection = self.lock()?;
///         let mut statement = connection
///             .prepare(
///                 "SELECT id, sender, timestamp, kind, text, background_argb, viewed
///                  FROM statuses
///                  WHERE timestamp >= ?1
///                  ORDER BY timestamp DESC",
///             )
///             .map_err(storage_error)?;
///         let rows = statement
///             .query_map(
///                 params![as_i64(now.saturating_sub(STATUS_TTL_SECS))],
///                 |row| {
///                     let timestamp = row.get::<_, i64>(2)? as u64;
///                     Ok(StatusUpdate {
///                         id: row.get(0)?,
///                         sender: Jid::new(row.get::<_, String>(1)?),
///                         timestamp,
///                         kind: StatusKind::from_stored(row.get::<_, String>(3)?.as_str()),
///                         text: row.get(4)?,
///                         background_argb: row
///                             .get::<_, Option<i64>>(5)?
///                             .map(|value| value as u32),
///                         expires_at: timestamp.saturating_add(STATUS_TTL_SECS),
///                         viewed: row.get::<_, i64>(6)? != 0,
///                     })
///                 },
///             )
///             .map_err(storage_error)?;
///         rows.collect::<std::result::Result<Vec<_>, _>>()
///             .map_err(storage_error)
///     }
///
///     fn mark_status_viewed(&self, id: &str) -> Result<()> {
///         let connection = self.lock()?;
///         connection
///             .execute("UPDATE statuses SET viewed = 1 WHERE id = ?1", params![id])
///             .map_err(storage_error)?;
///         Ok(())
///     }
///
///     fn prune_statuses(&self, before: u64) -> Result<u64> {
///         let connection = self.lock()?;
///         let deleted = connection
///             .execute(
///                 "DELETE FROM statuses WHERE timestamp < ?1",
///                 params![as_i64(before)],
///             )
///             .map_err(storage_error)?;
///         Ok(deleted as u64)
///     }
/// }
/// ```
pub trait StatusStore: Send + Sync {
    /// Insert a status update, or refresh the row when `id` already exists
    /// (keeps the local `viewed` flag).
    fn upsert_status(&self, update: &StatusUpdate) -> Result<()>;

    /// Updates posted within [`STATUS_TTL_SECS`] of `now`, newest first.
    fn list_statuses(&self, now: u64) -> Result<Vec<StatusUpdate>>;

    /// Mark one update as viewed.
    fn mark_status_viewed(&self, id: &str) -> Result<()>;

    /// Delete updates older than `before`; returns the number of rows removed.
    fn prune_statuses(&self, before: u64) -> Result<u64>;
}

/// Store one inbound message when it is a status; `Ok(false)` for everything
/// else. This is the shared implementation of [`WaClient::import_status`] and
/// the `handle_inbound_message` hook, which only has the store at hand.
pub fn import_status_message(
    store: &dyn StatusStore,
    message: &Message,
    raw_proto: Option<&[u8]>,
) -> Result<bool> {
    let Some(update) = status_update_from_message(message, raw_proto) else {
        return Ok(false);
    };
    store.upsert_status(&update)?;
    Ok(true)
}

impl WaClient {
    /// Persist an inbound message when it is a status update. Returns `true`
    /// when the message was a status and got stored, `false` otherwise.
    ///
    /// The background color comes from the message's stored raw protobuf, so
    /// the hook in `handle_inbound_message` must run after
    /// `Store::set_raw_proto`.
    pub fn import_status(&self, message: &Message) -> Result<bool> {
        let store = self.store();
        let raw_proto = store.raw_proto(&message.id)?;
        import_status_message(store.as_ref(), message, raw_proto.as_deref())
    }

    /// Status updates young enough to show, newest first. Also prunes rows
    /// that fell out of the 24 h window.
    pub fn statuses(&self) -> Result<Vec<StatusUpdate>> {
        let store = self.store();
        let now = now_unix();
        store.prune_statuses(now.saturating_sub(STATUS_TTL_SECS))?;
        store.list_statuses(now)
    }

    /// Mark one status update as viewed.
    pub fn mark_status_viewed(&self, id: &str) -> Result<()> {
        self.store().mark_status_viewed(id)
    }
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
    use std::sync::Mutex;
    use whatsapp_rust::buffa::MessageField;

    fn status_message(kind: MessageKind, sender: &str) -> Message {
        Message {
            id: "status-1".to_owned(),
            chat_id: Jid::new(STATUS_BROADCAST_JID),
            sender_id: Jid::new(sender),
            from_me: false,
            timestamp: 1_700_000_000,
            kind,
            text: Some("hello".to_owned()),
            status: crate::types::MessageStatus::Delivered,
            view_once: false,
        }
    }

    #[test]
    fn classifies_the_status_feed() {
        assert!(is_status_chat(&Jid::new("status@broadcast")));
        assert!(!is_status_chat(&Jid::new("alice@s.whatsapp.net")));
        assert!(!is_status_chat(&Jid::new("123@broadcast")));

        assert!(is_status_sender(&Jid::new("alice@s.whatsapp.net")));
        assert!(!is_status_sender(&Jid::new("status@broadcast")));
    }

    #[test]
    fn maps_message_kinds_onto_status_kinds() {
        assert_eq!(status_kind(MessageKind::Text), StatusKind::Text);
        assert_eq!(status_kind(MessageKind::Image), StatusKind::Image);
        assert_eq!(status_kind(MessageKind::Video), StatusKind::Video);
        assert_eq!(status_kind(MessageKind::Gif), StatusKind::Video);
        assert_eq!(status_kind(MessageKind::VoiceNote), StatusKind::Voice);
        assert_eq!(status_kind(MessageKind::Audio), StatusKind::Unknown);
        assert_eq!(status_kind(MessageKind::Document), StatusKind::Unknown);
    }

    #[test]
    fn expires_after_the_24_hour_window() {
        assert_eq!(status_expires_at(1_700_000_000), 1_700_086_400);
        assert_eq!(status_expires_at(u64::MAX), u64::MAX);
    }

    #[test]
    fn maps_status_messages_and_rejects_everything_else() {
        let update = status_update_from_message(
            &status_message(MessageKind::Image, "alice@s.whatsapp.net"),
            None,
        )
        .expect("status message");
        assert_eq!(update.id, "status-1");
        assert_eq!(update.sender, Jid::new("alice@s.whatsapp.net"));
        assert_eq!(update.timestamp, 1_700_000_000);
        assert_eq!(update.kind, StatusKind::Image);
        assert_eq!(update.text.as_deref(), Some("hello"));
        assert_eq!(update.expires_at, 1_700_086_400);
        assert!(!update.viewed);

        // A regular chat message is not a status.
        let mut chat = status_message(MessageKind::Text, "alice@s.whatsapp.net");
        chat.chat_id = Jid::new("alice@s.whatsapp.net");
        assert!(status_update_from_message(&chat, None).is_none());

        // The broadcast feed can never be the author.
        assert!(
            status_update_from_message(
                &status_message(MessageKind::Text, STATUS_BROADCAST_JID),
                None,
            )
            .is_none()
        );

        // Our own status belongs to "My status", not the Recent list.
        let mut own = status_message(MessageKind::Text, "me@s.whatsapp.net");
        own.from_me = true;
        assert!(status_update_from_message(&own, None).is_none());
    }

    #[test]
    fn reads_the_background_color_from_the_raw_protobuf() {
        let proto = wa::Message {
            extended_text_message: MessageField::some(wa::message::ExtendedTextMessage {
                text: Some("colored status".to_owned()),
                background_argb: Some(0xFF21C063),
                ..Default::default()
            }),
            ..Default::default()
        };
        let raw = proto.encode_to_vec();
        assert_eq!(status_background_argb(Some(&raw)), Some(0xFF21C063));

        let plain = wa::Message {
            conversation: Some("no background".to_owned()),
            ..Default::default()
        };
        assert_eq!(status_background_argb(Some(&plain.encode_to_vec())), None);
        assert_eq!(status_background_argb(None), None);
        assert_eq!(status_background_argb(Some(b"not a protobuf")), None);
    }

    #[test]
    fn kind_round_trips_through_its_stored_form() {
        for kind in [
            StatusKind::Text,
            StatusKind::Image,
            StatusKind::Video,
            StatusKind::Voice,
            StatusKind::Unknown,
        ] {
            assert_eq!(StatusKind::from_stored(kind.as_str()), kind);
        }
        assert_eq!(StatusKind::from_stored("future-kind"), StatusKind::Unknown);
    }

    /// Minimal in-memory [`StatusStore`] so the import path is testable
    /// without the SQLite `Store`.
    #[derive(Default)]
    struct RecordingStore(Mutex<Vec<StatusUpdate>>);

    impl StatusStore for RecordingStore {
        fn upsert_status(&self, update: &StatusUpdate) -> Result<()> {
            self.0.lock().unwrap().push(update.clone());
            Ok(())
        }

        fn list_statuses(&self, _now: u64) -> Result<Vec<StatusUpdate>> {
            Ok(self.0.lock().unwrap().clone())
        }

        fn mark_status_viewed(&self, _id: &str) -> Result<()> {
            Ok(())
        }

        fn prune_statuses(&self, _before: u64) -> Result<u64> {
            Ok(0)
        }
    }

    #[test]
    fn imports_only_status_messages_and_enriches_the_background() {
        let store = RecordingStore::default();
        let proto = wa::Message {
            extended_text_message: MessageField::some(wa::message::ExtendedTextMessage {
                text: Some("hello".to_owned()),
                background_argb: Some(0xFFF15C6D),
                ..Default::default()
            }),
            ..Default::default()
        };
        let raw = proto.encode_to_vec();

        let stored = import_status_message(
            &store,
            &status_message(MessageKind::Text, "alice@s.whatsapp.net"),
            Some(&raw),
        )
        .expect("import");
        assert!(stored);

        let rows = store.list_statuses(0).expect("list");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].background_argb, Some(0xFFF15C6D));

        // A regular chat message is not imported.
        let mut chat = status_message(MessageKind::Text, "alice@s.whatsapp.net");
        chat.chat_id = Jid::new("alice@s.whatsapp.net");
        assert!(!import_status_message(&store, &chat, None).expect("import"));
        assert_eq!(store.list_statuses(0).expect("list").len(), 1);
    }
}
