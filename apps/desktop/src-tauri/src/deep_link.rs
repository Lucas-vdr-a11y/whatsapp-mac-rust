//! `rustwa://` URL scheme handling and WhatsApp link parsing.
//!
//! # Registered scheme
//!
//! `tauri.conf.json` registers the `rustwa` scheme under
//! `plugins.deep-link.desktop.schemes`; the Tauri bundler reads the same
//! config and adds it to the macOS bundle's `CFBundleURLTypes`, so the OS
//! launches/forwards `rustwa://` links to RustWA.
//!
//! # Supported links
//!
//! | Link shape | Result |
//! | --- | --- |
//! | `rustwa://chat/<jid>` | opens the chat with JID `<jid>` |
//! | `https://wa.me/<phone>` | opens `wa.me` personal chats |
//! | `https://api.whatsapp.com/send?phone=<phone>` | same |
//!
//! `<jid>` and `<phone>` may be percent-encoded
//! (`rustwa://chat/31612345678%40s.whatsapp.net`), and `<phone>` may carry a
//! leading `+`. Phone numbers are normalised to
//! `<digits>@s.whatsapp.net`; full JIDs are passed through unchanged. Anything
//! else — including `chat.whatsapp.com` invite links, which would need a group
//! join round-trip — is ignored with a debug log.
//!
//! # Delivery to the UI
//!
//! The host emits [`OPEN_CHAT_EVENT`] (`ui://open-chat`) with an
//! [`OpenChatPayload`] (`{ chatId }`). A link that arrives before the webview
//! has mounted its event listeners (a cold start via `rustwa://`) is also
//! buffered, because the emit would otherwise be lost: the UI calls the
//! [`deep_link_ready`] command once after registering its listener, and the
//! command returns the buffered chat id, if any. After that call every link is
//! emitted live and nothing is buffered.
//!
//! # Pasting links into the app
//!
//! [`open_link`] exposes the same parser as a command for links the UI handles
//! explicitly (a detected `wa.me` link in a message, a paste action, …), so the
//! parsing rules live in one place. It uses the identical delivery path and
//! returns an error for links without a chat target.

use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_deep_link::DeepLinkExt;
use url::Url;

/// Event emitted when a link should open a chat.
///
/// Payload: [`OpenChatPayload`], serialising to `{ "chatId": "<jid>" }`.
pub const OPEN_CHAT_EVENT: &str = "ui://open-chat";

/// Payload of [`OPEN_CHAT_EVENT`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenChatPayload {
    /// WhatsApp JID, e.g. `31612345678@s.whatsapp.net` or `1234567890-123@g.us`.
    pub chat_id: String,
}

/// Attaches the URL listener, drains the plugin's cold-start cache and
/// installs the delivery state.
pub fn setup<R: Runtime>(app: &AppHandle<R>) {
    app.manage(DeliveryState::default());

    let handle = app.clone();
    app.deep_link().on_open_url(move |event| {
        for url in event.urls() {
            dispatch(&handle, &url);
        }
    });

    // Cold start: the plugin caches URLs that arrived before this listener was
    // attached (`RunEvent::Opened` can fire during plugin setup). Draining the
    // cache here can not double-deliver: setup runs synchronously on the main
    // thread, so no URL event can interleave between the listener above and
    // this read.
    match app.deep_link().get_current() {
        Ok(Some(urls)) => {
            for url in urls {
                dispatch(app, &url);
            }
        }
        Ok(None) => {}
        Err(error) => tracing::warn!(%error, "failed to read the pending deep link"),
    }
}

/// Turns a deep link into a chat-open request. Returns the JID when the link
/// targets a chat, `None` for links this host does not understand.
pub fn chat_id_from_link(url: &Url) -> Option<String> {
    match url.scheme() {
        "rustwa" => rustwa_chat(url),
        "http" | "https" => whatsapp_web_chat(url),
        _ => None,
    }
}

/// Parses `rustwa://chat/<jid>`.
fn rustwa_chat(url: &Url) -> Option<String> {
    if url.host_str() != Some("chat") {
        return None;
    }
    let raw = url.path().trim_start_matches('/');
    if raw.is_empty() {
        return None;
    }
    percent_encoding::percent_decode_str(raw)
        .decode_utf8()
        .ok()
        .map(|jid| jid.into_owned())
}

/// Parses `wa.me/<phone>` and `api.whatsapp.com/send?phone=<phone>`.
fn whatsapp_web_chat(url: &Url) -> Option<String> {
    let host = url.host_str()?;
    let raw_phone = if host == "wa.me" || host == "www.wa.me" {
        url.path_segments()?.next()?.to_owned()
    } else if (host == "api.whatsapp.com" || host == "web.whatsapp.com")
        && url.path().trim_end_matches('/') == "/send"
    {
        // `query_pairs` already percent-decodes the value.
        url.query_pairs()
            .find(|(key, _)| key == "phone")
            .map(|(_, value)| value.into_owned())?
    } else {
        return None;
    };

    normalize_phone(&raw_phone)
}

/// Converts a phone number into a personal-chat JID. `raw` must consist of
/// ASCII digits, optionally prefixed with `+`; anything else (for example
/// `wa.me/message/<token>` short links) is rejected.
fn normalize_phone(raw: &str) -> Option<String> {
    let digits = raw.strip_prefix('+').unwrap_or(raw);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some(format!("{digits}@s.whatsapp.net"))
}

