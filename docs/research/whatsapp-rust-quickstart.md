# whatsapp-rust 0.7.0 — Minimal Integration Quickstart

Narrow, compile-verified path for exactly this use case: **create a client → register handlers → connect → QR pairing → SQLite session persistence → send text → receive messages**.

- Upstream: <https://github.com/jlucaso1/whatsapp-rust>, tag/commit `0.7.0` (`f8165f2`, "chore(release): prepare 0.7.0")
- crates.io: `whatsapp-rust = "0.7.0"` (MIT, edition 2024, `rust-version = "1.94"`)
- Verified on macOS arm64 (Darwin), **stable** Rust/Cargo 1.98.1. No account was paired; the run below stops at the QR stage.
- All `file:line` references are relative to the upstream repo root at that commit.

The recommended entry point in 0.7 is the high-level `Bot` facade (`src/bot.rs`), which is also what upstream's README quickstart and `examples/demo.rs` use. It owns storage, transport, HTTP client, runtime, event wiring, and the connect loop; the underlying `Arc<Client>` is reachable for direct calls such as `send_text`.

---

## 1. Cargo.toml

```toml
[package]
name = "wa-quickstart"
version = "0.1.0"
edition = "2024"
rust-version = "1.94"

[dependencies]
# Exact pin; `whatsapp-rust = "0.7"` (as in the upstream README) is the
# semver-equivalent caret form.
whatsapp-rust = { version = "=0.7.0", default-features = false, features = [
    "sqlite-storage",  # persistent SQLite session/backend (SqliteStore)
    "tokio-transport", # bundled Tokio WebSocket transport factory
    "tokio-runtime",   # bundled Tokio Runtime adapter
    "tokio-native",    # Tokio multi-thread runtime (tokio/rt-multi-thread)
    "ureq-client",     # bundled HTTP client (version fetch, media)
] }
tokio = { version = "1", features = ["macros", "rt-multi-thread", "signal", "time"] }
log = "0.4"
env_logger = "0.11"
```

### Why `default-features = false`

The upstream default feature set is (`Cargo.toml:163-171`):

```toml
default = [
    "simd",
    "sqlite-storage",
    "tokio-transport",
    "tokio-runtime",
    "ureq-client",
    "tokio-native",
    "signal",
]
```

`simd` (`Cargo.toml:172`) pulls `wacore/simd`, which compiles `wacore-binary` with `#![cfg_attr(feature = "simd", feature(portable_simd))]` (`wacore/binary/src/lib.rs:1`) — that is a **nightly-only** feature and fails on stable with:

```
error[E0554]: `#![feature]` may not be used on the stable release channel
 --> .../wacore-binary-0.7.0/src/lib.rs:1:31
```

Upstream builds on `nightly-2026-06-16` (`rust-toolchain.toml`). On stable, keep `default-features = false` and re-enable the list above minus `simd` (the `signal` helper is optional; add `"signal"` only if you want `whatsapp_rust::shutdown_signal()` — the example uses `tokio::signal` directly). VoIP features are not in the default set and must stay off for this use case.

### Feature flags that matter here

| Feature | Pulls in | Effect |
|---|---|---|
| `sqlite-storage` | `whatsapp-rust-sqlite-storage` | re-exports `SqliteStore`, `SqliteStoreConfig` (`src/store/mod.rs:11-14`) |
| `tokio-transport` | `whatsapp-rust-tokio-transport` | pre-fills the builder's transport slot (`src/bot.rs:42-45,57-62`) |
| `tokio-runtime` | `tokio` | pre-fills the runtime slot (`src/bot.rs:52-55,77-80`) |
| `tokio-native` | `tokio/rt-multi-thread` | multi-thread runtime backing |
| `ureq-client` | `whatsapp-rust-ureq-http-client` | pre-fills the HTTP slot (version fetch/media) (`src/bot.rs:47-50,68-71`) |

---

## 2. Verified main.rs

Full contents of the compiled-and-run file (scratch crate `wa-quickstart`):

```rust
//! Minimal whatsapp-rust 0.7 quickstart: SQLite-backed session, QR pairing,
//! incoming-message logging, and an optional one-shot text send.
//!
//! Run:
//!   cargo run -- --db ./whatsapp.db --seconds 30
//!   cargo run -- --db ./whatsapp.db --to 15551234567@s.whatsapp.net --text "hello"

use std::time::Duration;

