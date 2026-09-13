//! Files opened "with RustWA": Finder *Open With*, Dock drops and the macOS
//! share sheet.
//!
//! # How macOS delivers files
//!
//! When the OS asks RustWA to open files — Finder's *Open With*, dragging files
//! onto the Dock icon, or the *Share* menu on a selection — `NSApplication`
//! calls `application:openURLs:` on the app delegate. Tauri surfaces that call
//! as [`tauri::RunEvent::Opened`] with `file://` URLs, and `lib.rs` passes them
//! to [`dispatch`] from the `app.run` callback. (Scheme links such as
//! `rustwa://chat/...` arrive through the same event; those are handled by
//! [`crate::deep_link`] and ignored here.)
//!
//! # Delivery to the UI
//!
//! The host emits [`OPEN_FILES_EVENT`] (`ui://open-files`) with an
//! [`OpenFilesPayload`] (`{ paths: [String] }`). A selection made before the
//! webview has mounted its listeners (a cold start) is buffered instead: the
//! UI calls [`file_open_ready`] once after registering its listener and
//! receives the buffered paths as the command result. Delivery is
//! exactly-once — unlike a deep link, a duplicated file batch would send every
//! file twice — so an arrival racing the [`file_open_ready`] call is either
//! buffered-and-drained or emitted live, never both.
//!
//! The frontend then routes the paths like a drop: send to the selected chat,
//! or show the chat chooser when no chat is selected.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use url::Url;

/// Event emitted when files should be sent to the UI's active chat.
///
/// Payload: [`OpenFilesPayload`], serialising to `{ "paths": ["…"] }`.
pub const OPEN_FILES_EVENT: &str = "ui://open-files";

/// Payload of [`OPEN_FILES_EVENT`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenFilesPayload {
    /// Absolute local file paths, in the order the OS provided them.
    pub paths: Vec<String>,
}

/// Installs the delivery state. Must run during `setup` so the state exists
/// before the event loop starts dispatching [`tauri::RunEvent::Opened`].
pub fn setup<R: Runtime>(app: &AppHandle<R>) {
    app.manage(OpenFilesState::default());
}

/// Turns a batch of OS-opened URLs into a file delivery.
///
/// URLs that are not `file://` (for example `rustwa://` links) are ignored;
/// calls carrying no files do nothing.
pub fn dispatch<R: Runtime>(app: &AppHandle<R>, urls: &[Url]) {
    let mut paths: Vec<String> = Vec::new();
    for url in urls {
        if let Some(path) = file_path_from_url(url) {
            if !paths.contains(&path) {
                paths.push(path);
            }
        } else {
            tracing::debug!(%url, "ignoring opened URL without a file path");
        }
    }
    if paths.is_empty() {
        return;
    }

    if let Err(error) = app.state::<OpenFilesState>().deliver(app, paths) {
        tracing::warn!(%error, "failed to deliver opened files");
    }
}

/// Converts a `file://` URL into an absolute local path, if possible.
pub fn file_path_from_url(url: &Url) -> Option<String> {
    if url.scheme() != "file" {
        return None;
    }
    url.to_file_path()
        .ok()
        .map(|path| path.to_string_lossy().into_owned())
}

/// Marks the UI as ready to receive [`OPEN_FILES_EVENT`] and returns the paths
/// of files that arrived before the UI mounted (cold start), if any.
///
/// The UI should call this once after registering its `ui://open-files`
/// listener.
#[tauri::command]
pub fn file_open_ready(state: tauri::State<'_, OpenFilesState>) -> Vec<String> {
    state.mark_ready()
}

/// Buffers file deliveries until the webview has mounted its listeners.
///
/// The ready flag and the buffer share one lock: a delivery that races the
/// [`file_open_ready`] call is either buffered (and later drained) or emitted
/// live, never both.
#[derive(Default)]
pub struct OpenFilesState {
    inner: Mutex<DeliveryInner>,
}

#[derive(Default)]
struct DeliveryInner {
    ui_ready: bool,
    pending: Vec<String>,
}

impl OpenFilesState {
    fn deliver<R: Runtime>(&self, app: &AppHandle<R>, paths: Vec<String>) -> tauri::Result<()> {
        let ready = {
            let mut inner = self
                .inner
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if inner.ui_ready {
                true
            } else {
                tracing::debug!(
                    count = paths.len(),
                    "buffering opened files until the UI is ready"
                );
                inner.pending.extend(paths.iter().cloned());
                false
            }
        };

        if ready {
            app.emit(OPEN_FILES_EVENT, OpenFilesPayload { paths })?;
        }
        Ok(())
    }

    fn mark_ready(&self) -> Vec<String> {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        inner.ui_ready = true;
        std::mem::take(&mut inner.pending)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path_of(raw: &str) -> Option<String> {
        file_path_from_url(&Url::parse(raw).expect("test URL should parse"))
    }

    #[test]
    fn accepts_file_urls() {
        assert_eq!(
            path_of("file:///Users/lucas/Downloads/photo.png"),
            Some("/Users/lucas/Downloads/photo.png".to_owned())
        );
    }

    #[test]
    fn decodes_escaped_file_urls() {
        assert_eq!(
            path_of("file:///Users/lucas/My%20Files/report%20(final).pdf"),
            Some("/Users/lucas/My Files/report (final).pdf".to_owned())
        );
    }

    #[test]
    fn ignores_non_file_urls() {
        assert_eq!(path_of("rustwa://chat/31612345678@s.whatsapp.net"), None);
        assert_eq!(path_of("https://wa.me/31612345678"), None);
    }
}
