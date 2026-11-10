//! Present covers the browser window with a spotlight, a zoom lens, a cursor, and drawing.
//!
//! The page webviews sit above the browser UI, so this is two owned windows, the same shape as
//! the recording overlay: a full-window stage and a small palette. The stage returns
//! `HTTRANSPARENT` except while a drawing tool is armed, while a lock drag is armed, or while
//! Alt is held for a spotlight or zoom drag. The palette is its own window, so it keeps clicks.

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};
use std::sync::{Mutex, mpsc};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::window::WindowBuilder;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, Window};
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_ESCAPE, VK_MENU};
use windows::Win32::UI::Magnification::{MAGTRANSFORM, MagInitialize, MagSetWindowSource, MagSetWindowTransform, MagUninitialize, WC_MAGNIFIER};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, CreateWindowExW, DefWindowProcW, DestroyWindow, EnumChildWindows, GetCursorPos, GetWindowRect, HC_ACTION,
    HHOOK, HTTRANSPARENT, HWND_TOPMOST, IsWindow, IsWindowVisible, KBDLLHOOKSTRUCT, LWA_ALPHA, RegisterClassExW, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_HIDE, SW_SHOWNA, SetLayeredWindowAttributes, SetWindowPos, SetWindowsHookExW,
    ShowWindow, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_NCHITTEST, WNDCLASSEXW, WS_CHILD, WS_CLIPCHILDREN,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP, WS_VISIBLE,
};
use windows::core::{BOOL, w};

use crate::{paths, tabs};

/// `main-*` is the capability for detached windows, so these can listen and call commands.
const STAGE: &str = "main-present";
const CONTROLS: &str = "main-present-controls";
const STAGE_PASS: usize = 0xce_0e51;
/// Logical px. The stage draws the same box; the lens window is this size times the scale.
const LENS_W: f64 = 280.0;
const LENS_H: f64 = 168.0;
const ZOOM: f32 = 2.0;
const LENS_INSET: f64 = 3.0;

static LIVE: AtomicBool = AtomicBool::new(false);
static GENERATION: AtomicU64 = AtomicU64::new(0);
static POSTED: AtomicBool = AtomicBool::new(false);
static CAPTURE: AtomicBool = AtomicBool::new(false);
static PLACEABLE: AtomicBool = AtomicBool::new(false);
static CONTROLS_HWND: AtomicIsize = AtomicIsize::new(0);
static LENS_HWND: AtomicIsize = AtomicIsize::new(0);
static MAG_HWND: AtomicIsize = AtomicIsize::new(0);
static MAG_READY: AtomicBool = AtomicBool::new(false);
static ESCAPE_HOOK: AtomicIsize = AtomicIsize::new(0);
static SESSION: Mutex<Option<LiveSession>> = Mutex::new(None);
static ESCAPE_APP: Mutex<Option<AppHandle>> = Mutex::new(None);

