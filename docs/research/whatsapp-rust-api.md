# whatsapp-rust 0.7.0 — Integration Guide for the macOS Client

**Status:** definitive API reference for building the native macOS WhatsApp client on top of the
upstream `whatsapp-rust` crate. Verified against the crates.io release **0.7.0** and the matching
git tag on 2026-09-13.

| Field | Value |
|---|---|
| Crate | `whatsapp-rust` |
| Version | `0.7.0` (crates.io checksum `3cb354c32641cfe832bf5c5767a016762234c5efd1b4ce979e2b3e32175d96bc`) |
| Upstream | <https://github.com/jlucaso1/whatsapp-rust> |
| v0.7.0 tag commit | `f8165f282008935732e0b26d6f5cca5038ecd222` (2026-08-06 23:25:49 -0300, "chore(release): prepare 0.7.0 (#1219)") |
| License | MIT |
| Edition / MSRV | Edition 2024, `rust-version = "1.94"` |
| Upstream pinned toolchain | `nightly-2026-06-16` (because of `portable_simd`; see [§1.2](#12-toolchain-nightly-vs-stable--read-this-first)) |
| Docs | <https://whatsapp-rust.jlucaso.com>, llms.txt at <https://whatsapp-rust.jlucaso.com/llms.txt> |
| Scope of this guide | Protocol core integration (`Bot`/`Client`, events, pairing, sending, groups, newsletters, presence, receipts, history sync, SQLite storage). |

> **Upstream disclaimer (README):** "This is an unofficial, open-source reimplementation. Using
> custom WhatsApp clients may violate Meta's Terms of Service and could result in account
> suspension. Use at your own risk."

---

## 0. Executive summary

- The crate ships **two layers**: a high-level `Bot` (typestate builder + closure callbacks +
  `run()`/`spawn()`; what most apps should use) and a low-level `Client` (full protocol surface,
  runtime-validated builder). Both are public; `MessageContext` bridges them.
- Everything the app needs is re-exported from the single `whatsapp-rust` dependency (including
  `wacore`, `wacore-binary`, `waproto`, the Tokio transport, ureq HTTP client, and SQLite store).
- **The default feature set does not compile on stable Rust.** It enables `simd`, which turns on
  `#![feature(portable_simd)]` in `wacore-binary`. On stable, use `default-features = false` with
  an explicit feature list ([§1.1](#11-recommended-cargotoml-stable-toolchain)). The upstream repo
  itself pins `nightly-2026-06-16`.
- The event system (`Event` enum + `EventKind`/`EventInterest` filters) is the heart of the
  integration: QR codes, inbound messages, receipts, presence, group changes, history sync,
  connection lifecycle, calls, etc. all arrive there.
- Persistence is pluggable; `SqliteStore` (bundled SQLite, Diesel) is the shipped adapter and is
  what we should use for the macOS app.

---

## 1. Dependencies and toolchain

### 1.1 Recommended Cargo.toml (stable toolchain)

The project pins `channel = "stable"` in `/Volumes/lucas-sn770/Projecten/whatsapp-rust/rust-toolchain.toml`,
so this is the config for us. It is the upstream default set **minus `simd`**:

```toml
[dependencies]
whatsapp-rust = { version = "0.7", default-features = false, features = [
    "sqlite-storage",   # SqliteStore (bundled SQLite via Diesel)
    "tokio-transport",  # TokioWebSocketTransportFactory
    "ureq-client",      # UreqHttpClient (media upload/download, version fetch)
    "tokio-runtime",    # TokioRuntime impl of the Runtime trait
    "tokio-native",     # tokio/rt-multi-thread
    "signal",           # whatsapp_rust::shutdown_signal() (SIGINT/SIGTERM helper)
] }
tokio = { version = "1", features = ["macros", "rt-multi-thread", "signal"] }
```

Verified: this exact set builds clean on **stable 1.98.1 (aarch64-apple-darwin)** in ~1m56s from a
cold target directory, including an application binary that exercises the API surface (see
`whatsapp-rust-api-compile-check.rs` next to this file).

### 1.2 Toolchain: nightly vs stable — READ THIS FIRST

| Config | Toolchain | Result on this machine |
|---|---|---|
| Upstream defaults (`default = ["simd", ...]`) | stable 1.98.1 | **FAILS**: `error[E0554]: #![feature] may not be used on the stable release channel` at `wacore-binary-0.7.0/src/lib.rs:1` (`feature(portable_simd)`) |
| Defaults **minus `simd`** (the snippet above) | stable 1.98.1 | **PASSES** |
| Upstream defaults incl. `simd` | nightly-2026-06-16 (`rustc 1.98.0-nightly`) | **PASSES** |

Implication for the macOS client: either

1. **Use stable + the feature list above** (recommended: stable toolchain is already pinned in the
   project), or
2. Pin `nightly-2026-06-16` (upstream's `rust-toolchain.toml`) and keep `simd` for the extra
   performance.

`simd` only affects `wacore-binary` hot paths; the public API is identical either way. Upstream CI
has a dedicated "Test Stable (no-simd)" job, so the no-simd configuration is a supported path.

### 1.3 Feature flags (v0.7.0, complete list)

Source: upstream `Cargo.toml` at tag `v0.7.0` (identical in the published package).

| Feature | Default | Effect |
|---|---|---|
| `simd` | yes | `wacore/simd`; requires nightly (`portable_simd`). |
| `sqlite-storage` | yes | `whatsapp-rust-sqlite-storage`; enables `store::SqliteStore`. |
| `tokio-transport` | yes | `TokioWebSocketTransportFactory` (WebSocket transport). |
| `tokio-runtime` | yes | `TokioRuntime` + Tokio dependency. |
| `tokio-native` | yes | `tokio/rt-multi-thread`. |
| `ureq-client` | yes | `UreqHttpClient` (blocking HTTP client used by media + version fetch). |
| `signal` | yes | `shutdown_signal()`; implies `tokio/signal`. |
| `plugins` | no | Native plugin host; implies `client-lifecycle`; adds `bon`. |
| `client-lifecycle` | no | Generation-scoped extension lifecycle (`ClientLifecycle`, `ConnectionScope`). |
| `tracing` | no | Emits `tracing` spans/events (app installs subscriber). |
| `tracing-pii` | no | Prints raw phone numbers in tracing fields; **debug only, never production**. |
| `metrics` | no | `metrics` facade counter/histogram emission. |
| `legacy-session-interop` | no | Migration surface for legacy SessionRecord v1 auth state. |
| `debug-snapshots` | no | Debug snapshots only. |
| `danger-skip-tls-verify` | no | Disables TLS verification. **Never ship.** |
| `danger-skip-cert-chain-verify` | no | Weakens Noise handshake cert-chain checks. **Never ship.** |
| `voip` / `voip-runtime` / `voip-encoded` / `voip-mlow` / `voip-libopus` | no | 1:1 VoIP audio. `voip` = MLOW + libopus; `voip-encoded` = no codec linked; `voip-runtime` = shared runtime. |

With `default-features = false` and no runtime/transport/http/storage features, the `Bot` builder
requires the corresponding `with_transport_factory`, `with_http_client`, `with_runtime` calls before
`build()` is reachable (typestate).

### 1.4 Published crate graph (why one dependency is enough)

`whatsapp-rust` 0.7.0 re-exports the whole stack. The git workspace members are published as
separate crates and are pulled in as normal dependencies of the main crate:

| Published crate | Workspace path | Role |
|---|---|---|
| `wacore` 0.7.0 | `wacore/` | Platform-agnostic core (protocol, crypto, IQ, state traits). |
| `wacore-binary` 0.7.0 | `wacore/binary/` | WhatsApp binary XML codec (`Node`, `Jid`, marshalling). |
| `wacore-appstate` 0.7.0 | `wacore/appstate/` | App-state (sync) engine. |
| `wacore-libsignal` 0.7.0 | `wacore/libsignal/` | Signal protocol. |
| `wacore-noise` 0.7.0 | `wacore/noise/` | Noise handshake. |
| `waproto` 0.7.0 | `waproto/` | Protobuf types (`wa::Message`, etc.). |
| `whatsapp-rust-sqlite-storage` 0.7.0 | `storages/sqlite-storage/` | SQLite backend. |
| `whatsapp-rust-tokio-transport` 0.7.0 | `transports/tokio-transport/` | Tokio WebSocket transport. |
| `whatsapp-rust-ureq-http-client` 0.7.0 | `http_clients/ureq-client/` | ureq HTTP client. |

(The crates.io source of all nine packages was diffed against the v0.7.0 tag: `src/` is identical
for every one.)

The main crate also re-exports the third-party crates whose types appear in its API so versions
cannot drift: `anyhow`, `async_channel`, `async_trait`, `bytes`, `futures`, `serde`,
`serde_json`, `wacore::chrono`, `waproto::buffa`. Prefer these re-exports in our code
(`whatsapp_rust::anyhow`, …).

---

## 2. Architecture: the two layers

```text
whatsapp_rust::bot
├── Bot            — configured session, handlers attached at build time
├── BotBuilder     — typestate builder: Backend / TransportFactory / HttpClient / Runtime
├── BotHandle      — background handle: client(), shutdown().await, abort(), impl Future
├── EventDelivery  — Concurrent (default) | Ordered { capacity }
└── MessageContext — per-inbound-message helpers (reply/react/edit/revoke/send)

whatsapp_rust::client
├── Client         — the full protocol surface (Arc<Client> is what handlers get)
├── ClientBuilder  — low-level, runtime-validated builder (no typestate; for FFI/embedded)
├── ClientBuild    — into_client() | into_parts()
└── errors: ClientError, ConnectError, ConnectStage, SignalMaintenanceError
```

- **`Bot`** is the recommended integration point: `.build()` initializes the device row in storage,
  subscribes our callbacks to the event bus, optionally starts the pair-code task and the
  sync-task worker, and returns a `Bot`. `bot.run().await` drives connect + auto-reconnect until
  logout/disconnect; `bot.spawn()` starts it on the runtime and returns a `BotHandle`.
- **`Client`** is reachable at any time via `bot.client()` / `handle.client()` and exposes
  everything (`send_*`, `groups()`, `newsletter()`, `presence()`, `mark_as_read`, `logout`, …).
- Handlers receive `Arc<Client>`, so everything is usable from callbacks.

`BotBuilder` is typestate: `build()` exists only on
`BotBuilder<Provided, Provided, Provided, Provided>`. With default (or our recommended) features,
transport/HTTP/runtime are pre-filled and only `with_backend` is required.

---

## 3. Minimal end-to-end example (create → QR → connect → send)

### 3.1 Upstream README quick start (quoted verbatim)

`README.md` at v0.7.0:

```toml
[dependencies]
whatsapp-rust = "0.7"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "signal"] }
```

```rust,no_run
use whatsapp_rust::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bot = Bot::builder()
        .with_backend(SqliteStore::new("whatsapp.db").await?)
        .on_qr_code(|code, _timeout| async move {
            println!("Scan to pair:\n{code}");
        })
        .on_message(|ctx| async move {
            if ctx.message.text_content() == Some("ping") {
                let _ = ctx.reply("pong").await;
            }
        })
        .build()
        .await?;

    // Runs until logout or shutdown; a single await.
    bot.run().await;
    Ok(())
}
```

> Note: the README's `whatsapp-rust = "0.7"` uses default features, which require nightly. On our
> stable toolchain use [§1.1](#11-recommended-cargotoml-stable-toolchain).

### 3.2 Our recommended skeleton (compile-verified against 0.7.0)

This is the shape to put into the macOS client. It uses only real upstream names; the exact same
code is part of `whatsapp-rust-api-compile-check.rs` which compiled successfully.

```rust,no_run
use std::sync::Arc;
use whatsapp_rust::prelude::*;                 // Bot, SqliteStore, Event, Jid, wa, MessageField, ...
use whatsapp_rust::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Storage: file path is resolved by Diesel; use an absolute path inside
    //    the app sandbox container, e.g. Application Support/whatsapp.db.
    let store = SqliteStore::new("whatsapp.db").await?;

    // 2. Configure the bot: backend + lifecycle handlers.
    let bot = Bot::builder()
        .with_backend(store)
        .on_qr_code(|code, timeout| async move {
            // `code` is the raw QR payload string; render it as a QR image in the UI.
            // It rotates: first ref is valid ~60s, the next five ~20s each.
            let _ = timeout;
            println!("QR code (valid {:?}): {code}", timeout);
        })
        .on_connected(|client: Arc<Client>| async move {
            println!("connected as {:?}", client.pn());
        })
        .on_logged_out(|info| async move {
            // Unlinked from the phone; must re-pair (QR or pair-code).
            eprintln!("logged out: {:?}", info.reason);
        })
        .on_message(|ctx| async move {
            // Every decrypted inbound message. `ctx.message` is wa::Message.
            if ctx.message.text_content() == Some("ping") {
                let _ = ctx.reply("pong").await;              // same chat, no quote
                let _ = ctx.reply_quoting("pong!").await;     // quotes the inbound message
            }
        })
        .build()
        .await?;

    // 3. Send once connected. `client()` is valid immediately; sending before
    //    login fails with SendError::NotLoggedIn.
    let client: Arc<Client> = bot.client();

    // Drive the connection until shutdown. run() owns the auto-reconnect loop.
    tokio::spawn({
        let client = client.clone();
        async move {
            let _ = client.wait_for_connected(std::time::Duration::from_secs(60)).await;
            let to: Jid = "15551234567@s.whatsapp.net".parse().unwrap();
            match client.send_text(to, "hello from macOS").await {
                Ok(sent) => println!("sent {} to {}", sent.message_id, sent.to),
                Err(e) => eprintln!("send failed: {e}"),
            }
        }
    });

    bot.run().await; // never returns except after logout/disconnect
    Ok(())
}
```

### 3.3 Background + graceful shutdown (quoted from README, adapted)

The README's spawn pattern:

```rust,no_run
use whatsapp_rust::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bot = Bot::builder()
        .with_backend(SqliteStore::new("whatsapp.db").await?)
        .build()
        .await?;

    let handle = bot.spawn(); // full Client API stays available via handle.client()

    tokio::signal::ctrl_c().await?;
    handle.shutdown().await; // graceful: flushes pending state, then stops
    Ok(())
}
```

Upstream Unix signal helper (behind the `signal` feature):

```rust
use whatsapp_rust::shutdown_signal;

tokio::select! {
    _ = &mut handle => {}                       // bot stopped by itself
    _ = shutdown_signal() => handle.shutdown().await, // SIGINT or SIGTERM
}
```

`BotHandle` implements `Future<Output = ()>`; dropping it aborts the bot task, so keep it alive.

---

## 4. Pairing

### 4.1 QR pairing

The client receives `<pair-device>` refs and emits `Event::PairingQrCode` for each rotation:

- First ref: valid **60 s**; subsequent refs: **20 s** each; at most six refs per connection
  (`60s + 5×20s`). After the refs run out, `Event::PairingQrCodesExhausted { disconnected }` is
  dispatched. QR-only flows self-disconnect (`disconnected = true`); if a pair-code flow is still
  outstanding, the socket is kept up (`disconnected = false`).
- The QR payload embeds the ADV secret; if the secret is re-minted the client re-emits the payload
  for the same ref automatically.

High-level (preferred):

```rust
Bot::builder()
    .with_backend(store)
    .on_qr_code(|code, timeout| async move { render_qr(&code, timeout); })
```

Raw events (for a UI that installs its own bus handler):

```rust
use whatsapp_rust::types::events::{Event, EventHandler, EventInterest, EventKind, Subscription};

client.subscribe(
    EventInterest::of(&[EventKind::PairingQrCode, EventKind::PairingQrCodesExhausted]),
    handler_arc,
); // -> Subscription; dropping it unsubscribes
```

Payloads:

```rust
pub struct PairingQrCode { pub code: String, pub timeout: std::time::Duration }
pub struct PairingQrCodesExhausted { pub disconnected: bool }
```

### 4.2 Pair-code (phone-number linking)

`PairCodeOptions` (`whatsapp_rust::pair_code::PairCodeOptions`, re-exported from `wacore`):

```rust
pub struct PairCodeOptions {
    /// Phone number with country code, no leading zeros or special chars (e.g. "15551234567").
    pub phone_number: String,
    /// Whether to show push notification on phone (default `true`, matching WA Web).
    pub show_push_notification: bool,
    /// Custom pairing code (8 chars from Crockford alphabet, or None for random).
    pub custom_code: Option<String>,
    /// `None` auto-derives from `Device.device_props.platform_type`.
    pub platform_id: Option<CompanionWebClientType>,
    /// Advanced OS override for `companion_platform_display`; Some(os) sends verbatim (server may reject).
    pub display_os: Option<String>,
}
// Default: phone_number empty, show_push_notification: true, rest None.
```

Two ways to use it:

**A. Builder-driven (runs detached after the socket is ready, concurrent with QR):**

```rust
let mut builder = Bot::builder()
    .with_backend(store)
    .on_pair_code(|code, timeout| async move {
        // 8-char code; enter on phone: WhatsApp > Linked Devices > Link a Device
        // > Link with phone number instead.
        println!("pair code {code} (valid {:?})", timeout);
    })
    .on_pair_code_error(|err, _client| async move {
        // The only failure surface for the detached request. Branch on `err.rejection`.
        if err.rejection.is_some_and(|r| r.is_throttled()) {
            // back off before requesting again
        }
    })
    .on_pair_code_refresh(|force_manual, client| async move {
        // Server asked to regenerate; call client.pair_with_code(...) again when force_manual,
        // or auto-rotate otherwise.
        let _ = (force_manual, client);
    });

builder = builder.with_pair_code(PairCodeOptions {
    phone_number: "15551234567".to_string(),
    custom_code: None,
    ..Default::default()
});

let bot = builder.build().await?;
```

**B. Direct call (upstream doc example, verbatim):**

```rust,no_run
use whatsapp_rust::pair_code::PairCodeOptions;

# async fn example(client: std::sync::Arc<whatsapp_rust::Client>) -> Result<(), Box<dyn std::error::Error>> {
let options = PairCodeOptions {
    phone_number: "15551234567".to_string(),
    show_push_notification: true,
    custom_code: None, // Generate random code
    ..Default::default()
};

let code = client.pair_with_code(options).await?;
println!("Enter this code on your phone: {}", code);
# Ok(())
# }
```

Semantics that matter for UX:

- **One code at a time**: a second request fails with `PairCodeError::CodeAlreadyOutstanding` until
  the first expires or `client.cancel_pair_code().await` is called. Never drive pair-code from QR
  rotation.
- `pair_with_code` both returns the code **and** dispatches `Event::PairingCode`; failures both
  return `Err(PairError)` **and** dispatch `Event::PairingCodeError` (except two cases that are
  deliberately silent: `CodeAlreadyOutstanding` and `Cancelled`).
- `PairingCodeError { rejection: Option<PairCodeRejection>, backoff: Option<Duration>, error: String }`.
  `PairCodeRejection::{BadRequest, Forbidden, RateOverlimit, FeatureNotAvailable,
  InternalServerError, Unknown(i32)}`; `is_throttled()` is true for `RateOverlimit` **and**
  `BadRequest` (the server throttles per number under 400). Do not treat a 400 as permanently fatal.
- `Event::PairingCodeRefresh { force_manual }` means the in-progress code must be replaced
  (server request, or a `companion_finish` silence timeout).
- Pair-code and QR run concurrently; whichever completes first wins.

### 4.3 Pair success / failure

```rust
pub struct PairSuccess { pub id: Jid, pub lid: Jid, pub business_name: String, pub platform: String }
pub struct PairError   { pub id: Jid, pub lid: Jid, pub business_name: String, pub platform: String,
                         pub error: String }
```

`PairError` is the protocol-level pairing failure; `PairingCodeError` is the phone-number-flow
failure. After `PairSuccess`, the device row is populated (`pn`, `lid`, `account`, push name) and
`bot` continues into the connected/login flow. Expect an immediate reconnect (515-style) right
after pairing; `Client::run` handles it.

### 4.4 Passkeys (SHORTCAKE_PASSKEY)

Events exist for WebAuthn-gated linking: `Event::PairPasskeyRequest { request_options_json }`,
`Event::PairPasskeyConfirmation { code, skip_handoff_ux }`, `Event::PairPasskeyError { error,
continuation }`. A passkey authenticator, if registered with the client, drives the assertion
automatically; otherwise the host must respond. Not required for basic QR/pair-code flows.

---

## 5. Event system

### 5.1 Registration paths

Three public ways to receive events:

1. **`BotBuilder` closures (recommended)** — typed helpers, interest-filtered automatically:

```rust
pub fn on_event<F, Fut>(self, handler: F) -> Self
where F: Fn(Arc<Event>, Arc<Client>) -> Fut + Send + Sync + 'static, Fut: Future<Output = ()> + Send + 'static;

pub fn on_event_for<F, Fut>(self, kinds: &[EventKind], handler: F) -> Self;
pub fn on_message<F, Fut>(self, handler: F) -> Self
where F: Fn(MessageContext) -> Fut + Send + Sync + 'static;
pub fn on_qr_code<F, Fut>(self, handler: F) -> Self
where F: Fn(String, std::time::Duration) -> Fut + Send + Sync + 'static;
pub fn on_pair_code<F, Fut>(self, handler: F) -> Self
where F: Fn(String, std::time::Duration) -> Fut + Send + Sync + 'static;
pub fn on_pair_code_error<F, Fut>(self, handler: F) -> Self
where F: Fn(PairingCodeError, Arc<Client>) -> Fut + Send + Sync + 'static;
pub fn on_pair_code_refresh<F, Fut>(self, handler: F) -> Self
where F: Fn(bool, Arc<Client>) -> Fut + Send + Sync + 'static;
pub fn on_connected<F, Fut>(self, handler: F) -> Self
where F: Fn(Arc<Client>) -> Fut + Send + Sync + 'static;
pub fn on_logged_out<F, Fut>(self, handler: F) -> Self
where F: Fn(LoggedOut) -> Fut + Send + Sync + 'static;
pub fn with_event_handler(self, handler: impl EventHandler + 'static) -> Self; // inline, stateful
```

2. **Struct handler** — implement `EventHandler` and pass via `with_event_handler` (runs inline on
   the dispatch path; spawn your own task for slow work), or subscribe on the live client:

```rust
pub trait EventHandler: MaybeSendSync {
    fn handle_event(&self, event: Arc<Event>);
    fn interest(&self) -> EventInterest { EventInterest::ALL }
}

impl Client {
    pub fn subscribe(&self, interest: EventInterest, handler: Arc<dyn EventHandler>) -> Subscription;
    pub fn subscribe_handler(&self, handler: Arc<dyn EventHandler>) -> Subscription;
}
```

3. **Channel bridge** — the easiest way to feed your UI event loop:

```rust
pub struct ChannelEventHandler { /* … */ }
impl ChannelEventHandler {
    pub fn new() -> (Arc<Self>, async_channel::Receiver<Arc<Event>>);
}
// usage:
let (handler, rx) = ChannelEventHandler::new();
let subscription = client.subscribe_handler(handler);
while let Ok(event) = rx.recv().await {
    match &*event { /* … */ }
}
subscription.detach(); // or drop it to unsubscribe
```

`Subscription` has `update_interest(EventInterest) -> bool`, `unsubscribe(self) -> bool`, and
`detach(self)` (keep forever). Dropping a non-detached `Subscription` unregisters the handler.

### 5.2 Interest filtering

```rust
#[repr(u8)] #[non_exhaustive]
pub enum EventKind { Connected, Disconnected, …, AppStateSyncFailed } // 61 kinds, append-only

pub struct EventInterest(u128);
impl EventInterest {
    pub const ALL: EventInterest;
    pub const fn none() -> Self;
    pub fn of(kinds: &[EventKind]) -> Self;
    pub const fn with(self, kind: EventKind) -> Self;
    pub const fn wants(self, kind: EventKind) -> bool;
    pub const fn union(self, other: Self) -> Self;
}
```

- `EventKind` discriminants are **frozen, append-only** (they are persisted/transmitted and pack
  into a `u128`; 128-kind ceiling).
- The bus skips materializing an event kind no handler wants (`has_handler_for`), so narrow filters
  are a real optimization. A catch-all `on_event` widens the union to `ALL`.

### 5.3 Delivery semantics (`EventDelivery`)

```rust
#[non_exhaustive]
pub enum EventDelivery {
    #[default]
    Concurrent,                 // each event -> its own spawned task per callback
    Ordered { capacity: usize }, // single bounded mailbox, arrival-order drain, drops when full
}
```

- `Concurrent` (default): a slow callback stalls nothing, but **cross-event ordering is not
  guaranteed** and a persistently slow consumer accumulates unbounded in-flight tasks.
- `Ordered { capacity }`: strictly in-arrival-order delivery to callbacks; when the mailbox is full
  the event is **dropped** and counted in `StatsSnapshot::events_dropped`. For "no drop" use an
  inbound durability hook ([§10.4](#104-at-least-once-delivery-inbound-durability-hook)).
- Raw `with_event_handler` handlers always run inline on the dispatch path regardless of this
  setting.

### 5.4 Complete `Event` variant table (v0.7.0)

`Event` is `#[non_exhaustive]` + `Serialize`; payload structs are `#[non_exhaustive]` with `bon`
builders. **Always match with a `_` arm.** Debug prints only the variant name; serialize for full
contents. Variants in declaration order:

| # | Event variant | Payload fields | `EventKind` |
|---|---|---|---|
| 1 | `Connected(Connected)` | `{}` (marker) | `Connected` |
| 2 | `Disconnected(Disconnected)` | `reason: DisconnectReason` | `Disconnected` |
| 3 | `PairSuccess(PairSuccess)` | `id: Jid, lid: Jid, business_name: String, platform: String` | `PairSuccess` |
| 4 | `PairError(PairError)` | same + `error: String` | `PairError` |
| 5 | `LoggedOut(LoggedOut)` | `on_connect: bool, reason: ConnectFailureReason, logout_message: Option<LogoutMessage>, raw: Option<Node>` | `LoggedOut` |
| 6 | `PairingQrCode(PairingQrCode)` | `code: String, timeout: Duration` | `PairingQrCode` |
| 7 | `PairingCode(PairingCode)` | `code: String, timeout: Duration` | `PairingCode` |
| 8 | `PairingCodeRefresh(PairingCodeRefresh)` | `force_manual: bool` | `PairingCodeRefresh` |
| 9 | `PairingCodeError(PairingCodeError)` | `rejection: Option<PairCodeRejection>, backoff: Option<Duration>, error: String` | `PairingCodeError` |
| 10 | `PairingQrCodesExhausted(PairingQrCodesExhausted)` | `disconnected: bool` | `PairingQrCodesExhausted` |
| 11 | `QrScannedWithoutMultidevice(QrScannedWithoutMultidevice)` | `{}` (marker) | `QrScannedWithoutMultidevice` |
| 12 | `ClientOutdated(ClientOutdated)` | `raw: Option<Node>` | `ClientOutdated` |
| 13 | `Messages(MessageBatch)` | `messages: Arc<[InboundMessage]>, origin: BatchOrigin, hook_committed: bool` | `Messages` |
| 14 | `Receipt(Receipt)` | `source: MessageSource, message_ids: Vec<MessageId>, timestamp: DateTime<Utc>, type: ReceiptType, offline: bool` | `Receipt` |
| 15 | `ServerAck(ServerAck)` | `id: String, class: Option<String>, from: Option<Jid>, timestamp: Option<DateTime<Utc>>, error: Option<String>` | `ServerAck` |
| 16 | `UndecryptableMessage(UndecryptableMessage)` | `info: Arc<MessageInfo>, is_unavailable: bool, unavailable_type: UnavailableType, decrypt_fail_mode: DecryptFailMode` | `UndecryptableMessage` |
| 17 | `Notification(Arc<OwnedNodeRef>)` | raw notification stanza | `Notification` |
| 18 | `ChatPresence(ChatPresenceUpdate)` | `source: MessageSource, state: ChatPresence, media: ChatPresenceMedia` | `ChatPresence` |
| 19 | `Presence(PresenceUpdate)` | `from: Jid, unavailable: bool, last_seen: Option<DateTime<Utc>>` | `Presence` |
| 20 | `PictureUpdate(PictureUpdate)` | `jid, author: Option<Jid>, timestamp, removed: bool, picture_id: Option<String>` | `PictureUpdate` |
| 21 | `UserAboutUpdate(UserAboutUpdate)` | `jid, status: String, timestamp` | `UserAboutUpdate` |
| 22 | `ContactUpdated(ContactUpdated)` | `jid, timestamp` | `ContactUpdated` |
| 23 | `ContactNumberChanged(ContactNumberChanged)` | `old_jid, new_jid, old_lid: Option<Jid>, new_lid: Option<Jid>, timestamp` | `ContactNumberChanged` |
| 24 | `ContactSyncRequested(ContactSyncRequested)` | `after: Option<DateTime<Utc>>, timestamp` | `ContactSyncRequested` |
| 25 | `GroupUpdate(GroupUpdate)` | `group_jid, notification_id, notify, offline, action_index, participant, participant_pn, participant_username, participant_country_code, timestamp, is_lid_addressing_mode, has_incomplete_participant_information, action: GroupNotificationAction` | `GroupUpdate` |
| 26 | `ContactUpdate(ContactUpdate)` | `jid, timestamp, action: Box<ContactAction>, from_full_sync` | `ContactUpdate` |
| 27 | `IncomingCall(IncomingCall)` | `from, stanza_id, notify, platform, version, participant, recipient, timestamp, offline, action: CallAction, group: Option<Box<GroupCallUpdate>>` | `IncomingCall` |
| 28 | `MissedCall(MissedCall)` | replay of a dead offer; must not ring | `MissedCall` |
| 29 | `CallEndedElsewhere(CallEndedElsewhere)` | answered/declined on another device | `CallEndedElsewhere` |
| 30 | `PushNameUpdate(PushNameUpdate)` | `jid, message: Box<MessageInfo>, old_push_name, new_push_name` | `PushNameUpdate` |
| 31 | `SelfPushNameUpdated(SelfPushNameUpdated)` | `from_server: bool, old_name, new_name` | `SelfPushNameUpdated` |
| 32 | `PinUpdate(PinUpdate)` | `jid, timestamp, action: Box<PinAction>, from_full_sync` | `PinUpdate` |
| 33 | `MuteUpdate(MuteUpdate)` | `jid, timestamp, action: Box<MuteAction>, from_full_sync` | `MuteUpdate` |
| 34 | `ArchiveUpdate(ArchiveUpdate)` | `jid, timestamp, action: Box<ArchiveChatAction>, from_full_sync` | `ArchiveUpdate` |
| 35 | `StarUpdate(StarUpdate)` | `chat_jid, participant_jid: Option<Jid>, message_id, from_me, timestamp, action, from_full_sync` | `StarUpdate` |
| 36 | `MarkChatAsReadUpdate` | `jid, timestamp, action, from_full_sync` | `MarkChatAsReadUpdate` |
| 37 | `DeleteChatUpdate` | `jid, delete_media, timestamp, action, from_full_sync` | `DeleteChatUpdate` |
| 38 | `ClearChatUpdate` | `jid, delete_starred, delete_media, timestamp, action, from_full_sync` | `ClearChatUpdate` |
| 39 | `UserStatusMuteUpdate` | `jid, muted: bool, timestamp, action, from_full_sync` | `UserStatusMuteUpdate` |
| 40 | `DeleteMessageForMeUpdate` | `chat_jid, participant_jid, message_id, from_me, timestamp, action, from_full_sync` | `DeleteMessageForMeUpdate` |
| 41 | `LabelEditUpdate` | `label_id, timestamp, action, from_full_sync` | `LabelEditUpdate` |
| 42 | `LabelAssociationUpdate` | `label_id, chat_jid, timestamp, action, from_full_sync` | `LabelAssociationUpdate` |
| 43 | `HistorySync(Box<LazyHistorySync>)` | see [§9](#9-presence-chat-state-and-history-sync) | `HistorySync` |
| 44 | `OfflineSyncPreview(OfflineSyncPreview)` | `total, app_data_changes, messages, notifications, receipts, calls, statuses` (i32) | `OfflineSyncPreview` |
| 45 | `OfflineSyncCompleted(OfflineSyncCompleted)` | `count: i32` | `OfflineSyncCompleted` |
| 46 | `DirtyState(DirtyState)` | `dirty_type: DirtyType, timestamp: Option<u64>` | `DirtyState` |
| 47 | `DeviceListUpdate(DeviceListUpdate)` | `user, lid_user: Option<Jid>, update_type: DeviceListUpdateType, devices: Vec<DeviceNotificationInfo>, key_index, contact_hash` | `DeviceListUpdate` |
| 48 | `IdentityChange(IdentityChange)` | `user, lid_user: Option<Jid>, implicit: bool` | `IdentityChange` |
| 49 | `BusinessStatusUpdate(BusinessStatusUpdate)` | `jid, update_type, timestamp, target_jid, hash, verified_name, product_ids, collection_ids, subscriptions` | `BusinessStatusUpdate` |
| 50 | `StreamReplaced(StreamReplaced)` | `{}` (marker) | `StreamReplaced` |
| 51 | `TemporaryBan(TemporaryBan)` | `code: TempBanReason, expire: Duration, message, url, raw` | `TemporaryBan` |
| 52 | `ConnectFailure(ConnectFailure)` | `reason: ConnectFailureReason, message, raw` | `ConnectFailure` |
| 53 | `StreamError(StreamError)` | `code: String, raw: Option<Node>` | `StreamError` |
| 54 | `DisappearingModeChanged(DisappearingModeChanged)` | `from: Jid, duration: u32, setting_timestamp: DateTime<Utc>` | `DisappearingModeChanged` |
| 55 | `NewsletterLiveUpdate(NewsletterLiveUpdate)` | `newsletter_jid: Jid, messages: Vec<NewsletterLiveUpdateMessage>` | `NewsletterLiveUpdate` |
| 56 | `RawNode(Arc<OwnedNodeRef>)` | raw decoded stanza; requires `acquire_raw_node_forwarding()` lease | `RawNode` |
| 57 | `MexNotification(MexNotification)` | `op_name, from, stanza_id, offline, payload: serde_json::Value` | `MexNotification` |
| 58 | `PairPasskeyRequest(PairPasskeyRequest)` | `request_options_json: String` | `PairPasskeyRequest` |
| 59 | `PairPasskeyConfirmation(PairPasskeyConfirmation)` | `code: String, skip_handoff_ux: bool` | `PairPasskeyConfirmation` |
| 60 | `PairPasskeyError(PairPasskeyError)` | `error: String, continuation: bool` | `PairPasskeyError` |
| 61 | `AppStateSyncFailed(AppStateSyncFailed)` | `fatal: Vec<String>, retryable: Vec<String>, skipped: Vec<String>, connected: bool` | `AppStateSyncFailed` |

Helpers on `Event`:

```rust
impl Event {
    pub fn kind(&self) -> EventKind;
    pub fn as_messages(&self) -> Option<&MessageBatch>;
    pub fn messages(&self) -> impl Iterator<Item = &InboundMessage>; // empty for other kinds
}
```

### 5.5 Inbound message payload

```rust
pub struct InboundMessage { pub message: Arc<wa::Message>, pub info: Arc<MessageInfo> }
pub struct MessageBatch {
    pub messages: Arc<[InboundMessage]>, pub origin: BatchOrigin, pub hook_committed: bool,
}
pub enum BatchOrigin { Live, OfflineDrain }
impl MessageBatch {
    pub fn iter(&self) -> std::slice::Iter<'_, InboundMessage>;
    pub fn len(&self) -> usize;  pub fn is_empty(&self) -> bool;  pub fn first(&self) -> Option<&InboundMessage>;
}
// &MessageBatch: IntoIterator<Item = &InboundMessage>
```

`MessageContext` (what `on_message` gives you):

```rust
#[derive(Clone)]
pub struct MessageContext {
    pub message: Arc<wa::Message>,
    pub info: MessageInfo,
    pub client: Arc<Client>,
}
impl MessageContext {
    pub fn from_parts(message: &wa::Message, info: &MessageInfo, client: Arc<Client>) -> Self;
    pub fn from_arc(message: Arc<wa::Message>, info: &MessageInfo, client: Arc<Client>) -> Self;
    pub fn from_inbound(inbound: &wacore::types::events::InboundMessage, client: Arc<Client>) -> Self;

    pub async fn send_message(&self, message: wa::Message) -> Result<SendResult, SendError>;
    pub async fn reply(&self, text: impl Into<String>) -> Result<SendResult, SendError>;
    pub async fn reply_quoting(&self, text: impl Into<String>) -> Result<SendResult, SendError>;
    pub fn build_quote_context(&self) -> wa::ContextInfo;
    pub fn message_key(&self) -> wa::MessageKey;
    pub async fn edit_message(&self, original_message_id: impl Into<String>, new_message: wa::Message)
        -> Result<String, SendError>;
    pub async fn revoke_message(&self, message_id: impl Into<String>, revoke_type: RevokeType)
        -> Result<(), SendError>;
    pub async fn react(&self, emoji: &str) -> Result<SendResult, SendError>;
}
```

`MessageInfo` fields (the ones UI code will read):

```rust
pub struct MessageInfo {
    pub source: MessageSource,
    pub id: String,                          // stanza/message id
    pub server_id: MessageServerId,          // server-assigned id
    pub r#type: String,
    pub push_name: String,
    pub timestamp: DateTime<Utc>,
    pub category: MessageCategory,           // Empty | Peer | Other(String)
    pub multicast: bool,
    pub media_type: String,
    pub edit: EditAttribute,                 // "" | 1 MessageEdit | 2 PinInChat | 3 AdminEdit | 7 SenderRevoke | 8 AdminRevoke
    pub bot_info: Option<MsgBotInfo>,
    pub meta_info: MsgMetaInfo,
    pub verified_name: Option<Box<VerifiedName>>,
    pub device_sent_meta: Option<DeviceSentMeta>,
    pub ephemeral_expiration: Option<u32>,
    pub is_offline: bool,
    pub unavailable_request_id: Option<String>,
    pub server_timestamp_us: Option<i64>,
    pub verified_level: Option<String>,
    pub verified_name_serial: Option<i64>,
    pub peer_recipient_pn: Option<Jid>,
    pub comment_target: Option<wa::MessageKey>,
    pub bcl_participants: Vec<Jid>,
}
pub struct MessageSource {
    pub chat: Jid, pub sender: Jid, pub is_from_me: bool, pub is_group: bool,
    pub addressing_mode: Option<AddressingMode>,
    pub sender_alt: Option<Jid>, pub recipient_alt: Option<Jid>,
    pub broadcast_list_owner: Option<Jid>, pub recipient: Option<Jid>,
}
```

`MessageExt` helpers on `wa::Message` (in the prelude): `get_base_message()`, `into_base_message()`,
`is_ephemeral()`, `is_view_once()`, `get_caption()`, `text_content()`, `prepare_for_quote()`,
`prepare_for_forward()`, `set_context_info()`, `get_ephemeral_expiration()`,
`set_ephemeral_expiration()`, `is_forwarded()`, `mentions_any_bot()`.
`MessageBuilderExt`: `wa::Message::text(...)`, `wa::Message::text_with_context(text, ctx)`.

---

## 6. Sending messages

All send APIs are `Client` methods; from a `MessageContext` use `ctx.send_message` or `ctx.client`.
Errors are `SendError` (`NotLoggedIn`, `Iq`, `InvalidRequest`, `Client`, `Internal`). Result:

```rust
#[non_exhaustive]
pub struct SendResult { pub message_id: String, pub to: Jid }
impl SendResult { pub fn message_key(&self) -> wa::MessageKey; } // from_me = true, participant = None
```

### 6.1 Core send API (exact signatures)

```rust
impl Client {
    pub fn send_message(&self, to: impl Into<Jid>, message: wa::Message)
        -> impl Future<Output = Result<SendResult, SendError>> + '_;

    pub fn send_text(&self, to: impl Into<Jid>, text: impl Into<String>)
        -> impl Future<Output = Result<SendResult, SendError>> + '_;

    pub fn forward_message(&self, to: impl Into<Jid>, message: &wa::Message)
        -> impl Future<Output = Result<SendResult, SendError>> + '_;

    pub fn send_message_with_options(&self, to: impl Into<Jid>, message: wa::Message, options: SendOptions)
        -> impl Future<Output = Result<SendResult, SendError>> + '_;
}
```

`SendOptions` (`#[non_exhaustive]`, chainable setters):

```rust
pub struct SendOptions {
    pub message_id: Option<String>,             // override auto-generated id (idempotency/resend)
    pub extra_stanza_nodes: Vec<Node>,          // extra XML children
    pub ephemeral_expiration: Option<u32>,      // seconds: 86400 / 604800 / 7776000
    pub stanza_type_override: Option<StanzaType>, // escape hatch; default derives from content
    pub group_metadata_freshness: crate::cache::Freshness,
    pub device_freshness: crate::cache::Freshness,
}
// .with_message_id(..) .with_extra_stanza_nodes(..) .with_ephemeral_expiration(..)
// .with_stanza_type_override(..) .with_group_metadata_freshness(..) .with_device_freshness(..)
```

Notes:

- `send_message` routes newsletters as plaintext (SMAX) automatically; status/story must go through
  `client.status()`.
- The send path generates a message id, stamps the biz node, handles sender-key distribution and
  retry caching. `SendResult.to` is the original target JID.
- Forwarding: `forward_message` strips the quote/mention chain, sets `is_forwarded` and bumps the
  forwarding score; existing media is relayed from the same CDN blob (no re-upload).
- `wa::Message::text(...)` (bare `conversation`) vs `text_with_context(...)` (promotes to
  `extendedTextMessage`); attaching any context (quote, mentions, expiration) promotes the type.

### 6.2 Replies / quotes

`MessageContext::reply_quoting(text)` is the canonical same-chat reply. For a manual quote:

```rust
use whatsapp_rust::wacore::proto_helpers::{build_quote_context, build_quote_context_with_info, MessageExt};

// Simple (message id + sender):
let ctx = build_quote_context("original-id", "sender@s.whatsapp.net", &original_message);
client.send_message(chat, wa::Message::text_with_context("reply", ctx)).await?;

// Full (WA-Web parity, used by MessageContext):
let ctx = build_quote_context_with_info(
    "original-id", &sender_jid, &quoted_chat_jid, &target_chat_jid, &original_message,
);
```

### 6.3 Reactions

```rust
pub async fn send_reaction(
    &self, chat: impl Into<Jid>, target_key: wa::MessageKey, emoji: &str,
) -> Result<SendResult, SendError>;
```

- `target_key` must carry `participant` for groups and status@broadcast (the original sender);
  `MessageContext::message_key()` builds the correct key (sets `participant` for group/status,
  omits it in DMs).
- Empty `emoji` removes a previous reaction.
- Community Announcement Groups require an encrypted reaction (`enc_reaction_message`); the method
  handles this transparently and fails if the target message's `messageSecret` was never captured.

### 6.4 Edits and deletes

```rust
pub async fn edit_message(&self, to: impl Into<Jid>, original_id: impl Into<String>, new_content: wa::Message)
    -> Result<String, SendError>;                     // returns original_id
pub async fn edit_message_with_options(&self, to, original_id, new_content, options: EditOptions)
    -> Result<String, SendError>;
pub async fn edit_message_encrypted(&self, to, original_id, message_secret: &[u8] /*32 bytes*/, new_content)
    -> Result<String, SendError>;                     // CAG/channel secret-encrypted edit
pub async fn revoke_message(&self, to: impl Into<Jid>, message_id: impl Into<String>, revoke_type: RevokeType)
    -> Result<(), SendError>;
pub async fn keep_message(&self, chat: impl Into<Jid>, key: wa::MessageKey, keep: bool)
    -> Result<SendResult, SendError>;                 // disappearing-chat keep / undo-keep
pub async fn pin_message(&self, chat: impl Into<Jid>, key: wa::MessageKey, duration: PinDuration)
    -> Result<(), SendError>;
pub async fn unpin_message(&self, chat: impl Into<Jid>, key: wa::MessageKey) -> Result<(), SendError>;

pub enum RevokeType {
    Sender,                                  // delete your own message
    Admin { original_sender: Jid },          // group admin deletes another user's message
}
pub enum PinDuration { Hours24, #[default] Days7, Days30 }
pub struct EditOptions { pub stanza_id: Option<String> /* best-effort outer-id override */ }
```

- Edits use a fresh outer stanza id by default (reusing the original id makes the server dedupe and
  silently drop the edit — that behavior is preserved on purpose).
- `edit_message_encrypted` is invalid for newsletters (use `newsletter().edit_message`).
- Admin revoke is group-only; it forces sender-key distribution with `phash`/`<participants>`.

### 6.5 Media: upload, build message, download

Upload returns CDN/crypto fields; the `media` module turns them into protobuf messages:

```rust
use whatsapp_rust::download::MediaType;
use whatsapp_rust::upload::UploadOptions;
use whatsapp_rust::media::{self, ImageOptions, VideoOptions, DocumentOptions, AudioOptions};

let data: Vec<u8> = std::fs::read("photo.jpg")?;
let upload = client.upload(data, MediaType::Image, UploadOptions::default()).await?;
let msg = media::image_message(upload, ImageOptions {
    caption: Some("hi".into()),
    ..Default::default()
});
let sent = client.send_message(chat, msg).await?;
```

```rust
// Exact signatures:
pub async fn upload(&self, data: Vec<u8>, media_type: MediaType, options: UploadOptions)
    -> anyhow::Result<UploadResponse>;
pub async fn upload_stream<S: wacore::upload::UploadSource + 'static>(
    &self, source: S, info: wacore::upload::EncryptedMediaInfo, media_type: MediaType,
) -> anyhow::Result<UploadResponse>;
pub async fn download(&self, downloadable: &dyn Downloadable) -> anyhow::Result<Vec<u8>>;
pub async fn download_from_params(&self, params: &DownloadParams) -> anyhow::Result<Vec<u8>>;
pub async fn download_to_writer<W: DownloadWriter + Send + 'static>(
    &self, downloadable: &dyn Downloadable, writer: W,
) -> anyhow::Result<W>;
```

```rust
#[non_exhaustive]
pub struct UploadResponse {
    pub url: String, pub direct_path: String,
    pub media_key: [u8; 32], pub file_sha256: [u8; 32], pub file_enc_sha256: [u8; 32],
    pub file_length: u64, pub media_key_timestamp: i64,
    pub streaming_sidecar: Option<Vec<u8>>,   // audio/video progressive playback
}
#[non_exhaustive] #[derive(Default)]
pub struct UploadOptions { pub media_key: Option<[u8; 32]>, pub streaming_sidecar: Option<bool> }

pub enum MediaType { Image, Video, Audio, Document, History, AppState, Sticker,
                     StickerPack, StickerPackThumbnail, LinkThumbnail, ProductCatalogImage }

pub struct ImageOptions  { caption, mimetype (default image/jpeg), jpeg_thumbnail, context_info }
pub struct VideoOptions  { caption, mimetype (video/mp4), jpeg_thumbnail, duration_seconds, gif_playback, context_info }
pub struct DocumentOptions { mimetype (application/octet-stream), file_name, title, caption, page_count, jpeg_thumbnail, context_info }
pub struct AudioOptions  { mimetype (audio/ogg; codecs=opus), duration_seconds, ptt, waveform, context_info }
// media::image_message / video_message / document_message / audio_message -> wa::Message
```

Download accepts any `Downloadable`; the protobuf media messages implement it:

```rust
if let Some(img) = msg.image_message.as_option() {            // &wa::message::ImageMessage
    let bytes: Vec<u8> = client.download(img).await?;         // &dyn Downloadable coercion
}
```

`Downloadable` implementors include `ImageMessage`, `VideoMessage`, `DocumentMessage`,
`AudioMessage`, `StickerMessage`, `StickerPackMessage`, `ExternalBlobReference` (app state) and
`HistorySyncNotification`. Newsletter media carries a static URL and no media key
(`is_encrypted() == false`). For re-downloading without a live session there is
`MediaDownloader::with_default_hosts(http_client, runtime)` with the same `download` /
`download_to_writer` methods (`whatsapp_rust::download::MediaDownloader`).

Large-file behavior worth knowing: uploads ≥ 5 MiB first check for an existing/resumable upload and
resume by byte offset; `upload_stream` keeps memory constant (~40 KB for download-to-writer).

### 6.6 Status (stories)

Status is not a normal `send_message` target (it uses LID addressing + sender keys):

```rust
use whatsapp_rust::features::StatusSendOptions; // re-exported at crate root too
client.status().send_text(text, background_argb, font, &recipients, StatusSendOptions::default()).await?;
client.status().send_image(upload, thumbnail_jpeg, caption, &recipients, opts).await?;
client.status().send_video(upload, thumbnail_jpeg, duration_secs, caption, &recipients, opts).await?;
```

`StatusPrivacySetting::{Contacts, AllowList, DenyList}` lives in `StatusSendOptions.privacy`.
Sending status requires the device to have a LID (fails with `SendError::InvalidRequest` otherwise).

### 6.7 Receipts we send (read / played)

```rust
pub async fn mark_as_read(&self, chat: &Jid, sender: Option<&Jid>, message_ids: &[&str])
    -> Result<(), anyhow::Error>;
pub async fn mark_as_played(&self, chat: &Jid, sender: Option<&Jid>, message_ids: &[&str])
    -> Result<(), anyhow::Error>;
```

- Group/status messages: pass the original sender as `sender` (receipt `participant`).
- Respects the account's `readreceipts == none` privacy: DMs then emit `read-self`/`played-self`
  (the sender is not notified). Read receipts are chunked at 256 ids per stanza.
- Delivery receipts are sent automatically by the receive pipeline; `Event::Receipt` and
  `Event::ServerAck` expose the traffic. `set_force_active_delivery_receipts(true)` forces active
  (`inactive` → tick) delivery receipts even while offline.

---

## 7. Groups

Accessor: `client.groups() -> Groups<'_>`. Main methods (exact signatures):

```rust
pub async fn query_info(&self, jid: &Jid) -> Result<Arc<GroupInfo>, GroupError>;
pub async fn query_info_with_freshness(&self, ...) -> Result<Arc<GroupInfo>, GroupError>;
pub async fn get_metadata(&self, jid: &Jid) -> Result<GroupMetadata, GroupError>;
pub async fn get_participating(&self) -> Result<HashMap<Jid, GroupMetadata>, GroupError>;
pub async fn batch_get_info(&self, jids: Vec<Jid>) -> Result<Vec<BatchGroupResult>, GroupError>;
pub async fn create_group(&self, options: GroupCreateOptions) -> Result<CreateGroupResult, GroupError>;
pub async fn set_subject(&self, jid: impl Into<Jid>, subject: GroupSubject) -> Result<(), GroupError>;
pub async fn set_description(&self, jid: impl Into<Jid>, description: Option<GroupDescription>,
                             prev: PreviousDescription<'_>) -> Result<(), GroupError>;
pub async fn leave(&self, jid: impl Into<Jid>) -> Result<(), GroupError>;
pub async fn add_participants(&self, jid: impl Into<Jid>, participants: &[Jid])
    -> Result<Vec<ParticipantChangeResponse>, GroupError>;
pub async fn remove_participants(&self, jid: impl Into<Jid>, participants: &[Jid]) -> …;
pub async fn remove_participants_including_linked_groups(...) -> …;
pub async fn promote_participants(&self, jid: impl Into<Jid>, participants: &[Jid]) -> …;
pub async fn demote_participants(&self, jid: impl Into<Jid>, participants: &[Jid]) -> …;
pub async fn get_invite_link(&self, jid: impl Into<Jid>, reset: bool) -> Result<String, GroupError>;
pub async fn set_locked(&self, jid: impl Into<Jid>, locked: bool) -> Result<(), GroupError>;
pub async fn set_announce(&self, jid: impl Into<Jid>, announce: bool) -> Result<(), GroupError>;
pub async fn set_ephemeral(&self, jid: impl Into<Jid>, expiration: u32) -> Result<(), GroupError>;
pub async fn set_membership_approval(&self, jid, mode: MembershipApprovalMode) -> …;
pub async fn join_with_invite_code(&self, code: &str) -> Result<JoinGroupResult, GroupError>;
pub async fn join_with_invite_v4(&self, ...) -> …;
pub async fn get_invite_info(&self, code: &str) -> Result<GroupMetadata, GroupError>;
pub async fn get_membership_requests(&self, jid: impl Into<Jid>) -> …;
pub async fn approve_membership_requests(...); pub async fn reject_membership_requests(...);
pub async fn cancel_membership_requests(...);  pub async fn revoke_request_code(...);
pub async fn set_member_add_mode(...); pub async fn set_no_frequently_forwarded(...);
pub async fn set_allow_admin_reports(...); pub async fn set_group_history(...);
pub async fn set_member_link_mode(...); pub async fn set_member_share_history_mode(...);
pub async fn set_limit_sharing(&self, jid: &Jid, enabled: bool) -> …;
pub async fn acknowledge(&self, jid: impl Into<Jid>) -> Result<(), GroupError>;
pub async fn get_profile_pictures(&self, group_jids: Vec<Jid>, picture_type: PictureType)
    -> Result<Vec<GroupProfilePicture>, GroupError>;
pub async fn set_profile_picture(&self, group_jid: impl Into<Jid>, image_data: Vec<u8>)
    -> Result<SetProfilePictureResponse, GroupError>;
pub async fn remove_profile_picture(&self, group_jid: impl Into<Jid>)
    -> Result<SetProfilePictureResponse, GroupError>;
pub async fn update_member_label(&self, ...); pub async fn update_member_label_with_id(&self, ...);
```

Creation (`GroupCreateOptions` is `bon`-built; the `new`/`with_*` helpers also exist):

```rust
#[derive(Debug, Clone, bon::Builder)]
pub struct GroupCreateOptions {
    pub subject: String,
    pub participants: Vec<GroupParticipantOptions>,     // default empty
    pub member_link_mode: Option<MemberLinkMode>,        // default Some(AdminLink)
    pub member_add_mode: Option<MemberAddMode>,          // default Some(AllMemberAdd)
    pub membership_approval_mode: Option<MembershipApprovalMode>, // default Some(Off)
    pub ephemeral_expiration: Option<u32>,               // default Some(0)
    pub is_parent: bool,                                 // create as community
    pub closed: bool,
    pub allow_non_admin_sub_group_creation: bool,
    pub create_general_chat: bool,
    pub linked_parent: Option<Jid>,                      // create subgroup linked to a community
    pub description: Option<GroupDescription>,           // inline description on create
}
pub struct GroupParticipantOptions { pub jid: Jid, pub phone_number: Option<Jid>, pub privacy: Option<Vec<u8>> }
impl GroupParticipantOptions {
    pub fn new(jid: Jid) -> Self;  pub fn from_phone(phone_number: Jid) -> Self;
    pub fn with_phone_number(self, phone_number: Jid) -> Self; pub fn with_privacy(self, privacy: Vec<u8>) -> Self;
}
```

Usage:

```rust
let created = client.groups().create_group(
    GroupCreateOptions::new("Design team")
        .with_participants(vec![GroupParticipantOptions::new(Jid::pn("15551230000"))]),
).await?;
let metadata: GroupMetadata = created.metadata; // id, subject, participants, description, settings, …
```

`GroupMetadata` carries `id`, `subject`, `notify`, `participants: Vec<GroupParticipant>`,
`addressing_mode`, `creator(+_pn/_username)`, `creation_time`, version ids, `description(+_id/_owner/_time)`,
`is_locked`, `is_announcement`, `ephemeral: Option<GroupEphemeralSettings>`, `membership_approval`,
`member_add_mode`, `member_link_mode`, `size`, community flags (`is_parent_group`, `parent_group_jid`,
`is_default_sub_group`, `is_general_chat`, `allow_non_admin_sub_group_creation`), `no_frequently_forwarded`,
`member_share_history_mode`, `growth_locked`, `is_suspended`, `appeal_status`, `is_support_group`,
`allow_admin_reports`, etc. Participant flags are on `GroupParticipant`
(`is_admin()`, `is_super_admin()`).

Metadata mutation notifications arrive as `Event::GroupUpdate` (one event per action; `action:
GroupNotificationAction`, plus `action_index` for multi-action stanzas).

Group description updates need the previous description token
(`PreviousDescription::{Absent, Id(&str), Resolve}`); `Resolve` costs one extra query but is always
current.

---

## 8. Newsletters / channels

Accessor: `client.newsletter() -> Newsletter<'_>`. Channels are **not E2E-encrypted**; messages are
sent as plaintext SMAX stanzas. `Client::send_message` handles newsletter JIDs automatically with
`stanza_type_from_message` and a plaintext body.

```rust
pub async fn list_subscribed(&self) -> Result<Vec<NewsletterMetadata>, NewsletterError>;
pub async fn get_metadata(&self, jid: &Jid) -> Result<NewsletterMetadata, NewsletterError>;
pub async fn create(&self, ...) -> Result<NewsletterMetadata, NewsletterError>;
pub async fn join(&self, jid: &Jid) -> Result<NewsletterMetadata, NewsletterError>;
pub async fn leave(&self, jid: &Jid) -> Result<(), NewsletterError>;
pub async fn update(&self, ...) -> Result<NewsletterMetadata, NewsletterError>;
pub async fn set_follower_mute(&self, jid: &Jid, muted: bool) -> Result<(), NewsletterError>;
pub async fn set_admin_mute(&self, jid: &Jid, muted: bool) -> Result<(), NewsletterError>;
pub async fn get_metadata_by_invite(&self, ...) -> Result<NewsletterMetadata, NewsletterError>;
pub async fn subscribe_live_updates(&self, jid: impl Into<Jid>) -> Result<u64, NewsletterError>;
pub async fn send_reaction(&self, jid: &Jid, server_id: u64, reaction: &str) -> Result<(), NewsletterError>;
pub async fn edit_message(&self, jid: &Jid, message_id: impl Into<String>, new_content: wa::Message)
    -> Result<(), NewsletterError>;
pub async fn revoke_message(&self, jid: &Jid, message_id: impl Into<String>) -> Result<(), NewsletterError>;
pub async fn get_messages(&self, jid: impl Into<Jid>, count: u32, before: Option<u64>)
    -> Result<Vec<NewsletterMessage>, NewsletterError>;
```

- Reactions key on `server_id` (u64); edits/revokes key on `message_id` (string). Do not mix them up.
- `subscribe_live_updates` makes the server push reaction-count updates:
  `Event::NewsletterLiveUpdate { newsletter_jid, messages }`.
- `NewsletterMessage { message_id, server_id, timestamp, message_type, is_sender, message:
  Option<wa::Message>, reactions }`; `NewsletterMetadata { jid, name, description,
  subscriber_count, verification, state, picture_url, preview_url, invite_code, role,
  creation_time }`.

---

## 9. Presence, chat state, and history sync

### 9.1 Presence

```rust
pub enum PresenceStatus { Available, Unavailable }
impl Client { pub fn presence(&self) -> Presence<'_>; }
impl Presence<'_> {
    pub async fn set(&self, status: PresenceStatus) -> Result<(), PresenceError>;
    pub async fn set_available(&self) -> Result<(), PresenceError>;
    pub async fn set_unavailable(&self) -> Result<(), PresenceError>;
    pub async fn subscribe(&self, jid: impl Into<Jid>) -> Result<(), PresenceError>;
    pub async fn unsubscribe(&self, jid: &Jid) -> Result<(), PresenceError>;
}
```

- `set` requires a non-empty push name on the device (else `PresenceError::PushNameEmpty`).
- Incoming: `Event::Presence { from, unavailable, last_seen }` for subscribed contacts;
  subscriptions are re-established automatically after reconnect.
- Set our push name before/around connecting (`with_push_name` on the builder, `profile().set_push_name`
  at runtime).

### 9.2 Chat state (typing/recording)

```rust
pub enum ChatStateType { Composing, Recording, Paused }
impl Client { pub fn chatstate(&self) -> Chatstate<'_>; }
impl Chatstate<'_> {
    pub async fn send(&self, to: &Jid, state: ChatStateType) -> Result<(), ChatStateError>;
    pub async fn send_composing(&self, to: &Jid) -> Result<(), ChatStateError>;
    pub async fn send_recording(&self, to: &Jid) -> Result<(), ChatStateError>;
    pub async fn send_paused(&self, to: &Jid) -> Result<(), ChatStateError>;
}
```

Incoming: `Event::ChatPresence { source, state: ChatPresence::{Composing,Paused}, media:
ChatPresenceMedia::{Text,Audio} }`. A legacy callback API also exists:
`client.register_chatstate_handler(Arc<dyn Fn(ChatStateEvent) + Send + Sync>).await`.

### 9.3 History sync

- By default the client processes history sync notifications. For a client that wants the full
  history, register `on_event_for(&[EventKind::HistorySync])` and decode the payload.
- `BotBuilder::skip_history_sync()` (or `client.set_skip_history_sync(true)`) acks notifications
  without downloading/processing them. Good for bots, wrong for a chat app that needs history.
- Payload: `Event::HistorySync(Box<LazyHistorySync>)`.

```rust
pub struct LazyHistorySync { /* compressed Bytes + metadata */ }
impl LazyHistorySync {
    pub fn sync_type(&self) -> i32;
    pub fn chunk_order(&self) -> Option<u32>;
    pub fn progress(&self) -> Option<u32>;
    pub fn peer_data_request_session_id(&self) -> Option<&str>;
    pub fn compressed_bytes(&self) -> &bytes::Bytes;
    pub fn decompressed_size(&self) -> usize;
    pub fn decompress(&self) -> std::io::Result<bytes::Bytes>;       // inflates every call
    pub fn stream(&self) -> wacore::history_sync::HistorySyncStream<'_>;
    pub fn get(&self) -> Option<&wa::HistorySync>;                   // cached decode
}
```

- Multi-MB chunks take tens of ms to inflate: do `get()`/`decompress()` inside
  `tokio::task::spawn_blocking`, cloning `compressed_bytes()` into the closure.
- Streaming decode: `HistorySyncStream::next_conversation()`, `next_conversation_bytes()`,
  `remainder()`, `skipped_conversations()`.
- `Event::OfflineSyncPreview` / `OfflineSyncCompleted` bracket the offline drain; `Event::DirtyState`
  reports server "dirty" domains (client also self-heals internally).
- On a non-network download failure you can ask the phone to resend:
  `client.send_history_sync_server_error_receipt(message_id, media_key).await?`.

---

## 10. Storage, persistence, and configuration

### 10.1 SQLite adapter (`SqliteStore`)

```rust
#[cfg(feature = "sqlite-storage")]
pub use whatsapp_rust_sqlite_storage::{ConnectionInitHook, SqliteStore, SqliteStoreConfig, Synchronous};

impl SqliteStore {
    pub async fn new(database_url: &str) -> Result<Self, StoreError>;
    pub async fn with_config(database_url: &str, config: SqliteStoreConfig) -> Result<Self, StoreError>;
    pub async fn new_for_device(database_url: &str, device_id: i32) -> Result<Self, StoreError>;
    pub async fn with_config_for_device(database_url: &str, device_id: i32, config: SqliteStoreConfig)
        -> Result<Self, StoreError>;
}
```

`database_url` is a Diesel SQLite URL: a plain path (`whatsapp.db`), absolute path, or
`file:` URI. Tests use `file:memdb_x?mode=memory&cache=shared` for in-memory DBs.

`SqliteStoreConfig` (defaults shown):

| Field | Default | Meaning |
|---|---|---|
| `pool_size` | `1` | Write concurrency. **Keep at 1** — concurrent write transactions can deadlock SQLite on read→write upgrades. |
| `read_pool_size` | `0` | Extra read-only connections that can run while a write holds the lock (safe in WAL). |
| `cache_size_kib` | `512` | `PRAGMA cache_size` per connection. |
| `mmap_size` | `None` | `PRAGMA mmap_size`; off by default. |
| `busy_timeout` | `30s` | `PRAGMA busy_timeout`. |
| `synchronous` | `Synchronous::Normal` | `PRAGMA synchronous` (`Off`/`Normal`/`Full`). |
| `thread_pool` | `None` | Shared r2d2 scheduled thread pool. |
| `connection_init` | `None` | Hook run first on each new pooled connection. |

The bundled SQLite amalgamation is compiled from source (`libsqlite3-sys` `bundled` feature is on
by default in `whatsapp-rust-sqlite-storage`), so there is no system-SQLite dependency on macOS.
(Post-release HEAD adds a `sqlite-storage-bundled` opt-out feature — **not in 0.7.0**.)

### 10.2 Backend trait

`SqliteStore` implements `whatsapp_rust::store::Backend`, a composite:

```rust
pub trait Backend: SignalStore + AppSyncStore + ProtocolStore + MsgSecretStore + DeviceStore + Send + Sync {}
```

Any backend implementing those five domain traits automatically implements `Backend`. The trait
surface is large (≈100 async methods, one per persisted domain: device, sessions, prekeys, sender
keys, app-state keys/mutations, message secrets, pending-inbound durability rows). Implementing a
custom backend for the macOS app is possible but not advisable; wrap `SqliteStore` if extra
persistence is needed.

### 10.3 Device state and commands

```rust
pub struct Device {          // wacore::store::Device, re-exported as whatsapp_rust::store::Device
    pub pn: Option<Jid>, pub lid: Option<Jid>,
    pub registration_id: u32,
    pub noise_key: KeyPair, pub identity_key: KeyPair,
    pub signed_pre_key: KeyPair, pub signed_pre_key_id: u32, pub signed_pre_key_signature: [u8; 64],
    pub adv_secret_key: [u8; 32], pub account: Option<Arc<wa::ADVSignedDeviceIdentity>>,
    pub push_name: String,
    pub app_version_primary: u32, pub app_version_secondary: u32, pub app_version_tertiary: u32,
    pub app_version_last_fetched_ms: i64,
    pub device_props: Arc<wa::DeviceProps>,
    pub client_profile: ClientProfile,       // runtime-only
    pub edge_routing_info: Option<Vec<u8>>, pub props_hash: Option<String>,
    pub next_pre_key_id: u32, pub first_unupload_pre_key_id: u32,
    pub server_has_prekeys: bool, pub nct_salt: Option<Vec<u8>>,
    pub server_cert_chain: Option<CachedServerCertChain>,
    pub login_counter: i32, pub lid_migrated: bool,
    pub last_signed_pre_key_rotation_ms: i64, pub read_receipts_disabled: bool,
    // …
}
```

**Never mutate `Device` directly** (upstream AGENTS.md gotcha: a write-lock mutation bypasses the
cached snapshot). Read through `PersistenceManager::get_device_snapshot()` (cached `Arc<Device>`,
cheap per message), mutate through `DeviceCommand` + `PersistenceManager::process_command()`.

```rust
impl PersistenceManager {
    pub async fn new(backend: Arc<dyn Backend>) -> Result<Self, StoreError>;
    pub fn get_device_snapshot(&self) -> Arc<Device>;
    pub async fn process_command(&self, command: DeviceCommand);
    pub async fn modify_device<F, R>(&self, modifier: F) -> R where F: FnOnce(&mut Device) -> R;
    pub async fn flush(&self) -> Result<(), StoreError>;
    // …
}
```

`DeviceCommand` variants (v0.7.0): `SetId`, `SetLid`, `SetPushName`, `SetAccount`, `SetAppVersion`,
`SetDeviceProps`, `SetClientProfile`, `SetPropsHash`, `SetPreKeyWatermarks`,
`SetAdvSecretKey`, `SetNctSalt`, `SetNctSaltFromHistorySync`, `SetServerCertChain`,
`ClearServerCertChain`, `IncrementLoginCounter`, `SetLidMigrated`, `SetSignedPreKey`,
`SetSignedPreKeyRotationBaseline`, `SetReadReceiptsDisabled`.

Reachable from the app as `client.persistence_manager()`.

### 10.4 At-least-once delivery (inbound durability hook)

Default behavior is **at-most-once**: messages are acked after decryption, before the app persists
them. Registering a hook defers the ack until the hook commits:

```rust
#[async_trait::async_trait]
pub trait InboundDurabilityHook: MaybeSendSync {
    /// Durably commit the whole batch, all-or-nothing, in slice order.
    /// Return Ok only after the commit is durable; the SDK then acks every message.
    async fn on_messages(&self, client: Arc<Client>, batch: &[InboundMessage]) -> anyhow::Result<()>;
}
// builder: .with_inbound_durability_hook(hook)
```

Contract highlights (from upstream docs):

- The hook **must be idempotent** (at-least-once): dedupe by `(chat, sender, message id)` — stanza
  ids are only unique within `(chat, sender)`. A failed batch is redelivered whole.
- Durable replay across process crashes requires a backend implementing the `ProtocolStore`
  pending-inbound methods (`SqliteStore` does).
- The hook runs on the receive path; a slow hook backpressures inbound processing. Never do a
  synchronous send to a sender inside the batch (can deadlock the per-sender Signal lock).
- Scope gaps: newsletter/broadcast messages and PDO placeholder recoveries are **not** gated by the
  hook. If the durable buffer write itself fails and the process keeps running, the Signal ratchet
  has already advanced and those messages degrade to at-most-once on redelivery.
- On a redelivery replay, a few derived `info` fields (ephemeral timer, comment threading) may be
  absent; the message body is the original.

`examples/durability_hook.rs` shows a tab-separated append + fsync implementation (the dedupe set
is loaded from the archive at startup, keys are only recorded after the write succeeds).

### 10.5 Builder configuration summary (`BotBuilder`)

| Method | Signature (abridged) | Purpose |
|---|---|---|
| `with_backend` | `(impl Backend + 'static)` | Required storage backend. |
| `with_backend_arc` | `(Arc<dyn Backend>)` | Multi-account variant (see `SqliteStore::new_for_device`). |
| `with_transport_factory` | `(impl TransportFactory + 'static)` | Replace Tokio WebSocket transport. |
| `with_http_client` | `(impl HttpClient + 'static)` | Replace ureq HTTP client. |
| `with_runtime` | `(impl Runtime)` | Replace Tokio runtime, e.g. `TokioRuntime`. |
| `with_task_instrument` | `(Arc<dyn TaskInstrument>)` | Per-poll instrumentation/CPU accounting. Mutually exclusive with alloc meter (last wins). |
| `with_alloc_meter` | `(Arc<wacore::stats::AllocMeter>)` | Allocation-churn snapshot in `resource_report()`. |
| `with_version` | `((u32, u32, u32))` | Pin the advertised WA Web version; skips version fetch. |
| `with_device_props` | `(DevicePropsOverride)` | Linked-devices name/platform. Applied on initial pairing only. |
| `with_pair_code` | `(PairCodeOptions)` | Concurrent pair-code flow. |
| `skip_history_sync` | `()` | Ack but don't process history sync. |
| `with_wanted_pre_key_count` | `(usize)` | Pre-key upload batch size (default 812). |
| `with_resend_rate_limit` | `(burst: u32, refill_per_min: u32)` | Per-chat outbound retry-resend token bucket (default 20 / 10). |
| `with_push_name` | `(impl Into<String>)` | Initial push name before connecting. |
| `with_cache_config` | `(CacheConfig)` | TTL/capacity/custom stores for all caches. |
| `with_enc_handler` | `(enc_type, Eh: EncHandler)` | Custom encrypted message types. |
| `with_inbound_durability_hook` | `(Dh: InboundDurabilityHook)` | At-least-once inbound. |
| `with_event_handler` | `(impl EventHandler + 'static)` | Inline struct handler. |
| `with_event_delivery` | `(EventDelivery)` | Concurrent (default) vs Ordered{capacity}. |
| `with_plugin*` | plugin feature only | Native plugins. |

Cache defaults (from `CacheConfig`): group cache 1 h TTL / 250 entries; device registry 1 h / 5000;
LID-PN cache no TTL / effectively unbounded; recent-messages L1 disabled (capacity 0, DB-only);
retry counts 1 h / 500; undecryptable dedupe 5 min / 1000; PDO pending 30 s / 200; sent-message DB
TTL 7200 s. `OriginalMessageResolver` + `MsgSecretPolicy` let an app own message-secret retention.

Custom cache stores (e.g. external KV) can be injected via `CacheStores { group_cache,
device_registry_cache, lid_pn_cache }`; coordination caches and the Signal write-behind cache always
stay in-process.

---

## 11. Contacts and profile (supporting APIs)

```rust
// Contacts
client.contacts().is_on_whatsapp(&[Jid]) -> Result<Vec<IsOnWhatsAppResult>, ContactError>;
client.contacts().get_profile_picture(&Jid, preview: bool) -> Result<Option<ProfilePicture>, ContactError>;
client.contacts().get_profile_picture_with_timeout(&Jid, preview, Option<Duration>) -> …;
client.contacts().get_user_info(&[Jid]) -> Result<HashMap<Jid, UserInfo>, ContactError>;
// Profile (own account)
client.profile().set_push_name(&str) -> Result<(), ProfileError>;      // presence + app-state mutation
client.profile().set_status_text(&str) -> Result<(), ProfileError>;    // "About"
client.profile().set_profile_picture(Vec<u8> /*jpeg*/) -> Result<SetProfilePictureResponse, ProfileError>;
client.profile().remove_profile_picture() -> …;
// Chat actions (archive/pin/mute/star/read/deletes/…)
client.chat_actions().mark_chat_as_read(&Jid, read: bool, Option<SyncActionMessageRange>) -> …;
client.chat_actions().delete_chat(...); client.chat_actions().clear_chat(...);
client.chat_actions().star_message(...); client.chat_actions().unstar_message(...);
client.chat_actions().set_user_status_mute(&Jid, muted) -> …;
// Other feature handles
client.blocking(); client.labels(); client.community(); client.polls(); client.events(); // calendar events
client.comments(); client.status(); client.signal(); client.mex(); client.voip(); // voip feature
```

`mark_chat_as_read` is the cross-device app-state sync; `mark_as_read` (§6.7) is the wire read
receipt. A chat app usually wants both.

For the app's own push name at startup, read `client.push_name()`; own identity is `client.pn()` /
`client.lid()`.

---

## 12. Known limitations and repo TODOs (0.7.0)

Upstream has no "limitations" section; the following is collected from source docs, verified in
this study:

1. **Nightly vs stable**: upstream default features (`simd`) do not compile on stable. A stable
   build must drop `simd`; upstream pins `nightly-2026-06-16` for its own CI/dev.
2. **Durability hook scope**: newsletter/broadcast messages and PDO placeholder recoveries bypass
   the hook; if the durable buffer write fails without a crash, those messages degrade to
   at-most-once; redelivered `info` can lack a few derived fields.
3. **Ordered event delivery drops on overflow** (counted in `events_dropped`); it bounds memory but
   is not lossless — use the durability hook for lossless inbound.
4. **Pair-code flow**: one outstanding code at a time; `bad-request` doubles as rate limiting;
   `Event::PairingCodeError` is intentionally silent for `CodeAlreadyOutstanding`/`Cancelled`.
5. **QR rotation**: six refs per connection (60 s + 5×20 s); after exhaustion, a QR-only flow
   self-disconnects, while an outstanding pair-code keeps the socket.
6. **Status posts require a LID** on the device; fail fast otherwise.
7. **Community Announcement Group reactions/edits** need the target message's `messageSecret`
   captured at receive time (encrypted add-on path).
8. **Newsletter/channel messages are plaintext** (no E2E); reaction/edit/revoke key differently
   (`server_id` vs `message_id`).
9. **`Client::logout()` does not wipe stored keys** — "Delete the storage backend to fully clear
   credentials" (used to avoid re-linking a deleted device).
10. **VoIP is opt-in** (`voip` feature family); the call events exist regardless, but media
    (WebRTC/DTLS/SCTP + codec) needs the feature and native codec deps.
11. **Post-release HEAD adds** `sqlite-storage-bundled` (opt out of the bundled SQLite
    amalgamation, commit 6502b87, 2026-09-11). Not in crates.io 0.7.0. Other post-0.7.0 work
    includes VoIP video resume and STAP-A aggregation — do not rely on it until a release.
12. **In-code TODOs** at v0.7.0 are minimal: poll `result_snapshot` meta is gated behind a WA
    A/B flag; one receipt path logs "VoIP not implemented" for group-call creator receipts.
13. **ToS risk**: unofficial client (upstream disclaimer, quoted at the top of this document).

---

## 13. Gotchas checklist for the macOS client

1. **Toolchain**: use stable + no `simd`, or pin `nightly-2026-06-16`. Don't blindly copy the
   README's `whatsapp-rust = "0.7"` into a stable project — it fails with E0554.
2. **`BotBuilder` is `#[must_use]` and typestate**: chain then `.build().await`; missing
   backend/transport/http/runtime are compile errors (or require the `with_*` calls when features
   are off). `build()` initializes the device row; don't hand-roll the store.
3. **Keep `BotHandle` alive**: dropping it aborts the bot. Await it or keep it on the app's state
   and call `shutdown().await` on quit. `shutdown` flushes pending state; `abort` skips flushes.
4. **Ordering**: the default concurrent delivery does not preserve event order across events and
   can pile up tasks for slow handlers; use `EventDelivery::Ordered { capacity }` for a UI queue and
   size the capacity for the offline drain (or accept counted drops).
5. **Never match `Event`/payloads exhaustively**: they are `#[non_exhaustive]`. Construct payloads
   via their `bon` builders, not literals. `Debug` for `Event` prints only the variant name;
   `Serialize` for contents.
6. **`on_event` catch-all widens interest to `ALL`**, forcing materialization of every event
   (history sync blobs included). Prefer `on_event_for` with an explicit `EventKind` list.
7. **Raw nodes**: `Event::RawNode` requires an active lease from
   `client.acquire_raw_node_forwarding()` (returns `RawNodeLease`; forwarding stays on while any
   lease lives).
8. **At-most-once default**: if the app must not lose inbound messages on crash, register an
   `InboundDurabilityHook`; keep it fast, idempotent, and never send synchronously to a sender in
   the batch from inside the hook.
9. **`MessageContext` borrow rules**: it is `Clone` (Arc-based message); cheap to move into spawned
   tasks. `info` is a value clone, `message` is shared.
10. **Replies**: `ctx.reply` is a plain text reply; `ctx.reply_quoting` quotes. Cross-chat quoting
    needs `build_quote_context_with_info` (remote_jid is emitted only when chats differ).
11. **`mark_as_read` respects privacy** (`readreceipts == none` → `read-self`), and group/status
    receipts need the original sender in `sender`.
12. **`wa::Message` fields are `buffa::MessageField<T>`**, not `Option<T>`: construct with
    `MessageField::some(..)`; read with `.as_option()` / `.is_set()`.
13. **Text extraction**: `text_content()` only covers `conversation` and `extendedTextMessage`;
    captions need `get_caption()`; wrapper messages (ephemeral, view-once, document-with-caption,
    edited) need `get_base_message()` first.
14. **Media**: upload once, build the proto via `media::*_message`, then send; forwarding/editing
    reuse the CDN fields. Downloads take `&dyn Downloadable` (the proto media message itself).
    Don't hold media bytes in the event payloads; they're references.
15. **Storage**: keep `pool_size` at 1; raise `read_pool_size` for read concurrency. Pick a stable
    absolute path inside the sandbox container (Application Support), not the process CWD. The DB
    holds Signal state and device keys — treat it as secret, back it up with the app's container.
16. **Multi-account**: create the store with `SqliteStore::new_for_device(db, device_id)` and pass
    via `with_backend_arc`; the device id must be unique per session.
17. **Devices are linked, not cloned**: a device row is account state. Logging out does not delete
    keys; to fully unlink/clear, delete the backend file (and tell the user the phone's Linked
    Devices list is authoritative).
18. **Network**: the client needs outbound WSS to WhatsApp + HTTPS to web.whatsapp.com (version) +
    CDN HTTP(S). Under App Sandbox add `com.apple.security.network.client`; version fetch happens
    before the handshake, so pin `with_version` if the app must connect before any other HTTP is
    allowed. Never enable `danger-skip-tls-verify`/`danger-skip-cert-chain-verify`.
19. **TLS**: rustls + webpki roots (no system keychain integration). Corporate MITM proxies will not
    be trusted.
20. **Version drift**: pin exactly `=0.7.0` (or `~0.7.0`) plus the lockfile; this API is young and
    0.8 may change signatures. The crate re-exports its third-party types so version matching is
    not a concern within the tree.
21. **`Jid`s**: `"1555...@s.whatsapp.net"` (PN), `"...@lid"` (LID), `"...@g.us"` (group),
    `"...@newsletter"` (channel), `status@broadcast`. Modern WhatsApp is LID-addressed; the client
    resolves PN↔LID internally (cache + server), but UI cache keys should handle both.
22. **Reconnect behavior**: `Client::run` (and thus `Bot::run`/`spawn`) auto-reconnects with
    Fibonacci backoff (10 % jitter, 900 s cap) unless `client.enable_auto_reconnect` (public
    `Arc<AtomicBool>`) is false or `disconnect()` is called. `reconnect_immediately()` skips the
    backoff for an expected disconnect.
23. **Logging**: the crate uses the `log` facade; install `env_logger`/`tracing` ourselves. The
    `tracing` feature only emits spans; `tracing-pii` (raw phone numbers) must never ship.
24. **Timestamps**: WA event timestamps are `chrono::DateTime<Utc>` (seconds). Use
    `whatsapp_rust::chrono` to avoid a second chrono version in the tree.
25. **App lifecycle**: on macOS sleep/wake the socket dies; the run loop reconnects. Surface
    `Event::Connected`/`Disconnected`/`ConnectFailure`/`TemporaryBan`/`StreamReplaced` in the UI so
    users can tell "offline" from "logged out" from "banned".

---

## 14. Suggested integration layout for this project

```text
src/
├── main.rs / app.rs            — UI shell, spawns the protocol task
├── protocol/
│   ├── mod.rs                  — WhatsAppSession: owns BotHandle, exposes AsyncStream<UiEvent>
│   ├── bot.rs                  — Bot::builder(...).on_qr_code(..).on_message(..).build()
│   ├── events.rs               — event -> UiEvent projection (ChannelEventHandler or on_event_for)
│   ├── send.rs                 — typed send commands (text, media, reply, react, edit, delete)
│   ├── media.rs                — upload/download, thumbnail generation, file caching
│   └── store.rs                — SqliteStore path/config + durability hook wrapper
└── …
```

- Bridge the bus into the UI with `ChannelEventHandler` + `subscribe_handler`, or use
  `BotBuilder::on_event_for` and forward through an `async_channel`.
- Keep `Arc<Client>` (from `bot.client()`) as the command surface; the UI sends commands through a
  channel and awaits `SendResult`s.
- Persist incoming messages in our own store inside the durability hook (idempotent
  `(chat, sender, id)` dedupe).

---

## 15. Verification

### Verified by compiling on this machine (macOS 26.2, arm64, 2026-09-13)

Toolchains: `stable-aarch64-apple-darwin` (rustc 1.98.1) and freshly installed
`nightly-2026-06-16-aarch64-apple-darwin` (rustc 1.98.0-nightly). Scratch projects were created
**outside** the project directory (`$TMPDIR/whatsapp-rust-build-test`,
`$TMPDIR/whatsapp-rust-build-nightly`), depending on the crates.io release
`whatsapp-rust = "0.7"` (resolved to 0.7.0, checksum
`3cb354c32641cfe832bf5c5767a016762234c5efd1b4ce979e2b3e32175d96bc`).

| Build | Toolchain | Features | Result | Wall time |
|---|---|---|---|---|
| Upstream defaults | stable 1.98.1 | `default` (includes `simd`) | **FAILED**: E0554 `#![feature]` may not be used on stable, `portable_simd` in `wacore-binary` | failure at ~20 s |
| Recommended | stable 1.98.1 | `default-features = false` + `sqlite-storage,tokio-transport,ureq-client,tokio-runtime,tokio-native,signal` | **PASSED** | 50.7 s (warm deps) / **1 m 56 s clean** |
| Upstream defaults | nightly-2026-06-16 | `default` (includes `simd`) | **PASSED** | 1 m 05 s |
| API compile-check binary | stable 1.98.1 | recommended set | **PASSED** (`cargo build`, 4.8 s incremental after dependency build) | — |

The API compile-check is preserved at `docs/research/whatsapp-rust-api-compile-check.rs` (with its
`Cargo.toml` at `whatsapp-rust-api-compile-check.Cargo.toml`). It exercises, and therefore
type-verifies: `Bot::builder` + all closure registrars, `with_pair_code`, `with_version`,
`with_event_delivery`, `skip_history_sync`, `with_push_name`, `Bot::spawn`/`BotHandle::shutdown`,
`MessageContext::{reply, reply_quoting, react, edit_message, revoke_message, send_message,
message_key}`, `Client::{connect, wait_for_socket, wait_for_connected, is_connected, is_logged_in,
push_name, pn, lid, stats, memory_report, resource_report, send_text, send_message,
send_message_with_options, forward_message, send_reaction, edit_message, revoke_message,
keep_message, pin_message, unpin_message, upload, download, mark_as_read, mark_as_played, logout,
disconnect}`, `build_quote_context_with_info`, `media::{image_message, ImageOptions}`,
`Downloadable` coercion from `ImageMessage`, `groups()` (metadata/create/add/promote/invite),
`newsletter()` (metadata/list/mute/messages), `presence()`, `chatstate()`, `profile()`,
`contacts()`, `ChannelEventHandler` + `subscribe_handler` + `EventInterest`/`EventKind`, and an
`InboundDurabilityHook` implementation.

Also verified:

- The crates.io sources of `whatsapp-rust`, `wacore`, `wacore-binary`, `wacore-appstate`,
  `waproto`, `whatsapp-rust-sqlite-storage`, `whatsapp-rust-tokio-transport`, and
  `whatsapp-rust-ureq-http-client` are **byte-identical** (`diff -rq src/`) to the upstream v0.7.0
  tag checkout.
- `rustup show` on this machine confirms the project's `rust-toolchain.toml` pins `stable`.
- No macOS/arm64-specific compile issue was encountered beyond the nightly-only `simd` feature;
  `libsqlite3-sys` compiled its bundled amalgamation successfully.

### Verified by reading upstream source (tag v0.7.0, commit f8165f28)

- `README.md` (all quoted snippets), `AGENTS.md`, `Cargo.toml` features, `rust-toolchain.toml`.
- `examples/demo.rs`, `examples/benchmark.rs`, `examples/durability_hook.rs` (quoted handlers and
  builder usage).
- `src/lib.rs` (full module/re-export map and `prelude`), `src/bot.rs` (full), `src/client.rs`,
  `src/client/{builder,lifecycle,accessors,messaging}.rs`, `src/pair.rs`, `src/pair_code.rs`,
  `src/send/mod.rs` + `src/send/actions.rs`, `src/features/{reaction,groups,newsletter,presence,
  chatstate,contacts,profile,status,chat_actions}.rs`, `src/receipt.rs`, `src/download.rs`,
  `src/upload.rs`, `src/media.rs`, `src/history_sync.rs`, `src/store/{mod,traits,commands,
  persistence_manager}.rs`, `storages/sqlite-storage/src/{lib,sqlite_store}.rs`,
  `wacore/src/types/events.rs` (complete Event/EventKind/payload definitions),
  `wacore/src/types/{message,presence,call}.rs`, `wacore/src/store/device.rs`,
  `wacore/src/proto_helpers.rs`, `wacore/src/download.rs`, `wacore/src/pair_code.rs`.
- Every method signature and struct field quoted in this guide was copied from those files at the
  v0.7.0 tag, not from memory or documentation prose.

### Inferred, not verified

- **Runtime behavior was not executed**: no live pairing, QR scan, send, receive, or media round
  trip was performed (no WhatsApp account is linked from this machine). All behavioral claims
  (rotation timings, receipt chunks, retry backoff, drop semantics) come from reading the upstream
  source and its comments, not from observation.
- macOS **App Sandbox entitlements / notarization** requirements for networking and the SQLite
  path are standard practice, not tested here.
- Release-profile build times, memory footprint, and performance are not measured (debug builds
  only, as requested).
- The guide targets the crates.io 0.7.0 = git tag `v0.7.0`. The upstream default branch (HEAD
  `6502b871`, 2026-09-11) contains unreleased changes (e.g. `sqlite-storage-bundled`) that are
  explicitly **not** part of 0.7.0.
