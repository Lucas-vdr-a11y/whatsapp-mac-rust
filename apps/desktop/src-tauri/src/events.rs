//! Forwards core events to the webview.
//!
//! The core owns a `tokio::sync::broadcast` channel; this module bridges it to
//! Tauri's event system so the UI receives every [`CoreEvent`] on one channel.

use tauri::{AppHandle, Emitter};
use whatsapp_core::CoreEvent;

/// Channel name used for all core events.
pub const CORE_EVENT_CHANNEL: &str = "core://event";

/// Forwards events until the core's sender is dropped.
pub fn spawn_event_forwarder(
    app: AppHandle,
    mut receiver: tokio::sync::broadcast::Receiver<CoreEvent>,
) {
    tauri::async_runtime::spawn(async move {
        loop {
            match receiver.recv().await {
                Ok(event) => {
                    if let Err(error) = app.emit(CORE_EVENT_CHANNEL, &event) {
                        tracing::warn!(%error, "failed to emit core event to the webview");
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    // The UI missed events (slow render or a packed history
                    // sync). It re-syncs from `list_*` commands; keep going.
                    tracing::warn!(skipped, "core event stream lagged");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}
