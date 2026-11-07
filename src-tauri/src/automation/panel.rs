//! The agent panel: a docked column beside the pages that shows agent runs.
//!
//! Dragging its left edge calls `track_resize`. The pointer leaves the panel webview as the
//! column grows, so the drag follows the screen cursor until the mouse button comes up.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use tauri::webview::Color;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl};
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

use crate::{paths, settings, tabs, webview2};

/// CSS pixels kept for the page when the panel grows.
const MIN_PAGE: f64 = 320.0;

struct Drag {
    window: String,
    /// How far the pointer sits to the right of the panel's left edge, in CSS pixels.
    grab: Option<f64>,
}

static DRAG: Mutex<Option<Drag>> = Mutex::new(None);
static RESIZING: AtomicBool = AtomicBool::new(false);
static FRAME: AtomicBool = AtomicBool::new(false);

struct ResizeGuard;

impl Drop for ResizeGuard {
    fn drop(&mut self) {
        RESIZING.store(false, Ordering::Release);
        FRAME.store(false, Ordering::Release);
        if let Ok(mut drag) = DRAG.lock() {
            *drag = None;
        }
    }
}

/// Opens or closes the panel in `window`. `open` forces a state. Returns whether it is open.
pub fn toggle(app: &AppHandle, window: &str, open: Option<bool>) -> Result<bool, String> {
    let is_open = tabs::agent_panel_width(app, window) > 0.0;
    let want = open.unwrap_or(!is_open);
    if want {
        ensure_view(app, window)?;
        tabs::set_agent_panel(app, window, open_width(app));
        if let Some(view) = app.get_webview(&tabs::agent_panel_label(window)) {
            let _ = view.set_focus();
        }
    } else {
        tabs::set_agent_panel(app, window, 0.0);
    }
    Ok(want)
}

fn open_width(app: &AppHandle) -> f64 {
    settings::get(app).agent_panel_width as f64
}

/// Follows the left button and resizes `window`'s open panel until the button comes up.
pub fn track_resize(app: &AppHandle, window: &str) -> Result<(), String> {
    if tabs::agent_panel_width(app, window) <= 0.0 {
        return Ok(());
    }
    if RESIZING.swap(true, Ordering::AcqRel) {
        return Ok(());
    }
    let _guard = ResizeGuard;
    {
        let mut drag = DRAG.lock().expect("agent resize");
        *drag = Some(Drag {
            window: window.to_string(),
            grab: None,
        });
    }
    request_frame(app);
    while left_down() {
        thread::sleep(Duration::from_millis(16));
        request_frame(app);
    }
    let started = Instant::now();
    while FRAME.load(Ordering::Acquire) && started.elapsed() < Duration::from_millis(500) {
        thread::sleep(Duration::from_millis(4));
    }
    let (sender, receiver) = mpsc::channel();
    let finish = app.clone();
    app.run_on_main_thread(move || {
        remember_current(&finish);
        let _ = sender.send(());
    })
    .map_err(|error| error.to_string())?;
    receiver
        .recv_timeout(Duration::from_secs(2))
        .map_err(|_| "The agent panel resize did not finish.".to_string())?;
    Ok(())
}

/// Sets the open panel to `width` CSS pixels and remembers it for the next open.
pub fn set_width(app: &AppHandle, window: &str, width: f64) -> Result<(), String> {
    let Some(width) = clamped_width(app, window, width) else {
        return Ok(());
    };
    if let Some(applied) = tabs::resize_agent_panel(app, window, width) {
        remember(app, applied);
    }
    Ok(())
}

fn remember_current(app: &AppHandle) {
    let window = {
        let Ok(drag) = DRAG.lock() else {
            return;
        };
        drag.as_ref().map(|drag| drag.window.clone())
    };
    let Some(window) = window else {
        return;
    };
    let width = tabs::agent_panel_width(app, &window);
    if width > 0.0 {
        remember(app, width);
    }
}

fn remember(app: &AppHandle, width: f64) {
    let width = width.round().clamp(0.0, u32::MAX as f64) as u32;
    let mut current = settings::get(app);
    if current.agent_panel_width == width {
        return;
    }
    current.agent_panel_width = width;
    if let Err(error) = settings::save(app, current) {
        eprintln!("agent panel width: {error}");
    }
}

fn request_frame(app: &AppHandle) {
    if FRAME.swap(true, Ordering::AcqRel) {
        return;
    }
    let moved = app.clone();
    if app
        .run_on_main_thread(move || {
            apply_drag(&moved);
            FRAME.store(false, Ordering::Release);
        })
        .is_err()
    {
        FRAME.store(false, Ordering::Release);
    }
}

