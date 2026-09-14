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

/// Accept a ringing incoming call.
#[tauri::command]
pub async fn calls_answer(state: State<'_, AppState>, call_id: String) -> Result<(), String> {
    state
        .core()
        .answer_call(&call_id)
        .await
        .map_err(|error| error.to_string())
}

/// Reject a ringing incoming call.
#[tauri::command]
pub async fn calls_reject(state: State<'_, AppState>, call_id: String) -> Result<(), String> {
    state
        .core()
        .reject_call(&call_id)
        .await
        .map_err(|error| error.to_string())
}

/// Mute or unmute the active call with a chat.
#[tauri::command]
pub async fn calls_mute(
    state: State<'_, AppState>,
    chat_id: String,
    muted: bool,
) -> Result<(), String> {
    state
        .core()
        .set_call_muted(&Jid::new(chat_id), muted)
        .await
        .map_err(|error| error.to_string())
}
