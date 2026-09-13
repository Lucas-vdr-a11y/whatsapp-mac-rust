# WhatsApp calling for RustWA — verified integration plan (M7)

Status: research complete, skeleton landed, **not end-to-end yet**. This document records what was
verified against `whatsapp-rust` 0.7.0 on stable Rust 1.98.1 (aarch64-apple-darwin, 2026-09-13),
what was implemented in `crates/whatsapp-core/src/calls.rs`, and exactly what remains before RustWA
can place or receive a real call.

> Scope note: milestone **M7** in `docs/ROADMAP.md` — 1:1 audio, then video and screen share, call
> links, waiting room, group calls, call history sync, missed/ended-elsewhere states. This plan
> covers the protocol side only; the macOS capture/UI shell is summarized in §5.

---

## 0. Verdict in one page

| Capability | Upstream 0.7.0 | RustWA status | Honest assessment |
| --- | --- | --- | --- |
| 1:1 audio call (place/answer/reject/hang up/mute) | `voip` feature; full facade | `calls.rs` `CallManager` compile-verified behind `--features calls`; **unreachable** until `client.rs` owns it | Feasible now. Missing: client.rs/events.rs wiring + a PCM capture backend. |
| Incoming ringing + accept/reject | `Event::IncomingCall` with raw offer + callKey; `Voip::accept` builder | `CallManager::handle_event` maps it and retains the offer; **event not subscribed** by `WaClient` | Needs a ~15-line `client.rs` patch (§7). Cannot be done from `calls.rs` (fields private). |
| Missed calls / answered-elsewhere | `Event::MissedCall` (`Offline`/`Remote`), `Event::CallEndedElsewhere` (`Accepted`/`Rejected`) | Mapped to `CallUpdate::{Missed,EndedElsewhere}` | Same wiring requirement. |
| 1:1 video | `VideoSource`/`VideoSink` (H.264 Annex-B AUs), `CallHandle::start_video/accept_video/stop_video` | No code yet; `CallMediaFactory` can supply video endpoints | Transport exists; **media conformance unproven** (upstream's own gate says callback traces are pending). |
| Screen share | Group-control path only (`set_screen_share`, needs group state + active video plane) | Not started | **1:1 screen share is not exposed at all in 0.7.0**; group screen share works only after video is live. |
| Group calls | `group_call`, `group_call_by_id`, invite/ring, roster, reactions, raise hand | Not started | APIs exist; upstream conformance suite leaves slow group scenarios unverified. |
| Call links | `create_call_link`, `preview_call_link`, `join_call_link`, waiting room admit/deny/approval | Not started | Complete API incl. heartbeats; live-server maturity unverified. |
| Call history sync | `HistorySync.callLogRecords` + `Message.callLogMesssage` proto; **no typed app-state reader in 0.7.0** | `CallLogEntry` parsers implemented and unit-tested in the default build | Readable from history-sync blobs today; typed `call_log` app-state sync only exists on upstream `main` (post-0.7.0). |
| Receive while app is closed | — | — | Not possible on macOS without a push channel (APNs VoIP is iOS-only). Ringing works while the app runs. |

**The one structural blocker:** `WaClient` stores the upstream `BotHandle` in a private field and its
`client()` accessor is private to `client.rs`. A sibling module cannot reach `Arc<whatsapp_rust::Client>`,
so `calls.rs` cannot implement a single real method on `WaClient` without a wiring edit to
`client.rs` (which another workstream owns). `calls.rs` therefore ships a complete, feature-gated
adapter plus the exact patch needed to make it reachable (§7).

---

## 1. Toolchain and feature matrix (verified)

### 1.1 Stable vs nightly

Upstream pins `nightly-2026-06-16` (`whatsapp-rust-0.7.0/rust-toolchain.toml`) because its default
`simd` feature enables `portable_simd`. RustWA builds on stable (`rust-toolchain.toml` → `stable`,
currently rustc 1.98.1). We already compile upstream with `default-features = false` and no `simd`.

**Verified: the VoIP features are stable-clean.** A scratch probe crate with the upstream
`voip-mlow` profile compiled with `rustc 1.98.1` with no `#![feature]` errors:

```
$ CARGO_TARGET_DIR=.../whatsapp-rust/target cargo check     # probe crate, features = [... , "voip-mlow"]
   Checking webrtc-dtls v0.12.0
   Checking webrtc-sctp v0.17.2
   Checking webrtc-data v0.17.2
   Checking whatsapp-rust v0.7.0
   Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 07s
```

The full aggregate (`voip`) additionally compiles the vendored, cmake-built native libopus:

```
$ cargo check     # probe crate, features = [..., "voip"]
   Compiling audiopus_sys v0.2.2
   Checking opus v0.3.1
   Finished `dev` profile [unoptimized + debuginfo] target(s) in 45.70s
```

So `simd` remains the only nightly blocker; no `voip` code path needs nightly.

### 1.2 Upstream feature graph (0.7.0 `Cargo.toml:77-100`)

| Feature | Pulls | What you get | Verified on stable |
| --- | --- | --- | --- |
| `voip-runtime` | `wacore/voip`, tokio net, `webrtc-dtls 0.12`, `webrtc-sctp 0.17`, `webrtc-data 0.17`, `webrtc-util 0.17`+`0.11`, `rustls 0.23` | Relay transport (UDP/DTLS/SCTP), signaling facade, encoded I/O; no audio codec | ✅ |
| `voip-encoded` | `voip-runtime` | Same as above (alias) | ✅ (implied) |
| `voip-mlow` | `voip-runtime` + `wacore/voip-mlow` (pure Rust, `aes-gcm`/`zerocopy`) | PCM↔MLOW adapter, the compatibility default | ✅ |
| `voip-libopus` | `voip-encoded` + `opus 0.3` (`audiopus_sys`, vendored C) | Native Opus adapter (`WaOpusEncoder`/`WaOpusDecoder`) | ✅, needs cmake (4.4.2 used) |
| `voip` | `voip-mlow` + `voip-libopus` | MLOW + native Opus | ✅ |

`voip` cannot be built for `wasm32`/`espidf` (upstream `compile_error!` in `src/voip/mod.rs:10-18`);
irrelevant for macOS.

### 1.3 RustWA feature additions (in `crates/whatsapp-core/Cargo.toml`)

```toml
[features]
calls = ["whatsapp-rust/voip-mlow"]                    # pure-Rust MLOW audio + relay runtime
calls-opus = ["calls", "whatsapp-rust/voip-libopus"]   # + native libopus (cmake, vendored C)
```

Nothing else changed; the default feature set is unchanged and remains free of the media stack.
`cargo check -p whatsapp-core --features calls` and `--features calls-opus` both pass (see §6).

Recommendation: ship `calls` first (no native build, no cmake in CI). Add `calls-opus` only when an
encoded-Opus producer exists.

### 1.4 Build/repro notes

- Enabling `calls` adds the webrtc-*/rustls/aes-gcm/zerocopy tree to `Cargo.lock` (`calls-opus`
  adds `opus`, `audiopus_sys`, `cmake`). They appear in the lockfile even in default builds.
- `audiopus_sys` vendors libopus and builds it with cmake; CI images must provide cmake or the
  `calls-opus` feature fails at build time (not a Rust error).
- The upstream CLI used in `agent_docs/voip_audio_codecs.md` (`whatsapp-rust-voip-cli`) and the
  oracle tooling (`cargo xt oracle …`) are **not** published on crates.io; live testing needs the
  upstream git workspace.

---

## 2. Upstream API surface (0.7.0, exact signatures)

All paths are in the registry checkout
`whatsapp-rust-0.7.0/`; `client/voip.rs` is re-exported at the crate root as
`whatsapp_rust::{CallError, Voip}` (`src/lib.rs:127`). `pub mod voip` is gated on
`voip-runtime` (`src/lib.rs:183-184`).

### 2.1 Accessor and 1:1 flows (`src/client/voip.rs`)

```rust
impl Client { pub fn voip(&self) -> Voip<'_> }                                 // :393

impl Voip<'_> {
    // Always available (no voip feature needed — the stanza builders are in wacore):
    pub async fn reject(&self, incoming: &IncomingCall) -> Result<(), CallError>;            // :934
    pub async fn reject_call(&self, call_id: &str, peer: &Jid, creator: &Jid) -> ...;        // :949
    pub async fn terminate(&self, call_id: &str, peer: &Jid, creator: &Jid) -> ...;          // :1815

    #[cfg(feature = "voip-runtime")]
    pub fn accept<'b>(&'b self, incoming: &'b IncomingCall) -> AcceptCall<'b>;                // :999
    #[cfg(feature = "voip-runtime")]
    pub fn call<'b>(&'b self, peer: &'b Jid) -> OutgoingCall<'b>;                             // :1010
}
```

Builder → handle, both in `src/voip/facade.rs`:

```rust
impl AcceptCall<'_>   { pub async fn start(self) -> Result<CallHandle, CallError>; }  // :177
impl OutgoingCall<'_> { pub async fn start(self) -> Result<CallHandle, CallError>; }  // :531
```

Before `start()`, attach endpoints with the shared media-builder macro
(`facade.rs:77-117`):

```rust
pub fn audio<S: AudioSource, K: AudioSink>(self, source: S, sink: K) -> Self;       // :81
pub fn encoded_audio<S: EncodedAudioSource, K: EncodedAudioSink>(
    self, format: AudioFormat, source: S, sink: K) -> Self;                         // :94
pub fn video<S: VideoSource, K: VideoSink>(self, source: S, sink: K) -> Self;       // :108
```

`OutgoingCall` resolves PN→LID (usync on cache miss), generates the callKey, encrypts it per peer
device, sends `<offer>`, and returns a dormant handle until the server's relay ack arrives
(`facade.rs:489-531`).

### 2.2 Group calls (`src/client/voip.rs`, `facade.rs`)

```rust
pub fn group_call<'b>(&'b self, targets: &'b [Jid]) -> OutgoingGroupCall<'b>;   // voip.rs:1016
pub fn group_call_by_id<'b>(&'b self, group_jid: &'b Jid) -> GroupBoundCall<'b>;// voip.rs:1023
pub async fn preaccept_group_invite(&self, incoming: &IncomingCall) -> ...;     // voip.rs:1039
pub async fn accept_group_invite(&self, incoming: &IncomingCall) -> ...;        // voip.rs:1081

impl OutgoingGroupCall<'_> { pub async fn start(self) -> ...; }                 // facade.rs:664
impl GroupBoundCall<'_>   { pub async fn start(self) -> ...; }                  // facade.rs:943
```

`OutgoingGroupCall::group(jid)` binds an ad-hoc call to an existing group (`facade.rs:655`);
`GroupBoundCall` resolves the group roster automatically (`facade.rs:942`).

`CallHandle` controls (`facade.rs:3001`, all clone the same underlying call):

```rust
pub fn call_id(&self) -> &str;                  // :3136
pub fn peer_jid(&self) -> Jid;                  // :3144  (call-scoped JID once promoted/answered)
pub fn call_creator(&self) -> &Jid;             // :3158
pub fn group_state(&self) -> Option<GroupCallState>; // :3163
pub async fn invite_participant(&self, target: &Jid) -> Result<(), CallError>; // :3169
pub async fn ring_participant(&self, target: &Jid) -> Result<(), CallError>;   // :3174
pub async fn set_hand_raised(&self, raised: bool) -> ...;                       // :3245
pub fn send_reaction(&self, emoji: impl Into<String>) -> ...;                   // :3260
pub async fn start_screen_share(&self, id: Option<u32>) -> ...;                 // :3274
pub async fn stop_screen_share(&self) -> ...;                                   // :3290
pub async fn set_approval_required(&self, enabled: bool) -> ...;                // :3306
pub async fn admit_waiting_user(&self, user: &Jid) -> ...;                      // :3321
pub async fn deny_waiting_user(&self, user: &Jid) -> ...;                       // :3336
pub fn set_muted(&self, muted: bool);                                           // :3353
pub fn is_muted(&self) -> bool;                                                 // :3358
pub async fn start_video<S: VideoSource, K: VideoSink>(&self, source: S, sink: K) -> ...;  // :3368
pub async fn accept_video<S, K>(&self, token: VideoUpgradeToken, source: S, sink: K) -> ...; // :3382
pub async fn announce_video_enabled(&self) -> ...;                              // :3402
pub async fn stop_video(&self) -> ...;                                          // :3427
pub async fn hangup(&self);                                                     // :3694
pub fn events(&self) -> async_channel::Receiver<CallEvent>;                     // :3727
pub async fn wait_ended(&self);                                                 // :3732
```

Crucial semantics from upstream:

- **Screen share is group-only.** `start_screen_share` → `set_screen_share_for_generation`, which
  hard-requires an active group state with `media == "video"` and a local `VideoState::Enabled`
  plane (`client/voip.rs:1659-1673`). For a 1:1 call the only 0.7.0 option is swapping the
  `VideoSource` to a screen-capture source and using the normal video plane (the peer sees a video
  call, not a screen-share tile).
- **Hangup tears down locally even if the stanza send fails** (`client/voip.rs:1832-1840`).
- **Video upgrade** is signalled via `CallEvent::VideoStateChanged { state, orientation,
  upgrade_token }`; accepting needs the exact token (`wacore/src/voip/engine.rs:483-489`).

### 2.3 Call links and waiting room

```rust
pub fn call_link<'b>(&'b self, token_or_url: &'b str, media: CallLinkMedia) -> CallLinkCall<'b>; // :1029
pub async fn create_call_link(&self, media: CallLinkMedia) -> Result<CallLink, CallError>;       // :1120
pub async fn preview_call_link(&self, token_or_url: &str, media: CallLinkMedia) -> Result<CallLinkPreview, CallError>; // :1141
pub async fn join_call_link(&self, token_or_url: &str, media: CallLinkMedia) -> Result<CallLinkJoin, CallError>;       // :1168
pub async fn waiting_room_heartbeat(&self, call_id: &str, creator: &Jid) -> ...;  // :1424
pub async fn set_approval_required(&self, call_id: &str, creator: &Jid, enabled: bool) -> ...; // :1375
pub async fn admit_waiting_user(&self, call_id: &str, creator: &Jid, user: &Jid) -> ...;       // :1439
pub async fn deny_waiting_user(&self, call_id: &str, creator: &Jid, user: &Jid) -> ...;        // :1474
```

`CallLink::url()` returns `https://call.whatsapp.com/{audio|video}/{token}`
(`wacore/src/types/group_call.rs:308-316`). `CallLinkJoin` explicitly reports
`in_waiting_room`, `is_admin`, the `WaitingRoom` roster and the admitted `GroupCallUpdate`
(`group_call.rs:381-393`); the facade owns the admission heartbeat (`client/voip.rs:1288-1308,
1750-1812`) and retries saturated unknown-id joins (`client/voip.rs:1196-1234`).

### 2.4 Media endpoint traits

Audio (`src/voip/audio.rs`; PCM is **16 kHz mono i16, exactly 960 samples per 60 ms frame**):

```rust
pub trait AudioSource: Send + Sync + 'static { fn frames(&self) -> async_channel::Receiver<Vec<i16>>; }       // :20
pub trait AudioSink:   Send + Sync + 'static { fn playout(&self) -> async_channel::Sender<Vec<i16>>; }      // :28
pub trait EncodedAudioSource { fn frames(&self) -> Receiver<Bytes>; }                                        // :50
pub trait EncodedAudioSink   { fn frames(&self) -> Sender<EncodedAudioFrame>; }                              // :55
```

Bare `async_channel` endpoints implement the traits (`audio.rs:35-45`), which is what
`calls::manager::CallMedia` uses. A closed source does **not** end the call (relay keepalive keeps
running).

Video (`src/voip/video.rs`; pre-encoded H.264 Annex-B access units, one per item):

```rust
pub trait VideoSource: Send + Sync + 'static {
    fn frames(&self) -> Receiver<Vec<u8>>;                                       // :15
    fn rtp_timestamp_stride(&self) -> u32 { 90_000 / 15 }                        // :19
}
pub trait VideoSink: Send + Sync + 'static { fn playout(&self) -> Sender<VideoFrame>; } // :26
```

The library never touches pixels. Upstream requires H.264 Constrained Baseline (`avc1.42E01F`),
repeated SPS/PPS, 15 fps default stride, up to 1280x720@20fps/~2 Mbps (`video.rs:1-6`).

### 2.5 Events, phases, errors

`Event` variants (`wacore/src/types/events.rs`; `EventKind` discriminants frozen and append-only):

| Event | Line | Payload essentials |
| --- | --- | --- |
| `Event::IncomingCall(IncomingCall)` | `:896` | `from`, `action: CallAction`, `group: Option<Box<GroupCallUpdate>>`, timestamp, and (with `wacore/voip`) `media: Option<Box<MediaOffer>>` — the encrypted callKey + relay. |
| `Event::MissedCall(MissedCall)` | `:901` | `from`, `call_id`, `reason: MissedReason::{Offline, Remote}`. Explicitly must NOT ring. |
| `Event::CallEndedElsewhere(CallEndedElsewhere)` | `:906` | `from`, `call_id`, `outcome: ElsewhereOutcome::{Accepted, Rejected}`. |

`EventKind::{IncomingCall, MissedCall, CallEndedElsewhere}` at `types/events.rs:240-242`.
Dispatch sites: `src/handlers/call.rs:198` (offline replay → `MissedCall::Offline`),
`:428-440` (unanswered `<terminate>` → `MissedCall::Remote`; `accepted_elsewhere` /
`rejected_elsewhere` → `CallEndedElsewhere`), `:796`/`:927` (`IncomingCall` for offers),
`:1077` (group-control replay). Ringing offers carry an internal `ringing_generation` used by
`accept/reject` to distinguish a re-offer of the same call
(`wacore/src/types/call.rs:418-429`).

`CallEvent` (media-engine diagnostics; `wacore/src/voip/engine.rs:459-532`) includes
`RelayAllocated`, `RelayAllocateFailed(u16)`, `RelayAllocateTimedOut`, `RelayReconnectTimedOut`,
`AudioFormatMismatch`, `VideoStateChanged`, `GroupUpdated`, `WaitingRoomUpdated`,
`WaitingRoomHeartbeatFailed`, `HandRaised`, `ScreenShareChanged`, `Reaction`,
`RtcpReceived`, `OutboundMediaDropped`. **There is no explicit "answered/connected" event**, so a
call cannot be marked `Active` from the media stream alone — this is why
`calls::manager::CallState::Active` is currently never emitted.

`CallError` (`client/voip.rs:867-930`, `#[non_exhaustive]`) covers `NotAnOffer`, `MissingAudio`,
`AudioFormatNotOffered(u32)`, `VideoNotOffered`, `CallEndedDuringSetup`, `Decrypt`, `Setup`,
`Connect`, `Media(&'static str)`, `NoDevices`, `MissingDeviceIdentity`, `Response`,
`ResponseTimeout`, `EmptyCallId`, `Send`.

### 2.6 Call history / call log in 0.7.0

- `wa::HistorySync.call_log_records: Vec<wa::CallLogRecord>` — proto field 13
  (`waproto/whatsapp.proto:2448`); the record carries result/type/duration/start/direction/
  video/call-link/participants/group/creator (`whatsapp.proto:1020-1065`).
- `wa::Message.call_log_messsage` (sic) — proto field 69 (`whatsapp.proto:2805`), the in-chat
  entry; nested `CallLogMessage` with `is_video`, `call_outcome`, `duration_secs`, `call_type`,
  `participants`.
- `LazyHistorySync::get()` full-decodes the retained blob into `wa::HistorySync`
  (`wacore/src/types/events.rs:155-164`); the compressed blob is always retained when
  `Event::HistorySync` is dispatched (`whatsapp-rust/src/history_sync.rs:358-390, 463-477`).
- The **app-state** `CallLog` schema constant exists (`WAWebCallLogSync`,
  `wacore-appstate/src/schemas.rs:359-379`) and the device advertises
  `support_call_log_history: Some(true)` (`wacore/src/store/device.rs:217`), but **0.7.0 has no
  typed reader/writer or `CallLogUpdate` event**: upstream `main` added
  `src/features/call_log.rs` after the 0.7.0 tag (the v0.7.0 tag's `src/features/` contains no
  such file). Our local `docs/parity-matrix.md` cites the main-branch file; treat that row as
  post-0.7.0.
- What RustWA can do today: parse `HistorySync` chunks and `CallLogMessage` messages with the
  helpers implemented in `calls.rs` (`CallLogEntry`, `call_log_entries`,
  `call_log_entry_from_message`). Wiring `EventKind::HistorySync` belongs to the same
  `client.rs` workstream and may already be landing concurrently (a `bus_history` handler was
  present during this session).

---

## 3. Capability-by-capability plan

### 3.1 1:1 audio (first slice)

Upstream flow: `client.voip().call(&peer).audio(mic_rx, speaker_tx).start().await` →
`CallHandle`; answer: `.voip().accept(&incoming).audio(...).start().await`. All building blocks
are in §2.1/§2.4.

RustWA work:
1. `calls::manager::CallManager` (already written, feature-gated): outgoing, answer/reject,
   hangup, mute, incoming/missed/elsewhere mapping.
2. `client.rs` wiring (§7) — required for reachability.
3. macOS PCM backend implementing `CallMediaFactory`: AVAudioEngine (mic tap + player node) or
   cpal; resample hardware rate → 16 kHz and buffer to exactly 960 samples.
4. UI: Calls screen, ringing overlay, in-call controls (design-spec §4.12).

Conformance: MLOW is the compatibility default and has 131 upstream tests; the PT120 native-Opus
path was verified against Android/Web and a live full-duplex Android call
(`agent_docs/voip_audio_codecs.md:49-50`).

### 3.2 1:1 video

Add `video` endpoints to `CallMedia` (already possible) and call `.video(source, sink)` on the
builder; mid-call upgrade uses `CallHandle::start_video` / `accept_video(token, ..)` driven by
`CallEvent::VideoStateChanged`.

macOS work: AVCaptureSession (camera) → VideoToolbox H.264 encoder (Constrained Baseline,
SPS/PPS repeat, ≥15 fps) → `Vec<u8>` AUs; peer AUs → VideoToolbox decode → Metal/CoreVideo render.
Camera selection, front/rear semantics and orientation are ours; upstream transports metadata.

**Unverified upstream:** the conformance gate lists "audio/video callback bytes" as
"infrastructure ready; callback ABIs still need derivation", and the camera-control test on
`main` is explicitly a ringing-call test ("the peer decoder and renderer have never started",
"live Android front/rear switching still requires a device retest"). Plan for a full
device-level video test campaign, not just unit tests.

### 3.3 Screen share

- **Group calls:** `CallHandle::start_screen_share(id)` / `stop_screen_share()` once the call has
  an active video plane; `ScreenShareChanged` events update the UI. Requires waiting-room/admin
  context only for call links.
- **1:1 calls:** no API in 0.7.0. Options: (a) deliver "video call with screen content" by
  swapping the `VideoSource` to ScreenCaptureKit and keeping the camera source idle; (b) wait for
  upstream to expose the 1:1 screen-share signal (WA Web uses a distinct video-state path that
  is not in the 0.7.0 surface). Recommend (a) with honest UI copy, and file an upstream request.

### 3.4 Group calls

Ad-hoc: `group_call(&targets).audio(...).video(...).start()`. Bound to a group:
`group_call_by_id(&group_jid)`. In-call: `invite_participant`, `ring_participant`,
`set_hand_raised`, `send_reaction`, participant snapshots via
`CallEvent::GroupUpdated(Box<GroupCallUpdate>)` and `CallHandle::group_state()`.
Group state is transaction-ordered and the registry handles epoch fan-out (`EncRekey`).
Upstream verification: slow group scenarios are in the ignored suite; the conformance doc says
the weekly slow run exists but "this run does not validate those slow call scenarios". Treat as
**preview**: build it, but gate release behind live multi-account testing.

### 3.5 Call links + waiting room

Create (`create_call_link(media)` → `CallLink { token, media }` + `url()`), preview
(`CallLinkPreview { creator, waiting_room_enabled, is_admin, .. }`), join
(`join_call_link` or the `call_link()` builder for media), admin controls
(`set_approval_required`, `admit_waiting_user`, `deny_waiting_user`), and heartbeat are complete.
Deep links: register `https://call.whatsapp.com/...` handling in the macOS app and normalize
audio/video from the URL path (upstream validates it in `normalize_call_link_token`,
`client/voip.rs:1844-1876`).

### 3.6 Call history sync

Phase 1 (available immediately): parse `HistorySync.call_log_records` whenever the app handles a
history-sync blob, and classify `Message.callLogMesssage` items; persist into our SQLite store and
render the Calls screen. `calls.rs` supplies the normalization.
Phase 2 (post-0.7.0 / app-state): the `CallLog` app-state schema (`WAWebCallLogSync`) has no typed
API in 0.7.0. Either port upstream `main`'s `features/call_log.rs` (or bump the pin when a release
includes it), then sync deletions (`DELETE_INDIVIDUAL_CALL_LOG`) and cross-device edits.

