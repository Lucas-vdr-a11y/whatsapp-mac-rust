//! Tauri host for RustWA.
//!
//! This crate is intentionally thin: it owns the window and marshals commands /
//! events between the webview and `whatsapp-core`. All business logic lives in
//! the core crate.

use serde::Serialize;
use tauri::Manager;

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

/// Starts the connection/pairing flow.
///
/// TODO(M1): delegate to `whatsapp_core::Client`.
#[tauri::command]
async fn core_connect() -> Result<(), String> {
    Err("protocol core is not wired up yet (milestone M1)".to_owned())
}

/// Logs out and clears the local session.
///
/// TODO(M1): delegate to `whatsapp_core::Client`.
#[tauri::command]
async fn core_logout() -> Result<(), String> {
    Err("protocol core is not wired up yet (milestone M1)".to_owned())
}

/// Sends a text message to a chat.
///
/// TODO(M1): delegate to `whatsapp_core::Client`.
#[tauri::command]
async fn send_text(_chat_id: String, _text: String) -> Result<(), String> {
    Err("protocol core is not wired up yet (milestone M1)".to_owned())
}

/// Application entry point.
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rustwa=info,whatsapp_core=info".into()),
        )
        .try_init();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            app_info,
            core_connect,
            core_logout,
            send_text
        ])
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running RustWA");
}
