# Architecture

RustWA is split into three layers with strict dependency direction:

```
UI (React/TS, WKWebView)
   │  Tauri commands (invoke)         ▲ Tauri events (emit)
   ▼                                  │
apps/desktop/src-tauri  ──────────────┘
   │  calls
   ▼
crates/whatsapp-core
   │  uses
   ▼
whatsapp-rust (external, MIT)
```

## Layers

### 1. `crates/whatsapp-core` — the domain core

Pure Rust. No Tauri, no webview, no UI types. Testable with `cargo test`, and
reusable from a CLI or a future native UI.

Responsibilities:

- **Connection lifecycle** — create the protocol client, connect, reconnect
  with backoff, log out, report state transitions.
- **Pairing** — surface QR codes / pair codes to the UI and complete the
  handshake.
- **Event bus** — translate upstream protocol events into a small, stable
  domain event enum (`CoreEvent`). The UI must never depend on upstream types.
- **State store** — SQLite via `rusqlite` (WAL mode): chats, messages,
  contacts, media metadata. All writes happen on the core side; the UI holds a
  read cache only.
- **Command surface** — `send_text`, `mark_read`, `set_typing`, `download_media`,
  … Each command returns rich errors the UI can render.

Threading model: one multi-threaded Tokio runtime owned by the core. The Tauri
host subscribes to a `tokio::sync::broadcast` channel and forwards events to
the webview; commands are `async` Tauri commands that delegate to core methods.

### 2. `apps/desktop/src-tauri` — the native host

- Window, menus, dock, tray, notifications, global shortcuts.
- Marshals commands/events between the webview and the core.
- Holds **no** business logic: every handler is a thin adapter.

Native integrations:

| Concern | Implementation |
|---|---|
| Menu bar | `src/menu.rs` builds the RustWA / Edit / View / Window / Help menus. `Preferences…` emits `ui://open-settings` to the webview; Reload, Actual Size, Zoom In/Out and Toggle Developer Tools (debug builds only) act on the main webview; Help opens the project's GitHub page. |
| Notifications | `tauri-plugin-notification`; commands `notify(title, body)` and `notification_permission()`. |
| Dock badge | `set_badge(count: Option<i64>)`; Tauri forwards the count to the macOS dock tile (`NSApplication.dockTile`). |
| Single instance | `tauri-plugin-single-instance`; a second launch focuses the existing window. |
| Window state | `tauri-plugin-window-state`; size and position are restored across launches. |
| Capabilities | `platform_capabilities()` returns `{ badge, notifications, tray }` (camelCase) so the UI can adapt. |

### 3. `apps/desktop/src` — the UI

- React 19 + TypeScript, Vite build, Zustand store.
- Renders exclusively from state received over Tauri IPC.
- Design tokens in `src/styles/tokens.css` mirror the WhatsApp design system
  (see [design-spec.md](design-spec.md)); no proprietary assets.

## Data flow

```
incoming message
  whatsapp-rust event ──► whatsapp-core (map + persist) ──► broadcast
                                                             │
         Tauri host task ◄───────────────────────────────────┘
                │ emit("core://event", payload)
                ▼
         UI store update ──► React re-render
```

User intent travels the other way:

```
UI click ──► invoke("send_text", …) ──► Tauri command ──► core.send_text ──► protocol
```

## Storage

| Data | Location | Notes |
|---|---|---|
| Protocol session (keys, credentials) | `~/Library/Application Support/RustWA/session/` | Managed by the upstream storage adapter; file mode `0600`; keychain integration planned |
| App database (chats, messages) | `~/Library/Application Support/RustWA/rustwa.db` | SQLite, WAL, schema migrations |
| Media cache | `~/Library/Caches/RustWA/media/` | LRU-evicted, user-clearable |

## Design principles

1. **UI cannot break the core.** Commands are validated in Rust; the webview is
   untrusted input.
2. **No proprietary assets, ever.** Icons are original or Lucide; fonts are
   system fonts.
3. **Measure, don't claim.** Performance numbers in the README are measured on
   the reference machine and updated per release.
4. **Stable domain types.** Upstream API churn is absorbed in one crate.
5. **Everything logged, nothing leaked.** `tracing` with PII redaction by
   default.

## Security

- Tauri CSP restricts the webview to local assets and IPC.
- Capabilities in `src-tauri/capabilities/` declare the minimum permissions.
- Session credentials are never logged and never cross IPC.
- `unsafe_code = "deny"` workspace-wide.
