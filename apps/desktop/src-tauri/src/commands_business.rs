//! Tauri commands for business and power features. Delegates to `whatsapp_core`.
//!
//! Quick replies are not exposed: whatsapp-rust 0.7.0 can write a `quick_reply`
//! app-state action through the generic escape hatch, but has no reader for the
//! `regular` collection it lives in and emits no quick-reply event, so there is
//! nothing honest to return for a list command.
//!
//! NOTE: this module is registered in `lib.rs` during integration; the file is
//! kept self-contained so the wiring is one `mod` line plus the handler
//! registrations.

use tauri::State;
use whatsapp_core::Jid;
use whatsapp_core::business::{BusinessProfile, Catalog, Label};

use crate::state::AppState;

/// Fetch a business profile for a user JID.
#[tauri::command]
pub async fn business_profile(
    state: State<'_, AppState>,
    jid: String,
) -> Result<BusinessProfile, String> {
    state
        .core()
        .business_profile(&Jid::new(jid))
        .await
        .map_err(|error| error.to_string())
}

/// List the account's chat labels.
#[tauri::command]
pub async fn labels_list(state: State<'_, AppState>) -> Result<Vec<Label>, String> {
    state
        .core()
        .fetch_labels()
        .await
        .map_err(|error| error.to_string())
}

/// Associate a label with a chat.
#[tauri::command]
pub async fn labels_add(
    state: State<'_, AppState>,
    chat_id: String,
    label_id: String,
) -> Result<(), String> {
    state
        .core()
        .add_label(&Jid::new(chat_id), &label_id)
        .await
        .map_err(|error| error.to_string())
}

/// Remove a label association from a chat.
#[tauri::command]
pub async fn labels_remove(
    state: State<'_, AppState>,
    chat_id: String,
    label_id: String,
) -> Result<(), String> {
    state
        .core()
        .remove_label(&Jid::new(chat_id), &label_id)
        .await
        .map_err(|error| error.to_string())
}

/// Fetch the first page of a business's product catalog.
#[tauri::command]
pub async fn catalog_fetch(state: State<'_, AppState>, jid: String) -> Result<Catalog, String> {
    state
        .core()
        .fetch_catalog(&Jid::new(jid))
        .await
        .map_err(|error| error.to_string())
}

/// Resolve a WhatsApp username to a JID (`null` when the name is unknown).
#[tauri::command]
pub async fn username_lookup(
    state: State<'_, AppState>,
    username: String,
) -> Result<Option<Jid>, String> {
    state
        .core()
        .username_lookup(&username)
        .await
        .map_err(|error| error.to_string())
}