struct LiveSession {
    tool: String,
    cursor: String,
    color: String,
    placing: bool,
    clear: u32,
    spotlight: Option<PresentRect>,
    zoom: Option<PresentRect>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PresentRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresentState {
    pub on: bool,
    pub tool: String,
    pub cursor: String,
    pub color: String,
    pub placing: bool,
    pub clear: u32,
    pub spotlight: Option<PresentRect>,
    pub zoom: Option<PresentRect>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresentPointer {
    pub x: f64,
    pub y: f64,
    pub inside: bool,
}

struct ScreenBox {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

pub fn toggle(app: &AppHandle) -> Result<PresentState, String> {
    if LIVE.load(Ordering::Acquire) {
        close(app)?;
    } else {
        open(app)?;
    }
    Ok(state())
}

pub fn state() -> PresentState {
    snapshot(&SESSION.lock().unwrap_or_else(|poisoned| poisoned.into_inner()))
}

pub fn set_tool(app: &AppHandle, tool: &str) -> Result<PresentState, String> {
    let tool = known_tool(tool)?;
    mutate(app, |live| {
        if live.tool == tool && (tool == "spotlight" || tool == "zoom") {
            let locked = if tool == "spotlight" { live.spotlight.take() } else { live.zoom.take() };
            if live.placing || locked.is_some() {
                live.placing = false;
            } else {
                live.placing = true;
            }
        } else {
            live.tool = tool;
            live.placing = false;
        }
    })
}

pub fn set_cursor(app: &AppHandle, cursor: &str) -> Result<PresentState, String> {
    let cursor = known_cursor(cursor)?;
    mutate(app, |live| live.cursor = cursor)
}

pub fn set_color(app: &AppHandle, color: &str) -> Result<PresentState, String> {
    let color = known_color(color)?;
    mutate(app, |live| live.color = color)
}

/// Stores a dragged spotlight or zoom rectangle. `None` ends the drag without a new lock.
pub fn finish_place(app: &AppHandle, kind: &str, rect: Option<PresentRect>) -> Result<PresentState, String> {
    let kind = kind.to_string();
    if kind != "spotlight" && kind != "zoom" {
        return Err("Present can lock a spotlight or a zoom.".into());
    }
    let rect = rect.and_then(clean_rect);
    mutate(app, |live| {
        live.placing = false;
        if let Some(rect) = rect {
            if kind == "spotlight" {
                live.spotlight = Some(rect);
            } else {
                live.zoom = Some(rect);
            }
        }
    })
}

pub fn clear_marks(app: &AppHandle) -> Result<PresentState, String> {
    mutate(app, |live| live.clear = live.clear.wrapping_add(1))
}

fn mutate(app: &AppHandle, change: impl FnOnce(&mut LiveSession)) -> Result<PresentState, String> {
    let mut slot = SESSION.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(live) = slot.as_mut() else {
        return Err("Present is off.".into());
    };
    change(live);
    sync_flags(live);
    let next = snapshot(&slot);
    drop(slot);
    let _ = app.emit("present-changed", &next);
    Ok(next)
}

fn open(app: &AppHandle) -> Result<(), String> {
    on_main(app, |app| open_main(app))?
}

fn close(app: &AppHandle) -> Result<(), String> {
    LIVE.store(false, Ordering::Release);
    on_main(app, |app| close_main(app))?;
    Ok(())
}

fn on_main<R: Send + 'static>(app: &AppHandle, task: impl FnOnce(&AppHandle) -> R + Send + 'static) -> Result<R, String> {
    let (tx, rx) = mpsc::channel();
    let queued = app.clone();
    let inside = queued.clone();
    queued
        .run_on_main_thread(move || {
            let _ = tx.send(task(&inside));
        })
        .map_err(|error| error.to_string())?;
    rx.recv().map_err(|error| error.to_string())
}

fn open_main(app: &AppHandle) -> Result<(), String> {
    close_main(app);
    let owner = app.get_window(tabs::WINDOW).ok_or("The browser window is missing.")?;
    LIVE.store(true, Ordering::Release);
    let opened = (|| {
        open_page(app, &owner, STAGE, "present")?;
        open_page(app, &owner, CONTROLS, "present-controls")?;
        if let Some(stage) = app.get_window(STAGE) {
            pass_clicks(&stage);
        }
        apply_layout(app);
        show(app, STAGE);
        show(app, CONTROLS);
        if let Err(error) = ensure_lens(&owner) {
            eprintln!("present lens: {error}");
        }
        arm_escape(app);
        let generation = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
        start_pump(app.clone(), generation);
        let mut slot = SESSION.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        *slot = Some(LiveSession {
            tool: "spotlight".into(),
            cursor: "dot".into(),
            color: "#f59e0b".into(),
            placing: false,
            clear: 0,
            spotlight: None,
            zoom: None,
        });
        sync_flags(slot.as_ref().expect("present session"));
        let next = snapshot(&slot);
        drop(slot);
        let _ = app.emit("present-changed", &next);
        Ok(())
    })();
    if opened.is_err() {
        close_main(app);
    }
    opened
}

fn close_main(app: &AppHandle) {
    LIVE.store(false, Ordering::Release);
    disarm_escape();
    destroy_lens();
    CONTROLS_HWND.store(0, Ordering::Relaxed);
    CAPTURE.store(false, Ordering::Relaxed);
    PLACEABLE.store(false, Ordering::Relaxed);
    for label in [STAGE, CONTROLS] {
        if let Some(window) = app.get_window(label) {
            let _ = window.close();
        }
    }
    let mut slot = SESSION.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let was = slot.take().is_some();
    drop(slot);
    if was {
        let _ = app.emit("present-changed", state());
    }
}

fn start_pump(app: AppHandle, generation: u64) {
    thread::spawn(move || {
        while LIVE.load(Ordering::Acquire) && GENERATION.load(Ordering::Relaxed) == generation {
            if !POSTED.swap(true, Ordering::AcqRel) {
                let queued = app.clone();
                let inside = queued.clone();
                let posted = queued.run_on_main_thread(move || {
                    POSTED.store(false, Ordering::Release);
                    if LIVE.load(Ordering::Acquire) && GENERATION.load(Ordering::Relaxed) == generation {
                        sync(&inside);
                    }
                });
                if posted.is_err() {
                    POSTED.store(false, Ordering::Release);
                }
            }
            thread::sleep(Duration::from_millis(32));
        }
    });
}

fn sync(app: &AppHandle) {
    let Some(box_) = window_box(app, tabs::WINDOW) else {
        return;
    };
    apply_layout(app);
    let scale = app.get_window(STAGE).and_then(|window| window.scale_factor().ok()).unwrap_or(1.0);
    let pointer = pointer_at(&box_, scale);
    let _ = app.emit("present-pointer", pointer);
    let slot = SESSION.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(live) = slot.as_ref() else {
        return;
    };
    let tool = live.tool.clone();
    let zoom_lock = live.zoom;
    drop(slot);
    place_lens(app, &box_, scale, pointer, &tool, zoom_lock);
}

fn apply_layout(app: &AppHandle) {
    let Some(box_) = window_box(app, tabs::WINDOW) else {
        return;
    };
    if let Some(stage) = app.get_window(STAGE) {
        place_window(&stage, box_.x, box_.y, box_.width as u32, box_.height as u32);
        pass_clicks(&stage);
    }
    if let Some(controls) = app.get_window(CONTROLS) {
        let scale = controls.scale_factor().unwrap_or(1.0);
        let width = (700.0 * scale).round().max(1.0) as i32;
        let height = (84.0 * scale).round().max(1.0) as i32;
        let x = box_.x + (box_.width - width) / 2;
        let y = box_.y + box_.height - height - (28.0 * scale).round() as i32;
        place_window(&controls, x, y, width as u32, height as u32);
        if let Ok(hwnd) = controls.hwnd() {
            CONTROLS_HWND.store(hwnd.0 as isize, Ordering::Relaxed);
        }
    }
    restack(app);
}

fn pointer_at(box_: &ScreenBox, scale: f64) -> PresentPointer {
    let mut point = POINT::default();
    let read = unsafe { GetCursorPos(&mut point) }.is_ok();
    let scale = if scale < 0.5 { 1.0 } else { scale };
    if !read {
        return PresentPointer { x: 0.0, y: 0.0, inside: false };
    }
    let inside = point.x >= box_.x && point.y >= box_.y && point.x < box_.x + box_.width && point.y < box_.y + box_.height;
    PresentPointer {
        x: (point.x - box_.x) as f64 / scale,
        y: (point.y - box_.y) as f64 / scale,
        inside,
    }
}

fn place_lens(app: &AppHandle, box_: &ScreenBox, scale: f64, pointer: PresentPointer, tool: &str, lock: Option<PresentRect>) {
    let hwnd = LENS_HWND.load(Ordering::Relaxed);
    let mag = MAG_HWND.load(Ordering::Relaxed);
    if hwnd == 0 || mag == 0 || !MAG_READY.load(Ordering::Relaxed) {
        return;
    }
    if tool != "zoom" {
        hide_lens();
        return;
    }
    let scale = if scale < 0.5 { 1.0 } else { scale };
    let lens = match lock {
        Some(rect) => Some(ScreenBox {
            x: box_.x + (rect.x * box_.width as f32) as i32,
            y: box_.y + (rect.y * box_.height as f32) as i32,
            width: (rect.width * box_.width as f32).round().max(1.0) as i32,
            height: (rect.height * box_.height as f32).round().max(1.0) as i32,
        }),
        None if pointer.inside => {
            let width = (LENS_W * scale).round().max(1.0) as i32;
            let height = (LENS_H * scale).round().max(1.0) as i32;
            let x = (box_.x as f64 + pointer.x * scale - width as f64 / 2.0).round() as i32;
            let y = (box_.y as f64 + pointer.y * scale - height as f64 / 2.0).round() as i32;
            Some(ScreenBox {
                x: x.clamp(box_.x, (box_.x + box_.width - width).max(box_.x)),
                y: y.clamp(box_.y, (box_.y + box_.height - height).max(box_.y)),
                width: width.min(box_.width).max(1),
                height: height.min(box_.height).max(1),
            })
        }
        None => None,
    };
    let Some(lens) = lens else {
        hide_lens();
        return;
    };
    let inset = (LENS_INSET * scale).round().max(1.0) as i32;
    if lens.width <= inset * 2 || lens.height <= inset * 2 {
        hide_lens();
        return;
    }
    let host = HWND(hwnd as *mut core::ffi::c_void);
    let glass = HWND(mag as *mut core::ffi::c_void);
    unsafe {
        let _ = SetWindowPos(
            host,
            Some(HWND_TOPMOST),
            lens.x + inset,
            lens.y + inset,
            lens.width - inset * 2,
            lens.height - inset * 2,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        let _ = ShowWindow(host, SW_SHOWNA);
        let _ = SetWindowPos(glass, None, 0, 0, lens.width - inset * 2, lens.height - inset * 2, SWP_NOACTIVATE | SWP_NOMOVE);
    }
    let aim_x = if lock.is_some() { lens.x + lens.width / 2 } else { box_.x + (pointer.x * scale) as i32 };
    let aim_y = if lock.is_some() { lens.y + lens.height / 2 } else { box_.y + (pointer.y * scale) as i32 };
    let (sx, sy, sw, sh) = zoom_source(lens.x, lens.y, lens.width, lens.height, aim_x, aim_y, ZOOM, (box_.x, box_.y, box_.width, box_.height));
    let mut transform = MAGTRANSFORM { v: [0.0; 9] };
    transform.v[0] = ZOOM;
    transform.v[4] = ZOOM;
    transform.v[8] = 1.0;
    unsafe {
        let _ = MagSetWindowTransform(glass, &mut transform);
        let _ = MagSetWindowSource(glass, RECT { left: sx, top: sy, right: sx + sw, bottom: sy + sh });
    }
    if let Some(controls) = app.get_window(CONTROLS) {
        above_top(&controls);
    }
}

fn hide_lens() {
    let hwnd = LENS_HWND.load(Ordering::Relaxed);
    if hwnd == 0 {
        return;
    }
    unsafe {
        let _ = ShowWindow(HWND(hwnd as *mut core::ffi::c_void), SW_HIDE);
    }
}

/// Screen rectangle magnified into `lens`, centered on `aim` and kept inside `bounds`.
pub fn zoom_source(lens_x: i32, lens_y: i32, lens_w: i32, lens_h: i32, aim_x: i32, aim_y: i32, zoom: f32, bounds: (i32, i32, i32, i32)) -> (i32, i32, i32, i32) {
    let _ = (lens_x, lens_y);
    let zoom = if zoom < 1.05 { 1.05 } else { zoom };
    let sw = ((lens_w as f32) / zoom).round().max(1.0) as i32;
    let sh = ((lens_h as f32) / zoom).round().max(1.0) as i32;
    let (bx, by, bw, bh) = bounds;
    let x = if sw >= bw { bx } else { (aim_x - sw / 2).clamp(bx, bx + bw - sw) };
    let y = if sh >= bh { by } else { (aim_y - sh / 2).clamp(by, by + bh - sh) };
    (x, y, sw, sh)
}

fn ensure_lens(owner: &Window) -> Result<(), String> {
    if MAG_READY.load(Ordering::Acquire) && LENS_HWND.load(Ordering::Relaxed) != 0 {
        return Ok(());
    }
    if !unsafe { MagInitialize() }.as_bool() {
        return Err(windows::core::Error::from_win32().to_string());
    }
    MAG_READY.store(true, Ordering::Release);
    register_lens_class()?;
    let owner_hwnd = owner.hwnd().map_err(|error| error.to_string())?;
    let instance = unsafe { windows::Win32::System::LibraryLoader::GetModuleHandleW(None) }.map_err(|error| error.to_string())?;
    let host = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT,
            w!("CePresentLens"),
            w!("Present lens"),
            WS_POPUP | WS_CLIPCHILDREN,
            0,
            0,
            40,
            40,
            Some(owner_hwnd),
            None,
            Some(HINSTANCE(instance.0)),
            None,
        )
    }
    .map_err(|error| error.to_string())?;
    unsafe {
        let _ = SetLayeredWindowAttributes(host, COLORREF(0), 255, LWA_ALPHA);
    }
    let glass = unsafe {
        CreateWindowExW(
            WS_EX_TRANSPARENT,
            WC_MAGNIFIER,
            w!("Present magnifier"),
            WS_CHILD | WS_VISIBLE,
            0,
            0,
            40,
            40,
            Some(host),
            None,
            Some(HINSTANCE(instance.0)),
            None,
        )
    }
    .map_err(|error| error.to_string())?;
    LENS_HWND.store(host.0 as isize, Ordering::Relaxed);
    MAG_HWND.store(glass.0 as isize, Ordering::Relaxed);
    hide_lens();
    Ok(())
}

unsafe extern "system" fn lens_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

fn register_lens_class() -> Result<(), String> {
    let instance = unsafe { windows::Win32::System::LibraryLoader::GetModuleHandleW(None) }.map_err(|error| error.to_string())?;
    let class = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(lens_proc),
        hInstance: instance.into(),
        lpszClassName: w!("CePresentLens"),
        ..Default::default()
    };
    let atom = unsafe { RegisterClassExW(&class) };
    if atom == 0 {
        let error = windows::core::Error::from_win32();
        if error.code().0 != 1410 {
            return Err(error.to_string());
        }
    }
    Ok(())
}

