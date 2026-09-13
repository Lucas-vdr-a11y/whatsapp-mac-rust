//! Tauri host for RustWA.
//!
//! This crate is intentionally thin: it owns the window, menus and native
//! integrations and marshals commands / events between the webview and
//! `whatsapp-core`. All business logic lives in the core crate.

mod events;
mod menu;
mod platform;
mod state;

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
        .invoke_handler(tauri::generate_handler![
            app_info,
            core_connect,
            core_logout,
            send_text,
            list_chats,
            list_messages,
            set_chat_pinned,
            set_chat_muted,
            set_chat_archived,
            mark_chat_read,
            set_typing,
            platform::notify,
            platform::notification_permission,
            platform::set_badge,
            platform::platform_capabilities
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
