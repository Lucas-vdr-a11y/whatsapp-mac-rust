//! Per-chat windows ("Open in new window").
//!
//! # Labels
//!
//! Tauri window labels may only contain `[a-zA-Z0-9-/:_]`, while WhatsApp JIDs
//! contain `@` (and often `.`), so the JID can not be used as a label
//! directly. Each chat window is labelled `chat-<fnv1a64 hex>` of the JID
//! ([`chat_label`]): deterministic (so a second `open_chat_window` for the
//! same chat resolves to the same label), collision-resistant enough for a
//! handful of windows, and free of PII in the macOS Window menu.
//!
//! # URL
//!
//! The webview loads `index.html?window=chat&chatId=<percent-encoded jid>`;
//! `main.tsx` routes that to `ChatWindow.tsx`, which renders a single
//! conversation without the rail/chat list.
//!
//! # Lifecycle
//!
//! [`open_chat_window`] focuses an existing window instead of creating a
//! duplicate. [`close_chat_window`] closes it again and is idempotent, so the
//! UI may call it as a "back to main window" action. The [`ChatWindows`]
//! registry keeps the JID → label association for the host; Tauri's own
//! window map stays the authority on whether a window is still alive.
//!
//! # Scope
//!
//! Chat windows are deliberately thin: they do not run the main window's
//! core bridge (notifications, deep links, call overlay), so opening one can
//! not duplicate desktop notifications or consume the cold-start deep-link
//! buffer. The frontend subscribes to `core://event` itself and applies only
//! the events for its own chat.

use std::collections::HashMap;
use std::sync::Mutex;

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

/// Prefix of every per-chat window label. `capabilities/chat.json` matches
/// this with the `chat-*` glob.
pub const CHAT_WINDOW_LABEL_PREFIX: &str = "chat-";

/// Default outer size of a chat window, in logical pixels.
const DEFAULT_WIDTH: f64 = 480.0;
const DEFAULT_HEIGHT: f64 = 720.0;

/// Smallest permitted size; the conversation stays usable down to this.
const MIN_WIDTH: f64 = 360.0;
const MIN_HEIGHT: f64 = 480.0;

/// FNV-1a 64-bit constants (public domain). A tiny local hash avoids adding a
/// digest dependency for what is only a label namespace.
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Stable, label-safe identifier for a JID: `chat-<16 hex digits>`.
///
/// Deterministic by design: `open_chat_window` derives the label from the JID
/// on every call, so a second open finds the existing window without any
/// persistent state.
pub fn chat_label(chat_id: &str) -> String {
    let mut digest = FNV_OFFSET_BASIS;
    for byte in chat_id.as_bytes() {
        digest ^= u64::from(*byte);
        digest = digest.wrapping_mul(FNV_PRIME);
    }
    format!("{CHAT_WINDOW_LABEL_PREFIX}{digest:016x}")
}

/// Relative app URL loaded by a chat window.
///
/// `window=chat` selects the compact shell in `main.tsx`; `chatId` is
/// percent-encoded because JIDs contain `@` (and `.`), which would otherwise
/// be ambiguous in a query string.
fn chat_window_url(chat_id: &str) -> String {
    let encoded: String =
        percent_encoding::utf8_percent_encode(chat_id, percent_encoding::NON_ALPHANUMERIC)
            .to_string();
    format!("index.html?window=chat&chatId={encoded}")
}

/// Registry of open per-chat windows, keyed by JID.
///
/// The label is derivable from the JID, so this map is a convenience for the
/// host (diagnostics, a future window cap) rather than a correctness
/// requirement. Existence is always re-checked against Tauri's live window
/// map, because the user can close a window without any command being run;
/// the `Destroyed` hook below keeps the registry tidy in that case too.
#[derive(Default)]
pub struct ChatWindows {
    labels: Mutex<HashMap<String, String>>,
}

impl ChatWindows {
    /// Records `chat_id` as open under `label`.
    fn track(&self, chat_id: &str, label: &str) {
        if let Ok(mut labels) = self.labels.lock() {
            labels.insert(chat_id.to_owned(), label.to_owned());
        }
    }

    /// Drops `chat_id` from the registry (window closed).
    fn forget(&self, chat_id: &str) {
        if let Ok(mut labels) = self.labels.lock() {
            labels.remove(chat_id);
        }
    }