### 3.7 Receiving calls, ringing and missed-call handling

Verified semantics to implement in the UI:

1. `Event::IncomingCall` (offer) → ring. The offer contains the call id, caller, video flag,
   `callKey`/relay material and (for group calls) `group`.
2. User answers → `Voip::accept(&incoming)...start()` (must use the retained offer; the raw payload
   is why `CallManager` stores it). User declines → `Voip::reject(&incoming)`.
3. Caller gives up / call times out → `Event::MissedCall { reason: Remote }`; offline replay of a
   dead offer → `MissedCall { reason: Offline }` and must not ring.
4. Another linked device answers/declines → `Event::CallEndedElsewhere { Accepted | Rejected }`;
   stop ringing and log "answered/declined on another device" (not "missed").
5. Multi-device: the ringing generation protects accept/reject against re-offers of the same call;
   upstream treats "companion rings without primary" as best-effort (parity-matrix row
   "Call on any linked device").

RustWA specifics: the desktop host must show the ring while the app runs (window + sound +
UNUserNotification); if the user clicks the notification, focus the app and show the ringing
overlay. macOS has no CallKit; a full-screen/floating `NSWindow` with
`.canJoinAllSpaces` + `.floating` is the standard approximation. Ringing while the app is fully
closed requires an APNs VoIP push, which Apple provides only on iOS — accepted limitation.

