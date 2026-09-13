//! Tauri host for RustWA.
//!
//! This crate is intentionally thin: it owns the window, menus and native
//! integrations and marshals commands / events between the webview and
//! `whatsapp-core`. All business logic lives in the core crate.

mod app_lock;
mod commands_actions;
mod commands_calls;
mod commands_channels;
mod commands_contacts;
mod commands_groups;
mod commands_media;
mod deep_link;
mod events;
mod menu;
mod platform;
mod state;
mod tray;

use std::sync::Arc;

use serde::Serialize;
use tauri::Manager;
use whatsapp_core::{ChatSummary, ClientConfig, Jid, Message, Store, WaClient};

/// Static information about the running build, used by the UI's about screen.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    name: &'static str,
    version: &'static str,
    /// Which protocol backend is compiled in.
    core: &'static str,
}

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        name: "RustWA",
        version: env!("CARGO_PKG_VERSION"),
        core: "whatsapp-rust",
    }
}

/// Starts the connection/pairing flow. Idempotent: calling it while already
/// connected (or connecting) succeeds without restarting the session.
#[tauri::command]
async fn core_connect(state: tauri::State<'_, state::AppState>) -> Result<(), String> {
    state
        .core()
        .connect()
        .await
        .map_err(|error| error.to_string())
}

/// Unlinks this device and clears the local session.
#[tauri::command]
async fn core_logout(state: tauri::State<'_, state::AppState>) -> Result<(), String> {
    state
        .core()
        .logout()
        .await
        .map_err(|error| error.to_string())
}

/// Stops the current attempt and requests fresh pairing QR codes.
#[tauri::command]
async fn core_restart_pairing(state: tauri::State<'_, state::AppState>) -> Result<(), String> {
    state
        .core()
        .restart_pairing()
        .await
        .map_err(|error| error.to_string())
}

/// Deletes the local session and starts over (recovery path).
#[tauri::command]
async fn core_reset_session(state: tauri::State<'_, state::AppState>) -> Result<(), String> {
    state
        .core()
        .reset_session()
        .await
        .map_err(|error| error.to_string())
}

/// Sends a text message to a chat. Returns the stored local echo.
#[tauri::command]
async fn send_text(
    state: tauri::State<'_, state::AppState>,
    chat_id: String,
    text: String,
) -> Result<Message, String> {
    state
        .core()
        .send_text(&Jid::new(chat_id), &text)
        .await
        .map_err(|error| error.to_string())
}

/// Chats known to the local store, newest activity first.
#[tauri::command]
async fn list_chats(state: tauri::State<'_, state::AppState>) -> Result<Vec<ChatSummary>, String> {
    state.core().list_chats().map_err(|error| error.to_string())
}

/// Messages of one chat, oldest first.
#[tauri::command]
async fn list_messages(
    state: tauri::State<'_, state::AppState>,
    chat_id: String,
    limit: Option<u32>,
) -> Result<Vec<Message>, String> {
    state
        .core()
        .list_messages(&Jid::new(chat_id), limit.unwrap_or(200))
        .map_err(|error| error.to_string())
}

/// Pin or unpin a chat (synced to the account's other devices).
#[tauri::command]
async fn set_chat_pinned(
    state: tauri::State<'_, state::AppState>,
    chat_id: String,
    pinned: bool,
) -> Result<(), String> {
    state
        .core()
        .set_chat_pinned(&Jid::new(chat_id), pinned)
        .await
        .map_err(|error| error.to_string())
}

/// Mute or unmute a chat (synced to the account's other devices).
#[tauri::command]
async fn set_chat_muted(
    state: tauri::State<'_, state::AppState>,
    chat_id: String,
    muted: bool,
) -> Result<(), String> {
    state
        .core()
        .set_chat_muted(&Jid::new(chat_id), muted)
        .await
        .map_err(|error| error.to_string())
}

