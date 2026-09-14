# UI parity audit — RustWA vs the official WhatsApp Mac app

Reference: the official app (26.33.73, Catalyst) side by side with RustWA on the
same machine, compared screen by screen with screenshots. Only structural and
visual facts are recorded here — no personal content.

Legend: ✅ matches · 🚧 in progress · ❌ differs

## Global shell

| Area | Official | RustWA | Status |
| --- | --- | --- | --- |
| Rail order | Chats, Calls, Status, Channels, Starred | same order (Communities accessible inside chats) | ✅ |
| Rail badges | Chats and Channels show unread counts | Chats only | 🚧 channels badge pending |
| Rail bottom | Settings gear + profile entry | same | ✅ |
| Window chrome | Traffic lights over content, drag anywhere on the title strip | same, plus explicit drag regions on every header and title | ✅ |
| Theme | Follows system, light/dark | Dark/light/system setting with persisted choice | ✅ |

## Chat list

| Area | Official | RustWA | Status |
| --- | --- | --- | --- |
| Header | “Chats” + compose (pencil) | pencil + filter menu (build in flight) | 🚧 |
| Search | Rounded pill, localized placeholder (“Zoek”) | Rounded pill, “Search” | 🚧 localization |
| Filters | None on macOS | All/Unread/Groups moved into the ⋮ menu | 🚧 matches after next build |
| Row metrics | 72px rows, ~49px round avatar, name + preview, time right, green unread badge | same metrics and structure | ✅ |
| Avatars | Profile pictures, initials fallback | Remote pictures (release CSP fixed), initials fallback | ✅ |
| Ordering | Newest activity first | Newest first (repair + monotonic timestamps) | ✅ |
| Names | Address-book names, numbers for unsaved contacts | 260 contacts synced; unsaved numbers remain (same behaviour) | ✅ |
| Group names | Group subject | Subjects resolved in the background; 59/82 named so far | 🚧 23 pending/denied |
| Media previews | “Foto”, “Video”, voice icon, tick prefix for own last message | “[Photo]”, “[Video]”, “[Voice message]” | 🚧 localized labels + tick prefix pending |

## Conversation

| Area | Official | RustWA | Status |
| --- | --- | --- | --- |
| Background | Doodle wallpaper | Original doodle tile, theme-aware mask | 🚧 next build |
| Header | Avatar, name, presence line when known, voice/video buttons, ⋮ | same; presence line now real (online/last seen/typing) | ✅ |
| Date pills | Centered gray pills | same | ✅ |
| Bubbles | 7.5px radius, tails, own green right / light left | same tokens | ✅ |
| Quoted replies | Colored left border, sender name | same, via quote context | ✅ |
| Reactions | Chip on the bubble edge | chip row under bubble | 🚧 chip placement differs slightly |
| Receipts | Grey double ticks sent/delivered, blue read | same icon set (verify blue in dark theme) | 🚧 needs visual check |
| Media | Inline images with captions, video tiles, voice waveform | inline cards, lightbox, waveform, view-once cover | ✅ |
| Composer | Emoji, input, attach, mic/send | attach, emoji, input, send (order differs slightly) | 🚧 minor |
| Mentions | @-autocomplete in groups | implemented | ✅ |
| Drafts | Per-chat drafts | localStorage drafts, restored per chat | ✅ |

## Feature surfaces (screens)

| Screen | Official | RustWA | Status |
| --- | --- | --- | --- |
| Calls | Recent calls (synced from phone) + call-back | session history + synced call log section | 🚧 verify after sync |
| Status | Recent updates ring, viewer, my status | list + viewer + text posting; media statuses pending | 🚧 |
| Channels | Followed list, discovery | follow/unfollow, post, honest empty states | 🚧 |
| Starred | Starred messages list | list wired to the local star flag | ✅ |
| Settings | Privacy, notifications, storage, about | same sections + privacy choices, blocking, disappearing timer | ✅ |
| Profile | Name, photo, about | placeholder + linked state | 🚧 own profile editing pending |

## Known remaining gaps

1. **Localization (Dutch)** — the largest visual difference; the official app
   follows the system language. RustWA is English-only today.
2. **Group subjects** for the remaining chats (metadata fetch failures/timeouts).
3. **Channel rail badge** and channel unread handling.
4. **Preview labels** (`[Photo]` → localized “Foto” with an icon).
5. **Reaction chip placement** — official anchors it to the bubble corner.
6. **Media statuses** (image/video) in the Status screen.
7. **Own profile editing** (name/about/photo).
8. **Calls**: video calls are upstream-preview only; group calls and screen
   share are not implemented by the protocol crate.