---

## 4. Upstream conformance: what is proven and what is not

Authoritative shipped doc for 0.7.0 — `agent_docs/voip_audio_codecs.md`:

- "The PT120 native-Opus path was verified against Android/Web implementations and with a live
  full-duplex Android call. MLOW remains the compatibility default." (`:49-50`)
- Encoded API contract, profiles table and negotiation rules (`:16-47`, `:65-95`).
- "Video already accepts external H.264 Annex-B access units. It is not wire-codec-agnostic:
  signaling, packetization, and PT 97 currently target H.264." (`:122-125`)

The conformance gate referenced by our parity matrix does **not** ship with the 0.7.0 crate; it
exists on upstream `main` (moved to the `oxidezap/whatsapp-rust` org). What it says (quoted):

- Audio/video callback bytes: "infrastructure ready; callback ABIs still need derivation".
- RTP/RTCP and H.264 packetization: "enforced; wasm differential trace pending".
- Full slow call scenarios: 26 tests are ignored in the normal run; "scheduling them is not proof
  they pass".
- Conclusion: "Full audio/video callback traces and end-to-end signaling/IQ differential cases
  remain separate evidence to derive. Until those cases exist and pass, this command reports the
  implemented gates, not complete VoIP equivalence."

The media-oracle doc adds for camera controls: the test "remains a ringing-call test. The call
stays in Calling, and the peer decoder and renderer have never started", and "Production
`stop_video`, source/sink ownership and upgrade signaling must not be changed on the strength of
this ringing test alone."