fn apply_drag(app: &AppHandle) {
    let window = {
        let Ok(drag) = DRAG.lock() else {
            return;
        };
        let Some(drag) = drag.as_ref() else {
            return;
        };
        drag.window.clone()
    };
    let Some(geom) = geometry(app, &window) else {
        return;
    };
    let Some(cursor) = cursor_x(&geom) else {
        return;
    };
    let current = tabs::agent_panel_width(app, &window);
    if current <= 0.0 {
        return;
    }
    let grab = {
        let Ok(mut drag) = DRAG.lock() else {
            return;
        };
        let Some(state) = drag.as_mut() else {
            return;
        };
        if state.grab.is_none() {
            let left = geom.client - geom.right - current;
            state.grab = Some(cursor - left);
        }
        state.grab.unwrap_or(0.0)
    };
    let next = width_beside_pointer(geom.client, geom.left, geom.right, cursor, grab);
    tabs::resize_agent_panel(app, &window, next);
}

fn clamped_width(app: &AppHandle, window: &str, width: f64) -> Option<f64> {
    if tabs::agent_panel_width(app, window) <= 0.0 {
        return None;
    }
    let geom = geometry(app, window)?;
    Some(width_beside_pointer(
        geom.client,
        geom.left,
        geom.right,
        geom.client - geom.right - width,
        0.0,
    ))
}

struct Geom {
    scale: f64,
    origin_x: i32,
    client: f64,
    left: f64,
    right: f64,
}

fn geometry(app: &AppHandle, window: &str) -> Option<Geom> {
    let host = app.get_window(window)?;
    let scale = host
        .scale_factor()
        .ok()
        .filter(|scale| *scale > 0.0)
        .unwrap_or(1.0);
    let origin = host.inner_position().ok()?;
    let size = host.inner_size().ok()?;
    let (_, left, right, _) = tabs::page_insets(app, window)?;
    Some(Geom {
        scale,
        origin_x: origin.x,
        client: size.width as f64 / scale,
        left,
        right,
    })
}

fn cursor_x(geom: &Geom) -> Option<f64> {
    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point) }.ok()?;
    Some(f64::from(point.x - geom.origin_x) / geom.scale)
}

fn left_down() -> bool {
    unsafe { (GetAsyncKeyState(i32::from(VK_LBUTTON.0)) as u16) & 0x8000 != 0 }
}

/// Panel width when the pointer is `cursor` CSS pixels from the window's left edge.
/// `grab` is how far that pointer sits to the right of the panel's left edge.
fn width_beside_pointer(client: f64, left: f64, right: f64, cursor: f64, grab: f64) -> f64 {
    let min = f64::from(settings::AGENT_PANEL_WIDTH_PX.0);
    let max = f64::from(settings::AGENT_PANEL_WIDTH_PX.1);
    if ![client, left, right, cursor, grab]
        .iter()
        .all(|value| value.is_finite())
    {
        return f64::from(settings::AGENT_PANEL_DEFAULT_WIDTH);
    }
    let panel_right = client - right.max(0.0);
    let raw = panel_right - (cursor - grab);
    let room = panel_right - left.max(0.0) - MIN_PAGE;
    let cap = room.min(max).max(min);
    if !raw.is_finite() {
        return f64::from(settings::AGENT_PANEL_DEFAULT_WIDTH).clamp(min, cap);
    }
    raw.clamp(min, cap)
}

fn ensure_view(app: &AppHandle, window: &str) -> Result<(), String> {
    let label = tabs::agent_panel_label(window);
    if app.get_webview(&label).is_some() {
        return Ok(());
    }
    let host = app
        .get_window(window)
        .ok_or("The browser window is missing.")?;
    let (r, g, b) = settings::popup_surface(app);
    let url = paths::internal_url(app, "agent-panel");
    let builder = tabs::base_builder(app, &label, WebviewUrl::External(url))
        .background_color(Color(r, g, b, 255))
        .focused(false);
    let view = host
        .add_child(
            builder,
            LogicalPosition::new(-10000.0, -10000.0),
            LogicalSize::new(open_width(app), 400.0),
        )
        .map_err(|error| error.to_string())?;
    webview2::attach_panel_hooks(app, &view);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::width_beside_pointer;

    #[test]
    fn the_left_edge_follows_the_pointer() {
        assert_eq!(width_beside_pointer(1200.0, 0.0, 48.0, 800.0, 0.0), 352.0);
    }

    #[test]
    fn the_grab_keeps_the_edge_from_jumping() {
        assert_eq!(width_beside_pointer(1200.0, 0.0, 48.0, 776.0, 4.0), 380.0);
    }

    #[test]
    fn the_width_stays_between_the_limits() {
        assert_eq!(width_beside_pointer(1600.0, 0.0, 0.0, 0.0, 0.0), 720.0);
        assert_eq!(width_beside_pointer(1600.0, 0.0, 0.0, 1500.0, 0.0), 280.0);
    }

    #[test]
    fn a_narrow_window_keeps_the_minimum() {
        assert_eq!(width_beside_pointer(500.0, 0.0, 0.0, 0.0, 0.0), 280.0);
    }

    #[test]
    fn a_bad_pointer_keeps_the_default() {
        assert_eq!(width_beside_pointer(f64::NAN, 0.0, 0.0, 0.0, 0.0), 380.0);
    }
}
