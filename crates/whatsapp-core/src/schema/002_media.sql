-- Schema v2: raw protobuf payloads and the media cache.
--
-- The media pipeline needs the original `wa::Message` bytes (direct path,
-- media key, hashes) long after the message row was written, so v2 adds a
-- nullable `raw_proto` column to `messages`. Downloaded binaries are never
-- stored in SQLite; `media` only mirrors their metadata and local cache path.
--
-- store.rs runs this file when `user_version < 2` and bumps SCHEMA_VERSION to
-- 2. `ALTER TABLE` is not idempotent, which is fine: the user_version guard
-- makes this batch run exactly once.

ALTER TABLE messages ADD COLUMN raw_proto BLOB;

CREATE TABLE IF NOT EXISTS media (
    message_id    TEXT PRIMARY KEY REFERENCES messages (id) ON DELETE CASCADE,
    mime          TEXT,
    file_name     TEXT,
    size          INTEGER NOT NULL DEFAULT 0,
    local_path    TEXT NOT NULL,
    sha256        BLOB,
    downloaded_at INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_media_downloaded_at ON media (downloaded_at);

PRAGMA user_version = 2;