fn destroy_lens() {
    hide_lens();
    let host = LENS_HWND.swap(0, Ordering::Relaxed);
    MAG_HWND.store(0, Ordering::Relaxed);
    if host != 0 {
        unsafe {
            let _ = DestroyWindow(HWND(host as *mut core::ffi::c_void));
        }
    }
    if MAG_READY.swap(false, Ordering::AcqRel) {
        unsafe {
            let _ = MagUninitialize();
        }
    }
}

fn open_page(app: &AppHandle, owner: &Window, label: &str, page: &str) -> Result<(), String> {
    if let Some(window) = app.get_window(label) {
        let _ = window.close();
    }
    let window = WindowBuilder::new(app, label)
        .parent(owner)
        .map_err(|error| error.to_string())?
        .title(format!("Present {page}"))
        .inner_size(320.0, 120.0)
        .decorations(false)
        .shadow(false)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .visible(false)
        .always_on_top(true)
        .transparent(true)
        .build()
        .map_err(|error| error.to_string())?;
    let url = paths::internal_url(app, page);
    let webview = tabs::base_builder(app, label, WebviewUrl::External(url))
        .transparent(true)
        .auto_resize()
        .disable_drag_drop_handler()
        .focused(false);
    let size = window.inner_size().map_err(|error| error.to_string())?;
    window.add_child(webview, PhysicalPosition::new(0, 0), size).map_err(|error| error.to_string())?;
    Ok(())
}