Conclusion for RustWA: audio 1:1 is the only capability we can honestly call proven; video,
screen share, group calls and call links need our own live end-to-end validation before claiming
support.

---

## 5. macOS platform work

### 5.1 Info.plist

| Key | Why |
| --- | --- |
| `NSMicrophoneUsageDescription` | Required before any mic access (TCC prompt text). |
| `NSCameraUsageDescription` | Required before any camera access. |
| `NSLocalNetworkUsageDescription` | Only if we ever use local-network candidate exchange; the 0.7.0 media path is relay-based, so not required for calls today. The key is only consulted when local-network traffic actually occurs. |

Screen Recording has no usage-string key; it is a separate TCC pane.

### 5.2 Entitlements (Hardened Runtime / App Sandbox)

- App Sandbox (Mac App Store): `com.apple.security.device.audio-input`,
  `com.apple.security.device.camera`, `com.apple.security.network.client` (socket + relay), and
  `com.apple.security.files.user-selected.read-write` only if needed for attachments.
- Hardened Runtime (notarized direct distribution): `com.apple.security.device.audio-input` and
  `com.apple.security.device.camera` are required for capture under the hardened runtime; no
  sandbox keys unless sandboxing.
- Screen recording under the hardened runtime needs Screen Recording TCC, not an entitlement.

