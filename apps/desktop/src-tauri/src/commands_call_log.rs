//! Tauri commands for the phone-synced call log. Delegates to `whatsapp_core`.

use tauri::State;
use whatsapp_core::calls::CallLogEntry;

use crate::state::AppState;

/// Phone-synced call-log rows, newest first.
///
/// Returns the UI-facing `calls::CallLogEntry` shape (camelCase over IPC);
/// the core reports a clear error until the store hook is wired.
#[tauri::command]
pub fn call_log_list(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<CallLogEntry>, String> {
    state
        .core()
        .list_call_log(limit.unwrap_or(200))
        .map_err(|error| error.to_string())
}