fn place_window(window: &Window, x: i32, y: i32, width: u32, height: u32) {
    let _ = window.set_position(PhysicalPosition::new(x, y));
    let _ = window.set_size(PhysicalSize::new(width.max(1), height.max(1)));
}

fn show(app: &AppHandle, label: &str) {
    if let Some(window) = app.get_window(label) {
        let _ = window.show();
        above_top(&window);
    }
}

fn restack(app: &AppHandle) {
    let stage = app.get_window(STAGE);
    let controls = app.get_window(CONTROLS);
    if let Some(stage) = stage.as_ref() {
        above_top(stage);
    }
    let lens = LENS_HWND.load(Ordering::Relaxed);
    if lens != 0 {
        if let Some(stage) = stage.as_ref().and_then(|window| window.hwnd().ok()) {
            unsafe {
                let _ = SetWindowPos(
                    HWND(lens as *mut core::ffi::c_void),
                    Some(stage),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
        }
    }
    if let Some(controls) = controls.as_ref() {
        above_top(controls);
    }
}

fn above_top(window: &Window) {
    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    unsafe {
        let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    }
}

fn window_box(app: &AppHandle, label: &str) -> Option<ScreenBox> {
    let window = app.get_window(label)?;
    let hwnd = window.hwnd().ok()?;
    let mut rect = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut rect).ok()? };
    let width = rect.right.saturating_sub(rect.left);
    let height = rect.bottom.saturating_sub(rect.top);
    if width < 40 || height < 40 {
        return None;
    }
    Some(ScreenBox { x: rect.left, y: rect.top, width, height })
}

