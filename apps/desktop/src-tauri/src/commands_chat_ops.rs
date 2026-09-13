//! Tauri commands for chat lifecycle, mentions, forwards, pins, polls and
//! events. Delegates to `whatsapp_core`.

use tauri::State;
use whatsapp_core::{Jid, Message};

use crate::state::AppState;

/// Delete a chat for this account.
#[tauri::command]
pub async fn chat_delete(state: State<'_, AppState>, chat_id: String) -> Result<(), String> {
    state
        .core()
        .delete_chat(&Jid::new(chat_id))
        .await
        .map_err(|error| error.to_string())
}

/// Clear a chat's messages while keeping the chat.
#[tauri::command]
pub async fn chat_clear(state: State<'_, AppState>, chat_id: String) -> Result<(), String> {
    state
        .core()
        .clear_chat(&Jid::new(chat_id))
        .await
        .map_err(|error| error.to_string())
}

/// Turn disappearing messages on for a chat (0 disables).
#[tauri::command]
pub async fn chat_set_disappearing(
    state: State<'_, AppState>,
    chat_id: String,
    seconds: u32,
) -> Result<(), String> {
    state
        .core()
        .set_disappearing_timer(&Jid::new(chat_id), seconds)
        .await
        .map_err(|error| error.to_string())
}

/// Send a text message that @-mentions the given JIDs.
#[tauri::command]
pub async fn chat_send_mentions(
    state: State<'_, AppState>,
    chat_id: String,
    text: String,
    mentions: Vec<String>,
) -> Result<Message, String> {
    let mentions: Vec<Jid> = mentions.into_iter().map(Jid::new).collect();
    state
        .core()
        .send_text_with_mentions(&Jid::new(chat_id), &text, &mentions)
        .await
        .map_err(|error| error.to_string())
}

/// Forward a stored text message to another chat.
#[tauri::command]
pub async fn message_forward(
    state: State<'_, AppState>,
    to_chat: String,
    from_chat: String,
    message_id: String,
) -> Result<Message, String> {
    state
        .core()
        .forward_message(&Jid::new(to_chat), &Jid::new(from_chat), &message_id)
        .await
        .map_err(|error| error.to_string())
}

/// Pin a message in a chat for all participants (1, 7 or 30 days).
#[tauri::command]
pub async fn message_pin(
    state: State<'_, AppState>,
    chat_id: String,
    message_id: String,
    days: u32,
    from_me: bool,
) -> Result<(), String> {
    state
        .core()
        .pin_message(&Jid::new(chat_id), &message_id, days, from_me)
        .await
        .map_err(|error| error.to_string())
}

/// Unpin a message previously pinned in a chat.
#[tauri::command]
pub async fn message_unpin(
    state: State<'_, AppState>,
    chat_id: String,
    message_id: String,
    from_me: bool,
) -> Result<(), String> {
    state
        .core()
        .unpin_message(&Jid::new(chat_id), &message_id, from_me)
        .await
        .map_err(|error| error.to_string())
}

/// Create a poll. Returns the poll's message id.
#[tauri::command]
pub async fn poll_create(
    state: State<'_, AppState>,
    chat_id: String,
    question: String,
    options: Vec<String>,
    selectable_count: u32,
) -> Result<String, String> {
    state
        .core()
        .create_poll(&Jid::new(chat_id), &question, &options, selectable_count)
        .await
        .map_err(|error| error.to_string())
}

/// Vote on a poll (an empty selection clears the previous vote).
#[tauri::command]
pub async fn poll_vote(
    state: State<'_, AppState>,
    chat_id: String,
    poll_message_id: String,
    option_names: Vec<String>,
) -> Result<(), String> {
    state
        .core()
        .vote_poll(&Jid::new(chat_id), &poll_message_id, &option_names)
        .await
        .map_err(|error| error.to_string())
}

/// Create an event (start_ts is Unix seconds; 0 means "no start time").
#[tauri::command]
pub async fn event_create(
    state: State<'_, AppState>,
    chat_id: String,
    name: String,
    description: Option<String>,
    start_ts: u64,
) -> Result<String, String> {
    state
        .core()
        .create_event(&Jid::new(chat_id), &name, description.as_deref(), start_ts)
        .await
        .map_err(|error| error.to_string())
}

/// Every starred message, newest first.
#[tauri::command]
pub async fn list_starred(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<Message>, String> {
    state
        .core()
        .store()
        .list_starred(limit.unwrap_or(200))
        .map_err(|error| error.to_string())
}

/// Search message text, newest first.
#[tauri::command]
pub async fn search_messages(
    state: State<'_, AppState>,
    query: String,
    limit: Option<u32>,
) -> Result<Vec<Message>, String> {
    state
        .core()
        .store()
        .search_messages(&query, limit.unwrap_or(100))
        .map_err(|error| error.to_string())
}
