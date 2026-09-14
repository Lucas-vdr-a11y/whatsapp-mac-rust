//! Tauri commands for the account's own profile (name, about, picture).
//! Delegates to `whatsapp_core`.
//!
//! NOTE: this module is registered in `lib.rs` during integration; the file is
//! kept self-contained so the wiring is a `mod` line plus the handler
//! registrations.

use tauri::State;
use whatsapp_core::profile::OwnProfile;

use crate::state::AppState;

/// Read the account's own profile (display name, about, avatar URL).
#[tauri::command]
pub async fn profile_get(state: State<'_, AppState>) -> Result<OwnProfile, String> {
    state
        .core()
        .own_profile()
        .await
        .map_err(|error| error.to_string())
}

/// Change the display name peers see (trimmed, max 25 characters).
#[tauri::command]
pub async fn profile_set_name(state: State<'_, AppState>, name: String) -> Result<(), String> {
    state
        .core()
        .set_push_name(&name)
        .await
        .map_err(|error| error.to_string())
}

/// Change the about/status text (trimmed, max 139 characters; empty clears).
#[tauri::command]
pub async fn profile_set_about(state: State<'_, AppState>, text: String) -> Result<(), String> {
    state
        .core()
        .set_about(&text)
        .await
        .map_err(|error| error.to_string())
}

/// Replace the profile picture with a JPEG file on disk.
#[tauri::command]
pub async fn profile_set_picture(state: State<'_, AppState>, path: String) -> Result<(), String> {
    state
        .core()
        .set_profile_picture(&path)
        .await
        .map_err(|error| error.to_string())
}

/// Remove the profile picture.
#[tauri::command]
pub async fn profile_remove_picture(state: State<'_, AppState>) -> Result<(), String> {
    state
        .core()
        .remove_profile_picture()
        .await
        .map_err(|error| error.to_string())
}
