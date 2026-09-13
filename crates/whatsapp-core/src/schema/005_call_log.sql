-- Schema v5: the phone-synced call log (history-sync records + live
-- `CallLogMessage` items).
--
-- `id` is a stable row key: the server call id when the record carries one,
-- otherwise a SHA-256 digest of the normalized record (see `calls.rs`). A row
-- stores only the derived chat id; the original proto record would live in
-- `raw` for future re-processing. `outcome` is the coarse UI vocabulary
-- ("missed" | "answered" | "declined" | "failed" | "ongoing").
--
-- store.rs runs this file when `user_version < 5` and bumps SCHEMA_VERSION to
-- 5. The `user_version` guard makes this batch run exactly once.

CREATE TABLE IF NOT EXISTS call_log (
    id            TEXT PRIMARY KEY,
    chat_id       TEXT,
    from_me       INTEGER NOT NULL DEFAULT 0,
    video         INTEGER NOT NULL DEFAULT 0,
    outcome       TEXT NOT NULL,
    started_at    INTEGER NOT NULL DEFAULT 0,
    duration_secs INTEGER,
    raw           BLOB
);

CREATE INDEX IF NOT EXISTS idx_call_log_started_at ON call_log (started_at DESC);

PRAGMA user_version = 5;
