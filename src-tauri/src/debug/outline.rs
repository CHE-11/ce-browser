//! Outlines one webview on screen until it is cleared.
//!
//! Native webviews draw above `ui`, so the outline is its own window over the target. Its
//! window region is only the border and a label tab, so clicks inside it reach the page.
//!
//! The window is shown and hidden with raw Win32 calls. Tauri never saw it shown, so its own
//! `hide` would do nothing.

use std::sync::{Mutex, MutexGuard};

use tauri::window::WindowBuilder;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, Window};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    CombineRgn, CreateRectRgn, DeleteObject, HGDIOBJ, RGN_DIFF, RGN_OR, SetWindowRgn,
};
use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, ShowWindow};

use crate::{paths, tabs, webview2};

use super::webviews::DebugWebview;

/// Logical px. `DebugOutline.page.tsx` draws the border and the tab at these sizes.
const BORDER: f64 = 3.0;
const TAB_WIDTH: f64 = 280.0;
const TAB_HEIGHT: f64 = 22.0;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Outlined {
    label: String,
    outline: String,
    /// Screen px the outline was last placed at: `(x, y, width, height)`.
    rect: (i32, i32, i32, i32),
}

/// The one webview outlined now, across every browser window.
static CURRENT: Mutex<Option<Outlined>> = Mutex::new(None);

fn current() -> MutexGuard<'static, Option<Outlined>> {
    CURRENT.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn outlined_label() -> Option<String> {
    current().as_ref().map(|outlined| outlined.label.clone())
}

/// Outlines `label`, or clears the outline when `label` already has it. Resolves to whether
/// `label` is outlined now.
pub fn toggle(app: &AppHandle, label: &str) -> Result<bool, String> {
    if outlined_label().as_deref() == Some(label) {
        clear(app);
        return Ok(false);
    }
    show(app, label)?;
    Ok(true)
}

fn show(app: &AppHandle, label: &str) -> Result<(), String> {
    let target = app
        .get_webview(label)
        .ok_or_else(|| format!("No webview is labelled {label}."))?;
    let host = target.window();
    let origin = host.inner_position().map_err(|error| error.to_string())?;
    let at = target.position().map_err(|error| error.to_string())?;
    let size = target.size().map_err(|error| error.to_string())?;
    let scale = host.scale_factor().unwrap_or(1.0);
    let owner = super::owner_of(host.label());
    let outline_label = format!("debug-outline-{owner}");
    let outline = match app.get_window(&outline_label) {
        Some(outline) => outline,
        None => create(app, &owner, &outline_label, label)?,
    };
    let _ = app.emit_to(outline_label.as_str(), "debug-outline", label);

    let rect = (
        origin.x + at.x,
        origin.y + at.y,
        size.width.max(1) as i32,
        size.height.max(1) as i32,
    );
    let previous = current().replace(Outlined {
        label: label.to_string(),
        outline: outline_label.clone(),
        rect,
    });
    if let Some(previous) = previous.filter(|previous| previous.outline != outline_label) {
        hide_window(app, &previous.outline);
    }

    let (x, y, width, height) = rect;
    let frame = Frame {
        width,
        height,
        border: ((BORDER * scale).round() as i32).max(1),
        tab_width: ((TAB_WIDTH * scale).round() as i32).min(width),
        tab_height: ((TAB_HEIGHT * scale).round() as i32).min(height),
    };
    let hwnd = outline.hwnd().map(|hwnd| hwnd.0 as isize).unwrap_or(0);
    let _ = app.run_on_main_thread(move || {
        webview2::place_popup_window(hwnd, x, y, width, height, true);
        frame.apply(hwnd);
    });
    Ok(())
}

pub fn clear(app: &AppHandle) {
    if let Some(outlined) = current().take() {
        hide_window(app, &outlined.outline);
    }
}

fn hide_window(app: &AppHandle, label: &str) {
    let Some(outline) = app.get_window(label) else {
        return;
    };
    let hwnd = outline.hwnd().map(|hwnd| hwnd.0 as isize).unwrap_or(0);
    if hwnd == 0 {
        return;
    }
    let _ = app.run_on_main_thread(move || unsafe {
        let _ = ShowWindow(HWND(hwnd as *mut std::ffi::c_void), SW_HIDE);
    });
}

