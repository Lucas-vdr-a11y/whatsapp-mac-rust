//! Tauri commands for privacy settings and blocking. Delegates to `whatsapp_core`.
//!
//! NOTE: this module is registered in `lib.rs` during integration; the file is
//! kept self-contained so the wiring is one `mod` line plus the handler
//! registrations.

use tauri::State;
use whatsapp_core::{Jid, PrivacySnapshot};

use crate::state::AppState;

/// Block a contact.
#[tauri::command]
pub async fn privacy_block(state: State<'_, AppState>, jid: String) -> Result<(), String> {
    state
        .core()
        .block_contact(&Jid::new(jid))
        .await
        .map_err(|error| error.to_string())
}

/// Unblock a contact.
#[tauri::command]
pub async fn privacy_unblock(state: State<'_, AppState>, jid: String) -> Result<(), String> {
    state
        .core()
        .unblock_contact(&Jid::new(jid))
        .await
        .map_err(|error| error.to_string())
}

/// Fetch the account's privacy settings.
#[tauri::command]
pub async fn privacy_get(state: State<'_, AppState>) -> Result<PrivacySnapshot, String> {
    state
        .core()
        .fetch_privacy_settings()
        .await
        .map_err(|error| error.to_string())
}

/// Update one privacy setting. `setting`/`value` use the upstream wire names
/// (`"last"`, `"all"`, `"contact_blacklist"`, …).
#[tauri::command]
pub async fn privacy_set(
    state: State<'_, AppState>,
    setting: String,
    value: String,
) -> Result<(), String> {
    state
        .core()
        .set_privacy_setting(&setting, &value)
        .await
        .map_err(|error| error.to_string())
}

/// Set the default disappearing-message timer for new chats (`0` disables).
#[tauri::command]
pub async fn privacy_set_disappearing_default(
    state: State<'_, AppState>,
    seconds: u32,
) -> Result<(), String> {
    state
        .core()
        .set_disappearing_default(seconds)
        .await
        .map_err(|error| error.to_string())
}
