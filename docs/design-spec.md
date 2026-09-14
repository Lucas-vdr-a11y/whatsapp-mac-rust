# WhatsApp Desktop UI — Pixel-Accurate Rebuild Spec

**Target:** reproduce the WhatsApp desktop UI (macOS app **26.33.73**, which mirrors the WhatsApp Web design system) as a React + TypeScript frontend rendered in WKWebView via **Tauri v2**.
**Document date:** 2026-09-13.
**Primary source of truth:** the live WhatsApp Web production bundle (CSS/JS) fetched on 2026-09-13, plus the installed macOS app `/Applications/WhatsApp.app` (version 26.33.73, build 1049819294).

---

## 0. Method, source data, and confidence legend

### 0.1 What was inspected

| Source | What it provided | Confidence |
|---|---|---|
| `https://web.whatsapp.com/` CSS bundle (`static.whatsapp.net/rsrc.php/v5/yk/…aaK18xivDacKkmsA6qiUsNOB_OY8k1KqH.css`, 1.29 MB minified, 11 431 rules) | The complete **WDS** (WhatsApp Design System) token tables — 164 semantic tokens per theme (light + dark), ~106 primitive tokens, ~540 component tokens, typography scale, radii, breakpoints, button/list/input/toast metrics | ✅ verified (values read from production CSS) |
| Live public landing page (`https://web.whatsapp.com/`, QR pairing screen) rendered in real Chrome and measured via CDP | Exact landing-screen colors/typography/geometry (marketing palette `#FCF5EB`, card radius 25 px, 32 px heading, QR code 228×228, step list, consent text) | ✅ verified (computed styles + layout measured) |
| App JS bundles (`en_US-j` build) | Keyboard shortcut engine (`WAWebKeyboardShortcuts` module), UI string table (1 206 English strings), emoji picker width (573 px), reactions panel width (388 px) | ✅ verified (source extraction) |
| Installed macOS app 26.33.73 (`Assets.car`) | Native `AccentColor` values: light `#1B934C` (P3) / dark `#21C063` (P3); confirms the same accent family as Web | ✅ verified |
| Public documentation, 2025–2026 articles, community clone projects | Cross-checks for chat-list width, resizable sidebar, historical geometry | ⚠️ best-known (marked `[K]` where used) |

> **No login to WhatsApp was performed.** Everything above is from public pages or local app resources. Logged-in-only layout values that could not be measured directly are marked `[K]` (best-known) and should be validated against the reference app during implementation.

### 0.2 Confidence markers used throughout

- ✅ **Verified** — value read directly from production CSS/JS or measured live.
- 🔎 **Measured** — measured on the live landing screen (applies to that screen only).
- 🟡 **Best-known** — standard WhatsApp geometry that is stable across builds but not re-verified in this build; treat as a starting value.
- ⚠️ **Conflict** — sources disagree; the newest source wins, conflict is noted inline.

### 0.3 Golden rules for the rebuild

1. **Never reference WhatsApp hashed CSS classes.** They are per-build (e.g. the current default theme class is `xfmqtgv`; it will change). Copy **values**, and define your own stable variables (Appendix A gives a ready-to-paste token block).
2. **Do not copy WhatsApp proprietary assets** — icons, doodle wallpaper images, sounds, the wordmark. Use open-license substitutes (this document maps every needed icon to a **Lucide** name) and draw your own doodle pattern.
3. Layout is **opaque surfaces** — unlike native macOS apps there is no vibrancy/material behind the panels; the app paints solid colors (see §7.3 for the one exception: the macOS titlebar area).
4. Follow the token names: semantics (`accent`, `surface`, `content`, `lines`, `systems-…`) let the light/dark themes and the 2025 chat themes swap cleanly.

---

# 1. Design tokens

## 1.1 Color architecture

WhatsApp Web/Desktop uses the **WDS** token system:

- **Semantic tokens** — `--WDS-*`, 164 per theme, defined once and swapped between light and dark (and per chat theme).
- **Primitive ramps** — `--WDS-green-500`, `--WDS-neutral-gray-900`, etc.
- **Component tokens** — non-prefixed (`--button-medium-height`, `--card-corner-radius`, `--navbar-width`, …) defined in a component token block (541 values extracted). These are theme-independent metrics plus theme-dependent colors.

**Theme switching in production:** the app toggles a `dark` class on `<body>` and the whole token set is overridden inside `@media (prefers-color-scheme: dark)` + `.dark`. The `theme-color` meta tag is refreshed from `--navbar-background`.

**Chat themes (2025 feature):** WhatsApp ships ~40 accent/bubble/wallpaper variants (pink, purple, cobalt, teal, orange, red, yellow, monochrome, …) implemented exactly as WDS token overrides (e.g. accent `#E63371` pink). Defaults below; build the theme layer as token overrides, don't hardcode.

### 1.2 Core semantic colors — LIGHT theme (default) ✅

| Purpose | Token | Value |
|---|---|---|
| App background (top-level wash) | `--WDS-background-wash-plain` | `#FFFFFF` |
| App background (inset/window edges) | `--WDS-background-wash-inset` | `#F7F5F3` |
| Panel surface (chat list, headers) | `--WDS-surface-default` | `#FFFFFF` |
| Elevated surface (cards, popovers, dialogs) | `--WDS-surface-elevated-default` | `#FFFFFF` |
| Emphasized surface (search bar, hovered rows, nav rail) | `--WDS-surface-emphasized` | `#F7F5F3` |
| Emphasized elevated (pressed menus, secondary cards) | `--WDS-surface-elevated-emphasized` | `#F7F5F3` |
| Inverse surface (tooltips in old theme) | `--WDS-surface-inverse` | `#242626` |
| Divider lines | `--WDS-lines-divider` | `rgba(0,0,0,.1)` |
| Outline default | `--WDS-lines-outline-default` | `#959393` |
| Outline deemphasized | `--WDS-lines-outline-deemphasized` | `rgba(0,0,0,.2)` |
| Primary text | `--WDS-content-default` | `#0A0A0A` |
| Action/icon text | `--WDS-content-action-default` | `#0A0A0A` |
| Secondary text, icons, timestamps in list | `--WDS-content-deemphasized` | `rgba(0,0,0,.6)` |
| Disabled text/icons | `--WDS-content-disabled` | `#BDBDBD` |
| Text on accent (button labels) | `--WDS-content-on-accent` | `#FFFFFF` |
| Accent (primary green) | `--WDS-accent` | `#1DAA61` |
| Accent text/icon on light surfaces | `--WDS-content-action-emphasized` | `#1B8755` |
| Accent subtle surface (selected filter chip) | `--WDS-accent-deemphasized` | `#D9FDD3` |
| Accent strong text (dark theme bubbles) | `--WDS-accent-emphasized` | `#15603E` |
| External link / "action" link | `--WDS-content-external-link` | `#1B8755` |
| Read receipts (blue ticks) | `--WDS-content-read` | `#007BFC` |
| Always-branded green (logo, activity dot) | `--WDS-persistent-always-branded` | `#1DAA61` |
| Activity indicator (online dot, unread badge) | `--WDS-persistent-activity-indicator` | `#25D366` |
| Verified badge blue | `--WDS-persistent-verified` | `#0085F4` |
| Danger/negative | `--WDS-secondary-negative` | `#EA0038` |
| Danger deemphasized bg | `--WDS-secondary-negative-deemphasized` | `#FDE8EB` |
| Danger emphasized text | `--WDS-secondary-negative-emphasized` | `#B80531` |
| Positive | `--WDS-secondary-positive` | `#1DAA61` |
| Warning | `--WDS-secondary-warning` | `#FFB938` |
| Row hover / highlight | `--WDS-surface-highlight` | `rgba(194,189,184,.15)` |
| Selected list row (active chat) | `--WDS-components-active-list-row` | `rgba(194,189,184,.15)` |
| Pressed overlay | `--WDS-surface-pressed` | `rgba(0,0,0,.2)` |
| Bubble overlay (scrim inside bubbles) | `--WDS-systems-bubble-surface-overlay` | `rgba(194,189,184,.15)` |
| Modal backdrop dimmer | `--WDS-background-dimmer` | `rgb(0,0,0,.32)` |
| Text selection highlight | `::selection` | `rgba(var(--WDS-accent-RGB), .4)` |
| Message selection tint | legacy `--message-selection-highlight` | `rgba(0,128,105,.08)` |
| In-message hyperlink | legacy `--link` | `#027EB5` ⚠️ (green token also exists; see §1.6) |

### 1.3 Core semantic colors — DARK theme ✅

| Purpose | Token | Value |
|---|---|---|
| App background (wash plain) | `--WDS-background-wash-plain` | `#161717` |
| App background inset | `--WDS-background-wash-inset` | `#161717` |
| Panel surface (chat list, headers) | `--WDS-surface-default` | `#161717` |
| Elevated surface (cards, popovers, dialogs) | `--WDS-surface-elevated-default` | `#1D1F1F` |
| Emphasized surface (search bar, nav rail) | `--WDS-surface-emphasized` | `#1D1F1F` |
| Emphasized elevated | `--WDS-surface-elevated-emphasized` | `#242626` |
| Inverse surface | `--WDS-surface-inverse` | `#EEEEEE` |
| Divider lines | `--WDS-lines-divider` | `rgba(255,255,255,.1)` |
| Outline default | `--WDS-lines-outline-default` | `#757778` |
| Primary text | `--WDS-content-default` | `#FAFAFA` |
| Action/icon text | `--WDS-content-action-default` | `#FAFAFA` |
| Secondary text/icons | `--WDS-content-deemphasized` | `rgba(255,255,255,.6)` |
| Disabled | `--WDS-content-disabled` | `#424445` |
| Text on accent | `--WDS-content-on-accent` | `#0A0A0A` |
| Accent | `--WDS-accent` | `#21C063` |
| Accent text/icon | `--WDS-content-action-emphasized` | `#21C063` |
| Accent subtle surface | `--WDS-accent-deemphasized` | `#103529` |
| Accent strong text | `--WDS-accent-emphasized` | `#D9FDD3` |
| External link | `--WDS-content-external-link` | `#21C063` |
| Read receipts (blue ticks) | `--WDS-content-read` | `#53BDEB` |
| Activity indicator | `--WDS-persistent-activity-indicator` | `#25D366` |
| Verified blue | `--WDS-persistent-verified` | `#0085F4` |
| Danger | `--WDS-secondary-negative` | `#FB5061` |
| Danger deemphasized bg | `--WDS-secondary-negative-deemphasized` | `#321622` |
| Danger emphasized | `--WDS-secondary-negative-emphasized` | `#FA99A4` |
| Positive | `--WDS-secondary-positive` | `#71EB85` |
| Warning | `--WDS-secondary-warning` | `#FFD279` |
| Row hover / highlight | `--WDS-surface-highlight` | `rgba(255,255,255,.1)` |
| Selected list row | `--WDS-components-active-list-row` | `rgba(255,255,255,.1)` |
| Pressed overlay | `--WDS-surface-pressed` | `rgba(255,255,255,.2)` |
| Bubble overlay | `--WDS-systems-bubble-surface-overlay` | `rgba(0,0,0,.2)` |
| Modal backdrop dimmer | `--WDS-background-dimmer` | `rgb(0,0,0,.32)` |
| In-message hyperlink | legacy `--link` | `#53BDEB` |

### 1.4 Chat-surface, bubble and wallpaper colors ✅

| Purpose | Token | Light | Dark |
|---|---|---|---|
| Chat wallpaper (base) | `--WDS-systems-chat-background-wallpaper` | `#F5F1EB` | `#161717` |
| Wallpaper foreground (doodle tint) | `--WDS-systems-chat-foreground-wallpaper` | `#EAE0D3` | `rgba(255,255,255,.1)` |
| Outgoing bubble | `--WDS-systems-bubble-surface-outgoing` | `#D9FDD3` | `#144D37` |
| Incoming bubble | `--WDS-systems-bubble-surface-incoming` | `#FFFFFF` | `#242626` |
| System/date chip surface | `--WDS-systems-bubble-surface-system` | `rgba(255,255,255,.9)` | `#1D1F1F` |
| E2E notice surface | `--WDS-systems-bubble-surface-e2e` | `#FFF0D4` | `#1D1F1F` |
| E2E notice text | `--WDS-systems-bubble-content-e2e` | `rgba(0,0,0,.6)` | `#FFD279` |
| Business bubble surface | `--WDS-systems-bubble-surface-business` | `#D5FDED` | `#1D1F1F` |
| Business bubble text | `--WDS-systems-bubble-content-business` | `rgba(0,0,0,.6)` | `#06CF9C` |
| Bubble meta (timestamps/ticks inside bubble) | `--WDS-systems-bubble-content-deemphasized` | `rgba(0,0,0,.6)` | `rgba(255,255,255,.6)` |
| Composer (input bar) surface | `--WDS-systems-chat-surface-composer` | `#FFFFFF` | `#242626` |
| Chat tray (footer/emoji bar, right panel edge) | `--WDS-systems-chat-surface-tray` | `#F7F5F3` | `#161717` |
| Status seen ring | `--WDS-systems-status-seen` | `#C2BDB8` | `#757778` |
| Nav rail / nav bar surface | `--WDS-components-surface-nav-bar` | `#F7F5F3` | `#1D1F1F` |
| Nav rail background var | legacy `--navbar-background` | `var(--WDS-surface-emphasized)` | same |

**Wallpaper art:** production overlays a repeating doodle image over the base color (`data-asset-chat-background-beige` = `bg-chat-beige` asset, `-light`, `-dark` variants; 2 asset IDs per theme, low/high DPI). The doodle tint is exactly `--WDS-systems-chat-foreground-wallpaper`. **Rebuild recommendation:** generate an original doodle/pattern SVG (chat bubbles, coffee cups, hearts — generic motifs) at ~15 % opacity in the foreground color, tile ~340×340 px, fixed-attachment so the pattern doesn't scroll with messages (WhatsApp's pattern is static; only messages scroll).

### 1.5 Primitive ramps (complete) ✅

| Ramp | Values |
|---|---|
| Green | 75 `#E7FCE3` · 100 `#D9FDD3` · 200 `#ACFCAC` · 300 `#71EB85` · 400 `#25D366` · 450 `#21C063` · 500 `#1DAA61` · 600 `#1B8755` · 700 `#15603E` · 750 `#144D37` · 800 `#103529` |
| Neutral gray | 50 `#FAFAFA` · 75 `#F4F4F4` · 100 `#EEEEEE` · 300 `#BDBDBD` · 400 `#959393` · 500 `#757778` · 700 `#424445` · 800 `#242626` · 850 `#1D1F1F` · 900 `#161717` · 1000 `#0A0A0A` |
| Warm gray | 75 `#F7F5F3` · 100 `#F1EEEB` · 200 `#DBD8D4` · 300 `#C2BDB8` · 800 `#262524` · 900 `#171616` |
| Red | 75 `#FDE8EB` · 200 `#FA99A4` · 300 `#FB5061` · 400 `#EA0038` · 500 `#B80531` · 800 `#321622` |
| Yellow | 75 `#FFF7E5` · 100 `#FFF0D4` · 200 `#FFE4AF` · 300 `#FFD279` · 400 `#FFB938` · 500 `#C58730` · 800 `#362C1F` |
| Orange | 200 `#FDC1AD` · 300 `#FC9775` · 400 `#FA6533` · 500 `#C4532D` |
| Pink | 200 `#FFABC7` · 300 `#FF72A1` · 400 `#FF2E74` |
| Purple | 200 `#D1C4FF` · 300 `#A791FF` · 400 `#7F66FF` · 500 `#5E47DE` |
| Cobalt / sky | cobalt 200 `#99CAFE` · 300 `#53A6FD` · 400 `#007BFC`; sky-blue 200 `#93D7F5` · 300 `#53BDEB` · 400 `#009DE2` |
| Teal / emerald | teal 200 `#95DBD4` · 300 `#42C7B8` · 400 `#02A698`; emerald 100 `#D5FDED` · 400 `#06CF9C` |
| Alpha helpers | black-alpha 10/20/30/50/60/80; white-alpha 10/20/60/90; warm-gray-300-alpha-15 `rgba(194,189,184,.15)` |

### 1.6 Profile-photo (avatar) palette ✅

Avatar fallback colors are deterministic per-contact (“AvatarHome” component: initials on a colored surface). Both surface and content tokens are provided by the theme.

| Color | Light surface / content | Dark surface / content |
|---|---|---|
| green | `#D9FDD3` / `#1B8755` | `#103529` / `#25D366` |
| teal | `#CBF2EE` / `#028377` | `#092D2F` / `#42C7B8` |
| sky-blue | `#CAECFA` / `#027EB5` | `#092C3D` / `#53BDEB` |
| cobalt | `#D2E8FE` / `#0063CB` | `#092642` / `#53A6FD` |
| purple | `#E8E0FF` / `#5E47DE` | `#242447` / `#A791FF` |
| pink | `#FFDAE7` / `#D42A66` | `#36192A` / `#FF72A1` |
| red | `#FBD8DC` / `#B80531` | `#321622` / `#FB5061` |
| orange | `#FEE2D8` / `#C4532D` | `#35221E` / `#FC9775` |
| yellow | `#FFF0D4` / `#9D6C2C` | `#362C1F` / `#FFD279` |
| brown | `#F4DED1` / `#855538` | `#35271E` / `#DBA685` |
| gray | `#EEEEEE` / `#757778` | `#242626` / `#BDBDBD` |
| Close-friends status ring | `#C15ADD` (both themes) | |
| Avatar outline (on photos) | `--WDS-components-outline-profile-photo` `rgba(0,0,0,.1)` | `rgba(255,255,255,.1)` |

### 1.7 Component tokens (metrics + theme colors) ✅ selected highlights

Full set is in Appendix B; these are the ones used in this spec:

| Token | Value | Use |
|---|---|---|
| `--navbar-width` | `64px` | Navigation rail width |
| `--height-pane-footer` | `62px` | Chat composer height |
| `--screen-main-header-height` | `77px` | Full-screen (settings/calls/status) header height |
| `--screen-side-panel-width-small` / `-medium` | `280px` / `360px` | Drawer side panel widths |
| `--width-announcement-bubble` | `480px` (and responsive `max(calc(100vw/2.14),333px)` variants) | Wide system/announcement cards |
| `--button-small-height` / `-medium-` / `-large-` | `32px` / `36px` / `44px` | Buttons |
| `--button-*-padding-horizontal` | `12px` / `16px` / `20px` | Buttons |
| `--button-*-corner-radius` | `16px` / `18px` / `22px` | Buttons (pill) |
| `--button-disabled-opacity` | `.4` | Disabled buttons |
| `--button-group-spacing` | `6px` | Dialog footer buttons |
| `--list-cell-min-height` / `--nav-list-cell-min-height` | `52px` | Settings/menu list rows |
| `--nav-list-cell-margin-horizontal` | `16px` | Settings rows inset |
| `--nav-list-cell-corner-radius` | `10px` | Settings row radius |
| `--input-corner-radius` | `8px` | Text inputs |
| `--input-border-width` / focus | `1px` / `1px` | Input borders |
| `--text-input-min-height` | `60px` | Multiline dialog input |
| `--text-input-dense-height` | `44px` | Single-line dialog input |
| `--dialog-corner-radius` | `32px` | Modals |
| `--card-corner-radius` | `16px` | Cards |
| `--media-corner-radius` | `16px` (large `24px`, small `12px`) | Image/video containers |
| `--tooltip-corner-radius` | `12px` | Tooltips |
| `--toast-corner-radius` / `--toast-container-min-width` | `4px` / `288px` | Toasts |
| `--toast-background` (light) / `--toast-background-web` | `rgba(32,39,43,1)` / `#283943` | Toasts (dark surface in both themes) |
| `--popover-padding` | `16px` | Context menus/popovers |
| `--badge-inner-padding-horizontal/vertical` | `8px` / `6px` | Badges |
| `--search-input-background` | `rgb(241,238,235)` / dark `rgba(36,38,38,1)` | Chat-list search field |
| `--icon-container-size` | `48px` | Round icon buttons in headers |
| Radii scale | none 0 · half 4 · half+ 6 · single 8 · single+ 12 · double 16 · triple 24 · triple+ 28 · circle 9999 px | WDS spacing/radius tokens (`--x14m3oik`, `--x15rupc7`, `--x1qa5nmm`, `--x1qxzflc`, `--x83gkqu`, `--x1lh8xxe`, …) |
| Spacing scale | 0, 2, 4, 6, 8, 12, 16, 20, 24, 28, 32, 40 px | WDS spacing tokens |

### 1.8 Legacy ("classic") palette — do **not** use for new work ⚠️

Still defined in the bundle for backward compatibility and for third-party/Meta components. Useful only to identify stale references:

| Token | Light (classic) | Dark (classic) |
|---|---|---|
| `--panel-background` | `#F0F2F5` | `#111B21` |
| `--panel-header-background` | `#F0F2F5` | `#202C33` |
| `--incoming-background` | `#FFFFFF` | `#202C33` |
| `--outgoing-background` | `#D9FDD3` → classic `#DCF8C6` / 2022 `#D9FDD3` | `#005C4B` |
| `--chat-list background` / `--conversation-panel-background` | `#EFEAE2` | `#0B141A` |
| `--primary-strong` | `#111B21` | `#E9EDEF` |
| `--secondary` / `--icon-*` | `#667781` / `#8696A0` | `#8696A0` / `#AEBAC1` |
| `--unread-marker-background` green | `#25D366` | `#00A884` |
| `--link` | `#027EB5` | `#53BDEB` |

⚠️ **Conflict note:** most 2024-era blog posts and clones still use the classic palette (`#111B21`, `#202C33`, `#005C4B`, `#00A884`, `#EFEAE2`). The current (2025-2026) app has moved to the WDS values in §1.2–1.4 (`#161717`, `#242626`, `#144D37`, `#1DAA61`, `#F5F1EB`). Prefer §1.2–1.4.

---

## 2. Typography

### 2.1 Font stacks ✅

| Context | Font stack |
|---|---|
| WhatsApp Web (what users see in a browser) | `"Roboto Variable", Roboto, "Helvetica Neue", Helvetica, sans-serif` (Roboto Variable webfont is loaded by the app) |
| macOS system-stack token (used by native macOS app and available on the web) | `system-ui, -apple-system, BlinkMacSystemFont, ".SFNSText-Regular", sans-serif` — token `--font-family-apple` |
| Windows | `Segoe UI Historic, Segoe UI, Helvetica, Arial, sans-serif` — `--font-family-segoe` |
| Fallback web default | `Helvetica, Arial, sans-serif` |
| Monospace (code blocks in messages) | `ui-monospace, Menlo, Consolas, Monaco, monospace` |

**Recommendation for the Tauri/macOS rebuild:** use `-apple-system, BlinkMacSystemFont, "SF Pro Text", "Helvetica Neue", sans-serif` (= `system-ui`) so the UI matches the native macOS app (SF Pro). Do **not** ship Roboto unless you want the browser look. Keep the metrics table below — it is font-independent.

### 2.2 Type scale ✅

WDS defines a full scale. Sizes are in rem (1 rem = 16 px). “Emphasized” = bold variants.

| Style | Size | Weight | Line-height | Letter-spacing | Typical use |
|---|---|---|---|---|---|
| `display-large` | 48 px (3 rem) | 400 | 1.1458 (55 px) | 0 | Full-screen empty states, landing hero |
| `display-medium` | 43 px (2.6875 rem) | 400 | 1.1628 | 0 | Landing sections |
| `display-small` | 34 px (2.125 rem) | 400 | 1.2059 | 0 | Landing headings |
| `display1` | 48 px | 600 | 1.1667 | 0 | WDS display |
| `headline-large` / `headline1` | 24 px (1.5 rem) | 600 | 1.1667 (28 px) | 0 | Settings page titles, dialog titles |
| `headline-medium` / `headline-small` / `headline-extra-small` / `headline3` | 17 px (1.063 rem) | 400 (headline3) / 500 | 1.2941 (22 px) | 0 | Panel headers (“Chats”, “Settings”), card titles |
| `body` / `body-large` / `body-medium` | 15 px (.9375 rem) | 400 (body), 500 (body-large) | 1.2667 (19 px) | 0 | Message text, input text, general body |
| `body-emphasized` | 15 px | 700 | 1.2667 | 0 | Message sender names, bold list titles |
| `body-small` | 13 px (.8125 rem) | 400 | 1.3077 (17 px) | 0 | Chat list preview, captions |
| `body-small-emphasized` | 13 px | 700 | 1.3077 | 0 | Caption emphasis |
| `label-large` / `label-medium` | 15 px | 500 | 1.2667 | 0 | Buttons (large/medium), menu items |
| `label-large-emphasized` / `label-medium-emphasized` | 15 px | 700 | 1.2667 | 0 | Emphasized buttons |
| `label-small` | 13 px | 500 | 1.3077 | 0 | Small buttons |
| `label-extra-small` | 11 px (.6875 rem) | 500 | 1.4545 (16 px) | 0 | Timestamps, badges, tiny labels |
| `text-input-field` | 15 px | 500 | 1.2667 | 0 | Dialog input values |
| `text-input-label` | 15 px (scaled ×.8667 when floating = 13 px) | 400 | 1.13334 | 0 | Floating labels |

Additional observed declarations in the app CSS (WhatsApp-specific components):

| Element | Size / weight | Source |
|---|---|---|
| Chat list contact name | **17 px / 400–500**, line-height ~22 px, ellipsis | headline-extra-small [K] |
| Chat list message preview | **14–15 px / 400**, one line, ellipsis; 13 px in dense mode | body [K] |
| Chat list timestamp (right) | **12 px / 400** (`--label-extra-small` 11 px min) | [K] |
| Unread badge number | 11–12 px / 500, white | [K] |
| Message text (inside bubble) | **15 px / 400**, line-height 19 px; quoted text 13–14 px | [V] token match |
| Message timestamp + ticks | **11 px / 500**, color = bubble-content-deemphasized | [V] token + [K] size |
| In-chat system/date chip | 12–13 px / 500 | [K] |
| Search field text | 15 px / 400; placeholder same size, deemphasized | [K] |
| Header contact name | 16–17 px / 500 | [K] |
| Header presence line | 13–14 px / 400, accent color for “online” | [K] |
| Composer placeholder (“Type a message”) | 15 px / 400 deemphasized | [K] |
| Dialog title | 24 px / 600 (`headline1`) | [V] |
| Keyboard-shortcut modal key labels | 13 px / 500 with keycap chrome | [K] |

**Letter spacing:** `0` for nearly everything; the only negative tracking found is `-.01em` on very large display text. Do not add tracking to body text.

**Paragraph formatting in messages:** `**bold**` (700), `*italic*` (italic), `~~strike~~`, `` `mono` `` (monospace stack), ``` ```code block``` ``` (14 px, monospace, background `rgba(0,0,0,.05)` / `rgba(255,255,255,.1)`, radius 4 px).

### 2.3 Fallback rendering notes

- `text-rendering: optimizeLegibility`; disable hyphenation; enable `overflow-wrap: anywhere` on message text.
- Message text direction per bubble content (`dir="auto"`).
- Emoji are rendered by the system emoji font; do not ship an emoji font.
- Support Dynamic Type–like chat font-size setting: WhatsApp offers Small/Medium/Large message text (13/15/17 px) → implement as a CSS var multiplier on message text only.

---

# 3. Geometry: spacing, radii, sizes, shadows, iconography

## 3.1 Spacing scale ✅

WDS spacing tokens: **0, 2, 4, 6, 8, 12, 16, 20, 24, 28, 32, 40 px** (internal CSS vars `--x3s3u4o:4px`, `--x14m3oik:8px`, `--xb871un:12px`, `--x1j6lpam:16px`, `--xpaog5i:20px`, `--x1qxzflc:24px`, `--x83gkqu:28px`, `--xbevxdb:32px`, `--x1kja0qa:40px`).

Common paddings observed/known:

| Element | Padding |
|---|---|
| Header (chat list + conversation) | 10–16 px vertical; 16 px horizontal; height 59 px |
| Chat-list row | 8 px vertical, 12–16 px horizontal (row content height 72 px) |
| Message bubble | 6 px top, 7 px right, 8 px bottom, 9 px left (classic WA geometry) 🟡 |
| Composer | 5–8 px vertical, 8–10 px horizontal; height 62 px |
| Settings/menu list row | 12–16 px vertical, 16 px horizontal (min-height 52 px) ✅ |
| Dialog | 24–32 px padding, 32 px radius ✅ (radius) |
| Context menu item | 8–10 px vertical, 16 px horizontal; popover padding 16 px ✅ |
| Toast | 16 px container padding, 6 px addon padding ✅ |

## 3.2 Corner radii ✅ (WDS scale + verified component values)

| Target | Radius |
|---|---|
| WDS scale | 0 · 4 · 6 · 8 · 12 · 16 · 24 · 28 · 9999 px |
| **Message bubble** | **7.5 px** (all corners; the tail corner is squared/clipped on the last bubble of a group) ✅ CSS contains `border-radius:7.5px` |
| Media inside bubble | 7.5 px minus 3 px padding → ~4.5 px; full-bleed images 7.5 px |
| Reply quote block | 7.5 px outer, 4 px inner accent bar |
| Buttons (pill) | small 16, medium 18, large 22 px ✅ |
| Text inputs / search | 8 px ✅ |
| Cards | 16 px ✅ |
| Media cards (image/video/sticker preview) | small 12, base 16, large 24 px ✅ |
| Dialog / modal | 32 px ✅ |
| Tooltip | 12 px ✅ |
| Toast | 4 px ✅ |
| Context menu / popover | 8–10 px (menu cell radius 10 px) ✅ |
| Nav rail active pill | 9999 px (pill) or 12–16 px thumbnail with `border-radius: 10px` for rail icons (WA uses pill highlight ~ 24×32) 🟡 |
| QR code container | 2 px ✅ (measured) |
| Avatar (people/groups) | 50 % circle |
| Avatar (entities/business/channels, squircle) | small 8, medium 12, large 16, xlarge 24 px ✅ |
| Status ring radius | circle |

## 3.3 Sizes and dimensions ✅/[K]

| Element | Size | Confidence |
|---|---|---|
| Nav rail width | **64 px** ✅ (`--navbar-width`) |
| Nav rail icon button | 40×40 px hit area, 24 px glyph; ~32×32 px active pill | 🟡 |
| Nav rail vertical padding | 10 px top/bottom; icon slot ~ 52 px vertical rhythm | 🟡 |
| Chat list default width | **400 px** (CSS contains `width:400px` for a panel), user-resizable by dragging the divider; min ~ 300 px, max ~ 500 px | 🟡/⚠️ |
| Conversation header height | **59 px** ✅ (`height:59px` in CSS) |
| Chat-list header height | 59 px ✅ (same class family) |
| Chat-list search area | 49–56 px tall including padding | 🟡 |
| Chat-list row height | **72 px** ✅ (`height:72px` present); dense mode 64 px | |
| Conversation composer height | **62 px** ✅ (`--height-pane-footer`) |
| Full-screen views header | 77 px ✅ (`--screen-main-header-height`) |
| Settings side panel | 280 / 360 px ✅ |
| Right-hand drawer (profile/contact info) | ~ 400 px (same family as chat list); side-panel tokens use 360 px on small screens ✅ | 🟡 |
| Emoji picker panel | **573 px wide** ✅ (`EMOJI_PICKER_WIDTH`) |
| Reactions panel | **388 px** ✅ (`REACTIONS_PANEL_WIDTH`) |
| Reaction picker row | 6 quick emojis, each ~ 40 px bubble | 🟡 |
| Attachment menu (grid) | 3×3 or 4-column grid of 32 px icons with 13 px labels; panel ~ 300×260 px | 🟡 |
| Announcement bubble max width | 480 px ✅ |
| Tooltip | min 24 px height, 8–16 px padding, 12 px radius ✅ | |
| Unread badge | 18–20 px circle/pill, 11–12 px type; dot variant 10 px (muted) | 🟡 |
| Rail badge | same as unread badge, positioned top-right of 24 px glyph | 🟡 |
| Icon hit area in headers | 40×40 px (48 px optional) ✅ `--icon-container-size:48px` |
| Composer icon buttons | 24 px glyph in 40 px hit area | 🟡 |
| Scrollbar | 6 px wide thumb, `rgba(0,0,0,.2)` light / `rgba(255,255,255,.16)`–`.2` dark, 3 px radius, transparent track | ✅ CSS |
| Focus ring | `outline: 2px solid var(--WDS-accent)`; 3 px variant on large targets | ✅ CSS |
| Minimum app window | 800 × 600 px (WhatsApp Desktop refuses smaller; also communicates the 3-column floor) | 🟡 |

**Avatar scale** ✅ (values present in CSS): 24, 28, 36, 40, 48, **49** (chat list rows), 56, 64, 96, 108, 120, 135, 150, 165, **212** (profile hero). Recommended mapping:

| Context | Size |
|---|---|
| Chat list row avatar | **49 px** |
| Conversation header avatar | **40 px** |
| Rail profile avatar | 32–36 px |
| Settings profile hero | 96–120 px |
| Contact/group info drawer hero | 212 px |
| Message sender avatar (group chats) | 28 px (only on last bubble of a group) |
| Mention autocomplete avatar | 24–28 px |
| Status list avatar | 49 px + ring |
| Call log avatar | 49 px |

## 3.4 Message bubble geometry ✅/[K]

| Property | Value |
|---|---|
| Max width (text messages) | **65 %** of the message-column width ✅ (`max-width:65%` in CSS) |
| Media/sticker max | 65 % for images/videos (up to ~ 500 px), sticker ~ 200 px, document card ~ 65 % |
| Bubble radius | 7.5 px ✅ |
| Tail | Short pointed "corner tail" attached to the **last bubble of a group** only. Geometry: 8 px wide × 13 px tall, drawn from the bubble's bottom corner (outgoing right / incoming left), same color as bubble, with the outer curve matching the 7.5 px radius. Implementation options: inline SVG path (recommended: `M8 13 C8 5.5 4.5 0 0 0 L0 13 Z` mirrored for incoming), or the classic two-pseudo-element CSS approach using two overlapping rounded rectangles. **Do not reuse WhatsApp's SVG path assets** — redraw. |
| Padding | 6 px top / 7 px right / 8 px bottom / 9 px left (text) 🟡 |
| Timestamp + ticks | inline flex at the bubble's bottom-right, 11 px / 500, with ~ 4 px gap after the last word; text reserves 60–74 px of trailing space so text never sits under the meta 🟡 |
| Group spacing | 2 px between consecutive bubbles from the same sender; 12 px between a bubble group and the next sender's first bubble; 4–6 px extra above date chips 🟡 |
| Outgoing alignment | right, with 8 px right margin from the panel edge (plus room for tail) |
| Incoming alignment | left, 8 px left margin; in groups, 49+8 px when the sender avatar is shown |
| Quote/reply inside bubble | full-width block above text: 4 px accent left bar, 13 px name in accent color, 13–14 px quoted text clamped to 2–3 lines, overlay background |
| Forwarded label | italic, 13 px, deemphasized, above text ("Forwarded", "Forwarded many times") |
| Reaction pill | attached to bubble bottom edge, offset −8 px below the bubble, 9999 radius, 2 px 8 px padding, 13 px emoji, 1 px border `--WDS-lines-divider`, background `--WDS-surface-elevated-default`, shadow L2 |
| System/date chip | centered; background `--WDS-systems-bubble-surface-system`; 7.5 px radius; 5 px 12 px padding; 12–13 px; text `--WDS-content-deemphasized` |
| Unread divider | full-width 1 px line `--WDS-accent` + centered green chip "Unread messages" (on scroll-to-unread) |
| E2E notice | centered yellow card (`#FFF0D4` light), lock icon 16 px, 12–13 px text, radius 7.5 px |

**Message sending states** (`✓` glyph color rules):

| State | Icon | Color |
|---|---|---|
| Pending (sending) | `clock` | `--WDS-systems-bubble-content-deemphasized` |
| Sent (server ACK) | single `check` | deemphasized |
| Delivered | double `check-check` | deemphasized |
| Read | double `check-check` | `--WDS-content-read` (`#007BFC` light / `#53BDEB` dark) |
| Failed | circle-alert (red) | `--WDS-secondary-negative` |
| Played (voice note) | mic + check | read color |

## 3.5 Elevation & shadows ✅/🟡

| Layer | Shadow |
|---|---|
| Bubbles | `0 1px .5px rgba(11,20,26,.13)` (classic) 🟡 |
| Chat-list/rail panels | none (flat, divided by 1 px `--WDS-lines-divider`) |
| Dropdown/context menu | `0 3px 12px rgba(11,20,26,.18)` light; in dark add 1 px `rgba(255,255,255,.06)` border 🟡 |
| Dialog/modal | `0 24px 38px 3px rgba(0,0,0,.14), 0 9px 46px 8px rgba(0,0,0,.12), 0 11px 15px -7px rgba(0,0,0,.2)` 🟡 |
| Card (WDS) | `0px 8px 20px 0px rgba(0,0,0,.2), 0px 2px 4px 0px rgba(0,0,0,.1)` ✅ (`--card-box-shadow`) |
| Toast | `0 2px 8px rgba(11,20,26,.32)` 🟡 |
| Focus | `outline: 2px solid var(--WDS-accent)` ✅ |

**Motion tokens** 🟡: standard 150 ms `ease-out` for hovers; 200 ms for menus (fade+scale .98→1); 250 ms `cubic-bezier(.4,0,.2,1)` for drawer/panels; typing dots 1.4 s loop; reaction pop 150 ms spring; skeleton shimmer 1.5 s. Honor `prefers-reduced-motion` (present in CSS ✅).

