//! Native application menu (the menu bar on macOS).
//!
//! The layout follows the conventional macOS structure: RustWA, Edit, View,
//! Window and Help. System entries (about, services, hide, clipboard,
//! undo/redo, minimize, zoom, fullscreen) are [`PredefinedMenuItem`]s, so
//! AppKit wires up the expected behaviour and the standard shortcuts. Every
//! RustWA-specific item is routed through [`on_event`].
//!
//! `Preferences…` does not open a window itself: it emits the
//! [`OPEN_SETTINGS_EVENT`] (`ui://open-settings`) Tauri event and the React UI
//! decides what to present.

use std::sync::atomic::{AtomicU32, Ordering};

use tauri::{
    AppHandle, Emitter, Manager, Runtime,
    menu::{
        AboutMetadata, HELP_SUBMENU_ID, IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem,
        Submenu, WINDOW_SUBMENU_ID,
    },
};

/// Event emitted when the user chooses "Preferences…" in the app menu.
///
/// Payload: `()`. The UI opens its settings surface on receipt.
pub const OPEN_SETTINGS_EVENT: &str = "ui://open-settings";

const PREFERENCES_ID: &str = "rustwa.menu.preferences";
const RELOAD_ID: &str = "rustwa.menu.view.reload";
const FORCE_RELOAD_ID: &str = "rustwa.menu.view.force-reload";
#[cfg(debug_assertions)]
const DEVTOOLS_ID: &str = "rustwa.menu.view.devtools";
const ACTUAL_SIZE_ID: &str = "rustwa.menu.view.actual-size";
const ZOOM_IN_ID: &str = "rustwa.menu.view.zoom-in";
const ZOOM_OUT_ID: &str = "rustwa.menu.view.zoom-out";
const BRING_ALL_TO_FRONT_ID: &str = "rustwa.menu.window.bring-all-to-front";
const WEBSITE_ID: &str = "rustwa.menu.help.website";

const WEBSITE_URL: &str = "https://github.com/Lucas-vdr-a11y/whatsapp-mac-rust";

/// Zoom bounds and step, in percent, for the View menu.
const ZOOM_DEFAULT_PERCENT: u32 = 100;
const ZOOM_MIN_PERCENT: u32 = 50;
const ZOOM_MAX_PERCENT: u32 = 300;
const ZOOM_STEP_PERCENT: u32 = 10;

/// Current webview zoom in percent.
///
/// Tauri exposes `set_zoom` but no getter, so the host tracks the value. Menu
/// events arrive on the main thread; the atomic keeps the bookkeeping
/// lock-free and future command handlers safe.
static ZOOM_PERCENT: AtomicU32 = AtomicU32::new(ZOOM_DEFAULT_PERCENT);

/// Builds the full menu bar.
pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let about = PredefinedMenuItem::about(
        app,
        Some("About RustWA"),
        Some(AboutMetadata {
            name: Some("RustWA".into()),
            version: Some(env!("CARGO_PKG_VERSION").into()),
            ..Default::default()
        }),
    )?;

    let app_menu = Submenu::with_items(
        app,
        "RustWA",
        true,
        &[
            &about,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(
                app,
                PREFERENCES_ID,
                "Preferences…",
                true,
                Some("CmdOrCtrl+,"),
            )?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::services(app, Some("Services"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, Some("Hide RustWA"))?,
            &PredefinedMenuItem::hide_others(app, Some("Hide Others"))?,
            &PredefinedMenuItem::show_all(app, Some("Show All"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, Some("Quit RustWA"))?,
        ],
    )?;

    let edit_menu = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, Some("Undo"))?,
            &PredefinedMenuItem::redo(app, Some("Redo"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, Some("Cut"))?,
            &PredefinedMenuItem::copy(app, Some("Copy"))?,
            &PredefinedMenuItem::paste(app, Some("Paste"))?,
            &PredefinedMenuItem::select_all(app, Some("Select All"))?,
        ],
    )?;

    let reload = MenuItem::with_id(app, RELOAD_ID, "Reload", true, Some("CmdOrCtrl+R"))?;
    let force_reload = MenuItem::with_id(
        app,
        FORCE_RELOAD_ID,
        "Force Reload",
        true,
        Some("CmdOrCtrl+Shift+R"),
    )?;
    #[cfg(debug_assertions)]
    let devtools = MenuItem::with_id(
        app,
        DEVTOOLS_ID,
        "Toggle Developer Tools",
        true,
        Some("Alt+CmdOrCtrl+I"),
    )?;
    let view_separator_1 = PredefinedMenuItem::separator(app)?;
    let actual_size = MenuItem::with_id(
        app,
        ACTUAL_SIZE_ID,
        "Actual Size",
        true,
        Some("CmdOrCtrl+0"),
    )?;
    let zoom_in = MenuItem::with_id(app, ZOOM_IN_ID, "Zoom In", true, Some("CmdOrCtrl+="))?;
    let zoom_out = MenuItem::with_id(app, ZOOM_OUT_ID, "Zoom Out", true, Some("CmdOrCtrl+-"))?;
    let view_separator_2 = PredefinedMenuItem::separator(app)?;
    let fullscreen = PredefinedMenuItem::fullscreen(app, Some("Enter Full Screen"))?;

    let mut view_items: Vec<&dyn IsMenuItem<R>> = vec![&reload, &force_reload];
    #[cfg(debug_assertions)]
    view_items.push(&devtools);
    view_items.push(&view_separator_1);
    view_items.push(&actual_size);
    view_items.push(&zoom_in);
    view_items.push(&zoom_out);
    view_items.push(&view_separator_2);
    view_items.push(&fullscreen);
    let view_menu = Submenu::with_items(app, "View", true, &view_items)?;

    let minimize = PredefinedMenuItem::minimize(app, Some("Minimize"))?;
    let zoom = PredefinedMenuItem::maximize(app, Some("Zoom"))?;
    let window_separator = PredefinedMenuItem::separator(app)?;
    let bring_all_to_front = MenuItem::with_id(
        app,
        BRING_ALL_TO_FRONT_ID,
        "Bring All to Front",
        true,
        None::<&str>,
    )?;
    // Using Tauri's window-menu id lets muda register the submenu with
    // `NSApplication.setWindowsMenu:`, which macOS uses for the window list.
    let window_menu = Submenu::with_id_and_items(
        app,
        WINDOW_SUBMENU_ID,
        "Window",
        true,
        &[&minimize, &zoom, &window_separator, &bring_all_to_front],
    )?;

    let website = MenuItem::with_id(app, WEBSITE_ID, "RustWA website", true, None::<&str>)?;
    let help_menu = Submenu::with_id_and_items(app, HELP_SUBMENU_ID, "Help", true, &[&website])?;

    Menu::with_items(
        app,
        &[&app_menu, &edit_menu, &view_menu, &window_menu, &help_menu],
    )
}

