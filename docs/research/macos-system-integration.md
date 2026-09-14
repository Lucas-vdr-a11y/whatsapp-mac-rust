# macOS system integration (M9) — Siri / App Intents / Shortcuts, widgets, multiple windows, notification click-through, share sheet

Status: **research only** — no code changes. Written 2026-09-13 against RustWA `0.1.0`
(`apps/desktop`, Tauri `2.11.5`) and WhatsApp for macOS `26.33.73` (build `1049819294`).

Every claim is tagged:

- **[verified]** — reproduced on this machine by reading a file, plist, binary, source tarball
  already in the Cargo registry, or a first-party doc page.
- **[reported]** — from a third-party doc/issue that was fetched, not reproduced locally.
- **[unverified]** — reasoning or expectation that still needs a spike.

The companion evidence log is Appendix A. All WhatsApp.app inspection was read-only
(`plutil`, `strings`, `nm`, `otool`, `codesign -d`, `pluginkit -m`); nothing in
`/Applications/WhatsApp.app` was modified.

---

## 1. Executive summary

| # | Feature | RustWA today | Recommended approach | Effort (engineer-days) | Biggest risk |
|---|---|---|---|---|---|
| 3 | Multiple windows | single `main` window | Tauri `WebviewWindowBuilder` + per-window routing; encode JIDs in window labels | 4–7 | duplicate notifications / per-window state divergence |
| 4 | Notification click-through | `notify_for_chat` stores `chatId` but the desktop plugin drops it; no click events | in-repo `objc2-user-notifications` backend + `UNUserNotificationCenterDelegate` | 3–5 | `UNUserNotificationCenter` does not work for the unbundled dev binary |
| 1 | Siri / App Intents / Shortcuts | none | ExtensionKit `.appex` (`com.apple.appintents-extension`) + `rustwa://` handoff | 5–9 | App Intents metadata/extraction + Siri surface on macOS |
| 5 | Share sheet | none (file associations in progress) | `com.apple.share-services` `.appex` + App Group/URL handoff | 4–6 (text/URL); +4–6 for files | App Group entitlement needs paid provisioning |
| 2 | Widgets | none | WidgetKit `.appex` + App Group snapshot written by the Rust host | 8–15 | same App Group/provisioning gate + no WidgetKit bridge from Rust |

Recommended order (rationale in §8): **multiple windows → notification click-through →
App Intents/Shortcuts → share sheet → widgets**.

None of the five features has a first-party Tauri plugin. The Tauri team's own widget
request (`tauri-apps/tauri#9766`) is open and says there is no way to ship widgets with
Tauri today **[reported]**; a search of the plugin directory, crates.io, and `awesome-tauri`
found no App Intents, WidgetKit, or macOS share-extension plugin **[verified]**.

The code sketches below are illustrative: they show API shape and handoff strategy, not
compile-checked code. Compile each sketch against the pinned crate/SDK versions in the
first implementation spike.

---

## 2. Shared foundation: native `.appex` bundles inside a Tauri app

Everything except multiple windows depends on this section. Get it right once and
features 1, 2 and 5 reuse the same build harness.

### 2.1 Extension kinds, locations and `Info.plist` keys

Two different extension runtimes are in play. Apple's older `NSExtension` mechanism and
the newer ExtensionKit (`EXAppExtensionAttributes`). They live in **different directories**
and use **different Info.plist keys** — mixing them up is the classic failure mode.

| Kind | Mechanism | Bundle location | Key | `Info.plist` fragment | Verified example |
|---|---|---|---|---|---|
| App Intents / Shortcuts | ExtensionKit | `Contents/Extensions/` | `EXAppExtensionAttributes.EXExtensionPointIdentifier` | `<key>EXAppExtensionAttributes</key><dict><key>EXExtensionPointIdentifier</key><string>com.apple.appintents-extension</string></dict>` | Color Picker `Intents Extension.appex`, Safari `SafariLinkExtension.appex` **[verified]** |
| Widget (WidgetKit) | `NSExtension` | `Contents/PlugIns/` (Microsoft/CodexBar/Goodnotes) or `Contents/Extensions/` (Safari, macOS 26) | `NSExtension` | `<key>NSExtension</key><dict><key>NSExtensionPointIdentifier</key><string>com.apple.widgetkit-extension</string></dict>` | Outlook `CalendarWidgetExtension.appex`, CodexBar, Safari **[verified]** |
| Share | `NSExtension` | `Contents/PlugIns/` | `NSExtension` | `NSExtensionPointIdentifier = com.apple.share-services`, `NSExtensionPrincipalClass`, `NSExtensionAttributes.NSExtensionActivationRule` | Tailscale `ShareExtension-macsys.appex`, Affinity, OneNote **[verified]** |
| SiriKit legacy intents | `NSExtension` | `Contents/PlugIns/` | `NSExtension` | `NSExtensionPointIdentifier = com.apple.intents-service`, `NSExtensionPrincipalClass`, `IntentsSupported` | WhatsApp `Intents.appex` **[verified]** |

Facts that follow from the table:

- The modern App Intents route is an **ExtensionKit extension in `Contents/Extensions`**,
  not an `NSExtension` in `PlugIns`. ExtensionKit extensions must not be placed in
  `PlugIns` **[reported]** — we have two on-disk examples in the right place **[verified]**.
- Widgets and share extensions are still `NSExtension` bundles in `Contents/PlugIns`;
  use that location for maximum macOS-version compatibility even though Safari ships its
  widget in `Contents/Extensions` on macOS 26 **[verified]**.
- The extension identifier convention is the host bundle id plus a suffix:
  `com.sindresorhus.Color-Picker.Intents-Extension` under `com.sindresorhus.Color-Picker`,
  `io.tailscale.ipn.macsys.share-extension` under `io.tailscale.ipn.macsys` **[verified]**.
  RustWA should use `io.github.lucas-vdr-a11y.rustwa.Intents`, `.Widget`, `.Share`.

### 2.2 Building the extension

There is no Tauri-side Swift toolchain integration. The pragmatic options:

1. **A checked-in Xcode project** (`apps/desktop/src-tauri/macos/RustWAExtensions.xcodeproj`)
   with one target per extension, built by `xcodebuild` from a script. Xcode handles
   `Metadata.appintents` extraction, code signing, and the correct product type.
   This is the only low-friction way to produce App Intents metadata.
2. A hand-rolled `swiftc` + `plutil` script. Works for trivial extensions, but the
   App Intents metadata file (`Metadata.appintents/extract.actionsdata`) is produced by an
   undocumented Xcode tool, `appintentsmetadataprocessor`, which exists at
   `/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin/appintentsmetadataprocessor`
   **[verified]**. Running it by hand is possible but fragile.

Recommendation: option 1. The Xcode project lives in the repo; the build script builds into
`apps/desktop/src-tauri/macos/build/<Configuration>/`, which is gitignored. Xcode's
"App Intents Extension", "Widget Extension", and "Share Extension" targets generate exactly
the `Info.plist` fragments in §2.1.

Xcode 26's widget template generates only
`NSExtension.NSExtensionPointIdentifier = com.apple.widgetkit-extension` **[verified]**,
with the widget bundle discovered via `@main` — no `NSExtensionPrincipalClass` needed.
The share-extension template does need a principal class. The App Intents extension has no
principal class; it is a Swift `@main` conforming to `AppIntents.AppIntentsExtension`
(macOS 13+, `AppIntentsExtension : ExtensionFoundation.AppExtension` in the SDK
`swiftinterface`) **[verified]**.

Build/launch facts:

