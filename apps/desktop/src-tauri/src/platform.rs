//! Desktop platform integration: notifications, dock badge, login item and
//! capability reporting.
//!
//! These commands are thin adapters over Tauri APIs and plugins; the UI calls
//! them through `invoke` and adapts its affordances from
//! [`platform_capabilities`]. The menu bar extra lives in [`crate::tray`],
//! deep links in [`crate::deep_link`] and the app-lock preference in
//! [`crate::app_lock`].

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;
use tauri_plugin_notification::{NotificationExt, PermissionState};

/// Native features available in this build, so the UI can hide what the host
/// cannot do. Field names serialize to camelCase.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformCapabilities {
    /// Dock badge (macOS) / taskbar count (Linux).
    pub badge: bool,
    /// Desktop notifications, with click-through on macOS
    /// (`UNUserNotificationCenter`), through `tauri-plugin-notification`
    /// elsewhere.
    pub notifications: bool,
    /// Menu bar extra (`TrayIcon`); installed by [`crate::tray::setup`].
    pub tray: bool,
    /// Login item toggling through `tauri-plugin-autostart`.
    pub autostart: bool,
    /// `rustwa://` links and the `wa.me` parser in [`crate::deep_link`].
    pub deep_links: bool,
    /// App-lock *preference storage*. The biometric unlock prompt is not
    /// implemented yet, see [`crate::app_lock`].
    pub app_lock: bool,
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
        // `tauri/tray-icon` is enabled in Cargo.toml and the icon is always
        // installed during setup; Linux needs a status area host, which the
        // UI cannot detect from here.
        tray: true,
        // `tauri-plugin-autostart` is compiled for every desktop platform.
        autostart: true,
        // The deep-link plugin and `rustwa://` scheme are always registered.
        deep_links: true,
        // Storage exists everywhere; unlocking does not exist yet.
        app_lock: true,
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

/// Shows a desktop notification for a chat and tags it with the chat id.
///
/// On macOS this posts through `UNUserNotificationCenter` (see
/// [`crate::notification_center`]) with the chat id in the notification's
/// `userInfo`; clicking the notification focuses the main window and opens the
/// chat through the same `ui://open-chat` event a `rustwa://` deep link uses.
/// Outside macOS, or in an unbundled dev build, it falls back to
/// `tauri-plugin-notification`, whose desktop backend drops the extra: that
/// path delivers the banner but cannot report clicks.
#[tauri::command]
pub fn notify_for_chat(
    app: AppHandle,
    chat_id: String,
    title: String,
    body: String,
) -> Result<(), String> {
    crate::notification_center::notify_chat(&app, &chat_id, &title, &body)
}

/// Returns whether RustWA is registered as a login item.
///
/// Errors degrade to `false` (the UI should show the toggle as off and let the
/// user retry), mirroring [`crate::app_lock::app_lock_enabled`].
#[tauri::command]
pub fn autostart_enabled(app: AppHandle) -> bool {
    match app.autolaunch().is_enabled() {
        Ok(enabled) => enabled,
        Err(error) => {
            tracing::warn!(%error, "failed to query the login item state");
            false
        }
    }
}

/// Enables or disables launching RustWA at login.
///
/// On macOS this writes a LaunchAgent (`MacosLauncher::LaunchAgent` passed at
/// plugin init), which needs no user consent prompt; the entry shows up under
/// System Settings → General → Login Items.
#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    let manager = app.autolaunch();
    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    result.map_err(|error| error.to_string())
}

/// Returns whether notifications are allowed, requesting permission when the
/// OS has not decided yet.
///
/// On macOS with the native backend this reflects (and prompts for) the real
/// `UNUserNotificationCenter` authorization state. The plugin's desktop
/// backend reports `Granted` unconditionally, so it is only the fallback for
/// other platforms and unbundled builds.
#[tauri::command]
pub async fn notification_permission(app: AppHandle) -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        if crate::notification_center::native_delivery() {
            return crate::notification_center::request_permission().await;
        }
    }

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
