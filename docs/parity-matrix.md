# WhatsApp for macOS — Feature Parity Matrix for a Rust Rewrite

Research artifact for the `whatsapp-rust` desktop client rewrite. It enumerates the user-visible
feature set of the official macOS client and maps each feature onto the three candidate/companion
open-source stacks:

- **whatsapp-rust** — the Rust rewrite target, `whatsapp-rust` v0.7.0 (commit `6502b871`, 2026-09-11)
- **Baileys** — `7.0.0-rc14` (commit `0af23862`, 2026-08-04), TypeScript
- **whatsmeow** — commit `b25a56d6` (2026-09-09), Go

The official client inspected is **WhatsApp 26.33.73 (build 1049819294, `FBAppVersion 2.26.33.73`)**,
a Mac Catalyst app whose core logic lives in `Contents/Frameworks/SharedModules.framework`
(183 MB binary) with macOS glue in `WAAppKitBridge.framework` and two extensions
(`Intents.appex`, `ServiceExtension.appex`).

## Status legend

| Mark | Meaning |
| --- | --- |
| ✅ | Full — a first-class API covers the normal use of the feature (send + receive where applicable) |
| 🟡 | Partial — works only through raw protobuf/stanza construction, only one direction, or only some sub-options |
| ❌ | None — no support found in that stack |
| ➖ | N/A — not a protocol-library concern (UI/platform behavior) or handled outside the library |
| ❓ | Unknown — behavior inferred from symbols/docs, not proven from source or tests |

`Notes / effort` cites evidence as **[App]** (official app binary/resources), **[WR]** (whatsapp-rust),
**[B]** (Baileys), **[WM]** (whatsmeow). Effort is a rough implementation estimate for the desktop app
on top of whatsapp-rust: **S** ≈ days, **M** ≈ 1–2 weeks, **L** ≈ 3–6 weeks, **XL** ≈ months/uncertain.

> Terms of service: unofficial clients can get accounts banned. Nothing in this document is legal or
> ToS advice; the official app's behavior was inspected read-only and no account was connected.

---

## A. Account, onboarding, and device linking

