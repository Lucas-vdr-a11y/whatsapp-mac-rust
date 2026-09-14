//! Tauri commands for message actions. Delegates to `whatsapp_core`.

use tauri::State;
use whatsapp_core::{Jid, Message};

use crate::state::AppState;

/// Send a text message quoting an existing message.
#[tauri::command]
pub async fn actions_send_quoting(
    state: State<'_, AppState>,
    chat_id: String,
    text: String,
    quoted_message_id: String,
) -> Result<Message, String> {
    state
        .core()
        .send_text_quoting(&Jid::new(chat_id), &text, &quoted_message_id)
        .await
        .map_err(|error| error.to_string())
}

/// React to (or un-react from) a message.
#[tauri::command]
pub async fn actions_react(
    state: State<'_, AppState>,
    chat_id: String,
    message_id: String,
    emoji: String,
    from_me: bool,
) -> Result<(), String> {
    state
        .core()
        .send_reaction(&Jid::new(chat_id), &message_id, &emoji, from_me)
        .await
        .map_err(|error| error.to_string())
}

/// Edit a message this account sent.
#[tauri::command]
pub async fn actions_edit(
    state: State<'_, AppState>,
    chat_id: String,
    message_id: String,
    text: String,
) -> Result<(), String> {
    state
        .core()
        .edit_message(&Jid::new(chat_id), &message_id, &text)
        .await
        .map_err(|error| error.to_string())
}

/// Delete a message, for everyone or locally.
#[tauri::command]
pub async fn actions_revoke(
    state: State<'_, AppState>,
    chat_id: String,
    message_id: String,
    for_everyone: bool,
) -> Result<(), String> {
    state
        .core()
        .revoke_message(&Jid::new(chat_id), &message_id, for_everyone)
        .await
        .map_err(|error| error.to_string())
}

/// Star or unstar a message.
#[tauri::command]
pub async fn actions_star(
    state: State<'_, AppState>,
    chat_id: String,
    message_id: String,
    from_me: bool,
    star: bool,
) -> Result<(), String> {
    state
        .core()
        .star_message(&Jid::new(chat_id), &message_id, from_me, star)
        .await
        .map_err(|error| error.to_string())
}