**Z-index plan** 🟡 (one verified anchor: animated emoji in conversation = 300):

| Layer | z-index |
|---|---|
| Sticky date chips | 10 |
| Right drawer / side panel | 100 |
| Contextual layer (menus, popovers) | 200 |
| Animated emoji (in conversation) | 300 ✅ |
| Modal + backdrop | 400 |
| Toast | 500 |
| Tooltips | 600 |

## 3.6 Iconography

### Rules

- **Do not copy WhatsApp icon SVGs.** Use **Lucide** (ISC license) or draw original glyphs. Every icon below has a Lucide counterpart.
- Grid: 24×24 viewBox for toolbar/nav glyphs; 20×20 for list addons; 16×16 for inline meta (ticks, muted, pinned, lock); 12–14 px for badge marks.
- Rendering: `stroke: currentColor`, `stroke-width: 2` at 24 px (optically 2.25–2.5 at 16 px), `stroke-linecap: round`, `stroke-linejoin: round`. WhatsApp's real set is mostly **filled**; for active/selected states you can fill the Lucide outline via `fill: currentColor` + `stroke: currentColor` (works well for `message-circle`, `heart`, `star`, `bell`, `image`). Alternatively, draw the filled variant for the five nav icons only.
- Alignment: 24 px glyph inside a 40×40 px button; icon-to-label gap 8 px.

### Icon inventory → substitute map

