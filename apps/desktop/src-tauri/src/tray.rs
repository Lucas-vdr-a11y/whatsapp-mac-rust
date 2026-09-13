//! Menu bar extra (system tray icon).
//!
//! The icon is generated in code as a speech bubble. On macOS it is flagged as
//! a [template image]: AppKit ignores the RGB channels and draws the alpha
//! channel in the menu bar's own tint, so the same asset works in light and
//! dark mode without a second asset. No PNG is read from disk and no new file
//! is added to `icons/`.
//!
//! Interaction:
//!
//! * **Left click** shows, unminimizes and focuses the main window. The menu is
//!   deliberately *not* shown on left click (`show_menu_on_left_click(false)`);
//!   otherwise every activation would be covered by the menu.
//! * **Right click** opens the tray menu (Tauri/muda default for the secondary
//!   button).
//! * **Menu items** are handled by [`crate::menu::on_event`], the same router
//!   as the regular app menu. Tauri dispatches menu events from every menu to
//!   the app-level listener, so this module does not register a second handler.
//!
//! [template image]: https://developer.apple.com/documentation/appkit/nsimage/1520017-template

use tauri::{
    AppHandle, Runtime,
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use crate::menu;

/// Unique id of the RustWA menu bar extra.
pub const TRAY_ID: &str = "rustwa.tray";

/// Side of the generated icon in pixels. The macOS status bar draws the icon
/// at 18 pt tall; 44 px keeps it crisp on Retina displays (2× and 2.5×).
const ICON_SIZE: u32 = 44;

/// Number of samples per pixel and axis used when rasterizing the icon. Larger
/// values produce smoother corners at a negligible startup cost.
const ICON_SUBSAMPLES: u32 = 3;

/// Installs the menu bar extra and its menu.
///
/// Must run on the main thread; [`TrayIconBuilder::build`] dispatches there
/// internally, so calling this from the Tauri `setup` hook is fine.
pub fn setup<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, menu::TRAY_OPEN_ID, "Open RustWA", true, None::<&str>)?;
    let new_chat = MenuItem::with_id(app, menu::NEW_CHAT_ID, "New chat", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, menu::PREFERENCES_ID, "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, menu::QUIT_ID, "Quit RustWA", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &open,
            &new_chat,
            &settings,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(template_icon())
        .icon_as_template(cfg!(target_os = "macos"))
        .menu(&menu)
        .tooltip("RustWA")
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                menu::bring_all_to_front(tray.app_handle());
            }
        })
        .build(app)?;

    tracing::debug!("menu bar extra installed");
    Ok(())
}

/// Rasterizes the speech-bubble glyph into an RGBA image.
///
/// The shape lives entirely in the alpha channel; RGB is opaque white so the
/// same bitmap also renders on platforms without template images.
fn template_icon() -> Image<'static> {
    let samples_per_pixel = ICON_SUBSAMPLES * ICON_SUBSAMPLES;
    let sample_step = 1.0 / ICON_SUBSAMPLES as f32;
    let mut rgba = vec![0_u8; (ICON_SIZE * ICON_SIZE * 4) as usize];

    for y in 0..ICON_SIZE {
        for x in 0..ICON_SIZE {
            let mut hits = 0_u32;
            for sample_y in 0..ICON_SUBSAMPLES {
                for sample_x in 0..ICON_SUBSAMPLES {
                    let px = x as f32 + (sample_x as f32 + 0.5) * sample_step;
                    let py = y as f32 + (sample_y as f32 + 0.5) * sample_step;
                    if inside_bubble(px, py) {
                        hits += 1;
                    }
                }
            }

            let alpha = (hits * 255 / samples_per_pixel) as u8;
            let offset = ((y * ICON_SIZE + x) * 4) as usize;
            rgba[offset..offset + 4].copy_from_slice(&[255, 255, 255, alpha]);
        }
    }

    Image::new_owned(rgba, ICON_SIZE, ICON_SIZE)
}

/// Whether `(x, y)` falls inside the speech bubble shape.
fn inside_bubble(x: f32, y: f32) -> bool {
    // Bubble body: a rounded rectangle spanning most of the canvas.
    const LEFT: f32 = 6.0;
    const TOP: f32 = 7.0;
    const RIGHT: f32 = 38.0;
    const BOTTOM: f32 = 31.0;
    const RADIUS: f32 = 7.0;

    let corner_x = x.clamp(LEFT + RADIUS, RIGHT - RADIUS);
    let corner_y = y.clamp(TOP + RADIUS, BOTTOM - RADIUS);
    let dx = x - corner_x;
    let dy = y - corner_y;
    if dx * dx + dy * dy <= RADIUS * RADIUS {
        return true;
    }

    // Tail: a triangle sweeping down from the bottom-left edge.
    triangle(x, y, (13.0, 27.0), (24.0, 30.0), (13.0, 41.0))
}

/// Point-in-triangle test using the sign of the three edge cross products.
fn triangle(x: f32, y: f32, a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> bool {
    let edges = [edge(x, y, a, b), edge(x, y, b, c), edge(x, y, c, a)];
    edges.iter().all(|d| *d <= 0.0) || edges.iter().all(|d| *d >= 0.0)
}

/// Cross product of (`point - a`) and (`b - a`); sign tells which side of the
/// directed edge `a → b` the point lies on.
fn edge(px: f32, py: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    (px - a.0) * (b.1 - a.1) - (py - a.1) * (b.0 - a.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_has_bubble_and_transparent_corners() {
        let icon = template_icon();
        assert_eq!(icon.width(), ICON_SIZE);
        assert_eq!(icon.height(), ICON_SIZE);
        assert_eq!(icon.rgba().len(), (ICON_SIZE * ICON_SIZE * 4) as usize);

        let alpha = |x: u32, y: u32| icon.rgba()[((y * ICON_SIZE + x) * 4 + 3) as usize];
        // Centre of the bubble body is opaque, the top-left corner is empty.
        assert_eq!(alpha(ICON_SIZE / 2, ICON_SIZE / 2), 255);
        assert_eq!(alpha(0, 0), 0);
    }
}