/// Keeps the outline on its webview as the HUD lists webviews. A webview that is gone or
/// parked, such as a background tab or a closed popup, loses the outline; one that moved
/// takes it along.
pub fn sync(app: &AppHandle, rows: &[DebugWebview]) {
    let Some(outlined) = current().clone() else {
        return;
    };
    let row = rows.iter().find(|row| row.label == outlined.label);
    match row {
        Some(row) if !row.parked => {
            let rect = (
                row.screen_x,
                row.screen_y,
                row.width.max(1) as i32,
                row.height.max(1) as i32,
            );
            if rect != outlined.rect {
                let _ = show(app, &outlined.label);
            }
        }
        _ => clear(app),
    }
}

fn create(app: &AppHandle, window: &str, label: &str, target: &str) -> Result<Window, String> {
    let owner = app
        .get_window(window)
        .ok_or("The browser window is missing.")?;
    let outline = WindowBuilder::new(app, label)
        .title("ce-browser outline")
        .decorations(false)
        .shadow(false)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .visible(false)
        .always_on_top(true)
        .inner_size(100.0, 100.0)
        .parent(&owner)
        .map_err(|error| error.to_string())?
        .build()
        .map_err(|error| error.to_string())?;
    let query: String = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("label", target)
        .finish();
    let url = paths::internal_url(app, &format!("debug-outline?{query}"));
    let builder = tabs::base_builder(app, label, WebviewUrl::External(url))
        .auto_resize()
        .disable_drag_drop_handler()
        .focused(false);
    let size = outline.inner_size().map_err(|error| error.to_string())?;
    outline
        .add_child(builder, PhysicalPosition::new(0, 0), size)
        .map_err(|error| error.to_string())?;
    Ok(outline)
}

/// The visible part of the outline window, physical px from its top-left corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Frame {
    width: i32,
    height: i32,
    border: i32,
    tab_width: i32,
    tab_height: i32,
}

impl Frame {
    /// `(left, top, right, bottom)` of the hole inside the border. `None` when the target is
    /// too small for one, so the whole window shows.
    fn hole(&self) -> Option<(i32, i32, i32, i32)> {
        let (left, top) = (self.border, self.border);
        let (right, bottom) = (self.width - self.border, self.height - self.border);
        (right > left && bottom > top).then_some((left, top, right, bottom))
    }

    /// Main thread only.
    fn apply(&self, hwnd: isize) {
        if hwnd == 0 {
            return;
        }
        unsafe {
            let region = CreateRectRgn(0, 0, self.width, self.height);
            if region.0.is_null() {
                return;
            }
            if let Some((left, top, right, bottom)) = self.hole() {
                let hole = CreateRectRgn(left, top, right, bottom);
                let tab = CreateRectRgn(0, 0, self.tab_width, self.tab_height);
                let _ = CombineRgn(Some(region), Some(region), Some(hole), RGN_DIFF);
                let _ = CombineRgn(Some(region), Some(region), Some(tab), RGN_OR);
                let _ = DeleteObject(HGDIOBJ(hole.0));
                let _ = DeleteObject(HGDIOBJ(tab.0));
            }
            if SetWindowRgn(HWND(hwnd as *mut std::ffi::c_void), Some(region), true) == 0 {
                let _ = DeleteObject(HGDIOBJ(region.0));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Frame;

    #[test]
    fn the_hole_leaves_only_the_border() {
        let frame = Frame {
            width: 800,
            height: 600,
            border: 3,
            tab_width: 280,
            tab_height: 22,
        };
        assert_eq!(frame.hole(), Some((3, 3, 797, 597)));
    }

    #[test]
    fn a_tiny_target_has_no_hole() {
        let frame = Frame {
            width: 4,
            height: 4,
            border: 3,
            tab_width: 4,
            tab_height: 4,
        };
        assert_eq!(frame.hole(), None);
    }
}
