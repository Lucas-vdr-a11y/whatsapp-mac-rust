//! Tauri commands for status updates (stories). Delegates to `whatsapp_core`.

use tauri::State;
use whatsapp_core::statuses::StatusUpdate;

use crate::state::AppState;

/// Status updates from the last 24 hours, newest first.
#[tauri::command]
pub async fn statuses_list(state: State<'_, AppState>) -> Result<Vec<StatusUpdate>, String> {
    state.core().statuses().map_err(|error| error.to_string())
}

/// Mark one status update (`id`) as viewed.
#[tauri::command]
pub async fn status_viewed(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state
        .core()
        .mark_status_viewed(&id)
        .map_err(|error| error.to_string())
}
