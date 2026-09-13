-- Metadata of the newest message per chat, so the UI can render a localized
-- preview label and the delivery tick without parsing the preview string.

ALTER TABLE chats ADD COLUMN last_kind TEXT;
ALTER TABLE chats ADD COLUMN last_from_me INTEGER NOT NULL DEFAULT 0;

PRAGMA user_version = 7;
