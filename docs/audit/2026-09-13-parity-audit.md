# Parity Audit — RustWA vs official WhatsApp Desktop (macOS)

Date: 2026-09-13 · Branch: `feat/m1-protocol-core` · Method: live side-by-side comparison
(official WhatsApp Desktop vs `tauri dev` build), AX/UI inspection, direct SQLite inspection of
`~/Library/Application Support/io.github.lucas-vdr-a11y.rustwa/rustwa.db`, plus code audits.

## Ground truth (official app)

- Main chat list (top→down): Familie Dierx 22:17 (27 unread), jacques Opa 21:20 (8),
  Peter 19:56, Keano 18:00, Vincent 16:52, Levi 16:51, Iwan 16:30, Sasha 16:12.
- Archived section contains 9 chats: 🪄 Algemene chat (85 unread, 23:22), 🔊 Het Krijspaleis
  (50, 22:30), 📍Wie is waar te vinden? (vrijdag), Community Algemeen (266, 30-07),
  Community Algemeen (2, 09-07), Pretpark Muziek (49, 04-07), Wie is er in de Efteling? (7,
  28-06), Efteling Gerelateerd (1, 28-06), Toverland Gerelateerd (68).
- Rail: Chats (unread badge 10), Oproepen, Updates, Gearchiveerd (badge 10), Met ster
  gemarkeerd, Media-ish icon, Instellingen. No filter chips in the chat panel.
- Keano conversation: 13 messages on 13-09 between 16:28–18:00, incl. a revoked message
  shown as "Je hebt dit bericht verwijderd." Messages bottom-anchored.

## Findings — RustWA

### P0 — data / sync integrity

1. **Archive flags never persisted.** `chats.is_archived = 1` count in DB: **0** (official: 9).
   Archived view renders "Geen gearchiveerde chats"; all 9 archived chats appear in the MAIN
   list instead (e.g. Algemene chat, Het Krijspaleis, Wie is waar te vinden?, Community
   Algemeen, Toverland Gerelateerd). Suspect `ArchiveUpdate` handling (client.rs:1054) +
   JID-form mismatch (`@lid` vs `@g.us`/`@s.whatsapp.net`, see migration `008_jid_aliases.sql`).
2. **Message history incomplete.** Keano chat: RustWA stores/shows only the last message
   ("Oke" 18:00); official has 13 messages that day. History appears to be imported only from
   chat-list app-state (last message), with per-chat history fetch not populating the DB.
3. **Group subjects unresolved for several groups.** `120363404062663307@g.us` is named
   "Meike" (participant fallback) instead of "🪄 Algemene chat"; `120363401946043601@g.us`
   = "Senna" instead of "🔊 Het Krijspaleis"; `31647820621-1537097334@g.us` = "Bianc".
   Other groups resolve fine → metadata pass partially works.
4. **Active chats missing / phantom duplicates.** Familie Dierx, Vincent, Levi, Iwan, Sasha
   (all active today in official) are absent from `chats` entirely. Conversely a raw-JID chat
   `254970750308491@lid` ("Bericht", 17:23) exists that official does not show. Two "Lucas"
   rows are `@lid` chats (`14083231338501@lid`, `193088945430567@lid`) — one of them is
   officially "jacques Opa" 21:20 with the same message text. LID↔PN identity resolution is
   creating parallel/orphan rows.
5. **Inbound reactions fail to persist**: `failed to store reaction — FOREIGN KEY constraint
   failed` (client.rs, at startup). Reactions processed before their message row exists.
6. **Unread counts wrong.** Per-chat counts inflated (125/108/14 vs official 85/50/~27) and
   rail badge shows "99+" vs official 10. Also unread is NOT cleared when a chat is opened
   (Keano keeps badge 1 after open) — `mark_chat_read` not invoked or result not applied.

### P1 — UI / behavior parity

7. **Messages top-anchored** in the conversation view; official anchors to the bottom
   (a single message sits directly above the composer).
8. **Preview placeholders**: generic "Bericht" for revoked/media/sticker last-messages;
   official renders typed previews ("Je hebt dit bericht verwijderd.", "Foto", "Sticker"…).
   DB previews diverge from official last messages for synced-incompletely chats.
9. **Conversation header subtitle** "Klik voor contactgegevens" — official shows nothing
   there (or presence), contact info lives in a side panel.
10. **Filter chips** (Alle/Ongelezen/Favorieten/Groepen) rendered in the chat panel; the
    official macOS app has none.
11. Rail has an extra "Media" item and slightly different icon set vs official.

### Performance (from code audit — P0s)

12. No virtualization of the message list (`Conversation.tsx:554`) nor chat list
    (`ChatList.tsx:406`); whole history in DOM.
