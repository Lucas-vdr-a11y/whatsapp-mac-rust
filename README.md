# RustWA — a fast, native WhatsApp client for macOS, written in Rust

[![CI](https://github.com/Lucas-vdr-a11y/whatsapp-mac-rust/actions/workflows/ci.yml/badge.svg)](https://github.com/Lucas-vdr-a11y/whatsapp-mac-rust/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

> **Status: pre-alpha, but it connects.** The Rust protocol core links via
> QR pairing against real WhatsApp servers and exchanges text messages; the UI
> is the familiar three-column layout with a live device-linking screen.
> Media, groups, channels, calls and notifications are still on the roadmap —
> see [docs/ROADMAP.md](docs/ROADMAP.md).

RustWA is an unofficial, from-scratch reimplementation of the WhatsApp desktop
client for macOS. The goal is a client that feels instant, uses a fraction of
the CPU and memory of the official Catalyst app, and reproduces the familiar
WhatsApp layout that millions of people already know.

The project is not a wrapper around WhatsApp Web: the protocol, cryptography
integration, state store and everything else that is not pixels is written in
Rust, on top of the excellent open-source
[`whatsapp-rust`](https://github.com/jlucaso1/whatsapp-rust) protocol library.
Only the UI is rendered with web technology (React + TypeScript in a native
WKWebView, hosted by [Tauri](https://tauri.app)) — the one toolchain that can
reproduce WhatsApp's layout faithfully at this level of detail.

## Why

Measured on the development machine (Mac Catalyst WhatsApp 26.33.73, idle):

| | Official app | RustWA | Target |
|---|---|---|---|
| Install size | 637 MB | — | < 50 MB |
| Idle memory | ~340 MB (2 processes) | — | < 150 MB |
| Idle CPU | 0.1–2% | — | ~0% |
| Cold start | seconds | — | < 1 s |

*(RustWA numbers land as milestones complete; we publish measured values, not
marketing ones.)*

## Architecture

```
┌────────────────────────────────────────────────────────────────┐
│  RustWA.app                                                    │
│                                                                │
│  ┌───────────────────────┐      ┌───────────────────────────┐  │
│  │  Tauri v2 (Rust)      │      │  WKWebView UI             │  │
│  │  window / tray /      │◄────►│  React 19 + TS            │  │
│  │  notifications / IPC  │ emit │  WhatsApp layout & tokens │  │
│  └──────────┬────────────┘      └───────────────────────────┘  │
│             │ commands / events                                │
│  ┌──────────▼───────────────────────────────────────────────┐  │
│  │  whatsapp-core (pure Rust)                               │  │
│  │  domain model · event bus · SQLite store · commands      │  │
│  └──────────┬───────────────────────────────────────────────┘  │
│             │                                                  │
│  ┌──────────▼───────────────────────────────────────────────┐  │
│  │  whatsapp-rust (MIT)   protocol · Signal · Noise · sync  │  │
│  └──────────┬───────────────────────────────────────────────┘  │
└─────────────┼──────────────────────────────────────────────────┘
              ▼
      WhatsApp servers (multi-device)
```

- **Protocol core is Rust.** `crates/whatsapp-core` owns the connection
  lifecycle, pairing, the event bus, and persistent state. The upstream
  `whatsapp-rust` crate provides the wire protocol and cryptography.
- **UI is a thin, replaceable shell.** The webview holds no business logic: it
  renders state it receives from Tauri commands/events and sends user intents
  back. If we ever want a GPUI UI, the core does not change.
- **Secrets stay local.** Session credentials never leave `~/Library/Application
  Support`. No telemetry, no analytics, no third-party servers.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the full design.

## Repository layout

```
crates/whatsapp-core/        protocol-agnostic domain core (Rust)
apps/desktop/                Tauri app: React UI + Rust host
  src/                       React 19 + TypeScript UI
  src-tauri/                 Tauri v2 host (Rust)
docs/                        architecture, roadmap, design spec, research
```

## Build & run

Prerequisites: macOS 13+, Xcode command line tools, Rust stable (1.94+),
Node.js 20+.

```bash
# 1. UI dependencies
cd apps/desktop && npm install

# 2. Development build (starts Vite + Tauri dev window)
npm run tauri dev

# 3. Production bundle → apps/desktop/src-tauri/target/release/bundle/
npm run tauri build

# 4. Rust tests
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Roadmap

| Milestone | Scope | Status |
|---|---|---|
| M0 Core connection | pairing (QR / pair code / passkey), history sync, reconnect, media pipeline | 🚧 in progress |
| M1 Everyday messaging | text, replies, receipts, typing, presence, notifications | ⏳ |
| M2 Chat management | archive/pin/mute/delete, starred, search, favorites | ⏳ |
| M3 Media experience | images, video, voice, documents, stickers, view-once, disappearing | ⏳ |
| M4 Message actions | reactions, edit, revoke, pins, forwarding, polls, events | ⏳ |
| M5 Groups & communities | admin, invite links, join requests, communities | ⏳ |
| M6 Status & channels | status/stories, channels/newsletters | ⏳ |
| M7 Calls | 1:1 audio/video, screen share, call links, group calls | ⏳ |
| M8 Privacy & security | privacy settings, app lock, chat lock, verification, proxy | ⏳ |
| M9 System integration | Siri, Shortcuts, widgets, share sheet, menu bar, deep links | ⏳ |
| M10 Power features | business profiles, catalog, labels, quick replies, usernames | ⏳ |
| M12 Out of scope | payments, Meta AI, interop bridges, E2E backups (Meta-gated) | ❌ |

The full evidence-backed plan — every feature of the official app mapped
against the open-source protocol stacks — lives in
[docs/parity-matrix.md](docs/parity-matrix.md); see
[docs/ROADMAP.md](docs/ROADMAP.md) for the condensed version.

## Legal & safety

- **Unofficial.** RustWA is not affiliated with, endorsed by, or connected to
  Meta Platforms, Inc. or WhatsApp LLC. "WhatsApp" is a trademark of Meta
  Platforms, Inc.; it is used here only to describe interoperability.
- **No proprietary assets.** No WhatsApp code, icons, fonts, sounds or other
  assets are included in this repository. Icons come from open-licensed sets
  (Lucide) or are drawn from scratch.
- **Terms of Service & account safety.** WhatsApp's Terms of Service do not
  officially permit third-party clients. Using an unofficial client **can get
  your account banned or restricted.** Use at your own risk, ideally with a
  secondary account. This project exists for interoperability research and
  personal use.
- **Security.** The `src-tauri` host and `whatsapp-core` are the only components
  with network and disk access; the UI is sandboxed by Tauri's CSP and IPC
  allow-lists.

## Credits

Standing on the shoulders of open-source giants:

- [`whatsapp-rust`](https://github.com/jlucaso1/whatsapp-rust) — Rust client for
  the WhatsApp Web protocol (MIT)
- [Baileys](https://github.com/WhiskeySockets/Baileys) — the TypeScript
  reference implementation of the multi-device protocol
- [whatsmeow](https://github.com/tulir/whatsmeow) — the Go implementation that
  documents the protocol exceptionally well
- [Tauri](https://tauri.app), [React](https://react.dev),
  [Vite](https://vite.dev), [Lucide](https://lucide.dev)

## License

MIT — see [LICENSE](LICENSE).
