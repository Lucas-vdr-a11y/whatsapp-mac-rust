-- Reactions from every participant, keyed by (message, reactor).
-- Reactor is a JID string; an empty emoji means the reaction was removed and
-- rows are deleted instead of stored.

CREATE TABLE IF NOT EXISTS reactions (
    message_id TEXT NOT NULL REFERENCES messages (id) ON DELETE CASCADE,
    reactor    TEXT NOT NULL,
    emoji      TEXT NOT NULL,
    updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (message_id, reactor)
);

CREATE INDEX IF NOT EXISTS idx_reactions_message ON reactions (message_id);

PRAGMA user_version = 3;
