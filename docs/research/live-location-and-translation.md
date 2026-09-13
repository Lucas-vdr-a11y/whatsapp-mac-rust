# M11 — live location and message translation: protocol research and implementation plan

Status: **research only, no protocol code shipped** (see [§7 Decision record](#7-decision-record)).
Scope: the two remaining M11 items
([`docs/parity-matrix.md:498`](../parity-matrix.md)):

1. **Live location** — can RustWA send, receive, and follow a continuously updating
   location share, and what does the official client actually put on the wire?
2. **Message translation** — how does the official macOS client translate messages, is any
   of it a wire feature, and what can RustWA build without Meta's private endpoints?

Confidence labels used throughout (same convention as
[`scheduled-messages.md`](./scheduled-messages.md)):

| Label | Meaning |
| --- | --- |
| ✅ verified | Read directly from source/proto/binary strings; quoted below. |
| 🧪 inferred | Follows from combining verified facts; no single source states it. |
| ❓ speculative | Hypothesis that needs a live capture or an upstream change; do not build on it yet. |

---

## 1. TL;DR

| Slice | Verdict |
| --- | --- |
| **Static location receive** (`LocationMessage`) | ✅ Feasible now. Upstream wacore parses and classifies it; RustWA's own `classify()` never maps it today, so it currently renders as `Unsupported`. Small, self-contained fix. |
| **Static location send** | ✅ Feasible now via the public `Client::send_message` with a `wa::Message.location_message`; upstream picks `type="media"` on the stanza and `mediatype="location"` on `<enc>` automatically (`wacore/src/send/dm.rs:192`, `group.rs:531-539`). |
| **Live location first frame send** (`liveLocationMessage`) | 🟡 Proto and send path exist; **the share duration/end-date encoding is not in the proto and is not verified**. Capture-gated (experiment E2). |
| **Live location updates — receive** | ❌ **Not feasible with the current stack.** The official client encrypts updates with a dedicated *location key* distributed by a dedicated KDM, decryptable only by a **fast-ratchet** sender-key implementation. `wacore-libsignal 0.7.0` implements only standard sender keys; unrecognized `<enc type>` values (e.g. `frskmsg`) are dropped. |
| **Live location updates — send (best effort)** | ❓ Speculative and capture-gated. Re-sending `liveLocationMessage` frames as ordinary messages *may* be merged by official receivers by sequence number, but it is not the official update channel (no location key, no server session, no `finalLiveLocation`). |
| **Full live session** (duration, expiry, stop-sharing, final location, late-join) | ❌ Not feasible now. The session is server-mediated (IQ subscribe/unsubscribe + notifications), not expressible through the message pipeline. |
| **Accepting a share** | Local only. There is no accept/ack message; the receiver just decrypts the first frame and (officially) subscribes via IQ for updates. |
| **Ending a share** | **Not revoke/edit semantics.** The official sender sends a *final location* frame and stops; a server notification (`…/sharing/from=…/disable`) tells subscribers. There is no live-location revoke in `ProtocolMessage.Type`. |
| **Message translation protocol** | ✅ **Nothing to implement.** Translation is local-only in the official client: Apple `Translation.framework` (on-device) + local caching. No proto fields, no syncd action, no MEX op, no notification. |
| **Message translation product** | 🟡 Implementable **on-device only**. macOS 15+ Apple Translation (highest parity, needs a small Swift shim) or a local NMT model later. Cloud translation contradicts the end-to-end-encryption promise and must not be a default. |

Two sentences: live location is a *blocked-upstream* item (decryption capability), while
translation is a *no-protocol* item (local UX with a platform dependency). Neither should be
promised as a protocol feature.

---

## 2. Method and evidence sources

| Source | Version / path | Used for |
| --- | --- | --- |
| Upstream `whatsapp-rust` | crates.io 0.7.0, `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/whatsapp-rust-0.7.0` | send/edit/receive surfaces, `SendOptions`, `EncHandler`, message-edit crypto |
| Upstream `wacore` | 0.7.0, same registry root | classification, user-content detection, reporting tokens, fast-ratchet carriers |
| Upstream `wacore-appstate` | 0.7.0, same registry root | syncd registry (66 actions) |
| Upstream `wacore-libsignal` | 0.7.0, same registry root | sender-key/group cipher capabilities |
| Upstream `waproto` | 0.7.0, same registry root | `whatsapp.proto` message shapes |
| Official app | `/Applications/WhatsApp.app` 26.33.73 (build 1049819294), macOS, read-only `strings -a`, `otool -L`, `nm -u`, `PlistBuddy` | feature markers, framework usage, wire/stanza names |
| WhatsApp Web proto (current) | `whatspec` IR, `generated/proto/WAProto.proto` @ `oxidezap/whatspec` main | confirm the live proto has no live-location/translation additions |
| WhatsApp Web IQ IR | `whatspec` IR, `generated/iq/index.json` (143 modeled stanzas, 30 namespaces) | confirm no modeled `location` IQ namespace (coverage gap noted) |
| External OSS | Baileys `master` (messages-send.ts), Baileys issue #1026, `wppconnect-team/wa-js` main, whatsmeow `main` (protos only) | cross-check update-channel behavior; no OSS stack implements it |
| RustWA repo | this repository | current client/UI surface, store schema, docs context |

Binary extraction was read-only (`strings`/`nm`/`otool` into a temp file, then `grep`). No app
modification, no network traffic, no account use.

**Caveat on citations:** all `*.strings` line numbers below are line numbers of the
`strings -a` dump of the named binary (two architecture slices mean most strings appear twice;
the second copy is the same content at a higher line number). Only the first occurrence is
cited. For the official app, `shared.strings` = `SharedModules`; `main.strings` =
`Contents/MacOS/WhatsApp`.

---

## 3. Live location

### 3.1 Proto surface — ✅ verified

`waproto-0.7.0/src/whatsapp.proto`:

```proto
// Message (starts line 2747), field 18:
optional LiveLocationMessage liveLocationMessage = 18;      // line 2764

// nested in Message (line 3650):
message LiveLocationMessage {
  optional double degreesLatitude = 1;
  optional double degreesLongitude = 2;
  optional uint32 accuracyInMeters = 3;
  optional float speedInMps = 4;
  optional uint32 degreesClockwiseFromMagneticNorth = 5;
  optional string caption = 6;
  optional int64 sequenceNumber = 7;     // update ordering
  optional uint32 timeOffset = 8;        // "elapsed" seconds since share start
  optional bytes jpegThumbnail = 16;
  optional ContextInfo contextInfo = 17;
}

// WebMessageInfo (line 6432), field 30 — server envelope, not E2E:
optional Message.LiveLocationMessage finalLiveLocation = 30;  // line 6453

// WebFeatures (client capability flags), lines 6381 / 6387:
optional Flag liveLocations = 7;
optional Flag liveLocationsFinal = 13;
```

Notes:

- There is **no duration/end-date/expiry field** on `LiveLocationMessage` (see §3.5).
- `LocationMessage` (line 3663) has an `isLive` flag (field 6), a legacy pre-`liveLocationMessage`
  marker; modern clients use `liveLocationMessage` ✅.
- A separate `ConsumerApplication.LiveLocationMessage` exists for the consumer/wearables
  envelope (`whatsapp.proto:1449,1502`): same data, different wrapper ✅.
- The **current WhatsApp Web proto** (`whatspec` `WAProto.proto` @ 2026) declares the identical
  `LiveLocationMessage` field set at the same field numbers ✅ — the wire shape has not changed.

### 3.2 Upstream code support — ✅ verified

| Capability | Where | Status |
| --- | --- | --- |
| Classify stanza payload | `wacore-0.7.0/src/send/classify.rs:199-210` | `<enc mediatype="location">` for a static location, `"livelocation"` for `liveLocationMessage` (and for `location_message.is_live == true`). |
| Stanza type | `wacore-0.7.0/src/send/classify.rs` (`stanza_type_from_message`) | Falls through to `type="media"` — the same shape Baileys sends (`Baileys-master/src/Socket/messages-send.ts:1195`). ✅ |
| User-content detection | `wacore-0.7.0/src/messages.rs:1008-1035` | `live_location_message` counts as user content (not SKDM-only), so it is surfaced. ✅ |
| `contextInfo` helpers | `wacore-0.7.0/src/proto_helpers.rs:7-33,450-500` | `live_location_message` is in the context-info list (e.g. ephemeral expiration works). ✅ |
| History-sync tags | `wacore-0.7.0/src/history_sync.rs:991,1124` | `live_location_message::CONTEXT_INFO == 17`, carrier slot 7. ✅ |
| Reporting token | `wacore-0.7.0/src/reporting_token.rs:163-167` | Whitelisted subfields `6` (caption), `16`, `17` (contextInfo). ⚠️ The `// comment` label on field 16 is **wrong** in upstream (proto line 3650: field 16 is `jpegThumbnail`); flagged only as an upstream annotation bug, no action. |
| Send arbitrary payload | `whatsapp-rust-0.7.0/src/send/mod.rs:846,901` | `send_message(to, wa::Message)` / `send_message_with_options(..., SendOptions)` accept any `Message`, so a `liveLocationMessage` is sendable. There is **no dedicated location builder** — the proto struct is the whole surface. ✅ |
| Edit arbitrary payload | `whatsapp-rust-0.7.0/src/client/messaging.rs:152,260` | `edit_message` / `edit_message_encrypted` accept any `new_content: wa::Message`. Available if a future capture proves the official update path is an edit. ✅ |
| Custom enc types | `whatsapp-rust-0.7.0/src/bot.rs:1135` (`with_enc_handler`), `types/enc_handler.rs:16` | Extension point for `"frskmsg"`-style unknown enc types; wacore counts them in `unknown_enc_types` (`wacore-0.7.0/src/message_processing.rs:180`) and otherwise drops them. |
| **Fast ratchet** | `wacore-libsignal-0.7.0/src/protocol/` | ❌ **Absent.** Only standard `SenderKeyMessage`/`group_cipher`. `fastRatchetKeySenderKeyDistributionMessage` (proto field 15) is recognized only as an SKDM carrier (`messages.rs:837,1011,1455`). |

### 3.3 App-state — ✅ verified absent

`wacore-appstate-0.7.0/src/schemas.rs` (generated from WhatsApp 2.3000.1042742319, 66 actions,
`ALL` at lines 1391-1458) has **no** live-location action: case-insensitive scans for `live`,
`location`, and `loc` return nothing. The official app's own syncd name registry (read from
`SharedModules` strings next to `wasa_root_secret`) likewise contains no location action.
Conclusion 🧪: the share is a message-layer + server-session feature, **not** syncd. Writing a
syncd mutation would be wrong.

### 3.4 RustWA's current state (gap analysis) — ✅ verified

Despite [`docs/parity-matrix.md:117`](../parity-matrix.md) marking "Location (static)" as ✅
(meaning: proto + upstream parse exists) and "Live location" as 🟡, **RustWA itself handles
neither**:

- `crates/whatsapp-core/src/types.rs:128-129` — `MessageKind::Location` exists
  ("Location or live location") but nothing ever produces it (only `kind_to_str` /
  `kind_from_str` in `store.rs:782,800`).
- `crates/whatsapp-core/src/client.rs:1165-1193` (`classify()`) — maps text/image/video/audio/
  document/sticker only; `location_message` and `live_location_message` fall to
  `MessageKind::Unsupported`.
- `crates/whatsapp-core/src/client.rs:714-798` (`handle_inbound_message`) — persists the row and
  the **raw proto** (`store.set_raw_proto`, `client.rs:788`), so coordinates survive for a
  later feature, but `Message` (IPC type, `types.rs:161-184`) has no coordinate fields.
- `crates/whatsapp-core/src/client.rs:1111-1143` (`convert_history_message`) — same gap for
  history sync.
- UI: `apps/desktop/src/lib/types.ts:21-43` mirrors the kind; `media.ts:139` already has a
  `"location"` label; there is no map/coordinate component and no `LocationContent.tsx`
  (`apps/desktop/src/components/message/`), so the bubble shows "Unsupported".
- No geolocation code anywhere in `crates/` or `apps/desktop/src` (grep for
  `latitude|longitude|geolocation` returns nothing), no geolocation plugin in
  `apps/desktop/package.json`, and `apps/desktop/src-tauri/Info.plist` only declares
  `NSMicrophoneUsageDescription` — a location share needs `NSLocationWhenInUseUsageDescription`
  before macOS will grant coordinates.

### 3.5 Official app evidence — ✅ verified (feature archaeology)

The macOS client ships the full feature. Read-only string evidence:

**Sender**

- `WALiveLocationMessageSender` (`main.strings:1269716`), `WALiveLocationUpdate`, and the
  sender callback `v56@0:8@"WALiveLocationUpdate"16@"WAUserJID"24@"WAChatJID"32d40@?<v@?@"NSError"@"WAPBMessage">48`
  (`main.strings:1269687`) show updates are produced as `WAPBMessage` values.
- Lifecycle logs: `live-location//startSendingLocationUpdatesForDuration %@`
  (`main.strings:1269487`), `live-location//stopSendingLocationUpdates`
  (`:1269488`), `live-location//send-final-location/%@/%@` (`:1269500`),
  `live-location//maybe-send-final-location/%@/%@`, `…/reallySendLiveLocationUntil:%@ chatJID:%@`.
- `walivelocationmessagesender//Sending message immediately` / `//Storing modified message`
  (`main.strings:1269704-1269705`) — the sender mutates and stores the local model.
- `WALiveLocationShareSettings`, duration picker event `WamEventLiveLocationDurationPicker`
  (`shared.strings:529183`), caption support.

**Receiver / ordering / expiry**

- `live-location//Did receive chat with location message. from=%@ end date=%@ sequenceNumber=%@`
  (`main.strings:1269507`).
- `live-location//Live location message with sequence number %@ rejected. Set end date in message.`
  (`main.strings:1269508`) — sequence numbers are gated per sender and an end date is required.
- `location-storage//Set end date for %@ to %@. Message key is %@. Timestamp is %@.`
  (`main.strings:1270511`), `location-storage//Sequence number %@ not greater than existing
  sequence number %@ for %@` (`:1270513`), `WALocationStorageMetadataSequenceNumber`
  (`main.strings:1270529`), and the UI/property names `liveLocationSequenceNumber`,
  `liveLocationEndDate`, `liveLocationCaption`, `liveLocationFinalLocation`
  (`shared.strings:550758-550761`). Place sending with a server end date:
  `sendMessageContainingPlaceWithPlace:liveLocationEndServerDate:controller:completion:`
  (`main.strings:1646403`).
- `WALocationStorage` maintains per-`(chat, sender)` state and posts
  `WALocationStorageSharingDidExpireNotification` / `DidChangeSenderEndDateNotification`
  (`main.strings:1270536-1270537`).

**The update channel is a dedicated key, not a plain message** — ✅

- `locationKeyDistributionStanzaForPayload:retryCount:error:` (`shared.strings:769489`) — a
  dedicated **location key** is distributed by its own stanza, with retry.
- `encInStanzaWithLiveLocation:` (`main.strings:1530486`) — builds an `<enc>` for a live
  location payload.
- `XMPPLiveLocationEnc` (`shared.strings:538262`) and
  `decryptRequestFromLiveLocationEncElement:performProtobufValidation:`
  (`main.strings:1530156`) — the update `<enc>` element has a dedicated decrypt path.
- `LiveLocationDecryption` (`main.strings:1269774`), `WAXLiveLocationIncomingStanzaFactory`
  (`shared.strings:573041`), `WCSInMessageDeliverMixinEncLiveLocation` with
  `"Attribute mediatype value is not \"livelocation\""` (`shared.strings:590340,590350`).
- Retry/KDM logs: `live-location//location-retry/add/not sending retry kdm because not
  connected`, `…/location-retry/didEncrypt/Missing intended recipient in fanout`,
  `…/request-retry Send to %@ with count %@`.
- **Fast ratchet**: the app contains a full fast-ratchet Signal implementation:
  `signal/coordinator/fastratchet//decrypt/skmsg/…` (`shared.strings:534420-534427`),
  `FastRatchetSenderKeyStateStructure` / `…RecordStructure` / `…DistributionMessage`
  (`shared.strings:587605,587608,587662`), `isFastRatchetGroupCipher` (`:680335`), and
  `fast_ratchet_future_message_limit` (`shared.strings:587677`). The proto field
  `fastRatchetKeySenderKeyDistributionMessage` (field 15) is the corresponding KDM carrier
  (`shared.strings:580469`). None of this exists in `wacore-libsignal 0.7.0` ✅.

**Session / subscription / server state** — ✅

- IQ logs: `connection/group/location-sharing/subscribe/%@/participants=%d [Subscribe to
  Location Updates (Client-to-Server)]` (`main.strings:1270360`), `…/stop/%@ [Unsubscribe from
  Location Updates (Client-to-Server)]` (`:1270364`), and a server push
  `connection/location/sharing/from=%@/disable [sequenceNumber=%@] [User Disabled Location
  Sharing (Server-to-Client)]` (`:1269650`).
- API: `subscribeToLocationUpdatesFromChatJID:reportParticipants:includeMessageInParticipantInfo:completion:`
  (`main.strings:1530280`), `unsubscribeFromLocationUpdatesFromChatJID:webClientRequestID:completion:`,
  `processIncomingLocationIQ:`, `errorLocationIQForIQ:`,
  `startLocationReportingWithChatJID:incomingIQStanza:duration:` (`:1596046`),
  `handleStopLocationReportingForIncomingIQStanza:`.
- `WAXOutgoingStanzaLiveLocationSubscribeToUpdatesRequest` (`main.strings:1270373`) plus
  error mixins `WAXOutgoingStanzaLiveLocationMixinIQErrorBadRequest/…NotAuthorized/…`
  (`shared.strings:573074-573085`) — a typed outbound IQ for the subscription.
- GraphQL model `WBQLocationQueryFinalLiveLocationExpirationDates` (`shared.strings:523794`)
  and `fetchFinalLiveLocationExpirationDates` (`shared.strings:757333`) — the server tracks
  final-location expiry.
- `finalLiveLocation` in the E2E proto is a `WebMessageInfo` field (`whatsapp.proto:6453`), i.e.
  the **server envelope**, not something a companion builds into the E2E payload ✅.

Cross-check: Baileys issue #1026 (external, 🧪 for our purposes) shows the subscription as
`<iq xmlns="location" type="get" to="s.whatsapp.net" target="…"><subscribe participants="true"/></iq>`
with a ~30 s refresh cadence, and reports that delivered updates arrive in an encryption class
the library cannot decrypt (`fskmsg` in the report; the app's own strings say fast-ratchet
`skmsg`). whatsmeow ships live-location **protos only** (no session logic). No OSS stack
implements the update receive path. The `whatspec` IQ IR does not model a `location` namespace,
so it could not confirm the exact IQ tags — mark the IQ shape ❓ until captured (E4).

### 3.6 Protocol model (best current understanding)

```
Share start (sender)
  │
  │  1. build Message { liveLocationMessage { lat, lng, accuracy, speed,
  │     degrees, caption, sequenceNumber = 0/1, timeOffset = 0 } }
  │  2. wrap in <message type="media"><enc type="msg|skmsg"
  │     mediatype="livelocation"> (per the classifier / app mixins)
  ▼
recipients: normal E2E message; RustWA can decrypt/render this today ✔ (once classify() maps it)
  │
  │  3. sender creates a per-share "location key" and distributes it
  │     via locationKeyDistributionStanzaForPayload (KDM, with retry)
  │  4. periodic updates encrypted under that key (fast-ratchet skmsg family,
  │     sequenceNumber/timeOffset incremented) ─► server relays to subscribers
  │  5. late/companion devices subscribe:
  │     <iq xmlns="location" type="get" target=chat><subscribe participants="true"/></iq>
  │     (exact tags ❓, refresh ≈30 s per Baileys)
  │  6. stop: final location frame + "Send request to stop sharing" +
  │     server "sharing disabled" notification; no revoke
  ▼
receiving updates requires fast-ratchet decryption  ✘ (not in wacore-libsignal 0.7.0)
```

Confidence: the proto shape and the sender/receiver split are ✅; the end-date encoding, the
IQ tag names, and the KDM wire shape are ❓.

### 3.7 Accepting / ending — edit/revoke semantics

- **Accepting a share is not a wire action.** The first frame is an ordinary message; nothing
  is sent back. Officially the client subscribes for updates when the share is active
  (IQ above), which is the only "receive-side" traffic.
- **Updates are not edits and not new UUIDs**: each update is its own encrypted frame carrying
  an increasing `sequenceNumber` and `timeOffset`, and the receiver merges frames into the
  already-displayed share by keeping a per-`(chat, sender)` "live location message unique key"
  cache (`main.strings:1269548-1269550`, `1530114`, `1530194`, `LastLiveLocationMessageUniqueKey`).
  There is **no** `ProtocolMessage.Type` variant for live locations (the enum at
  `whatsapp.proto:4188-4218` has `REVOKE`, `MESSAGE_EDIT`, `MESSAGE_UNSCHEDULE`, … — nothing
  location related) ✅.
- **Ending is a final location + session stop**, not a revoke: `send-final-location`,
  `Send request to stop sharing with %@`, the server push `…/sharing/from=…/disable`, and
  (server-side) `finalLiveLocation` + `expirationDate` on `WebMessageInfo` so late joiners can
  see the last position. Revoking the original message is not part of the flow (no string
  evidence ties revoke to live location) 🧪.
- **Implication for RustWA:** "Stop sharing" can always be implemented locally (stop the timer,
  optionally send a final frame); making the *server* retire the session is not expressible in
  the current public API ❓.

### 3.8 Capture experiments that close the unknowns

One real (disposable) account, one patched RustWA or an instrumented log of
`Event::Messages`, `Event::MexNotification` and raw stanza I/O.

| # | Experiment | Resolves |
| --- | --- | --- |
| E1 | Receive a live location from the official app (1:1 and group). Dump the decrypted `wa::Message` + stanza children + `mediatype`. | Initial frame shape; whether the end date/duration rides in `contextInfo.expiration` or another field; whether the first frame id is reused. |
| E2 | Dump the *second* and *final* frames (update + final location). Note stanza ids, `<enc type>` (`msg`/`skmsg`/`frskmsg`?), sequence numbers, retry KDMs. | Whether updates reuse ids, the actual enc type, KDM cadence, final-frame shape. |
| E3 | Capture the subscribe/unsubscribe IQ and the `sharing/…/disable` push (raw XML). | Exact IQ namespace/tags/attrs, lease duration, who may subscribe. |
| E4 | From a patched client, replay `liveLocationMessage` frames as ordinary messages (new ids, increasing sequence) to a test account and observe official clients + `finalLiveLocation`/expiry behavior. | Whether a best-effort update sender works at all; the minimum server acceptance criteria. |
| E5 | Link RustWA as companion and watch `Event` traffic while a phone shares. | Which parts arrive as messages vs. IQ pushes; decrypt failure modes on `frskmsg`. |

Until E1–E3 exist, no update-session code can be written honestly.

---

## 4. Message translation

### 4.1 Official client: on-device Apple Translation — ✅ verified

Binary-level evidence:

| Evidence | Detail |
| --- | --- |
| Framework link | `otool -L Contents/MacOS/WhatsApp` lists `/System/Library/Frameworks/Translation.framework` (compatibility 365.11.0) and `_Translation_SwiftUI`, both **weak**-linked (the app's `LSMinimumSystemVersion` is 12.1, so translation is runtime-gated, almost certainly `#available(macOS 15, *)`). |
| API usage | `nm -u` shows undefined symbols for `Translation.TranslationSession` (init `installedSource:target:`, `prepareTranslation()`, `translate(_:)`, `Configuration(source:target:)`), `Translation.LanguageAvailability.status(from:to:)`, and SwiftUI's `translationTask(_:action:)`. The app calls the public on-device API — not a Meta private endpoint. |
| App module | `WATranslation` Swift module: `_TtC13WATranslation22AutoTranslationHandler`, `…27MessageTranslationPresenter`, `…24MessageTranslationLogger`, `WATranslation/LanguageSelector.swift` (`main.strings:1437006-1437010`), plus plugins `WATranslationPlugin`, `MessageTranslationPresentingPlugin`, `TranslationContextMenuActionPlugin`, `AutoTranslationHandlingPlugin` (`main.strings:1461164-1461167`). |
| Local caching | `TextTranslationMessageMetadata` with `translated_text_dialect` / `translated_text` (`shared.strings:631530,631809`), `WAPBMediaItemMetadata_TranslationMetadata` + `…_TranslationStatus` (`shared.strings:581559,581589`; property names `translationMetadata`, `translationStatus` at `shared.strings:581734,581934`) — translations are cached as **local message metadata**, never as wire fields. |
| Per-chat preference | `WAPBChatSessionMetadata` (`shared.strings:546900`) carrying `WAPBTranslation` (`shared.strings:583205`) with `autoTranslateEnabled` (`shared.strings:583349`) and `TranslationPreferences` (`main.strings:1437015`) — local store/UserDefaults, not app state. |
| UI copy | "Translate all new messages in this chat" (`main.strings:1436968`), "Translate from/to" (`:1436970,1436972`), "Translation options" (`:1436986`), "Translated message" (`:1436997`), "Translate %1$@ to %2$@" (`:1360966`), **"Updates are translated on your device. Translations may not always be accurate."** (`:1436992`), **"Your personal messages remain end-to-end encrypted. No one outside of this chat, not even WhatsApp, can read them. Translations may not always be accurate."** (`:1436990`). |
| Language-pack download | `TRANSLATION_OFFLINE_MODEL_DOWNLOAD_ERROR_TEXT` → "Connect to the internet to download the language needed for translation." (`main.strings:1437001`). Exactly Apple's `TranslationSession.prepareTranslation()` behavior. |
| Gating | App-side flags `wa_ios_message_translation_enabled` (`main.strings:1545195`), `ios_message_translation_m2_enabled` (`:1545194`), `auto_message_translation_enabled` (`:1594604`), `is_translation_eligible` (`:1389674`). None of these exist in `wacore`'s extracted WA-Web AB registry, and no MEX/GraphQL translation operation exists either. |
| Meta AI path (separate, server-side) | `WAAIViews.AITranslationView`, `TranslateSuggestionFetcher` (`main.strings:1384541,1385375`), system-prompt text ("Translate the message below…") — that is the Meta AI assistant feature, not the translation button. It uses Meta's AI backend and is out of scope. |

### 4.2 What is *not* there — the wire is untouched — ✅ verified

- **Protos**: current WhatsApp Web proto (`whatspec` `WAProto.proto`, 7,726 lines) contains no
  translation message/field. The only `translat*` match is
  `InThreadSurveyOption.textTranslated` (`whatsapp.proto:2534`), an in-thread-survey UI string
  unrelated to message translation. The same is true of the shipped `waproto-0.7.0`.
- **`ContextInfo`**: no translation/source-language fields (`whatsapp.proto:1624-…`; only
  `expiration`, forwarding, quotes, etc.).
- **App state**: the official registry has no translation action (§3.3 method; `wacore-appstate
  0.7.0` has none); per-chat auto-translate lives in the local chat-session metadata.
- **MEX/IQ/notifications**: no translation operations in `wacore/src/iq/mex_operations.rs` or
  AB props, and no notification strings.
- Therefore 🧪: translating a message changes **nothing** on the wire. The peer never learns a
  translation happened; the original ciphertext is untouched. This is the privacy property the
  UI copy promises.

### 4.3 Reachability without Meta's private endpoints

| Option | Feasible | Privacy | Notes |
| --- | --- | --- | --- |
| **Apple Translation framework (macOS 15+)** | ✅ Highest parity | ✅ On-device; only language packs come from Apple's servers | Swift-only API (no ObjC exposure). Needs a tiny Swift shim compiled into the Tauri app and linked via a C ABI; `nm` proves the same calls the official app makes. Status API (`LanguageAvailability`) maps cleanly onto the official "download language needed" error copy. |
| **Local NMT model** (e.g. Bergamot/`bergamot-translator`, CTranslate2 + NLLB/OPUS-MT, `candle`) | 🟡 Feasible but heavy | ✅ On-device | Cross-platform; model download per language pair (tens to hundreds of MB), extra CPU/GPU + memory, quality varies. Best as an opt-in desktop feature later. |
| **OS APIs on other platforms** | ⚠️ Partial | ✅ | Windows has no equivalent OS translation API; Linux none. Option 1 only covers macOS, so a cross-platform client must choose "not available" or a local model. |
| **Cloud APIs / LLM (DeepL, Google, Meta AI)** | ⚠️ Technically easy | ❌ Contradicts the E2EE promise | Sending message plaintext to a third party is exactly what the feature's own disclosure says does not happen. If ever offered it must be explicit opt-in, never auto, with a per-chat warning; do not present it as the same feature. |
| **Sender-side translation in the message payload** | ❌ Not a thing | — | No proto field exists; do not invent one. |

Implementation risk to flag: Apple Translation needs language packs installed; the UI must
handle `LanguageAvailability.Status` (`installed`, `supported`, `unsupported`) and the
download flow. On macOS < 15 the feature must be hidden or disabled.

---

## 5. Slice plan

Everything below is additive; nothing touches the crypto or app-state paths. Slices are ordered
by risk: the two location slices are safely implementable today; the rest are gated on captures
or upstream capability.

### 5.1 L1 — receive and render static + live-location first frames (safe, ~2–3 days)

1. `crates/whatsapp-core/src/types.rs`
   - Keep `MessageKind::Location`; add `#[serde(default)] pub location: Option<LocationInfo>` to
     `Message` (camelCase over IPC). `LocationInfo { latitude, longitude, accuracy_meters,
     speed_mps, degrees, caption, sequence_number, time_offset_seconds, is_live, expires_at }`.
     All optional/default so old rows and old UI builds keep working.
   - Adding a new `MessageKind` variant is *not* needed: use `Location` + `is_live`.
2. `crates/whatsapp-core/src/schema/008_location.sql` + `store.rs` (`SCHEMA_VERSION = 7 → 8`,
   migration arm next to `007`, `store.rs:21,60-160`) — either a JSON `payload` column on
   `messages` or explicit nullable columns. `raw_proto` already holds the source of truth, so a
   single `location_json TEXT` column is enough.
3. `crates/whatsapp-core/src/store.rs` — read/write the new column in `upsert_message` /
   `find_message`; no change to `kind_to_str`/`kind_from_str` (kind stays `"location"`).
4. `crates/whatsapp-core/src/client.rs`
   - `classify()` (`:1165`): after `sticker_message`, map `base.location_message` and
     `base.live_location_message` to `MessageKind::Location`.
   - `handle_inbound_message` (`:714`) and `convert_history_message` (`:1111`): extract the
     `LocationInfo` (unwrap `location_message`/`live_location_message`, read
     `context_info.expiration` opportunistically for a provisional expiry ❓ until E1/E2).
5. `apps/desktop/src/lib/types.ts` — mirror `location` + `LocationInfo`.
6. `apps/desktop/src-tauri/src/commands_media.rs` (or a new
   `commands_location.rs` registered in `lib.rs:235-336`) — `media_location` / `message_location`
   read command only if the UI needs it beyond the `Message` payload (not required; the message
   already carries it).
7. `apps/desktop/src/components/message/LocationContent.tsx` (new) + `MessageBubble.tsx` wiring
   + `media.ts` preview label. First version: coordinates + "Open in Maps"
   (`https://maps.apple.com/?ll=…` via the opener plugin / `shell.open`) and a "Live location"
   badge when `is_live`. No map tiles yet (tile fetches leak the location to a third-party tile
   server; if added, make it opt-in and document the leak).
8. Tests: unit tests for `classify` and the conversion of both proto shapes; a store round-trip
   test at schema v8; a fixture with a live-location message.

### 5.2 L2 — send static location (safe, ~1–2 days + picker)

1. `crates/whatsapp-core/src/location.rs` (new; or fold into `actions.rs`):
   `send_location(chat_id, latitude, longitude, name, address) -> Result<()>` building
   `wa::Message { location_message: … }` and calling `client.send_message` — upstream already
   emits `type="media"`/`mediatype="location"` (`wacore` classifier). Persist a local echo row.
2. macOS coordinate acquisition: `apps/desktop/src-tauri/Cargo.toml`
   `[target.'cfg(target_os = "macos")'.dependencies] objc2-core-location = "0.3"` (pattern:
   the existing `objc2-*` block at `Cargo.toml:46-51`) + `Info.plist`
   `NSLocationWhenInUseUsageDescription`. Non-macOS targets return "unsupported".
3. Tauri command + UI: `commands_location.rs` → `location_get_current`, `location_send`;
   a small picker/dialog in the composer (composer dir:
   `apps/desktop/src/components/composer/`). i18n strings added next to the existing
   `media.location` key.

### 5.3 L3 — one-shot live-location share (capture-gated, do not ship before E1/E2)

- Add `send_live_location(chat_id, lat, lng, duration_secs, caption)` that sends one
  `liveLocationMessage` frame with `sequenceNumber = 0`, `timeOffset = 0`, and the duration in
  **whichever field E1/E2 proves** (candidate: `contextInfo.expiration`; proto has no duration
  field — ❓). Behind an experimental flag, with a visible sharing indicator and a
  hard client-side stop.
- If E4 shows official clients merge plain frames, extend to a timer that sends
  `liveLocationMessage` frames with incrementing `sequenceNumber`/`timeOffset` and a final
  frame (`timeOffset` = elapsed at stop). Do **not** attempt to replicate
  `locationKeyDistributionStanzaForPayload` or fast-ratchet KDMs by hand.
- If official receivers reject plain frames without a location key/KDM (plausible), stop here:
  the honest deliverable is "detect and render live shares, but RustWA cannot participate as a
  live sharer".

### 5.4 L4 — full live session / update receive: **not feasible — do not build**

Blocked on upstream `wacore-libsignal` gaining fast-ratchet sender keys (state structures,
KDM processing, `skmsg` decrypt) plus a location-key store and the IQ subscription loop. The
custom `EncHandler` hook (`bot.rs:1135`) is the eventual integration point; implementing the
cipher ourselves without captures would be unsafe and unverifiable. Track as an upstream
prerequisite, not a RustWA task.

### 5.5 T1 — translation (on-device only; no protocol work)

1. `crates/whatsapp-core/src/translate.rs` (new, behind a `translation` feature mirroring
   `crates/whatsapp-core/Cargo.toml:47`):
   ```rust
   pub struct TranslationRequest { pub message_id: String, pub text: String,
                                   pub source: Option<String>, pub target: String }
   pub struct TranslationResult { pub translated_text: String, pub source: Option<String>,
                                  pub target: String, pub on_device: bool }
   pub enum TranslationAvailability { Installed, NeedsDownload, Unsupported }
   pub trait Translator { /* async translate / availability / prepare */ }
   ```
   The trait lives in core so the UI never talks to a backend directly. **No trait method ever
   sends message text over the network by default.** Auto-translate preference is per chat and
   stored locally (new column/table in the schema — can ride with L1's migration or its own),
   matching the official client's local `WAPBChatSessionMetadata.translation`.
2. macOS backend (highest parity): `apps/desktop/src-tauri/swift/TranslationBridge.swift`
   (new) exposing a C ABI (`rustwa_translate`, `rustwa_translation_availability`) that wraps
   `TranslationSession(installedSource:target:)` / `LanguageAvailability`, compiled by
   `build.rs` (`swiftc -emit-library` or `-static`) and linked only for `target_os = "macos"`;
   `@available(macOS 15.0, *)` guards map `NeedsDownload` to the same UX the official app
   shows. Translation returns to the UI; the core stores the cached result as local metadata
   keyed by `(message_id, target_language)`.
3. Other platforms: `Unsupported` (honest "Translation isn't available on this system") until a
   local NMT backend (Bergamot/CTranslate2) is worth the download; never silently fall back to
   a cloud API.
4. UI: context-menu action "Translate" in `MessagePopover.tsx` / `MessageBubble.tsx` (mirrors
   `TranslationContextMenuActionPlugin`), translated text rendered under the original with the
   "Translated message" label, language selector (mirrors `LanguageSelector.swift`), and a
   per-chat "Translate all new messages" toggle in the chat info screen. Wording should reuse
   the official disclosure ("remain end-to-end encrypted"; "translated on your device") but
   with RustWA's own phrasing.
5. Tauri commands + registration in `lib.rs:235-336`: `translate_message`,
   `translation_availability`, `translation_set_auto`.

### 5.6 Explicitly out of scope / not appropriate

- Reimplementing fast-ratchet or the location key/KDM by hand without captures.
- Any cloud translation path enabled by default or without a per-use disclosure.
- Presenting single-frame/best-effort live sharing as "live location" parity.
- Syncd actions for either feature (none exist; writing guessed actions would desync ltHash —
  same rationale as [`scheduled-messages.md` §9](./scheduled-messages.md)).

---

## 6. Risks

| Risk | Severity | Notes / mitigation |
| --- | --- | --- |
| **Account flagging from synthetic live-location traffic** | High | Server accepts `liveLocationMessage` frames from normal clients; a third-party client emitting a non-standard update stream is exactly the kind of pattern anti-abuse watches. Test only on a disposable account, with captured wire shapes (never invented), behind a feature flag. Prefer read-only support if E4 fails. |
| **Privacy / safety of location data** | High | A live location is the most sensitive payload in the app. RustWA must never start sharing without an unambiguous user action, must show a persistent "sharing" indicator with a one-click stop (official app has proximity alerts and blocked-group rules, `ios_live_location_*`), must not log coordinates, and should not auto-download map tiles (third-party leak). |
| **Overpromising live parity** | Medium | The update channel is blocked upstream; document the difference in the UI ("updates from official clients aren't supported yet") rather than silently showing a stale pin. |
| **Translation privacy regression** | High | The only acceptable default is on-device. A cloud fallback must be opt-in, worded as breaking the E2EE guarantee, and never automatic. Cache translations locally; never send them to the peer. |
| **Platform dependency (Translation.framework)** | Medium | macOS 15+ only, Swift-only API, language packs downloaded from Apple. Needs the Swift shim + availability checks; hide/disable elsewhere. Build-time risk: requires Swift toolchain on the CI runner. |
| **Protocol drift** | Medium | Live location is old but the update crypto is being modernized (fast ratchet, 2026); the iOS AB props (`ios_live_location_*`) are not in the WA-Web registry. Anything built on E1–E4 must be re-captured after client updates. |
| **Store migration risk** | Low | Additive column + `user_version` bump; `raw_proto` already preserves everything, so the migration is recoverable. |

---

## 7. Decision record

For **live location**:

- Receive/render of static and first-frame live locations is implementable now and is the only
  slice that should be built without further evidence (L1).
- Sending a static location is implementable now (L2).
- A live *session* (updates, expiry, final location, server subscription) requires
  fast-ratchet sender-key support that does not exist in `wacore-libsignal 0.7.0`, plus
  unverified IQ/KDM wire shapes. Therefore: **no live-session module was designed as
  shippable**; L3 is explicitly capture-gated, and L4 is rejected as infeasible with the
  current stack.

For **message translation**:

- The official client translates on-device with Apple `Translation.framework`; the protocol is
  untouched. There is no upstream dependency and no capture work.
- It is implementable as a local feature, but only on macOS 15+ (Swift shim) unless RustWA
  later takes on an on-device NMT model. Cloud translation is rejected as a default for
  privacy reasons.

---

## 8. References

Upstream (registry root
`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`):

- `waproto-0.7.0/src/whatsapp.proto` — 2764 (`Message.liveLocationMessage = 18`), 3650
  (`Message.LiveLocationMessage`), 6381/6387 (`WebFeatures.liveLocations`),
  6453 (`WebMessageInfo.finalLiveLocation = 30`), 4188 (`ProtocolMessage.Type`, no live
  edits), 2534 (`textTranslated`, unrelated), 1624 (`ContextInfo`)
- `wacore-0.7.0/src/send/classify.rs:199-210` — location/livelocation mediatypes;
  `stanza_type_from_message` (media default)
- `wacore-0.7.0/src/messages.rs:1008-1035` — user-content detection incl. `live_location_message`
- `wacore-0.7.0/src/proto_helpers.rs:7-33` — `context_info` message list
- `wacore-0.7.0/src/history_sync.rs:991,1124` — live-location carrier tags
- `wacore-0.7.0/src/reporting_token.rs:163-168` — live-location reporting subfields
- `wacore-0.7.0/src/message_processing.rs:180` — `unknown_enc_types`
- `wacore-appstate-0.7.0/src/schemas.rs:1391-1458` — 66-action registry, no live location /
  translation
- `wacore-libsignal-0.7.0/src/protocol/` — sender-key/group ciphers; no fast ratchet
- `whatsapp-rust-0.7.0/src/send/mod.rs:282,481,846,901` — `SendOptions`,
  `infer_stanza_metadata` (no live-location branch), `send_message(_with_options)`
- `whatsapp-rust-0.7.0/src/client/messaging.rs:152,260` — `edit_message`,
  `edit_message_encrypted`
- `whatsapp-rust-0.7.0/src/bot.rs:1135`, `src/types/enc_handler.rs:16` — custom enc handlers

Official app (read-only): `/Applications/WhatsApp.app` 26.33.73 (1049819294) —
`Contents/MacOS/WhatsApp` (`main.strings` line refs inline), `SharedModules` (`shared.strings`),
`otool -L`, `nm -u`, `Contents/Info.plist`. Key strings listed in §3.5 and §4.1.

External: `whatspec` IR (`oxidezap/whatspec` main — `generated/proto/WAProto.proto`,
`generated/iq/index.json`); Baileys `master` `src/Socket/messages-send.ts:1195` and issue #1026
(subscribe IQ, `fskmsg` report); `wppconnect-team/wa-js` main
(`src/whatsapp/models/MsgModel.ts`, `src/chat/events/eventTypes.ts` — `shareDuration`,
`final*`, deprecated `live_location_update`); whatsmeow `main` (live-location protos only).

This repo:

- `crates/whatsapp-core/src/types.rs:111-184` — `MessageKind`, `Message`
- `crates/whatsapp-core/src/client.rs:714-798,1111-1143,1165-1193` — inbound/history paths,
  `classify()`
- `crates/whatsapp-core/src/store.rs:21,60-160,770-815` — schema/version, migrations, kind map
- `crates/whatsapp-core/src/schema/00*.sql` — migration pattern
- `crates/whatsapp-core/src/actions.rs:111,153` — text-only edit and revoke (reference for
  what exists)
- `crates/whatsapp-core/Cargo.toml:47` — `calls` feature-gating pattern
- `apps/desktop/src-tauri/Cargo.toml:46-51` — macOS-only `objc2` dependency pattern
- `apps/desktop/src-tauri/Info.plist` — current usage strings (microphone only)
- `apps/desktop/src-tauri/src/lib.rs:100,235-336` — command list/registration
- `apps/desktop/src/lib/types.ts:21-56`, `src/components/message/media.ts:139`,
  `MessageBubble.tsx` — UI type surface and bubble dispatch
- `docs/parity-matrix.md:117-118,438-439,446,460,498` — M11 context and prior verdicts
- `docs/research/scheduled-messages.md` — sibling method/format