/// Routes one link to the UI or the cold-start buffer.
fn dispatch<R: Runtime>(app: &AppHandle<R>, url: &Url) {
    match chat_id_from_link(url) {
        Some(chat_id) => {
            if let Err(error) = app.state::<DeliveryState>().deliver(app, chat_id) {
                tracing::warn!(%error, "failed to deliver the chat link");
            }
        }
        None => tracing::debug!(%url, "ignoring link without a chat target"),
    }
}

fn emit_open_chat<R: Runtime>(app: &AppHandle<R>, chat_id: String) -> tauri::Result<()> {
    app.emit(OPEN_CHAT_EVENT, OpenChatPayload { chat_id })
}

/// Parses a URL the UI handles explicitly (pasted `wa.me` link, detected link
/// in a message) and opens its chat.
///
/// Identical routing to a `rustwa://` deep link: the UI receives
/// [`OPEN_CHAT_EVENT`], or [`deep_link_ready`] returns the JID if the webview
/// has not mounted yet.
///
/// Errors when `url` is not a valid URL or does not point at a chat.
#[tauri::command]
pub fn open_link(app: AppHandle, url: String) -> Result<(), String> {
    let parsed = Url::parse(&url).map_err(|error| format!("invalid URL: {error}"))?;
    let chat_id = chat_id_from_link(&parsed).ok_or_else(|| format!("unsupported link: {url}"))?;
    app.state::<DeliveryState>()
        .deliver(&app, chat_id)
        .map_err(|error| error.to_string())
}

/// Marks the UI as ready to receive [`OPEN_CHAT_EVENT`] and returns the chat
/// id of a link that arrived before the UI mounted (cold start), if any.
///
/// The UI should call this once after registering its `ui://open-chat`
/// listener.
#[tauri::command]
pub fn deep_link_ready(state: tauri::State<'_, DeliveryState>) -> Option<String> {
    state.mark_ready()
}

/// Buffers link deliveries until the webview has mounted its listeners.
#[derive(Default)]
pub struct DeliveryState {
    ui_ready: AtomicBool,
    pending: Mutex<Option<String>>,
}

impl DeliveryState {
    fn deliver<R: Runtime>(&self, app: &AppHandle<R>, chat_id: String) -> tauri::Result<()> {
        if !self.ui_ready.load(Ordering::Acquire)
            && let Ok(mut pending) = self.pending.lock()
        {
            tracing::debug!(%chat_id, "buffering a chat link until the UI is ready");
            *pending = Some(chat_id.clone());
        }

        // Emit unconditionally: a webview that is already listening receives
        // the event even if it never called [`deep_link_ready`]; the buffer
        // above is the cold-start safety net.
        emit_open_chat(app, chat_id)
    }

    fn mark_ready(&self) -> Option<String> {
        self.ui_ready.store(true, Ordering::Release);
        self.pending
            .lock()
            .ok()
            .and_then(|mut pending| pending.take())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chat_id(raw: &str) -> Option<String> {
        chat_id_from_link(&Url::parse(raw).expect("test URL should parse"))
    }

    #[test]
    fn accepts_rustwa_chat_links() {
        assert_eq!(
            chat_id("rustwa://chat/31612345678@s.whatsapp.net"),
            Some("31612345678@s.whatsapp.net".to_owned())
        );
        assert_eq!(
            chat_id("rustwa://chat/31612345678%40s.whatsapp.net"),
            Some("31612345678@s.whatsapp.net".to_owned())
        );
        assert_eq!(
            chat_id("rustwa://chat/1234567890-123456@g.us"),
            Some("1234567890-123456@g.us".to_owned())
        );
    }

    #[test]
    fn rejects_unrelated_rustwa_links() {
        assert_eq!(chat_id("rustwa://settings"), None);
        assert_eq!(chat_id("rustwa://chat/"), None);
    }

    #[test]
    fn accepts_wa_me_links() {
        assert_eq!(
            chat_id("https://wa.me/31612345678"),
            Some("31612345678@s.whatsapp.net".to_owned())
        );
        assert_eq!(
            chat_id("https://wa.me/+31612345678?text=hi"),
            Some("31612345678@s.whatsapp.net".to_owned())
        );
        assert_eq!(chat_id("https://wa.me/message/AB12CD34"), None);
        assert_eq!(chat_id("https://wa.me/31-6-12345678"), None);
    }

    #[test]
    fn accepts_api_send_links() {
        assert_eq!(
            chat_id("https://api.whatsapp.com/send?phone=31612345678"),
            Some("31612345678@s.whatsapp.net".to_owned())
        );
        assert_eq!(chat_id("https://api.whatsapp.com/send?phone=abc"), None);
        assert_eq!(
            chat_id("https://api.whatsapp.com/other?phone=31612345678"),
            None
        );
    }

    #[test]
    fn ignores_unrelated_https_links() {
        assert_eq!(chat_id("https://example.com/31612345678"), None);
        assert_eq!(chat_id("https://chat.whatsapp.com/INVITECODE"), None);
    }
}
