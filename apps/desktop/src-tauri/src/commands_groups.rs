//! Tauri commands for groups. Delegates to `whatsapp_core`.

use tauri::State;
use whatsapp_core::{GroupInfo, Jid};

use crate::state::AppState;

/// Create a group.
#[tauri::command]
pub async fn groups_create(
    state: State<'_, AppState>,
    subject: String,
    participants: Vec<String>,
) -> Result<Jid, String> {
    let participants: Vec<Jid> = participants.into_iter().map(Jid::new).collect();
    state
        .core()
        .create_group(&subject, &participants)
        .await
        .map_err(|error| error.to_string())
}

/// Fetch group metadata.
#[tauri::command]
pub async fn groups_info(state: State<'_, AppState>, chat_id: String) -> Result<GroupInfo, String> {
    state
        .core()
        .group_info(&Jid::new(chat_id))
        .await
        .map_err(|error| error.to_string())
}

/// Add participants to a group.
#[tauri::command]
pub async fn groups_add(
    state: State<'_, AppState>,
    chat_id: String,
    participants: Vec<String>,
) -> Result<(), String> {
    let participants: Vec<Jid> = participants.into_iter().map(Jid::new).collect();
    state
        .core()
        .add_participants(&Jid::new(chat_id), &participants)
        .await
        .map_err(|error| error.to_string())
}

/// Remove participants from a group.
#[tauri::command]
pub async fn groups_remove(
    state: State<'_, AppState>,
    chat_id: String,
    participants: Vec<String>,
) -> Result<(), String> {
    let participants: Vec<Jid> = participants.into_iter().map(Jid::new).collect();
    state
        .core()
        .remove_participants(&Jid::new(chat_id), &participants)
        .await
        .map_err(|error| error.to_string())
}

/// Leave a group.
#[tauri::command]
pub async fn groups_leave(state: State<'_, AppState>, chat_id: String) -> Result<(), String> {
    state
        .core()
        .leave_group(&Jid::new(chat_id))
        .await
        .map_err(|error| error.to_string())
}

/// Fetch the group invite link.
#[tauri::command]
pub async fn groups_invite_link(
    state: State<'_, AppState>,
    chat_id: String,
) -> Result<String, String> {
    state
        .core()
        .invite_link(&Jid::new(chat_id))
        .await
        .map_err(|error| error.to_string())
}
