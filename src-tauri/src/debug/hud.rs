//! The floating debug HUD: one owned window per browser window, so it stays above the page
//! views and is minimized and closed with them. It is not a popup. It keeps its place when it
//! loses focus and when the browser window moves or resizes.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, MutexGuard};

use tauri::window::WindowBuilder;
use tauri::{AppHandle, Manager, PhysicalPosition, WebviewUrl, Window, WindowEvent};

use crate::{hang_guard, paths, tabs, webview2};

const WIDTH: f64 = 560.0;
const HEIGHT: f64 = 620.0;
/// Logical px. The collapsed HUD is only its header.
const MIN_WIDTH: f64 = 320.0;
const MIN_HEIGHT: f64 = 40.0;
/// Logical px from the browser window's top-right corner where a new HUD opens.
const RIGHT_INSET: f64 = 16.0;
const TOP_INSET: f64 = 96.0;

#[derive(Default)]
struct Place {
    /// From the browser window's top-right corner to the HUD's top-left corner, physical px.
    offset: Option<(i32, i32)>,
    /// Where `place` last put the HUD, so that move is not taken for a drag.
    placed_at: Option<(i32, i32)>,
    visible: bool,
}

/// One entry per HUD that exists, keyed by its label.
static PLACES: LazyLock<Mutex<HashMap<String, Place>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn places() -> MutexGuard<'static, HashMap<String, Place>> {
    PLACES.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn label(window: &str) -> String {
    format!("debug-{window}")
}

fn is_hud(label: &str) -> bool {
    label.starts_with("debug-") && !label.starts_with("debug-outline-")
}

/// Every HUD, shown or hidden, so a hidden HUD has the whole feed when it comes back.
pub fn labels(app: &AppHandle) -> Vec<String> {
    let labels: Vec<String> = places().keys().cloned().collect();
    labels
        .into_iter()
        .filter(|label| app.get_window(label).is_some())
        .collect()
}

/// Native handles of the HUDs on screen now.
fn shown_hwnds(app: &AppHandle) -> Vec<isize> {
    let shown: Vec<String> = places()
        .iter()
        .filter(|(_, place)| place.visible)
        .map(|(label, _)| label.clone())
        .collect();
    shown
        .iter()
        .filter_map(|label| app.get_window(label))
        .filter_map(|hud| hud.hwnd().ok().map(|hwnd| hwnd.0 as isize))
        .collect()
}

/// Focus in the HUD does not count as leaving a popup.
pub fn is_hud_hwnd(app: &AppHandle, hwnd: isize) -> bool {
    hwnd != 0 && shown_hwnds(app).contains(&hwnd)
}

/// A click in the HUD does not count as a click outside a popup. Screen px.
pub fn contains_point(app: &AppHandle, x: i32, y: i32) -> bool {
    shown_hwnds(app)
        .into_iter()
        .any(|hwnd| webview2::client_contains_screen_point(hwnd, x, y))
}

/// Shows or hides the HUD of `window`. Resolves to whether it is now shown.
pub fn toggle(app: &AppHandle, window: &str) -> Result<bool, String> {
    if hang_guard::on_main_thread() {
        return Err("The debug HUD is built off the main thread.".into());
    }
    let label = label(window);
    let Some(hud) = app.get_window(&label) else {
        create(app, window, &label)?;
        return Ok(true);
    };
    if hud.is_visible().unwrap_or(false) {
        set_visible(&label, false);
        super::outline::clear(app);
        hud.hide().map_err(|error| error.to_string())?;
        return Ok(false);
    }
    set_visible(&label, true);
    place(app, window, &hud);
    hud.show().map_err(|error| error.to_string())?;
    let _ = hud.set_focus();
    Ok(true)
}

fn set_visible(label: &str, visible: bool) {
    places().entry(label.to_string()).or_default().visible = visible;
}