13. Read receipts are one IPC event per message id, each remapping the full message array
    (client.rs:1017 → bridge.ts:178 → store/app.ts:881) → O(n²) on group read bursts.
    Plus: MessageBubble/ChatListItem not memoized; event forwarder emits per protocol event;
    avatars re-fetched per mount without disk cache; store access behind a single sync mutex
    without spawn_blocking.

## Full perf audit details

See subagent report (session log). Key file:line refs preserved in the findings above.

## Sync audit — verified root causes (subagent, code-verified)

- **S1 (P0, =1/4/5):** flag patches (Archive/Mute/Pin/MarkChatAsRead) applied via bare `UPDATE chats WHERE id=?` (store.rs:526-559) with no `canonical_jid()` resolution and no row-exists check; `Ok(0)` ignored; reconnect uses `full_sync: false` (client.rs:369-382) so patches are never replayed. Race: patches arrive before `import_history_sync` creates rows (client.rs:323 spawn_blocking). `upsert_chat_from_history` ON CONFLICT deliberately keeps local `is_archived` (store.rs:245).
- **S2 (P0, =5 reactions):** `handle_reaction` (client.rs:981-1010) inserts into `reactions` (FK→messages) without the message row existing; lost permanently on FK error; `chat_id` from `key.remote_jid` not canonicalized.
- **S3 (P0, =3 names):** `handle_inbound_message` applies `push_name` to group chats (client.rs:883-893, no is_group guard; store.rs:487-490 ON CONFLICT overwrites name); `chats_needing_group_names` (store.rs:653-670) only targets `''`/JID-like names so "Meike" is skipped forever. History path guards correctly (client.rs:1214-1219).
- **S4 (P0, =4 missing chats):** `import_history_sync` `Err => warn; break` (client.rs:1251-1254) — one bad conversation aborts the entire blob import; remainder never imported; nothing re-requests.
- **S5 (P1, =6 unread):** unread locally accumulated (store.rs:496-508); server counter applied only on first insert (store.rs:242); `mark_chat_read` never canonicalizes (store.rs:531-540); rail badge sums archived+duplicate chats (App.tsx:50-53).
- **S6 (P1, =raw JIDs):** name passes one-shot, clock-driven, capped (group +10s/200, contact +6s/80; client.rs:413-518); chats imported later never resolved; usync failures silent.
- **S7 (P1, =duplicates):** group `@lid` rows never merged with `@g.us` twins; `Jid::is_group` only matches `g.us` (types.rs:42-44) so group-LID rows are misclassified as direct chats.
- **S8 (P1):** dropped events: GroupUpdate, PushNameUpdate, DeleteChatUpdate/ClearChatUpdate/DeleteMessageForMeUpdate, UndecryptableMessage, DisappearingModeChanged, PictureUpdate, DirtyState (client.rs:290-311).
- **S9 (P1):** receipts: raw `chat_id` (client.rs:1019); `set_message_status` not monotonic (store.rs:439-448); group Read flips whole message.
- **S10-14 (P2):** failed sends not persisted as Failed; mark-as-unread ignored (client.rs:1093); `link_jids` misses call_log; `upsert_message` ON CONFLICT reassigns chat_id (store.rs:273-276); frontend fabricates ghost rows (app.ts:1373-1377).

**Structural fix:** every chat-list writer must (a) resolve `canonical_jid()` and (b) upsert-or-defer instead of bare UPDATE; plus full `regular` app-state sync on connect.

## UI reference details (official app, captured for repair E)

- jacques Opa conversation holds months of history (29-03 → 13-09) with localized date
  separators ("di 23 jun", "Gisteren"); outgoing messages show blue double-checks; a link
  preview renders as a card with "Meer informatie"; messages bottom-anchored; floating
  scroll-to-bottom button; composer = [+ attachment] [placeholder "Stel bericht op"]
  [emoji] [mic].
- Chat-list timestamps are relative: "Gisteren" for yesterday, weekday for this week,
  dd-mm-yy older. Unread badge clears immediately when the chat is opened (rail badge 10→9).
- Conversation header: avatar + name only (no subtitle text).

## Status / repair plan

- [x] Audit complete (this doc)
- [ ] R1: archive persistence + JID identity normalization (findings 1, 4)
- [ ] R2: full message history sync (finding 2)
- [ ] R3: group subject / contact name resolution (finding 3) + preview placeholders (8)
- [ ] R4: reaction FK fix (5) + unread accounting (6)
- [ ] R5: perf — virtualization, memoization, receipt batching (12, 13)
- [ ] R6: UI parity polish (7, 9, 10, 11) + remaining UI-audit findings
