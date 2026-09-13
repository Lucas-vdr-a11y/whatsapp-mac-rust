-- Schema v8: LID ↔ phone-number aliases.
--
-- Linked-device chats arrive under both `@s.whatsapp.net` and `@lid`.
-- Mapping them onto one chat row keeps history and live messages together.

CREATE TABLE IF NOT EXISTS jid_aliases (
    jid       TEXT PRIMARY KEY,
    canonical TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_jid_aliases_canonical ON jid_aliases (canonical);

PRAGMA user_version = 8;