| WhatsApp function | Lucide name | Notes |
|---|---|---|
| Chats (nav, inactive/active) | `message-circle` | fill on active |
| Calls (nav) | `phone` | |
| Status (nav) | `circle-dashed` | segmented ring; custom draw acceptable |
| Channels (nav) | `radio` | broadcast feel; `rss` as fallback |
| Communities (nav) | `users-round` | three-person group |
| Settings (nav + screens) | `settings` | gear |
| Search | `search` | |
| Back | `arrow-left` | macOS also supports swipe/⌘[ |
| Forward (arrow) | `forward` | |
| Reply | `reply` | |
| Kebab menu | `more-vertical` | |
| New chat | `square-pen` | or `message-square-plus` |
| New group | `users` + plus badge | `user-round-plus` fallback |
| New community | `users-round` + plus | |
| Attach | `paperclip` | |
| Plus (attach menu) | `plus` | |
| Send | `send-horizontal` | paper plane |
| Microphone | `mic` / `mic-off` | |
| Voice note play/pause | `play` / `pause` | |
| Emoji | `smile` | |
| Sticker | `sticker` | |
| GIF | `clapperboard` | WhatsApp uses the letters “GIF” |
| Camera | `camera` | |
| Switch camera | `refresh-cw` | or custom two-camera glyph |
| Photos & videos | `image` | |
| Document | `file-text` | |
| Audio file | `file-audio` | |
| Contact card | `contact-round` | |
| Poll | `chart-no-axes-column` | old name `bar-chart-3` |
| Event | `calendar` | |
| Location | `map-pin` | |
| Live location | `navigation` | |
| Star / starred | `star` | fill for selected |
| Pin / unpin | `pin` / `pin-off` | |
| Mute / unmute | `bell-off` / `bell` | |
| Archive / unarchive | `archive` / `archive-restore` | |
| Delete | `trash-2` | |
| Clear chat | `eraser` | |
| Block | `ban` | |
| Report | `flag` | |
| Mark as unread | `mail` | |
| Mark as read | `check-check` | |
| Single tick / delivered tick | `check` / `check-check` | 16 px, stroke 2.5 |
| Pending clock | `clock` | |
| Failed | `circle-alert` | old alias `alert-circle` |
| Download | `download` | |
| Disappearing messages | `timer` | |
| Lock / encryption | `lock` | |
| Chat lock | `lock-keyhole` | |
| Verified | `badge-check` | fill blue `--WDS-persistent-verified` |
| Business | `briefcase` | |
| Keyboard shortcuts | `keyboard` | |
| Privacy | `shield` | |
| Theme | `palette` | |
| Wallpaper | `image` | |
| Font size | `a-large-small` | |
| Storage & data | `database` | |
| Help | `circle-help` | old alias `help-circle` |
| Log out | `log-out` | |
| Linked devices | `monitor-smartphone` | |
| QR code | `qr-code` | |
| Sync / refresh | `refresh-cw` | |
| Loading spinner | `loader-circle` | spin 1 s linear |
| Select messages | `square-check` | |
| Copy | `copy` | |
| Edit | `pencil` | |
| Message info | `info` | |
| Exit group | `door-open` | |
| Chevron | `chevron-down` / `chevron-right` | |
| Close | `x` | |
| Filter (chat list tabs) | `list-filter` | |
| Unread filter | `circle` (filled dot) | |
| Favourites filter | `heart` | |
| Groups filter | `users` | |
| Muted chat marker (row) | `bell-off` | 16 px deemphasized |
| Pinned chat marker (row) | `pin` | 16 px deemphasized |
| Voice-note speed | `gauge` | or text “1×/1.5×/2×” | |
| Reactions entry | `smile-plus` | |
| Screen share | `screen-share` | |
| End call | custom handset-down | or `phone-off` (visually different) |
| Raise hand | `hand` | |
| Call camera toggle | `video` / `video-off` | |
| Speaker / volume | `volume-2` / `volume-x` | |
| Emoji categories | custom glyphs | Lucide has no emoji set; use 16 px category glyphs |

---

# 4. Component inventory

Every component lists: **anatomy → metrics → states → behavior**. Token names refer to §1; all colors must come from tokens, never literals (except where noted as brand constants).

## 4.1 App shell — 3-column layout

```
┌──────┬──────────────────┬────────────────────────────────────────────┐
│ rail │ chat list panel  │ conversation panel                         │
│ 64px │ 400px (resizable)│ flex:1 (min ~ 400px at 800px window)       │
└──────┴──────────────────┴────────────────────────────────────────────┘
```

| Property | Value |
|---|---|
| Container | `display:flex; height:100vh; overflow:hidden`; app wrapper accepts drag on titlebar area |
| Column 1 | Navigation rail: 64 px fixed (`--navbar-width`), background `--WDS-components-surface-nav-bar`, right border 1 px `--WDS-lines-divider` |
| Column 2 | Chat list / active section panel: default 400 px, resizable via 4 px drag handle (cursor `col-resize`), remembered per session |
| Column 3 | Conversation: `flex:1`; shows selected chat or the "empty" hero |
| Column divider | 1 px `--WDS-lines-divider`; the chat list drawer (archive/starred/labels) replaces column 2 with a slide-over, dimming it |
| Transitions | Rail item switch 150 ms; drawer slide 250 ms; narrow-window column swap 200 ms |

**Responsive behavior** (breakpoints verified in CSS: `max-width:767px`, `768–1024px`, `min-width:1025px`):

| Window width | Layout |
|---|---|
| ≥ 1025 px | Rail + chat list + conversation (3 columns) |
| 768–1024 px | Rail + chat list + conversation; the chat list narrows (min ~ 320 px) and header actions collapse into an overflow menu |
| ≤ 767 px | "Mobile" mode: rail collapses to a bottom tab bar or hidden; chat list occupies full width; opening a chat swaps the whole view to the conversation with a back button in its header. The navigation rail becomes a **bottom navigation bar** (Chats / Calls / Status / Channels / Communities) if the window is narrow and tall, or remains left rail if the window is wide but short — mirror the app: rail is dropped below 767 px, and a back button appears in the conversation header. |

## 4.2 macOS titlebar / window chrome

| Property | Value |
|---|---|
| Title bar style | `titleBarStyle: "Overlay"`, `hiddenTitle: true`, `decorations: true` (Tauri v2 config, see §7) |
| Traffic lights | Native, positioned inside the rail-less area; recommended `trafficLightPosition: { x: 19, y: 18 }` (logical px) |
| Height allocated | 28–38 px of the top of the app is draggable (`-webkit-app-region: drag` or Tauri `data-tauri-drag-region`); place it across the chat-list + conversation headers, **not** over buttons |
| Rail top | Rail starts below the traffic lights; the first rail icon sits ~ 38–50 px from the top |
| Fullscreen | `titleBarStyle` keeps traffic lights hidden in fullscreen; app content fills the window; no separate toolbar. Support `⌃⌘F` |
| Resize | Min 800×600; remember last size/position; restore on launch |
| Background | Window background `--WDS-background-wash-plain` (light `#FFFFFF`, dark `#161717`) to avoid white flash; no vibrancy in the app content (WhatsApp is opaque), but a subtle `sidebar` NSVisualEffectView under the rail is acceptable if you want native feel — the app itself does **not** do this |

## 4.3 Navigation rail (left, 64 px)

**Anatomy (top → bottom):**

1. Chats (`message-circle`) — default selected
2. Calls (`phone`)
3. Status (`circle-dashed`) — segmented ring; shows "recent updates" ring accent when unseen
4. Channels (`radio`)
5. Communities (`users-round`)
6. — flexible spacer —
7. Settings (`settings`) — pinned to the bottom
8. (optional, above Settings) Profile avatar 32 px with presence dot

| Metric | Value |
|---|---|
| Width | 64 px |
| Icon button | 40×40 px; glyph 24 px; `border-radius: 9999px` |
| Vertical rhythm | 8–10 px between buttons; 10 px top padding |
| Active state | Pill behind glyph 32×32–48×32 px, background `--WDS-accent-deemphasized` light (`#D9FDD3`) / `--WDS-accent-deemphasized` dark (`#103529`); glyph color `--WDS-content-action-emphasized` (`#1B8755` light / `#21C063` dark). Inactive glyph `--WDS-content-deemphasized` |
| Hover | `--WDS-surface-highlight` circle behind the glyph |
| Badge (unread count) | Green pill `#25D366` (`--WDS-persistent-activity-indicator`), text white 11 px, min-width 18 px, height 18 px, positioned top-right of the 24 px glyph box; hidden when 0 |
| Status ring | 2 px ring in `--WDS-persistent-activity-indicator` when there are unseen statuses; dimmed `--WDS-systems-status-seen` when all seen |
| Tooltip | Right-side tooltip, 12 px radius, surface `--WDS-surface-elevated-default`, text 13 px, appears after 500 ms |
| Behavior | Click switches the whole column 2 (chat list ↔ calls ↔ status ↔ channels ↔ communities). The rail never scrolls. Keyboard: `↑/↓` moves focus; `Enter` activates; focus ring 2 px accent |

## 4.4 Chat list panel (column 2)

**Top to bottom:** header (59 px) → search field → filter tabs → chat rows (virtualized) → optional "Archived" row.

### 4.4.1 Chat list header

| Element | Spec |
|---|---|
| Height | 59 px; background `--WDS-surface-default`; bottom divider optional 1 px |
| Title | "Chats" (or section name), 17 px / 600, `--WDS-content-default`; for Status: "Status"; Calls: "Calls"; Channels: "Channels"; Communities: "Communities" |
| Actions (right) | Icon buttons 40×40: New chat (`square-pen`), Menu (`more-vertical`) — the menu contains New group, New community, Linked devices, Starred messages, Select chats, Mark all as read, Settings, Log out |
| Hover | Icon buttons show circular `--WDS-surface-highlight` |

### 4.4.2 Search field

| Element | Spec |
|---|---|
| Container | 49–56 px tall incl. padding; background `--WDS-search-input-background` (`#F1EEEB` light / `#242626` dark); radius 8 px; margin 8–12 px |
| Leading icon | `search` 20 px, `--WDS-content-deemphasized` |
| Placeholder | "Search" (verified string), 15 px, `--WDS-content-deemphasized` |
| Focus | 1 px border `--WDS-content-action-emphasized` / `--WDS-accent`, background stays |
| Clear button | `x` when text non-empty |
| Behavior | Filters chats live (name + message content by default); `Esc` clears; `/` or the global search shortcut focuses it; arrow keys move the highlight; `Enter` opens the first result; show "No results found" empty state (verified string) |

### 4.4.3 Filter tabs

Chips row under search: **All**, **Unread**, **Favourites**, **Groups** (labels verified; the set is configurable in newer builds).

| State | Style |
|---|---|
| Unselected | transparent bg, text `--WDS-content-deemphasized`, 13 px / 500, padding 4 px 12 px, radius 9999 |
| Selected | background `--WDS-components-filter-surface-selected` (`#D9FDD3` / `#103529`), text `--WDS-content-action-emphasized`, 13 px / 600 |
| Hover | `--WDS-surface-highlight` |

### 4.4.4 Chat row

| Element | Spec |
|---|---|
| Height | 72 px (dense 64 px); padding 8 px 12–16 px; divider between rows inset 80 px from the left (after the avatar) |
| Avatar | 49 px circle (article), squircle for channels/business; status ring support |
| Row 1 | Contact/group name 17 px / 500, `--WDS-content-default`, truncate; timestamp right-aligned 12 px, `--WDS-content-deemphasized` |
| Row 2 | Preview 14–15 px, `--WDS-content-deemphasized`, single line truncate. Prefixes: “✓✓ ” for your last message, “Draft: ” red? (draft prefix uses `--WDS-secondary-negative` in the preview), typing indicator (animated 3 green dots), image/video/sticker with an icon prefix, voice note “🎤 0:12”, document “📄 name” |
| Right stack | unread badge (green pill `#25D366`, white 11–12 px text, min 20 px, radius 9999, pulses? no), muted `bell-off` 16 px, pinned `pin` 16 px — vertical stack bottom-right; row markers appear next to the preview line when the row is selected |
| Unread row | name + timestamp in `--WDS-content-default` bold-ish; read rows keep timestamp deemphasized |
| Hover | background `--WDS-surface-highlight`, avatar reveals a chevron-down affordance? No: WhatsApp shows a small caret on hover over the avatar area since 2024 — optional |
| Selected | background `--WDS-components-active-list-row`; left accent edge not used |
| Focus | 2 px accent outline offset −2 px |
| Context menu | Right-click: Archive/Unarchive, Mute/Unmute, Pin/Unpin, Mark as unread/read, Add to favourites, Add to list, Clear chat, Delete chat, Block, Exit group/Report (group), Close chat |
| Behavior | Click opens chat, swipe/drag not applicable on desktop; `⌘`-click multi-select supported via "Select chats"; rows virtualized |

### 4.4.5 Chat-list empty/edge states

- No chats: illustration + “No chats” / “Start a new chat” CTA.
- No search results: “No results found” centered, deemphasized.
- Archived chats row pinned above the list when archive exists: `archive` icon + "Archived" + count; opens the archived drawer.

## 4.5 Conversation panel

### 4.5.1 Conversation header (59 px)

```
┌──────────────────────────────────────────────────────────────────────────┐
│ (avatar40) Name  ·  presence        [search] [phone] [video] [kebab]     │
└──────────────────────────────────────────────────────────────────────────┘
```

| Element | Spec |
|---|---|
| Height | 59 px; bg `--WDS-surface-default`; bottom 1 px divider |
| Avatar | 40 px; click opens contact info drawer |
| Name | 16–17 px / 500, truncate to available width (not into buttons); group names show participant list as presence line |
| Presence | 13 px `--WDS-content-deemphasized`; “online” in `--WDS-content-action-emphasized`; “typing…” animated; “last seen today at 14:03”; in groups shows member names |
| Buttons | `search` (in-chat search), `video`, `phone`, `more-vertical` — each 40×40; in 768–1024 px collapse video/phone into the kebab; tooltips on hover |
| Search open | The header morphs into a search bar (see §4.11) |
| Business/verified | `badge-check` blue after name; business label under name |
| Behavior | `Esc` closes the chat (back to hero) when nothing else is open; kebab menu: Contact info, Select messages, Mute, Disappearing messages, Chat lock, Clear chat, Delete chat, Block, Report |

### 4.5.2 Message list

| Property | Value |
|---|---|
| Scroll container | `flex:1`, own scroll; wallpaper painted on the container (`--WDS-systems-chat-background-wallpaper` + doodle); `overflow-anchor` to keep position on prepend |
| Bubble column | Full width minus 8 px side padding; outgoing right-aligned, incoming left-aligned |
| Date chips | Sticky at top when scrolling (“TODAY”, “YESTERDAY”, “12/09/2026”), centered, uppercase 12–13 px / 500, chip style |
| Encryption notice | First item in every chat: lock icon 16 px + “Messages and calls are end-to-end encrypted…” 12–13 px, yellow bubble `#FFF0D4` light / `#1D1F1F` dark, deemphasized text |
| Unread divider | If opened with unread: 1 px accent line + "Unread messages" chip; auto-scrolls here on open |
| Jump to bottom | Floating circular button (`chevron-down`) bottom-right above composer when scrolled up; shows unread count badge |
| Load older | IntersectionObserver at the top: fetch previous page, preserve scroll offset; skeleton shimmer while loading |
| Virtualization | Windowed rendering with overscan ~ 10 screens; variable row heights cached by message id; “scroll to message” via `scrollIntoView({block:'center'})` |
| Hover actions | On bubble hover: `smile-plus` + `chevron-down` float at the top-right of the bubble (outgoing) / top-left (incoming); reaction bar with 6 quick emojis + plus |
| Selection mode | Row checkboxes appear; header switches to selection toolbar with count, forward/delete/star/copy/menu |

### 4.5.3 Bubble variants

| Variant | Spec |
|---|---|
| Text in/out | Colors per §1.4; radius 7.5; tail on last of group; link colors `--link`; emoji-only messages render larger (up to 3 emoji at 32–64 px, WhatsApp scales emoji-only text up to ~ 64 px) |
| Reply quote | Quote header inside bubble: 4 px accent bar, name (accent, 13 px / 600), snippet 13 px, overlay bg, radius 4; click scrolls to original |
| Forwarded | Italic "Forwarded" above text; "Forwarded many times" when ≥ 5 |
| Edited | “Edited” 11 px deemphasized appended near the meta (after timestamp) |
| Reactions | Pills under bubble edge (see §3.4); clicking opens reactor list; own reaction highlighted |
| Image | Max 65 % width; corner radius 7.5; caption below image inside bubble; `image` placeholder skeleton; download progress ring on hover |
| Album | 2–4 images in a grid (2×2) inside one bubble with rounded outer corners |
| Video | Thumbnail + centered play button (48 px circle, dark scrim), duration chip bottom-left, mute icon, GIF badge for GIFs |
| Voice note | 49 px avatar? no: play/pause 34 px circle; waveform bars 2 px wide, 1.5–2 px gap, accent for played portion; duration 12–13 px; mic icon + speed chip (1×/1.5×/2×) on the right; unplayed shows a green dot on the avatar (in list) |
| Document | File-type icon 40 px in a rounded square, name 15 px / 500 (2 lines), size/type caption 13 px, download icon button; tap opens preview |
| Sticker | Transparent background, no bubble chrome, max 200×200; hover reveals “⋯” menu; animated WebP/WebM supported |
| Location | Static map thumbnail (radius 7.5) + "Location" label + address; live location shows a countdown and pulsing dot |
| Contact | Avatar 40 px + name + phone; tap opens contact card |
| Link preview | Image (if any) 65 % width rounded top, title 15 px / 600, domain 12 px accent, description 13 px, all inside bubble above text |
| Poll | Question 15 px / 600; options as rows with progress bars (accent fill), vote count; “Select one/multiple”; voted state shows check on chosen |
| Event | Calendar tile with date, title, time, location, RSVP buttons |
| Payment/sticker/order cards | Card surface `--WDS-surface-elevated-default`, border `--WDS-lines-divider`, radius 7.5–16, CTA buttons |
| System messages | Centered chip ("Messages and calls are end-to-end encrypted", "You changed the group icon", "Missed voice call") |
| Call log entries | Centered or bubble with phone icon, "Voice call 12:34", status color (accent = answered, negative = missed) |
| Deleted message | "This message was deleted" italic deemphasized; "You deleted this message" |
| Disappearing messages | Timer icon in bubble meta; countdown chip in header |

### 4.5.4 Voice-note / media playback row

- Waveform: 30–40 bars, 2 px wide, 2 px gap, height 2–20 px varying; played color `--WDS-content-action-emphasized`, pending `--WDS-content-deemphasized`; scrub by click/drag; playback speed cycles 1×→1.5×→2×.
- Video player: full-screen overlay on click with top bar (back, sender, download), center play, bottom scrubber + volume.

## 4.6 Composer (62 px)

```
┌──────────────────────────────────────────────────────────────────────────┐
│ [smile] [plus]   ┌──────────────────────────────┐   [mic | send]         │
│                  │ Type a message               │                        │
└──────────────────────────────────────────────────────────────────────────┘
```

| Element | Spec |
|---|---|
| Height | 62 px min; grows with multiline input (max ~ 50 % of panel, then internal scroll) |
| Background | `--WDS-systems-chat-surface-composer` |
| Emoji button | `smile` 24 px in 40 px hit area, left; highlights when the picker is open |
| Attach button | `plus` 24 px in 40 px hit area; opens attachment menu (paperclip replaced by plus in the 2024+ design) |
| Input | `contenteditable` or textarea, 15 px, placeholder "Type a message" (`--WDS-content-deemphasized`); padding 9 px 12 px; no border; radius 8 px if a bubble-style input is used |
| Trailing button | When input empty and no text: `mic` (start PTT); when input non-empty: `send-horizontal`. WhatsApp shows mic by default; typing swaps to send |
| Right side | In-chat emoji reactions? No — right side carries send/mic only; there is also a `sticker` shortcut in the attach menu |
| Reply/edit banners | Above the composer inside the same surface: reply quote (with `x`), edit banner “Edit message”, both with left accent bar and preview text |
| Slow-mode/disappearing banner | Centered text row above composer when the chat has a disappearing timer |
| States | Focus outline none; the whole row is a drop target for file drag-and-drop; disabled when read-only (channels) shows “Only admins can message this group” style notices above |
| Behavior | `Enter` sends (configurable), `Shift+Enter` newline; `⌘B/I/U` formatting via markdown shortcuts; mention popup on `@`; emoji autocomplete on `:`; pasting an image opens a send preview; drag-drop files shows an overlay dropzone with preview thumbnails |

**PTT recording state:** input row swaps to a recording UI: red dot, timer `0:03`, slide-to-cancel affordance, trash icon (`trash-2`) to cancel, send. `⌃⌥R` starts, `⌥p` pauses, `⌃Enter` sends.

## 4.7 Attachment menu, emoji picker, sticker & GIF panels

**Attachment menu** (plus button): popover anchored bottom-left above composer, width ~ 300 px, sections:
- Media (Photos & videos `image`, Camera `camera`, Document `file-text`, Audio `file-audio`)
- Contact `contact-round`, Poll `chart-no-axes-column`, Event `calendar`
- Location `map-pin`, Live location `navigation` (if available)
- Quick actions: Sticker `sticker`, GIF `clapperboard`
Item: 48 px row, icon 24 px accent, label 15 px. Mobile widths become a bottom sheet grid (3×N).

**Emoji picker:** panel **573 px** wide ✅, opens above the composer anchored left; background `--WDS-surface-elevated-default`; radius 16; shadow L2; layout: search field on top (radius 8), skin-tone selector, category tabs (smileys, people, animals, food, travel, objects, symbols, flags), 8 columns × ~ 40 px emoji cells with 28 px glyphs; recently-used section; keyboard navigation with arrow keys; `Esc` closes.

**Sticker/GIF panel:** same footprint (~ 400–573 px wide), tabs: Stickers / GIFs / Emoji; sticker grid 3–4 columns, transparent backgrounds; GIF search input; favorites row.

## 4.8 Context menus (right-click)

| Context | Items (verified labels) |
|---|---|
| Chat list row | Archive chat, Mute notifications (submenu: 8 hours / 1 week / Always), Pin chat, Mark as unread, Add to favourites, Add to list, Clear chat, Delete chat, Block, Report, Exit group |
| Message bubble | Reply, Reply privately, React (emoji row inline), Forward, Star message, Copy, Pin message, Message info, Edit (own, ≤ 15 min), Delete (submenu: Delete for me / Delete for everyone), Download, Select messages |
| Media viewer | Download, Forward, Star, Reply, Copy, Show in chat, Message info, Delete |
| Rail item | Usually none (tooltips only) |
| Chat list header menu | New group, New community, Linked devices, Starred messages, Select chats, Mark all as read, Settings, Log out |

Menu styling: min-width 200 px, max ~ 360 px; item height 36–40 px, padding 8–10 px 16 px; hover `--WDS-surface-highlight`; radius 10 px per item; shortcuts right-aligned 12 px deemphasized; separator 1 px `--WDS-lines-divider` with 8 px vertical padding; submenus open on hover to the right.

## 4.9 Modals, dialogs and drawers

### 4.9.1 General dialog anatomy

- Backdrop: `rgba(0,0,0,.32)` (`--WDS-background-dimmer`), click dismisses unless `blockClose`.
- Surface: `--WDS-surface-elevated-default`, radius 32 px ✅, shadow L3, padding 24–32 px.
- Title: 24 px / 600 (`headline1`), centered on mac-style alerts or left-aligned in web.
- Footer: buttons right-aligned, gap 6 px (`--button-group-spacing`); primary = accent pill, secondary = borderless text button; danger actions use `--WDS-secondary-negative` text.
- Max width: 420–560 px depending on content; animate scale .96→1 + fade 200 ms.

### 4.9.2 Specific modals/drawers

| Modal | Contents |
|---|---|
| **New chat** | Search field "Search name or number"; sections: contacts list (avatar + name + about), “New group”, “New contact”, “Message yourself”; footer note "Your personal messages are end-to-end encrypted" |
| **New group** | Step 1: add participants (search + selected chips + list), Step 2: group subject (60 char limit), icon picker, “Group description” optional, create |
| **New community** | Wizard: name, description, add groups, privacy, create |
| **Profile drawer** | Right side panel ~ 400 px: avatar 212 px (editable for self), name, phone/about, media tabs; actions: mute, search, disappearing messages, chat lock; group variant: members list, add members, invite link, media, exit group |
| **Settings** | Full-screen 2-pane within the app: left list (280/360 px) + detail pane. Sections: Profile, Privacy, Chats (wallpaper, theme, font size), Notifications, Keyboard shortcuts, Help, Storage and data, + business settings. Header 77 px; back arrow on narrow. Profile settings: name, about, phone, avatar. Privacy: read receipts, last seen, profile photo, groups, status, blocked contacts, disappearing messages defaults, app lock. Chats: theme (Light/Dark/System), wallpaper (color + doodle tint + custom upload), font size. Notifications: message/group/call/status notifications, sounds, badge. Keyboard shortcuts: table of actions with keycap chips (see §6.4). |
| **Keyboard shortcuts** | Two-column list, headings “Navigation”, “Chat actions”, “Message actions”, “Formatting”; keycaps: 28 px tall, min 24 px wide, radius 6 px, background `--WDS-surface-emphasized`, 1 px border `--WDS-lines-divider`, 13 px / 600 |
| **Delete / clear confirm** | Title + description + Cancel / Delete buttons (danger), checkbox “Delete for everyone” when applicable |
| **Block contact** | Block / Cancel, “Block and report” link |
| **Link preview dialog** | Large preview card before sending a URL |
| **Media send preview** | Full-screen black overlay; bottom bar with caption input, hd toggle, crop/rotate, delete; send button accent |
| **Toast** | Bottom-center, dark surface `rgba(32,39,43,1)` / `#283943`, radius 4 px, min-width 288 px, text 14 px white, single-line + optional action in accent, auto-dismiss ~ 4 s, slide-up 200 ms |
| **QR pairing screen** | See §4.13 |
| **Lock screen** | Modal with app logo, passcode input (6 boxes), “Unlock”, error state; backdrop is the last UI blurred |

## 4.10 Search overlay (in-chat and global)

| Mode | Shortcut | Spec |
|---|---|---|
| In-chat search | `⌘F` | Replaces the conversation header: back arrow, input “Search…”, result count “1 of 12”, up/down buttons, `x` close. Results highlight in `--WDS-accent-deemphasized`/`#FFF3C7` for current match; auto-scrolls to match; `Enter` next, `⇧Enter` previous, `Esc` closes |
| Filter search | in-chat search → filter chip | Opens a filter panel: Media, Links, Docs, Audio, Date, From member; calendar picker for dates; results shown as a message list with sender + date |
| Global search | `⌘⌃/` (see §6.4) or clicking the chat-list search | Focuses the chat-list search; “Extended search” opens contacts/messages global results |
| Command palette | `⌘⌃K` | Quick switcher: jumps to chats and runs actions; list with avatar/icon + name + hint, filter as you type, arrows navigate, `Enter` executes |

Highlight color for matches inside text: `background: rgba(var(--WDS-accent-RGB),.4)` (`::selection` value) or legacy `--text-highlight` (#1B8755 light). Use 2 px underline for current match.

## 4.11 Status screen

- **Status list** (column 2): header "Status" with camera/text/pen actions; rows: “My status” (avatar 49 + plus), then “Recent updates” section (unseen = green ring 2 px, 49 px avatar), “Viewed updates” (gray ring), “Muted updates”; each row shows name + relative time.
- **Status viewer**: full-screen dark (`#0B141A`/`#000`) viewer over the app; top progress bars (64 segments), header with author avatar 32 + name + time, close `x`, kebab; bottom: reply input “Reply”, heart `heart`, share `forward`; tap right/left to advance; swipe-down/`Esc` closes; mute toggle top-right; caption area with text on gradient.
- Media: images fit contain with black bars, text statuses use colored backgrounds (accent variants), links show preview cards.
- Behavior: auto-advance 5 s (images) / video duration; pause on hold; muted by default; mark seen when viewed.

## 4.12 Calls screen

- **Call list** (column 2): header “Calls” + “Start a call” search icon; rows 72 px: avatar 49, name 16/500, direction icon (`phone-outgoing`/`phone-incoming`/`phone-missed` in red `--WDS-secondary-negative`), line “Today, 14:02 · 4 min”; right: call-back `phone`/`video` buttons on hover.
- **Call log row** semantics: missed = red name + red arrow; outgoing = green arrow; incoming = grey arrow; video calls get a camera glyph.
- **In-call UI** (full-screen overlay): remote video fills, local PiP 120×160 px draggable (radius 12, shadow); top bar: name + duration chip, encryption lock; bottom controls (56 px circles, dark scrim): camera toggle, mic toggle, screen share, raise hand, reactions `smile`, end call (red `#EA0038` circle, 64 px). Ringing: centered avatar 128 px pulsing, name 24 px, accept (green) / decline (red) 64 px buttons, "Ringing…" text.
- **Call link join** / group call variants: participant grid 2×2/3×3 aware, active-speaker ring accent.

## 4.13 Channels & Communities

### Channels
- **List** (rail → Channels): header + “Discover” entry; rows: channel avatar (squircle 49 px), name 17/500 + `badge-check` for verified, preview line, timestamp; unread badge; mute marker.
- **Viewer**: 3-column variant: channel list (400 px) + message list. Header: avatar 40 rounded, name + `badge-check`, follower count, mute/search/menu. Composer replaced with a bottom bar: reaction quick row, comments, share. Messages render like broadcasts (no incoming bubble; last-message footer shows views/forwards). Admin composer allows text/media/poll.
- **Channel preview** (not following): hero with cover image, description, follower count, Follow button (accent pill 36 px).

### Communities
- **List**: header “Communities”; row: community icon (squircle 49), name, “1 group · 3 members”, announcement badge; hover menu.
- **Home**: header with community avatar 40, name, member count; tabs: Announcements, Groups, Members, Media; announcement chat viewer; “+ New group” action; group rows with subgroup badges.

## 4.14 Empty states, QR pairing, lock screen

### Empty conversation hero (no chat selected) 🔎
The landing/pre-login variant was measured live; the in-app empty state mirrors it:

| Element | Light measured | Notes |
|---|---|---|
| Background | `#FCF5EB` (marketing cream) | outside the app shell; in-app empty state uses `--WDS-background-wash-plain` + doodle |
| Logo | WhatsApp logo 31×30 px, `#25D366` | do not copy; substitute an original mark/generic glyph |
| Card | white `#FFFFFF`, radius 25 px, centered, max-width ~ 532 px, padding 32 px | |
| Heading | “Scan to log in” 32 px / 400 (`display-small`), `#1D1F1F` | in-app: “Send and receive messages without keeping your phone online.” style copy |
| Body | 16 px, `rgba(0,0,0,.6)`, line-height 1.5 | |
| Steps list | numbered 1-2-3 with 24 px icon + 16 px text | |
| QR | 228×228 px inside a 2 px radius container (border 1 px `#E7E5E4`), regenerates with a spinner overlay | |
| Consent | “Stay logged in on this browser” checkbox 20 px | |
| Footer | “Log in with phone number” link; Terms & Privacy Policy 12 px | |
| Bottom-left | language selector; bottom-right “Don’t have an account? Get started” | |

### Lock screen
- Optional app-level screen lock (passcode). Centered lock icon, field, “Unlock” button; background blurs the last screen; 5-attempt cooldown; `⌥L` triggers lock.

### Toasts
See §4.9.2. Common toasts: “Chat muted”, “Chat archived”, “Message deleted”, “Couldn’t mute chat.” (verified strings; failure toasts use `--WDS-secondary-negative` text).

---

# 5. Layout measurements and wireframes

## 5.1 Consolidated measurement sheet

| # | Element | Value | Confidence |
|---|---|---|---|
| 1 | Navigation rail width | **64 px** | ✅ `--navbar-width` |
| 2 | Chat list panel width | **400 px**, drag-resizable ~300–500 px | 🟡 |
| 3 | Conversation panel width | `flex: 1`, min ~ 400 px at the 800 px window floor | 🟡 |
| 4 | Total minimum window | 800 × 600 px | 🟡 |
| 5 | Chat list / conversation header height | **59 px** | ✅ CSS |
| 6 | Chat row height | **72 px** | ✅ CSS |
| 7 | Composer height | **62 px** | ✅ `--height-pane-footer` |
| 8 | Full-screen section header | 77 px | ✅ |
| 9 | Status/calls/channels/community list row | 72 px (reuses chat row) | 🟡 |
| 10 | Settings list row | 52 px min | ✅ |
| 11 | Chat list avatar | 49 px | ✅ CSS |
| 12 | Header avatar | 40 px | ✅ CSS |
| 13 | Profile hero avatar | 212 px | ✅ CSS |
| 14 | Bubble max width | 65 % | ✅ CSS |
| 15 | Bubble radius / tail | 7.5 px / 8×13 px | ✅/🟡 |
| 16 | Emoji picker | 573 px wide | ✅ |
| 17 | Reactions panel | 388 px wide | ✅ |
| 18 | Drawer side panels | 280 / 360 px | ✅ |
| 19 | Announcement/system card max | 480 px | ✅ |
| 20 | Buttons | 32 / 36 / 44 px tall | ✅ |
| 21 | Dialog radius | 32 px | ✅ |
| 22 | Toast min width | 288 px | ✅ |
| 23 | Breakpoints | 767 / 768–1024 / 1025+ | ✅ CSS |
| 24 | Toasts | bottom center, 4 s | 🟡 |

## 5.2 Wireframe — normal window (≥ 1025 px), chat open

```
┌────────────┬──────────────────────────────┬──────────────────────────────────────────────────────┐
│ macOS      │  ●●●                         │                                                      │
│ traffic    ├──────────────────────────────┼──────────────────────────────────────────────────────┤
│ lights     │ Chats            ✎   ⋮       │  (40) Naomi Vasquez            🔍  📹  📞  ⋮         │ 59px headers
│            │                              │        online                                        │
│            ├──────────────────────────────┼──────────────────────────────────────────────────────┤
│   (💬)     │ 🔍 Search                    │                                                      │
│  Chats ③   │ [All][Unread][Favourites][Grp]│        ╭──────────────────────────╮                  │
│            ├──────────────────────────────┤        │ TODAY                    │                  │
│   (📞)     │ (49)  Naomi V.      14:02    │        ╰──────────────────────────╯                  │
│  Calls     │       ✓✓ Typing…             │                                                      │
│            │ ──────────────────────────── │   ╭────────────────────────────────╮                 │
│   (◌)      │ (49)  Team Design  13:58 ②   │   │ Messages and calls are end-to- │                 │
│  Status    │       📄 brief.pdf           │   │ end encrypted. Click to learn  │                 │
│            │ ──────────────────────────── │   ╰────────────────────────────────╯                 │
│   (📡)     │ (49)  Lina         13:41     │                                                      │
│  Channels  │       🎤 0:12                │            ╭─────────────────────────────╮            │
│            │ ──────────────────────────── │            │ Hey! Are we still on for    │            │
│   (👥)     │ (49)  Work chat    12:30 ─   │            │ tomorrow at 10?   14:01 ✓✓  │            │
│ Communities│       Marc: ok!    📌        │            ╰─────────────────────────────╯            │
│            │ ──────────────────────────── │                                                      │
│            │ ...virtualized rows...       │       ╭───────────────────────────────────────╮      │
│            │                              │       │ Sounds good — see you then!    14:02  │      │
│            │ ──────────────────────────── │       │                                        │      │
│    (⚙)     │ 🗄 Archived             12   │       ╰───────────────────────────────────────╯      │
│  Settings  │                              │                                                      │
│            │                              ├──────────────────────────────────────────────────────┤
│            │                              │ 😊  ＋ │ Type a message              │  🎤/➤      │ 62px
└────────────┴──────────────────────────────┴──────────────────────────────────────────────────────┘
     64px                 400px                                  flex:1
```

Annotations:

- The rail background is `#F7F5F3` (light) / `#1D1F1F` (dark); chat-list and conversation are `#FFFFFF` / `#161717`; the conversation **chat area** uses the wallpaper `#F5F1EB` / `#161717` + doodle, while its header and composer are surface colors.
- Unread badge `③` = green pill `#25D366`; `📌` pin marker; `−`/muted = `bell-off`.
- Date chip is sticky; bubbles have tails only on the last of a group; timestamps + ticks inline inside bubbles.

## 5.3 Wireframe — medium window (768–1024 px)

```
┌──────┬────────────────────┬──────────────────────────────────────┐
│ rail │ chat list (~320px) │ conversation (flex)                  │
│ 64px │                    │                                      │
│      │ Chats      ✎  ⋮    │ (40) Naomi        🔍  ⋮  (overflow)  │
│      │ 🔍 Search          │      online                          │
│      │ [All][Unread]      │                                      │
│      │ (49) Naomi  14:02  │        ╭────────────────────╮        │
│      │      ✓✓ Typing…    │        │ Hey! Are we still… │        │
│      │ (49) Team   13:58  │        ╰────────────────────╯        │
│      │ ...                │                                      │
│      │                    ├──────────────────────────────────────┤
│      │                    │ 😊 ＋ │ Type a message   │ 🎤/➤       │
└──────┴────────────────────┴──────────────────────────────────────┘
```

- Header actions collapse: video/phone move into the kebab; search remains visible.
- Drawers (profile, settings panes) overlay the conversation at 360 px instead of 400 px.

## 5.4 Wireframe — narrow window (≤ 767 px, "mobile" mode)

```
┌───────────────────────────────────────┐      ┌───────────────────────────────────────┐
│ Chats                       ✎    ⋮    │      │ ‹    (40) Naomi          🔍  ⋮        │
│ 🔍 Search                             │  →   │          online                       │
│ [All][Unread][Favourites][Groups]     │      │                                       │
│ (49) Naomi                   14:02    │      │   ╭─────────────────────────╮         │
│      ✓✓ Typing…                       │      │   │ Hey! Are we still on…   │         │
│ (49) Team Design            ② 13:58   │      │   ╰─────────────────────────╯         │
│ (49) Work chat               12:30    │      │                                       │
│ ...                                   │      │   ╭───────────────────────╮           │
│                                       │      │   │ Sounds good!    14:02 │           │
│                                       │      │   ╰───────────────────────╯           │
│                                       ├──────┼───────────────────────────────────────┤
│ 💬  📞  ◌  📡  👥            ⚙        │      │ 😊 ＋ │ Type a message    │ 🎤/➤       │
└───────────────────────────────────────┘      └───────────────────────────────────────┘
        list screen                                        conversation screen
```

- Rail becomes a bottom tab bar (56–64 px) with 5 destinations; Settings moves into the header kebab.
- Conversation opens as a full-screen push with a back arrow; `Esc` and `⌘[` go back.
- The emoji picker/attachment menu become bottom sheets spanning the width minus 16 px.

## 5.5 Wireframes — key dialogs and overlays

### New chat dialog

```
        ┌────────────────────────────────────────────┐
        │  ✕   New chat                              │
        │  🔍 Search name or number                   │
        │  ──────────────────────────────────────    │
        │  (36) 👤 New group                          │
        │  (36) 👤 New contact                        │
        │  (36) 👤 Message yourself                   │
        │  ──────────────────────────────────────    │
        │  CONTACTS                                   │
        │  (49) Alice Wong          Hey there!        │
        │  (49) Bruno Silva         At work           │
        │  (49) Carla Mendes        🎧 music           │
        │  ...                                        │
        │  🔒 Your personal messages are end-to-end   │
        │     encrypted                               │
        └────────────────────────────────────────────┘
                    width 420–560px, radius 32
```

### Settings (2-pane, full screen)

```
┌───────────────────────────────────────────────────────────────────────────────┐
│ ‹  Settings                                                                    │ 77px
├───────────────────────┬───────────────────────────────────────────────────────┤
│ (49) You              │  Account                                              │
│      +91 98…          │  ────────────────────────────────────────────────     │
│ ─────────────────     │  (49) Avatar 212px hero (tap to change)               │
│ 👤 Profile            │  Name            Your name                       ›    │
│ 🔒 Privacy            │  About           Available                       ›    │
│ 💬 Chats              │  Phone           +91 98…                              │
│ 🔔 Notifications      │                                                       │
│ ⌨ Keyboard shortcuts  │  ────────────────────────────────────────────────     │
│ 🗄 Storage and data    │  Two-step verification                           ›    │
│ ❓ Help               │                                                       │
│                       │                                                       │
└───────────────────────┴───────────────────────────────────────────────────────┘
        280–360px                                flex
```

Chats settings pane (high-value for the rebuild): Theme [Light|Dark|System chips], Chat wallpaper (color swatches + doodle intensity + custom), Chat font size [Small|Medium|Large], Enter-to-send toggle, Media auto-download rules.

### Keyboard shortcuts modal

```
┌────────────────────────────────────────────────────┐
│  Keyboard shortcuts                            ✕   │
│  ───────────────────────────────────────────────   │
│  Navigation                                        │
│   Next chat          ⌘⌃⇥        Previous  ⌘⌃⇧⇥     │
│   Close chat         Esc                           │
│  Chat actions                                      │
│   New chat           ⌘⌃N        New group ⌘⌃N      │
│   Search             ⌘⌃/        Search chat ⌘F     │
│   Archive chat       ⌘⌃E        Mute chat ⌘⌃M      │
│   Pin chat           ⌘⌃⇧P       Mark unread ⌘⌃⇧U   │
│  Message actions                                   │
│   Reply              ⌥R         Forward   ⌃⌥D      │
│   Star message       ⌥8         Edit last ⌘↑       │
│  Formatting                                        │
│   Bold ⌘B  Italic ⌘I  Strikethrough ⌘X  Code ⌘K    │
└────────────────────────────────────────────────────┘
```

### QR pairing screen (measured live)

```
┌───────────────────────────────────────────────────────────────────────────────┐
│  (logo 31×30 #25D366)                                             Download    │
│                                                                               │
│                  ┌──────────────────────────────────────────┐                 │
│                  │  Scan to log in                          │                 │
│                  │  Link with phone number instead.         │                 │
│                  │                                          │                 │
│                  │  1  Scan the QR code with your phone     │                 │
│                  │  2  Tap the link to open WhatsApp        │                 │
│                  │  3  Scan the QR code again to link       │                 │
│                  │                                          │                 │
│                  │        ┌──────────────────────┐          │                 │
│                  │        │    QR 228×228 px     │          │                 │
│                  │        └──────────────────────┘          │                 │
│                  │                                          │                 │
│                  │  ☑ Stay logged in on this browser        │                 │
│                  │  Log in with phone number                │                 │
│                  │  Your personal messages are end-to-end   │                 │
│                  │  encrypted                               │                 │
│                  │  Terms & Privacy Policy                  │                 │
│                  └──────────────────────────────────────────┘                 │
│    language ▾                                          Don't have an account? │
└───────────────────────────────────────────────────────────────────────────────┘
     cream #FCF5EB background · white card radius 25px · heading 32px #1D1F1F
```

### Profile drawer / context menu / attachment menu (docked right and above composer)

```
┌──────────────────────────────┐        ┌───────────────────────┐   ┌──────────────────┐
│  ✕   Contact info            │        │ Reply…          ⌥R    │   │ 🖼 Photos        │
│  (212) Naomi Vasquez         │        │ React           ›     │   │ 📄 Document      │
│        +91 98…               │        │ Forward…        ⌃⌥D   │   │ 📍 Location      │
│        "Available"           │        │ Star message    ⌥8    │   │ 👤 Contact       │
│  ───────────────────────     │        │ Copy                  │   │ 📊 Poll          │
│  🔔 Mute        🔍 Search     │        │ Pin message           │   │ 📅 Event         │
│  ⏱ Disappearing ▶            │        │ Message info          │   │ ────────────     │
│  🔒 Chat lock                 │        │ Delete            ›   │   │ 🎨 Sticker       │
│  ───────────────────────     │        └───────────────────────┘   │ 🎞 GIF           │
│  Media, links, docs   12   ›  │          min 200px, radius 10      └──────────────────┘
│  Starred messages          ›  │                                    ~300×260, radius 12
└──────────────────────────────┘
        400px (360 narrow)
```

---

# 6. Interactions, keyboard and performance

## 6.1 Interaction states

| State | Treatment |
|---|---|
| Hover (icon buttons) | Circular background `--WDS-surface-highlight`, 150 ms ease-out |
| Hover (list rows) | Full-row background `--WDS-surface-highlight`; timestamp turns slightly stronger |
| Hover (bubbles) | None by default; on hover the reaction/menu affordances fade in over 150 ms |
| Active/pressed | `--WDS-surface-pressed` overlay (`rgba(0,0,0,.2)` light, `rgba(255,255,255,.2)` dark) or the app’s 0.97 scale on buttons (buttons: opacity/scale micro-press) |
| Selected (list) | `--WDS-components-active-list-row`; persists while the chat is open |
| Focus (keyboard) | `outline: 2px solid var(--WDS-accent)` with 1–2 px offset; the app switches to “keyboard mode” on first key and shows focus rings only then ✅ (`useWAWebIsKeyboardUser`) |
| Disabled | 40 % opacity (`--button-disabled-opacity`), `cursor: default`, no hover |
| Loading | Skeleton shimmer (`--glimmer-*` tokens: rectangle heights 44/60/70 px) 1.5 s loop; spinners rotate 1 s linear |
| Drag over composer | Overlay dropzone with dashed accent border and a 96 px preview tile |
| Error | Inline text `--WDS-secondary-negative`; destructive buttons use danger text/background |

**Cursor rules:** pointer on all clickable; `text` in inputs and message text; `col-resize` on the panel divider; `grab/grabbing` on media drag and the local PiP call tile; `not-allowed` only on truly disabled controls.

## 6.2 Context menus and secondary interactions

- Right-click opens the context menu at the pointer (flip to fit viewport, 8 px margin); `Ctrl+click` on macOS opens the same menu; `Esc`/click-away closes; `↑/↓` navigate, `→` opens submenu, `Enter` activates, type-ahead is not required.
- Double-click a message opens Message info; double-click the header name opens contact info; double-click an image opens the viewer.
- Long-press is not used on desktop (mobile-only).
- The native WebView context menu must be suppressed globally; allow the system edit menu inside inputs (`Cut/Copy/Paste/Select All`, spell-check) — see §7.7.

## 6.3 Scrolling, resizing, drag & drop

| Behavior | Spec |
|---|---|
| Chat list scroll | Independent; virtualized; scrollbar 6 px overlay (visible on hover/scroll); “Archived” row pinned above when present |
| Message scroll | Independent; bottom-anchored on open when unread == 0; when unread > 0, anchor at the unread divider; new incoming messages while scrolled up show a jump button with count; sending scrolls to bottom smoothly |
| Sticky date chips | Top chip replaces as you scroll sections |
| Scroll restoration | Per-chat scroll offset is cached in memory (and restored when the app reopens to the same chat if the message store is local) |
| Panel resize | 4 px hit strip on the chat list’s right edge; drag with `col-resize`; live resize; persist width |
| Drag & drop | Files onto the conversation → send preview overlay; text/image drops into the composer; drag a message? not supported; drag the local call tile |
| Pinch / ⌘scroll | Zoom the app UI: `⌘+`, `⌘-`, `⌘0` change the WebView zoom (browser-like), matching the app’s zoom actions |

## 6.4 Keyboard shortcuts

> **Source:** extracted from the production shortcut engine (`WAWebKeyboardShortcuts`) on 2026-09-13. macOS combos below are the engine’s own display strings. ⚠️ Some combos intentionally avoid browser conflicts by combining `⌘` and `⌃`; a few collide across contexts (archive vs emoji panel, italic vs inline code) because actions are registered context-sensitively. Third-party lists often show single-modifier variants — validate against the reference app’s own “Keyboard shortcuts” modal when in doubt.

**Navigation**

| Action | macOS | Windows/Linux |
|---|---|---|
| Search (chats) | `⌘⌃/` ⚠️ some sources say `⌘/` | `Ctrl+Alt+/` or `Ctrl+F` |
| Search in chat | `⌘F` | `Ctrl+F` (context) / `Ctrl+Shift+F` |
| Extended search | via search field filter | — |
| Next chat | `⌘⌃⇥` (also `⌘⌃}`) | `Ctrl+]` / `Ctrl+Tab` |
| Previous chat | `⌘⌃⇧⇥` (also `⌘⌃{`) | `Ctrl+[` / `Ctrl+Shift+Tab` |
| Close chat/panel | `Esc` | `Esc` |
| Open settings | `⌘⌃,` ⚠️ macOS convention `⌘,` | `Ctrl+Alt+,` |
| Command palette | `⌘⌃K` | `Ctrl+Alt+K` |
| Lock app | `⌘⌃L` | `Ctrl+Alt+L` |
| Toggle theme (dev) | `⌘⌃T` | `Ctrl+Alt+T` |

**Chat actions**

| Action | macOS | Windows/Linux |
|---|---|---|
| New chat | `⌘⌃N` (Safari: `⌃N`) | `Ctrl+Alt+N` |
| New group | `⌘⌃N` ⚠️ (same map; context-resolved) | `Ctrl+Alt+N` |
| Open profile | `⌘⌃P` | `Ctrl+Alt+P` |
| Archive / unarchive chat | `⌘⌃E` | `Ctrl+Alt+E` |
| Mute / unmute chat | `⌘⌃M` | `Ctrl+Alt+M` |
| Pin / unpin chat | `⌘⌃⇧P` | `Ctrl+Alt+Shift+P` |
| Mark as unread | `⌘⌃⇧U` | `Ctrl+Alt+Shift+U` |
| Label chat | `⌘⌃⇧L` | `Ctrl+Alt+Shift+L` |
| Contact us | `⌘⌃H` | `Ctrl+Alt+H` |

**Message actions**

| Action | macOS | Windows/Linux |
|---|---|---|
| Reply | `⌥R` | `Alt+R` |
| Reply privately | `⌃⌥R` | `Ctrl+Alt+R` |
| Forward | `⌃⌥D` | `Ctrl+Alt+D` |
| Star message | `⌥8` | `Alt+8` |
| Edit last message | `⌘↑` | `Ctrl+↑` |
| Open (selected chat/item) | `Enter` | `Enter` |
| Open attachment dropdown | `⌥A` | `Alt+A` |
| Block chat | `⌃B` | `Ctrl+B` |
| Start PTT recording | `⌃⌥R` | `Ctrl+Alt+R` |
| Pause PTT recording | `⌥P` | `Alt+P` |
| Send PTT | `⌃Enter` | `Ctrl+Enter` |
| Voice-note speed ± | `⌘⌃>` / `⌘⌃<` | `Ctrl+Alt+>` / `<` |
| Open emoji panel | `⌘⌃E` | `Ctrl+Alt+E` |
| Open GIF panel | `⌘⌃G` | `Ctrl+Alt+G` |
| Open sticker panel | `⌘⌃S` | `Ctrl+Alt+S` |

**Text formatting (composer)**

| Action | macOS | Windows/Linux |
|---|---|---|
| Bold | `⌘B` | `Ctrl+B` |
| Italic | `⌘I` | `Ctrl+I` |
| Strikethrough | `⌘X` | `Ctrl+Shift+X` (engine: `Ctrl+X`) |
| Inline code | `⌘I` ⚠️ collides with italic | `Ctrl+I` |
| Code block | `⌘K` | `Ctrl+K` |
| Numbered list | `⌘⇧7` | `Ctrl+Shift+7` |
| Bulleted list | `⌘⇧8` | `Ctrl+Shift+8` |
| Quote | `⌘⇧.` | `Ctrl+Shift+.` |
| Zoom in / out / reset | `⌘+` `⌘-` `⌘0` | `Ctrl+=` `Ctrl+-` `Ctrl+0` |

**Calls** (in-call): `V` camera, `M` mute, `R` reactions, `H` raise hand, `S` screen share, `W` end call (bare keys, no modifiers — only while a call is active).

**Other system keys to implement:** `⌘,` settings (macOS HIG), `⌘W` close window (menu), `⌘H` hide app, `⌥⌘H` hide others, `⌘Q` quit, `⌘M` minimize, `⌃⌘F` fullscreen, `⌘[`/`⌘]` back/forward navigation in drawers, `⌘R` reload (dev), `⌘⇧R` hard reload (dev), `F6` focus next pane (Windows convention; optional on mac).

## 6.5 Virtualization and performance

- **Chat list:** windowed list (row height fixed 72 px) with ±10 rows overscan; avatars lazy-loaded; each row memoized by `{chatId, lastMsgId, unread, selected}` signature.
- **Message list:** windowed with variable heights. Recommended model: maintain a measured-height cache (`ResizeObserver`), estimate initial heights (text 40–120 px, image by aspect-ratio placeholder), ±10 viewports overscan, and an IntersectionObserver at both ends to trigger history load and trim.
- **Images:** render a fixed-height placeholder from the message’s width/height metadata; lazy-load with `loading="lazy"` and decode off the main thread (`img.decode()`); blob URLs revoked on unmount.
- **Budgets:** scroll ≥ 55 FPS on a 2019 MacBook Air; first paint of a chat < 100 ms after click; no layout thrash — use CSS containment (`contain: content`) on rows; avoid re-rendering the composer on message list updates (split stores).
- **Sticky behaviors:** date chip and “jump to bottom” are absolutely positioned; do not use sticky positioning inside the virtualizer.
- **Data:** message pages of 50 (`DEFAULT_BATCH_SIZE = 50` observed in the app), max batch 500.

## 6.6 Accessibility

- Roles: rail = `navigation` with `role="tablist"`-like semantics or list of buttons; chat list = `role="listbox"` / `option`; message list = `role="log"` with `aria-live="polite"`; composer = `role="textbox" aria-multiline`; dialogs `role="dialog" aria-modal`.
- Every icon-only button needs an accessible label (use the string table, e.g. "Search", "New chat", "Mute", "Archive chat").
- Delivery ticks expose text alternatives: “Sending”, “Sent”, “Delivered”, “Read”, “Failed”.
- Unread counts announced via `aria-label` on the rail item ("Chats, 3 unread").
- Focus trap in modals; return focus to the invoking element; `Esc` closes; `Tab` cycles within.
- Support `prefers-reduced-motion` (disable shimmer/pop/parallax) and `prefers-contrast: more` (bundle contains high-contrast variants ✅).
- Contrast: body text `#0A0A0A` on `#FFFFFF` and `#FAFAFA` on `#161717` exceed AAA; deemphasized text is intentionally 60 % opacity (AA large only) — do not reduce further.
- Respect the system “Increase contrast” and “Reduce transparency” settings (Tauri: expose via media queries; the app treats them as theme variants).

---

# 7. macOS integration (Tauri v2 + WKWebView)

## 7.1 Window configuration (recommended `tauri.conf.json`)

```json
{
  "app": {
    "windows": [
      {
        "label": "main",
        "title": "WhatsApp",
        "width": 1280,
        "height": 800,
        "minWidth": 800,
        "minHeight": 600,
        "resizable": true,
        "titleBarStyle": "Overlay",
        "hiddenTitle": true,
        "trafficLightPosition": { "x": 19, "y": 18 },
        "theme": null,
        "dragDropEnabled": true,
        "acceptFirstMouse": true,
        "zoomHotkeysEnabled": false
      }
    ]
  }
}
```

- `trafficLightPosition` requires `"titleBarStyle": "Overlay"` and `decorations: true` (verified in Tauri v2 config docs) ✅.
- `zoomHotkeysEnabled: false` disables the WebView’s built-in zoom hotkeys if you implement app-level zoom.
- To remember size/position, use the window-state plugin, or `appWindow.setSize/Position` with persisted values.

## 7.2 Traffic-light inset and draggable regions

- Allocate the top **38 px** of the window: the chat-list header and conversation header start below the titlebar? No — the app’s headers fill to the top; the traffic lights overlay the rail/top-left area. Recommended: keep a transparent 38 px strip at the top of the **rail** for the lights and make the headers start at y=0 but add `padding-left` on the traffic-light area only if the rail content would collide.
- Draggable region: apply `data-tauri-drag-region` to the rail’s empty top strip and to the header background (not to buttons), plus `-webkit-app-region: drag` on the titlebar strip in CSS; buttons must set `-webkit-app-region: no-drag`.
- Double-click on the drag region must toggle maximize (macOS convention) — Tauri handles this when the drag region is set.

## 7.3 Material/vibrancy

- WhatsApp paints opaque surfaces; no vibrancy inside panels. If you want the native feel, a `windowEffects` `sidebar` material behind the 64 px rail is acceptable but will read lighter/darker than the app’s `#F7F5F3`/`#1D1F1F`. If used, set `state: "followsWindowActiveState"` and keep the rail’s own background at ~92 % opacity.
- Dark titlebar: when the app theme is dark set `window.setTheme('dark')`; Tauri supports `theme` on the window and `appWindow.setTheme`.
- Avoid `transparent: true` (requires the macOS private API flag and breaks App Store distribution).

## 7.4 Fullscreen and window behavior

- Native fullscreen (`⌃⌘F`, green button) hides traffic lights and extends the webview; keep headers draggable so the toolbar area still works; no app-level “custom fullscreen”.
- Restore last window size/position/maximized state on launch; keep the window single-instance (`tauri-plugin-single-instance`).
- Reopen from Dock re-focuses the existing window. Closing the window quits the app (WhatsApp behavior on macOS: window close keeps the app in the Dock? The native app quits when the last window closes — implement quit-on-close; the Dock icon stays while running with no window only if you decide to keep it).
- `Esc` never quits; it closes the topmost overlay/chat.

## 7.5 Menu bar structure (recommended, mirroring the app + macOS HIG)

Tauri v2 `MenuBuilder` with `PredefinedMenuItem`s. Accelerators shown as they should appear to the user (match §6.4; native menus should use `⌘`-only versions for app-level commands and let the webview handle the rest — menu items can send events to the frontend when clicked).

```
WhatsApp
  About WhatsApp                      (predefined about)
  ───────────
  Settings…                    ⌘,     → emit "open-settings"
  ───────────
  Services                            (predefined)
  ───────────
  Hide WhatsApp                ⌘H     (predefined)
  Hide Others                  ⌥⌘H    (predefined)
  Show All                            (predefined)
  ───────────
  Quit WhatsApp                ⌘Q     (predefined)

File
  New Chat                     ⌘N     → emit "new-chat"        (macOS-native accelerator)
  New Group                    ⌘⇧N    → emit "new-group"
  ───────────
  Close Window                 ⌘W     (predefined close window)

Edit
  Undo/Redo, Cut/Copy/Paste, Select All (predefined; routed to the composer)
  ───────────
  Find in Chat                 ⌘F     → emit "search-in-chat"

View
  Toggle Sidebar               ⌘⇧S    → emit "toggle-sidebar"
  ───────────
  Actual Size                  ⌘0     → emit "zoom-reset"
  Zoom In                      ⌘+     → emit "zoom-in"
  Zoom Out                     ⌘-     → emit "zoom-out"
  ───────────
  Enter Full Screen            ⌃⌘F    (predefined)

Window
  Minimize                     ⌘M     (predefined)
  Zoom                                (predefined)
  Bring All to Front                  (predefined)

Help
  WhatsApp Help Center                → open external URL
  Keyboard Shortcuts                  → emit "open-shortcuts"
  Report a Problem…                   → emit "report-bug"
```

## 7.6 Dock badge and notifications

- **Dock badge:** use `getCurrentWindow().setBadgeCount(n)` (app-wide; macOS supported; Tauri ≥ 2.1) ✅; `setBadgeLabel` is macOS-only and can show a short label instead of a number. Clear with `undefined`.
- **Unread total:** sum of unread across chats; update on incoming/read events; debounce 200 ms.
- **Notifications:** `@tauri-apps/plugin-notification` (`isPermissionGranted` → `requestPermission` → `sendNotification`) ✅.
  - Notification content: title = chat name (group: “Sender in Group”), body = message text/media label; use the sender avatar via attachment/icon when possible.
  - Click → focus window and open the chat (`notification.click` listener on the Rust side or frontend event).
  - Respect per-chat mute and global notification settings; status updates and channel posts use their own toggles.
  - Notification permission must be requested after first user action; store the result; show the in-app banner “Show notification banner” hint if denied (verified string exists).
- **Sound:** play the app’s own alert sound (do not copy WhatsApp’s audio); 0.6 s ping, volume per settings; no sound for muted chats.
- **Badge + notifications must follow Do Not Disturb / Focus** — macOS handles this natively for notifications; badge is not suppressed by DND.

## 7.7 WKWebView specifics

| Topic | Guidance |
|---|---|
| Right-click | Suppress the default WebView menu on app surfaces (`oncontextmenu` prevent + custom menu); allow the default menu on text inputs so Cut/Copy/Paste/Spellcheck remain |
| Text selection | `user-select: none` on chrome (rail, headers, list rows), `user-select: text` on message text and inputs; `-webkit-user-select` prefix for older WKWebView |
| Momentum scroll | `-webkit-overflow-scrolling: touch` not needed on macOS; use native overflow |
| Font smoothing | `-webkit-font-smoothing: antialiased` for SF Pro consistency |
| Autofill/zoom | Disable pinch zoom (`maximum-scale=1`), block double-tap zoom; allow `⌘+/-` only through app zoom |
| External links | Intercept `http(s)` in message content → open in the default browser via `tauri-plugin-opener`; `mailto:`/`tel:` via the system handler; never navigate the WebView |
| Drag & drop | Tauri `dragDropEnabled: true`; handle `tauri://drag-drop` events to attach files; disable dropping on chrome elements |
| Permissions | Camera/mic usage descriptions in `Info.plist` (`NSCameraUsageDescription`, `NSMicrophoneUsageDescription`) for calls; speech recognition only if implemented |
| Appearance | Follow system theme by default; watch `prefers-color-scheme` change to swap tokens live |
| Scrolling perf | `will-change: transform` sparingly on pinned chips; `content-visibility: auto` on heavy rows (test carefully with virtualization) |

## 7.8 App lifecycle and OS conventions

- Single instance; second launch focuses the existing window.
- Restore the previously selected section (Chats/Calls/…) and last chat; show a splash only for cold start < 400 ms.
- `⌘,` opens Settings (menu item + accelerator), not the WebView’s default.
- About panel uses the app name/version; no WhatsApp trademarks in a clone.
- Respect “Reduce motion”, “Increase contrast”, “Reduce transparency”, and system accent? (WhatsApp keeps its own accent; do not follow system accent color).
- Right-to-left locales: mirror the entire layout (`dir="rtl"`), swap chevrons and bubble sides.

---

# 8. Sources, conflicts, and verification checklist

## 8.1 Sources used (all public/local; no login)

| Source | URL / path | Date |
|---|---|---|
| WhatsApp Web production CSS (v5 bundle) | `https://static.whatsapp.net/rsrc.php/v5/yk/l/0,cross/aaK18xivDacKkmsA6qiUsNOB_OY8k1KqH.css` | 2026-09-13 |
| WhatsApp Web landing page (live DOM/computed styles) | `https://web.whatsapp.com/` (public QR screen) | 2026-09-13 |
| WhatsApp Web JS bundles (`en_US-j`) | extracted `WAWebKeyboardShortcuts`, `WAWebThemeContext`, `WAWebDropdown.react`, string table | 2026-09-13 |
| macOS app 26.33.73 | `/Applications/WhatsApp.app` (`Info.plist`, `Assets.car`) | installed build |
| WhatsApp Help Center | “About keyboard shortcuts” `faq.whatsapp.com/6204576529560565` | 2026 |
| Third-party shortcut/UI articles | wwebcustomizer.com (2026 list), XDA, GadgetsNow FAQ | 2025–2026 |
| Tauri v2 docs | config reference (`trafficLightPosition`, `windowEffects`), window API (`setBadgeCount`, `setBadgeLabel`), notification plugin | 2026 |

## 8.2 Conflict register

| # | Conflict | Newest / preferred | Older claim | Resolution |
|---|---|---|---|---|
| 1 | Palette generation | **WDS 2025-2026** (`#161717`, `#242626`, `#144D37`, `#1DAA61`, `#F5F1EB`) | Classic (`#111B21`, `#202C33`, `#005C4B`, `#00A884`, `#EFEAE2`) | Use WDS values; classic tokens remain only for legacy Meta components |
| 2 | macOS search shortcut | Engine: `⌘⌃/` | Articles: `⌘/` | Implement engine value; additionally accept `⌘/` as an alias (harmless) |
| 3 | macOS settings shortcut | Engine: `⌘⌃,` | macOS norm + articles: `⌘,` | Menu uses `⌘,`; webview accepts both |
| 4 | “Search in chat” shortcut | Engine: `⌘F` | 2021 FAQ: `⌘⌃⇧F` | Use `⌘F` (current engine) |
| 5 | Header height | CSS: `59px` | “New design” articles/designs use 60–64 px | Use 59 px; if matching a specific screen recording shows 64 px, treat as a scaled capture |
| 6 | Chat-list width | CSS contains a `width:400px` panel rule; community reports resizable | Some clones use 30 % width | Default 400 px, make it drag-resizable |
| 7 | Link color in messages | Legacy `--link` still defined: `#027EB5` light / `#53BDEB` dark | WDS `--WDS-content-external-link` is green `#1B8755` / `#21C063` | Use `--link` for hyperlinks inside message text; use the green token for external-link CTAs |
| 8 | Web font | Live web app uses Roboto Variable | Native macOS app uses SF Pro (`--font-family-apple`) | Tauri build: use `system-ui` (SF Pro) to match macOS; keep Roboto stack behind a flag |
| 9 | Rail badge color | `#25D366` (`--WDS-persistent-activity-indicator`) | Older `#00A884` | Use `#25D366`; text white |
| 10 | Emoji picker width | 573 px (app constant) | Older clones ~ 400 px | Use 573 px |

## 8.3 Verification checklist for the implementation

- [ ] Toggle light/dark in Settings and compare every surface against §1.2–1.4.
- [ ] Verify bubble tail direction/alignment on the last bubble of a group, both sides, including replies/reactions.
- [ ] Verify unread marker and jump-to-bottom badge behavior with a chat that has 0 / 1 / 50 unread messages.
- [ ] Verify keyboard shortcuts against the reference app’s in-app modal (see §6.4 caveats).
- [ ] Verify window at 800×600 and in fullscreen; traffic lights never overlap interactive UI.
- [ ] Verify chat-list resize limits and persistence.
- [ ] Verify Dock badge and notification click-through.
- [ ] Verify emoji picker/reactions panel widths (573 / 388 px) and that they flip at screen edges.
- [ ] Verify voice-note waveform scrubbing and speed cycling.
- [ ] Verify `prefers-reduced-motion` and `prefers-contrast` variants.

---

# Appendix A — Complete WDS token block (copy-paste)

> Values extracted from the production CSS on 2026-09-13. Light values are the default; the dark block overrides them. Use these exact variable names or map them 1:1 to your own theme.

```css
/* ===== WhatsApp WDS tokens — LIGHT (default) ===== */
:root {
  --WDS-accent: #1DAA61;
  --WDS-accent-RGB: 29, 170, 97;
  --WDS-accent-deemphasized: #D9FDD3;
  --WDS-accent-deemphasized-RGB: 217, 253, 211;
  --WDS-accent-emphasized: #15603E;
  --WDS-accent-emphasized-RGB: 21, 96, 62;
  --WDS-background-dimmer: rgb(0, 0, 0, .32);
  --WDS-background-dimmer-RGB: #1DAA61;
  --WDS-background-elevated-wash-inset: #F7F5F3;
  --WDS-background-elevated-wash-inset-RGB: 247, 245, 243;
  --WDS-background-elevated-wash-plain: #FFFFFF;
  --WDS-background-elevated-wash-plain-RGB: 255, 255, 255;
  --WDS-background-wash-inset: #F7F5F3;
  --WDS-background-wash-inset-RGB: 247, 245, 243;
  --WDS-background-wash-plain: #FFFFFF;
  --WDS-background-wash-plain-RGB: 255, 255, 255;
  --WDS-components-active-list-row: rgba(194, 189, 184, .15);
  --WDS-components-active-list-row-RGB: 194, 189, 184;
  --WDS-components-filter-surface-selected: #D9FDD3;
  --WDS-components-filter-surface-selected-RGB: 217, 253, 211;
  --WDS-components-outline-profile-photo: rgba(0, 0, 0, .1);
  --WDS-components-outline-profile-photo-RGB: 0, 0, 0;
  --WDS-components-platform-gesture-bar: rgba(0, 0, 0, .5);
  --WDS-components-platform-gesture-bar-RGB: 0, 0, 0;
  --WDS-components-platform-status-bar: rgba(0, 0, 0, .8);
  --WDS-components-platform-status-bar-RGB: 0, 0, 0;
  --WDS-components-profile-photo-content-brown: #855538;
  --WDS-components-profile-photo-content-brown-RGB: 133, 85, 56;
  --WDS-components-profile-photo-content-cobalt: #0063CB;
  --WDS-components-profile-photo-content-cobalt-RGB: 0, 99, 203;
  --WDS-components-profile-photo-content-gray: #757778;
  --WDS-components-profile-photo-content-gray-RGB: 117, 119, 120;
  --WDS-components-profile-photo-content-green: #1B8755;
  --WDS-components-profile-photo-content-green-RGB: 27, 135, 85;
  --WDS-components-profile-photo-content-orange: #C4532D;
  --WDS-components-profile-photo-content-orange-RGB: 196, 83, 45;
  --WDS-components-profile-photo-content-pink: #D42A66;
  --WDS-components-profile-photo-content-pink-RGB: 212, 42, 102;
  --WDS-components-profile-photo-content-purple: #5E47DE;
  --WDS-components-profile-photo-content-purple-RGB: 94, 71, 222;
  --WDS-components-profile-photo-content-red: #B80531;
  --WDS-components-profile-photo-content-red-RGB: 184, 5, 49;
  --WDS-components-profile-photo-content-sky-blue: #027EB5;
  --WDS-components-profile-photo-content-sky-blue-RGB: 2, 126, 181;
  --WDS-components-profile-photo-content-teal: #028377;
  --WDS-components-profile-photo-content-teal-RGB: 2, 131, 119;
  --WDS-components-profile-photo-content-yellow: #9D6C2C;
  --WDS-components-profile-photo-content-yellow-RGB: 157, 108, 44;
  --WDS-components-profile-photo-status-ring-close-friends: #C15ADD;
  --WDS-components-profile-photo-surface-brown: #F4DED1;
  --WDS-components-profile-photo-surface-brown-RGB: 244, 222, 209;
  --WDS-components-profile-photo-surface-cobalt: #D2E8FE;
  --WDS-components-profile-photo-surface-cobalt-RGB: 210, 232, 254;
  --WDS-components-profile-photo-surface-gray: #EEEEEE;
  --WDS-components-profile-photo-surface-gray-RGB: 238, 238, 238;
  --WDS-components-profile-photo-surface-green: #D9FDD3;
  --WDS-components-profile-photo-surface-green-RGB: 217, 253, 211;
  --WDS-components-profile-photo-surface-orange: #FEE2D8;
  --WDS-components-profile-photo-surface-orange-RGB: 254, 226, 216;
  --WDS-components-profile-photo-surface-pink: #FFDAE7;
  --WDS-components-profile-photo-surface-pink-RGB: 255, 218, 231;
  --WDS-components-profile-photo-surface-purple: #E8E0FF;
  --WDS-components-profile-photo-surface-purple-RGB: 232, 224, 255;
  --WDS-components-profile-photo-surface-red: #FBD8DC;
  --WDS-components-profile-photo-surface-red-RGB: 251, 216, 220;
  --WDS-components-profile-photo-surface-sky-blue: #CAECFA;
  --WDS-components-profile-photo-surface-sky-blue-RGB: 202, 236, 250;
  --WDS-components-profile-photo-surface-teal: #CBF2EE;
  --WDS-components-profile-photo-surface-teal-RGB: 203, 242, 238;
  --WDS-components-profile-photo-surface-yellow: #FFF0D4;
  --WDS-components-profile-photo-surface-yellow-RGB: 255, 240, 212;
  --WDS-components-surface-nav-bar: #F7F5F3;
  --WDS-components-surface-nav-bar-RGB: 247, 245, 243;
  --WDS-content-action-default: #0A0A0A;
  --WDS-content-action-default-RGB: 10, 10, 10;
  --WDS-content-action-emphasized: #1B8755;
  --WDS-content-action-emphasized-RGB: 27, 135, 85;
  --WDS-content-deemphasized: rgba(0, 0, 0, .6);
  --WDS-content-deemphasized-RGB: 0, 0, 0;
  --WDS-content-default: #0A0A0A;
  --WDS-content-default-RGB: 10, 10, 10;
  --WDS-content-disabled: #BDBDBD;
  --WDS-content-disabled-RGB: 189, 189, 189;
  --WDS-content-external-link: #1B8755;
  --WDS-content-external-link-RGB: 27, 135, 85;
  --WDS-content-inverse: #FFFFFF;
  --WDS-content-inverse-RGB: 255, 255, 255;
  --WDS-content-on-accent: #FFFFFF;
  --WDS-content-on-accent-RGB: 255, 255, 255;
  --WDS-content-read: #007BFC;
  --WDS-content-read-RGB: 0, 123, 252;
  --WDS-lines-divider: rgba(0, 0, 0, .1);
  --WDS-lines-divider-RGB: 0, 0, 0;
  --WDS-lines-outline-deemphasized: rgba(0, 0, 0, .2);
  --WDS-lines-outline-deemphasized-RGB: 0, 0, 0;
  --WDS-lines-outline-default: #959393;
  --WDS-lines-outline-default-RGB: 149, 147, 147;
  --WDS-persistent-activity-indicator: #25D366;
  --WDS-persistent-activity-indicator-RGB: 37, 211, 102;
  --WDS-persistent-always-black: #0A0A0A;
  --WDS-persistent-always-black-RGB: 10, 10, 10;
  --WDS-persistent-always-branded: #1DAA61;
  --WDS-persistent-always-branded-RGB: 29, 170, 97;
  --WDS-persistent-always-white: #FFFFFF;
  --WDS-persistent-always-white-RGB: 255, 255, 255;
  --WDS-persistent-verified: #0085F4;
  --WDS-persistent-verified-RGB: 0, 133, 244;
  --WDS-secondary-negative: #EA0038;
  --WDS-secondary-negative-RGB: 234, 0, 56;
  --WDS-secondary-negative-deemphasized: #FDE8EB;
  --WDS-secondary-negative-deemphasized-RGB: 253, 232, 235;
  --WDS-secondary-negative-emphasized: #B80531;
  --WDS-secondary-negative-emphasized-RGB: 184, 5, 49;
  --WDS-secondary-positive: #1DAA61;
  --WDS-secondary-positive-RGB: 29, 170, 97;
  --WDS-secondary-positive-deemphasized: #E7FCE3;
  --WDS-secondary-positive-deemphasized-RGB: 231, 252, 227;
  --WDS-secondary-warning: #FFB938;
  --WDS-secondary-warning-RGB: 255, 185, 56;
  --WDS-secondary-warning-deemphasized: #FFF7E5;
  --WDS-secondary-warning-deemphasized-RGB: 255, 247, 229;
  --WDS-surface-default: #FFFFFF;
  --WDS-surface-default-RGB: 255, 255, 255;
  --WDS-surface-elevated-default: #FFFFFF;
  --WDS-surface-elevated-default-RGB: 255, 255, 255;
  --WDS-surface-elevated-emphasized: #F7F5F3;
  --WDS-surface-elevated-emphasized-RGB: 247, 245, 243;
  --WDS-surface-emphasized: #F7F5F3;
  --WDS-surface-emphasized-RGB: 247, 245, 243;
  --WDS-surface-highlight: rgba(194, 189, 184, .15);
  --WDS-surface-highlight-RGB: 194, 189, 184;
  --WDS-surface-inverse: #242626;
  --WDS-surface-inverse-RGB: 36, 38, 38;
  --WDS-surface-pressed: rgba(0, 0, 0, .2);
  --WDS-surface-pressed-RGB: 0, 0, 0;
  --WDS-systems-attachment-drawing: #FF553B;
  --WDS-systems-bubble-content-business: rgba(0, 0, 0, .6);
  --WDS-systems-bubble-content-business-RGB: 0, 0, 0;
  --WDS-systems-bubble-content-deemphasized: rgba(0, 0, 0, .6);
  --WDS-systems-bubble-content-deemphasized-RGB: 0, 0, 0;
  --WDS-systems-bubble-content-e2e: rgba(0, 0, 0, .6);
  --WDS-systems-bubble-content-e2e-RGB: 0, 0, 0;
  --WDS-systems-bubble-surface-business: #D5FDED;
  --WDS-systems-bubble-surface-business-RGB: 213, 253, 237;
  --WDS-systems-bubble-surface-e2e: #FFF0D4;
  --WDS-systems-bubble-surface-e2e-RGB: 255, 240, 212;
  --WDS-systems-bubble-surface-incoming: #FFFFFF;
  --WDS-systems-bubble-surface-incoming-RGB: 255, 255, 255;
  --WDS-systems-bubble-surface-outgoing: #D9FDD3;
  --WDS-systems-bubble-surface-outgoing-RGB: 217, 253, 211;
  --WDS-systems-bubble-surface-overlay: rgba(194, 189, 184, .15);
  --WDS-systems-bubble-surface-overlay-RGB: 194, 189, 184;
  --WDS-systems-bubble-surface-system: rgba(255, 255, 255, .9);
  --WDS-systems-bubble-surface-system-RGB: 255, 255, 255;
  --WDS-systems-chat-background-wallpaper: #F5F1EB;
  --WDS-systems-chat-background-wallpaper-RGB: 245, 241, 235;
  --WDS-systems-chat-foreground-wallpaper: #EAE0D3;
  --WDS-systems-chat-foreground-wallpaper-RGB: 234, 224, 211;
  --WDS-systems-chat-surface-composer: #FFFFFF;
  --WDS-systems-chat-surface-composer-RGB: 255, 255, 255;
  --WDS-systems-chat-surface-tray: #F7F5F3;
  --WDS-systems-chat-surface-tray-RGB: 247, 245, 243;
  --WDS-systems-status-seen: #C2BDB8;
  --WDS-systems-status-seen-RGB: 194, 189, 184;
}

/* ===== DARK ===== */
:root[data-theme="dark"],
:root[data-theme="dark"] {
  --WDS-accent: #21C063;
  --WDS-accent-RGB: 33, 192, 99;
  --WDS-accent-deemphasized: #103529;
  --WDS-accent-deemphasized-RGB: 16, 53, 41;
  --WDS-accent-emphasized: #D9FDD3;
  --WDS-accent-emphasized-RGB: 217, 253, 211;
  --WDS-background-dimmer: rgb(0, 0, 0, .32);
  --WDS-background-dimmer-RGB: #21C063;
  --WDS-background-elevated-wash-inset: #1D1F1F;
  --WDS-background-elevated-wash-inset-RGB: 29, 31, 31;
  --WDS-background-elevated-wash-plain: #1D1F1F;
  --WDS-background-elevated-wash-plain-RGB: 29, 31, 31;
  --WDS-background-wash-inset: #161717;
  --WDS-background-wash-inset-RGB: 22, 23, 23;
  --WDS-background-wash-plain: #161717;
  --WDS-background-wash-plain-RGB: 22, 23, 23;
  --WDS-components-active-list-row: rgba(255, 255, 255, .1);
  --WDS-components-active-list-row-RGB: 255, 255, 255;
  --WDS-components-filter-surface-selected: #103529;
  --WDS-components-filter-surface-selected-RGB: 16, 53, 41;
  --WDS-components-outline-profile-photo: rgba(255, 255, 255, .1);
  --WDS-components-outline-profile-photo-RGB: 255, 255, 255;
  --WDS-components-platform-gesture-bar: rgba(255, 255, 255, .6);
  --WDS-components-platform-gesture-bar-RGB: 255, 255, 255;
  --WDS-components-platform-status-bar: #FFFFFF;
  --WDS-components-platform-status-bar-RGB: 255, 255, 255;
  --WDS-components-profile-photo-content-brown: #DBA685;
  --WDS-components-profile-photo-content-brown-RGB: 219, 166, 133;
  --WDS-components-profile-photo-content-cobalt: #53A6FD;
  --WDS-components-profile-photo-content-cobalt-RGB: 83, 166, 253;
  --WDS-components-profile-photo-content-gray: #BDBDBD;
  --WDS-components-profile-photo-content-gray-RGB: 189, 189, 189;
  --WDS-components-profile-photo-content-green: #25D366;
  --WDS-components-profile-photo-content-green-RGB: 37, 211, 102;
  --WDS-components-profile-photo-content-orange: #FC9775;
  --WDS-components-profile-photo-content-orange-RGB: 252, 151, 117;
  --WDS-components-profile-photo-content-pink: #FF72A1;
  --WDS-components-profile-photo-content-pink-RGB: 255, 114, 161;
  --WDS-components-profile-photo-content-purple: #A791FF;
  --WDS-components-profile-photo-content-purple-RGB: 167, 145, 255;
  --WDS-components-profile-photo-content-red: #FB5061;
  --WDS-components-profile-photo-content-red-RGB: 251, 80, 97;
  --WDS-components-profile-photo-content-sky-blue: #53BDEB;
  --WDS-components-profile-photo-content-sky-blue-RGB: 83, 189, 235;
  --WDS-components-profile-photo-content-teal: #42C7B8;
  --WDS-components-profile-photo-content-teal-RGB: 66, 199, 184;
  --WDS-components-profile-photo-content-yellow: #FFD279;
  --WDS-components-profile-photo-content-yellow-RGB: 255, 210, 121;
  --WDS-components-profile-photo-status-ring-close-friends: #C15ADD;
  --WDS-components-profile-photo-surface-brown: #35271E;
  --WDS-components-profile-photo-surface-brown-RGB: 53, 39, 30;
  --WDS-components-profile-photo-surface-cobalt: #092642;
  --WDS-components-profile-photo-surface-cobalt-RGB: 9, 38, 66;
  --WDS-components-profile-photo-surface-gray: #242626;
  --WDS-components-profile-photo-surface-gray-RGB: 36, 38, 38;
  --WDS-components-profile-photo-surface-green: #103529;
  --WDS-components-profile-photo-surface-green-RGB: 16, 53, 41;
  --WDS-components-profile-photo-surface-orange: #35221E;
  --WDS-components-profile-photo-surface-orange-RGB: 53, 34, 30;
  --WDS-components-profile-photo-surface-pink: #36192A;
  --WDS-components-profile-photo-surface-pink-RGB: 54, 25, 42;
  --WDS-components-profile-photo-surface-purple: #242447;
  --WDS-components-profile-photo-surface-purple-RGB: 36, 36, 71;
  --WDS-components-profile-photo-surface-red: #321622;
  --WDS-components-profile-photo-surface-red-RGB: 50, 22, 34;
  --WDS-components-profile-photo-surface-sky-blue: #092C3D;
  --WDS-components-profile-photo-surface-sky-blue-RGB: 9, 44, 61;
  --WDS-components-profile-photo-surface-teal: #092D2F;
  --WDS-components-profile-photo-surface-teal-RGB: 9, 45, 47;
  --WDS-components-profile-photo-surface-yellow: #362C1F;
  --WDS-components-profile-photo-surface-yellow-RGB: 54, 44, 31;
  --WDS-components-surface-nav-bar: #1D1F1F;
  --WDS-components-surface-nav-bar-RGB: 29, 31, 31;
  --WDS-content-action-default: #FAFAFA;
  --WDS-content-action-default-RGB: 250, 250, 250;
  --WDS-content-action-emphasized: #21C063;
  --WDS-content-action-emphasized-RGB: 33, 192, 99;
  --WDS-content-deemphasized: rgba(255, 255, 255, .6);
  --WDS-content-deemphasized-RGB: 255, 255, 255;
  --WDS-content-default: #FAFAFA;
  --WDS-content-default-RGB: 250, 250, 250;
  --WDS-content-disabled: #424445;
  --WDS-content-disabled-RGB: 66, 68, 69;
  --WDS-content-external-link: #21C063;
  --WDS-content-external-link-RGB: 33, 192, 99;
  --WDS-content-inverse: #0A0A0A;
  --WDS-content-inverse-RGB: 10, 10, 10;
  --WDS-content-on-accent: #0A0A0A;
  --WDS-content-on-accent-RGB: 10, 10, 10;
  --WDS-content-read: #53BDEB;
  --WDS-content-read-RGB: 83, 189, 235;
  --WDS-lines-divider: rgba(255, 255, 255, .1);
  --WDS-lines-divider-RGB: 255, 255, 255;
  --WDS-lines-outline-deemphasized: rgba(255, 255, 255, .1);
  --WDS-lines-outline-deemphasized-RGB: 255, 255, 255;
  --WDS-lines-outline-default: #757778;
  --WDS-lines-outline-default-RGB: 117, 119, 120;
  --WDS-persistent-activity-indicator: #25D366;
  --WDS-persistent-activity-indicator-RGB: 37, 211, 102;
  --WDS-persistent-always-black: #0A0A0A;
  --WDS-persistent-always-black-RGB: 10, 10, 10;
  --WDS-persistent-always-branded: #21C063;
  --WDS-persistent-always-branded-RGB: 33, 192, 99;
  --WDS-persistent-always-white: #FFFFFF;
  --WDS-persistent-always-white-RGB: 255, 255, 255;
  --WDS-persistent-verified: #0085F4;
  --WDS-persistent-verified-RGB: 0, 133, 244;
  --WDS-secondary-negative: #FB5061;
  --WDS-secondary-negative-RGB: 251, 80, 97;
  --WDS-secondary-negative-deemphasized: #321622;
  --WDS-secondary-negative-deemphasized-RGB: 50, 22, 34;
  --WDS-secondary-negative-emphasized: #FA99A4;
  --WDS-secondary-negative-emphasized-RGB: 250, 153, 164;
  --WDS-secondary-positive: #71EB85;
  --WDS-secondary-positive-RGB: 113, 235, 133;
  --WDS-secondary-positive-deemphasized: #103529;
  --WDS-secondary-positive-deemphasized-RGB: 16, 53, 41;
  --WDS-secondary-warning: #FFD279;
  --WDS-secondary-warning-RGB: 255, 210, 121;
  --WDS-secondary-warning-deemphasized: #362C1F;
  --WDS-secondary-warning-deemphasized-RGB: 54, 44, 31;
  --WDS-surface-default: #161717;
  --WDS-surface-default-RGB: 22, 23, 23;
  --WDS-surface-elevated-default: #1D1F1F;
  --WDS-surface-elevated-default-RGB: 29, 31, 31;
  --WDS-surface-elevated-emphasized: #242626;
  --WDS-surface-elevated-emphasized-RGB: 36, 38, 38;
  --WDS-surface-emphasized: #1D1F1F;
  --WDS-surface-emphasized-RGB: 29, 31, 31;
  --WDS-surface-highlight: rgba(255, 255, 255, .1);
  --WDS-surface-highlight-RGB: 255, 255, 255;
  --WDS-surface-inverse: #EEEEEE;
  --WDS-surface-inverse-RGB: 238, 238, 238;
  --WDS-surface-pressed: rgba(255, 255, 255, .2);
  --WDS-surface-pressed-RGB: 255, 255, 255;
  --WDS-systems-attachment-drawing: #FF553B;
  --WDS-systems-bubble-content-business: #06CF9C;
  --WDS-systems-bubble-content-business-RGB: 6, 207, 156;
  --WDS-systems-bubble-content-deemphasized: rgba(255, 255, 255, .6);
  --WDS-systems-bubble-content-deemphasized-RGB: 255, 255, 255;
  --WDS-systems-bubble-content-e2e: #FFD279;
  --WDS-systems-bubble-content-e2e-RGB: 255, 210, 121;
  --WDS-systems-bubble-surface-business: #1D1F1F;
  --WDS-systems-bubble-surface-business-RGB: 29, 31, 31;
  --WDS-systems-bubble-surface-e2e: #1D1F1F;
  --WDS-systems-bubble-surface-e2e-RGB: 29, 31, 31;
  --WDS-systems-bubble-surface-incoming: #242626;
  --WDS-systems-bubble-surface-incoming-RGB: 36, 38, 38;
  --WDS-systems-bubble-surface-outgoing: #144D37;
  --WDS-systems-bubble-surface-outgoing-RGB: 20, 77, 55;
  --WDS-systems-bubble-surface-overlay: rgba(0, 0, 0, .2);
  --WDS-systems-bubble-surface-overlay-RGB: 0, 0, 0;
  --WDS-systems-bubble-surface-system: #1D1F1F;
  --WDS-systems-bubble-surface-system-RGB: 29, 31, 31;
  --WDS-systems-chat-background-wallpaper: #161717;
  --WDS-systems-chat-background-wallpaper-RGB: 22, 23, 23;
  --WDS-systems-chat-foreground-wallpaper: rgba(255, 255, 255, .1);
  --WDS-systems-chat-foreground-wallpaper-RGB: 255, 255, 255;
  --WDS-systems-chat-surface-composer: #242626;
  --WDS-systems-chat-surface-composer-RGB: 36, 38, 38;
  --WDS-systems-chat-surface-tray: #161717;
  --WDS-systems-chat-surface-tray-RGB: 22, 23, 23;
  --WDS-systems-status-seen: #757778;
  --WDS-systems-status-seen-RGB: 117, 119, 120;
}
@media (prefers-color-scheme: dark) {
  :root[data-theme="system"] {
    --WDS-accent: #21C063;
    --WDS-accent-RGB: 33, 192, 99;
    --WDS-accent-deemphasized: #103529;
    --WDS-accent-deemphasized-RGB: 16, 53, 41;
    --WDS-accent-emphasized: #D9FDD3;
    --WDS-accent-emphasized-RGB: 217, 253, 211;
    --WDS-background-dimmer: rgb(0, 0, 0, .32);
    --WDS-background-dimmer-RGB: #21C063;
    --WDS-background-elevated-wash-inset: #1D1F1F;
    --WDS-background-elevated-wash-inset-RGB: 29, 31, 31;
    --WDS-background-elevated-wash-plain: #1D1F1F;
    --WDS-background-elevated-wash-plain-RGB: 29, 31, 31;
    --WDS-background-wash-inset: #161717;
    --WDS-background-wash-inset-RGB: 22, 23, 23;
    --WDS-background-wash-plain: #161717;
    --WDS-background-wash-plain-RGB: 22, 23, 23;
    --WDS-components-active-list-row: rgba(255, 255, 255, .1);
    --WDS-components-active-list-row-RGB: 255, 255, 255;
    --WDS-components-filter-surface-selected: #103529;
    --WDS-components-filter-surface-selected-RGB: 16, 53, 41;
    --WDS-components-outline-profile-photo: rgba(255, 255, 255, .1);
    --WDS-components-outline-profile-photo-RGB: 255, 255, 255;
    --WDS-components-platform-gesture-bar: rgba(255, 255, 255, .6);
    --WDS-components-platform-gesture-bar-RGB: 255, 255, 255;
    --WDS-components-platform-status-bar: #FFFFFF;
    --WDS-components-platform-status-bar-RGB: 255, 255, 255;
    --WDS-components-profile-photo-content-brown: #DBA685;
    --WDS-components-profile-photo-content-brown-RGB: 219, 166, 133;
    --WDS-components-profile-photo-content-cobalt: #53A6FD;
    --WDS-components-profile-photo-content-cobalt-RGB: 83, 166, 253;
    --WDS-components-profile-photo-content-gray: #BDBDBD;
    --WDS-components-profile-photo-content-gray-RGB: 189, 189, 189;
    --WDS-components-profile-photo-content-green: #25D366;
    --WDS-components-profile-photo-content-green-RGB: 37, 211, 102;
    --WDS-components-profile-photo-content-orange: #FC9775;
    --WDS-components-profile-photo-content-orange-RGB: 252, 151, 117;
    --WDS-components-profile-photo-content-pink: #FF72A1;
    --WDS-components-profile-photo-content-pink-RGB: 255, 114, 161;
    --WDS-components-profile-photo-content-purple: #A791FF;
    --WDS-components-profile-photo-content-purple-RGB: 167, 145, 255;
    --WDS-components-profile-photo-content-red: #FB5061;
    --WDS-components-profile-photo-content-red-RGB: 251, 80, 97;
    --WDS-components-profile-photo-content-sky-blue: #53BDEB;
    --WDS-components-profile-photo-content-sky-blue-RGB: 83, 189, 235;
    --WDS-components-profile-photo-content-teal: #42C7B8;
    --WDS-components-profile-photo-content-teal-RGB: 66, 199, 184;
    --WDS-components-profile-photo-content-yellow: #FFD279;
    --WDS-components-profile-photo-content-yellow-RGB: 255, 210, 121;
    --WDS-components-profile-photo-status-ring-close-friends: #C15ADD;
    --WDS-components-profile-photo-surface-brown: #35271E;
    --WDS-components-profile-photo-surface-brown-RGB: 53, 39, 30;
    --WDS-components-profile-photo-surface-cobalt: #092642;
    --WDS-components-profile-photo-surface-cobalt-RGB: 9, 38, 66;
    --WDS-components-profile-photo-surface-gray: #242626;
    --WDS-components-profile-photo-surface-gray-RGB: 36, 38, 38;
    --WDS-components-profile-photo-surface-green: #103529;
    --WDS-components-profile-photo-surface-green-RGB: 16, 53, 41;
    --WDS-components-profile-photo-surface-orange: #35221E;
    --WDS-components-profile-photo-surface-orange-RGB: 53, 34, 30;
    --WDS-components-profile-photo-surface-pink: #36192A;
    --WDS-components-profile-photo-surface-pink-RGB: 54, 25, 42;
    --WDS-components-profile-photo-surface-purple: #242447;
    --WDS-components-profile-photo-surface-purple-RGB: 36, 36, 71;
    --WDS-components-profile-photo-surface-red: #321622;
    --WDS-components-profile-photo-surface-red-RGB: 50, 22, 34;
    --WDS-components-profile-photo-surface-sky-blue: #092C3D;
    --WDS-components-profile-photo-surface-sky-blue-RGB: 9, 44, 61;
    --WDS-components-profile-photo-surface-teal: #092D2F;
    --WDS-components-profile-photo-surface-teal-RGB: 9, 45, 47;
    --WDS-components-profile-photo-surface-yellow: #362C1F;
    --WDS-components-profile-photo-surface-yellow-RGB: 54, 44, 31;
    --WDS-components-surface-nav-bar: #1D1F1F;
    --WDS-components-surface-nav-bar-RGB: 29, 31, 31;
    --WDS-content-action-default: #FAFAFA;
    --WDS-content-action-default-RGB: 250, 250, 250;
    --WDS-content-action-emphasized: #21C063;
    --WDS-content-action-emphasized-RGB: 33, 192, 99;
    --WDS-content-deemphasized: rgba(255, 255, 255, .6);
    --WDS-content-deemphasized-RGB: 255, 255, 255;
    --WDS-content-default: #FAFAFA;
    --WDS-content-default-RGB: 250, 250, 250;
    --WDS-content-disabled: #424445;
    --WDS-content-disabled-RGB: 66, 68, 69;
    --WDS-content-external-link: #21C063;
    --WDS-content-external-link-RGB: 33, 192, 99;
    --WDS-content-inverse: #0A0A0A;
    --WDS-content-inverse-RGB: 10, 10, 10;
    --WDS-content-on-accent: #0A0A0A;
    --WDS-content-on-accent-RGB: 10, 10, 10;
    --WDS-content-read: #53BDEB;
    --WDS-content-read-RGB: 83, 189, 235;
    --WDS-lines-divider: rgba(255, 255, 255, .1);
    --WDS-lines-divider-RGB: 255, 255, 255;
    --WDS-lines-outline-deemphasized: rgba(255, 255, 255, .1);
    --WDS-lines-outline-deemphasized-RGB: 255, 255, 255;
    --WDS-lines-outline-default: #757778;
    --WDS-lines-outline-default-RGB: 117, 119, 120;
    --WDS-persistent-activity-indicator: #25D366;
    --WDS-persistent-activity-indicator-RGB: 37, 211, 102;
    --WDS-persistent-always-black: #0A0A0A;
    --WDS-persistent-always-black-RGB: 10, 10, 10;
    --WDS-persistent-always-branded: #21C063;
    --WDS-persistent-always-branded-RGB: 33, 192, 99;
    --WDS-persistent-always-white: #FFFFFF;
    --WDS-persistent-always-white-RGB: 255, 255, 255;
    --WDS-persistent-verified: #0085F4;
    --WDS-persistent-verified-RGB: 0, 133, 244;
    --WDS-secondary-negative: #FB5061;
    --WDS-secondary-negative-RGB: 251, 80, 97;
    --WDS-secondary-negative-deemphasized: #321622;
    --WDS-secondary-negative-deemphasized-RGB: 50, 22, 34;
    --WDS-secondary-negative-emphasized: #FA99A4;
    --WDS-secondary-negative-emphasized-RGB: 250, 153, 164;
    --WDS-secondary-positive: #71EB85;
    --WDS-secondary-positive-RGB: 113, 235, 133;
    --WDS-secondary-positive-deemphasized: #103529;
    --WDS-secondary-positive-deemphasized-RGB: 16, 53, 41;
    --WDS-secondary-warning: #FFD279;
    --WDS-secondary-warning-RGB: 255, 210, 121;
    --WDS-secondary-warning-deemphasized: #362C1F;
    --WDS-secondary-warning-deemphasized-RGB: 54, 44, 31;
    --WDS-surface-default: #161717;
    --WDS-surface-default-RGB: 22, 23, 23;
    --WDS-surface-elevated-default: #1D1F1F;
    --WDS-surface-elevated-default-RGB: 29, 31, 31;
    --WDS-surface-elevated-emphasized: #242626;
    --WDS-surface-elevated-emphasized-RGB: 36, 38, 38;
    --WDS-surface-emphasized: #1D1F1F;
    --WDS-surface-emphasized-RGB: 29, 31, 31;
    --WDS-surface-highlight: rgba(255, 255, 255, .1);
    --WDS-surface-highlight-RGB: 255, 255, 255;
    --WDS-surface-inverse: #EEEEEE;
    --WDS-surface-inverse-RGB: 238, 238, 238;
    --WDS-surface-pressed: rgba(255, 255, 255, .2);
    --WDS-surface-pressed-RGB: 255, 255, 255;
    --WDS-systems-attachment-drawing: #FF553B;
    --WDS-systems-bubble-content-business: #06CF9C;
    --WDS-systems-bubble-content-business-RGB: 6, 207, 156;
    --WDS-systems-bubble-content-deemphasized: rgba(255, 255, 255, .6);
    --WDS-systems-bubble-content-deemphasized-RGB: 255, 255, 255;
    --WDS-systems-bubble-content-e2e: #FFD279;
    --WDS-systems-bubble-content-e2e-RGB: 255, 210, 121;
    --WDS-systems-bubble-surface-business: #1D1F1F;
    --WDS-systems-bubble-surface-business-RGB: 29, 31, 31;
    --WDS-systems-bubble-surface-e2e: #1D1F1F;
    --WDS-systems-bubble-surface-e2e-RGB: 29, 31, 31;
    --WDS-systems-bubble-surface-incoming: #242626;
    --WDS-systems-bubble-surface-incoming-RGB: 36, 38, 38;
    --WDS-systems-bubble-surface-outgoing: #144D37;
    --WDS-systems-bubble-surface-outgoing-RGB: 20, 77, 55;
    --WDS-systems-bubble-surface-overlay: rgba(0, 0, 0, .2);
    --WDS-systems-bubble-surface-overlay-RGB: 0, 0, 0;
    --WDS-systems-bubble-surface-system: #1D1F1F;
    --WDS-systems-bubble-surface-system-RGB: 29, 31, 31;
    --WDS-systems-chat-background-wallpaper: #161717;
    --WDS-systems-chat-background-wallpaper-RGB: 22, 23, 23;
    --WDS-systems-chat-foreground-wallpaper: rgba(255, 255, 255, .1);
    --WDS-systems-chat-foreground-wallpaper-RGB: 255, 255, 255;
    --WDS-systems-chat-surface-composer: #242626;
    --WDS-systems-chat-surface-composer-RGB: 36, 38, 38;
    --WDS-systems-chat-surface-tray: #161717;
    --WDS-systems-chat-surface-tray-RGB: 22, 23, 23;
    --WDS-systems-status-seen: #757778;
    --WDS-systems-status-seen-RGB: 117, 119, 120;
  }
}
```

# Appendix B — Additional component tokens (selected, verified)

These were extracted from the production bundle and complement §1.7. Grouped by component. (Theme-dependent values shown as light/dark.)

**Search / input**

| Token | Value |
|---|---|
| `--search-input-background` | `rgb(241,238,235)` / `rgba(36,38,38,1)` |
| `--input-background` | `#FFFFFF` / `rgba(10,16,20,1)` |
| `--input-border-color` | `#CED0D4` / `#3E4042`; hover `#65676B` / `#8A8D91` |
| `--input-corner-radius` | `8px` |
| `--input-border-width` / `--text-input-focus-border-width` | `1px` / `1px` |
| `--text-input-min-height` / dense | `60px` / `44px` |
| `--text-input-horizontal-padding` | `16px` |
| `--text-input-label-top` | `18px` |
| `--text-input-multiline-top-padding` | `20px` |
| `--text-input-end-icon-size` / padding | `24px` / `24px` |

**Banner / badge**

| Token | Value |
|---|---|
| `--banner-background-color` | `rgb(241,238,235)` / `rgba(36,38,38,1)` |
| `--banner-icon-width` / padding | `28px` / `12px` |
| `--badge-inner-padding-horizontal` / vertical | `8px` / `6px` |
| `--badge-success-background-color` | `rgb(196,248,185)` / `rgb(9,68,31)` |
| `--badge-attention-background-color` | `rgb(252,236,133)` / `rgb(93,46,4)` |
| `--badge-critical-background-color` | `rgb(254,228,230)` / `rgb(123,2,16)` |
| `--badge-informational-background-color` | `rgb(219,236,255)` / `rgb(4,47,151)` |
| `--badge-neutral-background-color` | `rgb(230,235,239)` / `rgb(43,61,70)` |
| `--badge-pending-background-color` | `rgb(179,176,254)` / `rgb(69,7,169)` |

**Switch / radio (settings)**

| Token | Value |
|---|---|
| `--switch-active` | `rgba(250,250,250,1)` / `rgb(23,22,22)` |
| `--switch-inactive` | `rgb(247,245,243)` / `rgba(18,24,28,1)` |
| `--switch-active-track-android` | `rgb(0,100,224)` / `rgb(0,130,251)` |
| `--radio-size-medium/large` | `24px` |
| `--radio-border-width` | `2px` |
| `--radio-checked-icon-size-medium/large` | `10px` |
| `--radio-border-color` | `rgba(141,149,153,1)` / `rgba(108,117,122,1)` |

**Lists / nav lists**

| Token | Value |
|---|---|
| `--list-cell-min-height` | `52px` |
| `--list-cell-padding-vertical-with-addon` / no-addon | `12px` / `16px` |
| `--list-cell-bottom-addon-padding` | `16px` |
| `--list-cell-nested-padding-start` | `36px` |
| `--list-cell-corner-radius` / `--nav-list-cell-corner-radius` | `10px` |
| `--nav-list-cell-min-height` / dense | `52px` / `44px` |
| `--nav-list-cell-margin-horizontal` | `16px` |
| `--nav-list-selected` | `rgb(221,226,232)` / `rgb(53,72,85)` |
| `--nav-list-selected-accent-text` | `rgb(10,19,23)` / `rgb(0,100,224)` |
| `--list-cell-chevron` | `rgb(93,108,123)` / `rgb(164,176,188)` |

**Overlays**

| Token | Value |
|---|---|
| `--popover-background` | `#FFFFFF` / `rgb(21,33,39)` |
| `--popover-padding` | `16px` |
| `--tooltip-background` | `rgba(32,39,43,1)` / `#FFFFFF` (inverted) |
| `--tooltip-padding-horizontal/vertical` | `16px` / `16px` |
| `--tooltip-corner-radius` | `12px` |
| `--toast-container-min-width` / max | `288px` / `100%` |
| `--toast-container-padding-horizontal/vertical` | `14px` / `16px` |
| `--toast-background` | `rgba(32,39,43,1)`; web `#283943` |
| `--toast-text-web` | `#FFFFFF` |
| `--toast-corner-radius` | `4px` |
| `--card-padding-horizontal/vertical` | `16px` / `16px` |
| `--card-border-color` | `rgba(17,27,33,.2)` / `rgba(255,255,255,.1)` |
| `--dialog-anchor-vertical-padding` | `32px` |

**Media / profile**

| Token | Value |
|---|---|
| `--media-corner-radius` / large / small | `16px` / `24px` / `12px` |
| `--media-inner-border` | `rgba(10,19,23,.1)` / `rgba(241,244,247,.1)` |
| `--media-pressed` | `rgba(10,19,23,.3)` / `rgba(255,255,255,.3)` |
| `--profile-photo-entity-small/medium/large/xlarge-corner-radius` | `8px` / `12px` / `16px` / `24px` |
| `--icon-container-size` | `48px` |
| `--squircle-polygon` | 128-point polygon (entity avatars); approximate with `border-radius` unless you need the exact squircle |
| `--screen-side-panel-width-small/medium` | `280px` / `360px` |
| `--width-announcement-bubble` | `480px` (responsive variants as in §1.7) |
| `--height-pane-footer` / `--navbar-width` / `--screen-main-header-height` | `62px` / `64px` / `77px` |
| `--animated-emoji-zindex-conversation` | `300` |
| `--font-family-apple` | `system-ui, -apple-system, BlinkMacSystemFont, ".SFNSText-Regular", sans-serif` |
| `--font-family-code` | `ui-monospace, Menlo, Consolas, Monaco, monospace` |

# Appendix C — UI string inventory (verified, en-US build)

Use these exact strings for the rebuild (they came from the production string table). Grouped by area.

**Chat list / navigation:** Chats · Calls · Status · Channels · Communities · Settings · Search · Archived · Favourites · Unread · Groups · No chats · No results found · Archived chats · Select chats · Mark all as read · New chat · New group · New community · Starred messages · Linked devices · Log out · Keyboard shortcuts · Profile · Privacy · Notifications · Storage and data · Help · Terms & Privacy Policy

**Conversation:** Type a message · Search chat · Extended search · Message info · Contact info · Group info · Exit group · Leave group · Mute · Unmute · Archive chat · Unarchive · Pin chat · Unpin chat · Mark as unread · Mark as read · Clear chat · Delete chat · Block · Block chat · Report · Star message · Reply · Reply privately · Forward · Copy · Edit last message · Select messages · Close chat · Disappearing messages · Chat lock · Lock app · Live location · Media files · Media no longer available · Message type not supported · End-to-end encrypted · Messages and calls are end-to-end encrypted. Only people in this chat can read, listen to, or share them. · Your personal messages are end-to-end encrypted · Sync is paused / Syncing older messages (progress variants) · New message

**Calls:** Calls · Start a call · End call · Calling… · Call ended · Call failed · Missed voice call · Missed video call · Incoming voice call from {name} · Incoming video call from {name} · Incoming group voice call from {name} · Incoming group video call from {name} · Join video call on this device · Join voice call on this device · Calling is available on the Mac app · Open WhatsApp Web to answer this call · Group call · Screen share

**Status:** My status · Recent updates · Viewed updates · Muted updates · Status update not found · Liked your status · Mentioned you privately in a status · Select to reload QR code · Status: Delivered (and the other status enums)

**Auth/landing:** Scan to log in · Scan the QR code with your phone's camera · Tap the link to open WhatsApp · Scan the QR code again to link to your account · Stay logged in on this browser · Log in with phone number · Log in with QR code · Link with QR code · Link with phone number instead · Select a country and enter your phone number. · Don't close this window. Your messages are downloading. · Downloading messages: {progress}% · Loading your chats · Syncing messages in the background · Syncing messages complete · Get started · Need help? · Download WhatsApp · WhatsApp works in Google Chrome 100 or higher…

**Toast/menu confirmations:** Chat muted · Chat unmuted · Group muted · Group unmuted · Channel muted · Channel unmuted · Muted until today at {time} · Muted until tomorrow at {time} · Muted always · Marked as unread · Archive · Archive instead · Added to Favourites · Removed from Favourites · Couldn't mute chat. · Couldn't mark chat as read. · Couldn't mark chat as unread.

# Appendix D — Implementation notes for React + Tauri

**Suggested structure**

```
src/
  styles/tokens.css            # Appendix A (WDS tokens) + component tokens
  styles/typography.css        # §2 scale utilities
  theme/ThemeProvider.tsx      # light/dark/system → data-theme attr on <html>
  shell/AppShell.tsx           # 3-column layout + responsive breakpoints
  shell/NavRail.tsx
  chatlist/{ChatList,ChatRow,SearchField,FilterTabs,ArchiveRow}.tsx
  conversation/{Header,MessageList,MessageRow,Bubble,BubbleTail,Meta,Reactions}.tsx
  conversation/composer/{Composer,AttachMenu,EmojiPicker,StickerPanel}.tsx
  overlays/{Modal,Drawer,ContextMenu,Toast,CommandPalette}.tsx
  screens/{Settings,Status,Calls,Channels,Communities,QRPairing,Lock}.tsx
  lib/{virtualizer,scrollAnchor,keyboard,shortcuts,formatTime}.ts
  i18n/strings.en.ts           # Appendix C
```

**State:** separate stores per concern (chats list, active chat messages, composer, UI overlays) to avoid whole-tree re-renders. Use an event bus or Zustand/Jotai; keep message rows memoized (React.memo + stable keys) and avoid context for the message list.

**Virtualization:** `@tanstack/react-virtual` (variable sizes, `measureElement`) or `react-window` for the chat list. For messages, precompute estimated heights and let `measureElement` refine.

**Theming:** put Appendix A in `:root` / `[data-theme="dark"]`; component tokens in a second block; expose `data-theme="light|dark|system"` and listen to `matchMedia('(prefers-color-scheme: dark)')` for system mode. Also apply `window.setTheme` in Tauri when dark/light changes.

**Keyboard:** implement a small shortcut engine mirroring §6.4 with context registration (global vs chat-list vs composer vs call), which resolves the noted collisions naturally. Do not rely on the menu for shortcuts that the WebView handles.

**Testing:** Storybook stories per state (bubble variants, row states, themes); Playwright tests for the golden paths (send/receive mock, search, theme toggle, resize, notifications mocked); visual regression at 3 widths × 2 themes.

**Do not ship:** WhatsApp name/logo/wordmark assets, icon SVGs, doodle wallpaper images, notification sounds, or exact in-app copy where it is a trademark. This document is for a personal/technical rebuild; verify legal constraints before distribution.

---

*End of specification.*

