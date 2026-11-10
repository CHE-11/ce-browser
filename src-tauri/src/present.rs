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
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_BACK, VK_DELETE, VK_ESCAPE, VK_MENU,
};
use windows::Win32::UI::Magnification::{
    MAGTRANSFORM, MW_FILTERMODE_EXCLUDE, MagInitialize, MagSetWindowFilterList, MagSetWindowSource,
    MagSetWindowTransform, MagUninitialize, WC_MAGNIFIER,
};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, CreateWindowExW, DefWindowProcW, DestroyWindow, EnumChildWindows, GA_ROOT,
    GWL_EXSTYLE, GetAncestor, GetCursorPos, GetForegroundWindow, GetParent, GetWindowLongPtrW,
    GetWindowRect, HC_ACTION, HHOOK, HTCLIENT, HTTRANSPARENT, HWND_TOPMOST, IsWindow,
    IsWindowVisible, KBDLLHOOKSTRUCT, LWA_ALPHA, RegisterClassExW, SW_HIDE, SW_SHOWNA,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SetCursor,
    SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos, SetWindowsHookExW, ShowWindow,
    UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_NCHITTEST, WM_SETCURSOR, WNDCLASSEXW,
    WS_CHILD, WS_CLIPCHILDREN, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_EX_TRANSPARENT, WS_POPUP, WS_VISIBLE,
};
use windows::core::{BOOL, w};

use crate::{paths, tabs};

/// `main-*` is the capability for detached windows, so these can listen and call commands.
const STAGE: &str = "main-present";
const CONTROLS: &str = "main-present-controls";
const STAGE_PASS: usize = 0xce_0e51;
const CURSOR_PASS: usize = 0xce_0e52;
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
static HOLD_ESCAPE: AtomicBool = AtomicBool::new(false);
static FAST_POINTER: AtomicBool = AtomicBool::new(false);
static BLANK_CURSOR: AtomicBool = AtomicBool::new(false);
static FOCUSED: AtomicBool = AtomicBool::new(true);
static ESCAPE_REQUESTED: AtomicBool = AtomicBool::new(false);
static ESCAPE_DOWN: AtomicBool = AtomicBool::new(false);
static DELETE_REQUESTED: AtomicBool = AtomicBool::new(false);
static BLANK_TICK: AtomicU64 = AtomicU64::new(0);
static SESSION: Mutex<Option<LiveSession>> = Mutex::new(None);
static ESCAPE_APP: Mutex<Option<AppHandle>> = Mutex::new(None);
static LAST_LAYOUT: Mutex<Option<ScreenBox>> = Mutex::new(None);

struct LiveSession {
    tool: String,
    cursor: String,
    color: String,
    width: f32,
    dim: f32,
    radius: f32,
    spread: f32,
    circle: bool,
    placing: bool,
    clear: u32,
    undo: u32,
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
    pub width: f32,
    /// Spotlight shade, from 0 (clear) to 1 (solid).
    pub dim: f32,
    /// Corner radius of a highlight, in CSS pixels.
    pub radius: f32,
    /// Follow circle diameter for the cursor tool, in CSS pixels.
    pub spread: f32,
    /// Draws the large circle around the pointer while the cursor tool is selected.
    pub circle: bool,
    pub placing: bool,
    pub clear: u32,
    pub undo: u32,
    pub spotlight: Option<PresentRect>,
    pub zoom: Option<PresentRect>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresentPointer {
    pub x: f64,
    pub y: f64,
    pub inside: bool,
    pub lens: Option<PresentRect>,
}

/// Glass, zoom, and source for a locked selection. Rects are fractions of the window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LockLens {
    pub glass: PresentRect,
    pub zoom: f32,
    pub source: PresentRect,
}