### 5.3 TCC behavior

- The first `AVCaptureDevice`/`AVAudioEngine` capture triggers the system prompt; prompts show the
  app name and usage string. Denial is sticky per bundle ID; "Reset" is in System Settings →
  Privacy & Security → Microphone/Camera (or `tccutil reset Microphone <bundle-id>` for dev).
- Request access explicitly before starting a call so the ringing UI can explain a denial:
  `AVCaptureDevice.requestAccess(for: .audio|.video)`; screen capture uses
  `CGPreflightScreenCaptureAccess()` / `CGRequestScreenCaptureAccess()`.
- Tauri/WKWebView caveat: if we ever used `getUserMedia` in the webview instead of native capture,
  macOS WebKit routes the permission decision through the `WKUIDelegate`
  (`requestMediaCapturePermissionForOrigin`); whether the wry version in use implements that must
  be verified before relying on it. Native capture in Rust avoids the question entirely — another
  reason to feed `CallMediaFactory` from Rust.

### 5.4 Capture/playback pipeline

- Mic: AVAudioEngine input tap at hardware rate → AVAudioConverter to 16 kHz mono i16 → chunk to
  960 samples → `async_channel::Sender<Vec<i16>>` (engine side receives it). Playback: engine
  `speaker` channel → AVAudioConverter → AVAudioPlayerNode.
  Alternative pure-Rust: cpal + rubato (fewer ObjC bindings, more CPU/glue).
