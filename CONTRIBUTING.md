# Contributing

Thanks for your interest in RustWA! A few ground rules:

1. **No proprietary assets.** Never commit WhatsApp-owned code, icons, fonts,
   sounds, or artwork. Icons come from Lucide or are original. If you need a
   glyph, draw it.
2. **The core stays UI-free.** `crates/whatsapp-core` must not depend on Tauri
   or anything webview-related.
3. **Small PRs.** One milestone slice per PR, with tests where feasible.
4. **Run the checks locally** before pushing:

   ```bash
   cargo fmt --all
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   cd apps/desktop && npm run build
   ```

5. **Be honest in docs.** When something is partial or untested, say so.

## Commit style

Conventional-commit-ish prefixes: `feat:`, `fix:`, `docs:`, `chore:`, `ci:`,
`refactor:`, `perf:`, `test:`.

## Reporting bugs

Include the app version, macOS version, and reproduction steps. Never paste
session credentials, QR payloads, or private message content into issues.