- The system only loads extensions from a real `.app` bundle. `cargo tauri dev` runs
  `target/debug/rustwa` directly, so **no extension will be visible in dev mode** and any
  `UNUserNotificationCenter`-based notification code will fail **[verified in the community
  plugin's macOS code, which rejects unbundled processes]**.
- Test builds must use `cargo tauri build --debug --bundles app` (or a release build),
  then register the bundle with LaunchServices if the system does not pick it up:
  `lsregister -f path/to/RustWA.app`; verify registration with
  `pluginkit -m -p com.apple.widgetkit-extension` (or the relevant extension point)
  **[verified — pluginkit lists WhatsApp's two extensions and the Safari/Color Picker App
  Intents extensions on this machine]**.

### 2.3 Tauri bundler integration

`tauri.conf.json > bundle > macOS > files` maps *destination under `Contents/`* →
*source path relative to `src-tauri/`*, and directories are copied recursively. This was
read from the current `tauri-bundler` source (`copy_custom_files_to_bundle` and its unit
tests, `crates/tauri-bundler/src/bundle/macos/app.rs`) **[verified]**:

```jsonc
// apps/desktop/src-tauri/tauri.conf.json (excerpt, proposed)
"bundle": {
  "macOS": {
    "files": {
      "Extensions/RustWAIntents.appex": "macos/build/Release/RustWAIntents.appex",
      "PlugIns/RustWAWidget.appex":    "macos/build/Release/RustWAWidget.appex",
      "PlugIns/RustWAShare.appex":     "macos/build/Release/RustWAShare.appex"
    }
  }
},
"build": {
  "beforeBundleCommand": "bash macos/build-extensions.sh"
}
```

`beforeBundleCommand` is documented in the config schema as "a shell command to run before
the bundling phase in `tauri build` kicks in" **[verified in `tauri-utils`]**.

Signing is the sharp edge:

- The bundler signs **only** its own `sign_paths` (frameworks, external binaries) plus the
  app bundle; files added through `macOS.files` are **not** signed **[verified from
  `tauri-bundler` source]**.
- The actual `codesign` invocation is `codesign --force -s <identity> [--options runtime]
  [--entitlements <plist>] <path>` — **no `--deep`** **[verified from
  `tauri-macos-sign/src/keychain.rs`]**.
- If no signing identity is configured, Tauri does not sign the bundle at all
  **[verified]**. Local builds therefore rely on the linker's ad-hoc signature on the main
  binary and on whatever signature the Xcode-built `.appex` carries.
- Notarization runs after signing when Apple credentials are present **[verified]**.
  A Developer ID-signed app containing an ad-hoc-signed extension will fail notarization
  (nested code must be signed with the same team and hardened runtime), so the extension
  build script must sign each `.appex` with the same identity when one is configured —
  e.g. read `APPLE_SIGNING_IDENTITY` / `APPLE_CERTIFICATE` and pass the identity through
  to `codesign` inside the Xcode build (`CODE_SIGN_IDENTITY`).
- `codesign --force` on the outer app seals the already-signed extension; it does not
  re-sign it. Correct order: sign extensions (inside out), then let Tauri sign the app
  **[verified from bundler order: `copy_custom_files_to_bundle` runs before `sign`]**.

### 2.4 What breaks in a non-notarized local build

RustWA currently ships no signing identity, no entitlements file, and no provisioning
profile. That has concrete consequences:

| Capability | Local ad-hoc build | Notes |
|---|---|---|
| Extension loads from the `.app` | ✅ expected | Ad-hoc signatures are valid locally; Xcode debug builds are ad-hoc. Must test in a bundled build, never `tauri dev`. **[unverified until spiked]** |
| Deep-link handoff (`rustwa://`) | ✅ | Already works; the scheme is added to `CFBundleURLTypes` by the Tauri bundler from `plugins.deep-link` **[verified in bundler source]** |
| Local notifications + click-through | ✅ | Local notifications need no entitlement. `UNUserNotificationCenter` requires a bundled `.app`. |
| Shortcuts actions / Siri | ⚠️ | Metadata must be present; whether Siri/Shortcuts surface an un-notarized app is **[unverified]** |
| Widget shows data from the app | ❌ | App Group is a **restricted entitlement**: it must be authorized by a provisioning profile, which requires a paid team; it cannot be provisioned for ad-hoc/local builds **[reported from Apple TN3125/forum guidance; expected to apply]**. The widget can still load and show a static/placeholder state. |
| Share extension file handoff via App Group | ❌ | same gate; URL-only handoff still works |
| Communication Notifications (Focus/Time Sensitive) | ❌ | `com.apple.developer.usernotifications.communication`, requires Apple approval. Not needed for click-through. **[verified that WhatsApp has it; not needed for plain notifications]** |
| Distribution to other Macs | ❌ | Notarization (Developer ID) is required for a clean first launch on other machines. |

App Group naming: Apple documents macOS app groups as `<Developer team ID>.<group name>`
**[reported — Apple "Configuring app groups"]**. On-disk evidence on this machine shows both
conventions: CodexBar uses `Y5PE65HELJ.com.steipete.codexbar` (team-ID style, Developer ID
distribution with Sparkle), while Goodnotes/Outlook use `group.*` names **[verified]**. The
value must match the provisioning profile; pick whichever style the Apple Developer portal
assigns.

---

## 3. Siri / App Intents / Shortcuts

### 3.1 Current state in RustWA

- No Swift, no `.appex`, no App Intents metadata, no `AppIntentVocabulary.plist`.
- `apps/desktop/src-tauri/src/deep_link.rs` already parses `rustwa://chat/<jid>` and
  `wa.me` / `api.whatsapp.com/send` links, emits `ui://open-chat`, and has a cold-start
  buffer drained by the `deep_link_ready` command **[verified in repo]**.
- `tauri.conf.json` registers the `rustwa` scheme; the bundler writes `CFBundleURLTypes`.
- There is no URL action that composes or sends a message (`rustwa://send` or equivalent).
- `platform_capabilities` has no flag for App Intents/Shortcuts.

### 3.2 How the official WhatsApp Mac app does it

Read-only inspection of `/Applications/WhatsApp.app`:

- `Contents/PlugIns/Intents.appex` — a legacy **SiriKit** `com.apple.intents-service`
  extension, principal class `WAIntentHandler`, with
  `IntentsSupported = [INSearchForMessagesIntent, INSendMessageIntent,
  INSetMessageAttributeIntent, INStartAudioCallIntent, INStartCallIntent,
  INStartVideoCallIntent, SelectChatSourceIntent]` **[verified]**. It links
  `Intents.framework` and UIKit (it is a Mac Catalyst binary) **[verified via `otool -L`]**.
- `Info.plist` of the host declares `NSUserActivityTypes` for the same intent classes plus
  `net.whatsapp.WhatsApp.chat`, and `NSSiriUsageDescription` **[verified]**.
- `en.lproj/AppIntentVocabulary.plist` defines Siri example phrases for the six intents,
  e.g. "Send a WhatsApp message to Alice saying I'll be there in 15 minutes" **[verified]**.
- `Contents/Resources/Metadata.appintents/extract.actionsdata` is present in the **main app
  bundle** (not an extension) and contains exactly five App Intents, all Meta AI related
  (`OpenMetaAIIntent`, `EndMetaAICallIntent`, `ToggleMetaAICallIntent`,
  `EndCallingLiveActivityIntent`, `ToggleCallingMicIntent`) plus one
  `autoShortcutProvider` (`WAOpenMetaAIAppShortcutsProvider`, phrases "Open Meta AI in
  `${applicationName}`" etc.) **[verified]**. In other words: WhatsApp's message/read/call
  Siri support is SiriKit, not App Intents; App Intents are only used for Meta AI.
- `Base.lproj/WidgetIntents.intentdefinition` (in both the app and `Intents.appex`
  resources) defines `SelectChatSource` with `INIntentEligibleForWidgets = true` and a
  `ChatSource` enum (recents/favorites/pinned/frequents) **[verified]** — legacy config
  intent for their iOS widget, not an App Intent.

### 3.3 What a Tauri app needs

**Recommended architecture: an ExtensionKit App Intents extension plus URL-scheme handoff.**
The extension runs out of process; it never touches RustWA's protocol state. Its job is to
turn a Siri/Shortcuts invocation into a `rustwa://` open, exactly what `deep_link.rs`
already consumes. `openAppWhenRun` (or `supportedModes` on macOS 26+) brings the app to the
foreground.

**Bundle structure**

```text
RustWA.app/Contents/
├── Extensions/
│   └── RustWAIntents.appex/
│       ├── Contents/
│       │   ├── Info.plist
│       │   ├── MacOS/RustWAIntents            (Mach-O, signed)
│       │   └── Resources/Metadata.appintents/extract.actionsdata   (Xcode-generated)
```

`Info.plist` (the only required keys beyond the standard bundle keys) **[verified against
Color Picker/Safari on disk]**:

```xml
<key>EXAppExtensionAttributes</key>
<dict>
  <key>EXExtensionPointIdentifier</key>
  <string>com.apple.appintents-extension</string>
</dict>
<key>CFBundleIdentifier</key>
<string>io.github.lucas-vdr-a11y.rustwa.Intents</string>
<key>CFBundlePackageType</key>
<string>XPC!</string>
<key>LSMinimumSystemVersion</key>
<string>13.0</string>
```

**Entitlements:** none required. Color Picker's App Intents extension is sandbox-only
(`com.apple.security.app-sandbox`) **[verified]**; a locally signed extension can omit even
that.

**Swift sketch** (entry point + intents + App Shortcuts provider). API availability checked
against the macOS SDK `AppIntents.swiftinterface`: `AppIntentsExtension` and
`AppShortcutsProvider` are macOS 13+, `openAppWhenRun` is macOS 13–26 (deprecated in 26 in
favor of `supportedModes`) **[verified]**:

```swift
import AppIntents
import AppKit

@main
struct RustWAIntentsExtension: AppIntentsExtension {}

struct OpenChatIntent: AppIntent {
    static var title: LocalizedStringResource = "Open Chat"
    // macOS 13–15 path; on macOS 26 prefer `supportedModes = .foreground(.dynamic)`
    static var openAppWhenRun: Bool = true

    @Parameter(title: "Chat name or phone number")
    var chat: String

    func perform() async throws -> some IntentResult {
        var components = URLComponents()
        components.scheme = "rustwa"
        components.host = "chat"
        components.queryItems = [URLQueryItem(name: "chat", value: chat)]
        if let url = components.url {
            NSWorkspace.shared.open(url)   // hand off to the running or cold-started app
        }
        return .result()
    }
}

struct ComposeMessageIntent: AppIntent {
    static var title: LocalizedStringResource = "Compose WhatsApp Message"
    static var openAppWhenRun: Bool = true

    @Parameter(title: "Chat name or phone number")
    var chat: String
    @Parameter(title: "Message")
    var text: String

    func perform() async throws -> some IntentResult {
        var components = URLComponents()
        components.scheme = "rustwa"
        components.host = "compose"
        components.queryItems = [
            URLQueryItem(name: "chat", value: chat),
            URLQueryItem(name: "text", value: text),
        ]
        if let url = components.url { NSWorkspace.shared.open(url) }
        return .result()
    }
}

struct RustWAShortcutsProvider: AppShortcutsProvider {
    static var appShortcuts: [AppShortcut] {
        AppShortcut(intent: OpenChatIntent(),
                     phrases: ["Open a chat in \(.applicationName)",
                               "Open \(\.$chat) in \(.applicationName)"],
                     shortTitle: "Open chat",
                     systemImageName: "bubble.left")
        AppShortcut(intent: ComposeMessageIntent(),
                     phrases: ["Send a message on \(.applicationName)"],
                     shortTitle: "Compose message",
                     systemImageName: "square.and.pencil")
    }
}
```

Notes and caveats:

- Contact/chat resolution. `@Parameter(title:) var chat: String` keeps v1 simple: the app
  resolves the string via its own contacts store when the deep link arrives, and opens the
  search UI when ambiguous. A structured `AppEntity` contact picker (Siri-level contact
  resolution) is a significantly larger job and can be deferred.
- "Send without opening" is not achievable this way: `perform()` runs in the extension,
  which has no access to RustWA's protocol session. Only the app can send. This matches
  how the deep link flow works today; the UX difference is acceptable for a first cut and
  should be documented in the Shortcuts action description ("opens RustWA").
- The app must accept the new links. `deep_link.rs` needs `rustwa://open?chat=…` and
  `rustwa://compose?chat=…&text=…` parsing (or extend `rustwa://chat/<jid>` with a `text`
  query). Query items are used in the sketch so contact names with spaces survive
  encoding. The existing `DeliveryState` buffer and `ui://open-chat` event are the delivery
  path; compose needs a new `ui://compose` event or an extended `OpenChatPayload`.
- Siri phrases require `${applicationName}`; Shortcuts discovers `AppShortcutsProvider`
  automatically. Whether **Siri on macOS** surfaces third-party App Shortcuts from a
  locally signed, non-notarized build is **[unverified]** — Shortcuts.app is the safer
  first target.
- Do **not** copy WhatsApp's SiriKit route unless Siri parity is explicitly required.
  Reproducing it means an `intents-service` extension with a `WAIntentHandler`-equivalent
  principal class, `IntentsSupported`, an `.intentdefinition` file and
  `AppIntentVocabulary.plist`, plus custom intent handlers — more code, an older framework,
  and Siri-only visibility on macOS **[verified that WhatsApp uses this; effort L]**.

**Tauri plugin availability:** none. This is a small amount of Swift plus build wiring; no
plugin is needed or exists **[verified]**.

### 3.4 Effort, risks, non-notarized behavior

Effort (one engineer): **5–9 days**, depending on whether the shared `.appex` build harness
(§2.2–2.3) already exists.

- day 1: Xcode project + Intents extension target + `xcodebuild` script + Tauri files
  mapping; verify the extension registers and `Metadata.appintents` is produced.
- days 2–3: intents + shortcuts provider + URL parsing in Rust (`open`, `compose`) +
  UI handling of the compose event.
- days 4–5: build/sign plumbing (identity passthrough, entitlements), cold-start and
  running-app handoff tests.
- buffer: Siri/Shortcuts discovery, macOS 26 `supportedModes` transition, docs.

Risks:

| Risk | Severity | Mitigation |
|---|---|---|
| No `Metadata.appintents` → Shortcuts never sees the actions | high | build with Xcode; assert the file exists in the appex after bundling; fail the build otherwise |
| Extension not loaded because the host is unsigned/ad-hoc | medium | verify locally early; keep extension ad-hoc; add Developer ID signing when distribution starts |
| Siri does not list App Shortcuts on macOS for this app | medium | treat Shortcuts as the primary surface; test Siri manually before advertising |
| String parameter ambiguity (two contacts named "Alice") | low | resolve in-app, show disambiguation UI; upgrade to `AppEntity` later |
| macOS 26 deprecates `openAppWhenRun` | low | also set `supportedModes = .foreground(.dynamic)` when building with SDK 26; gate with `#available` |

### 3.5 Implementation plan (files)

Added:

- `apps/desktop/src-tauri/macos/RustWAExtensions.xcodeproj/` — Xcode project.
- `apps/desktop/src-tauri/macos/extensions/Intents/Info.plist`
- `apps/desktop/src-tauri/macos/extensions/Intents/RustWAIntentsExtension.swift`
- `apps/desktop/src-tauri/macos/extensions/Intents/OpenChatIntent.swift`
- `apps/desktop/src-tauri/macos/extensions/Intents/ComposeMessageIntent.swift`
- `apps/desktop/src-tauri/macos/extensions/Intents/RustWAShortcutsProvider.swift`
- `apps/desktop/src-tauri/macos/build-extensions.sh` — `xcodebuild` per configuration,
  signing identity passthrough, output into `macos/build/<Configuration>/`.
- `apps/desktop/src-tauri/macos/README.md` — build and test instructions.

Changed (for the implementing milestone, not this research task):

- `apps/desktop/src-tauri/tauri.conf.json` — `bundle.macOS.files` entry, `beforeBundleCommand`.
- `apps/desktop/src-tauri/src/deep_link.rs` — parse `rustwa://open` and `rustwa://compose`.
- `apps/desktop/src-tauri/src/lib.rs` — register any new command; none needed for the
  open-chat path.
- Frontend: handle the compose payload (prefill the composer); emit
  `platform_capabilities.app_intents = true` so Settings can describe the feature.

---

## 4. Widgets (WidgetKit)

### 4.1 Current state in RustWA

No widget, no App Group, no shared container. The unread total exists in the frontend
(`App.tsx` computes it from the zustand store), and chats live in the Rust-side SQLite
store `rustwa.db` under `app_data_dir()` **[verified in repo]**.

### 4.2 How the official WhatsApp Mac app does it

- This macOS install has **no WidgetKit extension**: `pluginkit -m -p
  com.apple.widgetkit-extension` lists no WhatsApp entry, and `Contents/PlugIns` contains
  only `Intents.appex` (SiriKit) and `ServiceExtension.appex` (notification service)
  **[verified]**. The Mac build simply does not ship a widget; the widget code is in the
  shared iOS/Catalyst module.
- The binary still contains widget machinery from the iOS side: `WAWidgetUpdater`,
  `WAWidgetUpdaterPlugin`, `WAWidgetDeepLink`, `widget-deep-link` and the shared widget
  container names `group.net.whatsapp.WhatsApp.shared` etc. **[verified via strings]**.
- The widget's configuration intent is the legacy SiriKit definition
  `SelectChatSource` (recents/favorites/pinned/frequents) **[verified]**.

So "how WhatsApp does it" for widgets is mostly *not* observable on this Mac. The useful
local evidence is the third-party Developer ID app **CodexBar**: host app is **not**
sandboxed but carries `com.apple.security.application-groups =
[Y5PE65HELJ.com.steipete.codexbar]`; its `Contents/PlugIns/CodexBarWidget.appex` is
**sandboxed** with the same App Group and
`NSExtensionPointIdentifier = com.apple.widgetkit-extension` **[verified]**. That is the
exact shape RustWA needs: unsandboxed Rust host + sandboxed Swift widget sharing an
App Group.

### 4.3 What a Tauri app needs

**Why a Swift extension is unavoidable.** Widgets are SwiftUI views rendered by the system
in a separate process; there is no webview, no Tauri IPC, and no WidgetKit C/Rust binding.
Rust can only write the data; the widget reads it.

**Bundle structure**

```text
RustWA.app/Contents/
├── PlugIns/
│   └── RustWAWidget.appex/
│       └── Contents/
│           ├── Info.plist        # NSExtension → com.apple.widgetkit-extension
│           └── MacOS/RustWAWidget (signed, sandboxed)
└── Resources/                    # (no web content for the widget)
```

**Entitlements**

Host (`bundle.macOS.entitlements` for the Tauri bundle, `codesign`d by Tauri via
`bundle.macOS.entitlements` config):

```xml
<key>com.apple.security.application-groups</key>
<array>
  <string>TEAMID.io.github.lucas-vdr-a11y.rustwa</string>
</array>
```

Widget:

```xml
<key>com.apple.security.app-sandbox</key><true/>
<key>com.apple.security.application-groups</key>
<array>
  <string>TEAMID.io.github.lucas-vdr-a11y.rustwa</string>
</array>
```

The group ID must be a **literal string** in these files: `$(TeamIdentifierPrefix)`-style
Xcode macros are expanded by Xcode's build system, but Tauri passes the entitlements plist
to `codesign` verbatim. If the group must vary by team, have `build-extensions.sh` generate
the entitlements files (or use a single fixed group ID that matches the provisioning
profile).

Both are **restricted entitlements** in the sense that the profile must list the group
(§2.4). With no paid team/provisioning profile the widget can still load and render a
static state, but `containerURL(forSecurityApplicationGroupIdentifier:)` will not resolve —
so the shared-data pipeline is a release-build-only feature, and the widget must degrade
gracefully.

**Data pipeline (recommended)**

1. Rust host owns the data. The core already knows unread counts and recent chats
   (SQLite + `CoreEvent` stream). Each incoming/read event updates a small JSON snapshot:
   `~/Library/Group Containers/<group-id>/widget-state.json` with
   `{unreadTotal, chats:[{name, unread, lastActivity}], updatedAt}`.
   Write atomically (temp file + rename); debounce to ~500 ms.
2. The widget's `TimelineProvider` reads that file (or a `UserDefaults(suiteName:)` mirror)
   and returns one entry with `TimelineReloadPolicy.after(...)`.
3. Timeline reload: there is **no `objc2-widgetkit` crate** and WidgetKit is Swift-only;
   `WidgetCenter.shared.reloadTimelines(ofKind:)` cannot be called from Rust without
   native shims **[verified: crates.io has no `objc2-widgetkit`; the unrelated `widgetkit`
   crate is not it]**. Two acceptable options:
   - **v1:** rely on the timeline policy (e.g. every 15–30 minutes) plus system-driven
     refreshes when the widget becomes visible. Simple, stale.
   - **v1.1:** add a tiny Swift static library (`librustwa_widgetbridge.a`) compiled by the
     same Xcode project that calls `WidgetCenter.shared.reloadTimelines` and exposes
     `extern "C" fn rustwa_widget_reload()`. Link it into the Rust binary on macOS and call
     it after snapshot writes. This also gives the host an `App Group` path helper.
     Building a static Swift lib into a Cargo binary is straightforward with a build script,
     but is extra build plumbing.
4. Privacy: show chat names and unread counts only, never message text; add a Settings
   toggle later if needed.

**What the widget could show:** small = unread total + app glyph; medium/large = top 3–5
recent chats with unread badges; tapping a row deep-links via a widget URL
(`rustwa://chat/<jid>`, which already works). Widgets support `widgetURL`/`Link` with custom
schemes **[unverified for custom schemes in macOS widgets — needs a spike]**.

**Tauri plugin availability:** none; Tauri's own widget request is open **[reported]**.

### 4.4 Effort, risks, non-notarized behavior

Effort: **8–15 days**, including the shared appex harness if not already in place.

- days 1–2: Xcode widget target, static/placeholder widget rendering in Notification
  Center / desktop.
- days 3–5: snapshot writer in Rust (`widget_state.rs`), event subscriptions, atomic writes,
  debounce; unit tests for the snapshot.
- days 6–8: App Group entitlements, provisioning, signing in the build script, graceful
  no-group fallback.
- days 9–10: deep links from widget rows, timeline policy tuning, Settings toggle.
- buffer: reload shim (Swift static lib) and distribution/notarization validation.

Risks:

| Risk | Severity | Mitigation |
|---|---|---|
| App Group unavailable locally / for free teams | high | treat shared data as a release feature; static placeholder otherwise; document |
| No reload push from Rust | medium | timeline policy first; Swift static-lib shim later |
| WidgetKit rejects an unsigned/not-notarized host on other Macs | high | notarize the whole bundle with all extensions signed with one identity |
| Data staleness (15–30 min) | medium | reload shim; also reload on app activation |
| System widget gallery does not list the extension | medium | verify with `pluginkit -m -p com.apple.widgetkit-extension` after `lsregister`; confirm `LSMinimumSystemVersion` matches host (13.0) |
| macOS 26 widget placement/location differences | low | PlugIns location works for Outlook/CodexBar/Goodnotes on this macOS 26.5 machine |

### 4.5 Implementation plan (files)

Added:

- `apps/desktop/src-tauri/macos/extensions/Widget/Info.plist`
- `apps/desktop/src-tauri/macos/extensions/Widget/RustWAWidgetBundle.swift`
- `apps/desktop/src-tauri/macos/extensions/Widget/RustWAWidget.swift` (timeline provider + views)
- `apps/desktop/src-tauri/macos/extensions/Widget/WidgetState.swift` (shared `Codable` model)
- `apps/desktop/src-tauri/macos/extensions/Shared/WidgetState.swift` (if sharing the model)
- `apps/desktop/src-tauri/Entitlements.plist` (host: app group)
- `apps/desktop/src-tauri/macos/extensions/Widget/Widget.entitlements`
- `apps/desktop/src-tauri/src/widget_state.rs` — snapshot writer + `CoreEvent` hook
- (optional) `apps/desktop/src-tauri/macos/widgetbridge/` — Swift static lib + header

Changed: `tauri.conf.json` (`files`, `entitlements`, `beforeBundleCommand`),
`lib.rs` (start snapshot writer), `platform_capabilities.widgets`.

---

## 5. Multiple windows

### 5.1 Current state in RustWA

- One window declared in `tauri.conf.json` (`label: "main"`), the only one listed in
  `capabilities/default.json` **[verified]**.
- `deep_link::dispatch` calls `app.emit(OPEN_CHAT_EVENT, …)` — a broadcast to every
  webview; the cold-start buffer (`DeliveryState`) is a single `Option<String>` consumed by
  whichever window calls `deep_link_ready` first; the frontend listener lives in
  `lib/bridge.ts` and calls `selectChat` / `startChat` **[verified]**.
- `menu::bring_all_to_front` already loops `app.webview_windows()` but focuses `main`
  **[verified]**.
- `tauri-plugin-window-state` is registered and currently persists/restores `main`
  **[verified]**.

### 5.2 How the official WhatsApp Mac app does it

- `UIApplicationSceneManifest` with `UIApplicationSupportsMultipleScenes = true`
  (and `UIApplicationSupportsTabbedSceneCollection = false`), `UIDesignRequiresCompatibility`
  **[verified in WhatsApp Info.plist]**. This is a Catalyst app using scenes; symbols such as
  `WAModalSplitViewController`, `WASettingsSplitViewController`, `UIScene` handling appear
  in the binary **[verified via strings]**. Each scene owns its own view hierarchy; the data
  layer (SharedModules) is shared per process.

### 5.3 What a Tauri app needs

**Creating chat windows (Rust).** `WebviewWindowBuilder::new` + `WebviewUrl::App(path)`.
The path goes through `Url::join`, so a query or fragment survives **[verified in
`tauri/src/manager/webview.rs`]**:

```rust
use tauri::{WebviewUrl, WebviewWindowBuilder, Manager};

fn chat_label(jid: &str) -> String {
    // Labels may only contain [a-zA-Z0-9-/:_] (verified in @tauri-apps/api docs),
    // so @ and . in a JID must be encoded.
    let digest = blake3::hash(jid.as_bytes()).to_hex();
    format!("chat-{}", &digest[..16])
}

pub fn open_chat_window(app: &tauri::AppHandle, jid: &str) -> tauri::Result<()> {
    let label = chat_label(jid);
    if let Some(window) = app.get_webview_window(&label) {
        return window.set_focus().map(|_| ());
    }
    let url = WebviewUrl::App(format!("index.html?chat={}", urlencoding::encode(jid)).into());
    WebviewWindowBuilder::new(app, label, url)
        .title("RustWA")
        .inner_size(900.0, 700.0)
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .build()?;
    Ok(())
}
```

(The sketch uses `blake3` for label hashing and `urlencoding` for the query value; RustWA
already depends on `percent-encoding`, which can replace `urlencoding`, and any stable
digest (for example `sha2`) will do for the label. New dependencies need to be added to
`Cargo.toml`.)

**Capabilities.** Capabilities match window labels with glob patterns **[verified in
`tauri-utils`]**. Add a least-privilege capability for chat windows instead of widening the
`main` one:

```jsonc
// capabilities/chat.json (new)
{
  "identifier": "chat",
  "description": "Ephemeral chat windows — core IPC and events only.",
  "windows": ["chat-*"],
  "permissions": ["core:default"]
}
```

**Routing.** Replace app-wide emits with targeted ones:

- `deep_link.rs`: if a `chat-<hash>` window exists for the JID, `emit_to(label, OPEN_CHAT_EVENT, …)`;
  else create a window and emit to it. The main window remains the fallback for unknown
  or `null` targets. `Emitter::emit_to` exists in Tauri 2.11 **[verified]**.
- Cold start is the subtle part. Today one global `Option<String>` is consumed once. For
  multi-window, either (a) keep a queue of pending actions drained by the first ready
  window that claims a chat target, or (b) make `deep_link_ready(label)` return the action
  destined for *that* window. Option (b) is cleaner once windows are created by Rust:
  `DeliveryState` becomes `Mutex<HashMap<String, PendingAction>>` keyed by window label.

**State implications (important).**

- Every webview gets its own JS context and its own zustand store. `selectedChatId`,
  `chats`, drafts and call state are **not shared**. Window A will not see window B's
  selection. For chat windows that is fine (each shows one conversation), but `chats` is
  hydrated per window through `list_chats`, so every new window pays the hydration cost
  and repeats event-driven rehydrates.
- `core://event` is emitted app-wide, so **all windows run the bridge**. Two consequences:
  duplicate desktop notifications (each window checks `document.hidden` independently) and
  duplicated history rehydrates. Fixes: move notification emission to the Rust host
  (recommended together with §6), or gate on `window.is_focused()` and a single owner
  window. `document.hidden` is unreliable once several windows exist.
- `tauri-plugin-window-state` will save/restore chat windows too. Use
  `Builder::with_denylist(&["chat-*"])` or `with_filter` (both exist in 2.11 plugin source
  **[verified]**) so ephemeral per-chat windows do not pile up across launches.
- macOS's Window menu lists open windows automatically because `menu.rs` uses Tauri's
  `WINDOW_SUBMENU_ID` **[verified in repo]**; "Bring All to Front" already iterates all
  windows.
- Tray left-click and menu events still target `main` via `bring_all_to_front`; decide
  whether tray activation should focus the most recent chat window instead.
- Single-instance behavior is unaffected (the second-instance plugin focuses `main` on
  Windows/Linux; macOS deep links go through `RunEvent::Opened`) **[verified]**.

**Frontend changes.** `App.tsx` should parse `location.search`/fragment on boot:
with `?chat=<jid>` it renders the conversation without the rail/chat list (or with a
compact rail); `bridge.ts` should skip its `ui://open-chat` handling in chat windows (or
handle it as "navigate this window"). No router library is needed; one `useState` initialised
from `URLSearchParams` is enough for v1.

**Tauri plugin availability:** core Tauri, no plugin required.

### 5.4 Effort, risks

Effort: **4–7 days** (no native code, no signing implications).

- day 1: window factory + label encoding + capability + window-state denylist.
- days 2–3: deep-link routing per window; `DeliveryState` rework; focus/reuse behavior.
- days 4–5: frontend routing and notification de-duplication; menu/tray behavior.
- buffer: edge cases (closing the window of the selected chat, restore behavior, zoom/menu
  actions that assume `main`).

Risks:

| Risk | Severity | Mitigation |
|---|---|---|
| Duplicate notifications | high | centralize notifications in Rust (§6) or elect a single notifier window |
| Cold-start deep link consumed by the wrong window | medium | per-window pending map; post the action after the window calls ready |
| Memory/CPU grows with windows | low | cap chat windows (e.g. 8), reuse/focus existing |
| Window-state restores stale chat windows | low | `with_denylist(["chat-*"])` |
| JID in a window label is invalid / leaks PII in the menu | medium | hash the label; keep the JID in the URL only |

### 5.5 Implementation plan (files)

Added:

- `apps/desktop/src-tauri/src/multi_window.rs` — `open_chat_window`, label encoding,
  window registry helpers.
- `apps/desktop/src-tauri/capabilities/chat.json` — `chat-*` capability.
- Frontend: `apps/desktop/src/lib/routing.ts` — parse `?chat=`/`#` into a route.

Changed: `deep_link.rs` (targeted emit + per-window buffer), `lib.rs` (register command),
`platform.rs` (`platform_capabilities.multi_window = true`), `menu.rs`/`tray.rs` (focus
policy), `lib.rs` window-state builder (denylist), `App.tsx`/`bridge.ts` (route + notifier
gating), `tauri.conf.json` (no new windows needed).

---

## 6. Notification click-through

### 6.1 Current state in RustWA

- `platform.rs` shows notifications through `tauri-plugin-notification 2.4.0`.
  `notify_for_chat` attaches `.extra("chatId", &chat_id)`.
- The plugin's desktop backend is `notify-rust` (**default `NSUserNotificationCenter` path**;
  `notification.show()` returns `()`), and the `show` path reads only title/body/icon/sound —
  **the `extra` payload is never delivered on desktop** **[verified in
  `tauri-plugin-notification-2.4.0/src/desktop.rs`]**. There is no click/action event on
  desktop; `registerActionTypes`/`onAction` exist only in `mobile.rs` **[verified]**.
  `platform.rs`'s doc comment already documents this limitation.
- `lib/bridge.ts` sends `notify_for_chat` for incoming messages when `document.hidden` and
  falls back to `notify` **[verified]**.
- The design spec (§7.6) assumed a `notification.click` listener; that does not exist in
  the current backend **[verified]**.

### 6.2 How the official WhatsApp Mac app does it

- The main binary contains a `UNUserNotificationCenterDelegate` implementation:
  selectors `userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:`,
  `userNotificationCenter:willPresentNotification:withCompletionHandler:`,
  `userNotificationCenter:openSettingsForNotification:` **[verified via strings]**.
- Notification payload keys present in the binary include `chatId`, `chatID`, `chatJid`,
  `chatJID`, `messageId`, `messageID`, `senderJid`, `senderJID`, `threadIdentifier`,
  `categoryIdentifier` **[verified via strings]**.
- It registers categories/actions (`categoryWithIdentifier:actions:intentIdentifiers:…`) and
  supports reply-from-notification (`replyWithMessage:toChatJID:toMessageWithUniqueKey:markAsRead:notification:completion:`,
  `markAsReadButton`, `replyAction`) **[verified via strings]**.
- Rich/decrypted content is prepared by `ServiceExtension.appex`
  (`com.apple.usernotifications.service`, principal class `WANotificationService`) — that is
  for remote/encrypted push, not needed for RustWA's live-socket notifications.
- Entitlements: the host has `com.apple.developer.usernotifications.communication = true`
  (Communication Notifications) and `aps-environment`; the service extension has
  `com.apple.developer.usernotifications.filtering` **[verified]**.
- Because the app is Catalyst/thin, its notification click path converges on the same scene
  machinery as deep links.

### 6.3 What a Tauri app needs

**Recommended: Option A — in-repo `UNUserNotificationCenter` backend via `objc2`.**

Why not the alternatives:

- **Status quo**: clicks cannot be correlated to a chat; unacceptable for a chat client.
- **`notify-rust` `preview-macos-un` feature** exists (`notify-rust 4.18.0` has a
  `UNUserNotificationCenter` backend and response APIs) **[verified in the registry
  source]**, but `tauri-plugin-notification` neither enables the feature nor surfaces
  handles/responses, so using it means forking the plugin anyway.
- **`notify-rust` default `NSUserNotificationCenter` + `wait_for_action`** returns the
  action label, not `userInfo`, and blocks on the main run loop; it cannot identify the
  chat **[verified in registry source]**.
- **Community plugin `tauri-plugin-notifications` 0.5.0-rc.13 (Choochmeque)** does
  implement a native macOS `UNUserNotificationCenter` backend through `swift-bridge`,
  including `registerActionTypes` and `setClickListenerActive`/action events, and explicitly
  refuses to run outside a `.app` bundle **[verified from its `src/macos.rs`]**. It is a
  reasonable **Option B** if we want actions/reply without writing the delegate ourselves,
  but it is a release candidate, it replaces the official plugin, and it brings a Swift
  build into the notification path. Recommend evaluating it for reply actions later, not as
  the foundation.

**Option A sketch (Rust, `objc2`).** Crates: `objc2` (0.6), `objc2-foundation` (0.3.2),
`objc2-user-notifications` (0.3.2), `block2` (0.6) — all on crates.io as of this writing
**[verified]**. The delegate trait methods were checked against docs.rs
**[verified]**:

```rust
// apps/desktop/src-tauri/src/notifications_macos.rs  (sketch)
use objc2::rc::Retained;
use objc2::{declare_class, msg_send_id, mutability, ClassType, DeclaredClass};
use objc2_foundation::{NSDictionary, NSString};
use objc2_user_notifications::{
    UNMutableNotificationContent, UNNotification, UNNotificationRequest,
    UNNotificationResponse, UNUserNotificationCenter, UNUserNotificationCenterDelegate,
};
use tauri::{AppHandle, Manager};

pub struct DelegateIvars {
    app: AppHandle, // or a Sender<NotifAction>
}
declare_class!(
    pub struct RustWANotificationDelegate;
    unsafe impl ClassType for RustWANotificationDelegate {
        type Super = objc2::runtime::NSObject;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "RustWANotificationDelegate";
    }
    impl DeclaredClass for RustWANotificationDelegate {
        type Ivars = DelegateIvars;
    }
    unsafe impl UNUserNotificationCenterDelegate for RustWANotificationDelegate {
        #[method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:)]
        fn did_receive(
            &self,
            _center: &UNUserNotificationCenter,
            response: &UNNotificationResponse,
            completion: &block2::DynBlock<dyn Fn()>,
        ) {
            let content = unsafe { response.notification().request().content() };
            let user_info = unsafe { content.userInfo() };
            if let Some(chat_id) = user_info.objectForKey(&NSString::from_str("chatId")) {
                let chat_id: Retained<NSString> = unsafe { Retained::cast_unchecked(chat_id) };
                // route exactly like a deep link
                crate::deep_link::deliver_chat(self.ivars().app.clone(), chat_id.to_string());
            }
            completion.call(());
        }
    }
);

/// Call from `setup()`, on the main thread, before the user can receive notifications.
pub fn install(app: &AppHandle) {
    let center = unsafe { UNUserNotificationCenter::currentNotificationCenter() };
    let delegate = RustWANotificationDelegate::alloc(
        objc2::MainThreadMarker::new().expect("setup runs on the main thread"),
    );
    // set_ivars + set delegate: center.setDelegate(Some(ProtocolObject::from_ref(&delegate)));
}

pub fn show_chat_notification(app: &AppHandle, chat_id: &str, title: &str, body: &str) {
    let content = UNMutableNotificationContent::new();
    unsafe {
        content.setTitle(&NSString::from_str(title));
        content.setBody(&NSString::from_str(body));
        let dict = NSDictionary::from_retained_objects(
            &[NSString::from_str("chatId")],
            &[NSString::from_str(chat_id)],
        );
        content.setUserInfo(&dict);
        content.setThreadIdentifier(&NSString::from_str(chat_id));
    }
    let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
        &NSString::from_str(&format!("{chat_id}:{message_id}")),
        &content,
        None, // deliver immediately
    );
    let center = unsafe { UNUserNotificationCenter::currentNotificationCenter() };
    center.addNotificationRequest_withCompletionHandler(&request, None);
}
```

Implementation notes:

- Only one object can be the center's delegate at a time; the Tauri plugin does not set one,
  so there is no conflict **[verified that the plugin's desktop backend never installs a
  delegate]**. Remove/ignore the plugin's desktop notification path; keep the plugin for
  permission APIs only if convenient (it always returns `Granted` on desktop, so it should
  be replaced by real `requestAuthorizationWithOptions` / `getNotificationSettings` checks).
- Apple requires the delegate be set before the app finishes launching to avoid missing
  early responses. Tauri's `setup()` runs before the event loop but the exact relation to
  `applicationDidFinishLaunching` needs a spike **[unverified]**; setting it in `setup()`
  is the earliest usable hook. Responses for notifications we post ourselves are only
  relevant after the app is running, so this is low risk.
- `willPresentNotification` should return banner options while the app is frontmost, or
  suppress for the currently focused chat (this pairs with §5's notification de-duplication).
- Permission: call `requestAuthorizationWithOptions` (alert/sound/badge) once, triggered by
  the first user action as the design spec requires; cache the `UNAuthorizationStatus`.
  `platform_capabilities.notifications` stays `true`; add `notification_click_through = true`.
- Message identifiers: `chatId:messageId` so re-posts coalesce, `threadIdentifier = chatId`
  so notifications group like WhatsApp's.
- Keep `notify`/`notify_for_chat` command signatures unchanged so `bridge.ts` needs no
  change; the Rust implementation switches to the native backend.
- Reply actions (text input) and mark-as-read later: `UNNotificationAction` +
  `UNNotificationCategory` + `UNTextInputNotificationAction`; the response handler gets
  `.userText` from `UNTextInputNotificationResponse`. This is a second milestone (M).

**Option B sketch (community plugin):** `tauri-plugin-notifications` with
`default-features = false`, permissions `notifications:default`, JS package
`@choochmeque/tauri-plugin-notifications-api`, `onAction` listener carrying `extra`.
Effort drops to ~1–2 days but adds a third-party RC dependency and a `swift-bridge` build.

### 6.4 Effort, risks, non-notarized behavior

Effort: **3–5 days** (Option A), **1–2 days + migration** (Option B).

- day 1: `objc2` delegate + install/teardown + permission plumbing; post local
  notification with `userInfo`.
- day 2: response → deep-link delivery → window focus; `willPresent` behavior;
  command wiring.
- day 3: replace plugin calls, error paths, notifications while frontmost, tests on a
  bundled debug build.
- buffer: reply actions (separate), edge cases (permission denied, DND).

Risks:

| Risk | Severity | Mitigation |
|---|---|---|
| `UNUserNotificationCenter` crashes/fails for the unbundled dev binary | high | always test through `tauri build --debug --bundles app`; document; community plugin uses the same policy |
| Delegate set too late / overridden | medium | set once in `setup`; assert no other delegate; keep notifications disabled until installed |
| `objc2` API drift (`block2`/repository versions) | low | pin versions; the sketch above is compile-checked only by the implementer's spike **[unverified]** |
| Plugin and native backend both fire | medium | remove plugin calls on macOS in one commit; keep one source of truth |
| Inline reply needs categories and more delegate work | low | defer to a follow-up milestone |

### 6.5 Implementation plan (files)

Added:

- `apps/desktop/src-tauri/src/notifications_macos.rs` — center access, permission, posting,
  delegate class, `userInfo` extraction, deep-link handoff.
- `apps/desktop/src-tauri/src/notifications.rs` (optional) — a thin cross-platform facade
  so `platform.rs` stays platform-neutral.

Changed: `apps/desktop/src-tauri/src/platform.rs` (route `notify*` through the facade),
`lib.rs` (install delegate in `setup`; register commands if changed),
`deep_link.rs` (reuse delivery for notification clicks), `Cargo.toml` (macOS-target deps:
`objc2`, `objc2-foundation`, `objc2-user-notifications`, `block2`),
`tauri.conf.json` capability maybe needs no change (commands are Rust-side).

---

## 7. Share sheet extension

### 7.1 Current state in RustWA

- No share extension; no `com.apple.share-services` registration.
- File associations are being added (parity matrix item); deep links already handle
  `wa.me` and `rustwa://chat/<jid>`.
- Sharing **out** of the app (a chat message to another app) is not implemented on the
  frontend; sharing **into** the app is the requested extension.

### 7.2 How the official WhatsApp Mac app does it

- The macOS bundle registers **no share extension**: `pluginkit -m -p
  com.apple.share-services` shows no WhatsApp entry, and `Contents/PlugIns` has only
  `Intents.appex` and `ServiceExtension.appex` **[verified]**. WhatsApp appears in the
  "Open With" flow through `CFBundleDocumentTypes` (image/audio/movie UTIs with
  `LSHandlerRank`, plus exported `net.whatsapp.*` UTIs) **[verified in Info.plist]**, and it
  accepts drag & drop; its Swift code still contains the iOS share-extension classes
  (`net.whatsapp.WhatsApp.ShareExtension`, `WAShareExtensionViewController`,
  `ShareExtensionMediaShareDeepLinkViewController`) but the Mac build does not ship the
  extension **[verified via strings]**.
- Sharing *out* is via `UIActivityViewController`/`WATextSharingViewController`
  **[verified via symbols]**.

So for parity, the macOS-first behavior is "Open With + drag & drop"; a share extension is
an addition, not a reproduction.

### 7.3 What a Tauri app needs

**Bundle structure and Info.plist** (modeled on the verified Tailscale extension, which is
Developer ID-distributed outside the Mac App Store):

```text
RustWA.app/Contents/PlugIns/RustWAShare.appex/Contents/
├── Info.plist
├── MacOS/RustWAShare
└── Resources/
```

```xml
<key>NSExtension</key>
<dict>
  <key>NSExtensionAttributes</key>
  <dict>
    <key>NSExtensionActivationRule</key>
    <dict>
      <key>NSExtensionActivationSupportsText</key><true/>
      <key>NSExtensionActivationSupportsWebURLWithMaxCount</key><integer>1</integer>
      <key>NSExtensionActivationSupportsFileWithMaxCount</key><integer>8</integer>
      <key>NSExtensionActivationSupportsImageWithMaxCount</key><integer>8</integer>
    </dict>
  </dict>
  <key>NSExtensionPointIdentifier</key>
  <string>com.apple.share-services</string>
  <key>NSExtensionPrincipalClass</key>
  <string>RustWAShare.ShareViewController</string>
</dict>
```

**Entitlements:** `com.apple.security.app-sandbox` (required for Mac App Store; good
practice anyway), `com.apple.security.application-groups` for the shared inbox, plus
`com.apple.security.files.user-selected.read-write` if files are copied. Entitlements are
the same App Group gate as §4.

**Swift sketch:**

```swift
import Cocoa
import UniformTypeIdentifiers

class ShareViewController: NSViewController {
    override func loadView() {
        // Minimal confirmation UI: "Send to RustWA" / "Add to a chat"
        view = NSView(frame: .zero)
    }

    override func viewDidAppear() {
        super.viewDidAppear()
        handleItems()
    }

    private func handleItems() {
        guard let items = extensionContext?.inputItems as? [NSExtensionItem] else { return }
        var payload = SharePayload()
        let group = DispatchGroup()
        for item in items {
            for provider in item.attachments ?? [] {
                if provider.hasItemConformingToTypeIdentifier(UTType.plainText.identifier) {
                    group.enter()
                    provider.loadItem(forTypeIdentifier: UTType.plainText.identifier) { data, _ in
                        if let s = data as? String { payload.text = s }
                        group.leave()
                    }
                } else if provider.hasItemConformingToTypeIdentifier(UTType.url.identifier) {
                    group.enter()
                    provider.loadItem(forTypeIdentifier: UTType.url.identifier) { data, _ in
                        if let u = data as? URL { payload.url = u.absoluteString }
                        group.leave()
                    }
                } else if provider.hasItemConformingToTypeIdentifier(UTType.fileURL.identifier) {
                    // Copy into the App Group inbox before completeRequest (see notes);
                    // this sketch only records the file name.
                    group.enter()
                    provider.loadItem(forTypeIdentifier: UTType.fileURL.identifier) { data, _ in
                        if let u = data as? URL { payload.files.append(u.lastPathComponent) }
                        group.leave()
                    }
                }
            }
        }
        group.notify(queue: .main) {
            self.finish(payload)
        }
    }

    private func finish(_ payload: SharePayload) {
        // Preferred: write JSON to the App Group, then deep-link with an id.
        if let container = FileManager.default
            .containerURL(forSecurityApplicationGroupIdentifier: "TEAMID.io.github.lucas-vdr-a11y.rustwa") {
            let inbox = container.appendingPathComponent("share-inbox")
            try? FileManager.default.createDirectory(at: inbox, withIntermediateDirectories: true)
            let id = UUID().uuidString
            try? payload.encoded().write(to: inbox.appendingPathComponent("\(id).json"))
            NSWorkspace.shared.open(URL(string: "rustwa://share?id=\(id)")!)
        } else if let text = payload.text,
                  let url = URL(string: "rustwa://share?text=\(text.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? "")") {
            // Fallback without App Groups: URL-only handoff.
            NSWorkspace.shared.open(url)
        }
        extensionContext?.completeRequest(returningItems: nil, completionHandler: nil)
    }
}
```

A share extension is a hosted `NSViewController`; macOS loads it inside the sharing UI
process. `NSWorkspace.shared.open` from a sandboxed extension is the same handoff pattern
the App Intents extension uses **[expected; Tailscale's extension is sandboxed with an app
group and network access, but its exact handoff is unverified]**.

**Rust/app side:** add `rustwa://share?id=` and `rustwa://share?text=` parsing to
`deep_link.rs`; on `id`, read the App Group inbox file, delete it, and open a chat picker
with the payload prefilled. For file attachments without App Groups, the extension can only
pass the original path; if the file lives outside the app's sandbox, the app must use the
path (it is unsandboxed, so it can read user-selected files) — but the user-selected
security scope belongs to the extension, so this is unreliable; prefer the App Group copy.

**Tauri plugin availability:** only a mobile share-target plugin exists (iOS/Android FIFO
queue) and it is explicitly mobile-only; nothing for macOS **[verified/reported]**.

### 7.4 Effort, risks, non-notarized behavior

Effort: **4–6 days** for text/URL sharing; **+4–6 days** for files with App Group inbox.

- days 1–2: Xcode share target, activation rule, minimal UI, local registration test.
- days 3–4: payload plumbing (JSON inbox or URL), Rust parsing, chat picker UI.
- days 5–6: signing/entitlements wiring, edge cases (no App Group, huge files,
  multiple items), docs.

Risks:

| Risk | Severity | Mitigation |
|---|---|---|
| App Group unavailable locally | high | URL-only fallback for text/URL; files degrade to "share to a temporary copy" or are release-only |
| Extension not shown in the share sheet | medium | correct `NSExtensionActivationRule`; verify with `pluginkit`; restart the sharing app |
| Sandboxed extension cannot read the source file after `loadItem` | medium | copy the file into the App Group inside the completion handler, before calling `completeRequest` |
| URL length limits for long text | low | use the App Group inbox (preferred) or truncate with a note |
| Duplicate/legacy file-association flows | low | keep `CFBundleDocumentTypes` (open-with) and the share extension independent |

### 7.5 Implementation plan (files)

Added:

- `apps/desktop/src-tauri/macos/extensions/Share/Info.plist`
- `apps/desktop/src-tauri/macos/extensions/Share/ShareViewController.swift`
- `apps/desktop/src-tauri/macos/extensions/Share/SharePayload.swift`
- `apps/desktop/src-tauri/macos/extensions/Share/Share.entitlements`
- `apps/desktop/src-tauri/src/share.rs` — inbox file read/cleanup + `share_pending` command
  for the UI.
- Frontend: `apps/desktop/src/components/ShareTargetPicker.tsx` — choose a chat for a
  shared URL/text/file and prefill the composer.

Changed: `deep_link.rs` (share links), `lib.rs`, `platform_capabilities.share_sheet`.

---

## 8. Effort & risk summary, recommended order

### 8.1 Consolidated effort/risk

| Feature | Effort | Native code needed | Signing/provisioning gate | Depends on | Payoff |
|---|---|---|---|---|---|
| Multiple windows | 4–7 d | no | none | — | high (chat-per-window; prerequisite for clean Siri/share UX) |
| Notification click-through | 3–5 d | Rust (`objc2`) | none (bundle required for testing) | recommended after multi-window routing | high (fixes a shipped gap) |
| App Intents / Shortcuts | 5–9 d | Swift extension | identity for distribution; none for local | shared appex harness; deep-link actions | medium/high |
| Share sheet | 4–6 d (+4–6 files) | Swift extension | App Group needs provisioning | shared appex harness; multi-window picker | medium |
| Widgets | 8–15 d | Swift extension (+ shim) | App Group + provisioning + notarization | shared appex harness; core event stream | medium (nice-to-have) |

### 8.2 Recommended order and rationale

1. **Multiple windows (4–7 d).** Pure Tauri; unlocks the UX all later features want
   (notification/Siri/share targets should open a chat window, not hijack the main one).
   It also forces the notification de-duplication decision that the click-through work
   depends on.
2. **Notification click-through (3–5 d).** The most visible existing gap; no new signing
   requirements; can be developed and tested on its own with a bundled debug build. Do it
   after window routing so clicking a notification opens the right chat in the right
   window.
3. **App Intents / Shortcuts (5–9 d).** Builds the shared `.appex` harness and validates
   the build/sign pipeline with the least signing friction (no App Group needed). Adds
   `rustwa://compose`, which the share sheet and widgets both reuse.
4. **Share sheet (4–6 d).** Reuses the harness, deep-link actions, and chat picker. Do it
   before widgets because text/URL sharing works without App Groups, delivering value
   while the provisioning story is sorted out.
5. **Widgets (8–15 d).** Last: most build/distribution-gated (App Group + notarization),
   needs a data-snapshot pipeline and either accepts staleness or adds a Swift reload shim.
   No other feature depends on it.

Cross-cutting milestone tasks if this becomes a schedule: create the Xcode project and
`build-extensions.sh` harness once (1–2 d, amortized across items 1/3/5), add
`platform_capabilities` flags per feature, and document the bundled-build testing
workflow (`tauri build --debug --bundles app` + `lsregister` + `pluginkit`).

---

## 9. Open questions / to verify in a spike

1. **UNUserNotificationCenter in a Tauri bundle**: delegate timing relative to
   `applicationDidFinishLaunching`, and dev-binary behavior. Test with a bundled debug
   build first.
2. **Siri/Shortcuts discovery for an un-notarized app**: does Shortcuts.app list the
   actions locally, and does Siri accept the phrases? Test both.
3. **App Group with ad-hoc signing**: confirm the failure mode (ignored entitlement vs
   process kill) and whether a `<TeamID>.<name>` group can work for local development with
   a paid team's profile but no notarization.
4. **Widget reload**: whether `WidgetCenter.reloadTimelines` is usable through the ObjC
   runtime (no `objc2-widgetkit` exists); otherwise build the tiny Swift static lib.
5. **Custom-scheme links inside widgets** (`widgetURL` with `rustwa://`).
6. **Share extension `NSWorkspace.open` from sandboxed extension**: confirm behavior on
   macOS 26 (expected to work, as in App Intents extensions).
7. **`supportedModes` vs `openAppWhenRun`** on macOS 26 SDK-based builds.
8. Whether Tauri's `bundle.macOS.files` copy preserves the executable bit and signature
   through its `xattr -crs` cleanup step (expected: yes; verify once).

---

## Appendix A. Evidence log

### A.1 WhatsApp.app (read-only)

- `Contents/Info.plist`: `CFBundleIdentifier net.whatsapp.WhatsApp`, version 26.33.73,
  `LSMinimumSystemVersion 12.1`, `CFBundleURLTypes` (`whatsapp`, `whatsapp-consumer`,
  `upi`, `fb306069495113`), `NSUserActivityTypes` (5× `IN*Intent` + `net.whatsapp.WhatsApp.chat`),
  `NSSiriUsageDescription`, `CFBundleDocumentTypes`, exported UTIs, App Group names,
  `UIApplicationSceneManifest.UIApplicationSupportsMultipleScenes = true`.
- `Contents/PlugIns/Intents.appex/Contents/Info.plist`: `NSExtensionPointIdentifier
  com.apple.intents-service`, `NSExtensionPrincipalClass WAIntentHandler`, `IntentsSupported`
  list (7 intents).
- `Contents/PlugIns/ServiceExtension.appex/Contents/Info.plist`:
  `com.apple.usernotifications.service`, `WANotificationService`.
- Entitlements: host has `com.apple.security.app-sandbox`,
  `com.apple.security.application-groups` (5 groups),
  `com.apple.developer.usernotifications.communication`, `aps-environment`;
  `Intents.appex` sandbox + app groups + keychain groups;
  `ServiceExtension.appex` adds `com.apple.developer.usernotifications.filtering`.
- `otool -L`: main binary links `WidgetKit`, `AppIntents` (weak), `Intents`,
  `UserNotifications`, `AppKit`, `SwiftUI`; `Intents.appex` links `Intents.framework`.
- `pluginkit -m`: `net.whatsapp.WhatsApp.Intents`, `net.whatsapp.WhatsApp.ServiceExtension`;
  no `whatsapp` under share-services or widgetkit extension points.
- `Contents/Resources/Metadata.appintents/extract.actionsdata` (JSON + plist view):
  five actions (`OpenMetaAIIntent` with `openAppWhenRun true`, `EndMetaAICallIntent`,
  `EndCallingLiveActivityIntent`, `ToggleMetaAICallIntent`, `ToggleCallingMicIntent`),
  `autoShortcutProviderMangledName WAAIActivityIntents.WAOpenMetaAIAppShortcutsProvider`,
  `shortcutTileColor 6`.
- `en.lproj/AppIntentVocabulary.plist`: phrases for the six SiriKit intents.
- `Base.lproj/WidgetIntents.intentdefinition`: `SelectChatSource` (+ `ChatSource` enum),
  `INIntentEligibleForWidgets = true`.
- `strings -a` over the main binary (1,514,413 lines, min length 6) greps:
  `WAWidgetUpdater*`, `WAWidgetDeepLink`, `widget-deep-link`,
  `net.whatsapp.WhatsApp.ShareExtension`, `WAShareExtension*`, `UIActivityViewController`,
  `UNUserNotificationCenterDelegate` selectors, notification `userInfo` keys
  (`chatId/chatID/chatJid/chatJID/messageId/messageID/senderJid/senderJID/threadIdentifier`),
  `categoryWithIdentifier:actions:…`, `replyWithMessage:toChatJID:…`, `markAsReadButton`,
  `replyAction`, `donateSendMessageIntentForChatJID:completion:`,
  `donateStartCallIntentForPeerJIDs:video:callID:`.
  Grep for `IN[A-Za-z]+Intent` found nothing because those constants live in frameworks,
  not as strings in the main binary; the SiriKit intent list comes from the extension
  plist.

### A.2 Machine references (other installed apps, read-only)

- Color Picker `2.2.2`: `Contents/Extensions/Intents Extension.appex` with
  `EXAppExtensionAttributes.EXExtensionPointIdentifier = com.apple.appintents-extension`,
  sandbox-only entitlements, `Metadata.appintents` present, host URL scheme
  `system-color-picker`, Developer ID `YG56YK5RN5`.
- Safari: `SafariLinkExtension.appex` (ExtensionKit, `com.apple.appintents-extension`) and
  `SafariWidgetExtension.appex` (`NSExtension`/`com.apple.widgetkit-extension`), both in
  `Contents/Extensions`.
- CodexBar: Developer ID + Sparkle (`SUFeedURL`), host unsandboxed with App Group
  `Y5PE65HELJ.com.steipete.codexbar`; widget in `Contents/PlugIns`, sandboxed with the same
  group and `com.apple.widgetkit-extension`.
- Outlook/Word/Excel/PowerPoint/Goodnotes: widget appexes in `Contents/PlugIns`,
  `com.apple.widgetkit-extension`, sandbox + App Group.
- Tailscale: `ShareExtension-macsys.appex`, `com.apple.share-services`,
  `NSExtensionPrincipalClass ShareExtension_macsys.ShareViewControllerMacOS`,
  file/image/movie activation rule, sandbox + app group `W5364U7YZB.group.io.tailscale.ipn.macsys`.

### A.3 Tauri / Rust toolchain

- Repo: `tauri 2.11.5`, `tauri-plugin-notification 2.4.0` (desktop = `notify-rust 4.18.0`),
  `tauri-plugin-deep-link 2.4.10`, `tauri-plugin-window-state 2.4.1`,
  `tauri-plugin-single-instance` with `deep-link`, `tauri-plugin-autostart`.
- `tauri-plugin-notification/src/desktop.rs`: `show()` reads only title/body/icon/sound and
  returns `()`; `register_action_types` exists only in `mobile.rs`; permission state always
  `Granted` on desktop.
- `tauri-bundler` (dev branch): `copy_custom_files_to_bundle` copies files/directories to
  `Contents/<destination>`; custom files are not added to `sign_paths`;
  `tauri-macos-sign/src/keychain.rs`: `codesign --force -s <id> [--options runtime]
  [--entitlements <plist>] <path>`, no `--deep`.
- `tauri-utils` config: `build.beforeBundleCommand` runs before bundling;
  `bundle.macOS.files`; capability `windows` supports glob patterns.
- `tauri/src/manager/webview.rs`: `WebviewUrl::App(path)` joins the path onto the app URL.
- `tauri/src/app.rs`: `RunEvent::Reopen { has_visible_windows }` (macOS) and `Emitter::emit_to`.
- `tauri-plugin-window-state`: `with_denylist`, `with_filter`, `with_state_flags`.
- `@tauri-apps/api` types: webview labels allow `[a-zA-Z0-9-/:_]` only.
- Xcode 26: widget template Info.plist uses `com.apple.widgetkit-extension`; no App Intents
  extension template file is present, but `AppIntents.swiftinterface` defines
  `AppIntentsExtension : ExtensionFoundation.AppExtension` (macOS 13+) and
  `appintentsmetadataprocessor` exists in the default toolchain.
- macOS SDK `AppIntents.swiftinterface`: `AppShortcutsProvider` (macOS 13+),
  `openAppWhenRun` (deprecated macOS 26 in favor of `supportedModes`), `OpenURLIntent`.

### A.4 External sources

- Apple: `NSExtensionPointIdentifier` property-list doc (lists `com.apple.intents-service`,
  `com.apple.intents-ui-service`); `Configuring app groups` (macOS `<Team ID>.<group>` form);
  `Creating a widget extension`.
- Tauri docs: macOS Application Bundle (`bundle.macOS.files` → `Contents`).
- Tauri issue #9766 — widget support is not available.
- `tauri-plugin-notifications` (Choochmeque) `src/macos.rs` — native UN backend,
  `require_bundle()` validation, `setClickListenerActive`/`registerActionTypes`.
- `tauri-apps/plugins-workspace#2150` — desktop notification onclick feature request (open).

### A.5 Not verified (explicitly)

- Siri/Shortcuts/SiriKit behavior on macOS for a locally built, non-notarized app.
- App Group semantics under ad-hoc signing (expected to fail/deny).
- Whether `WidgetCenter.reloadTimelines` can be reached without a Swift shim.
- Runtime behavior of `NSWorkspace.open` from a sandboxed share extension on macOS 26.
- Tauri `setup()` timing relative to `applicationDidFinishLaunching`.
- Custom-scheme links from widget rows.
