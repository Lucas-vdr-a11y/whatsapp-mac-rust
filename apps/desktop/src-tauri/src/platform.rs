//! Desktop platform integration: notifications, dock badge and capability
//! reporting.
//!
//! These commands are thin adapters over Tauri APIs and plugins; the UI calls
//! them through `invoke` and adapts its affordances from
//! [`platform_capabilities`].

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::{NotificationExt, PermissionState};

/// Native features available in this build, so the UI can hide what the host
/// cannot do. Field names serialize to camelCase.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformCapabilities {
    /// Dock badge (macOS) / taskbar count (Linux).
    pub badge: bool,
    /// Desktop notifications through `tauri-plugin-notification`.
    pub notifications: bool,
    /// Tray icon. Always `false` for now: the tray is not implemented.
    pub tray: bool,
}

/// Reports which native integrations the UI can rely on.
#[tauri::command]
pub fn platform_capabilities() -> PlatformCapabilities {
    PlatformCapabilities {
        // Tauri forwards `set_badge_count` to `NSApplication.dockTile` on
        // macOS and to the Unity/Launcher API on Linux; Windows has no
        // equivalent API in Tauri.
        badge: cfg!(any(target_os = "macos", target_os = "linux")),
        // The notification plugin is compiled in on desktop and unused on
        // mobile, where this host does not run.
        notifications: true,
        tray: false,
    }
}

/// Shows a desktop notification.
#[tauri::command]
pub fn notify(app: AppHandle, title: String, body: String) -> Result<(), String> {
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(|error| error.to_string())
}

/// Returns whether notifications are allowed, requesting permission when the
/// OS has not decided yet.
///
/// On desktop the plugin reports `Granted` unconditionally: the OS may still
/// gate delivery, but there is no app-facing permission state to query.
#[tauri::command]
pub fn notification_permission(app: AppHandle) -> Result<bool, String> {
    let notification = app.notification();

    match notification
        .permission_state()
        .map_err(|error| error.to_string())?
    {
        PermissionState::Granted => Ok(true),
        PermissionState::Denied => Ok(false),
        PermissionState::Prompt | PermissionState::PromptWithRationale => {
            let state = notification
                .request_permission()
                .map_err(|error| error.to_string())?;
            Ok(matches!(state, PermissionState::Granted))
        }
    }
}

/// Sets the dock badge. `None`, `0` and negative counts clear it.
///
/// Tauri 2.11 has a first-class API for this: on macOS the wry runtime maps
/// [`tauri::WebviewWindow::set_badge_count`] to
/// `NSApplication.shared.dockTile.badgeLabel`, so no `objc2` bindings are
/// needed. The method is a no-op on Windows and dispatchable from any thread.
#[tauri::command]
pub fn set_badge(app: AppHandle, count: Option<i64>) -> Result<(), String> {
    let count = count.filter(|count| *count > 0);
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window is not available".to_owned())?;

    window
        .set_badge_count(count)
        .map_err(|error| error.to_string())
}