struct ScreenBox {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

pub fn is_on() -> bool {
    LIVE.load(Ordering::Acquire)
}

pub fn toggle(app: &AppHandle) -> Result<PresentState, String> {
    if is_on() {
        close(app)?;
    } else {
        if crate::recording::running() {
            crate::recording::stop().map(|_| ())?;
        }
        open(app)?;
    }
    Ok(state())
}

pub fn state() -> PresentState {
    snapshot(
        &SESSION
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
    )
}

pub fn set_tool(app: &AppHandle, tool: &str) -> Result<PresentState, String> {
    let tool = known_tool(tool)?;
    mutate(app, |live| {
        if live.tool == tool && (tool == "spotlight" || tool == "cursor" || tool == "zoom") {
            let locked = if tool == "zoom" {
                live.zoom.take()
            } else {
                live.spotlight.take()
            };
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

/// True when Escape should leave the current tool for the crosshair.
pub fn escape_is_useful() -> bool {
    if !LIVE.load(Ordering::Acquire) {
        return false;
    }
    if HOLD_ESCAPE.load(Ordering::Acquire) {
        return true;
    }
    let Ok(slot) = SESSION.try_lock() else {
        return true;
    };
    slot.as_ref()
        .is_some_and(|live| live.tool != "cursor" || live.cursor != "crosshair" || live.placing)
}

pub fn on_delete(app: &AppHandle) {
    let _ = app.emit("present-delete", ());
}

pub fn delete_is_useful() -> bool {
    if !LIVE.load(Ordering::Acquire) {
        return false;
    }
    let Ok(slot) = SESSION.try_lock() else {
        return true;
    };
    slot.as_ref().is_some_and(|live| live.tool == "lasso")
}

pub fn on_escape(app: &AppHandle) {
    if !LIVE.load(Ordering::Acquire) {
        return;
    }
    if HOLD_ESCAPE.load(Ordering::Acquire) {
        let _ = app.emit("present-escape", ());
    }
    let _ = release_cursor(app);
}

/// Leaves the cursor tool with the crosshair selected. Present and its palette stay open.
pub fn release_cursor(app: &AppHandle) -> Result<PresentState, String> {
    mutate(app, |live| {
        live.tool = "cursor".into();
        live.cursor = "crosshair".into();
        live.placing = false;
    })
}

pub fn set_color(app: &AppHandle, color: &str) -> Result<PresentState, String> {
    let color = known_color(color)?;
    mutate(app, |live| live.color = color)
}

pub fn set_width(app: &AppHandle, width: f32) -> Result<PresentState, String> {
    let width = known_width(width)?;
    mutate(app, |live| live.width = width)
}

pub fn set_dim(app: &AppHandle, dim: f32) -> Result<PresentState, String> {
    let dim = known_dim(dim)?;
    mutate(app, |live| live.dim = dim)
}

pub fn set_radius(app: &AppHandle, radius: f32) -> Result<PresentState, String> {
    let radius = known_radius(radius)?;
    mutate(app, |live| live.radius = radius)
}

pub fn set_spread(app: &AppHandle, spread: f32) -> Result<PresentState, String> {
    let spread = known_spread(spread)?;
    mutate(app, |live| live.spread = spread)
}

pub fn set_circle(app: &AppHandle, circle: bool) -> Result<PresentState, String> {
    mutate(app, |live| live.circle = circle)
}

pub fn undo_mark(app: &AppHandle) -> Result<PresentState, String> {
    mutate(app, |live| live.undo = live.undo.wrapping_add(1))
}

/// While a label or a stroke is open, Escape cancels that instead of closing Present.
pub fn set_hold_escape(hold: bool) {
    HOLD_ESCAPE.store(hold, Ordering::Relaxed);
}

/// Stores a dragged spotlight or zoom rectangle. `None` ends the drag without a new lock.
pub fn finish_place(
    app: &AppHandle,
    kind: &str,
    rect: Option<PresentRect>,
) -> Result<PresentState, String> {
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
    let mut slot = SESSION
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
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

pub fn close(app: &AppHandle) -> Result<(), String> {
    LIVE.store(false, Ordering::Release);
    on_main(app, |app| close_main(app))?;
    Ok(())
}

fn on_main<R: Send + 'static>(
    app: &AppHandle,
    task: impl FnOnce(&AppHandle) -> R + Send + 'static,
) -> Result<R, String> {
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
    let owner = app
        .get_window(tabs::WINDOW)
        .ok_or("The browser window is missing.")?;
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
        let mut slot = SESSION
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *slot = Some(LiveSession {
            tool: "spotlight".into(),
            cursor: "dot".into(),
            color: "#f59e0b".into(),
            width: 1.0,
            dim: 0.62,
            radius: 12.0,
            spread: 240.0,
            circle: false,
            placing: false,
            clear: 0,
            undo: 0,
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
    HOLD_ESCAPE.store(false, Ordering::Relaxed);
    FAST_POINTER.store(false, Ordering::Relaxed);
    BLANK_CURSOR.store(false, Ordering::Relaxed);
    FOCUSED.store(true, Ordering::Relaxed);
    ESCAPE_REQUESTED.store(false, Ordering::Relaxed);
    ESCAPE_DOWN.store(false, Ordering::Relaxed);
    DELETE_REQUESTED.store(false, Ordering::Relaxed);
    if let Ok(mut last) = LAST_LAYOUT.lock() {
        *last = None;
    }
    for label in [STAGE, CONTROLS] {
        if let Some(window) = app.get_window(label) {
            let _ = window.close();
        }
    }
    let mut slot = SESSION
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
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
                    if LIVE.load(Ordering::Acquire)
                        && GENERATION.load(Ordering::Relaxed) == generation
                    {
                        sync(&inside);
                    }
                });
                if posted.is_err() {
                    POSTED.store(false, Ordering::Release);
                }
            }
            let wait = if FAST_POINTER.load(Ordering::Relaxed) {
                8
            } else {
                32
            };
            thread::sleep(Duration::from_millis(wait));
        }
    });
}

fn sync(app: &AppHandle) {
    // The low-level hook and WebView2 only see some Escape presses. The key state
    // itself is visible here on every tick, which is the same loop that moves the cursor.
    poll_escape(app);
    if ESCAPE_REQUESTED.swap(false, Ordering::AcqRel) {
        on_escape(app);
    }
    if DELETE_REQUESTED.swap(false, Ordering::AcqRel) {
        on_delete(app);
    }
    let focused = browser_focused(app);
    let was = FOCUSED.swap(focused, Ordering::Relaxed);
    if !focused {
        if was {
            hide_overlay(app);
        }
        return;
    }
    if !was {
        reveal_overlay(app);
    }
    let Some(box_) = window_box(app, tabs::WINDOW) else {
        return;
    };
    if layout_changed(&box_) {
        apply_layout(app);
    }
    let tick = BLANK_TICK.fetch_add(1, Ordering::Relaxed);
    if tick % 40 == 0 {
        if let Some(owner) = app.get_window(tabs::WINDOW) {
            blank_cursor_tree(&owner);
        }
    }
    let scale = app
        .get_window(STAGE)
        .and_then(|window| window.scale_factor().ok())
        .unwrap_or(1.0);
    let mut pointer = pointer_at(&box_, scale);
    let slot = SESSION
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(live) = slot.as_ref() else {
        return;
    };
    sync_flags(live);
    let tool = live.tool.clone();
    let zoom_lock = live.zoom;
    drop(slot);
    let lens = place_lens(app, &box_, scale, pointer, &tool, zoom_lock);
    pointer.lens = if tool == "zoom" { lens } else { None };
    let _ = app.emit("present-pointer", pointer);
    pass_stage_clicks(app);
    apply_click_through(app);
    raise_controls(app);
}

fn browser_focused(app: &AppHandle) -> bool {
    let foreground = unsafe { GetForegroundWindow() };
    if foreground.0.is_null() {
        return false;
    }
    for label in [tabs::WINDOW, STAGE, CONTROLS] {
        let Some(window) = app.get_window(label) else {
            continue;
        };
        let Ok(root) = window.hwnd() else {
            continue;
        };
        if owned_by(foreground, root) {
            return true;
        }
    }
    false
}

fn owned_by(hwnd: HWND, root: HWND) -> bool {
    if hwnd.0.is_null() {
        return false;
    }
    if hwnd == root || unsafe { GetAncestor(hwnd, GA_ROOT) } == root {
        return true;
    }
    let mut current = hwnd;
    for _ in 0..16 {
        if current == root {
            return true;
        }
        current = match unsafe { GetParent(current) } {
            Ok(parent) if !parent.0.is_null() && parent != current => parent,
            _ => return false,
        };
    }
    false
}

fn hide_overlay(app: &AppHandle) {
    hide_lens();
    BLANK_CURSOR.store(false, Ordering::Relaxed);
    for label in [STAGE, CONTROLS] {
        if let Some(window) = app.get_window(label) {
            let _ = window.hide();
        }
    }
}

fn reveal_overlay(app: &AppHandle) {
    show(app, STAGE);
    show(app, CONTROLS);
    apply_layout(app);
}

fn raise_controls(app: &AppHandle) {
    if let Some(controls) = app.get_window(CONTROLS) {
        above_top(&controls);
    }
}

fn apply_layout(app: &AppHandle) {
    let Some(box_) = window_box(app, tabs::WINDOW) else {
        return;
    };
    if let Ok(mut last) = LAST_LAYOUT.lock() {
        *last = Some(ScreenBox {
            x: box_.x,
            y: box_.y,
            width: box_.width,
            height: box_.height,
        });
    }
    if let Some(stage) = app.get_window(STAGE) {
        place_window(
            &stage,
            box_.x,
            box_.y,
            box_.width as u32,
            box_.height as u32,
        );
        pass_clicks(&stage);
    }
    if let Some(controls) = app.get_window(CONTROLS) {
        let scale = controls.scale_factor().unwrap_or(1.0);
        let width = (780.0 * scale).round().max(1.0) as i32;
        let height = (84.0 * scale).round().max(1.0) as i32;
        let x = box_.x + (box_.width - width) / 2;
        let y = box_.y + box_.height - height - (28.0 * scale).round() as i32;
        place_window(&controls, x, y, width as u32, height as u32);
        if let Ok(hwnd) = controls.hwnd() {
            CONTROLS_HWND.store(hwnd.0 as isize, Ordering::Relaxed);
        }
    }
    if let Some(owner) = app.get_window(tabs::WINDOW) {
        blank_cursor_tree(&owner);
    }
    restack(app);
}

fn layout_changed(box_: &ScreenBox) -> bool {
    let mut last = LAST_LAYOUT
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let changed = match last.as_ref() {
        Some(prev) => {
            prev.x != box_.x
                || prev.y != box_.y
                || prev.width != box_.width
                || prev.height != box_.height
        }
        None => true,
    };
    if changed {
        *last = Some(ScreenBox {
            x: box_.x,
            y: box_.y,
            width: box_.width,
            height: box_.height,
        });
    }
    changed
}

fn pointer_at(box_: &ScreenBox, scale: f64) -> PresentPointer {
    let mut point = POINT::default();
    let read = unsafe { GetCursorPos(&mut point) }.is_ok();
    let scale = if scale < 0.5 { 1.0 } else { scale };
    if !read {
        return PresentPointer {
            x: 0.0,
            y: 0.0,
            inside: false,
            lens: None,
        };
    }
    let inside = point.x >= box_.x
        && point.y >= box_.y
        && point.x < box_.x + box_.width
        && point.y < box_.y + box_.height;
    PresentPointer {
        x: (point.x - box_.x) as f64 / scale,
        y: (point.y - box_.y) as f64 / scale,
        inside,
        lens: None,
    }
}

fn place_lens(
    app: &AppHandle,
    box_: &ScreenBox,
    scale: f64,
    pointer: PresentPointer,
    tool: &str,
    lock: Option<PresentRect>,
) -> Option<PresentRect> {
    let hwnd = LENS_HWND.load(Ordering::Relaxed);
    let mag = MAG_HWND.load(Ordering::Relaxed);
    if hwnd == 0 || mag == 0 || !MAG_READY.load(Ordering::Relaxed) {
        return None;
    }
    if tool != "zoom" {
        hide_lens();
        return None;
    }
    let scale = if scale < 0.5 { 1.0 } else { scale };
    let inset = (LENS_INSET * scale).round().max(1.0) as i32;
    let placed = match lock {
        Some(rect) => lock_lens(rect, box_.width, box_.height, inset).map(|lens| {
            let glass = fraction_box(&lens.glass, box_);
            let source = fraction_box(&lens.source, box_);
            (glass, lens.zoom, source)
        }),
        None if pointer.inside => {
            let width = (LENS_W * scale).round().max(1.0) as i32;
            let height = (LENS_H * scale).round().max(1.0) as i32;
            let x = (box_.x as f64 + pointer.x * scale - width as f64 / 2.0).round() as i32;
            let y = (box_.y as f64 + pointer.y * scale - height as f64 / 2.0).round() as i32;
            let glass = ScreenBox {
                x: x.clamp(box_.x, (box_.x + box_.width - width).max(box_.x)),
                y: y.clamp(box_.y, (box_.y + box_.height - height).max(box_.y)),
                width: width.min(box_.width).max(1),
                height: height.min(box_.height).max(1),
            };
            let aim_x = box_.x + (pointer.x * scale) as i32;
            let aim_y = box_.y + (pointer.y * scale) as i32;
            let (sx, sy, sw, sh) = zoom_source(
                glass.x,
                glass.y,
                glass.width,
                glass.height,
                aim_x,
                aim_y,
                ZOOM,
                (box_.x, box_.y, box_.width, box_.height),
            );
            Some((
                glass,
                ZOOM,
                ScreenBox {
                    x: sx,
                    y: sy,
                    width: sw,
                    height: sh,
                },
            ))
        }
        None => None,
    };
    let Some((lens, zoom, source)) = placed else {
        hide_lens();
        return None;
    };
    if lens.width <= inset * 2 || lens.height <= inset * 2 {
        hide_lens();
        return None;
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
        let _ = SetWindowPos(
            glass,
            None,
            0,
            0,
            lens.width - inset * 2,
            lens.height - inset * 2,
            SWP_NOACTIVATE | SWP_NOMOVE,
        );
    }
    let mut transform = MAGTRANSFORM { v: [0.0; 9] };
    transform.v[0] = zoom;
    transform.v[4] = zoom;
    transform.v[8] = 1.0;
    unsafe {
        let _ = MagSetWindowTransform(glass, &mut transform);
        let _ = MagSetWindowSource(
            glass,
            RECT {
                left: source.x,
                top: source.y,
                right: source.x + source.width,
                bottom: source.y + source.height,
            },
        );
    }
    exclude_overlay(glass, app);
    cover_lens(app);
    Some(window_fraction(&lens, box_))
}

fn fraction_box(rect: &PresentRect, window: &ScreenBox) -> ScreenBox {
    ScreenBox {
        x: window.x + (rect.x * window.width as f32).round() as i32,
        y: window.y + (rect.y * window.height as f32).round() as i32,
        width: (rect.width * window.width as f32).round().max(1.0) as i32,
        height: (rect.height * window.height as f32).round().max(1.0) as i32,
    }
}

fn window_fraction(glass: &ScreenBox, window: &ScreenBox) -> PresentRect {
    let width = window.width.max(1) as f32;
    let height = window.height.max(1) as f32;
    PresentRect {
        x: (glass.x - window.x) as f32 / width,
        y: (glass.y - window.y) as f32 / height,
        width: glass.width as f32 / width,
        height: glass.height as f32 / height,
    }
}

fn exclude_overlay(glass: HWND, app: &AppHandle) {
    let mut list = Vec::new();
    for label in [STAGE, CONTROLS] {
        if let Some(hwnd) = app.get_window(label).and_then(|window| window.hwnd().ok()) {
            list.push(hwnd);
            unsafe {
                let _ = EnumChildWindows(
                    Some(hwnd),
                    Some(collect_filter_child),
                    LPARAM(&mut list as *mut Vec<HWND> as isize),
                );
            }
        }
    }
    if list.is_empty() {
        return;
    }
    unsafe {
        let _ = MagSetWindowFilterList(
            glass,
            MW_FILTERMODE_EXCLUDE,
            list.len() as i32,
            list.as_mut_ptr(),
        );
    }
}

unsafe extern "system" fn collect_filter_child(child: HWND, param: LPARAM) -> BOOL {
    let list = unsafe { &mut *(param.0 as *mut Vec<HWND>) };
    list.push(child);
    unsafe {
        let _ = EnumChildWindows(Some(child), Some(collect_filter_child), param);
    }
    BOOL(1)
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
pub fn zoom_source(
    lens_x: i32,
    lens_y: i32,
    lens_w: i32,
    lens_h: i32,
    aim_x: i32,
    aim_y: i32,
    zoom: f32,
    bounds: (i32, i32, i32, i32),
) -> (i32, i32, i32, i32) {
    let _ = (lens_x, lens_y);
    let zoom = if zoom < 1.05 { 1.05 } else { zoom };
    let sw = ((lens_w as f32) / zoom).round().max(1.0) as i32;
    let sh = ((lens_h as f32) / zoom).round().max(1.0) as i32;
    let (bx, by, bw, bh) = bounds;
    let x = if sw >= bw {
        bx
    } else {
        (aim_x - sw / 2).clamp(bx, bx + bw - sw)
    };
    let y = if sh >= bh {
        by
    } else {
        (aim_y - sh / 2).clamp(by, by + bh - sh)
    };
    (x, y, sw, sh)
}

/// Glass that shows `selection` at up to 2x. `inset` is the frame around the magnifier, in pixels.
/// A selection larger than the window shrinks the zoom until the whole selection still fits.
pub fn lock_lens(
    selection: PresentRect,
    window_w: i32,
    window_h: i32,
    inset: i32,
) -> Option<LockLens> {
    if window_w < 1 || window_h < 1 || !selection.width.is_finite() || !selection.height.is_finite()
    {
        return None;
    }
    let max_w = window_w as f32;
    let max_h = window_h as f32;
    let sw = (selection.width * max_w).round().max(1.0);
    let sh = (selection.height * max_h).round().max(1.0);
    let inset = inset.max(0) as f32;
    let mut inner_w = (sw * 2.0).round().max(1.0);
    let mut inner_h = (sh * 2.0).round().max(1.0);
    let room_w = (max_w - inset * 2.0).max(1.0);
    let room_h = (max_h - inset * 2.0).max(1.0);
    if inner_w > room_w || inner_h > room_h {
        let fit = (room_w / inner_w).min(room_h / inner_h);
        inner_w = (inner_w * fit).round().max(1.0);
        inner_h = (inner_h * fit).round().max(1.0);
    }
    inner_w = inner_w.min(room_w);
    inner_h = inner_h.min(room_h);
    let zoom = (inner_w / sw).min(inner_h / sh).max(1.0);
    let source_w = (inner_w / zoom).clamp(1.0, max_w);
    let source_h = (inner_h / zoom).clamp(1.0, max_h);
    let sel_x = (selection.x * max_w).clamp(0.0, max_w);
    let sel_y = (selection.y * max_h).clamp(0.0, max_h);
    let cx = sel_x + sw / 2.0;
    let cy = sel_y + sh / 2.0;
    let source_x = contain(cx - source_w / 2.0, source_w, sel_x, sw, max_w);
    let source_y = contain(cy - source_h / 2.0, source_h, sel_y, sh, max_h);
    let outer_w = (inner_w + inset * 2.0).min(max_w).max(1.0);
    let outer_h = (inner_h + inset * 2.0).min(max_h).max(1.0);
    let glass_x = (cx - outer_w / 2.0).clamp(0.0, (max_w - outer_w).max(0.0));
    let glass_y = (cy - outer_h / 2.0).clamp(0.0, (max_h - outer_h).max(0.0));
    Some(LockLens {
        glass: PresentRect {
            x: glass_x / max_w,
            y: glass_y / max_h,
            width: outer_w / max_w,
            height: outer_h / max_h,
        },
        zoom,
        source: PresentRect {
            x: source_x / max_w,
            y: source_y / max_h,
            width: source_w / max_w,
            height: source_h / max_h,
        },
    })
}

fn contain(origin: f32, size: f32, cover: f32, cover_size: f32, limit: f32) -> f32 {
    let mut origin = origin;
    if origin > cover {
        origin = cover;
    }
    if origin + size < cover + cover_size {
        origin = cover + cover_size - size;
    }
    origin.clamp(0.0, (limit - size).max(0.0))
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
    let instance = unsafe { windows::Win32::System::LibraryLoader::GetModuleHandleW(None) }
        .map_err(|error| error.to_string())?;
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

unsafe extern "system" fn lens_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

fn register_lens_class() -> Result<(), String> {
    let instance = unsafe { windows::Win32::System::LibraryLoader::GetModuleHandleW(None) }
        .map_err(|error| error.to_string())?;
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
    window
        .add_child(webview, PhysicalPosition::new(0, 0), size)
        .map_err(|error| error.to_string())?;
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
    let lens = LENS_HWND.load(Ordering::Relaxed);
    if lens != 0 {
        unsafe {
            let _ = SetWindowPos(
                HWND(lens as *mut core::ffi::c_void),
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }
    cover_lens(app);
    if lens == 0 {
        if let Some(stage) = app.get_window(STAGE) {
            above_top(&stage);
        }
    }
    if let Some(controls) = app.get_window(CONTROLS) {
        above_top(&controls);
    }
}

fn cover_lens(app: &AppHandle) {
    let lens = LENS_HWND.load(Ordering::Relaxed);
    if lens == 0 {
        return;
    }
    let after = HWND(lens as *mut core::ffi::c_void);
    if let Some(stage) = app.get_window(STAGE).and_then(|window| window.hwnd().ok()) {
        unsafe {
            let _ = SetWindowPos(
                after,
                Some(stage),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }
    if let Some(controls) = app.get_window(CONTROLS) {
        above_top(&controls);
    }
}

fn above_top(window: &Window) {
    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
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
    Some(ScreenBox {
        x: rect.left,
        y: rect.top,
        width,
        height,
    })
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
        let _ = EnumChildWindows(Some(child), Some(pass_child), LPARAM(0));
    }
    BOOL(1)
}

fn pass_stage_clicks(app: &AppHandle) {
    if let Some(stage) = app.get_window(STAGE) {
        pass_clicks(&stage);
    }
}

/// Spotlight and cursor stay click-through. A drawing tool, or Alt while locking, takes the click.
fn apply_click_through(app: &AppHandle) {
    let Some(stage) = app.get_window(STAGE) else {
        return;
    };
    let through =
        !(CAPTURE.load(Ordering::Relaxed) || (PLACEABLE.load(Ordering::Relaxed) && alt_down()));
    let _ = stage.set_ignore_cursor_events(through);
    let Ok(hwnd) = stage.hwnd() else {
        return;
    };
    set_click_through(hwnd, through);
    unsafe {
        let _ = EnumChildWindows(
            Some(hwnd),
            Some(through_child),
            LPARAM(isize::from(through)),
        );
    }
}

unsafe extern "system" fn through_child(child: HWND, param: LPARAM) -> BOOL {
    set_click_through(child, param.0 != 0);
    unsafe {
        let _ = EnumChildWindows(Some(child), Some(through_child), param);
    }
    BOOL(1)
}

fn set_click_through(hwnd: HWND, through: bool) {
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let flag = WS_EX_TRANSPARENT.0;
        let next = if through { style | flag } else { style & !flag };
        if next == style {
            return;
        }
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next as isize);
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
}

unsafe extern "system" fn stage_hit_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    _: usize,
) -> LRESULT {
    if message == WM_NCHITTEST {
        let point = hit_point(lparam);
        if over_controls(point)
            || !(CAPTURE.load(Ordering::Relaxed)
                || (PLACEABLE.load(Ordering::Relaxed) && alt_down()))
        {
            return LRESULT(HTTRANSPARENT as isize);
        }
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

fn alt_down() -> bool {
    unsafe { GetAsyncKeyState(VK_MENU.0 as i32) < 0 }
}

fn poll_escape(app: &AppHandle) {
    let down = unsafe { GetAsyncKeyState(VK_ESCAPE.0 as i32) < 0 };
    let was = ESCAPE_DOWN.swap(down, Ordering::AcqRel);
    if down && !was && escape_is_useful() {
        on_escape(app);
    }
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
    let Ok(installed) = (unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(escape_proc), None, 0) })
    else {
        eprintln!("present: Escape hook was not installed");
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
        if info.vkCode == VK_ESCAPE.0 as u32 && escape_is_useful() {
            ESCAPE_REQUESTED.store(true, Ordering::Release);
            return LRESULT(1);
        }
        let deleting = info.vkCode == VK_DELETE.0 as u32 || info.vkCode == VK_BACK.0 as u32;
        if deleting && delete_is_useful() {
            DELETE_REQUESTED.store(true, Ordering::Release);
            return LRESULT(1);
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn sync_flags(live: &LiveSession) {
    let draw = matches!(
        live.tool.as_str(),
        "lasso" | "highlight" | "pen" | "arrow" | "text" | "eraser"
    );
    let capture = draw || live.placing;
    CAPTURE.store(capture, Ordering::Relaxed);
    PLACEABLE.store(
        matches!(live.tool.as_str(), "spotlight" | "cursor" | "zoom"),
        Ordering::Relaxed,
    );
    let follow_spot = matches!(live.tool.as_str(), "spotlight" | "cursor")
        && live.spotlight.is_none()
        && !live.placing;
    let follow_zoom = live.tool == "zoom" && live.zoom.is_none() && !live.placing;
    FAST_POINTER.store(
        live.cursor != "off" || follow_spot || follow_zoom,
        Ordering::Relaxed,
    );
    let hide_pointer = live.tool == "zoom" && !capture;
    BLANK_CURSOR.store(
        hide_pointer || (live.cursor != "off" && !capture),
        Ordering::Relaxed,
    );
}

fn blank_cursor_tree(window: &Window) {
    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    unsafe {
        let _ = SetWindowSubclass(hwnd, Some(cursor_proc), CURSOR_PASS, 0);
        let _ = EnumChildWindows(Some(hwnd), Some(blank_child), LPARAM(0));
    }
}

unsafe extern "system" fn blank_child(child: HWND, _: LPARAM) -> BOOL {
    if in_controls(child) {
        return BOOL(1);
    }
    unsafe {
        let _ = SetWindowSubclass(child, Some(cursor_proc), CURSOR_PASS, 0);
        let _ = EnumChildWindows(Some(child), Some(blank_child), LPARAM(0));
    }
    BOOL(1)
}

fn in_controls(hwnd: HWND) -> bool {
    let raw = CONTROLS_HWND.load(Ordering::Relaxed);
    if raw == 0 {
        return false;
    }
    owned_by(hwnd, HWND(raw as *mut core::ffi::c_void))
}

unsafe extern "system" fn cursor_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    _: usize,
) -> LRESULT {
    let blank = message == WM_SETCURSOR
        && BLANK_CURSOR.load(Ordering::Relaxed)
        && LIVE.load(Ordering::Relaxed)
        && FOCUSED.load(Ordering::Relaxed);
    if blank && !in_controls(hwnd) {
        let hit = (lparam.0 as u32) & 0xffff;
        if hit == HTCLIENT {
            unsafe {
                let _ = SetCursor(None);
            }
            return LRESULT(1);
        }
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

fn snapshot(slot: &Option<LiveSession>) -> PresentState {
    match slot {
        Some(live) => PresentState {
            on: true,
            tool: live.tool.clone(),
            cursor: live.cursor.clone(),
            color: live.color.clone(),
            width: live.width,
            dim: live.dim,
            radius: live.radius,
            spread: live.spread,
            circle: live.circle,
            placing: live.placing,
            clear: live.clear,
            undo: live.undo,
            spotlight: live.spotlight,
            zoom: live.zoom,
        },
        None => PresentState {
            on: false,
            tool: "spotlight".into(),
            cursor: "dot".into(),
            color: "#f59e0b".into(),
            width: 1.0,
            dim: 0.62,
            radius: 12.0,
            spread: 240.0,
            circle: false,
            placing: false,
            clear: 0,
            undo: 0,
            spotlight: None,
            zoom: None,
        },
    }
}

fn known_tool(tool: &str) -> Result<String, String> {
    match tool {
        "none" | "lasso" | "spotlight" | "cursor" | "zoom" | "highlight" | "pen" | "arrow"
        | "text" | "eraser" => Ok(tool.to_string()),
        _ => Err("Unknown present tool.".into()),
    }
}

fn known_cursor(cursor: &str) -> Result<String, String> {
    match cursor {
        "dot" | "ring" | "arrow" | "crosshair" | "off" => Ok(cursor.to_string()),
        _ => Err("Unknown present cursor.".into()),
    }
}

fn known_spread(spread: f32) -> Result<f32, String> {
    if !spread.is_finite() {
        return Err("Present spread is a circle size in pixels.".into());
    }
    Ok(spread.clamp(64.0, 560.0))
}

fn known_radius(radius: f32) -> Result<f32, String> {
    if !radius.is_finite() {
        return Err("Present radius is a corner size in pixels.".into());
    }
    Ok(radius.clamp(0.0, 48.0))
}

fn known_dim(dim: f32) -> Result<f32, String> {
    if !dim.is_finite() {
        return Err("Present dim is a fraction from 0 to 1.".into());
    }
    Ok(dim.clamp(0.0, 0.92))
}

fn known_width(width: f32) -> Result<f32, String> {
    const CHOICES: [f32; 3] = [0.32, 1.0, 2.4];
    CHOICES
        .into_iter()
        .find(|choice| (choice - width).abs() < 0.02)
        .ok_or_else(|| "Present width is a stroke size.".into())
}

fn known_color(color: &str) -> Result<String, String> {
    let bytes = color.as_bytes();
    if bytes.len() == 7
        && bytes[0] == b'#'
        && bytes[1..].iter().all(|byte| byte.is_ascii_hexdigit())
    {
        Ok(color.to_ascii_lowercase())
    } else {
        Err("Present color is a #rrggbb value.".into())
    }
}

fn clean_rect(rect: PresentRect) -> Option<PresentRect> {
    if !rect.x.is_finite()
        || !rect.y.is_finite()
        || !rect.width.is_finite()
        || !rect.height.is_finite()
    {
        return None;
    }
    let x = rect.x.min(rect.x + rect.width).clamp(0.0, 1.0);
    let y = rect.y.min(rect.y + rect.height).clamp(0.0, 1.0);
    let width = rect.width.abs();
    let height = rect.height.abs();
    if width.max(height) < 0.01 {
        return None;
    }
    Some(PresentRect {
        x,
        y,
        width: width.min(1.0 - x).max(0.002),
        height: height.min(1.0 - y).max(0.002),
    })
}

#[cfg(test)]
mod tests {
    use super::{PresentRect, clean_rect, lock_lens, zoom_source};

    fn covers(outer: PresentRect, inner: PresentRect) -> bool {
        outer.x <= inner.x + 0.01
            && outer.y <= inner.y + 0.01
            && outer.x + outer.width + 0.01 >= inner.x + inner.width
            && outer.y + outer.height + 0.01 >= inner.y + inner.height
    }

    #[test]
    fn zoom_source_is_the_lens_divided_by_the_factor_and_kept_on_the_window() {
        let source = zoom_source(100, 80, 200, 120, 200, 140, 2.0, (0, 0, 1000, 800));
        assert_eq!(source, (150, 110, 100, 60));
        let clamped = zoom_source(0, 0, 200, 120, 10, 10, 2.0, (0, 0, 1000, 800));
        assert_eq!(clamped, (0, 0, 100, 60));
    }

    #[test]
    fn lock_lens_doubles_a_small_selection_and_keeps_it_in_view() {
        let selection = PresentRect {
            x: 0.2,
            y: 0.2,
            width: 0.1,
            height: 0.1,
        };
        let lens = lock_lens(selection, 1000, 800, 0).expect("lens");
        assert!((lens.zoom - 2.0).abs() < 0.05, "zoom {}", lens.zoom);
        assert!(
            (lens.glass.width - 0.2).abs() < 0.02,
            "glass width {}",
            lens.glass.width
        );
        assert!(
            (lens.glass.height - 0.2).abs() < 0.02,
            "glass height {}",
            lens.glass.height
        );
        assert!(lens.glass.x >= 0.0 && lens.glass.y >= 0.0);
        assert!(lens.glass.x + lens.glass.width <= 1.01);
        assert!(lens.glass.y + lens.glass.height <= 1.01);
        assert!(
            covers(lens.source, selection),
            "source {:?} selection {:?}",
            lens.source,
            selection
        );
    }

    #[test]
    fn lock_lens_shrinks_zoom_so_a_large_selection_still_fits() {
        let selection = PresentRect {
            x: 0.0,
            y: 0.0,
            width: 0.8,
            height: 0.8,
        };
        let lens = lock_lens(selection, 1000, 800, 0).expect("lens");
        assert!(lens.zoom < 2.0, "zoom {}", lens.zoom);
        assert!(lens.glass.x >= -0.001 && lens.glass.y >= -0.001);
        assert!(lens.glass.x + lens.glass.width <= 1.01);
        assert!(lens.glass.y + lens.glass.height <= 1.01);
        assert!(
            covers(lens.source, selection),
            "source {:?} selection {:?}",
            lens.source,
            selection
        );
    }

    #[test]
    fn clean_rect_keeps_a_thin_bar_and_drops_a_dot() {
        let bar = clean_rect(PresentRect {
            x: 0.1,
            y: 0.4,
            width: 0.5,
            height: 0.0,
        })
        .expect("bar");
        assert!(bar.width > 0.4);
        assert!(bar.height > 0.0);
        assert!(
            clean_rect(PresentRect {
                x: 0.2,
                y: 0.2,
                width: 0.004,
                height: 0.004
            })
            .is_none()
        );
    }
}
