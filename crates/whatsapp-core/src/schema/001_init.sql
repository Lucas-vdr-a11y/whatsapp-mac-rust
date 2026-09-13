-- Schema v1: chats, messages, contacts.
--
-- Timestamps are Unix seconds. Booleans are stored as 0/1. Enum values are
-- stored as the same camelCase strings used over IPC so debugging stays easy.

CREATE TABLE IF NOT EXISTS chats (
    id                   TEXT PRIMARY KEY,
    name                 TEXT NOT NULL DEFAULT '',
    last_message_preview TEXT,
    last_activity_ts     INTEGER NOT NULL DEFAULT 0,
    unread_count         INTEGER NOT NULL DEFAULT 0,
    muted                INTEGER NOT NULL DEFAULT 0,
    pinned               INTEGER NOT NULL DEFAULT 0,
    is_group             INTEGER NOT NULL DEFAULT 0,
    is_archived          INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_chats_activity ON chats (last_activity_ts DESC);

CREATE TABLE IF NOT EXISTS messages (
    id        TEXT PRIMARY KEY,
    chat_id   TEXT NOT NULL REFERENCES chats (id) ON DELETE CASCADE,
    sender_id TEXT NOT NULL,
    from_me   INTEGER NOT NULL,
    timestamp INTEGER NOT NULL,
    kind      TEXT NOT NULL,
    text      TEXT,
    status    TEXT NOT NULL DEFAULT 'pending'
);

CREATE INDEX IF NOT EXISTS idx_messages_chat_ts
    ON messages (chat_id, timestamp);

CREATE TABLE IF NOT EXISTS contacts (
    id          TEXT PRIMARY KEY,
    name        TEXT,
    push_name   TEXT,
    avatar_path TEXT
);
