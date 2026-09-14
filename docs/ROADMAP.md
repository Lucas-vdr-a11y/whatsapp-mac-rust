# Roadmap

The canonical, evidence-backed plan lives in
**[docs/parity-matrix.md](parity-matrix.md)**: every user-visible feature of
the official app, checked against `whatsapp-rust`, Baileys and whatsmeow.

Key finding: the upstream Rust protocol library is far more complete than its
README suggests — QR/pair-code/passkey linking, history sync, the full send
surface (replies, mentions, edits, revokes, reactions, polls, events, pins,
keep-in-chat, forwarding), groups/communities/channels/status, and a full VoIP
stack (1:1 audio/video, screen share, group-call primitives, call links). The
work therefore concentrates on the desktop app: state, UI and platform
integration.

## Milestones

| # | Milestone | Contents | Status |
| --- | --- | --- | --- |
| M0 | Core connection | QR/pair-code/passkey linking, session persistence, history sync, reconnect, media pipeline | ✅ pairing, session and history verified live (5k+ messages imported); media in progress |
| M1 | Everyday messaging | Text, replies, mentions, receipts, typing/recording, presence, unread counts, notifications, drafts | 🚧 text, receipts, typing, presence, notifications and auto read done; quotes/mentions/drafts in progress |
| M2 | Chat management | Archive/pin/mute/delete/clear/mark-read, starred, local search, favorites, export | 🚧 actions sync live; search backend done; archive/starred UI pending |
| M3 | Media experience | Images/videos/GIF/voice/documents/stickers/PTV/albums, view-once, disappearing, captions, thumbnails, auto-download settings, HD | 🚧 pipeline under construction |
| M4 | Message actions | Reactions, edit, revoke, delete-for-me, pin, keep-in-chat, forwarding, polls/quizzes, events/RSVP | 🚧 core + UI under construction |
| M5 | Groups & communities | Group admin, invite links, join requests, events; community create/link/join | 🚧 groups + communities (create/join/link/invite) shipped; join requests pending |
| M6 | Status & channels | Status post/view/privacy/reactions; channels follow/post/react/comments | 🚧 text statuses post + receive/view shipped; media statuses and channel comments pending |
| M7 | Calls | 1:1 audio → video → screen share, call links, waiting room, group calls, call history | 🚧 1:1 audio with CoreAudio + call UI + call-log sync; upstream video is preview-grade |
| M8 | Privacy & security | Privacy settings, block/report, app lock, chat lock, identity verification, passkeys, proxy | 🚧 privacy/blocking/Touch ID app lock shipped; chat lock, verification, proxy pending |
| M9 | System integration | Siri intents, App Intents/Shortcuts, widgets, share sheet, menu bar, multiple windows, deep links | 🚧 tray, deep links, autostart, app lock groundwork in progress |
| M10 | Business & power features | Business profiles, catalog, labels, quick replies, usernames | ⏳ |
| M11 | Research | Scheduled messages, live location, message translation/transcripts | ⏳ |
| M12 | Out of scope | Payments, Meta AI suite, interop bridges, E2E backups, cross-posting — Meta-gated or infeasible for unofficial clients; the UI presents honest "not supported" states | ❌ |

### Verified live (against a real linked account)

- QR pairing completes in seconds; the session survives restarts without re-scanning.
- History sync imports conversations and messages (5,017 messages / 6 chats on the reference account).
- Delivery receipts, typing indicators and chat actions (pin/mute/archive/read) round-trip through the protocol.
- Store: the release app opens to ~15 MB with 111 MB idle RSS and 0.41 s cold start (see README).

## How we work

- One branch per milestone slice, PR into `main`, CI green before merge.
- Protocol work ships with tests; UI work ships with screenshots.
- Performance budgets (install size, RAM, CPU, start time) are measured per
  milestone and published in the README, not estimated.
- When something cannot be done with an open protocol, it is documented in the
  parity matrix rather than faked.
