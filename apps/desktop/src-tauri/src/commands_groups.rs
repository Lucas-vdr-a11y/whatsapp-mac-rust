//! Tauri commands for groups. Delegates to `whatsapp_core`.

use tauri::State;
use whatsapp_core::groups::JoinRequest;
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

/// Reset the group invite link, returning the new link.
#[tauri::command]
pub async fn groups_reset_invite_link(
    state: State<'_, AppState>,
    chat_id: String,
) -> Result<String, String> {
    state
        .core()
        .reset_invite_link(&Jid::new(chat_id))
        .await
        .map_err(|error| error.to_string())
}

/// Change the group subject.
#[tauri::command]
pub async fn groups_set_subject(
    state: State<'_, AppState>,
    chat_id: String,
    subject: String,
) -> Result<(), String> {
    state
        .core()
        .set_group_subject(&Jid::new(chat_id), &subject)
        .await
        .map_err(|error| error.to_string())
}

/// Set (or clear, with an empty string) the group description.
#[tauri::command]
pub async fn groups_set_description(
    state: State<'_, AppState>,
    chat_id: String,
    text: String,
) -> Result<(), String> {
    state
        .core()
        .set_group_description(&Jid::new(chat_id), &text)
        .await
        .map_err(|error| error.to_string())
}

/// List the group's pending join requests.
#[tauri::command]
pub async fn groups_pending_participants(
    state: State<'_, AppState>,
    chat_id: String,
) -> Result<Vec<JoinRequest>, String> {
    state
        .core()
        .pending_participants(&Jid::new(chat_id))
        .await
        .map_err(|error| error.to_string())
}

/// Approve pending join requests.
#[tauri::command]
pub async fn groups_approve_participants(
    state: State<'_, AppState>,
    chat_id: String,
    participants: Vec<String>,
) -> Result<(), String> {
    let participants: Vec<Jid> = participants.into_iter().map(Jid::new).collect();
    state
        .core()
        .approve_participants(&Jid::new(chat_id), &participants)
        .await
        .map_err(|error| error.to_string())
}

/// Reject pending join requests.
#[tauri::command]
pub async fn groups_reject_participants(
    state: State<'_, AppState>,
    chat_id: String,
    participants: Vec<String>,
) -> Result<(), String> {
    let participants: Vec<Jid> = participants.into_iter().map(Jid::new).collect();
    state
        .core()
        .reject_participants(&Jid::new(chat_id), &participants)
        .await
        .map_err(|error| error.to_string())
}
