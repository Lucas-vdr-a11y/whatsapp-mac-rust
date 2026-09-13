//! Tauri commands for the media pipeline. Delegates to `whatsapp_core`.

use tauri::State;
use whatsapp_core::{Jid, MediaFile};

use crate::state::AppState;

/// Download and cache the media attached to a message.
#[tauri::command]
pub async fn media_download(
    state: State<'_, AppState>,
    message_id: String,
) -> Result<MediaFile, String> {
    state
        .core()
        .download_media(&message_id)
        .await
        .map_err(|error| error.to_string())
}

/// Send a file from disk to a chat.
///
/// `ptv` sends a short video as a push-to-video ("video note") message
/// instead of a regular video; the core enforces the 60-second cap.
#[tauri::command]
pub async fn media_send_file(
    state: State<'_, AppState>,
    chat_id: String,
    path: String,
    caption: Option<String>,
    ptv: Option<bool>,
) -> Result<(), String> {
    let chat_id = Jid::new(chat_id);
    let core = state.core();
    let result = if ptv.unwrap_or(false) {
        core.send_video_note(&chat_id, &path, caption.as_deref())
            .await
    } else {
        core.send_file(&chat_id, &path, caption.as_deref()).await
    };
    result.map_err(|error| error.to_string())
}
