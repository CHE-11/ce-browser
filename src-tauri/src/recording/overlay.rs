//! The control bar, camera bubble, and draw layer. Each is its own always-on-top window, and
//! each is left out of screen capture so a recording of the desktop does not film the controls.

use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Mutex;

use tauri::window::WindowBuilder;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, Window};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL, VK_ESCAPE, VK_MENU};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, EnumChildWindows, GetWindowRect, HC_ACTION, HHOOK, HTTRANSPARENT, HWND_TOPMOST, IsWindowVisible,
    KBDLLHOOKSTRUCT, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowDisplayAffinity, SetWindowPos,
    SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_NCHITTEST, WDA_EXCLUDEFROMCAPTURE,
};
use windows::core::BOOL;

use crate::{paths, tabs};

use super::targets::{self, ScreenRect, Target};

const CONTROLS: &str = "recording-controls";
const CAMERA: &str = "recording-camera";
const DRAW: &str = "recording-draw";
const DRAW_PASS: usize = 0xce_d4a9;

/// Screen rectangles the draw layer must not steal. Clicks there belong to the controls and camera.
static CHROME: [AtomicIsize; 2] = [AtomicIsize::new(0), AtomicIsize::new(0)];
static ESCAPE_HOOK: AtomicIsize = AtomicIsize::new(0);
static ESCAPE_APP: Mutex<Option<AppHandle>> = Mutex::new(None);

pub fn open(app: &AppHandle, target: &Target, camera: bool) -> Result<(), String> {
    close(app);
    let rect = targets::screen_rect(target);
    open_page(app, CONTROLS, "recording-controls", true)?;
    open_page(app, DRAW, "recording-draw", true)?;
    if camera {
        open_page(app, CAMERA, "recording-camera", true)?;
    }
    place(app, target, rect.as_ref(), camera);
    hide(app, DRAW);
    Ok(())
}

/// Grows or shrinks the pill window. Its child page fills the window on resize, and the bottom
/// edge stays put so the settings open upward instead of off the screen.
pub fn set_controls_mode(app: &AppHandle, mode: &str) -> Result<(), String> {
    let Some(window) = app.get_window(CONTROLS) else {
        return Ok(());
    };
    let scale = window.scale_factor().unwrap_or(1.0);
    let (logical_w, logical_h) = match mode {
        "hidden" => (168.0, 48.0),
        "settings" => (460.0, 168.0),
        "tools" => (460.0, 104.0),
        _ => (460.0, 56.0),
    };
    let width = (logical_w * scale).round().max(1.0) as u32;
    let height = (logical_h * scale).round().max(1.0) as u32;
    let outer = window.outer_position().map_err(|error| error.to_string())?;
    let outer_size = window.outer_size().map_err(|error| error.to_string())?;
    let y = outer.y + outer_size.height as i32 - height as i32;
    let _ = window.set_position(PhysicalPosition::new(outer.x, y));
    let _ = window.set_size(PhysicalSize::new(width, height));
    remember(&window, 0);
    restack(app);
    Ok(())
}

pub fn close(app: &AppHandle) {
    disarm_escape();
    for label in [CONTROLS, CAMERA, DRAW] {
        if let Some(window) = app.get_window(label) {
            let _ = window.close();
        }
    }
}

pub fn show_camera(app: &AppHandle, target: &Target) -> Result<(), String> {
    move_camera(app, target, "bottom-left")
}

pub fn move_camera(app: &AppHandle, target: &Target, corner: &str) -> Result<(), String> {
    if app.get_window(CAMERA).is_none() {
        open_page(app, CAMERA, "recording-camera", true)?;
    }
    let Some(rect) = targets::screen_rect(target) else {
        return Ok(());
    };
    let Some(window) = app.get_window(CAMERA) else {
        return Ok(());
    };
    let size = 200i32;
    let margin = 24i32;
    let (x, y) = match corner {
        "bottom-right" => (rect.x + rect.width - size - margin, rect.y + rect.height - size - margin),
        "top-left" => (rect.x + margin, rect.y + margin),
        "top-right" => (rect.x + rect.width - size - margin, rect.y + margin),
        _ => (rect.x + margin, rect.y + rect.height - size - margin),
    };
    place_window(&window, x, y, size as u32, size as u32);
    let _ = window.show();
    exclude(&window);
    remember(&window, 1);
    restack(app);
    Ok(())
}