- Camera: AVCaptureSession → VideoToolbox `VTCompressionSession` (H.264 Constrained Baseline,
  `kVTCompressionPropertyKey_RealTime`, repeated SPS/PPS on keyframes, bitrate/fps per upstream
  guidance) → AUs to `Vec<u8>`. Decode: `VTDecompressionSession` → CVPixelBuffer → Metal/CA.
- Screen share: ScreenCaptureKit (`SCStream`) → VideoToolbox, same AU path.
- Audio session quirks on macOS are minimal (no AVAudioSession); handle device changes via
  CoreAudio notifications and rebuild the engine.
- App Nap can throttle timers/network while backgrounded; keep the call alive with
  `ProcessInfo.processInfo.beginActivity(options: .userInitiated | .latencyCritical)` while a call
  is active.
- Ring/notification: `UNUserNotificationCenter` authorization at first run, `NSSound` or
  AVAudioPlayer for the ringtone (repeats), plus a floating call window (no CallKit on macOS).
- CPU: MLOW's analysis-by-synthesis encoder is heavier than CELT; measure on Macs before enabling
  high-bitrate paths.

### 5.5 Signing/CI

- CI builds default features only; add a separate `calls` job (no cmake needed) and a `calls-opus`
  job (cmake + C toolchain) if we adopt Opus.
