# Companion scheduled messages — protocol research and implementation plan

Status: **research only, no protocol code shipped** (see [§9 Decision record](#9-decision-record)).
Scope: can a linked (companion) device schedule a message that the WhatsApp server sends at a
future time while the phone and the companion are offline? What would RustWA (`whatsapp-core`)
have to send, and what is still unknown?

Confidence labels used throughout:

| Label | Meaning |
| --- | --- |
| ✅ verified | Read directly from source/proto/binary strings; quoted below. |
| 🧪 inferred | Follows from combining verified facts; no single source states it. |
| ❓ speculative | Hypothesis that needs a live capture; do not build on it yet. |

---

## 1. TL;DR

1. **There is no app-state mutation for scheduled messages.** The upstream syncd registry
   (`wacore_appstate::schemas`, re-exported as `whatsapp_rust::schemas`) has 66 actions and none is
   schedule-related ✅. The installed official macOS app 26.33.73 embeds its own syncd action
   registry of 58 names in the binary and none is schedule-related either ✅. The
   `companion_scheduled_message` schema this task asked about does not exist in either artifact.
2. **The feature is a message-layer feature, not a syncd feature.** The protos already carry
   everything needed to *represent* it: a conditional-reveal envelope on the E2E `Message`
   (`conditionalRevealMessage = 120`, type `SCHEDULED_MESSAGE`) and scheduling metadata on the
   server envelope (`WebMessageInfo.scheduledMessageMetadata = 81`) ✅.
3. **The server does the sending.** At schedule time the server delivers a pre-built E2E payload and
   notifies the account's devices through two MEX (GraphQL) notifications the official app parses:
   `xwa2_notify_scheduled_message_post` / `xwa2_notify_scheduled_message_reveal` ✅. That is what
   makes "phone offline at send time" work.
4. **Upstream 0.7.0 already surfaces MEX notifications generically.**
   `Event::MexNotification { op_name, payload: serde_json::Value }` is dispatched from
   `<notification type="mex"><update op_name="…">{json}</update></notification>` ✅. The two
   scheduled ops are not in `wacore::iq::mex_operations` (no typed decoder), but the raw JSON will
   already reach a consumer today.
5. **The create stanza cannot be reconstructed without guessing.** The one unambiguous wire marker
   is `<meta type="scheduled_message">` ✅, but the attribute that carries the send time, the
   conditional-reveal key derivation/encryption, and the key-registration message are not visible in
   strings. The creation flow still needs a capture with a real account (exact experiment in [§8](#8-capture-experiments-that-close-the-unknowns)).

---

## 2. Method and evidence sources

| Source | Version / path | Used for |
| --- | --- | --- |
| Upstream `whatsapp-rust` | crates.io 0.7.0, `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/whatsapp-rust-0.7.0` | send path, MEX plumbing, app-state mutation API |
| Upstream `wacore` | 0.7.0, same registry root | send classification, AB-props registry, `MexNotification` event |
| Upstream `wacore-appstate` | 0.7.0, same registry root | syncd action schemas and mutation encoding |
| Upstream `waproto` | 0.7.0, same registry root | `whatsapp.proto` message shapes |
| Official app binary | `/Applications/WhatsApp.app` 26.33.73 (build 1049819294), macOS, read-only `strings -a` of `Contents/MacOS/WhatsApp` and `Contents/Frameworks/SharedModules.framework/Versions/A/SharedModules` | symbol-level feature evidence |
| RustWA repo | this repository | current client surface, docs/parity-matrix context |

Binary extraction was read-only (`strings` into a temp file, then `grep`). No app modification, no
network traffic, no account use.

---

## 3. Upstream state (whatsapp-rust 0.7.0 / wacore / waproto)

### 3.1 Proto surface — ✅ verified

`waproto-0.7.0/src/whatsapp.proto`:

```proto
// Message (starts line 2747), field 120:
optional ConditionalRevealMessage conditionalRevealMessage = 120;   // line 2847

// nested in Message (line 3094):
message ConditionalRevealMessage {
  optional bytes encPayload = 1;
  optional bytes encIv = 2;
  optional ConditionalRevealMessageType conditionalRevealMessageType = 3;
  optional string revealKeyId = 4;
  enum ConditionalRevealMessageType {
    UNKNOWN = 0;
    SCHEDULED_MESSAGE = 1;
  }
}

// top-level (line 5197):
message ScheduledMessageMetadata {
  optional string revealKeyId = 1;
  optional bytes revealKey = 2;
  optional uint64 scheduledTime = 3;
}

// WebMessageInfo (starts line 6432), field 81:
optional ScheduledMessageMetadata scheduledMessageMetadata = 81;    // line 6502

// WebMessageInfo.StubType (line 6745):
SCHEDULED_MESSAGE_CREATED = 225;
```

Two structural facts matter 🧪:

- `ConditionalRevealMessage` lives on the **E2E `Message`**: recipients decrypt a message whose
  visible content is a sealed envelope.
- `ScheduledMessageMetadata` lives on the **`WebMessageInfo` envelope**: the server can read the
  schedule and the reveal-key material, and can return it to every companion device.

### 3.2 Syncd action registry — ✅ verified absent

`wacore-appstate-0.7.0/src/schemas.rs` is generated from WhatsApp 2.3000.1042742319 and contains
exactly 66 action schemas (`ALL` list, lines 1391–1458). A case-insensitive search for
`schedul` in the crate returns only the comment "key-schedule compressions" in `lthash.rs`. There
is no action whose key, wire name, index, or value type mentions scheduled messages.

This is decisive for task item 3: there is **no app-state mutation to send**, so there is nothing
that could be implemented "without guessing". Writing `scheduled.rs` against a made-up schema would
also be actively harmful: `AppStateProcessor::build_patch` folds every mutation into the local
ltHash (`wacore-0.7.0/src/appstate_sync.rs`), so an unregistered action/version written to a real
collection can diverge the client's hash state from the server's.

### 3.3 The feature is nevertheless known to upstream — ✅ verified

- `wacore-0.7.0/src/iq/abprops.rs` ships five server flags:

  | Constant | Wire name | Code | Type | Default |
  | --- | --- | --- | --- | --- |
  | `SCHEDULED_MESSAGES_SENDER_ENABLED` | `scheduled_messages_sender_enabled` | 23845 | bool | `false` |
  | `SCHEDULED_MESSAGES_RECEIVER_ENABLED` | `scheduled_messages_receiver_enabled` | 24610 | bool | `false` |
  | `SCHEDULED_MESSAGES_PHOTO_VIDEO_SENDER_ENABLED` | `scheduled_messages_photo_video_sender_enabled` | 32553 | bool | `false` |
  | `SCHEDULED_MESSAGES_WINDOW_DURATION_MIN_SECONDS` | `scheduled_messages_window_duration_min_seconds` | 26348 | int | `600` (10 min) |
  | `SCHEDULED_MESSAGES_WINDOW_DURATION_MAX_SECONDS` | `scheduled_messages_window_duration_max_seconds` | 26347 | int | `1209600` (14 days) |

  These are defaults; the server overrides them per account. `AbPropsCache::get(prop)` exists
  (`wacore-0.7.0/src/store/ab_props.rs:80`) but the client accessor is `pub(crate)`
  (`whatsapp-rust-0.7.0/src/client/iq_ops.rs:62`), so RustWA cannot read them through the public
  API today.
- `wacore-0.7.0/src/send/classify.rs:292` already treats `conditional_reveal_message` as an
  infrastructure message (`should_hide_decrypt_fail`), with a unit test at
  `wacore-0.7.0/src/send/tests.rs:2444`. There is no other scheduled-message logic anywhere in
  `whatsapp-rust`, `wacore`, or `wacore-appstate` ✅.
- MEX plumbing already exists: `handle_mex_notification`
  (`whatsapp-rust-0.7.0/src/handlers/notification/groups.rs:352`) parses
  `<notification type="mex"><update op_name="…">{json}</update></notification>` and dispatches
  `Event::MexNotification` (`wacore-0.7.0/src/types/events.rs:1017`) with the payload as
  `serde_json::Value`. `mex_operations.rs` has no scheduled ops ✅.

### 3.4 App-state mutation API a schema would plug into (for orientation)

`whatsapp-rust-0.7.0/src/features/chat_actions.rs:849`:

```rust
pub async fn send_app_state_action(
    &self,
    schema: &Schema,           // from whatsapp_rust::schemas
    index_args: &[&str],
    value: &wa::SyncActionValue,
) -> Result<(), AppStateError>
```

`send_app_state_mutation` (line 776) encodes one `Set` mutation with `encode_record`, then
`send_app_state_patch` builds the patch (`AppStateProcessor::build_patch`,
`wacore-0.7.0/src/appstate_sync.rs:478`) and sends it. If a scheduled-message action ever appears
in a future upstream schema, this is the API to use — but not before.

---

## 4. What the official app shows (26.33.73 binary evidence)

### 4.1 Symbols — ✅ verified

Extracted from `strings -a` of the main binary and `SharedModules`:

**Conditional reveal / crypto**

- `WAConditionalRevealMessagesShared34ConditionalRevealEncryptionManager`
- `ConditionalRevealEncryptionManager.encryptData:withRevealKey:wciContext:`
- `ConditionalRevealEncryptionManager.decryptData:withRevealKey:encIV:wciContext:`
- `ConditionalRevealMessageEncryptionOutput.initWithEncPayload:encIV:revealKey:`
- `revealKey must not be nil`, `encPayload must not be nil` (outgoing validation)
- `validateConditionalRevealMessage` / `isConditionalRevealMessageWithValidTypeWith:`
- `WAPBMessage_ConditionalRevealMessage` accessors: `setEncPayload:`, `setEncIv:`, `hasEncIv`,
  `setConditionalRevealMessageType:`, `setRevealKeyId:`
- `WAPBScheduledMessageMetadata` accessors: `setRevealKey:`, `setScheduledTime:`,
  `setScheduledMessageMetadata:`, `hasScheduledMessageMetadata`, `scheduledMessageMetadata`
- `encryptedRevealKey` / `encryptedRevealKeyID` (message-model properties)
- local SQLite table and indexes:
  `message_conditional_reveal (key_id, key_sender_jid)`,
  `message_conditional_reveal_key_index`; observers `registerMessageConditionalRevealFetchByKeyIdObserver…`,
  `registerMessageConditionalRevealFetchByMessageObserver…`, `fetchRevealKeyTupleFor:`
- message-pipeline hooks: `processScheduledMessageKeyRegistrationWithMessageInfo:to:`,
  `processConditionalRevealKeyRegistrationWithMessageInfo:to:`

**Stanza / delivery**

- `WCSInMessageDeliverMixinMetaScheduledMessage` —
  `"Attribute meta.type value is not \"scheduled_message\""` (message delivery validation)
- `WCSInMessageDeliverMixinDeliverLinkedDeviceScheduleMessage` — requires
  `Mixin MetaScheduledMessage not present`
- other mixins in the same family validate sibling attributes (`meta.appdata`,
  `meta.polltype`, `meta.event_type`, `meta.read`, `meta.is_group_status`, …), confirming the
  scheduled marker is a `<meta>` child node on the delivery stanza with `type="scheduled_message"`
- `ScheduledMessageStanzaConstants` (constants class; only the `type` value is visible in strings)
- `WAScheduledMessagePlaceholder` / `scheduled-placeholder` — local placeholder message

**Server notifications (Pando GraphQL models)**

- `NotificationScheduledMessagePost` + `NotificationScheduledMessagePostResponse` (`…Impl`,
  `…PandoImpl`, `…MinimalBuilder`, `…PandoBuilder`)
- `NotificationScheduledMessageReveal` + `…Response`
- operation names `xwa2_notify_scheduled_message_post`, `xwa2_notify_scheduled_message_reveal`

**Client state / UI**

- send state enum includes `PendingSend`, `PendingResponse`, **`SendScheduled`**
- `ScheduledMessageCreated` in the `StubType` enum list (matches proto enum 225)
- `ScheduledMessageSent` in the internal message-type enum list
- `WAScheduledMessageUpdated`, `WAScheduledMessageDeletion`, `scheduledMessageDeletionsByUUID`,
  `WAScheduledMessageSend`, `scheduledSendDate`, `canUnscheduleMessage`,
  `prepareForScheduledResend`, `firstScheduledTime`, `WAScheduledFallbackContext`
- UI strings: "Send later", "Press and hold to schedule a message", "Review scheduled messages",
  deletion/preservation banners, "Scheduled messages will not be sent…" group-admin variants
- AB-prop key strings for all five flags from [§3.3](#33-the-feature-is-nevertheless-known-to-upstream--verified)

### 4.2 The syncd registry in the official app — ✅ verified absent

The binary embeds the full action-name registry twice (one copy per architecture slice). The list
contains the same names as upstream (`pin_v1`, `markChatAsRead`, `setting_pushName`,
`wasa_root_secret`, `interactive_message_action`, `agentChatAssignment`, …) plus a few newer ones
(`lock_message`, `device_capabilities_v2`, `generated_wui`, `music_user_id`). None of the 58
entries contains "schedule". A case-insensitive scan of the entire string table for
`companion_scheduled`, `scheduled_message_sync`, or a schedule-flavored action name returns nothing
beyond analytics events (`scheduled_message_action`, `WamEventScheduledMessageAction`) and MEX
notification names. **The companion-scheduled feature does not sync through app state.**

### 4.3 What is still not visible in strings

- Which `<meta>` attribute carries the send time (`scheduled_time_ms` / `scheduled_time` appear in
  app-side model coding keys, not proven to be wire attributes).
- How `ConditionalRevealEncryptionManager` derives/encrypts (`wciContext`, possible reuse of the
  message-secret / `SecretEncryptedMessage` infrastructure seen next to it).
- The semantics of `processScheduledMessageKeyRegistrationWithMessageInfo:to:` — is the reveal key
  carried in `ScheduledMessageMetadata.revealKey` already, or registered through a separate
  message? (Both mechanisms appear in the binary.)
- The payloads of the two MEX notifications (fields are GraphQL response models, not wire dumps).
- Whether creation is an ordinary `<message>` stanza carrying `<meta type="scheduled_message">`, or
  a separate server call that produces the same notification. The mixin evidence points at the
  ordinary stanza, but the server must be *told* the time somehow.

---

## 5. Protocol model (best current understanding)

```
companion creates scheduled message
  │
  │  1. builds the real user Message (text/media)
  │  2. generates reveal key + key_id
  │  3. seals it:  ConditionalRevealMessage { enc_payload, enc_iv, type=SCHEDULED_MESSAGE,
  │                                            reveal_key_id }
  │  4. sends a normal E2E <message> whose payload is the sealed Message; stanza carries
  │     <meta type="scheduled_message" …> (time attribute unknown) and the scheduling metadata
  ▼
server stores the E2E payload + schedule (no re-encryption needed at send time)
  │
  │  5. server pushes xwa2_notify_scheduled_message_post to the account's other devices
  │     so their scheduled-message lists refresh (payload shape unknown)
  │
  │  6. at scheduledTime the server delivers the stored payload to recipients and pushes
  │     xwa2_notify_scheduled_message_reveal to the account's devices
  ▼
recipients / companion devices unwrap the sealed payload locally                (❓ unverified)
  • the local message_conditional_reveal table maps key_id(+sender) → key
  • content is only "revealed" when the client deems the condition met
```

Confidence per step: 1–3 🧪 (proto + crypto symbols), 4 ❓ (stanza shape), 5 ✅ (notification ops
exist), 6 ❓ (key delivery mechanism). Two competing hypotheses for step 6:

- **H1 — key in the metadata:** `ScheduledMessageMetadata.revealKey` (proto field 2) is populated
  at creation; every receiving client caches it per `revealKeyId`/sender and gates display on time.
  The server holds a sealed payload *and* the key, so confidentiality relies on the Signal layer,
  and the conditional envelope is mainly a UX/atomicity device.
- **H2 — key registered separately:** the reveal key is never given to the server; the posting
  device registers it with the account's other devices through a dedicated message
  (`processScheduledMessageKeyRegistrationWithMessageInfo:to:`), and recipients receive it at
  reveal time. The `encryptedRevealKey`/`encryptedRevealKeyID` model properties support this.

Do not implement either until the capture in [§8](#8-capture-experiments-that-close-the-unknowns)
decides between them.

---

## 6. The payload the client would have to send

### 6.1 App-state mutation — none exists

Per [§3.2](#32-syncd-action-registry--verified-absent): no schema = no mutation. The correct
app-state payload is **nothing**. Sending a guessed action would risk a local/server ltHash
divergence and a NACK.

### 6.2 Message stanza candidate (to be confirmed byte-for-byte by capture)

Wire-level view (names/levels verified in `whatsapp.proto`; attributes marked `?` are unknown):

```xml
<message id="3EB0…" to="1555…@s.whatsapp.net" type="text">
  <!-- server-readable scheduling marker; only `type` is verified: -->
  <meta type="scheduled_message" ???="<send-time attribute unknown>" />

  <!-- Signal-encrypted content; plaintext is a wa::Message: -->
  <enc …>
    Message {
      conditional_reveal_message: {
        enc_payload: <sealed bytes>,
        enc_iv:      <iv>,
        conditional_reveal_message_type: SCHEDULED_MESSAGE,
        reveal_key_id: "<key-id>",
      },
      // Possibly also here (H1), possibly only in the WebMessageInfo the server
      // returns to companions (H2):
      // scheduled_message_metadata: { reveal_key_id, reveal_key, scheduled_time }
    }
  </enc>
</message>
```

Rust sketch using the existing 0.7.0 surface:

```rust
use whatsapp_rust::waproto::whatsapp as wa;
use whatsapp_rust::{SendOptions, NodeBuilder};
use wa::message::{ConditionalRevealMessage, conditional_reveal_message::ConditionalRevealMessageType};

// NOTE: sealing algorithm + key derivation are unverified; this is the shape only.
let sealed = ConditionalRevealMessage {
    enc_payload: Some(ciphertext),
    enc_iv: Some(iv),
    conditional_reveal_message_type: Some(ConditionalRevealMessageType::ScheduledMessage),
    reveal_key_id: Some(key_id),
};

let payload = wa::Message {
    conditional_reveal_message: buffa::MessageField::some(sealed),
    ..Default::default()
};

let client = /* … */;
client
    .send_message_with_options(
        to,
        payload,
        SendOptions::default().with_extra_stanza_nodes(vec![
            NodeBuilder::new("meta").attr("type", "scheduled_message")/* + time attr (unknown) */ .build(),
        ]),
    )
    .await?;
```

Feasibility notes 🧪:

- `SendOptions::extra_stanza_nodes` is public and lets a consumer add the `<meta>` child without
  forking upstream (`whatsapp-rust-0.7.0/src/send/mod.rs:287`, re-exported in the crate root).
  Reserved child tags are only `enc`, `participants`, `device-identity`, `plaintext`
  (`send/mod.rs:240`), so `meta` is allowed.
- Caveat: upstream builds its own `<meta>` node via `infer_stanza_metadata` (`send/mod.rs:481`)
  for other features. A scheduled message does not currently trigger that path, so there is one
  `<meta>`; if upstream later adds scheduled inference, the two would need to merge. A first-class
  `SendOptions` knob (e.g. `schedule_at`) would be the cleaner home.
- `WebMessageInfo.scheduledMessageMetadata` is an *envelope* field. If H1 is right and the reveal
  key must ride in the `WebMessageInfo` rather than the E2E payload, the current public send API
  cannot express it and an upstream change is required. This is a key question for the capture.

### 6.3 What RustWA could still do today (different feature, honest label)

A pure client-side scheduler on top of `WaClient::send_text`
(`crates/whatsapp-core/src/client.rs:316`): persist `(chat_id, text, send_at)`, keep a tokio timer
in `whatsapp-core`, and send when due. It works only while the process runs and is connected, it
does not survive being offline at send time, and the official apps will not list it in their
scheduled-message UI. It shares the UX intent but not the protocol; it must not be presented as
companion scheduled messages.

---

## 7. Implementation plan (post-capture)

Ordered, assuming the capture confirms the stanza shape.

1. **Capture fixture** — anonymized protobuf/notification dumps for create, list refresh, unschedule,
   reveal, plus the time-attribute/value from the meta node. Store under `docs/research/fixtures/`
   (not in `src/`). This is the only artifact that unblocks everything.
2. **Sealing primitive** — port the conditional-reveal seal/open used by the official app once the
   algorithm is known (AES family + KDF from `wciContext`; test against a captured vector:
   `decrypt(captured enc_payload, captured reveal_key, captured enc_iv) == captured inner Message`).
3. **Key-store migration** — the official app keys reveal material by `(key_id, key_sender_jid)`
   (`message_conditional_reveal`). RustWA's SQLite `Store` would need an equivalent table plus a
   lookup used by the receive path.
4. **Feature module `crates/whatsapp-core/src/scheduled.rs`** behind a `scheduled-messages` feature
   (pattern: the `calls` feature in `crates/whatsapp-core/Cargo.toml:34`) exposing:
   - `schedule_message(chat_id, text, send_at_unix) -> Result<Message>` (store a pending row with
     status `Scheduled`, send the stanza, emit a `CoreEvent`),
   - `unschedule_message(message_id)`, `list_scheduled()`,
   - feature-detect via the sender/receiver AB props once upstream exposes `ab_props()` publicly.
5. **Receive path** — unwrap `conditional_reveal_message` when the condition is due; cache and
   expire reveal keys. Upstream currently only recognizes the wrapper for decrypt-fail hiding; the
   unwrap/reveal logic is ours.
6. **Notifications** — subscribe to `Event::MexNotification`, route
   `xwa2_notify_scheduled_message_post` / `_reveal` payloads to the module (raw JSON can be captured
   before typed decoding exists).
7. **Tests** — sealing vectors from the capture; store round-trip; a fake-server test asserting the
   outgoing stanza contains `<meta type="scheduled_message">` and the expected proto fields; AB-prop
   gating; clock skew vs `scheduledTime` units.

Exact mutation payload to implement: [§6.2](#62-message-stanza-candidate-to-be-confirmed-byte-for-byte-by-capture).
No syncd patch is involved at any stage.

---

## 8. Capture experiments that close the unknowns

All require one real account and a linked RustWA/patched client that can log traffic and decrypt its
own E2E payloads. Suggested order:

| # | Experiment | Resolves |
| --- | --- | --- |
| E1 | Link RustWA as a companion; on the phone schedule a text message ~10 min out. Log `Event::MexNotification` with `op_name` in {`xwa2_notify_scheduled_message_post`, `xwa2_notify_scheduled_message_reveal`}. | Payload schema of both notifications; whether other companions get a preview of the content or only ids. |
| E2 | While E1 runs, capture the incoming `<message>` echo (or the delivery to a second linked device) and dump the decrypted `wa::Message` + the `<meta>` child of the stanza. | `meta` attribute names carrying the send time; whether `ScheduledMessageMetadata` appears on the E2E payload, the envelope, or both; scheduledTime units. |
| E3 | From the patched client, replay the captured create stanza verbatim to a test chat (its own copy), then vary one field at a time: remove `<meta>`, wrong time attribute, unsealed payload, wrong `revealKeyId`. Observe server ACK/NACK and whether recipients get content immediately or at the time. | Minimum server acceptance criteria — the difference between "server holds it" and "just a normal message". |
| E4 | From the captured `enc_payload`/`enc_iv`/`reveal_key` tuple, test candidate decryptions (AES-CBC/CTR/GCM, key derivation candidates). Confirm with a second, independently captured tuple. | Sealing algorithm and key derivation; whether `revealKey` is encrypted (`encryptedRevealKey`) or raw. |
| E5 | Capture an unschedule/edit from the official app (the UI exposes it). | Whether cancellation is a revoke, a MEX mutation, a meta attribute, or a new message id; list-refresh semantics. |
| E6 | Schedule from the patched client to a chat with a controlled second account, keep the patched client offline at send time. | End-to-end proof that the server sends while the companion is offline (the actual acceptance test). |

Until E1–E6 exist, every `❓` in [§5](#5-protocol-model-best-current-understanding) stays open.
E1/E2 are cheap: `MexNotification` and raw-stanza logging already exist upstream.

---

## 9. Decision record

Task item 3 allowed `crates/whatsapp-core/src/scheduled.rs` only if (a) the upstream app-state
schema exists and (b) the mutation can be constructed without guessing.

- (a) fails ✅ (no schema in `wacore-appstate 0.7.0`, no action in the official app's registry).
- (b) fails independently: the create path is a message-stanza feature with at least three
  unknown wire elements (time attribute, sealing/KDF, key registration).

Therefore: **no `scheduled.rs` was written.** A speculative module would either be a local-only
scheduler under a misleading name, or protocol code that risks corrupted app-state hashes and
account-flagging on a real account. The plan above is the honest artifact.

---

## 10. Risk assessment

| Risk | Severity | Notes / mitigation |
| --- | --- | --- |
| **Account flagging / bans** | High | Feature is AB-gated (`scheduled_messages_sender_enabled` default false); official clients enforce window limits (default 10 min – 14 days). Synthetic stanzas from a third-party client are exactly the traffic pattern anti-abuse systems watch. Only test on a disposable account, with captured (not invented) wire data. |
| **App-state corruption** | High (if attempted) | `build_patch` updates the local ltHash and persists version + MACs; a guessed action name/version/index desyncs client state from the server. Mitigation: never write syncd actions without an upstream schema; currently there is nothing to write. |
| **Protocol drift** | Medium | The feature is young (proto fields and MEX ops are recent). Notification payloads and meta attributes can change without a client version bump. Mitigation: decode defensively (raw JSON), gate on AB props, keep the envelope schema versioned. |
| **Key-material mistakes** | High | Sealing with a wrong cipher/KDF or logging reveal keys would leak message content; storing keys unencrypted at rest is worse than the official app. Mitigation: port no crypto before E4 confirms it against captured vectors; never log key bytes; zeroize reveal keys; expiry (the app deletes expired conditional-reveal keys). |
| **Offline semantics overpromise** | Medium | A local timer cannot send while the process is offline. Do not ship it under the scheduled-messages name; document the difference. |
| **AB-prop unavailability** | Low | Flags are server-controlled and the public client API cannot read them today; feature must degrade gracefully and be exercised on an account where the sender flag is on. |
| **Receive-side regressions** | Medium | Introducing `conditional_reveal_message` unwrapping touches the shared message pipeline; upstream already treats the wrapper as infra for decrypt-fail hiding. Mitigation: keep unwrap logic local to the feature, add fixtures for sealed messages from the capture, never drop messages on unwrap failure (surface as placeholder). |

---

## 11. References

Upstream (registry root
`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`):

- `waproto-0.7.0/src/whatsapp.proto` — lines 2847 (`conditionalRevealMessage = 120`), 3094
  (`ConditionalRevealMessage`), 5197 (`ScheduledMessageMetadata`), 6432 (`WebMessageInfo`),
  6502 (`scheduledMessageMetadata = 81`), 6745 (`SCHEDULED_MESSAGE_CREATED = 225`)
- `wacore-appstate-0.7.0/src/schemas.rs` — generated syncd registry (66 actions, none scheduled)
- `wacore-0.7.0/src/iq/abprops.rs` — lines 7572–7599 (scheduled-message flags)
- `wacore-0.7.0/src/send/classify.rs:292`, `src/send/tests.rs:2444` — conditional-reveal infra
  handling
- `wacore-0.7.0/src/appstate_sync.rs:478` — `build_patch` (why guessed syncd actions are unsafe)
- `wacore-0.7.0/src/types/events.rs:1017` — `MexNotification`
- `whatsapp-rust-0.7.0/src/handlers/notification/groups.rs:352` — MEX notification parsing
- `whatsapp-rust-0.7.0/src/send/mod.rs` — `SendOptions` (282), `extra_stanza_nodes` (287),
  `infer_stanza_metadata` (481), `send_message_with_options` (901)
- `whatsapp-rust-0.7.0/src/features/chat_actions.rs:776,849` — `send_app_state_mutation`,
  `send_app_state_action`
- `whatsapp-rust-0.7.0/src/lib.rs:68` — `pub use wacore::appstate::schemas`

Official app (read-only strings): `/Applications/WhatsApp.app` 26.33.73 (1049819294) —
`Contents/MacOS/WhatsApp`, `Contents/Frameworks/SharedModules.framework/Versions/A/SharedModules`.
Key symbols listed in [§4.1](#41-symbols--verified).

This repo:

- `crates/whatsapp-core/src/client.rs:316` — current `send_text`
- `crates/whatsapp-core/Cargo.toml:34` — feature-gating pattern (`calls`)
- `docs/research/whatsapp-rust-api.md:756` — documented `SendOptions` surface
- `docs/parity-matrix.md:83,441` — scheduled messages marked as needing capture work
