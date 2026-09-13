//! Tauri commands for calls. Delegates to `whatsapp_core`.

use tauri::State;
use whatsapp_core::Jid;

use crate::state::AppState;

/// Start an outgoing voice or video call.
#[tauri::command]
pub async fn calls_start(
    state: State<'_, AppState>,
    chat_id: String,
    video: bool,
) -> Result<(), String> {
    state
        .core()
        .start_call(&Jid::new(chat_id), video)
        .await
        .map_err(|error| error.to_string())
}

/// Hang up the current call.
#[tauri::command]
pub async fn calls_end(state: State<'_, AppState>, chat_id: String) -> Result<(), String> {
    state
        .core()
        .end_call(&Jid::new(chat_id))
        .await
        .map_err(|error| error.to_string())
}