- Live call tests cannot run in CI (need two linked accounts and real network); they become a
  documented, serialized manual/device campaign (mirroring upstream's oracle approach).

---

## 6. What was implemented in this repo (this session)

`crates/whatsapp-core/src/calls.rs`:

| Item | Feature gate | State |
| --- | --- | --- |
| `CallLogResult`, `CallLogEntry`, `call_log_entries`, `call_log_entry_from_record`, `call_log_entry_from_message` | none (default build) | Implemented + 4 unit tests, passing |
| `WaClient::start_call` / `end_call` | none | Accurate stubs: default says "not compiled in", `calls` says "compiled but not wired" |
| `manager::CallManager` (outgoing, answer/reject by call id or raw offer, hangup, mute, ringing/missed/elsewhere mapping, engine-event monitor) | `calls` | Implemented against the real upstream API; compiles clean with `--features calls` |
| `manager::CallMedia`, `CallMediaFactory` | `calls` | Implemented channel-based endpoint contract for the macOS backend |
| `manager::CallUpdate`, `CallState`, `CallInfo` | `calls` | Serializable UI schema, mirrors `CoreEvent` conventions |

`crates/whatsapp-core/Cargo.toml`: added `[features] calls` and `calls-opus`; no other changes.

Exact commands and observed results (default toolchain, workspace root):

```
$ cargo check -p whatsapp-core
    Checking whatsapp-core v0.1.0 (...)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.69s

$ cargo check -p whatsapp-core --features calls
    Checking whatsapp-core v0.1.0 (...)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.02s

$ cargo check -p whatsapp-core --features calls-opus
   Compiling cmake v0.1.58
   Compiling audiopus_sys v0.2.2
    Checking opus v0.3.1
    Checking whatsapp-rust v0.7.0
    Checking whatsapp-core v0.1.0 (...)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 15.57s

$ cargo test -p whatsapp-core --lib calls::
running 4 tests
test calls::tests::call_log_record_defaults_are_honest ... ok
test calls::tests::maps_history_sync_call_log_records ... ok
test calls::tests::non_call_log_message_has_no_entry ... ok
test calls::tests::maps_call_log_message ... ok
test result: ok. 4 passed; 0 failed; 0 ignored

$ cargo fmt --all
(no output; exit 0)

$ cargo clippy -p whatsapp-core --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.12s

$ cargo clippy -p whatsapp-core --all-targets --features calls -- -D warnings
    Checking whatsapp-core v0.1.0 (...)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.58s
```

Both clippy configurations are green. Note: during the session other workstreams were concurrently
editing `client.rs`/`media.rs` and produced transient compile/clippy failures (E0433, E0560,
`let_underscore_future`); all were resolved by their owners before the final runs above. Re-run
these commands after any rebase.

---

## 7. The wiring `calls.rs` cannot do itself (exact patch)

`WaClient`'s `handle: Mutex<Option<BotHandle>>` and `async fn client()` are private to
`client.rs`, and `CoreEvent` has no call variants (`events.rs:13-30`). `calls.rs` cannot change
either file. The following patch is the complete integration; it should be a separate commit by
whoever owns `client.rs`/`events.rs`.

### 7.1 `events.rs`

```rust
use crate::calls::CallUpdate;

pub enum CoreEvent {
    // ... existing variants ...
    /// A call lifecycle update (ringing, missed, ended-elsewhere, phase, ended).
    Call(CallUpdate),
}
```

### 7.2 `client.rs`

```rust
// 1) New field on WaClient (Option because the upstream Client only exists after connect()).
#[cfg(feature = "calls")]
calls: Arc<tokio::sync::Mutex<Option<crate::calls::manager::CallManager>>>,   // set in new(): None

// 2) Subscribe to call events on the builder, right beside the other on_event_for blocks.
.on_event_for(
    &[
        EventKind::IncomingCall,
        EventKind::MissedCall,
        EventKind::CallEndedElsewhere,
    ],
    {
        let calls = Arc::clone(&self.calls);
        move |event, _client| {
            let calls = Arc::clone(&calls);
            async move {
                let guard = calls.lock().await;
                if let Some(manager) = guard.as_ref() {
                    manager.handle_event(event.as_ref());
                }
            }
        }
    },
)

// 3) After `let bot = Bot::builder()...build().await?;`:
#[cfg(feature = "calls")]
{
    let manager = crate::calls::manager::CallManager::new(
        bot.client(),                                  // Arc<Client>, valid before spawn()
        self.config.call_media.clone(),                // see 7.3
    );
    // Forward manager updates to the domain bus (requires the events.rs variant above).
    let mut updates = manager.subscribe();
    let bus = self.events.clone();
    tokio::spawn(async move {
        while let Ok(update) = updates.recv().await {
            let _ = bus.send(CoreEvent::Call(update));
        }
    });
    *self.calls.lock().await = Some(manager);
}

// 4) Forward the public API (replace the stubs in calls.rs at that point):
pub async fn start_call(&self, chat_id: &Jid, video: bool) -> Result<()> {
    let manager = self.calls.lock().await.clone().ok_or(CoreError::NotConnected)?;
    manager.start_call(chat_id, video).await
}
pub async fn end_call(&self, chat_id: &Jid) -> Result<()> { /* same shape */ }
pub async fn answer_call(&self, call_id: &str) -> Result<()> { /* manager.answer_call */ }
pub async fn reject_call(&self, call_id: &str) -> Result<()> { /* manager.reject_call */ }
pub fn set_call_muted(&self, chat_id: &Jid, muted: bool) -> Result<()> { /* manager.set_muted */ }
```

### 7.3 `ClientConfig` (media backend injection)

```rust
pub struct ClientConfig {
    pub data_dir: PathBuf,
    pub device_name: String,
    #[cfg(feature = "calls")]
    pub call_media: Option<Arc<dyn crate::calls::manager::CallMediaFactory>>, // None until macOS backend lands
}
```

`CallManager::start_call`/`answer_call` return `CoreError::InvalidInput` when the factory is
missing or has no video endpoints, so the app fails cleanly rather than half-connecting.

### 7.4 UI/command surface

- `apps/desktop/src-tauri/src/commands_calls.rs` currently exposes only `calls_start`/`calls_end`.
  Add `calls_answer(call_id)`, `calls_reject(call_id)`, `calls_mute(chat_id, muted)` and a
  `CoreEvent::Call` branch in the webview event forwarding.
- The webview needs the same state machine the design spec describes (§4.12): calls list, ringing
  overlay with accept/decline, in-call controls (`M` mute, `V` camera, `S` screen share, `H`
  raise hand, `R` reaction, `W` hang up).

### 7.5 Ringing path, step by step (what the wiring buys)

1. Upstream `handlers/call.rs` dispatches `Event::IncomingCall` → our `on_event_for` closure →
   `CallManager::handle_event` stores the raw offer and publishes `CallUpdate::Ringing`.
2. Forwarder sends `CoreEvent::Call` to the webview; the UI rings.
3. User accepts → Tauri `calls_answer(call_id)` → `CallManager::answer_call` → upstream
   `accept(...).audio(...).start()`; `CallUpdate::Phase(Connecting)` then engine events flow.
4. User declines → `calls_reject(call_id)` → upstream `Voip::reject`.
5. Caller gives up → `Event::MissedCall` → `CallUpdate::Missed` → call-log row.
6. Another device answers → `Event::CallEndedElsewhere` → `CallUpdate::EndedElsewhere` → stop
   ringing with the correct outcome.

---

## 8. Risks (honest, ranked)

1. **Account risk (highest impact, uncontrollable).** Upstream: "Using custom WhatsApp clients may
   violate Meta's Terms of Service and could result in account suspension." Calling is likely more
   scrutinized than messaging. Test with a disposable account; keep a kill switch for the feature.
2. **Video/group/call-link conformance is not proven.** Upstream's own gate admits missing
   callback traces and never-executed established-call video; slow signaling tests are ignored.
   Budget an XL device-test campaign (two accounts, Mac + Android) before claiming support.
3. **Media backend is greenfield and large.** AVAudioEngine resampling/framing, VideoToolbox
   encode/decode, ScreenCaptureKit, device switching, CPU budgets. None of it exists in the repo;
   `CallMediaFactory` is only the contract.
4. **Ringing while closed is impossible on macOS** without APNs (iOS-only) or a third-party push
   channel. Users must have the app running; document it.
5. **Call history app-state sync is absent in 0.7.0.** History-sync parsing covers initial logs,
   but cross-device edits/deletions need `main`'s `call_log.rs` (or an upstream release) plus a
   pin bump that must be re-validated for stable.
6. **No 1:1 screen share.** Only a video-call-with-screen-content workaround; be honest in the UI.
7. **`calls-opus` adds a native build dependency (cmake + vendored libopus).** Avoid in CI until
   needed; `calls` (MLOW) is pure Rust.
8. **Binary size / build time.** The webrtc-* + rustls stack is not small; measure install size
   against the ROADMAP's published budget before shipping.
9. **Upstream API churn.** 0.7.0 is a snapshot; `main` already adds fixes (video resume, STAP-A,
   call_log). Pinning means bugs stay pinned; bumping means re-validation.
10. **`CallState::Active` is not derivable from `CallEvent`.** Until signaling is forwarded, the UI
    cannot distinguish "ringing" from "talking" on an outgoing call; the duration counter needs
    the `<accept>` signal wired through `client.rs` (or an upstream `CallEvent` addition).
11. **`client.rs`/`media.rs` are actively edited by other workstreams.** Transient compile and
    clippy failures were observed during this session (E0433 in `media.rs`, a `let_underscore_future`
    lint in `client.rs`); the wiring patch must be rebased and the checks in §6 re-run before merge.

---

## 9. Suggested slice order for M7

| Slice | Deliverable | Exit criteria |
| --- | --- | --- |
| M7.0 | `client.rs`/`events.rs` wiring + `CallMediaFactory` with silent/null endpoints | `CoreEvent::Call` reaches the UI; ringing/missed renders for a real call |
| M7.1 | macOS PCM backend + 1:1 audio | Two-account live call with bidirectional audio; mute works |
| M7.2 | Ringing UI + notifications + call-log rows | Missed/accepted-elsewhere states verified on two devices |
| M7.3 | 1:1 video | Live Android↔macOS video, camera toggle, upgrade, orientation |
| M7.4 | Group calls | 3-participant live call; invite/ring; raise hand/reactions |
| M7.5 | Call links + waiting room | Create/preview/join from URL; admin admit/deny |
| M7.6 | Screen share | Group share live; 1:1 workaround documented |
| M7.7 | Call history sync | `HistorySync` records persisted and rendered; app-state sync when the pin allows |

## 10. Evidence index

- Upstream sources: `/Users/lucasvanderunstraat/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/whatsapp-rust-0.7.0/`
  and `wacore-0.7.0/`, `wacore-appstate-0.7.0/`, `waproto-0.7.0/` (crates.io checksums match crates).
- Conformance docs on upstream `main` (post-0.7.0, `oxidezap/whatsapp-rust`):
  `agent_docs/voip_conformance.md`, `agent_docs/voip_media_oracle.md`; shipped 0.7.0 doc:
  `agent_docs/voip_audio_codecs.md`.
- RustWA: `docs/research/whatsapp-rust-api.md` (§1.3 features, §5.4 event table),
  `docs/parity-matrix.md` (Calls domain), `docs/ROADMAP.md` (M7).
- Session artifacts: probe crate compiled with `voip-mlow` and `voip` on rustc 1.98.1; feature
  checks and tests as quoted in §6.
