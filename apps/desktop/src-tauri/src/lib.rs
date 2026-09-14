//! Tauri host for RustWA.
//!
//! This crate is intentionally thin: it owns the window, menus and native
//! integrations and marshals commands / events between the webview and
//! `whatsapp-core`. All business logic lives in the core crate.

mod app_lock;
mod commands_actions;
mod commands_business;
mod commands_call_log;
mod commands_calls;
mod commands_channels;
mod commands_chat_ops;
mod commands_communities;
mod commands_contacts;
mod commands_groups;
mod commands_media;
mod commands_privacy;
mod commands_profile;
mod commands_statuses;
mod deep_link;
mod events;
mod file_open;
mod menu;
mod notification_center;
mod platform;
mod security;
mod state;
mod tray;
mod windows;

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

/// Current connection state, so a UI reload can pick up a live session
/// without waiting for the next lifecycle event.
#[tauri::command]
fn core_connection_state(state: tauri::State<'_, state::AppState>) -> String {
    use whatsapp_core::events::ConnectionState;
    match state.core().connection_state() {
        ConnectionState::Connected => "connected",
        ConnectionState::Connecting => "connecting",
        ConnectionState::Disconnected => "disconnected",
    }
    .to_string()
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

/// One page of messages older than `before_id`, oldest first.
///
/// Powers scrolling up in a conversation without holding the whole history in
/// memory.
#[tauri::command]
async fn list_messages_before(
    state: tauri::State<'_, state::AppState>,
    chat_id: String,
    before_id: String,
    limit: Option<u32>,
) -> Result<Vec<Message>, String> {
    state
        .core()
        .store()
        .messages_before(&Jid::new(chat_id), &before_id, limit.unwrap_or(100))
        .map_err(|error| error.to_string())
}

/// Requests older messages for a chat from the primary phone (on-demand
/// history sync). The reply lands in the store and the UI refreshes.
#[tauri::command]
async fn fetch_older_history(
    state: tauri::State<'_, state::AppState>,
    chat_id: String,
    count: Option<u32>,
) -> Result<bool, String> {
    state
        .core()
        .fetch_older_history(&Jid::new(chat_id), count.unwrap_or(100))
        .await
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

/// Keep the main window resizable and above the configured minimum after
/// `tauri-plugin-window-state` restores a previous frame. Overlay titlebars
/// plus a saved-too-small size is what made the window look "stuck".
fn enforce_main_window(window: &tauri::WebviewWindow) {
    let _ = window.set_resizable(true);
    let _ = window.set_maximizable(true);
    let min = tauri::LogicalSize::new(800.0, 560.0);
    let _ = window.set_min_size(Some(min));
    if let (Ok(physical), Ok(scale)) = (window.inner_size(), window.scale_factor()) {
        let width = f64::from(physical.width) / scale;
        let height = f64::from(physical.height) / scale;
        if width + 0.5 < 800.0 || height + 0.5 < 560.0 {
            let _ = window.set_size(tauri::LogicalSize::new(
                width.max(1100.0),
                height.max(760.0),
            ));
        }
    }
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
        // Per-chat windows are ephemeral: do not persist or restore them
        // across launches (`chat-*` labels), only the main window.
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_denylist(&["chat-*"])
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        // JID → window-label registry for the per-chat windows.
        .manage(windows::ChatWindows::default())
        .invoke_handler(tauri::generate_handler![
            app_info,
            core_connect,
            core_logout,
            core_restart_pairing,
            core_reset_session,
            core_connection_state,
            send_text,
            list_chats,
            list_messages,
            list_messages_before,
            fetch_older_history,
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
            commands_privacy::privacy_block,
            commands_privacy::privacy_unblock,
            commands_privacy::privacy_get,
            commands_privacy::privacy_set,
            commands_privacy::privacy_set_disappearing_default,
            commands_profile::profile_get,
            commands_profile::profile_set_name,
            commands_profile::profile_set_about,
            commands_profile::profile_set_picture,
            commands_profile::profile_remove_picture,
            commands_business::business_profile,
            commands_business::labels_list,
            commands_business::labels_add,
            commands_business::labels_remove,
            commands_business::catalog_fetch,
            commands_business::username_lookup,
            commands_chat_ops::chat_delete,
            commands_chat_ops::chat_clear,
            commands_chat_ops::chat_set_disappearing,
            commands_chat_ops::chat_send_mentions,
            commands_chat_ops::message_forward,
            commands_chat_ops::message_pin,
            commands_chat_ops::message_unpin,
            commands_chat_ops::poll_create,
            commands_chat_ops::poll_vote,
            commands_chat_ops::event_create,
            commands_chat_ops::list_starred,
            commands_chat_ops::search_messages,
            commands_groups::groups_create,
            commands_groups::groups_info,
            commands_groups::groups_add,
            commands_groups::groups_remove,
            commands_groups::groups_leave,
            commands_groups::groups_invite_link,
            commands_groups::groups_reset_invite_link,
            commands_groups::groups_set_subject,
            commands_groups::groups_set_description,
            commands_groups::groups_pending_participants,
            commands_groups::groups_approve_participants,
            commands_groups::groups_reject_participants,
            commands_communities::communities_create,
            commands_communities::communities_info,
            commands_communities::communities_link_group,
            commands_communities::communities_unlink_group,
            commands_communities::communities_invite_link,
            commands_communities::communities_join,
            commands_communities::communities_deactivate,
            commands_channels::channels_follow,
            commands_channels::channels_unfollow,
            commands_channels::channels_send,
            commands_channels::channels_post_status,
            commands_statuses::statuses_list,
            commands_statuses::status_viewed,
            commands_calls::calls_start,
            commands_calls::calls_end,
            commands_calls::calls_answer,
            commands_calls::calls_reject,
            commands_calls::calls_mute,
            commands_call_log::call_log_list,
            platform::notify,
            platform::notify_for_chat,
            platform::notification_permission,
            platform::set_badge,
            platform::platform_capabilities,
            platform::autostart_enabled,
            platform::set_autostart,
            app_lock::set_app_lock,
            app_lock::app_lock_enabled,
            app_lock::set_app_lock_timeout,
            app_lock::app_lock_timeout,
            security::security_biometry_available,
            security::security_authenticate,
            security::security_unlock,
            deep_link::open_link,
            deep_link::deep_link_ready,
            file_open::file_open_ready,
            windows::open_chat_window,
            windows::close_chat_window
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
            let mut config = ClientConfig::new(data_dir.join("session"));
            // Real CoreAudio capture/playback for calls; without it the manager
            // reports `NotConnected`. Video stays rejected by the backend.
            config.call_media = Some(Arc::new(whatsapp_core::CoreAudioFactory::new()));
            let core = Arc::new(WaClient::new(config, store));
            app.manage(state::AppState::new(Arc::clone(&core)));

            // Bridge core events to the webview.
            events::spawn_event_forwarder(app.handle().clone(), core.subscribe());

            // Native menu bar. `Preferences…` emits `ui://open-settings`.
            app.set_menu(menu::build(app.handle())?)?;
            app.on_menu_event(menu::on_event);

            // Menu bar extra and `rustwa://` deep links.
            tray::setup(app.handle())?;
            deep_link::setup(app.handle());
            // macOS notification click-through: the delegate is set once,
            // before the first chat notification can be posted.
            notification_center::install(app.handle());
            // Finder "Open With" / share-sheet file deliveries.
            file_open::setup(app.handle());

            // App lock (M8): track main-window focus so a refocus after the
            // configured timeout emits `ui://lock` and covers the UI again.
            app.manage(security::FocusClock::new());
            if let Some(window) = app.get_webview_window("main") {
                enforce_main_window(&window);
                let delayed = window.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
                    enforce_main_window(&delayed);
                });
                let handle = app.handle().clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(focused) = event {
                        security::handle_focus_change(&handle, *focused);
                    }
                });
                let _ = window.set_focus();
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building RustWA");

    app.run(|app_handle, event| match event {
        tauri::RunEvent::Exit => {
            // Flush protocol state before the process goes away.
            let core = Arc::clone(app_handle.state::<state::AppState>().core());
            tauri::async_runtime::block_on(core.shutdown());
        }
        // macOS delivers Finder "Open With" / Dock-drop / share-sheet files
        // through the same `Opened` event that carries `rustwa://` URLs.
        // `dispatch` only takes the `file://` ones.
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
        tauri::RunEvent::Opened { urls } => {
            file_open::dispatch(app_handle, &urls);
        }
        _ => {}
    });
}
