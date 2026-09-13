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
pub const SCHEMA_VERSION: i64 = 9;

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

        if version < 2 {
            connection
                .execute_batch(include_str!("schema/002_media.sql"))
                .map_err(storage_error)?;
        }

        if version < 3 {
            connection
                .execute_batch(include_str!("schema/003_reactions.sql"))
                .map_err(storage_error)?;
        }

        if version < 4 {
            connection
                .execute_batch(include_str!("schema/004_starred.sql"))
                .map_err(storage_error)?;
        }

        if version < 5 {
            connection
                .execute_batch(include_str!("schema/005_call_log.sql"))
                .map_err(storage_error)?;
        }

        if version < 6 {
            connection
                .execute_batch(include_str!("schema/006_statuses.sql"))
                .map_err(storage_error)?;
        }

        if version < 7 {
            connection
                .execute_batch(include_str!("schema/007_preview_meta.sql"))
                .map_err(storage_error)?;
        }

        if version < 8 {
            connection
                .execute_batch(include_str!("schema/008_jid_aliases.sql"))
                .map_err(storage_error)?;
        }

        if version < 9 {
            connection
                .execute_batch(include_str!("schema/009_pending_reactions.sql"))
                .map_err(storage_error)?;
        }

        // Backfill the newest-message metadata for chats that predate it.
        connection
            .execute(
                "UPDATE chats SET
                     last_kind = COALESCE((
                         SELECT kind FROM messages
                         WHERE messages.chat_id = chats.id
                         ORDER BY timestamp DESC, rowid DESC LIMIT 1
                     ), last_kind),
                     last_from_me = COALESCE((
                         SELECT from_me FROM messages
                         WHERE messages.chat_id = chats.id
                         ORDER BY timestamp DESC, rowid DESC LIMIT 1
                     ), last_from_me)
                 WHERE EXISTS (
                     SELECT 1 FROM messages WHERE messages.chat_id = chats.id
                 )",
                [],
            )
            .map_err(storage_error)?;

        // Self-heal activity timestamps that older builds let history sync
        // regress. The chat list must sort by the newest message, so this
        // runs on every open and is cheap (idx_messages_chat_ts covers it).
        connection
            .execute(
                "UPDATE chats
                 SET last_activity_ts = (
                     SELECT MAX(timestamp) FROM messages
                     WHERE messages.chat_id = chats.id
                 )
                 WHERE EXISTS (
                     SELECT 1 FROM messages
                     WHERE messages.chat_id = chats.id
                       AND messages.timestamp > chats.last_activity_ts
                 )",
                [],
            )
            .map_err(storage_error)?;

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

    /// Merge a history-sync conversation into the chat list without clobbering
    /// better data:
    ///
    /// - a real name (group subject, contact name) replaces only empty or
    ///   numeric placeholder names; an empty history name never overwrites;
    /// - unread counts, pin/mute/archive flags and activity timestamps only
    ///   ever move forward, so live state survives a late history chunk.
    pub fn upsert_chat_from_history(
        &self,
        chat: &ChatSummary,
        name_is_real: bool,
        fallback_name: &str,
    ) -> Result<()> {
        let connection = self.lock()?;
        connection
            .execute(
                "INSERT INTO chats (
                     id, name, last_message_preview, last_activity_ts,
                     unread_count, muted, pinned, is_group, is_archived
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(id) DO UPDATE SET
                     name = CASE
                         WHEN ?10 AND (chats.name = '' OR chats.name = ?11)
                             THEN excluded.name
                         ELSE chats.name
                     END,
                     last_message_preview = COALESCE(
                         excluded.last_message_preview,
                         chats.last_message_preview
                     ),
                     last_activity_ts = MAX(
                         chats.last_activity_ts,
                         excluded.last_activity_ts
                     ),
                     -- Live read/pin/mute/archive patches own these flags.
                     -- A late history chunk must not re-inflate unread or
                     -- resurrect a flag the user (or another device) cleared.
                     unread_count = chats.unread_count,
                     muted = chats.muted,
                     pinned = chats.pinned,
                     is_archived = chats.is_archived",
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
                    i64::from(name_is_real),
                    fallback_name,
                ],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    /// Insert or update a message row.
    pub fn upsert_message(&self, message: &Message) -> Result<()> {
        let chat_id = self.canonical_jid(&message.chat_id)?;
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
                    chat_id.as_str(),
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
                        unread_count, muted, pinned, is_group, is_archived,
                        last_kind, last_from_me,
                        (SELECT status FROM messages
                          WHERE messages.chat_id = chats.id
                          ORDER BY timestamp DESC, rowid DESC LIMIT 1)
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
                    last_message_kind: row
                        .get::<_, Option<String>>(9)?
                        .as_deref()
                        .map(kind_from_str),
                    last_from_me: row.get::<_, i64>(10)? != 0,
                    last_status: row
                        .get::<_, Option<String>>(11)?
                        .as_deref()
                        .map(status_from_str),
                })
            })
            .map_err(storage_error)?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(storage_error)
    }

    /// Messages of one chat, oldest first. `limit` keeps memory bounded for
    /// very long histories; the UI paginates by asking for older offsets later.
    pub fn list_messages(&self, chat_id: &Jid, limit: u32) -> Result<Vec<Message>> {
        let canonical = self.canonical_jid(chat_id)?;
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT id, chat_id, sender_id, from_me, timestamp, kind, text, status
                 FROM messages
                 WHERE chat_id = ?1
                    OR chat_id IN (SELECT jid FROM jid_aliases WHERE canonical = ?1)
                 ORDER BY timestamp DESC, rowid DESC
                 LIMIT ?2",
            )
            .map_err(storage_error)?;

        let rows = statement
            .query_map(params![canonical.as_str(), limit], row_to_message)
            .map_err(storage_error)?;

        let mut messages = rows
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(storage_error)?;
        messages.reverse();
        Ok(messages)
    }

    /// Messages of one chat older than `before_id`, oldest first.
    ///
    /// Backwards pagination for the conversation view: the caller passes the
    /// oldest message it currently shows and receives the page before it.
    /// Ordering mirrors [`Store::list_messages`] (`timestamp DESC, rowid
    /// DESC`), so pages never overlap or skip rows with equal timestamps.
    pub fn messages_before(
        &self,
        chat_id: &Jid,
        before_id: &str,
        limit: u32,
    ) -> Result<Vec<Message>> {
        let canonical = self.canonical_jid(chat_id)?;
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT m.id, m.chat_id, m.sender_id, m.from_me, m.timestamp, m.kind, m.text, m.status
                 FROM messages m
                 JOIN messages anchor ON anchor.id = ?2
                 WHERE (m.chat_id = ?1
                    OR m.chat_id IN (SELECT jid FROM jid_aliases WHERE canonical = ?1))
                   AND (
                       m.timestamp < anchor.timestamp
                       OR (m.timestamp = anchor.timestamp AND m.rowid < anchor.rowid)
                   )
                 ORDER BY m.timestamp DESC, m.rowid DESC
                 LIMIT ?3",
            )
            .map_err(storage_error)?;

        let rows = statement
            .query_map(params![canonical.as_str(), before_id, limit], row_to_message)
            .map_err(storage_error)?;

        let mut messages = rows
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(storage_error)?;
        messages.reverse();
        Ok(messages)
    }

    /// Oldest stored message of a chat: `(id, from_me, timestamp_secs)`.
    ///
    /// Used as the anchor for on-demand history requests to the primary phone.
    pub fn oldest_message(&self, chat_id: &Jid) -> Result<Option<(String, bool, u64)>> {
        let canonical = self.canonical_jid(chat_id)?;
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT id, from_me, timestamp
                 FROM messages
                 WHERE chat_id = ?1
                    OR chat_id IN (SELECT jid FROM jid_aliases WHERE canonical = ?1)
                 ORDER BY timestamp ASC, rowid ASC
                 LIMIT 1",
            )
            .map_err(storage_error)?;

        let mut rows = statement
            .query_map(params![canonical.as_str()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)? != 0,
                    row.get::<_, i64>(2)? as u64,
                ))
            })
            .map_err(storage_error)?;

        match rows.next() {
            Some(row) => Ok(Some(row.map_err(storage_error)?)),
            None => Ok(None),
        }
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
        let chat_id = self.canonical_jid(chat_id)?;
        let connection = self.lock()?;
        let fallback_name = chat_id.user().to_owned();
        let name_hint = name_hint.unwrap_or("");
        connection
            .execute(
                "INSERT INTO chats (
                     id, name, last_message_preview, last_activity_ts,
                     unread_count, muted, pinned, is_group, is_archived
                 ) VALUES (
                     ?1,
                     CASE
                         WHEN ?2 != '' THEN ?2
                         ELSE COALESCE(
                             (SELECT name FROM contacts WHERE id = ?1),
                             ?3
                         )
                     END,
                     ?4, ?5, ?6, 0, 0, ?7, 0)
                 ON CONFLICT(id) DO UPDATE SET
                     name = CASE
                         -- A real push name replaces empty or numeric
                         -- placeholder names (JID user parts), but never
                         -- clobbers a better name.
                         WHEN ?2 != '' AND (chats.name = '' OR chats.name = ?3)
                             THEN ?2
                         ELSE chats.name
                     END,
                     last_message_preview = excluded.last_message_preview,
                     last_activity_ts = MAX(
                         chats.last_activity_ts,
                         excluded.last_activity_ts
                     ),
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

    /// Pin or unpin a chat. Returns `true` when the patch landed on a
    /// deferred chat row (created because history had not synced yet).
    pub fn set_chat_pinned(&self, chat_id: &Jid, pinned: bool) -> Result<bool> {
        self.set_chat_flag("pinned", chat_id, pinned)
    }

    /// Mute or unmute a chat. Returns `true` when the patch landed on a
    /// deferred chat row (created because history had not synced yet).
    pub fn set_chat_muted(&self, chat_id: &Jid, muted: bool) -> Result<bool> {
        self.set_chat_flag("muted", chat_id, muted)
    }

    /// Archive or unarchive a chat. Returns `true` when the patch landed on a
    /// deferred chat row (created because history had not synced yet).
    pub fn set_chat_archived(&self, chat_id: &Jid, archived: bool) -> Result<bool> {
        self.set_chat_flag("is_archived", chat_id, archived)
    }

    /// Clear the unread counter of a chat. The canonical id is resolved first
    /// so alias rows land on the same chat, and a missing row is created on
    /// the fly so read patches that arrive before history sync survive.
    /// Returns `true` when a deferred chat row was created.
    pub fn mark_chat_read(&self, chat_id: &Jid) -> Result<bool> {
        let chat_id = self.canonical_jid(chat_id)?;
        let connection = self.lock()?;
        let deferred = !chat_row_exists(&connection, chat_id.as_str())?;
        connection
            .execute(
                "INSERT INTO chats (id, name, last_activity_ts, unread_count)
                 VALUES (?1, '', 0, 0)
                 ON CONFLICT(id) DO UPDATE SET unread_count = 0",
                params![chat_id.as_str()],
            )
            .map_err(storage_error)?;
        Ok(deferred)
    }

    /// Mark a chat unread (the phone's `MarkChatAsRead` with `read = false`).
    /// Keeps at least one unread message so the badge stays visible; the exact
    /// count is restored by history sync. Returns `true` when a deferred chat
    /// row was created.
    pub fn mark_chat_unread(&self, chat_id: &Jid) -> Result<bool> {
        let chat_id = self.canonical_jid(chat_id)?;
        let connection = self.lock()?;
        let deferred = !chat_row_exists(&connection, chat_id.as_str())?;
        connection
            .execute(
                "INSERT INTO chats (id, name, last_activity_ts, unread_count)
                 VALUES (?1, '', 0, 1)
                 ON CONFLICT(id) DO UPDATE SET unread_count = MAX(chats.unread_count, 1)",
                params![chat_id.as_str()],
            )
            .map_err(storage_error)?;
        Ok(deferred)
    }

    /// `column` is a hard-coded caller-supplied name, never user input.
    ///
    /// The canonical id is resolved first so alias rows land on the same chat,
    /// and a missing row is created on the fly so flag patches that arrive
    /// before history sync survive. Returns `true` when a deferred chat row
    /// was created.
    fn set_chat_flag(&self, column: &str, chat_id: &Jid, value: bool) -> Result<bool> {
        let chat_id = self.canonical_jid(chat_id)?;
        let sql = match column {
            "pinned" | "muted" | "is_archived" => format!(
                "INSERT INTO chats (id, name, last_activity_ts, {column})
                 VALUES (?1, '', 0, ?2)
                 ON CONFLICT(id) DO UPDATE SET {column} = ?2"
            ),
            other => {
                return Err(CoreError::Internal(format!(
                    "unknown chat flag column: {other}"
                )));
            }
        };
        let connection = self.lock()?;
        let deferred = !chat_row_exists(&connection, chat_id.as_str())?;
        connection
            .execute(&sql, params![chat_id.as_str(), i64::from(value)])
            .map_err(storage_error)?;
        Ok(deferred)
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

    /// Search message text, newest first. `%` and `_` are treated literally.
    pub fn search_messages(&self, query: &str, limit: u32) -> Result<Vec<Message>> {
        let escaped = query
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("%{escaped}%");
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT id, chat_id, sender_id, from_me, timestamp, kind, text, status
                 FROM messages
                 WHERE text LIKE ?1 ESCAPE '\\'
                 ORDER BY timestamp DESC, rowid DESC
                 LIMIT ?2",
            )
            .map_err(storage_error)?;
        let rows = statement
            .query_map(params![pattern, limit], row_to_message)
            .map_err(storage_error)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(storage_error)
    }

    /// Insert or update a contact's names, keeping the strongest value.
    pub fn upsert_contact(
        &self,
        jid: &Jid,
        name: Option<&str>,
        push_name: Option<&str>,
    ) -> Result<()> {
        let connection = self.lock()?;
        connection
            .execute(
                "INSERT INTO contacts (id, name, push_name) VALUES (?1, ?2, ?3)
                 ON CONFLICT(id) DO UPDATE SET
                     name = COALESCE(excluded.name, contacts.name),
                     push_name = COALESCE(excluded.push_name, contacts.push_name)",
                params![jid.as_str(), name, push_name],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    /// The stored contact name for `jid`, if any.
    pub fn contact_name(&self, jid: &Jid) -> Result<Option<String>> {
        let connection = self.lock()?;
        connection
            .query_row(
                "SELECT COALESCE(name, push_name) FROM contacts WHERE id = ?1",
                params![jid.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map(Option::flatten)
            .map_err(storage_error)
    }

    /// How many address-book contacts have been received.
    pub fn contact_count(&self) -> Result<i64> {
        let connection = self.lock()?;
        connection
            .query_row("SELECT COUNT(*) FROM contacts", [], |row| row.get(0))
            .map_err(storage_error)
    }

    /// Group chats whose name is still a numeric placeholder.
    pub fn chats_needing_group_names(&self, limit: u32) -> Result<Vec<Jid>> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT id FROM chats
                 WHERE is_group = 1
                   AND (name = '' OR name = substr(id, 1, instr(id, '@') - 1))
                 ORDER BY last_activity_ts DESC
                 LIMIT ?1",
            )
            .map_err(storage_error)?;
        let rows = statement
            .query_map(params![limit], |row| row.get::<_, String>(0))
            .map_err(storage_error)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map(|ids| ids.into_iter().map(Jid::new).collect())
            .map_err(storage_error)
    }

    /// Record the newest message's kind and direction for the chat-list row
    /// (tick prefix and localized media labels). The chat row is created on
    /// the fly when it does not exist yet, mirroring the flag writers.
    pub fn set_chat_last_message_meta(
        &self,
        chat_id: &Jid,
        kind: MessageKind,
        from_me: bool,
    ) -> Result<()> {
        let chat_id = self.canonical_jid(chat_id)?;
        let connection = self.lock()?;
        connection
            .execute(
                "INSERT INTO chats (id, name, last_activity_ts, last_kind, last_from_me)
                 VALUES (?1, '', 0, ?2, ?3)
                 ON CONFLICT(id) DO UPDATE SET
                     last_kind = ?2,
                     last_from_me = ?3",
                params![chat_id.as_str(), kind_to_str(kind), i64::from(from_me)],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    /// Rename a chat (group subjects, manual renames). Returns true when the
    /// row actually changed.
    pub fn rename_chat(&self, jid: &Jid, name: &str) -> Result<bool> {
        let connection = self.lock()?;
        let changed = connection
            .execute(
                "UPDATE chats SET name = ?2 WHERE id = ?1 AND name != ?2",
                params![jid.as_str(), name],
            )
            .map_err(storage_error)?;
        Ok(changed > 0)
    }

    /// Apply an address-book name: store it as the contact name and rename any
    /// existing chat row (address-book names are authoritative).
    pub fn apply_contact_name(&self, jid: &Jid, name: &str) -> Result<bool> {
        let canonical = self.canonical_jid(jid)?;
        let connection = self.lock()?;
        connection
            .execute(
                "INSERT INTO contacts (id, name) VALUES (?1, ?2)
                 ON CONFLICT(id) DO UPDATE SET name = excluded.name",
                params![jid.as_str(), name],
            )
            .map_err(storage_error)?;
        if canonical.as_str() != jid.as_str() {
            connection
                .execute(
                    "INSERT INTO contacts (id, name) VALUES (?1, ?2)
                     ON CONFLICT(id) DO UPDATE SET name = excluded.name",
                    params![canonical.as_str(), name],
                )
                .map_err(storage_error)?;
        }
        let renamed = connection
            .execute(
                "UPDATE chats SET name = ?2 WHERE (id = ?1 OR id = ?3) AND name != ?2",
                params![jid.as_str(), name, canonical.as_str()],
            )
            .map_err(storage_error)?;
        Ok(renamed > 0)
    }

    /// The chat id that `jid` should be stored under. Identity aliases
    /// (phone-number ↔ LID) collapse onto one row so history and live
    /// messages land in the same conversation.
    pub fn canonical_jid(&self, jid: &Jid) -> Result<Jid> {
        let connection = self.lock()?;
        let mapped: Option<String> = connection
            .query_row(
                "SELECT canonical FROM jid_aliases WHERE jid = ?1",
                params![jid.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(storage_error)?;
        Ok(mapped.map(Jid::new).unwrap_or_else(|| jid.clone()))
    }

    /// Direct chats whose name is still a numeric placeholder (LID or phone).
    pub fn chats_needing_contact_names(&self, limit: u32) -> Result<Vec<Jid>> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT id FROM chats
                 WHERE is_group = 0
                   AND id NOT LIKE '%@g.us'
                   AND id NOT LIKE '%@broadcast'
                   AND id NOT LIKE '%@newsletter'
                   AND (name = '' OR name = substr(id, 1, instr(id, '@') - 1))
                 ORDER BY last_activity_ts DESC
                 LIMIT ?1",
            )
            .map_err(storage_error)?;
        let rows = statement
            .query_map(params![limit], |row| row.get::<_, String>(0))
            .map_err(storage_error)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map(|ids| ids.into_iter().map(Jid::new).collect())
            .map_err(storage_error)
    }

    /// Record that `left` and `right` are the same person (PN + LID).
    ///
    /// Messages and the chat row of the non-canonical id are moved onto the
    /// canonical one. Returns the canonical JID when a merge happened, or
    /// `None` when the two were already the same identity.
    pub fn link_jids(&self, left: &Jid, right: &Jid) -> Result<Option<Jid>> {
        if left.as_str() == right.as_str() {
            return Ok(None);
        }

        let left_canon = self.canonical_jid(left)?;
        let right_canon = self.canonical_jid(right)?;
        if left_canon.as_str() == right_canon.as_str() {
            return Ok(None);
        }

        let connection = self.lock()?;
        let tx = connection.unchecked_transaction().map_err(storage_error)?;

        let left_row = chat_identity(&tx, left_canon.as_str())?;
        let right_row = chat_identity(&tx, right_canon.as_str())?;
        let canonical = pick_canonical_jid(&left_canon, left_row.as_ref(), &right_canon, right_row.as_ref());
        let other = if canonical.as_str() == left_canon.as_str() {
            right_canon.clone()
        } else {
            left_canon.clone()
        };

        tx.execute(
            "UPDATE messages SET chat_id = ?1 WHERE chat_id = ?2",
            params![canonical.as_str(), other.as_str()],
        )
        .map_err(storage_error)?;

        // Call-log rows carry a chat id too; moving them keeps the call
        // history of the merged identity in one place.
        tx.execute(
            "UPDATE call_log SET chat_id = ?1 WHERE chat_id = ?2",
            params![canonical.as_str(), other.as_str()],
        )
        .map_err(storage_error)?;

        if let Some(other_row) = chat_identity(&tx, other.as_str())? {
            tx.execute(
                "INSERT INTO chats (
                     id, name, last_message_preview, last_activity_ts,
                     unread_count, muted, pinned, is_group, is_archived
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(id) DO UPDATE SET
                     name = CASE
                         WHEN chats.name = '' OR chats.name = substr(chats.id, 1, instr(chats.id, '@') - 1)
                             THEN excluded.name
                         ELSE chats.name
                     END,
                     last_message_preview = COALESCE(
                         chats.last_message_preview,
                         excluded.last_message_preview
                     ),
                     last_activity_ts = MAX(chats.last_activity_ts, excluded.last_activity_ts),
                     unread_count = MAX(chats.unread_count, excluded.unread_count),
                     muted = MAX(chats.muted, excluded.muted),
                     pinned = MAX(chats.pinned, excluded.pinned),
                     is_archived = MAX(chats.is_archived, excluded.is_archived)",
                params![
                    canonical.as_str(),
                    other_row.name,
                    other_row.preview,
                    other_row.activity,
                    other_row.unread,
                    other_row.muted,
                    other_row.pinned,
                    other_row.is_group,
                    other_row.archived,
                ],
            )
            .map_err(storage_error)?;
            tx.execute("DELETE FROM chats WHERE id = ?1", params![other.as_str()])
                .map_err(storage_error)?;
        }

        for jid in [left.as_str(), right.as_str(), left_canon.as_str(), right_canon.as_str()] {
            tx.execute(
                "INSERT INTO jid_aliases (jid, canonical) VALUES (?1, ?2)
                 ON CONFLICT(jid) DO UPDATE SET canonical = excluded.canonical",
                params![jid, canonical.as_str()],
            )
            .map_err(storage_error)?;
        }
        tx.execute(
            "UPDATE jid_aliases SET canonical = ?1 WHERE canonical = ?2",
            params![canonical.as_str(), other.as_str()],
        )
        .map_err(storage_error)?;

        tx.commit().map_err(storage_error)?;
        Ok(Some(canonical))
    }

    /// Add or replace a reaction; an empty emoji removes it.
    pub fn upsert_reaction(&self, message_id: &str, reactor: &Jid, emoji: &str) -> Result<()> {
        let connection = self.lock()?;
        if emoji.is_empty() {
            connection
                .execute(
                    "DELETE FROM reactions WHERE message_id = ?1 AND reactor = ?2",
                    params![message_id, reactor.as_str()],
                )
                .map_err(storage_error)?;
            return Ok(());
        }
        connection
            .execute(
                "INSERT INTO reactions (message_id, reactor, emoji, updated_at)
                 VALUES (?1, ?2, ?3, unixepoch())
                 ON CONFLICT(message_id, reactor) DO UPDATE SET
                     emoji = excluded.emoji,
                     updated_at = excluded.updated_at",
                params![message_id, reactor.as_str(), emoji],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    /// Every reaction on messages in `chat_id`:
    /// `(message_id, reactor, emoji)`.
    pub fn reactions_for_chat(&self, chat_id: &Jid) -> Result<Vec<(String, String, String)>> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT r.message_id, r.reactor, r.emoji
                 FROM reactions r JOIN messages m ON m.id = r.message_id
                 WHERE m.chat_id = ?1",
            )
            .map_err(storage_error)?;
        let rows = statement
            .query_map(params![chat_id.as_str()], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .map_err(storage_error)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(storage_error)
    }

    /// Buffer a reaction whose message row does not exist yet (the reaction
    /// beat the message: live traffic does not wait for history sync). The
    /// chat id is stored canonical; a later reaction from the same actor on
    /// the same message replaces the buffered one.
    pub fn buffer_pending_reaction(
        &self,
        chat_id: &Jid,
        message_id: &str,
        reactor: &Jid,
        emoji: &str,
        timestamp: u64,
    ) -> Result<()> {
        let chat_id = self.canonical_jid(chat_id)?;
        let connection = self.lock()?;
        connection
            .execute(
                "INSERT INTO pending_reactions (chat_id, message_id, reactor, emoji, ts)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(message_id, reactor) DO UPDATE SET
                     chat_id = excluded.chat_id,
                     emoji = excluded.emoji,
                     ts = excluded.ts",
                params![
                    chat_id.as_str(),
                    message_id,
                    reactor.as_str(),
                    emoji,
                    as_i64(timestamp),
                ],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    /// Apply buffered reactions for `message_id` once its row exists and
    /// delete the ones that were applied. Called right after a message insert
    /// so late-arriving reactions are not lost. Returns how many were applied.
    pub fn drain_pending_reactions(&self, message_id: &str) -> Result<usize> {
        let connection = self.lock()?;
        let known: i64 = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM messages WHERE id = ?1)",
                params![message_id],
                |row| row.get(0),
            )
            .map_err(storage_error)?;
        if known == 0 {
            return Ok(0);
        }

        let pending: Vec<(String, String)> = {
            let mut statement = connection
                .prepare(
                    "SELECT reactor, emoji FROM pending_reactions WHERE message_id = ?1",
                )
                .map_err(storage_error)?;
            let rows = statement
                .query_map(params![message_id], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })
                .map_err(storage_error)?;
            rows.collect::<std::result::Result<Vec<_>, _>>()
                .map_err(storage_error)?
        };

        let mut applied = 0usize;
        for (reactor, emoji) in pending {
            if emoji.is_empty() {
                // An empty emoji removes the reaction.
                connection
                    .execute(
                        "DELETE FROM reactions WHERE message_id = ?1 AND reactor = ?2",
                        params![message_id, reactor],
                    )
                    .map_err(storage_error)?;
            } else {
                connection
                    .execute(
                        "INSERT INTO reactions (message_id, reactor, emoji, updated_at)
                         VALUES (?1, ?2, ?3, unixepoch())
                         ON CONFLICT(message_id, reactor) DO UPDATE SET
                             emoji = excluded.emoji,
                             updated_at = excluded.updated_at",
                        params![message_id, reactor, emoji],
                    )
                    .map_err(storage_error)?;
            }
            connection
                .execute(
                    "DELETE FROM pending_reactions WHERE message_id = ?1 AND reactor = ?2",
                    params![message_id, reactor],
                )
                .map_err(storage_error)?;
            applied += 1;
        }
        Ok(applied)
    }

    /// Tombstone a message that was revoked for everyone.
    pub fn mark_message_revoked(&self, message_id: &str) -> Result<()> {
        let connection = self.lock()?;
        connection
            .execute(
                "UPDATE messages SET text = NULL, kind = 'unsupported' WHERE id = ?1",
                params![message_id],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    /// Replace a message's text after an edit.
    pub fn update_message_text(&self, message_id: &str, text: &str) -> Result<()> {
        let connection = self.lock()?;
        connection
            .execute(
                "UPDATE messages
                 SET text = ?2,
                     kind = CASE WHEN kind = 'unsupported' THEN 'text' ELSE kind END
                 WHERE id = ?1",
                params![message_id, text],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    /// Delete one message row (delete-for-me).
    pub fn delete_message(&self, message_id: &str) -> Result<()> {
        let connection = self.lock()?;
        connection
            .execute("DELETE FROM messages WHERE id = ?1", params![message_id])
            .map_err(storage_error)?;
        Ok(())
    }

    /// Mark a message as starred (or clear it).
    pub fn set_message_starred(&self, message_id: &str, starred: bool) -> Result<()> {
        let connection = self.lock()?;
        connection
            .execute(
                "UPDATE messages SET starred = ?2 WHERE id = ?1",
                params![message_id, i64::from(starred)],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    /// Every starred message, newest first.
    pub fn list_starred(&self, limit: u32) -> Result<Vec<Message>> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT id, chat_id, sender_id, from_me, timestamp, kind, text, status
                 FROM messages
                 WHERE starred = 1
                 ORDER BY timestamp DESC, rowid DESC
                 LIMIT ?1",
            )
            .map_err(storage_error)?;
        let rows = statement
            .query_map(params![limit], row_to_message)
            .map_err(storage_error)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
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
        // The `messages` table has no view-once column yet, so the flag is
        // only known for messages converted from a live payload. Follow-up:
        // persist it (schema v3) or re-derive it from `raw_proto`.
        view_once: false,
    })
}

fn storage_error(error: rusqlite::Error) -> CoreError {
    CoreError::Storage(error.to_string())
}

/// Whether a chat row for `id` exists. Flag patches compare against this so
/// they can report that they landed on a row they created themselves (a
/// deferred patch that arrived before history sync).
fn chat_row_exists(connection: &Connection, id: &str) -> Result<bool> {
    let exists: i64 = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM chats WHERE id = ?1)",
            params![id],
            |row| row.get(0),
        )
        .map_err(storage_error)?;
    Ok(exists != 0)
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

struct ChatIdentity {
    name: String,
    preview: Option<String>,
    activity: i64,
    unread: i64,
    muted: i64,
    pinned: i64,
    is_group: i64,
    archived: i64,
}

fn chat_identity(connection: &Connection, id: &str) -> Result<Option<ChatIdentity>> {
    connection
        .query_row(
            "SELECT name, last_message_preview, last_activity_ts,
                    unread_count, muted, pinned, is_group, is_archived
             FROM chats WHERE id = ?1",
            params![id],
            |row| {
                Ok(ChatIdentity {
                    name: row.get(0)?,
                    preview: row.get(1)?,
                    activity: row.get(2)?,
                    unread: row.get(3)?,
                    muted: row.get(4)?,
                    pinned: row.get(5)?,
                    is_group: row.get(6)?,
                    archived: row.get(7)?,
                })
            },
        )
        .optional()
        .map_err(storage_error)
}

fn pick_canonical_jid(
    left: &Jid,
    left_row: Option<&ChatIdentity>,
    right: &Jid,
    right_row: Option<&ChatIdentity>,
) -> Jid {
    let left_named = left_row.is_some_and(|row| !left.name_is_placeholder(&row.name));
    let right_named = right_row.is_some_and(|row| !right.name_is_placeholder(&row.name));
    if left_named && !right_named {
        return left.clone();
    }
    if right_named && !left_named {
        return right.clone();
    }
    if !left.is_lid() && right.is_lid() {
        return left.clone();
    }
    if left.is_lid() && !right.is_lid() {
        return right.clone();
    }
    if left_row.is_some() {
        return left.clone();
    }
    right.clone()
}

/// Persistence hooks the media pipeline needs. Written here (not in
/// `media.rs`) because it uses this module's private connection helpers.
use crate::media::{MediaRecord, MediaStore};

impl MediaStore for Store {
    fn raw_proto(&self, message_id: &str) -> Result<Option<Vec<u8>>> {
        let connection = self.lock()?;
        connection
            .query_row(
                "SELECT raw_proto FROM messages WHERE id = ?1",
                params![message_id],
                |row| row.get::<_, Option<Vec<u8>>>(0),
            )
            .optional()
            .map(Option::flatten)
            .map_err(storage_error)
    }

    fn set_raw_proto(&self, message_id: &str, raw_proto: &[u8]) -> Result<()> {
        let connection = self.lock()?;
        connection
            .execute(
                "UPDATE messages SET raw_proto = ?1 WHERE id = ?2",
                params![raw_proto, message_id],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    fn media_record(&self, message_id: &str) -> Result<Option<MediaRecord>> {
        let connection = self.lock()?;
        connection
            .query_row(
                "SELECT message_id, mime, file_name, size, local_path, sha256,
                        downloaded_at
                 FROM media WHERE message_id = ?1",
                params![message_id],
                |row| {
                    Ok(MediaRecord {
                        message_id: row.get(0)?,
                        mime: row.get(1)?,
                        file_name: row.get(2)?,
                        size: row.get::<_, i64>(3)? as u64,
                        local_path: row.get(4)?,
                        sha256: row.get(5)?,
                        downloaded_at: row.get::<_, i64>(6)? as u64,
                    })
                },
            )
            .optional()
            .map_err(storage_error)
    }

    fn upsert_media_record(&self, record: &MediaRecord) -> Result<()> {
        let connection = self.lock()?;
        connection
            .execute(
                "INSERT INTO media (message_id, mime, file_name, size,
                                    local_path, sha256, downloaded_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(message_id) DO UPDATE SET
                     mime = excluded.mime,
                     file_name = excluded.file_name,
                     size = excluded.size,
                     local_path = excluded.local_path,
                     sha256 = excluded.sha256,
                     downloaded_at = excluded.downloaded_at",
                params![
                    &record.message_id,
                    record.mime.as_deref(),
                    record.file_name.as_deref(),
                    as_i64(record.size),
                    &record.local_path,
                    record.sha256.as_deref(),
                    as_i64(record.downloaded_at),
                ],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    fn save_outgoing_message(&self, chat_id: &Jid, message: &Message, preview: &str) -> Result<()> {
        // Same order as `WaClient::send_text`: the chat row must exist before
        // the message references it.
        self.record_message_activity(chat_id, preview, message.timestamp, None, false)?;
        self.set_chat_last_message_meta(chat_id, message.kind, message.from_me)?;
        self.upsert_message(message)
    }
}

/// Call-log persistence, implemented here because it needs the private
/// connection helpers (same pattern as the media pipeline).
use crate::calls::{CallLogRecord, CallLogStore, CallOutcome};

impl CallLogStore for Store {
    fn upsert_call_log(&self, record: &CallLogRecord) -> Result<()> {
        let connection = self.lock()?;
        connection
            .execute(
                "INSERT INTO call_log (id, chat_id, from_me, video, outcome,
                                      started_at, duration_secs, raw)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(id) DO UPDATE SET
                     chat_id = excluded.chat_id,
                     from_me = excluded.from_me,
                     video = excluded.video,
                     outcome = excluded.outcome,
                     started_at = excluded.started_at,
                     duration_secs = excluded.duration_secs,
                     raw = excluded.raw",
                params![
                    &record.id,
                    record.chat_id.as_ref().map(Jid::as_str),
                    i64::from(record.from_me),
                    i64::from(record.video),
                    record.outcome.as_str(),
                    as_i64(record.started_at),
                    record.duration_secs.map(as_i64),
                    record.raw.as_deref(),
                ],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    fn list_call_log(&self, limit: u32) -> Result<Vec<CallLogRecord>> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT id, chat_id, from_me, video, outcome, started_at,
                        duration_secs, raw
                 FROM call_log
                 ORDER BY started_at DESC, rowid DESC
                 LIMIT ?1",
            )
            .map_err(storage_error)?;
        let rows = statement
            .query_map(params![limit], |row| {
                Ok(CallLogRecord {
                    id: row.get(0)?,
                    chat_id: row.get::<_, Option<String>>(1)?.map(Jid::new),
                    from_me: row.get::<_, i64>(2)? != 0,
                    video: row.get::<_, i64>(3)? != 0,
                    outcome: CallOutcome::from(row.get::<_, String>(4)?.as_str()),
                    started_at: row.get::<_, i64>(5)?.max(0) as u64,
                    duration_secs: row
                        .get::<_, Option<i64>>(6)?
                        .map(|value| value.max(0) as u64),
                    raw: row.get(7)?,
                })
            })
            .map_err(storage_error)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(storage_error)
    }
}

/// Status (stories) persistence, same pattern as the media and call-log
/// stores: the SQL needs this module's private connection helpers.
use crate::statuses::{STATUS_TTL_SECS, StatusKind, StatusStore, StatusUpdate};

impl StatusStore for Store {
    fn upsert_status(&self, update: &StatusUpdate) -> Result<()> {
        let connection = self.lock()?;
        connection
            .execute(
                "INSERT INTO statuses (
                     id, sender, timestamp, kind, text, background_argb, viewed
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(id) DO UPDATE SET
                     sender = excluded.sender,
                     timestamp = excluded.timestamp,
                     kind = excluded.kind,
                     text = excluded.text,
                     background_argb = excluded.background_argb",
                params![
                    &update.id,
                    update.sender.as_str(),
                    as_i64(update.timestamp),
                    update.kind.as_str(),
                    update.text.as_deref(),
                    update.background_argb.map(|value| value as i64),
                    i64::from(update.viewed),
                ],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    fn list_statuses(&self, now: u64) -> Result<Vec<StatusUpdate>> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT id, sender, timestamp, kind, text, background_argb, viewed
                 FROM statuses
                 WHERE timestamp >= ?1
                 ORDER BY timestamp DESC",
            )
            .map_err(storage_error)?;
        let rows = statement
            .query_map(
                params![as_i64(now.saturating_sub(STATUS_TTL_SECS))],
                |row| {
                    let timestamp = row.get::<_, i64>(2)? as u64;
                    Ok(StatusUpdate {
                        id: row.get(0)?,
                        sender: Jid::new(row.get::<_, String>(1)?),
                        timestamp,
                        kind: StatusKind::from_stored(row.get::<_, String>(3)?.as_str()),
                        text: row.get(4)?,
                        background_argb: row.get::<_, Option<i64>>(5)?.map(|value| value as u32),
                        expires_at: timestamp.saturating_add(STATUS_TTL_SECS),
                        viewed: row.get::<_, i64>(6)? != 0,
                    })
                },
            )
            .map_err(storage_error)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(storage_error)
    }

    fn mark_status_viewed(&self, id: &str) -> Result<()> {
        let connection = self.lock()?;
        connection
            .execute("UPDATE statuses SET viewed = 1 WHERE id = ?1", params![id])
            .map_err(storage_error)?;
        Ok(())
    }

    fn prune_statuses(&self, before: u64) -> Result<u64> {
        let connection = self.lock()?;
        let deleted = connection
            .execute(
                "DELETE FROM statuses WHERE timestamp < ?1",
                params![as_i64(before)],
            )
            .map_err(storage_error)?;
        Ok(deleted as u64)
    }
}