use log::{error, info};
use whatsapp_rust::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // ── arguments ────────────────────────────────────────────────────────
    let args: Vec<String> = std::env::args().collect();
    let db_path = arg_value(&args, "--db").unwrap_or_else(|| "whatsapp.db".to_string());
    let run_secs: u64 = arg_value(&args, "--seconds")
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);
    let send_to = arg_value(&args, "--to");
    let send_text = arg_value(&args, "--text");

    // ── storage: one SQLite file = the whole persistent session ──────────
    // Restarting with the same file restores the paired device; no QR is
    // shown again once pairing has completed.
    let store = SqliteStore::new(&db_path).await?;
    info!("SQLite session store opened at {db_path}");

    // ── client + event handlers + connect (the Bot facade) ───────────────
    let mut builder = Bot::builder()
        .with_backend(store)
        .on_qr_code(|code, timeout| async move {
            println!(
                "Scan with WhatsApp > Linked Devices (rotates every {}s):\n{code}",
                timeout.as_secs()
            );
        })
        .on_connected(|_client| async {
            info!("connected and authenticated");
        })
        .on_message(|ctx| async move {
            let source = &ctx.info.source;
            let scope = if source.is_group { " (group)" } else { "" };
            match ctx.message.text_content() {
                Some(text) => println!("incoming text{scope} from {}: {text}", source.sender),
                None => println!("incoming non-text message{scope} from {}", source.sender),
            }
        });

    // Optional one-shot send, performed once the connection is authenticated.
    if let (Some(to), Some(text)) = (send_to, send_text) {
        builder = builder.on_connected(move |client| {
            let to = to.clone();
            let text = text.clone();
            async move {
                match to.parse::<Jid>() {
                    Ok(jid) => match client.send_text(jid, text).await {
                        Ok(sent) => info!("sent message {} to {}", sent.message_id, sent.to),
                        Err(e) => error!("send failed: {e}"),
                    },
                    Err(e) => error!("invalid --to JID {to:?}: {e}"),
                }
            }
        });
    }

    let bot = builder.build().await?;

    // ── connect and run until Ctrl-C or --seconds elapses ────────────────
    let mut handle = bot.spawn(); // starts the run loop; `handle.client()` is the Client
    info!("connecting to WhatsApp (run budget: {run_secs}s)...");

    tokio::select! {
        _ = &mut handle => info!("bot run loop ended"),
        _ = tokio::signal::ctrl_c() => {
            info!("Ctrl-C received, shutting down");
            handle.shutdown().await;
        }
        _ = tokio::time::sleep(Duration::from_secs(run_secs)) => {
            info!("{run_secs}s elapsed, shutting down");
            handle.shutdown().await;
        }
    }

    Ok(())
}

