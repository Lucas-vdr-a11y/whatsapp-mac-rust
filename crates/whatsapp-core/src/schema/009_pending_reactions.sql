-- Schema v9: reactions that arrive before their message row.
--
-- Reaction envelopes can beat the message they point at (history sync imports
-- on a blocking task; live traffic does not wait for it). Inserting them into
-- `reactions` would violate its foreign key and lose the reaction forever, so
-- they are buffered here (chat_id stored canonical) and drained once the
-- message row exists. The primary key matches `reactions`: one reaction per
-- (message, actor), the newest wins.

CREATE TABLE IF NOT EXISTS pending_reactions (
    chat_id    TEXT NOT NULL,
    message_id TEXT NOT NULL,
    reactor    TEXT NOT NULL,
    emoji      TEXT NOT NULL,
    ts         INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (message_id, reactor)
);

CREATE INDEX IF NOT EXISTS idx_pending_reactions_message ON pending_reactions (message_id);

PRAGMA user_version = 9;