| Feature | Official app | whatsapp-rust | Baileys | whatsmeow | Notes / effort |
| --- | --- | --- | --- | --- | --- |
| QR-code device linking | ✅ | ✅ | ✅ | ✅ | [WR] `src/pair.rs`, `PairingQrCode`/`PairingQrCodesExhausted` events, `examples/demo.rs` [B] README "Connect with QR-CODE" [WM] `pair.go`. |
| Pair-code (phone number) linking | ✅ | ✅ | ✅ | ✅ | [WR] `src/pair_code.rs`, `PairingCode`/`PairingCodeRefresh`/`PairingCodeError` events, demo `-p` flag [B] README "Connect with Pairing Code" [WM] `pair-code.go`. |
| Passkey device linking | ✅ | ✅ | ❌ | ✅ | [App] "A new device wants to link to your WhatsApp account using your passkey"; [WR] `passkey` cargo feature, `src/passkey/flow.rs` (`send_passkey_confirmation`, `send_passkey_response`, `set_passkey_authenticator`), events `PairPasskeyRequest/Confirmation/Error` [WM] `pair-passkey.go` (`SendPasskeyResponse`, `SendPasskeyConfirmation`) [B] only a WAM telemetry field `passkeyExists`. Effort: S (wire whatsapp-rust's already-built flow). |
| Multi-device companion sessions | ✅ | ✅ | ✅ | ✅ | All three implement the MD protocol; multiple sessions supported by the app (multiple windows/scenes). |
| History sync after linking | ✅ | ✅ | ✅ | ✅ | [WR] streaming parser `wacore/src/history_sync.rs` + `src/history_sync.rs` (message secrets retained, secrets seeded) [B] `Utils/history.ts`, full-history options [WM] `DownloadHistorySync`, `HistorySync` events. |
| Logout / unlink this device | ✅ | ✅ | ✅ | ✅ | [WR] `Client::logout` [B] `logout()` [WM] `Logout()`. |
| Linked-device list & updates | ✅ | 🟡 | 🟡 | 🟡 | [App] `WADevicesMain`, `DeviceListViewController`. Libraries receive `DeviceListUpdate`/device notifications ([WR] `EventKind::DeviceListUpdate`, `src/client/device_registry.rs`; [B] `device-list.update` via usync; [WM] `GetUserDevices`) but **removing another companion is a phone-side operation** not exposed by any library. |
| Device properties (OS, app version, push name) | ✅ | ✅ | ✅ | ✅ | [WR] `set_device_props`, `set_client_profile` [B] `browser` option [WM] `DeviceProps`/`SetStatusMessage`. |
| Proxy support for connection | ✅ | 🟡 | ✅ | ✅ | [App] Proxy screen, "Connect to WhatsApp using proxy" [B] `SocketConfig` proxy agent [WM] `SetProxy`, `SetProxyAddress`, `SetSOCKSProxy` [WR] no built-in proxy; transport is pluggable and the ureq HTTP client is configurable — a custom connector is needed. Effort: S/M. |
| Companion-mode / companion registration refresh | ✅ | ✅ | ✅ | ✅ | [App] `WAAccountService`, companion registration + scheduled messages from companion ([App] `...MessageProcessorCompanionScheduledMessage`) [WR] `src/pair_code.rs`, `CompanionRegRefresh` notification handler. |
| Account migration / move to a new phone | ✅ | ❌ | ❌ | ❌ | [App] `WAMigration`, `WAMigrationCoreShared`, device migration IQs. No OSS implementation. Effort: XL, low value for a desktop client. |
| Managed accounts / Meta Accounts Center | ✅ | ❌ | ❌ | ❌ | [App] `WAManagedAccountSettings`. Not protocol-level in any library. Effort: XL, likely gated. |
| Ban appeal / account review | ✅ | ❌ | ❌ | ❌ | [App] `WABanAppealsMain`, `BAN_APPEALS_NUDGE_NOTIFICATION_*` strings; [WR] only receives `TemporaryBan`/`LoggedOut`. Effort: L (server-owned flow). |
| Message capping / reachout timelocks (new accounts) | ✅ | ✅ | ❌ | ❌ | [WR] `Mex::fetch_new_chat_message_capping_info`, `fetch_reachout_timelock`; app shows `WACappingMain`, `WAReachoutTimelocks*`. |
| Group safety check on join | ✅ | ❌ | ❌ | ❌ | [App] `WAGroupSafetyCheck`. Effort: M. |

---

## B. Chats and conversation lifecycle

| Feature | Official app | whatsapp-rust | Baileys | whatsmeow | Notes / effort |
| --- | --- | --- | --- | --- | --- |
| 1:1 text send/receive | ✅ | ✅ | ✅ | ✅ | [WR] `src/send/mod.rs` (`send_text`, `send_message`), `EventKind::Messages` [B] `sendMessage` [WM] `SendMessage`. |
| Group text send/receive | ✅ | ✅ | ✅ | ✅ | Same paths with `g.us` JIDs; [WR] group send/repair `src/send/group_repair.rs`. |
| Replies / quotes | ✅ | ✅ | ✅ | 🟡 | [WR] `SendOptions::quote` / `bot.reply_quoting`, `build_quote_context` [B] README "Quote Message (works with all types)" [WM] quote is hand-built (no helper in `send.go`). |
| Mentions | ✅ | ✅ | ✅ | 🟡 | [WR] `SendOptions` mentions; `mentioned_jid` handling [B] README "Mention User" [WM] no builder; raw proto. |
| Forward messages | ✅ | ✅ | ✅ | 🟡 | [WR] `forward_message` (score/label handling) [B] README "Forward Messages" [WM] `SendMessage` on an existing message proto. |
| Edit message | ✅ | ✅ | ✅ | ✅ | [WR] `Client::edit_message(_encrypted/_with_options)`, `src/features/message_edit.rs` [B] README "Edit Messages" [WM] `BuildEdit`. |
| Delete for everyone (revoke) | ✅ | ✅ | ✅ | ✅ | [WR] `revoke_message`, `RevokeType` [B] README "Delete Messages (for everyone)" [WM] `BuildRevoke`/`RevokeMessage`. |
| Delete for me | ✅ | ✅ | 🟡 | ✅ | [WR] `delete_message_for_me` app-state action [B] `deleteMessageForMe` chat modification [WM] app-state patch (no high-level builder). |
| Star / unstar message | ✅ | ✅ | ✅ | ✅ | [WR] `star_message`/`unstar_message` [B] `chatModify({star: ...})` [WM] `appstate.BuildStar`. |
| Pin message in chat | ✅ | ✅ | ✅ | 🟡 | [App] "%@ pinned a message", `WAPinnedMessagesShared` [WR] `pin_message`/`unpin_message` + `PinDuration` [B] `pinInChatMessage` builder [WM] `waE2E.PinInChatMessage` referenced on send (`send.go:1060`) but no public builder. |
| Keep in chat | ✅ | ✅ | ❌ | 🟡 | [WR] `keep_message` + `build_keep_in_chat_message` [WM] `KeepInChatMessage` handled as an edit attribute, no builder [B] no references found. Effort: S in B/WM. |
| View once (send/receive) | ✅ | 🟡 | ✅ | 🟡 | [WR] send meta `view_once="true"` (`src/send/mod.rs:694`) and receive stub handling (`src/message/receive.rs:252`); no high-level "open once" reveal state machine [B] `viewOnce` option wraps V2/V2-extension [WM] raw proto. |
| Disappearing messages (per-chat timer) | ✅ | ✅ | ✅ | ✅ | [WR] `set_chat_disappearing_timer`, `SendOptions::with_ephemeral_expiration`, `DisappearingModeChanged` [B] `ephemeralExpiration`/`ephemeralSettingTimestamp` [WM] `SetDisappearingTimer` + `ParseDisappearingTimerString`. |
| Default disappearing timer for new chats | ✅ | ✅ | ✅ | ✅ | [WR] `set_default_disappearing_mode` [B] `chatModify`/settings [WM] `SetDefaultDisappearingTimer`. |
| Clear chat | ✅ | ✅ | ✅ | ✅ | [WR] `clear_chat` [B] `chatModify({clear: ...})` [WM] app-state `ClearChat` incoming; clearing is local + `BuildDeleteChat`-class patches. |
| Delete chat | ✅ | ✅ | ✅ | ✅ | [WR] `delete_chat` [B] `chatModify({delete: true})` [WM] `appstate.BuildDeleteChat`. |
| Archive / unarchive, keep archived | ✅ | ✅ | ✅ | ✅ | [WR] `archive_chat`/`unarchive_chat`, `ArchiveUpdate` [B] `archiveChat` [WM] `appstate.BuildArchive`. |
| Pin / unpin chat | ✅ | ✅ | ✅ | ✅ | [WR] `pin_chat`/`unpin_chat` [B] `pinChat` [WM] `appstate.BuildPin`. |
| Mute / unmute (with "until") | ✅ | ✅ | ✅ | ✅ | [WR] `mute_chat`, `mute_chat_until`, `announce_muted` [B] `muteChat` [WM] `appstate.BuildMute`/`BuildMuteAbs`. |
| Mark chat read / unread | ✅ | ✅ | ✅ | ✅ | [WR] `mark_chat_as_read`, `receipt.rs` [B] `readMessages`, `chatModify` [WM] `MarkRead`, `appstate.BuildMarkChatAsRead`. |
| Typing / recording indicators | ✅ | ✅ | ✅ | ✅ | [WR] `features/chatstate.rs` (`send_composing`, `send_paused`, `send_recording`) [B] `sendPresenceUpdate` [WM] `SendChatPresence`. |
| Drafts | ✅ | ➖ | ➖ | ➖ | Client-side UI state (app stores locally). The rewrite owns this. Effort: S. |
| Scheduled messages | ✅ | ❌ | ❌ | ❌ | [App] `WAScheduledMessageDeletion`, "Review scheduled messages", `...CompanionScheduledMessage` — the protocol exists (companion-sent scheduled placeholders) but no OSS stack implements it. Effort: M/L (needs capture work). |
| Chat lists / custom filters ("Lists", Favorites) | ✅ | ❌ | ❌ | ❌ | [App] `WALists`; strings "Any list you create becomes a filter at the top of your Chats tab", "Add communities list", "%@ list". No app-state schema in any library. Effort: M (app-state reverse engineering). |
| Favorites / starred entry points | ✅ | 🟡 | 🟡 | 🟡 | Starring messages exists everywhere; the Favorites *list* feature is app-state (see above). |
| Wallpaper & chat theme | ✅ | ➖ | ➖ | ➖ | [App] `WAWallpaperPickerViewController`, `WallpaperV2` assets. Local UI, not synced protocol (except default wallpaper app-state). |
| In-chat search / message search | ✅ | ➖ | ➖ | ➖ | [App] `WASearchController*`, `WAWWASearchEngine`, `WASemanticSearch` (AI search). Local DB work in the Rust client; semantic search is server/AI-gated. |
| Text formatting (bold/italic/~~strike~~/mono) | ✅ | ➖ | ➖ | ➖ | Rendering concern; the wire carries markdown-like text. Effort: S. |
| Link previews (send) | ✅ | 🟡 | ✅ | ❌ | [B] `getUrlInfo` fetches OG tags + thumbnail and uploads it [WR] can send an `ExtendedTextMessage` preview if the caller fetches it, but no fetcher exists [WM] none. Effort: S/M for a Rust OG fetcher. |
| Link previews (disable account-wide) | ✅ | ✅ | ✅ | ✅ | [WR] `set_link_previews_disabled` + `DisableLinkPreviewsUpdate` [B] app-state setting [WM] privacy settings. |
| Notification reply / quick actions | ✅ | ➖ | ➖ | ➖ | macOS notification category actions; client-side. Effort: M. |
| Copy / paste, select text | ✅ | ➖ | ➖ | ➖ | Client-side. |

---

## C. Message content types

| Feature | Official app | whatsapp-rust | Baileys | whatsmeow | Notes / effort |
| --- | --- | --- | --- | --- | --- |
| Image (with caption) | ✅ | ✅ | ✅ | ✅ | [WR] `media::image_message`, upload/download pipeline, `HD` not supported [B] README "Image Message" [WM] `Upload`+proto. |
| Video (with caption) | ✅ | ✅ | ✅ | ✅ | [WR] `media::video_message` [B] README "Video Message" [WM] proto. |
| GIF (mp4, silent) | ✅ | ✅ | ✅ | 🟡 | [WR] classify handles `gif`; sends as video with `gifPlayback` [B] README "Gif Message" [WM] raw proto. |
| Voice note (PTT) | ✅ | ✅ | ✅ | ✅ | [WR] `media::audio_message` (`ptt=true`) [B] README "Audio Message" (audio→ptt) [WM] proto + `Upload`. |
| Audio file (music) | ✅ | ✅ | ✅ | ✅ | Same pipelines without PTT. |
| Document (any file) | ✅ | ✅ | ✅ | ✅ | [WR] `media::document_message` [B] README "Document" [WM] `Upload`. |
| Document scanner | ✅ | ➖ | ➖ | ➖ | [App] `VNDocumentCameraViewController`. UI/platform. |
| Photo editor (draw, crop, text, stickers) | ✅ | ➖ | ➖ | ➖ | [App] `WAPhotoStampEditingViewController`, `MediaEditor*`. UI. Effort: L (or use system editor). |
| Camera capture | ✅ | ➖ | ➖ | ➖ | UI. |
| Sticker (send/receive, static + animated) | ✅ | 🟡 | ✅ | 🟡 | [WR] receive/classify/download support (`wacore/src/send/classify.rs:221`, `wacore/src/download.rs:216`) and `wacore::webp` can detect animation, but there is no sticker builder or WebP encoder; caller must build `StickerMessage` + encode [B] media pipeline supports stickers (users typically add sharp for WebP) [WM] raw proto. Effort: S/M. |
| Sticker packs (browse/install) | ✅ | 🟡 | 🟡 | ✅ | [WR] `fetch_sticker_pack` (CDN metadata + items) [WM] `FetchStickerPack` [B] no first-party fetch. |
| Sticker / emoji search & recent | ✅ | ➖ | ➖ | ➖ | UI + local DB. |
| Avatars (Meta avatar stickers, profile avatar) | ⚠️ retired | ❌ | ❌ | ❌ | [App] strings "Avatar creation is not available anymore", "Avatars are no longer available" — the feature was deprecated by Meta; `WAStatusItemViews`/`Avatars` module remains. Low priority. |
| AI-generated stickers ("Meta AI stickers") | ✅ | ❌ | ❌ | ❌ | [App] `WAAIStickers`; requires Meta AI backend. Likely infeasible (see gaps). |
| Video note / PTV ("instant video message") | ✅ | 🟡 | 🟡 | 🟡 | Proto `ptvMessage` present in all three proto packages; no first-class helper in any (whatsapp-rust has no `ptv` references). Effort: S/M. |
| Albums / multi-send (up to 30) | ✅ | 🟡 | 🟡 | 🟡 | [WR] `wrap_as_album_child` in `wacore/src/proto_helpers.rs` [B] `albumMessage` proto only [WM] proto only. |
| Contact card (vCard) | ✅ | 🟡 | ✅ | ✅ | [B] README "Contact Message"; [WR]/[WM] raw `ContactMessage` proto. Effort: S. |
| Location (static) | ✅ | 🟡 | ✅ | ✅ | [B] README "Location Message"; [WR] no helper (proto only; receive supported in `wacore/src/messages.rs`); [WM] proto. Effort: S. |
| Live location | ✅ | 🟡 | 🟡 | 🟡 | Proto + receive parse everywhere; no periodic-refresh/session helper in any library; [WR] `src/request.rs` notes live-location use cases. Effort: M. |
| Poll (create / vote / results) | ✅ | ✅ | 🟡 | ✅ | [WR] `features/polls.rs`: `create`, `create_quiz`, `vote`, `decrypt_vote`, `aggregate_votes` [WM] `BuildPollCreation`, `BuildPollVote`, `DecryptPollVote`, `HashPollOptions` [B] create + decrypt vote + aggregate (`Utils/process-message.ts`, `Utils/messages.ts:936`) but **no outgoing vote helper**. |
| Quiz poll | ✅ | ✅ | 🟡 | 🟡 | [WR] `create_quiz` [B]/[WM] proto only. |
| Event (create, RSVP, edit, cancel) | ✅ | ✅ | ✅ | 🟡 | [WR] `features/events.rs` (`create`/`respond`, encrypted RSVP) [B] builds `eventMessage`, decrypts responses (`Utils/messages.ts:531`, `process-message.ts:269`) [WM] `msgsecret.go` recognizes event-response secrets; no public builder. |
| Event share links / sharable events | ✅ | 🟡 | ❌ | ❌ | [App] `WASharableEventsMain`, event join links are part of `eventMessage.joinLink`; [WR] passes `join_link` through. |
| Status questions / Q&A | ✅ | ❌ | ❌ | ❌ | [App] `WAQuestionsShared`, `WAPBStatusQuestionAnswers` ("Status Questions"?). Newer format, no OSS support. Effort: M/L. |
| Channel comments (encrypted threaded) | ✅ | ✅ | ❌ | ❌ | [WR] `features/comments.rs` (encrypted comment envelope, transparent decrypt) — a differentiator. |
| Product message (catalog share) | ✅ | 🟡 | ✅ | 🟡 | [WR] catalog read APIs are full (`get_catalog`/`get_collections`/`get_order`) but sending a product message is raw proto [B] `parseProductNode`, `productMessage` builder + send handling [WM] `send.go:985` handles `ProductMessage`, no catalog fetch API. |
| Order message | ✅ | 🟡 | ✅ | 🟡 | [B] `orderMessage` in send path [WM] `GetOrderDetails` read + proto send. |
| Invoice message (legacy) | ❌ / legacy | 🟡 | 🟡 | 🟡 | The `invoiceMessage` field name has **0 hits** in current app binaries; the proto still exists in all three stacks, so only raw-proto send/receive is possible. Deprioritize. |
| Payment invite / request money message | ✅ | ❌ | ❌ | ❌ | [App] `paymentInviteMessage` in strings; no OSS builder. Effort: XL (payments). |
| Call log message (sync from phone) | ✅ | ✅ | 🟡 | 🟡 | [WR] `src/features/call_log.rs` app-state `call_log` sync [B]/[WM] receive `callLogMessage` in history sync; no app-state reader. |
| Group invite message | ✅ | ✅ | ✅ | ✅ | `groupInviteMessage` proto everywhere; [WR] `accept_group_invite`/`preaccept_group_invite`. |
| Newsletter invite messages | ✅ | 🟡 | 🟡 | 🟡 | Proto `newsletterAdminInviteMessage`; join via invite handled by libraries; admin invite acceptance not first-class. |
| Poll result snapshot | ✅ | ✅ | 🟡 | 🟡 | Proto; [WR] `aggregate_votes` produces snapshots. |
| Business buttons / lists / templates / interactive | ✅ | 🟡 | 🟡 | 🟡 | Proto messages exist in all stacks; sending is raw-proto. [WR] `src/send/mod.rs` has biz category mapping incl. payment flow names (`payment_info`, `order_details`). Effort: M. |
| WhatsApp Flows | ✅ | ❌ | ❌ | ❌ | [App] `WAFlows`, `FlowsWebViewController`, `bloks_*` templates. No OSS support. Effort: L, business-only. |
| Rich AI responses (richResponseMessage) | ✅ | ❌ | ❌ | ❌ | [App] `WAMessageViewsRichResponse`, `MAIRichResponseExamples`. Meta AI only. |

---

## D. Media pipeline and settings

| Feature | Official app | whatsapp-rust | Baileys | whatsmeow | Notes / effort |
| --- | --- | --- | --- | --- | --- |
| Media upload | ✅ | ✅ | ✅ | ✅ | [WR] `upload.rs` (streaming sidecar, media key) [B] `waUploadToServer` [WM] `Upload`/`UploadReader`. |
| Media download | ✅ | ✅ | ✅ | ✅ | [WR] `download.rs` (`download`, `download_to_writer`, params-based) [B] `downloadMediaMessage`/stream [WM] `Download`, `DownloadAny`, `DownloadThumbnail`. |
| Streaming / progressive media | ✅ | 🟡 | 🟡 | 🟡 | [WR] streaming sidecar for upload, download-to-writer [B] stream download [WM] `UploadReader`. UI-level progressive playback remains app work. |
| Thumbnails + blurhash + JPEG sidecar | ✅ | ✅ | ✅ | ✅ | [WR] `wacore/src/download.rs`, media builders attach `jpeg_thumbnail`/`blur_hash` [B] `extractImageThumb` [WM] `DownloadThumbnail`. |
| HD photo / video quality | ✅ | ❌ | ❌ | ❌ | [App] "HD quality", "Media will be uploaded on HD quality", `HD (%@)`. None of the stacks build the dual-quality (HD + standard) message. Effort: M (protocol shape known: two uploads + `quality` differs). |
| Media transcoding (video/audio to compatible codecs) | ✅ | ➖ | 🟡 | ➖ | [App] `WAMediaTranscoder`. [B] relies on `music-metadata` for audio; video transcoding is external. The Rust client should call ffmpeg or system AVFoundation. Effort: M. |
| Media reupload on failure | ✅ | ✅ | ✅ | ✅ | [WR] `features/media_reupload.rs` + receive-side reupload handling [B] retry manager [WM] `mediaretry.go`. |
| Auto-download settings (per network/type) | ✅ | ➖ | ➖ | ➖ | Local settings; the rewrite stores them. |
| Save to Photos / gallery | ✅ | ➖ | ➖ | ➖ | Platform integration (PhotoKit). Effort: S/M. |
| Storage management / data usage screens | ✅ | ➖ | ➖ | ➖ | [App] `WADataUsageViewController`, BG task `com.whatsapp.storage_management`, `WADiagnosticsCollector`. Client-side. |
| Voice message playback speed / scrubbing | ✅ | ➖ | ➖ | ➖ | [App] `WAPTTWaveformViewController`. UI. |
| Voice message transcripts | ✅ | ❌ | ❌ | ❌ | [App] `WAPTTTranscription`, `WATranscriptLanguageSelectorViewController`, "Read your voice messages with transcripts". Server/OS-side; no OSS. Effort: L, likely server-dependent. |
| Media gallery / Documents browser | ✅ | ➖ | ➖ | ➖ | [App] `WAMediaGalleryViewController`, `WADocumentsBrowserViewController`. Local DB/UI. |
| Media editor integrations (Quick Look / Preview) | ✅ | ➖ | ➖ | ➖ | macOS integration. |

---

## E. Reactions, receipts, presence, and notifications

| Feature | Official app | whatsapp-rust | Baileys | whatsmeow | Notes / effort |
| --- | --- | --- | --- | --- | --- |
| Emoji reaction (send/receive) | ✅ | ✅ | ✅ | ✅ | [WR] `features/reaction.rs` (`send_reaction`, incl. status fan-out), `frequentReactionsManager` on app side [B] `sendMessage({react})` [WM] `BuildReaction`. |
| Reaction revoke / change | ✅ | ✅ | ✅ | ✅ | Empty-string reaction retracts. |
| Reaction details ("who reacted") | ✅ | 🟡 | 🟡 | 🟡 | [App] `WAReactionDetailViewController`. Libraries deliver reaction events; storing/aggregating is client work. Effort: S. |
| Sticker reactions | ✅ | ❓ | ❌ | ❌ | [App] `WAReactionStickerUIShared`. Protocol support unverified; possibly UI-only rendering of sticker packs. Effort: unknown. |
| Delivery receipts | ✅ | ✅ | ✅ | ✅ | [WR] `src/receipt.rs`, receipt handler [B] `messages.update` [WM] `Receipt` events. |
| Read receipts (+ privacy toggle) | ✅ | ✅ | ✅ | ✅ | [WR] `mark_as_read` [B] `readMessages` [WM] `MarkRead`; privacy via settings. |
| Played receipts (audio/video me) | ✅ | ✅ | 🟡 | ✅ | [WR] `mark_as_played` [WM] `sendMessageReceipt` with played type [B] receipt types exist; no dedicated API. |
| Status view receipts / viewers list | ✅ | 🟡 | 🟡 | 🟡 | [WR] status receipts on the receive path; viewers aggregation is client work. |
| Typing indicators | ✅ | ✅ | ✅ | ✅ | See chatstate row. |
| Recording indicator | ✅ | ✅ | 🟡 | 🟡 | [WR] `send_recording` [B] `recording` chatstate [WM] chatstate via `SendChatPresence` media param. |
| Online / last seen presence | ✅ | ✅ | ✅ | ✅ | [WR] `features/presence.rs` (set/subscribe/unsubscribe, `PresencePolicy`) [B] `sendPresenceUpdate`/`presence.update` [WM] `SendPresence`, `SubscribePresence`. |
| Push notifications: messages/groups | ✅ | ➖ | ➖ | ➖ | Library emits events; the desktop client must own notification presentation (and keep the socket alive). Effort: M. |
| Push notifications: reactions/mentions | ✅ | ➖ | ➖ | ➖ | Reaction + mention events exist (`Messages` metadata); presentation is client-side. |
| Push notifications: calls (VOIP push) | ✅ | 🟡 | ❌ | ❌ | [App] `UIBackgroundModes: voip`, local-network call receive. [WR] `IncomingCall`/`MissedCall` events arrive on the live socket; true APNs VoIP wake is platform-specific (macOS Catalyst uses `voip` background mode). |
| Notification Service Extension (rich notification, decrypt in NSE) | ✅ | ➖ | ➖ | ➖ | [App] `ServiceExtension.appex` `WANotificationService`, E2E status decrypt, media reupload inside NSE. On macOS, an equivalent helper app/extension is needed. Effort: L. |
| Notification controls (per chat, mention-only, call ringtone) | ✅ | ➖ | ➖ | ➖ | [App] `WANotificationControlsViewController`; settings are local/app-state. |
| Mute schedules / custom mute per list | ✅ | ❌ | ❌ | ❌ | [App] "%@ is in a list that has custom mute schedule" — tied to the Lists feature (app-state). |
| Missed call notifications | ✅ | ✅ | ✅ | ✅ | [WR] `MissedCall`, `CallEndedElsewhere` events [B] `WACallEvent` status [WM] `CallTerminate` reason. |

---

## F. Groups

| Feature | Official app | whatsapp-rust | Baileys | whatsmeow | Notes / effort |
| --- | --- | --- | --- | --- | --- |
| Create group | ✅ | ✅ | ✅ | ✅ | [WR] `features/groups.rs::create_group` [B] `groupCreate` [WM] `CreateGroup`. |
| Group metadata (subject/description/picture) | ✅ | ✅ | ✅ | ✅ | [WR] `get_metadata`, `set_subject`, `set_description`, `set_profile_picture`/`remove_profile_picture` [B] `groupUpdateSubject/Description` [WM] `SetGroupName`, `SetGroupTopic`, `SetGroupPhoto`. |
| Add / remove participants | ✅ | ✅ | ✅ | ✅ | [WR] `add_participants`, `remove_participants`, incl. linked groups [B] `groupParticipantsUpdate` [WM] `UpdateGroupParticipants`. |
| Promote / demote admins | ✅ | ✅ | ✅ | ✅ | [WR] `promote_participants`, `demote_participants`, `is_admin`/`is_super_admin` [WM] `UpdateGroupParticipants`. |
| Leave group | ✅ | ✅ | ✅ | ✅ | [WR] `leave` [B] `groupLeave` [WM] `LeaveGroup`. |
| Invite links (create/revoke/lookup/join) | ✅ | ✅ | ✅ | ✅ | [WR] `get_invite_link`, `join_with_invite_code`, `join_with_invite_v4`, `accept_group_invite`, `preaccept_group_invite`, `revoke_request_code` [B] `groupInviteCode`, `groupRevokeInvite`, `groupAcceptInvite`, `groupGetInviteInfo` [WM] `GetGroupInviteLink`, `JoinGroupWithLink/Invite`, `GetGroupInfoFromLink/Invite`. |
| Membership approval mode + requests | ✅ | ✅ | ✅ | ✅ | [WR] `set_membership_approval`, `get_membership_requests`, `approve/reject/cancel_membership_requests` [B] `groupJoinApprovalMode`, `groupRequestParticipantsList/Update` [WM] `SetGroupJoinApprovalMode`, `GetGroupRequestParticipants`, `UpdateGroupRequestParticipants`. |
| Announce mode, locked mode, member-add mode | ✅ | ✅ | ✅ | ✅ | [WR] `set_announce`, `set_locked`, `set_member_add_mode`, `set_member_link_mode` [B] `groupSettingUpdate`, `groupMemberAddMode` [WM] `SetGroupAnnounce`, `SetGroupLocked`, `SetGroupMemberAddMode`. |
| Group ephemeral timer | ✅ | ✅ | ✅ | ✅ | [WR] `set_ephemeral`, `set_group_history` [B] `groupToggleEphemeral` [WM] `SetDisappearingTimer` on group. |
| History for new members | ✅ | ✅ | ✅ | ✅ | [WR] `set_group_history`, `set_member_share_history_mode` [B] group history options; [WM] via `CreateGroup`/settings. |
| Group events (scheduled calls) | ✅ | ✅ | 🟡 | 🟡 | [App] `WAGroupEvents`, `WAScheduleCallEventsListViewController`, `scheduledCallCreationMessage`/`scheduledCallEditMessage` [WR] events support `is_scheduled_call` [B]/[WM] proto only. |
| Group call start/join UI | ✅ | 🟡 | ❌ | ❌ | See Calls domain. |
| Group description/subject change system messages | ✅ | ✅ | ✅ | ✅ | Event parsing everywhere (`GroupUpdate` [WR], `groups.update` [B], `GroupInfo` [WM]). |
| Reported messages → admins | ✅ | ✅ | 🟡 | 🟡 | [WR] `get_reported_messages`, `report_messages_to_admins`, `set_allow_admin_reports`. |
| Group safety check / suspicious group detection | ✅ | ❌ | ❌ | ❌ | [App] `WAGroupSafetyCheck`; server-driven; low priority. |
| "No frequently forwarded" setting | ✅ | ✅ | 🟡 | 🟡 | [WR] `set_no_frequently_forwarded`. |
| Member labels / tags (business groups) | ✅ | ✅ | ❌ | ❌ | [WR] `update_member_label(_with_id)`. |

---

## G. Communities

| Feature | Official app | whatsapp-rust | Baileys | whatsmeow | Notes / effort |
| --- | --- | --- | --- | --- | --- |
| Create community (announcement group + subgroups) | ✅ | ✅ | ✅ | ✅ | [WR] `features/community.rs::create`, `create_subgroup`, `query_linked_group` [B] `communityCreate`, `communityCreateGroup`, `communityFetchLinkedGroups` [WM] `CreateGroup` parent variants, `GetSubGroups`. |
| Link / unlink existing groups | ✅ | ✅ | ✅ | ✅ | [WR] `link_subgroups`, `unlink_subgroups`, `get_linked_groups_participants` [B] `communityLinkGroup`, `communityUnlinkGroup` [WM] `LinkGroup`, `UnlinkGroup`, `GetLinkedGroupsParticipants`. |
| Join / leave community & subgroups | ✅ | ✅ | ✅ | ✅ | [WR] `join_subgroup`, `get_participating`, `leave` (as group) [B] `communityAcceptInvite`, `communityLeave` [WM] invite/link joins. |
| Community admin & participant management | ✅ | ✅ | ✅ | 🟡 | [WR] `remove_participants`, `get_subgroup_participant_counts` [B] `communityParticipantsUpdate`, `communityRequestParticipants*` [WM] group participant APIs apply; community-specific helpers lighter. |
| Deactivate / delete community | ✅ | ✅ | 🟡 | 🟡 | [WR] `deactivate`; [B] `communityLeave`; [WM] raw. |
| Subgroup discovery / auto-add | ✅ | ✅ | ✅ | ✅ | Parent-group metadata + participant pickers; [WR] `get_subgroups`, `get_subgroup_participant_counts`. |
| Community description/settings | ✅ | ✅ | ✅ | ✅ | Same metadata ops as groups ([B] `communityUpdateDescription`, `communitySettingUpdate`). |

---

## H. Channels (protocol name: newsletters)

| Feature | Official app | whatsapp-rust | Baileys | whatsmeow | Notes / effort |
| --- | --- | --- | --- | --- | --- |
| Create / delete channel | ✅ | ✅ | ✅ | 🟡 | [WR] `features/newsletter.rs::create`/`delete` [B] `newsletterCreate`, `newsletterDelete` [WM] `CreateNewsletter` (no delete exposed). |
| Rename / description / picture | ✅ | ✅ | ✅ | 🟡 | [WR] `update`, `set_picture`, `remove_picture` [B] `newsletterUpdateName/Description/Picture`, `newsletterRemovePicture` [WM] update via MEX mutations internally, no public API. |
| Follow / unfollow / mute | ✅ | ✅ | ✅ | ✅ | [WR] `join`, `leave`, `set_follower_mute` [B] follow/mute functions [WM] `FollowNewsletter`, `UnfollowNewsletter`, `NewsletterToggleMute`. |
| Admin management (owner transfer, demote, admin info) | ✅ | ✅ | ✅ | ❌ | [WR] `change_owner`, `demote_admin`, `get_admin_info`, `set_admin_mute` [B] `newsletterChangeOwner`, `newsletterDemote`, `newsletterAdminCount` [WM] not exposed. |
| Post message (plaintext, not E2E) | ✅ | ✅ | ✅ | ✅ | [WR] `src/message/special.rs` plaintext newsletter path + generic send [B] `sendMessage` handles `newsletter` JIDs [WM] `sendNewsletter` in `send.go`. |
| Edit / revoke channel post | ✅ | ✅ | ✅ | 🟡 | [WR] `edit_message`, `revoke_message` [B] message edit/revoke against newsletter JID [WM] raw proto only. |
| Reactions | ✅ | ✅ | ✅ | ✅ | [WR] `send_reaction` [B] `newsletterReactMessage` [WM] `NewsletterSendReaction`. |
| Live updates (reaction counts, message updates) | ✅ | ✅ | ✅ | ✅ | [WR] `subscribe_live_updates` + `NewsletterLiveUpdate` event [B] `subscribeNewsletterUpdates` [WM] `NewsletterSubscribeLiveUpdates`. |
| Fetch messages / pagination | ✅ | ✅ | ✅ | ✅ | [WR] `get_messages` [B] `newsletterFetchMessages` [WM] `GetNewsletterMessages`, `GetNewsletterMessageUpdates`. |
| Subscribers / followers list | ✅ | ✅ | ✅ | 🟡 | [WR] `get_followers` [B] `newsletterSubscribers` [WM] metadata only. |
| Mark viewed / analytics | ✅ | 🟡 | 🟡 | ✅ | [WM] `NewsletterMarkViewed`, `GetNewsletterMessageUpdates`; [WR]/[B] no explicit mark-viewed. |
| Directory search + invite links | ✅ | 🟡 | 🟡 | ✅ | [App] `WANewsletterDirectoryViewController`; [WR] `get_metadata_by_invite`, `list_subscribed`; [WM] `GetNewsletterInfoWithInvite`, `AcceptTOSNotice`; [B] metadata with invite. |
| Channel insights (views, growth) | ✅ | ❌ | ❓ | ❌ | [App] `WANewsletterInsightsViewController`. Requires MEX op not implemented anywhere. Effort: M. |
| Channel comments | ✅ | ✅ | ❌ | ❌ | [WR] `features/comments.rs`; [B]/[WM] parse `commentMessage` in history but no encrypted comment envelope support. |
| Channel admin profile status / channel status posts | ✅ | ❌ | ❌ | ❌ | [App] `WANewsletterStatus`, `NewsletterStatusAdminProfileStatusProtobufAdapter`. Effort: M/L. |
| Integrity / violating message reporting | ✅ | ❌ | ❌ | ❌ | [App] `WANewsletterViolatingMessages`, `WANewsletterIntegrity`. Low priority. |
| Admin invite message (accept/decline) | ✅ | 🟡 | 🟡 | 🟡 | Proto `newsletterAdminInviteMessage` + admin-change notifications parsed, no first-class accept path in any. Effort: S. |

---

## I. Status / Stories

| Feature | Official app | whatsapp-rust | Baileys | whatsmeow | Notes / effort |
| --- | --- | --- | --- | --- | --- |
| Text status | ✅ | ✅ | ✅ | 🟡 | [WR] `features/status.rs::send_text` [B] send to `status@broadcast` [WM] README "Sending status messages (experimental)". |
| Image / video status | ✅ | ✅ | ✅ | 🟡 | [WR] `send_image`, `send_video` [B] media to status JID [WM] proto + `Upload`. |
| Voice status | ✅ | ❌ | ❌ | ❌ | [App] `WAPTTWaveform`/`WAPTTTranscription` used in status composer; protocol is an audio message to `status@broadcast` (constructible). Effort: S with raw proto. |
| Status privacy (contacts, except, only-share-with, close friends) | ✅ | 🟡 | 🟡 | 🟡 | [App] `WAAudienceSelector`, `Close friends`, `WACloseSharingAudience*` [WR] `StatusPrivacySetting` enum + privacy settings; [B] `statusJidList`; [WM] `GetStatusPrivacy` (read). Audience-list management is not in any library. Effort: M. |
| View receipts / viewers list | ✅ | 🟡 | 🟡 | 🟡 | Receive-side receipts exist; aggregation + list UI is client work. |
| Status replies (DM) & reactions | ✅ | ✅ | 🟡 | 🟡 | [WR] reactions fan out to the status author; replies are ordinary DMs [B]/[WM] raw proto. |
| Status mentions | ✅ | ❌ | ❌ | ❌ | [App] `WAStatusMentionsTableViewController`, `WAStatusMentionParticipantPickerViewController`. Effort: M. |
| Group statuses (post to a group) | ✅ | ❌ | ❌ | ❌ | [App] "Add group status", "%@ added a group status", `WAGroupStatusShared`, `GroupStatusStatusProtobufAdapter`, `GroupStatusForwardPickerViewController`. Not implemented in any OSS stack. Effort: M. |
| Status music (Apple Music integration) | ✅ | ❌ | ❌ | ❌ | [App] `WAMusicShared` (lyrics scrubber, representation picker), "Add status with music". Effort: L (Apple Music licensing/API). |
| Status likes | ✅ | 🟡 | 🟡 | 🟡 | [App] `WAStatusLikesNotificationManager`; protocol likely reactions — not distinguishable in OSS. |
| Status archive | ✅ | ❌ | ❌ | ❌ | [App] `WAStatusArchiveViewController`, `WAStatusArchiveRollbackManager`. Local/DB feature. Effort: S/M. |
| Status questions / Q&A | ✅ | ❌ | ❌ | ❌ | [App] `WAQuestionsShared`, "Quiz responses"/`WAPBStatusQuestionAnswers`. Effort: M/L. |
| Cross-post status to Facebook / Instagram | ✅ | ❌ | ❌ | ❌ | [App] `WACrossFamilyShared`, `_igAutoCrosspostingEnabledStore`, `_fbAutoCrosspostingEnabledStore`; Meta cross-family APIs. Effort: XL, likely infeasible. |

---

## J. Calls and VoIP

| Feature | Official app | whatsapp-rust | Baileys | whatsmeow | Notes / effort |
| --- | --- | --- | --- | --- | --- |
| 1:1 voice call (media + signaling) | ✅ | ✅ | ❌ | ❌ | [WR] `voip` feature; `src/voip/` (signaling, SRTP/SFrame, MLOW/Opus, relay + P2P transport); README "1:1 VoIP audio calls"; conformance gates in `agent_docs/voip_conformance.md`. [B]/[WM] only receive call events + reject. Effort for B/WM would be XL. |
| 1:1 video call | ✅ | 🟡 | ❌ | ❌ | [WR] `voip/video.rs` (H.264 Annex-B transport), `CallHandle::start_video/accept_video/stop_video`; conformance doc says audio/video callback traces still being derived — treat as preview. [B] `WACallEvent.isVideo` event only. |
| Group voice / video call | ✅ | 🟡 | ❌ | ❌ | [WR] group call registry, `invite_participant`, `group_call`, `ring_participant`, `admit/deny_waiting_user`; maturity unverified against a live server. |
| Call links (create / preview / join) | ✅ | ✅ | 🟡 | ❌ | [App] "Anyone can join a call link on WhatsApp..."; [WR] `create_call_link`, `preview_call_link`, `join_call_link` [B] `createCallLink` in `Socket/chats.ts:787` (create only) [WM] none. |
| Waiting room / lobby | ✅ | ✅ | ❌ | ❌ | [WR] `admit_waiting_user`, `deny_waiting_user`, `set_approval_required`, `waiting_room_heartbeat`; [B] `CallOfferNotice` type only; [WM] none. |
| Screen sharing | ✅ | 🟡 | ❌ | ❌ | [App] "%@ started screen sharing", `WAScreenSharing`, `WAAppKitDesktopScreenCaptureBridge` [WR] `start_screen_share`/`stop_screen_share`/`set_screen_share` in facade + `set_screen_share` on Client; preview maturity. |
| Raise hand | ✅ | ✅ | ❌ | ❌ | [App] "Raise hand"; [WR] `set_hand_raised`. |
| Call reactions | ✅ | ✅ | ❌ | ❌ | [WR] `CallHandle::send_reaction`; [B]/[WM] none. |
| Mute / camera / speaker controls | ✅ | ✅ | ❌ | ❌ | [WR] `set_muted`, `video_states`, `announce_video_enabled`, `resume_video`; UI controls are app work. |
| Add participant / participant list | ✅ | ✅ | ❌ | ❌ | [WR] `invite_participant`, `CallControlMoreMenu`, participant events. |
| Call history + call log sync | ✅ | ✅ | 🟡 | 🟡 | [WR] `features/call_log.rs` (app-state `call_log`, documented direction ambiguity); [B]/[WM] receive `callLogMessage` in history sync only. |
| Missed / declined / ended-elsewhere states | ✅ | ✅ | ✅ | ✅ | [WR] `MissedCall`, `CallEndedElsewhere`, `IncomingCall` [B] `WACallEvent.status` [WM] `CallReject`/`CallTerminate`. |
| Call on any linked device (multi-device) | ✅ | 🟡 | ❌ | ❌ | [WR] per-device call registry and `CallEndedElsewhere`; whether a companion can ring without the primary is a protocol behavior the conformance suite still treats as best-effort. |
| End-to-end encryption of calls | ✅ | ✅ | ➖ | ➖ | Inherent in the signaling + media stack ([WR] HBH-SRTP/SFrame docs); B/WM don't do media at all. |
| "Protect IP address in calls" (relay only) | ✅ | 🟡 | ❌ | ❌ | [App] advanced privacy setting; [WR] relay transport exists (`voip-relay-native`), force-relay policy is app-side. |
| Call quality / network stats | ✅ | ✅ | ❌ | ❌ | [WR] `media_stats`, `relaylatency` handling; surface in UI. |
| Scheduled calls (events with `is_scheduled_call`) | ✅ | ✅ | 🟡 | 🟡 | Same as group events row; [WR] `EventCreationParams::is_scheduled_call`. |
| Call with Meta AI (AI participant) | ✅ | ❌ | ❌ | ❌ | [App] "A participant added Meta AI to this call", `WAAIVoiceViewsBase`. Meta backend; infeasible/low priority. |
| Call waiting / join ongoing call | ✅ | ❓ | ❌ | ❌ | [WR] multiple concurrent calls supported by registry; UI/protocol behavior unverified. |
| Call voicemail | ❌ (not a WhatsApp feature) | ➖ | ➖ | ➖ | — |

---

## K. Contacts, profiles, and usernames

| Feature | Official app | whatsapp-rust | Baileys | whatsmeow | Notes / effort |
| --- | --- | --- | --- | --- | --- |
| Contact list sync from primary device | ✅ | ✅ | ✅ | ✅ | Companion receives contact app-state mutations: [WR] `ContactUpdated`/`ContactSyncRequested` events [B] `contacts.upsert/update` [WM] app-state `ContactAction` → `events.Contact`. (Uploading the macOS address book is a primary-device feature; a companion does not re-upload it.) |
| Add / edit / remove contact | ✅ | ✅ | ✅ | 🟡 | [WR] `save_contact`, `remove_contact` (app-state) [B] `chatModify({ contact })` [WM] reads contact actions, no writer builder (`appstate/encode.go` has none). |
| "Is on WhatsApp" lookup | ✅ | ✅ | ✅ | ✅ | [WR] `is_on_whatsapp` [B] `onWhatsApp` [WM] `IsOnWhatsApp`. |
| Resolve phone number ↔ LID | ✅ | ✅ | ✅ | ✅ | [WR] `LidPnCache`, `add_lid_pn_mapping` [B] LID mapping in usync [WM] `store.LIDs`. |
| Contact QR code (share/add via QR) | ✅ | ❌ | ❌ | ✅ | [WM] `GetContactQRLink`, `ResolveContactQRLink` [App] `WAContactQRViewController`. Effort: S in WR to port from WM. |
| Contact number-change notification | ✅ | ✅ | 🟡 | 🟡 | [WR] `ContactNumberChanged` event. |
| User info (about, devices, business flags) | ✅ | ✅ | ✅ | ✅ | [WR] `get_user_info` [B] `getUSyncDevices`/`onWhatsApp` [WM] `GetUserInfo`. |
| Profile pictures (self/others, preview vs full) | ✅ | ✅ | ✅ | ✅ | [WR] `get_profile_picture(_with_timeout)`, `set_profile_picture`, `remove_profile_picture` [B] `profilePictureUrl`/`updateProfilePicture` [WM] `GetProfilePictureInfo` (set via raw IQ). |
| Own profile: push name, about/status text | ✅ | ✅ | ✅ | ✅ | [WR] `set_push_name`, `set_status_text`, `SelfPushNameUpdated` [B] `updateProfileName`, `updateProfileStatus` [WM] `UpdatePushName`, `SetStatusMessage`. |
| Business profile (address, hours, website, category) | ✅ | ✅ | ✅ | ✅ | [WR] `business.update_profile`, `get_business_profile` [B] `getBusinessProfile`/`updateBusinessProfile` [WM] `GetBusinessProfile`, `updateBusinessName`. |
| Usernames (set / change / delete handle, username key) | ✅ | 🟡 | 🟡 | ❌ | [App] "Add username", "Add a username key to control who can contact you", "%1$@ created the username %2$@" [WR] `Mex::get_username` is **read-only** (doc comment says setting is not exposed) + `contacts.find_by_username`; B exposes username fields in contact metadata only; WM none. Effort: M (MEX mutations). |
| Invite to WhatsApp (contact invite flow) | ✅ | ❌ | ❌ | ❌ | [App] `WAInviteToWhatsApp`. Client-side SMS/link; low effort. |

---

## L. Privacy, security, and account safety

| Feature | Official app | whatsapp-rust | Baileys | whatsmeow | Notes / effort |
| --- | --- | --- | --- | --- | --- |
| Privacy settings read/write (last seen, profile photo, about, status, read receipts, groups, calls) | ✅ | ✅ | ✅ | ✅ | [WR] `fetch_privacy_settings`, `set_privacy_setting`, `set_privacy_disallowed_list` [B] `fetchPrivacySettings`/`updatePrivacySettings` [WM] `GetPrivacySettings`, `SetPrivacySetting`. |
| "Who can add me to groups" / group permission lists | ✅ | ✅ | ✅ | ✅ | Included above (disallowed list / contact_blacklist). |
| Advanced privacy: protect IP in calls | ✅ | 🟡 | ❌ | ❌ | [App] advanced privacy toggle; [WR] relay transport exists but the setting plumbing is app-side. |
| Advanced privacy: disable link previews | ✅ | ✅ | ✅ | ✅ | [WR] `set_link_previews_disabled` [B] `disableLinkPreviews` chat modification [WM] privacy setting. |
| Advanced chat privacy (anti-export/anti-AI per chat) | ✅ | ❌ | ❌ | ❌ | [App] "Advanced chat privacy has been turned on..." No OSS support. Effort: M (app-state + enforcement). |
| Two-step verification (PIN, email) | ✅ | ❌ | ❌ | ❌ | [App] `WATwoFactorEmailViewController`, "Add email in case you forget your two-step verification PIN". Primary-device-side; companion clients typically only display state. Effort: N/A for companion, XL otherwise. |
| Passkey login to the account | ✅ | 🟡 | ❌ | 🟡 | Passkey **linking** is supported ([WR]/[WM]); using a passkey to re-authenticate a desktop app session is not implemented anywhere. |
| App lock (Face ID / Touch ID / device passcode) | ✅ | ➖ | ➖ | ➖ | [App] `WATimelockViewController`, `_secureLockManager`, "Use Face ID to authenticate on WhatsApp". macOS local auth (LocalAuthentication). Effort: S. |
| Chat lock + hidden "Locked chats" folder | ✅ | ➖ | ➖ | ➖ | [App] "Locked chats have moved here", "Save to photos is off for locked chats". Local DB + UI. Effort: M. |
| Security code / identity verification | ✅ | ✅ | ✅ | ✅ | [WR] `features/signal.rs` (`validate_session`, `session_info`), `IdentityChange` event [B] `identity-change-handler.ts` [WM] `IdentityChange`. UI for 60-digit codes = app work. |
| Security notifications (identity key changed) | ✅ | ✅ | ✅ | ✅ | Same events (`IdentityChange`/`identity.change`). |
| End-to-end encryption (default) | ✅ | ✅ | ✅ | ✅ | Signal protocol in all stacks. |
| E2E encrypted backups (iCloud/Google) + "back up with passkey" | ✅ | ❌ | ❌ | ❌ | [App] `WABackupShared`, `WAUnifiedBackup`, "Back up with a passkey". No OSS implementation; backup envelope/keys are server-coupled. Effort: XL, likely infeasible. |
| Backup to iCloud (device-level) | ✅ | ➖ | ➖ | ➖ | OS/phone concern; the Catalyst app relies on the phone. |
| Block / unblock contact or domain | ✅ | ✅ | ✅ | ✅ | [WR] `blocking.rs` (`block`, `unblock`, `get_blocklist`, `is_blocked`) [B] `updateBlockStatus` [WM] `GetBlocklist`, `UpdateBlocklist`. |
| Report spam / abuse | ✅ | ✅ | 🟡 | 🟡 | [WR] `spam_report.rs`; [App] "Report" flows; [B] no dedicated API (raw stanza needed); [WM] reporting tokens exist (`reportingtoken.go`) for telemetry, reporting flow not exposed. |
| Scam / suspicious-link warnings | ✅ | ❌ | ❌ | ❌ | [App] `WASuspiciousLinkWarningDialogViewController`, scam-group-messages BG task. Client/server heuristics; low priority. |
| Account ban handling / temporary ban | ✅ | ✅ | ✅ | ✅ | [WR] `TemporaryBan`, `LoggedOut`, client expiration events [B]/[WM] `TemporaryBan`, `LoggedOut`. |
| Delete account (from device) | ✅ | ❌ | ❌ | ❌ | [App] `DeleteAccount.storyboardc`. Server IQ; not implemented. Effort: M. |
| GDPR data export / account info report | ✅ | ❌ | ❌ | ❌ | [App] `GDPR_REPORT_NOTIFICATION_TEXT`. Server flow. |
| Youth / teen accounts + parental controls | ✅ | ❌ | ❌ | ❌ | [App] `WAYouth`, strings about teen accounts, parent links. Meta server feature; infeasible. |
| Account recovery / companion nonce exchange | ✅ | 🟡 | ❌ | ❌ | [App] `WACanonicalUserManager` companion recovery; [WR] has canonical-user logging/recovery scaffolding. |

---

## M. Search and organization

| Feature | Official app | whatsapp-rust | Baileys | whatsmeow | Notes / effort |
| --- | --- | --- | --- | --- | --- |
| Global search (chats, messages, contacts, channels) | ✅ | ➖ | ➖ | ➖ | Client DB work on top of received events. Effort: L. |
| In-chat / media / link / document search, search-by-date | ✅ | ➖ | ➖ | ➖ | Client DB work. |
| Semantic/AI search | ✅ | ❌ | ❌ | ❌ | [App] `WASemanticSearch`. Server/AI; likely infeasible. |
| Business labels (create/edit, apply to chat/message) | ✅ | ✅ | ✅ | 🟡 | [WR] `features/labels.rs` (create/delete labels, add/remove chat and message labels) [B] chat modifications `addLabel`, `addChatLabel`, `addMessageLabel`, ... [WM] app-state builders `BuildLabel*` only, no client API. |
| Quick replies | ✅ | ✅ | ✅ | ❌ | [WR] `features/quick_replies.rs` (set/delete) [B] `chatModify({ quickReply })` (`Types/Chat.ts:126`, `Utils/chat-utils.ts:728`) [WM] none. |
| Export chat (text archive) | ✅ | ➖ | ➖ | ➖ | [App] "Export chat", "Couldn't export chat". Client-side generation. Effort: S. |
| Archived chats / keep archived | ✅ | ✅ | ✅ | ✅ | See B. |
| Favorites / pinned / unread filters | ✅ | ➖ | ➖ | ➖ | Client-side (plus Lists feature gap). |

---

## N. Business and payments

| Feature | Official app | whatsapp-rust | Baileys | whatsmeow | Notes / effort |
| --- | --- | --- | --- | --- | --- |
| Business catalog (products) | ✅ | ✅ | ✅ | ❌ | [WR] `business.get_catalog` [B] `parseCatalogNode`, `toProductNode` [WM] only order-detail parsing (`business.go`) — no catalog fetch. |
| Collections | ✅ | ✅ | ✅ | ❌ | [WR] `get_collections` [B] `parseCollectionsNode`. |
| Cart / order / checkout | ✅ | 🟡 | ✅ | ✅ | [WR] `get_order` (read) [B] `orderMessage` send + `parseOrderDetailsNode` [WM] `GetOrderDetails`; checkout flow itself is app-side. |
| Share product / catalog message in chat | ✅ | 🟡 | ✅ | 🟡 | Raw proto everywhere; [B] builds `productMessage` with thumbnail upload; [WR]/[WM] proto only. |
| Business directory / discovery | ✅ | ❌ | ❌ | ❌ | [App] `BusinessDirectory`. Effort: L. |
| WhatsApp Flows (form experiences) | ✅ | ❌ | ❌ | ❌ | [App] `WAFlows`, Bloks templates. Effort: XL, business-only. |
| Click-to-WhatsApp lead generation | ✅ | ❌ | ❌ | ❌ | [App] `WALeadGenKit`. Ads business backend. |
| Payments — UPI (India) | ✅ (regional) | ❌ | ❌ | ❌ | [App] `WAPaymentsPICSUI`, UPI PIN flows, `upi://`. Effort: XL and likely infeasible for an unofficial client. |
| Payments — Pix (Brazil) | ✅ (regional) | ❌ | ❌ | ❌ | [App] `WAPaymentsBrazilUI`, Pix key management. Same feasibility caveat. |
| Payment requests / reminders / invites | ✅ | ❌ | ❌ | ❌ | [App] payment request/reminder strings, `paymentInviteMessage`. |
| Payment methods / accounts management | ✅ | ❌ | ❌ | ❌ | [App] "Add a payment method", `WAPaymentsAccounts`. |
| Automatic / recurring payments | ✅ | ❌ | ❌ | ❌ | [App] "Approve automatic payment", scheduled payments. |
| In-app purchases | ✅ | ❌ | ❌ | ❌ | [App] `WAInAppPurchase`. StoreKit on macOS; not protocol. |
| Marketing messages / business broadcast | ✅ | ❌ | ❌ | ❌ | [App] `WAMarketingMessageFeedback`, `WAMO`. |
| Business verification / quality signals | ✅ | 🟡 | 🟡 | 🟡 | `BusinessStatusUpdate` event [WR]; verified-name in profiles everywhere; quality/insights not exposed. |

---

## O. Meta AI and cross-app features

| Feature | Official app | whatsapp-rust | Baileys | whatsmeow | Notes / effort |
| --- | --- | --- | --- | --- | --- |
| Meta AI direct chat | ✅ | 🟡 | ❌ | 🟡 | [WR] `features/bots.rs::list` + generic send/receive with bot message-secret handling [WM] `GetBotListV2`, `GetBotProfiles`, bot send mode with `BotMetadata` in `send.go` [B] none. |
| Meta AI in group chats | ✅ | 🟡 | ❌ | 🟡 | Same as above; group bot invoke proto (`botInvokeMessage`) raw. |
| Meta AI in calls (AI participant) | ✅ | ❌ | ❌ | ❌ | [App] `WAAIVoiceViewsBase`, "A participant added Meta AI to this call". Infeasible without Meta's AI backend. |
| Meta AI media generation ("Imagine") | ✅ | ❌ | ❌ | ❌ | [App] `MAIImagineCanvasTaskHandlerImpl`, `WAImageEditor`. |
| AI stickers | ✅ | ❌ | ❌ | ❌ | [App] `WAAIStickers`. |
| AI summaries / "ask about unread messages" | ✅ | ❌ | ❌ | ❌ | [App] Advanced-chat-privacy strings reference AI summaries. |
| AI handoff (move a chat into Meta AI) | ✅ | ❌ | ❌ | ❌ | [App] "AI handoff" strings, `WAAIChatViewControllers`. |
| AI voice (talk to Meta AI) | ✅ | ❌ | ❌ | ❌ | [App] `WAAIVoiceViewsBase`, `openAIVoiceWithParentViewController`. |
| Private processing (TEE attestation) | ✅ | ❌ | ❌ | ❌ | [App] `Badge_PrivateProcessing_*.json`, `WAAccountEncryptionAttestation`, `WAAIInfraTEEConnection`. |
| Cross-post status to Facebook / Instagram | ✅ | ❌ | ❌ | ❌ | [App] `WACrossFamilyShared`, auto-crossposting stores. Meta Graph APIs; likely infeasible. |
| Instagram username on WhatsApp profile | ✅ | ❌ | ❌ | ❌ | [App] "Adding an Instagram username will make it visible on your WhatsApp profile". |
| Interop: third-party chats (DMA) | ✅ | 🟡 | ❌ | 🟡 | [App] `WAInteropControllers`, "From third-party chat messaging service", "About third-party groups" [WM] `SendFBMessage` + `FBMessage` event (Armadillo transport, `sendfb.go`, `armadillomessage.go`) [WR] `Server::Interop` JID parses, no Armadillo app-layer. Effort: XL; Meta-controlled. |
| Messenger/Instagram messaging bridge | ✅ | 🟡 | ❌ | 🟡 | Same as above (`waMsgApplication`, `instamadilloTransportPayload` in WM). |
| Threads sharing ("barcelona") | ✅ | ❌ | ❌ | ❌ | [App] `barcelona` in `LSApplicationQueriesSchemes`; share-sheet integration only. |
| Invite friends to WhatsApp | ✅ | ❌ | ❌ | ❌ | Client-side share link. |

---

## P. macOS / platform integration

| Feature | Official app | whatsapp-rust | Baileys | whatsmeow | Notes / effort |
| --- | --- | --- | --- | --- | --- |
| Siri: send message | ✅ | ➖ | ➖ | ➖ | [App] `INSendMessageIntent`, `AppIntentVocabulary.plist`, `WAIntentMessageSender`. AppKit/Intents work. Effort: M. |
| Siri: read/search messages | ✅ | ➖ | ➖ | ➖ | [App] `INSearchForMessagesIntent`, `INSetMessageAttributeIntent` (mark read). Effort: M. |
| Siri: audio/video call | ✅ | ➖ | ➖ | ➖ | [App] `INStartAudioCallIntent`, `INStartVideoCallIntent`, `INStartCallIntent`. Effort: M (ties into VoIP layer). |
| App Intents / Shortcuts | ✅ | ➖ | ➖ | ➖ | [App] `Metadata.appintents/extract.actionsdata`: `OpenMetaAIIntent`, `EndMetaAICallIntent`, `ToggleMetaAICallMicIntent`, `ToggleCallingMicIntent`, `EndCallingLiveActivityIntent`, `WAOpenMetaAIAppShortcutsProvider`. Effort: S–M. |
| Widget: quick chat shortcuts | ✅ | ➖ | ➖ | ➖ | [App] `WidgetIntents.intentdefinition` — `SelectChatSource` with Recents/Favorites/Pinned/Frequent. Effort: M. |
| Share sheet / open-with (images, audio, video, docs) | ✅ | ➖ | ➖ | ➖ | [App] `CFBundleDocumentTypes` (public.image/movie/mp3/…), `WAShareExtensionViewController` symbols. Effort: S. |
| Share text out of the app | ✅ | ➖ | ➖ | ➖ | [App] `WATextSharingViewController`, `textSharingViewController`. Effort: S. |
| Menu bar status item | ✅ | ➖ | ➖ | ➖ | [App] `WAAppKitBridge`: `WAAppKitStatusItemBridge`/`Delegate`/`Model`. macOS-specific. Effort: S. |
| Multiple windows / scenes | ✅ | ➖ | ➖ | ➖ | [App] `UIApplicationSupportsMultipleScenes = true`; `WAModalSplitViewController`, `WASettingsSplitViewController`. Effort: M. |
| Rich notifications via Notification Service Extension | ✅ | ➖ | ➖ | ➖ | [App] `ServiceExtension.appex` (`WANotificationService`, E2E status decrypt, media reupload in NSE). Effort: L. |
| Keyboard shortcuts | ✅ | ➖ | ➖ | ➖ | [App] `UIKeyCommand` usage (`escapeSelectChatsKeyCommand`, `toUIKeyCommandWithInput:modifierFlags:action:`). Effort: S. |
| URL schemes / deep links / universal links | ✅ | ➖ | ➖ | ➖ | [App] `whatsapp`, `whatsapp-consumer`, `wa.me`, `chat.whatsapp.com`, `fb306069495113`. Effort: S. |
| State restoration / Handoff (NSUserActivity `net.whatsapp.WhatsApp.chat`) | ✅ | ➖ | ➖ | ➖ | [App] `NSUserActivityTypes`. Effort: S. |
| Auto-update | ✅ | ➖ | ➖ | ➖ | [App] Sparkle (`SUPublicEDKey`, `WASparkleUpdaterBridge`). Use Sparkle or equivalent in the Rust app. Effort: S. |
| OS permission surfaces (camera, mic, photos, contacts, calendars, location, Bluetooth, local network) | ✅ | ➖ | ➖ | ➖ | [App] `Info.plist` usage strings; calendar access is used to add events; local network for calls. Effort: M. |
| Screen capture permission for screen sharing | ✅ | ➖ | ➖ | ➖ | [App] `WAAppKitDesktopScreenCaptureBridge`, `WADesktopCapturerServiceProtocol`. Effort: M (ScreenCaptureKit). |
| Background/keep-alive behavior (`voip`, `audio`, `fetch`, `processing`, `remote-notification`, …) | ✅ | 🟡 | 🟡 | 🟡 | Libraries keep a websocket; a macOS app needs background modes/agent behavior and reconnect policy. [WR] has reconnect/offline-resume machinery; UI shell must opt into platform background modes. |
| Notification Center actions (reply, mark read, quick react) | ✅ | ➖ | ➖ | ➖ | [App] `UNUserNotificationCenter` categories, `WANotificationControlsViewController`. Effort: M. |

---

## Protocol gaps

Two different questions hide behind "gaps": what the Rust stack (`whatsapp-rust`) cannot do yet, and
what *no* open-source implementation covers. The second list is short and mostly server-owned.

### G1. Gaps in whatsapp-rust specifically (feasible, not implemented)

| Gap | What it would take |
| --- | --- |
| HD photo/video quality | Build the dual upload (standard + HD) and set the quality fields WA Web sets; both variants are already uploadable through `Upload`. ~days–week. |
| Sticker builder + WebP encode | WebP decode/encode (e.g. `image-webp`) and a `StickerMessage` helper; `webp` module currently only detects animation. |
| PTV (video note) helper | `PtvMessage` + the matching media meta attributes; classify code has no PTV branch today. |
| Location / contact-card helpers | Thin builders over existing message structs and upload; receive path already parses them. |
| Live-location session | Periodic re-encryption + message edits + expiry handling; protocol shape exists, no orchestration. |
| Broadcast list send | What WA Web itself no longer supports; low value (see G2). |
| Scheduled messages | Needs the companion scheduled-message protocol (the official app shows `...ScheduledMessagePlaceholder` flows). Capture work, then a scheduler on top of `send_message`. |
| Chat lists / custom filters | App-state action (`WALists`) is absent from every OSS stack; requires schema reverse engineering from a real account. |
| Usernames write path | MEX mutation discovery; `get_username` is read-only today (`src/features/mex.rs`). |
| Status audiences (close friends, custom lists) + group statuses | Audience app-state + `GroupStatus` protobuf adapter; mentions and questions are further new formats. |
| Channel insights | MEX ops not yet generated. |
| Message translation / voice transcripts | Server-side or platform features; no known public stanza. Treat as research asks, not commitments. |
| 1:1 video / group-call / screen-share conformance | Code exists; the conformance gate (`agent_docs/voip_conformance.md`) still lists callback traces and slow signaling scenarios as pending. This is verification work, not greenfield. |

### G2. Features no open-source implementation covers (any language)

| Feature | Why it is a gap / what implementing requires | Feasibility |
| --- | --- | --- |
| WhatsApp Payments (UPI in India, Pix in Brazil, cards, automatic payments) | Proprietary rails, banking partners, regional licensing, PIN/HSM flows. The official app only enables it in specific countries. | 🚩 Infeasible for an unofficial client. |
| Meta AI suite (Imagine, AI voice, summaries, handoff, AI stickers, private processing/TEE) | Meta-proprietary backend; `WAAIInfraTEEConnection` implies hardware attestation. Generic bot messaging is *not* the same product. | 🚩 Infeasible / very high risk. |
| Full interop (DMA third-party chats, Messenger/Instagram bridging) | whatsmeow's Armadillo work (`sendfb.go`) is the only OSS foothold and is reverse-engineered, unstable, and account-risky; full interop is Meta-gated by policy/agreements. | 🚩 Meta-controlled; partial only. |
| E2E encrypted backups ("back up with passkey") | Server-coupled backup envelope, key derivation and escrow; no public protocol. `WABackupShared`/`WAUnifiedBackup` are app internals. | 🚩 High risk, likely infeasible. |
| Two-step verification management from a companion | The 2FA setting lives on the primary device; companion clients display state and enter the PIN when linking, but cannot enable/disable it. Not a protocol gap so much as an architecture fact. | ❌ Not applicable. |
| Broadcast lists | WhatsApp Web dropped them; whatsmeow returns `ErrBroadcastListUnsupported`; status broadcasts are the only supported fan-out. | ❌ Drop. |
| Voice-message transcription | Server/OS; no stanza known. | ⚠️ Unknown. |
| Message translation | Server-side; no stanza known publicly. | ⚠️ Unknown. |
| Cross-posting to Facebook/Instagram, Instagram username on profile | Meta Graph APIs + cross-family accounts. | 🚩 Meta-gated. |
| Business: Flows (Bloks), directory, lead-gen, marketing messages | Bloks is a Meta-hosted UI runtime; the rest are ads/business backends. | 🚩 Mostly out of scope. |
| Teen/youth accounts and parental controls | Server policy + Meta family accounts. | 🚩 Meta-gated. |
| Semantic/AI search | Server AI. | 🚩 Meta-gated. |
| Call media beyond whatsapp-rust | SRTP/SFrame/HBH-SRTP, DTLS/SCTP data channels, MLOW codec, H.264 packetization, relay signaling — whatsapp-rust has done the heavy lifting; Baileys/whatsmeow have none of it and would need to port an entire stack. | ⚠️ XL for non-Rust stacks; already largely done in Rust. |

### G3. Cross-cutting risks

- **Account bans / ToS.** Every stack here is unofficial. Shipping a client at scale risks bans; the
  official app's protocol is also actively changed.
- **A/B gates.** Many features found in the binary (AI, teen accounts, capping, interop) are gated by
  server flags; being "in the binary" does not mean being enabled for an account.
- **Dead code.** Symbol evidence is noisy: e.g. the app's avatar strings say the feature was retired,
  and whatsapp-rust keeps retired event variants for serialization stability.
- **Platform policy:** Siri/Shortcuts/notifications are AppKit/Intents work with Apple entitlements,
  not protocol work.

---

## Proposed milestone ordering (most valuable first, dependency-ordered)

The foundation below already exists in whatsapp-rust; the milestones are the **desktop app shell**
work plus the missing library pieces, ordered so each milestone is shippable.

| # | Milestone | Contents | Depends on | Risk |
| --- | --- | --- | --- | --- |
| M0 | Core connection | QR/pair-code/passkey linking, session persistence, history sync, reconnect/offline resume, media pipeline (upload/download/encrypt). | — | Low (already in `whatsapp-rust`). |
| M1 | Everyday messaging | Text, replies, mentions, formatting, receipts, typing/recording, presence, unread counts, notification plumbing, drafts. | M0 | Low. |
| M2 | Chat management | Archive/pin/mute/delete/clear/mark-read, starred, search over local DB, chat lists (G1), favorites, export chat. | M1, local DB | Medium (lists need app-state discovery). |
| M3 | Media experience | Images/videos/GIF/voice/documents/stickers/PTV/albums, view-once, disappearing, captions, thumbnails, auto-download settings, save-to-gallery, HD (G1). | M1 | Medium (editors/transcodes are UI-heavy). |
| M4 | Message actions | Reactions, edit, revoke, delete-for-me, pin, keep-in-chat, forwarding, polls/quizzes, events/RSVP. | M1 | Low–Medium (mostly in `whatsapp-rust`). |
| M5 | Groups & communities | Full group admin, invite links, join requests, events; community create/link/join. | M2, M4 | Low (library complete). |
| M6 | Status & channels | Status post/view/privacy/reactions (+ group statuses, music, questions as stretch), channel follow/post/react/comments, directory. | M3 | Medium (audience app-state gap). |
| M7 | Calls | 1:1 audio first, then video and screen share, call links, waiting room, group calls, call history sync, miss/ended-elsewhere states. | M0 (`voip` features), M1 for notifications | High (video/group conformance pending; macOS capture permissions). |
| M8 | Privacy & security | Privacy settings UI, block/report, app lock, chat lock, identity verification, passkey flows, advanced privacy, proxy. | M1–M2 | Medium. |
| M9 | System integration | Siri intents, App Intents/Shortcuts, widgets, share sheet, menu bar item, multiple windows, notification extension, keyboard shortcuts, auto-update, deep links. | M1–M4 (actions must exist for Siri/notifications) | Medium (Apple APIs). |
| M10 | Business & power features | Business profiles, catalog/collections/orders, labels, quick replies, usernames (read then write), channel insights. | M2–M4 | Medium. |
| M11 | Research / high-risk | Scheduled messages, live location, message translation/transcripts, HD quality if not earlier. | M1–M3 | Medium–High (protocol discovery). |
| M12 | Explicitly out of scope | Payments, Meta AI suite, interop, E2E backups, cross-posting, teen accounts. | — | 🚩 Infeasible or Meta-gated; revisit only with a partnership. |

**Ordering rationale.** M1–M4 are the utility core and all have library support today. M5–M6
monetize existing library capabilities with mostly UI work. M7 is the largest remaining engineering
lift and gates the Siri call intents in M9, so it comes before shell polish. M10’s catalog/orders
and labels are cheap wins that depend on M2’s app-state plumbing. M11 is deliberately last because
each item needs protocol capture before it can be scheduled; treating them as “known unknowns”
avoids promising dates.

**Infeasibility flags.** Payments, Meta AI, interop, and backups are flagged 🚩 in the tables above.
If the goal is literal 100% parity with the official client, those four are the items most likely to
remain permanently behind, and the product should decide how the UI behaves when they are absent
(e.g. hide the Payments tab, show a “not supported” state for Meta AI entry points).

---

## Verification

### Inspected directly (read-only)

**Official app (WhatsApp 26.33.73, build 1049819294):**
- `Contents/Info.plist` and extension `Info.plist`s via `plutil`: `UIBackgroundModes`
  (`voip`, `audio`, `bluetooth-central`, `fetch`, `location`, `processing`,
  `remote-notification`), `NSUserActivityTypes`, `CFBundleDocumentTypes`, exported UTIs,
  `LSApplicationQueriesSchemes`, permission usage strings.
- `strings -a` over `SharedModules.framework/Versions/A/SharedModules` (1,176,420 lines),
  `Contents/MacOS/WhatsApp` (2,252,305 lines), `WAAppKitBridge`, `Intents.appex`, and
  `ServiceExtension.appex`; extracted 2,925 `WA*` class-name lines, 368 `*ViewController`
  names, and demangled Swift module inventories (235 modules in SharedModules, 1,366 in the
  main binary).
- `Intents.appex` metadata: `IntentsSupported` = `INSearchForMessagesIntent`,
  `INSendMessageIntent`, `INSetMessageAttributeIntent`, `INStartAudioCallIntent`,
  `INStartCallIntent`, `INStartVideoCallIntent`, `SelectChatSourceIntent`;
  `AppIntentVocabulary.plist`; `Metadata.appintents/extract.actionsdata` (Meta AI and call
  App Intents); `Base.lproj/WidgetIntents.intentdefinition`.
- `en.lproj/Localizable.localite.values` (19,276 extracted UI strings, quoted throughout),
  `Localizable.strings` (notification strings), and the framework resource listing
  (MMd models, wallpapers, Lottie assets, fonts).
- Feature-specific greps for proto message names (`pollCreationMessage`, `eventMessage`,
  `albumMessage`, `ptvMessage`, `keepInChatMessage`, `liveLocationMessage`,
  `scheduledCallCreationMessage`, `richResponseMessage`, …) to confirm support in the official
  client.

**Open-source stacks (shallow clones in a temp directory, no project working tree was touched):**
- **whatsapp-rust** `v0.7.0` @ `6502b871` (2026-09-11): `Cargo.toml` feature definitions
  (`voip`, `voip-runtime`, `voip-relay-native`, `voip-mlow`, `voip-libopus`, `passkey`,
  `plugins`, `sqlite-storage`); `src/features/*` public APIs; `src/send/*`, `src/message/*`,
  `src/voip/*` (facade, video, audio, transport), `src/client/*`, `src/download.rs`,
  `src/upload.rs`, `src/media.rs`, `src/passkey/*`, `src/receipt.rs`, `src/history_sync.rs`,
  `wacore/src/types/events.rs` (`Event`/`EventKind`), and `agent_docs/` (especially
  `voip_conformance.md`, `voip_oracle_status.md`, `call_test_support.md`,
  `voip_media_oracle.md`, `wa_web_reference.md`).
- **Baileys** `7.0.0-rc14` @ `0af23862` (2026-08-04): `README.md` feature index and sections,
  `package.json`, `src/Socket/*` (`messages-send.ts`, `messages-recv.ts`, `chats.ts`,
  `groups.ts`, `communities.ts`, `newsletter.ts`, `business.ts`), `src/Utils/*`
  (`link-preview.ts`, `messages-media.ts`, `messages.ts`, `process-message.ts`,
  `chat-utils.ts`, `business.ts`, `companion-reg-client-utils.ts`), `src/Types/*`.
- **whatsmeow** @ `b25a56d6` (2026-09-09): `README.md`, `send.go`, `sendfb.go`,
  `armadillomessage.go`, `group.go`, `user.go`, `newsletter.go`, `business.go`, `broadcast.go`,
  `call.go`, `pair-passkey.go`, `pair-code.go`, `privacysettings.go`, `presence.go`,
  `receipt.go`, `msgsecret.go`, `mediaconn.go`, `download.go`, `upload.go`, `message.go`,
  `notification.go`, `appstate.go`, `appstate/encode.go`, `types/events/*`.

### Not executed / limits

- No WhatsApp account was connected, no server traffic was generated, and no call was placed. All
  library support levels come from source and documentation inspection, not live testing.
- The official app was not launched or driven; UI behavior is inferred from binaries, plists, and
  localization strings. Symbol names can reflect dead, experimental, or A/B-gated code (e.g.
  Avatars are marked retired in the strings; some modules come from SharedModules used by multiple
  WhatsApp apps). Treat ❓-marked rows and feature-gated items as candidates for a follow-up smoke
  test on a real account.
- `whatsapp-rust`'s docs website and Baileys' wiki/godoc were not fetched; README and source were
  the authority. Package versions are from the clones above; upstream may have moved since.
- Region/platform availability of features (e.g. payments, channels, usernames) was not evaluated.
- No git command was run inside `/Volumes/lucas-sn770/Projecten/whatsapp-rust`; the OSS clones live
  under a temp directory. `/Applications/WhatsApp.app` was only read.

### Deliverable placement note

The requested deliverable path is `docs/parity-matrix.md`; an identical copy is kept at
`docs/research/parity-matrix.md`. The temp inspection artifacts (strings dumps, class lists) are
outside the repo and can be discarded.

