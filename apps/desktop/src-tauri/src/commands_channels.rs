//! Tauri commands for channels and status. Delegates to `whatsapp_core`.

use tauri::State;
use whatsapp_core::Jid;

use crate::state::AppState;

/// Follow a channel from an invite link.
#[tauri::command]
pub async fn channels_follow(
    state: State<'_, AppState>,
    invite_url: String,
) -> Result<Jid, String> {
    state
        .core()
        .follow_channel(&invite_url)
        .await
        .map_err(|error| error.to_string())
}

/// Unfollow a channel.
#[tauri::command]
pub async fn channels_unfollow(state: State<'_, AppState>, chat_id: String) -> Result<(), String> {
    state
        .core()
        .unfollow_channel(&Jid::new(chat_id))
        .await
        .map_err(|error| error.to_string())
}

/// Send a text message to a channel we administer.
#[tauri::command]
pub async fn channels_send(
    state: State<'_, AppState>,
    chat_id: String,
    text: String,
) -> Result<(), String> {
    state
        .core()
        .send_channel_message(&Jid::new(chat_id), &text)
        .await
        .map_err(|error| error.to_string())
}

/// Post a text status update.
#[tauri::command]
pub async fn channels_post_status(
    state: State<'_, AppState>,
    text: String,
    background_argb: Option<u32>,
) -> Result<(), String> {
    state
        .core()
        .post_status_text(&text, background_argb.unwrap_or(0xFF1DAA61))
        .await
        .map_err(|error| error.to_string())
}
