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
#[tauri::command]
pub async fn media_send_file(
    state: State<'_, AppState>,
    chat_id: String,
    path: String,
    caption: Option<String>,
) -> Result<(), String> {
    state
        .core()
        .send_file(&Jid::new(chat_id), &path, caption.as_deref())
        .await
        .map_err(|error| error.to_string())
}
