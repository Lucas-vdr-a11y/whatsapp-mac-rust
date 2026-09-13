//! Persistent state.
//!
//! A single SQLite database (WAL mode) holds chats, messages and contacts.
//! All access goes through [`Store`], which is `Send + Sync` and cheap to
//! clone-share: the connection is guarded by a mutex, and every operation is a
//! short transaction. Media binaries are not stored in the database; only
//! metadata and local cache paths are.
//!
//! Schema migrations are driven by `PRAGMA user_version`: bump
//! [`SCHEMA_VERSION`] and append a migration function when the schema changes.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::{CoreError, Result};
use crate::types::{ChatSummary, Jid, Message, MessageKind, MessageStatus};

/// Current schema version. Bump together with `migrations()`.
pub const SCHEMA_VERSION: i64 = 1;

/// Thread-safe handle to the application database.
pub struct Store {
    connection: Mutex<Connection>,
}

impl Store {
    /// Open (creating if needed) the database at `path`.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| CoreError::Storage(error.to_string()))?;
        }
        let connection =
            Connection::open(path).map_err(|error| CoreError::Storage(error.to_string()))?;
        Self::from_connection(connection)
    }

    /// In-memory database, for tests.
    pub fn open_in_memory() -> Result<Self> {
        let connection =
            Connection::open_in_memory().map_err(|error| CoreError::Storage(error.to_string()))?;
        Self::from_connection(connection)
    }

    fn from_connection(connection: Connection) -> Result<Self> {
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(storage_error)?;
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(storage_error)?;

        let store = Self {
            connection: Mutex::new(connection),
        };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<()> {
        let connection = self.lock()?;
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(storage_error)?;

        if version < 1 {
            connection
                .execute_batch(include_str!("schema/001_init.sql"))
                .map_err(storage_error)?;
        }

        if version < SCHEMA_VERSION {
            connection
                .pragma_update(None, "user_version", SCHEMA_VERSION)
                .map_err(storage_error)?;
        }
        Ok(())
    }

    /// Re-runs migration logic; exists so integration tests can prove
    /// idempotency. Not part of the public API contract.
    #[doc(hidden)]
    pub fn migrate_for_tests(&self) -> Result<()> {
        self.migrate()
    }

    /// Insert or update a chat row.
    pub fn upsert_chat(&self, chat: &ChatSummary) -> Result<()> {
        let connection = self.lock()?;
        connection
            .execute(
                "INSERT INTO chats (
                     id, name, last_message_preview, last_activity_ts,
                     unread_count, muted, pinned, is_group, is_archived
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(id) DO UPDATE SET
                     name = excluded.name,
                     last_message_preview = excluded.last_message_preview,
                     last_activity_ts = excluded.last_activity_ts,
                     unread_count = excluded.unread_count,
                     muted = excluded.muted,
                     pinned = excluded.pinned,
                     is_group = excluded.is_group,
                     is_archived = excluded.is_archived",
                params![
                    chat.id.as_str(),
                    chat.name,
                    chat.last_message_preview,
                    as_i64(chat.last_activity_ts),
                    chat.unread_count,
                    chat.muted as i64,
                    chat.pinned as i64,
                    chat.is_group as i64,
                    chat.is_archived as i64,
                ],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    /// Insert or update a message row.
    pub fn upsert_message(&self, message: &Message) -> Result<()> {
        let connection = self.lock()?;
        connection
            .execute(
                "INSERT INTO messages (
                     id, chat_id, sender_id, from_me, timestamp, kind, text, status
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(id) DO UPDATE SET
                     status = excluded.status,
                     text = excluded.text",
                params![
                    message.id,
                    message.chat_id.as_str(),
                    message.sender_id.as_str(),
                    message.from_me as i64,
                    as_i64(message.timestamp),
                    kind_to_str(message.kind),
                    message.text,
                    status_to_str(message.status),
                ],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    /// All chats, pinned first, then most recent activity.
    pub fn list_chats(&self) -> Result<Vec<ChatSummary>> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT id, name, last_message_preview, last_activity_ts,
                        unread_count, muted, pinned, is_group, is_archived
                 FROM chats
                 ORDER BY pinned DESC, last_activity_ts DESC",
            )
            .map_err(storage_error)?;

        let rows = statement
            .query_map([], |row| {
                Ok(ChatSummary {
                    id: Jid::new(row.get::<_, String>(0)?),
                    name: row.get(1)?,
                    last_message_preview: row.get(2)?,
                    last_activity_ts: row.get::<_, i64>(3)? as u64,
                    unread_count: row.get::<_, i64>(4)? as u32,
                    muted: row.get::<_, i64>(5)? != 0,
                    pinned: row.get::<_, i64>(6)? != 0,
                    is_group: row.get::<_, i64>(7)? != 0,
                    is_archived: row.get::<_, i64>(8)? != 0,
                })
            })
            .map_err(storage_error)?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(storage_error)
    }

    /// Messages of one chat, oldest first. `limit` keeps memory bounded for
    /// very long histories; the UI paginates by asking for older offsets later.
    pub fn list_messages(&self, chat_id: &Jid, limit: u32) -> Result<Vec<Message>> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT id, chat_id, sender_id, from_me, timestamp, kind, text, status
                 FROM messages
                 WHERE chat_id = ?1
                 ORDER BY timestamp DESC, rowid DESC
                 LIMIT ?2",
            )
            .map_err(storage_error)?;

        let rows = statement
            .query_map(params![chat_id.as_str(), limit], row_to_message)
            .map_err(storage_error)?;

        let mut messages = rows
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(storage_error)?;
        messages.reverse();
        Ok(messages)
    }

    /// Update the delivery state of an outgoing message.
    pub fn set_message_status(&self, message_id: &str, status: MessageStatus) -> Result<()> {
        let connection = self.lock()?;
        connection
            .execute(
                "UPDATE messages SET status = ?1 WHERE id = ?2",
                params![status_to_str(status), message_id],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    /// Record chat activity after a message row was written.
    ///
    /// Creates the chat row when it does not exist yet (name from `name_hint`
    /// or the JID user part), updates the preview and activity timestamp, and
    /// optionally increments the unread counter.
    pub fn record_message_activity(
        &self,
        chat_id: &Jid,
        preview: &str,
        timestamp: u64,
        name_hint: Option<&str>,
        increment_unread: bool,
    ) -> Result<()> {
        let connection = self.lock()?;
        let fallback_name = chat_id.user().to_owned();
        let name_hint = name_hint.unwrap_or("");
        connection
            .execute(
                "INSERT INTO chats (
                     id, name, last_message_preview, last_activity_ts,
                     unread_count, muted, pinned, is_group, is_archived
                 ) VALUES (?1, CASE WHEN ?2 = '' THEN ?3 ELSE ?2 END, ?4, ?5, ?6, 0, 0, ?7, 0)
                 ON CONFLICT(id) DO UPDATE SET
                     name = CASE WHEN chats.name = '' THEN excluded.name ELSE chats.name END,
                     last_message_preview = excluded.last_message_preview,
                     last_activity_ts = excluded.last_activity_ts,
                     unread_count = CASE
                         WHEN ?8 THEN chats.unread_count + 1
                         ELSE chats.unread_count
                     END",
                params![
                    chat_id.as_str(),
                    name_hint,
                    fallback_name,
                    preview,
                    as_i64(timestamp),
                    i64::from(increment_unread),
                    i64::from(chat_id.is_group()),
                    i64::from(increment_unread),
                ],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    /// Number of stored messages, for diagnostics.
    pub fn message_count(&self) -> Result<u64> {
        let connection = self.lock()?;
        connection
            .query_row("SELECT COUNT(*) FROM messages", [], |row| {
                row.get::<_, i64>(0)
            })
            .map(|count| count as u64)
            .map_err(storage_error)
    }

    /// Look up a stored message by id.
    pub fn find_message(&self, message_id: &str) -> Result<Option<Message>> {
        let connection = self.lock()?;
        connection
            .query_row(
                "SELECT id, chat_id, sender_id, from_me, timestamp, kind, text, status
                 FROM messages WHERE id = ?1",
                params![message_id],
                row_to_message,
            )
            .optional()
            .map_err(storage_error)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.connection
            .lock()
            .map_err(|_| CoreError::Internal("database mutex poisoned".into()))
    }
}

fn row_to_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<Message> {
    Ok(Message {
        id: row.get(0)?,
        chat_id: Jid::new(row.get::<_, String>(1)?),
        sender_id: Jid::new(row.get::<_, String>(2)?),
        from_me: row.get::<_, i64>(3)? != 0,
        timestamp: row.get::<_, i64>(4)? as u64,
        kind: kind_from_str(row.get::<_, String>(5)?.as_str()),
        text: row.get(6)?,
        status: status_from_str(row.get::<_, String>(7)?.as_str()),
    })
}

fn storage_error(error: rusqlite::Error) -> CoreError {
    CoreError::Storage(error.to_string())
}

fn as_i64(value: u64) -> i64 {
    value.min(i64::MAX as u64) as i64
}

fn kind_to_str(kind: MessageKind) -> &'static str {
    match kind {
        MessageKind::Text => "text",
        MessageKind::Image => "image",
        MessageKind::Video => "video",
        MessageKind::Audio => "audio",
        MessageKind::VoiceNote => "voiceNote",
        MessageKind::Document => "document",
        MessageKind::Sticker => "sticker",
        MessageKind::Gif => "gif",
        MessageKind::Location => "location",
        MessageKind::Contact => "contact",
        MessageKind::Poll => "poll",
        MessageKind::System => "system",
        MessageKind::Unsupported => "unsupported",
    }
}

fn kind_from_str(value: &str) -> MessageKind {
    match value {
        "text" => MessageKind::Text,
        "image" => MessageKind::Image,
        "video" => MessageKind::Video,
        "audio" => MessageKind::Audio,
        "voiceNote" => MessageKind::VoiceNote,
        "document" => MessageKind::Document,
        "sticker" => MessageKind::Sticker,
        "gif" => MessageKind::Gif,
        "location" => MessageKind::Location,
        "contact" => MessageKind::Contact,
        "poll" => MessageKind::Poll,
        "system" => MessageKind::System,
        _ => MessageKind::Unsupported,
    }
}

fn status_to_str(status: MessageStatus) -> &'static str {
    match status {
        MessageStatus::Pending => "pending",
        MessageStatus::Sent => "sent",
        MessageStatus::Delivered => "delivered",
        MessageStatus::Read => "read",
        MessageStatus::Played => "played",
        MessageStatus::Failed => "failed",
    }
}

fn status_from_str(value: &str) -> MessageStatus {
    match value {
        "sent" => MessageStatus::Sent,
        "delivered" => MessageStatus::Delivered,
        "read" => MessageStatus::Read,
        "played" => MessageStatus::Played,
        "failed" => MessageStatus::Failed,
        _ => MessageStatus::Pending,
    }
}