    /// Number of chat windows currently tracked; tests and diagnostics only.
    #[cfg(test)]
    fn tracked_count(&self) -> usize {
        self.labels.lock().map(|labels| labels.len()).unwrap_or(0)
    }
}

/// Opens the chat window for `chat_id`, or focuses it when it is already
/// open.
///
/// The window is sized 480×720 (min 360×480), resizable and titled "RustWA".
/// Errors for an empty JID or when the window can not be created.
#[tauri::command]
pub fn open_chat_window(app: AppHandle, chat_id: String) -> Result<(), String> {
    let chat_id = chat_id.trim();
    if chat_id.is_empty() {
        return Err("chat id must not be empty".to_owned());
    }

    let label = chat_label(chat_id);
    if let Some(window) = app.get_webview_window(&label) {
        // Already open: bring it forward instead of stacking a duplicate.
        let _ = window.show();
        let _ = window.unminimize();
        return window.set_focus().map_err(|error| error.to_string());
    }

    let window = WebviewWindowBuilder::new(
        &app,
        &label,
        WebviewUrl::App(chat_window_url(chat_id).into()),
    )
    .title("RustWA")
    .inner_size(DEFAULT_WIDTH, DEFAULT_HEIGHT)
    .min_inner_size(MIN_WIDTH, MIN_HEIGHT)
    .resizable(true)
    .build()
    .map_err(|error| error.to_string())?;

    // Bring the fresh window to the front; on macOS a new webview does not
    // necessarily take focus while the app is inactive.
    let _ = window.set_focus();

    // Keep the registry in sync when the user closes the window directly
    // (traffic-light button / Cmd+W), not through `close_chat_window`.
    let handle = app.clone();
    let closed_chat_id = chat_id.to_owned();
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) {
            handle.state::<ChatWindows>().forget(&closed_chat_id);
        }
    });

    app.state::<ChatWindows>().track(chat_id, &label);
    tracing::debug!(chat_id, label, "opened chat window");
    Ok(())
}

/// Closes the chat window for `chat_id`, if one is open.
///
/// Idempotent: a missing window is not an error, so the UI can use this as a
/// "back to main window" affordance from any state.
#[tauri::command]
pub fn close_chat_window(app: AppHandle, chat_id: String) -> Result<(), String> {
    let chat_id = chat_id.trim();
    let label = chat_label(chat_id);
    app.state::<ChatWindows>().forget(chat_id);

    if let Some(window) = app.get_webview_window(&label) {
        window.close().map_err(|error| error.to_string())?;
    }

    // The UI uses this as "back to main window": make sure the main window is
    // frontmost instead of leaving the user on the desktop after the closed
    // chat window was the key window.
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.set_focus();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_stable_hashes_without_pii() {
        let jid = "31612345678@s.whatsapp.net";
        let label = chat_label(jid);

        assert!(label.starts_with(CHAT_WINDOW_LABEL_PREFIX));
        assert_eq!(label, chat_label(jid));
        assert_ne!(label, chat_label("31612345678@s.whatsapp.net "));
        assert_eq!(label.len(), CHAT_WINDOW_LABEL_PREFIX.len() + 16);
        // Labels may only use `[a-zA-Z0-9-/:_]`.
        assert!(label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
        // The JID itself must not leak into the window label / menus.
        assert!(!label.contains('@'));
        assert!(!label.contains('.'));
    }

    #[test]
    fn labels_differ_per_jid() {
        assert_ne!(
            chat_label("1234567890-123456@g.us"),
            chat_label("31612345678@s.whatsapp.net"),
        );
    }

    #[test]
    fn url_percent_encodes_the_jid() {
        let jid = "1234567890-123@g.us";
        let url = chat_window_url(jid);

        assert!(url.starts_with("index.html?window=chat&chatId="));
        let encoded = url
            .split("chatId=")
            .nth(1)
            .expect("url carries the chat id");
        assert!(!encoded.contains('@'));
        assert!(!encoded.contains('.'));
        assert_eq!(
            percent_encoding::percent_decode_str(encoded)
                .decode_utf8()
                .expect("encoded chat id is UTF-8"),
            jid,
        );
    }

    #[test]
    fn registry_tracks_and_forgets() {
        let windows = ChatWindows::default();
        let jid = "a@s.whatsapp.net";
        let label = chat_label(jid);

        windows.track(jid, &label);
        assert_eq!(windows.tracked_count(), 1);
        windows.forget(jid);
        assert_eq!(windows.tracked_count(), 0);
    }
}