pub fn hide_camera(app: &AppHandle) {
    hide(app, CAMERA);
}

pub fn show_draw(app: &AppHandle, target: &Target) {
    let Some(rect) = targets::screen_rect(target) else {
        return;
    };
    let app = app.clone();
    let _ = app.clone().run_on_main_thread(move || show_draw_at(&app, &rect));
}

fn show_draw_at(app: &AppHandle, rect: &ScreenRect) {
    let Some(window) = app.get_window(DRAW) else {
        return;
    };
    place_window(&window, rect.x, rect.y, rect.width.max(1) as u32, rect.height.max(1) as u32);
    let _ = window.show();
    exclude(&window);
    pass_clicks(&window);
    restack(app);
    arm_escape(app);
}

pub fn hide_draw(app: &AppHandle) {
    disarm_escape();
    hide(app, DRAW);
}

pub fn follow_draw(app: &AppHandle, target: &Target) {
    if app.get_window(DRAW).is_some_and(|window| window.is_visible().unwrap_or(false)) {
        show_draw(app, target);
    }
}

pub fn camera_layout(app: &AppHandle, target: &Target) -> Option<super::CameraLayout> {
    let window = app.get_window(CAMERA)?;
    if !window.is_visible().unwrap_or(false) {
        return None;
    }
    let rect = targets::screen_rect(target)?;
    let position = window.outer_position().ok()?;
    let size = window.outer_size().ok()?;
    if rect.width <= 0 || rect.height <= 0 {
        return None;
    }
    Some(
        super::CameraLayout {
            x: (position.x - rect.x) as f32 / rect.width as f32,
            y: (position.y - rect.y) as f32 / rect.height as f32,
            size: size.width as f32 / rect.width as f32,
        }
        .clamped(),
    )
}

fn place(app: &AppHandle, target: &Target, rect: Option<&ScreenRect>, camera: bool) {
    let Some(rect) = rect else {
        return;
    };
    if let Some(window) = app.get_window(CONTROLS) {
        let width = 440i32;
        let height = 44i32;
        let x = rect.x + (rect.width - width) / 2;
        let y = rect.y + rect.height - height - 38;
        place_window(&window, x, y, width as u32, height as u32);
        let _ = window.show();
        exclude(&window);
        remember(&window, 0);
        restack(app);
    }
    if camera {
        let _ = show_camera(app, target);
    }
    if let Some(window) = app.get_window(DRAW) {
        place_window(&window, rect.x, rect.y, rect.width.max(1) as u32, rect.height.max(1) as u32);
        exclude(&window);
    }
}

fn open_page(app: &AppHandle, label: &str, page: &str, transparent: bool) -> Result<(), String> {
    if let Some(window) = app.get_window(label) {
        let _ = window.close();
    }
    let mut builder = WindowBuilder::new(app, label)
        .title(format!("Recording {page}"))
        .inner_size(320.0, 120.0)
        .decorations(false)
        .shadow(false)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .visible(false)
        .always_on_top(true)
        .transparent(transparent);
    if transparent {
        builder = builder.shadow(false);
    }
    let window = builder.build().map_err(|error| error.to_string())?;
    let url = paths::internal_url(app, page);
    let webview = tabs::base_builder(app, label, WebviewUrl::External(url))
        .transparent(transparent)
        .auto_resize()
        .disable_drag_drop_handler()
        .focused(false);
    let size = window.inner_size().map_err(|error| error.to_string())?;
    window
        .add_child(webview, PhysicalPosition::new(0, 0), size)
        .map_err(|error| error.to_string())?;
    if page == "recording-camera" {
        if let Some(view) = app.get_webview(label) {
            crate::webview2::allow_camera(&view);
        }
    }
    exclude(&window);
    Ok(())
}

fn place_window(window: &Window, x: i32, y: i32, width: u32, height: u32) {
    let _ = window.set_position(PhysicalPosition::new(x, y));
    let _ = window.set_size(PhysicalSize::new(width.max(1), height.max(1)));
}