fn create(app: &AppHandle, window: &str, label: &str) -> Result<(), String> {
    let owner = app
        .get_window(window)
        .ok_or("The browser window is missing.")?;
    let surface = tabs::popup_surface_color(app);
    let hud = WindowBuilder::new(app, label)
        .title("ce-browser debug")
        .decorations(false)
        .shadow(true)
        .skip_taskbar(true)
        .resizable(true)
        .min_inner_size(MIN_WIDTH, MIN_HEIGHT)
        .focused(false)
        .visible(false)
        .inner_size(WIDTH, HEIGHT)
        .parent(&owner)
        .map_err(|error| error.to_string())?
        .build()
        .map_err(|error| error.to_string())?;
    let _ = hud.set_background_color(Some(surface));
    let url = paths::internal_url(app, "debug-hud");
    let builder = tabs::base_builder(app, label, WebviewUrl::External(url))
        .auto_resize()
        .disable_drag_drop_handler()
        .background_color(surface);
    let size = hud.inner_size().map_err(|error| error.to_string())?;
    let webview = hud
        .add_child(builder, PhysicalPosition::new(0, 0), size)
        .map_err(|error| error.to_string())?;
    webview2::attach_debug_hud_blur(app, &webview, window);
    set_visible(label, true);
    place(app, window, &hud);
    hud.show().map_err(|error| error.to_string())?;
    let _ = hud.set_focus();
    Ok(())
}

/// Top-right corner of a browser window, physical px.
fn corner_of(owner: &Window) -> Option<(i32, i32)> {
    let origin = owner.outer_position().ok()?;
    let size = owner.outer_size().ok()?;
    Some((origin.x + size.width as i32, origin.y))
}

fn place(app: &AppHandle, window: &str, hud: &Window) {
    let Some(owner) = app.get_window(window) else {
        return;
    };
    let Some(corner) = corner_of(&owner) else {
        return;
    };
    let scale = owner.scale_factor().unwrap_or(1.0);
    let width = hud
        .outer_size()
        .map(|size| size.width as i32)
        .unwrap_or((WIDTH * scale).round() as i32);
    let at = {
        let mut places = places();
        let place = places.entry(hud.label().to_string()).or_default();
        let offset = *place.offset.get_or_insert((
            -(width + (RIGHT_INSET * scale).round() as i32),
            (TOP_INSET * scale).round() as i32,
        ));
        let at = (corner.0 + offset.0, corner.1 + offset.1);
        place.placed_at = Some(at);
        at
    };
    let _ = hud.set_position(PhysicalPosition::new(at.0, at.1));
}

/// Keeps a shown HUD at the same spot beside its browser window. Safe on the main thread.
pub fn follow(app: &AppHandle, window: &str) {
    let label = label(window);
    if !places().get(&label).is_some_and(|place| place.visible) {
        return;
    }
    let app = app.clone();
    let window = window.to_string();
    tauri::async_runtime::spawn(async move {
        if let Some(hud) = app.get_window(&label) {
            place(&app, &window, &hud);
        }
    });
}

pub fn window_event(window: &Window, event: &WindowEvent) {
    let label = window.label();
    if !is_hud(label) {
        return;
    }
    match event {
        WindowEvent::Moved(position) => {
            let at = (position.x, position.y);
            if places().get(label).and_then(|place| place.placed_at) == Some(at) {
                return;
            }
            let owner = super::owner_of(label);
            let Some(corner) = window
                .app_handle()
                .get_window(&owner)
                .as_ref()
                .and_then(corner_of)
            else {
                return;
            };
            if let Some(place) = places().get_mut(label) {
                place.offset = Some((at.0 - corner.0, at.1 - corner.1));
                place.placed_at = Some(at);
            }
        }
        WindowEvent::Destroyed => {
            places().remove(label);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{is_hud, label};

    #[test]
    fn the_hud_label_names_its_browser_window() {
        assert_eq!(label("main"), "debug-main");
        assert_eq!(label("main-2"), "debug-main-2");
        assert!(is_hud("debug-main"));
        assert!(!is_hud("debug-outline-main"));
        assert!(!is_hud("popup-main-0"));
    }
}