fn pass_clicks(window: &Window) {
    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    unsafe {
        let _ = SetWindowSubclass(hwnd, Some(stage_hit_proc), STAGE_PASS, 0);
        let _ = EnumChildWindows(Some(hwnd), Some(pass_child), LPARAM(0));
    }
}

unsafe extern "system" fn pass_child(child: HWND, _: LPARAM) -> BOOL {
    unsafe {
        let _ = SetWindowSubclass(child, Some(stage_hit_proc), STAGE_PASS, 0);
    }
    BOOL(1)
}

unsafe extern "system" fn stage_hit_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM, _: usize, _: usize) -> LRESULT {
    if message == WM_NCHITTEST {
        let point = hit_point(lparam);
        if over_controls(point) || !(CAPTURE.load(Ordering::Relaxed) || (PLACEABLE.load(Ordering::Relaxed) && alt_down())) {
            return LRESULT(HTTRANSPARENT as isize);
        }
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

fn hit_point(lparam: LPARAM) -> POINT {
    let packed = lparam.0 as u32;
    POINT { x: packed as i16 as i32, y: (packed >> 16) as i16 as i32 }
}

fn alt_down() -> bool {
    unsafe { GetAsyncKeyState(VK_MENU.0 as i32) < 0 }
}

fn over_controls(point: POINT) -> bool {
    let raw = CONTROLS_HWND.load(Ordering::Relaxed);
    if raw == 0 {
        return false;
    }
    let hwnd = HWND(raw as *mut core::ffi::c_void);
    unsafe {
        if !IsWindow(Some(hwnd)).as_bool() || !IsWindowVisible(hwnd).as_bool() {
            return false;
        }
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return false;
        }
        point.x >= rect.left && point.x < rect.right && point.y >= rect.top && point.y < rect.bottom
    }
}

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
    if let Ok(mut slot) = ESCAPE_APP.lock() {
        *slot = None;
    }
}