/// Routes a menu item activation to its handler.
pub fn on_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    match event.id().as_ref() {
        PREFERENCES_ID => {
            if let Err(error) = app.emit(OPEN_SETTINGS_EVENT, ()) {
                tracing::warn!(%error, "failed to emit the open-settings event");
            }
        }
        RELOAD_ID | FORCE_RELOAD_ID => reload(app),
        #[cfg(debug_assertions)]
        DEVTOOLS_ID => {
            if let Some(window) = app.get_webview_window("main") {
                window.open_devtools();
            }
        }
        ACTUAL_SIZE_ID => apply_zoom(app, ZOOM_DEFAULT_PERCENT),
        ZOOM_IN_ID => apply_zoom(app, current_zoom().saturating_add(ZOOM_STEP_PERCENT)),
        ZOOM_OUT_ID => apply_zoom(app, current_zoom().saturating_sub(ZOOM_STEP_PERCENT)),
        BRING_ALL_TO_FRONT_ID => bring_all_to_front(app),
        WEBSITE_ID => open_website(),
        other => tracing::debug!(id = other, "unhandled menu event"),
    }
}

/// Reloads the webview.
///
/// `Force Reload` shares this path: wry only exposes `WebView.reload`, not
/// `WKWebView.reloadFromOrigin`. It still reloads the app, but a cache-bypassing
/// reload is not available without AppKit access.
fn reload<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main")
        && let Err(error) = window.reload()
    {
        tracing::warn!(%error, "failed to reload the main window");
    }
}

fn current_zoom() -> u32 {
    ZOOM_PERCENT.load(Ordering::Relaxed)
}

fn apply_zoom<R: Runtime>(app: &AppHandle<R>, percent: u32) {
    let percent = percent.clamp(ZOOM_MIN_PERCENT, ZOOM_MAX_PERCENT);
    ZOOM_PERCENT.store(percent, Ordering::Relaxed);

    if let Some(window) = app.get_webview_window("main")
        && let Err(error) = window.set_zoom(f64::from(percent) / 100.0)
    {
        tracing::warn!(%error, percent, "failed to set webview zoom");
    }
}

/// Focuses the app's windows.
///
/// AppKit's actual "Bring All to Front" (`NSApplication.arrangeInFront:`) is
/// not exposed by Tauri. Showing and unminimizing each window and focusing the
/// main one is the equivalent user-visible behaviour for this single-window app.
fn bring_all_to_front<R: Runtime>(app: &AppHandle<R>) {
    for window in app.webview_windows().values() {
        let _ = window.show();
        let _ = window.unminimize();
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_focus();
    }
}

/// Opens the project's GitHub page in the default browser.
fn open_website() {
    match open_url(WEBSITE_URL) {
        Ok(()) => {}
        Err(error) => tracing::warn!(%error, url = WEBSITE_URL, "failed to open the website"),
    }
}

#[cfg(target_os = "macos")]
fn open_url(url: &str) -> std::io::Result<()> {
    std::process::Command::new("open")
        .arg(url)
        .spawn()
        .map(|_| ())
}

#[cfg(target_os = "linux")]
fn open_url(url: &str) -> std::io::Result<()> {
    std::process::Command::new("xdg-open")
        .arg(url)
        .spawn()
        .map(|_| ())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn open_url(_url: &str) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "no URL opener configured for this platform",
    ))
}
