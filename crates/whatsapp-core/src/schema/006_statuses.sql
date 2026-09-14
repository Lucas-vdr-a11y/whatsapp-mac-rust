-- Schema v6: status updates ("stories") received from contacts.
--
-- A status is a normal message posted to `status@broadcast`; only the fields
-- the 24-hour Recent list renders are kept. `expires_at` is derived in Rust
-- (`timestamp + 86400`), so pruning is a plain `timestamp < ?` delete covered
-- by `idx_statuses_timestamp`.
--
-- store.rs runs this file when `user_version < 6` and bumps SCHEMA_VERSION to
-- 6. (There is deliberately no v5 file: the milestone landed after v4.)

CREATE TABLE IF NOT EXISTS statuses (
    id              TEXT PRIMARY KEY,
    sender          TEXT NOT NULL,
    timestamp       INTEGER NOT NULL,
    kind            TEXT NOT NULL,
    text            TEXT,
    background_argb INTEGER,
    viewed          INTEGER NOT NULL DEFAULT 0
);

-- One row per sender for the grouped list.
CREATE INDEX IF NOT EXISTS idx_statuses_sender_ts
    ON statuses (sender, timestamp DESC);

-- 24-hour pruning (`DELETE FROM statuses WHERE timestamp < ?`).
CREATE INDEX IF NOT EXISTS idx_statuses_timestamp
    ON statuses (timestamp);

PRAGMA user_version = 6;