fn arg_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}
```

`whatsapp_rust::prelude::*` (defined at `src/lib.rs:234-269`) supplies everything used above: `Bot`, `SqliteStore`, `Jid`, `SendError`/`SendResult`, and the event types.

---

## 3. The exact API path, signature by signature

### 3.1 Create the client (with SQLite storage)

```rust
// src/bot.rs:510-514
pub fn builder()
-> BotBuilder<MissingBackend, DefaultTransportState, DefaultHttpState, DefaultRuntimeState>
```

With `tokio-transport`/`ureq-client`/`tokio-runtime` enabled, the three `Default*State` aliases resolve to `Provided` (`src/bot.rs:42-55`), so **the storage backend is the only required field**.

```rust
// src/bot.rs:770-772
pub fn with_backend(self, backend: impl Backend + 'static) -> BotBuilder<Provided, T, H, R>
```

Storage construction (SQLite):

```rust
// storages/sqlite-storage/src/sqlite_store.rs:430-433
pub async fn new(database_url: &str) -> std::result::Result<Self, StoreError>

// storages/sqlite-storage/src/sqlite_store.rs:444-449  (multi-account per-device)
pub async fn new_for_device(
    database_url: &str,
    device_id: i32,
) -> std::result::Result<Self, StoreError>
```

`SqliteStore::new` is `async` because it opens the pool and runs migrations inside `tokio::task::spawn_blocking` (`sqlite_store.rs:498-534`) — call it from within a Tokio runtime.

Building returns the client-bearing `Bot`:

```rust
// src/bot.rs:1318-1320
pub async fn build(self) -> Result<Bot, BotBuilderError>
```

`BotBuilder::build_graph` wires the real `Arc<Client>` via `Client::builder()` (`src/bot.rs:1386-1421`); `Bot::client(&self) -> Arc<Client>` (`src/bot.rs:516-518`) exposes it before starting.

### 3.2 Register the event handlers

Typed convenience registrars (all just `on_event_for` wrappers):

```rust
// src/bot.rs:975-979 — QR pairing payload + rotation window
pub fn on_qr_code<F, Fut>(self, handler: F) -> Self
where
    F: Fn(String, std::time::Duration) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
```

```rust
// src/bot.rs:951-955 — one MessageContext per inbound message
pub fn on_message<F, Fut>(self, handler: F) -> Self
where
    F: Fn(MessageContext) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
```

```rust
// src/bot.rs:1066-1070 — fires once connected and authenticated
pub fn on_connected<F, Fut>(self, handler: F) -> Self
where
    F: Fn(Arc<Client>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
```

Raw event-level registration (if you need `Event` directly):

```rust
// src/bot.rs:930-936
pub fn on_event_for<F, Fut>(self, kinds: &[EventKind], handler: F) -> Self
where
    F: Fn(Arc<Event>, Arc<Client>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
```

Handlers are stored at build time; the bus only materializes the kinds any handler asked for (`EventInterest::of(kinds)`, `wacore/src/types/events.rs:309`). Default delivery is `EventDelivery::Concurrent` (`src/bot.rs:241-262`); `with_event_delivery(EventDelivery::Ordered { capacity })` gives in-arrival-order delivery (`src/bot.rs:1123-1126`).

### 3.3 Connect / run

```rust
// src/bot.rs:530-532 — drive the session on the current task
pub async fn run(self)

// src/bot.rs:551-566 — start on the runtime, returns a handle
pub fn spawn(self) -> BotHandle
```

`Bot::run`/`Bot::spawn` call `Client::run`, whose loop performs the actual connect:

```rust
// src/client/lifecycle.rs:703-711
pub async fn connect(self: &Arc<Self>) -> Result<(), ConnectError>
```

Handle access and graceful stop:

```rust
// src/bot.rs:399-401
pub fn client(&self) -> Arc<Client>

// src/bot.rs:406-409 — disconnects, flushes pending state, waits for the loop
pub async fn shutdown(mut self)
```

Related, on the same client: `Client::disconnect` (`src/client/lifecycle.rs:897`), `Client::wait_for_connected(&self, timeout) -> Result<(), ConnectError>` (`src/client/lifecycle.rs:1331`), `Client::is_logged_in` (`src/client/lifecycle.rs:1362`). `BotHandle` itself implements `Future`, resolving when the run loop exits (used in the `select!` above).

### 3.4 QR pairing updates — exact event variants

The high-level callback receives the raw `PairingQrCode` payload; the underlying bus discriminators are:

```rust
// wacore/src/types/events.rs:216-229 (excerpt)
pub enum EventKind {
    Connected,        // 0
    Disconnected,     // 1
    PairSuccess,      // 2
    PairError,        // 3
    LoggedOut,        // 4
    PairingQrCode,    // 5  <- QR pairing updates
    ...
    Messages,         // 10 <- decrypted inbound messages
    ...
}

// wacore/src/types/events.rs:841 (excerpt)
pub enum Event {
    ...
    PairingQrCode(PairingQrCode),   // :847
    ...
    Messages(MessageBatch),         // :868
    ...
}
```

```rust
// wacore/src/types/events.rs:1245-1252
pub struct PairingQrCode {
    /// The QR payload to render.
    pub code: String,
    /// How long this code stays valid before the next one rotates in.
    pub timeout: std::time::Duration,
}
```

Both enums are `#[non_exhaustive]`: raw `match` arms need a wildcard. `Event::kind()` maps back to `EventKind` (`events.rs:1028`).

### 3.5 Receive incoming messages

`on_message` fans out `Event::Messages(MessageBatch)` into `MessageContext`s (implementation at `src/bot.rs:956-971`):

```rust
// src/bot.rs:99-104
pub struct MessageContext {
    pub message: Arc<wa::Message>,
    pub info: MessageInfo,
    pub client: Arc<Client>,
}
```

```rust
// wacore/src/types/events.rs:1126-1130
pub struct InboundMessage {
    pub message: Arc<wa::Message>,
    pub info: Arc<MessageInfo>,
}
```

`MessageInfo`/`MessageSource` carry the routing data (`wacore/src/types/message.rs:309`, `:104-119`): `info.source.chat`, `info.source.sender`, `info.source.is_from_me`, `info.source.is_group`, `info.id`, `info.timestamp`, `info.push_name`, `info.is_offline`.

Extract plain text with the `MessageExt` trait (in the prelude; declaration `wacore/src/proto_helpers.rs:139`, impl `:351`):

```rust
fn text_content(&self) -> Option<&str>;
```

Reply helpers on the context (`src/bot.rs:140-155`):

```rust
pub async fn reply(&self, text: impl Into<String>)
    -> Result<crate::send::SendResult, crate::send::SendError>
pub async fn reply_quoting(&self, text: impl Into<String>)
    -> Result<crate::send::SendResult, crate::send::SendError>
```

### 3.6 Send a text message

From inside a handler use `ctx.reply(...)`; from application code use the client:

```rust
// src/send/mod.rs:863-875
pub fn send_text(
    &self,
    to: impl Into<Jid>,
    text: impl Into<String>,
) -> impl Future<Output = Result<SendResult, SendError>> + '_
```

```rust
// src/send/mod.rs:846-860 — typed protobuf body, same result
pub fn send_message(
    &self,
    to: impl Into<Jid>,
    message: wa::Message,
) -> impl Future<Output = Result<SendResult, SendError>> + '_
```

```rust
// src/send/mod.rs:425-428
pub struct SendResult {
    pub message_id: String,
    pub to: Jid,
}
```

JID construction: `Jid: FromStr` (`wacore/binary/src/jid.rs:984`), e.g. `"15551234567@s.whatsapp.net".parse::<Jid>()` for a DM or `"<id>@g.us"` for a group. The example performs the send from `on_connected`, which fires after login, so `send_text` will not race the connection; sending before pairing returns `SendError::NotLoggedIn`.

---

## 4. SQLite session persistence

**Layout created:** one database file at the path you pass, plus transient `-wal` / `-shm` sidecar files next to it while the process is running. No directories are created; the parent directory must already exist.

- `SqliteStore::new(path)` opens/creates `<path>`; `PRAGMA journal_mode = WAL` is applied at open (`storages/sqlite-storage/src/sqlite_store.rs:523`) and the embedded migrations run immediately (`sqlite_store.rs:52`, `:527`).
- `parse_database_path` (`sqlite_store.rs:361`) accepts a bare path or a `sqlite://` prefix, and rejects `:memory:` (`sqlite_store.rs:362-367`). A relative path resolves against the process working directory.
- Default device id is `1` (`SqliteStore::new` -> `build(url, 1, ...)`, `sqlite_store.rs:431-432`); use `SqliteStore::new_for_device(url, id)` if one process serves multiple accounts.

**What persists:** the tables created by the migrations, observed in the verified run:

```
__diesel_schema_migrations   msg_secrets                 app_state_keys
app_state_mutation_macs      app_state_versions          base_keys
device                       device_registry             group_metadata
identities                   lid_pn_mapping              prekeys
pending_inbound_messages     sender_key_devices          sender_keys
sent_messages                sessions                    signed_prekeys
tc_tokens
```

`device` holds the authentication material (`noise_key`, `identity_key`, `signed_pre_key`, `adv_secret_key`, account/LID/PN). Signal session state lives in `sessions`/`identities`/`prekeys`/`sender_keys`, app state in `app_state_*`.

**Restart behavior:** on every `BotBuilder::build`, `PersistenceManager::new` (`src/store/persistence_manager.rs:36-72`) checks for the device row — it creates one on first run, otherwise loads it: `"Loaded existing device data (PushName: '...'). Initializing Device."`. Once the QR pairing has been scanned successfully, subsequent runs with the same file authenticate from the stored credentials and **no QR is shown**. (The verified run was never paired, so it still showed a QR on the second launch while proving the row was loaded.)

**Flushing:** the client saves on a background cadence (30 s, `src/bot.rs:1394`) and `BotHandle::shutdown().await` performs a graceful disconnect that flushes the device snapshot and pending secrets. Prefer `shutdown()` over dropping the handle (dropping aborts the task, `src/bot.rs:391`). For at-least-once inbound processing there is `BotBuilder::with_inbound_durability_hook` (`src/bot.rs:1153`), which is outside this quickstart.

---

## 5. Error handling

| Boundary | Type | Variants / notes |
|---|---|---|
| `SqliteStore::new` | `StoreError` (`wacore/src/store/error.rs:5-35`) | `Io`, `Connection`, `Database`, `Migration`, `InvalidConfig`, `Validation`, `Serialization`, `RetriesExhausted { op }`, `DeviceNotFound(i32)` |
| `BotBuilder::build` | `BotBuilderError` (`src/bot.rs:86-94`) | `Store(StoreError)` (device init failed) or `Client(ClientBuilderError)` |
| `Client::connect` | `ConnectError` (`src/client.rs:640-662`) | `AlreadyConnected`, `NotActivated`, `Timeout { stage, timeout }`, `Version`, `Transport`, `Handshake` |
| `Client::send_text` / `send_message` | `SendError` (`src/send/mod.rs:41-63`) | `Client(ClientError)`, `NotLoggedIn` (not paired yet), `Iq(IqError)`, `InvalidRequest(String)`, `Internal(anyhow::Error)` |

All implement `std::error::Error`, so the example's `Result<(), Box<dyn std::error::Error>>` works with plain `?`. Retriable SQLite `BUSY`/`LOCKED` conditions are already retried internally (`sqlite_store.rs:620-628`, 5 attempts), so callers should not add an unbounded retry loop around `SqliteStore` operations. The QR payload is a live credential: log/render it, but do not persist it in application logs once pairing completes.

---

## 6. Verification

Environment: macOS arm64, `rustc 1.98.1 (48a229cea 2026-09-01)`, `cargo 1.98.1` (stable), scratch crate depending on `whatsapp-rust v0.7.0` (resolved: `tokio 1.53.1`, `wacore 0.7.0`, `whatsapp-rust-sqlite-storage 0.7.0`).

### Build (debug)

```
$ cargo build
   Compiling whatsapp-rust v0.7.0
   Compiling wa-quickstart v0.1.0 (/private/var/.../whatsapp-rust-research/wa-quickstart)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 47s
```

Zero warnings. The earlier attempt with default features failed as expected:

```
error[E0554]: `#![feature]` may not be used on the stable release channel
 --> .../wacore-binary-0.7.0/src/lib.rs:1:31
1 | #![cfg_attr(feature = "simd", feature(portable_simd))]
  |                               ^^^^^^^^^^^^^^^^^^^^^^
```

### Run (no account; stops after 40 s)

```
$ ./target/debug/wa-quickstart --db ./wa-session.db --seconds 40
[2026-09-13T14:22:03Z INFO  wa_quickstart] SQLite session store opened at ./wa-session.db
[2026-09-13T14:22:03Z INFO  whatsapp_rust::bot] Creating client...
[2026-09-13T14:22:03Z INFO  wa_quickstart] connecting to WhatsApp (run budget: 40s)...
[2026-09-13T14:22:03Z INFO  whatsapp_rust::handshake] Handshake complete (XX), switching to encrypted communication
Scan with WhatsApp > Linked Devices (rotates every 59s):
2@AKL/Sk0BpeNXWg1GOxcMXChBZrns+W1F6L3Uv3X6IJhUJN16sXsK0R6cLVQjd/fAQsFBQwle…<truncated>
[2026-09-13T14:22:43Z INFO  wa_quickstart] 40s elapsed, shutting down
[2026-09-13T14:22:43Z INFO  whatsapp_rust::client::lifecycle] Disconnecting client intentionally.
[2026-09-13T14:22:43Z INFO  whatsapp_rust::client::lifecycle] Expected disconnect (e.g., 515), reconnecting immediately...
[2026-09-13T14:22:43Z INFO  whatsapp_rust::client::lifecycle] Client run loop has shut down.
```

The QR stage was reached (transport handshake + `PairingQrCode` dispatched), then the process shut down gracefully. `--to`/`--text` were not exercised because sending requires a paired account (`SendError::NotLoggedIn` otherwise); that code path is compile-verified.

### Persistence evidence

Second launch with the same file:

```
$ RUST_LOG=whatsapp_rust::store=debug,wa_quickstart=info ./target/debug/wa-quickstart --db ./wa-session.db --seconds 12
[... INFO  wa_quickstart] SQLite session store opened at ./wa-session.db
[... DEBUG whatsapp_rust::store::persistence_manager] PersistenceManager: Ensuring device row exists.
[... DEBUG whatsapp_rust::store::persistence_manager] PersistenceManager: Attempting to load device data via Backend.
[... DEBUG whatsapp_rust::store::persistence_manager] PersistenceManager: Loaded existing device data (PushName: ''). Initializing Device.
```

Database state after the clean shutdown:

```
$ sqlite3 wa-session.db "select id, push_name, length(noise_key), length(identity_key) from device;"
1||64|64
```
