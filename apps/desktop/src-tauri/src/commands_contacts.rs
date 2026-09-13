//! Tauri commands for contacts and avatars. Delegates to `whatsapp_core`.
//!
//! NOTE: this module is registered in `lib.rs` during integration; the file is
//! kept self-contained so the wiring is a two-line change.

use tauri::State;
use whatsapp_core::{ContactProfile, Jid};

use crate::state::AppState;

/// Resolve display metadata for a set of contacts.
#[tauri::command]
pub async fn contacts_resolve(
    state: State<'_, AppState>,
    jids: Vec<String>,
) -> Result<Vec<ContactProfile>, String> {
    let jids: Vec<Jid> = jids.into_iter().map(Jid::new).collect();
    state
        .core()
        .resolve_contacts(&jids)
        .await
        .map_err(|error| error.to_string())
}

/// Fetch the profile-picture URL for a contact or group, if any.
#[tauri::command]
pub async fn contacts_avatar(
    state: State<'_, AppState>,
    jid: String,
) -> Result<Option<String>, String> {
    state
        .core()
        .avatar_url(&Jid::new(jid))
        .await
        .map_err(|error| error.to_string())
}