/// Archive or unarchive a chat (synced to the account's other devices).
#[tauri::command]
async fn set_chat_archived(
    state: tauri::State<'_, state::AppState>,
    chat_id: String,
    archived: bool,
) -> Result<(), String> {
    state
        .core()
        .set_chat_archived(&Jid::new(chat_id), archived)
        .await
        .map_err(|error| error.to_string())
}

/// Mark a chat as read.
#[tauri::command]
async fn mark_chat_read(
    state: tauri::State<'_, state::AppState>,
    chat_id: String,
) -> Result<(), String> {
    state
        .core()
        .mark_chat_read(&Jid::new(chat_id))
        .await
        .map_err(|error| error.to_string())
}

/// Send a typing/paused chat-state update.
#[tauri::command]
async fn set_typing(
    state: tauri::State<'_, state::AppState>,
    chat_id: String,
    typing: bool,
) -> Result<(), String> {
    state
        .core()
        .set_typing(&Jid::new(chat_id), typing)
        .await
        .map_err(|error| error.to_string())
}

/// Application entry point.
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rustwa=info,whatsapp_core=info".into()),
        )
        .try_init();

    let app = tauri::Builder::default()
        // Single-instance must run first: it probes for a running instance
        // before the rest of the plugins boot up.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![
            app_info,
            core_connect,
            core_logout,
            core_restart_pairing,
            core_reset_session,
            send_text,
            list_chats,
            list_messages,
            set_chat_pinned,
            set_chat_muted,
            set_chat_archived,
            mark_chat_read,
            set_typing,
            commands_contacts::contacts_resolve,
            commands_contacts::contacts_avatar,
            commands_actions::actions_send_quoting,
            commands_actions::actions_react,
            commands_actions::actions_edit,
            commands_actions::actions_revoke,
            commands_actions::actions_star,
            commands_media::media_download,
            commands_media::media_send_file,
            commands_groups::groups_create,
            commands_groups::groups_info,
            commands_groups::groups_add,
            commands_groups::groups_remove,
            commands_groups::groups_leave,
            commands_groups::groups_invite_link,
            commands_channels::channels_follow,
            commands_channels::channels_unfollow,
            commands_channels::channels_send,
            commands_channels::channels_post_status,
            commands_calls::calls_start,
            commands_calls::calls_end,
            platform::notify,
            platform::notify_for_chat,
            platform::notification_permission,
            platform::set_badge,
            platform::platform_capabilities,
            platform::autostart_enabled,
            platform::set_autostart,
            app_lock::set_app_lock,
            app_lock::app_lock_enabled,
            deep_link::open_link,
            deep_link::deep_link_ready
        ])
        .setup(|app| {
            // Persistent state lives under the app data directory:
            //   rustwa.db      – chats, messages, contacts (our schema)
            //   session/       – protocol session keys (upstream adapter)
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|error| format!("failed to resolve the app data directory: {error}"))?;
            let store = Arc::new(
                Store::open(&data_dir.join("rustwa.db"))
                    .map_err(|error| format!("failed to open the store: {error}"))?,
            );
            let core = Arc::new(WaClient::new(
                ClientConfig::new(data_dir.join("session")),
                store,
            ));
            app.manage(state::AppState::new(Arc::clone(&core)));

            // Bridge core events to the webview.
            events::spawn_event_forwarder(app.handle().clone(), core.subscribe());

            // Native menu bar. `Preferences…` emits `ui://open-settings`.
            app.set_menu(menu::build(app.handle())?)?;
            app.on_menu_event(menu::on_event);

            // Menu bar extra and `rustwa://` deep links.
            tray::setup(app.handle())?;
            deep_link::setup(app.handle());

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building RustWA");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            // Flush protocol state before the process goes away.
            let core = Arc::clone(app_handle.state::<state::AppState>().core());
            tauri::async_runtime::block_on(core.shutdown());
        }
    });
}