unsafe extern "system" fn escape_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && wparam.0 as u32 == WM_KEYDOWN && lparam.0 != 0 {
        let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        if info.vkCode == VK_ESCAPE.0 as u32 && LIVE.load(Ordering::Acquire) {
            if let Some(app) = ESCAPE_APP.lock().ok().and_then(|slot| slot.clone()) {
                tauri::async_runtime::spawn(async move {
                    let _ = close(&app);
                });
            }
            return LRESULT(1);
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn sync_flags(live: &LiveSession) {
    let draw = matches!(live.tool.as_str(), "highlight" | "pen" | "arrow" | "text" | "eraser");
    CAPTURE.store(draw || live.placing, Ordering::Relaxed);
    PLACEABLE.store(live.tool == "spotlight" || live.tool == "zoom", Ordering::Relaxed);
}

fn snapshot(slot: &Option<LiveSession>) -> PresentState {
    match slot {
        Some(live) => PresentState {
            on: true,
            tool: live.tool.clone(),
            cursor: live.cursor.clone(),
            color: live.color.clone(),
            placing: live.placing,
            clear: live.clear,
            spotlight: live.spotlight,
            zoom: live.zoom,
        },
        None => PresentState {
            on: false,
            tool: "spotlight".into(),
            cursor: "dot".into(),
            color: "#f59e0b".into(),
            placing: false,
            clear: 0,
            spotlight: None,
            zoom: None,
        },
    }
}

fn known_tool(tool: &str) -> Result<String, String> {
    match tool {
        "spotlight" | "zoom" | "highlight" | "pen" | "arrow" | "text" | "eraser" => Ok(tool.to_string()),
        _ => Err("Unknown present tool.".into()),
    }
}

fn known_cursor(cursor: &str) -> Result<String, String> {
    match cursor {
        "dot" | "ring" | "arrow" | "crosshair" | "off" => Ok(cursor.to_string()),
        _ => Err("Unknown present cursor.".into()),
    }
}

fn known_color(color: &str) -> Result<String, String> {
    let bytes = color.as_bytes();
    if bytes.len() == 7 && bytes[0] == b'#' && bytes[1..].iter().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(color.to_ascii_lowercase())
    } else {
        Err("Present color is a #rrggbb value.".into())
    }
}

fn clean_rect(rect: PresentRect) -> Option<PresentRect> {
    if !rect.x.is_finite() || !rect.y.is_finite() || !rect.width.is_finite() || !rect.height.is_finite() {
        return None;
    }
    let x = rect.x.min(rect.x + rect.width);
    let y = rect.y.min(rect.y + rect.height);
    let width = rect.width.abs();
    let height = rect.height.abs();
    if width < 0.01 || height < 0.01 {
        return None;
    }
    Some(PresentRect {
        x: x.clamp(0.0, 1.0),
        y: y.clamp(0.0, 1.0),
        width: width.min(1.0),
        height: height.min(1.0),
    })
}

#[cfg(test)]
mod tests {
    use super::zoom_source;

    #[test]
    fn zoom_source_is_the_lens_divided_by_the_factor_and_kept_on_the_window() {
        let source = zoom_source(100, 80, 200, 120, 200, 140, 2.0, (0, 0, 1000, 800));
        assert_eq!(source, (150, 110, 100, 60));
        let clamped = zoom_source(0, 0, 200, 120, 10, 10, 2.0, (0, 0, 1000, 800));
        assert_eq!(clamped, (0, 0, 100, 60));
    }
}
