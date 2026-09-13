# Roadmap

Each milestone is a PR (or a small series of PRs) against `main`, with tests
where the milestone allows automated testing. Protocol milestones additionally
ship a manual test script that a human can run against a real phone.

## M0 — Bootstrap ✅

- Cargo workspace + crate layout
- CI (fmt, clippy, tests, UI typecheck/build)
- Architecture, roadmap, design-spec and protocol research docs
- Tauri shell with the WhatsApp 3-column layout skeleton and design tokens

## M1 — Protocol core 🚧

Deliverable: `rustwa-core` can pair (QR shown in UI), connect, reconnect,
persist the session across restarts, and exchange text messages with receipts.

- [ ] `WaClient` wrapper over `whatsapp-rust` (builder, lifecycle, graceful shutdown)
- [ ] Domain event bus (`CoreEvent`) + Tauri bridge
- [ ] QR pairing flow surfaced in the UI
- [ ] Session persistence (survives restart without re-pairing)
- [ ] Text send/receive, delivery/read receipts
- [ ] Typing + presence
- [ ] SQLite store: chats, messages, contacts (schema + migrations)
- [ ] Integration test: two clients talking to each other locally, if feasible
- [ ] Manual test script for real-device pairing

## M2 — Chat UI

Deliverable: usable day-to-day text messenger with WhatsApp-identical layout.

- [ ] Chat list: search, filters (all/unread/groups), unread badges, pin/mute/archive
- [ ] Conversation: bubbles, tails, date dividers, reply quotes, system messages
- [ ] Composer: text, emoji, keyboard send, draft persistence
- [ ] Message actions: reply, forward, copy, star, delete, edit
- [ ] Context menus, toasts, modal framework
- [ ] Virtualized lists for large histories
- [ ] Dark + light themes

## M3 — Media

- [ ] Images (thumbnails, full view, gallery)
- [ ] Video (inline playback, full player)
- [ ] Documents, audio files, voice notes (waveform + recording)
- [ ] Stickers, GIFs, link previews
- [ ] Upload/download queue, retry, progress
- [ ] Media cache + storage management screen

## M4 — Groups & communities

- [ ] Group chats, participant lists, mentions
- [ ] Group creation, admin, invite links
- [ ] Communities (subgroups, announcement groups)
- [ ] Group events (joins, leaves, edits)

## M5 — Channels & status

- [ ] Channels/newsletters: browse, follow, mute, forward restrictions
- [ ] Status/stories: post, view, replies, privacy, expiry

## M6 — Calls (research spike first)

- [ ] Feasibility report based on `whatsapp-rust` VoIP features
- [ ] 1:1 voice calls, then video, then group calls
- [ ] Call log, ring UI, call links
- ⚠️ Highest-risk milestone: the call signaling stack is the least documented
  part of the protocol. If it proves infeasible, this is documented honestly
  in the parity report rather than faked.

## M7 — Platform integration

- [ ] Native notifications (actionable), notification center behavior
- [ ] Dock badge, menu bar extras, tray
- [ ] Global shortcuts, app menu, share sheet
- [ ] Launch at login, idle suspension (pause work when hidden)
- [ ] Keychain-backed credential storage
- [ ] Siri/Shortcuts intents (parity with the official Intents extension)

## M8 — Parity audit & hardening

- [ ] Feature-by-feature checklist against the installed official app
- [ ] Performance budget verified and published (install size, RAM, CPU, start time)
- [ ] Crash recovery, migration safety, fuzz-tested parsers
- [ ] Signed, notarized `.dmg` release

## Non-goals (for now)

- iOS/Android clients
- Cloud/relay components; RustWA talks to WhatsApp servers directly
- Any redistribution of WhatsApp-owned assets
