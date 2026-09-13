-- Locally starred messages, synced from the star app-state updates.

ALTER TABLE messages ADD COLUMN starred INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_messages_starred
    ON messages (starred, timestamp DESC)
    WHERE starred = 1;

-- Protocol chatter was stored as `system` rows by early builds; it is not
-- user content and only pollutes the chat list.
DELETE FROM messages WHERE kind = 'system';

PRAGMA user_version = 4;
