//! Tauri commands for communities. Delegates to `whatsapp_core`.

use tauri::State;
use whatsapp_core::{CommunityInfo, Jid};

use crate::state::AppState;

/// Create a community and return its parent-group JID.
#[tauri::command]
pub async fn communities_create(
    state: State<'_, AppState>,
    name: String,
    description: Option<String>,
) -> Result<Jid, String> {
    state
        .core()
        .create_community(&name, description.as_deref())
        .await
        .map_err(|error| error.to_string())
}

/// Fetch a community's name, description, linked groups and member count.
#[tauri::command]
pub async fn communities_info(
    state: State<'_, AppState>,
    community_id: String,
) -> Result<CommunityInfo, String> {
    state
        .core()
        .community_metadata(&Jid::new(community_id))
        .await
        .map_err(|error| error.to_string())
}

/// Link an existing group to a community.
#[tauri::command]
pub async fn communities_link_group(
    state: State<'_, AppState>,
    community_id: String,
    group_id: String,
) -> Result<(), String> {
    state
        .core()
        .link_group_to_community(&Jid::new(community_id), &Jid::new(group_id))
        .await
        .map_err(|error| error.to_string())
}

/// Unlink a subgroup from a community.
#[tauri::command]
pub async fn communities_unlink_group(
    state: State<'_, AppState>,
    community_id: String,
    group_id: String,
) -> Result<(), String> {
    state
        .core()
        .unlink_group_from_community(&Jid::new(community_id), &Jid::new(group_id))
        .await
        .map_err(|error| error.to_string())
}

/// Fetch the community invite link without resetting it.
#[tauri::command]
pub async fn communities_invite_link(
    state: State<'_, AppState>,
    community_id: String,
) -> Result<String, String> {
    state
        .core()
        .community_invite_link(&Jid::new(community_id))
        .await
        .map_err(|error| error.to_string())
}

/// Join a community through its invite link.
#[tauri::command]
pub async fn communities_join(
    state: State<'_, AppState>,
    invite_url: String,
) -> Result<Jid, String> {
    state
        .core()
        .join_community(&invite_url)
        .await
        .map_err(|error| error.to_string())
}

/// Deactivate (delete) a community; its groups are unlinked, not deleted.
#[tauri::command]
pub async fn communities_deactivate(
    state: State<'_, AppState>,
    community_id: String,
) -> Result<(), String> {
    state
        .core()
        .deactivate_community(&Jid::new(community_id))
        .await
        .map_err(|error| error.to_string())
}
