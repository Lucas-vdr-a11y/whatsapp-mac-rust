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

## Status / repair plan

- [x] Audit complete (this doc)
- [ ] R1: archive persistence + JID identity normalization (findings 1, 4)
- [ ] R2: full message history sync (finding 2)
- [ ] R3: group subject / contact name resolution (finding 3) + preview placeholders (8)
- [ ] R4: reaction FK fix (5) + unread accounting (6)
- [ ] R5: perf — virtualization, memoization, receipt batching (12, 13)
- [ ] R6: UI parity polish (7, 9, 10, 11) + remaining UI-audit findings