/// Controls sit above the draw layer, and the camera sits above the controls.
fn restack(app: &AppHandle) {
    let draw = app.get_window(DRAW);
    let controls = app.get_window(CONTROLS);
    let camera = app.get_window(CAMERA);
    if let (Some(draw), Some(controls)) = (&draw, &controls) {
        above(controls, draw);
    }
    if let (Some(controls), Some(camera)) = (&controls, &camera) {
        above(camera, controls);
    }
}

fn above(front: &Window, back: &Window) {
    let Ok(front_hwnd) = front.hwnd() else {
        return;
    };
    let Ok(back_hwnd) = back.hwnd() else {
        return;
    };
    unsafe {
        let _ = SetWindowPos(front_hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        let _ = SetWindowPos(back_hwnd, Some(front_hwnd), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    }
}

fn remember(window: &Window, slot: usize) {
    if let Ok(hwnd) = window.hwnd() {
        CHROME[slot].store(hwnd.0 as isize, Ordering::Relaxed);
    }
}

/// Clicks that land on the controls or the camera fall through the draw layer.
fn pass_clicks(window: &Window) {
    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    unsafe {
        let _ = SetWindowSubclass(hwnd, Some(draw_hit_proc), DRAW_PASS, 0);
        let _ = EnumChildWindows(Some(hwnd), Some(pass_child), LPARAM(0));
    }
}

unsafe extern "system" fn pass_child(child: HWND, _: LPARAM) -> BOOL {
    unsafe {
        let _ = SetWindowSubclass(child, Some(draw_hit_proc), DRAW_PASS, 0);
    }
    BOOL(1)
}

unsafe extern "system" fn draw_hit_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM, _: usize, _: usize) -> LRESULT {
    if message == WM_NCHITTEST && over_chrome(hit_point(lparam)) {
        return LRESULT(HTTRANSPARENT as isize);
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

fn hit_point(lparam: LPARAM) -> POINT {
    let packed = lparam.0 as u32;
    POINT {
        x: packed as i16 as i32,
        y: (packed >> 16) as i16 as i32,
    }
}

fn over_chrome(point: POINT) -> bool {
    CHROME.iter().any(|slot| covers(slot, point))
}

fn covers(slot: &AtomicIsize, point: POINT) -> bool {
    let raw = slot.load(Ordering::Relaxed);
    if raw == 0 {
        return false;
    }
    let hwnd = HWND(raw as *mut core::ffi::c_void);
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() {
            return false;
        }
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return false;
        }
        point.x >= rect.left && point.x < rect.right && point.y >= rect.top && point.y < rect.bottom
    }
}

/// Escape leaves drawing while the tools are open, whichever window has the keyboard.
fn arm_escape(app: &AppHandle) {
    if let Ok(mut slot) = ESCAPE_APP.lock() {
        *slot = Some(app.clone());
    }
    if ESCAPE_HOOK.load(Ordering::Acquire) != 0 {
        return;
    }
    let Ok(installed) = (unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(escape_proc), None, 0) }) else {
        return;
    };
    ESCAPE_HOOK.store(installed.0 as isize, Ordering::Release);
}

fn disarm_escape() {
    let raw = ESCAPE_HOOK.swap(0, Ordering::AcqRel);
    if raw != 0 {
        unsafe {
            let _ = UnhookWindowsHookEx(HHOOK(raw as *mut core::ffi::c_void));
        }
    }
}

unsafe extern "system" fn escape_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && wparam.0 as u32 == WM_KEYDOWN && lparam.0 != 0 {
        let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let modified = unsafe { GetAsyncKeyState(VK_CONTROL.0 as i32) < 0 || GetAsyncKeyState(VK_MENU.0 as i32) < 0 };
        if info.vkCode == VK_ESCAPE.0 as u32 && !modified {
            if let Some(app) = ESCAPE_APP.lock().ok().and_then(|slot| slot.clone()) {
                tauri::async_runtime::spawn(async move {
                    let _ = super::set_drawing(&app, false);
                });
            }
            return LRESULT(1);
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn hide(app: &AppHandle, label: &str) {
    if let Some(window) = app.get_window(label) {
        let _ = window.hide();
    }
}

fn exclude(window: &Window) {
    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    unsafe {
        let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
    }
}
