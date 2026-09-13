-- Schema v10: chat name provenance (audit S3).
--
-- `name_source` records where a chat's display name came from:
-- 'subject' (group metadata / history sync), 'pushname' (sender push name,
-- direct chats only), 'contact' (address book / usync) or 'placeholder'
-- (still unresolved). The repair passes use it to tell a real group subject
-- from a push name that once leaked into a group row.

ALTER TABLE chats ADD COLUMN name_source TEXT NOT NULL DEFAULT '';

-- Reclassify group-LID rows (audit S7): `@lid` ids in the group-id numbering
-- space (18+ digits, digits only, same ids as `@g.us`) are groups, not
-- direct chats; direct-chat LIDs are phone-derived and much shorter. Runs
-- before the name reset below so reclassified rows are reset too.
UPDATE chats SET is_group = 1
WHERE is_group = 0
  AND id LIKE '%@lid'
  AND LENGTH(substr(id, 1, instr(id, '@') - 1)) >= 18
  AND substr(id, 1, instr(id, '@') - 1) NOT GLOB '*[^0-9]*';

-- Existing group rows predate provenance tracking and some of them hold a
-- participant's push name instead of the subject; the old repair selector
-- only targeted empty/JID-like names, so those rows were unrecoverable.
-- Reset every group name to placeholder so the group-metadata pass
-- re-fetches the subject on the next connect.
UPDATE chats SET name = '', name_source = 'placeholder'
WHERE is_group = 1;

PRAGMA user_version = 10;
