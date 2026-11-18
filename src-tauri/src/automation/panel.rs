//! The side column beside the pages. The agent panel and the automations panel share it.
//!
//! Dragging its left edge calls `track_resize`. The pointer leaves the panel webview as the
//! column grows, so the drag follows the screen cursor until the mouse button comes up.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use tauri::webview::Color;
use tauri::window::WindowBuilder;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, PhysicalPosition, WebviewUrl};
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

use crate::{paths, settings, tabs, webview2};

/// The agent column stops here. The automations column may grow to half the page instead.
const AGENT_MAX: f64 = 720.0;

struct Drag {
    window: String,
    id: String,
    /// How far the pointer sits to the right of the edge being dragged, in CSS pixels.
    grab: Option<f64>,
    edge: tabs::PanelEdge,
    min: f64,
    max: f64,
    /// Width kept when the pointer reading is unusable.
    fallback: f64,
    reorder: bool,
}

/// Limits for one drag of a docked panel.
pub struct ColumnDrag {
    pub id: String,
    pub edge: tabs::PanelEdge,
    pub min: f64,
    pub max: f64,
    /// Keep the finished width as the agent panel's saved width.
    pub remember_agent: bool,
    pub fallback: f64,
}

const PANEL_CLOSED: &str = "The side panel is closed.";
const PANEL_BUSY: &str = "The side panel is already being resized.";

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

/// Opens or closes the agent panel in `window`. `open` forces a state. Returns whether it is open.
/// Other docked panels stay where they are.
pub fn toggle(app: &AppHandle, window: &str, open: Option<bool>) -> Result<bool, String> {
    toggle_kind(app, window, tabs::SidePanel::Agent, open, true)
}

/// Opens or closes the automations panel in `window`. `open` forces a state. Returns whether it is open.
pub fn toggle_automations(
    app: &AppHandle,
    window: &str,
    open: Option<bool>,
) -> Result<bool, String> {
    toggle_kind(app, window, tabs::SidePanel::Automations, open, true)
}

/// Opens the automations panel beside `window`'s pages. Leaves focus on the page, so recording
/// can keep receiving clicks.
pub fn show_automations(app: &AppHandle, window: &str) -> Result<(), String> {
    toggle_kind(app, window, tabs::SidePanel::Automations, Some(true), false).map(|_| ())
}

/// Opens or closes the scripting panel in `window`. `open` forces a state. Returns whether it is open.
pub fn toggle_scripting(app: &AppHandle, window: &str, open: Option<bool>) -> Result<bool, String> {
    toggle_kind(app, window, tabs::SidePanel::Scripting, open, true)
}

fn toggle_kind(
    app: &AppHandle,
    window: &str,
    kind: tabs::SidePanel,
    open: Option<bool>,
    focus: bool,
) -> Result<bool, String> {
    let id = kind.dock_id();
    let showing = tabs::dock_contains(app, window, id);
    let want = open.unwrap_or(!showing);
    if want {
        ensure_kind(app, window, kind)?;
        let min = panel_min(kind);
        tabs::open_dock_panel(
            app,
            window,
            id,
            kind.title(),
            tabs::PanelEdge::Right,
            fitted_width(app, window, kind, saved_width(app)),
            min,
            true,
        );
        if focus && tabs::dock_expanded(app, window, id) {
            let label = panel_label(window, kind);
            if let Some(view) = app.get_webview(&label) {
                let _ = view.set_focus();
            }
        }
    } else {
        tabs::close_dock_panel(app, window, id);
    }
    Ok(want)
}

fn panel_label(window: &str, kind: tabs::SidePanel) -> String {
    match kind {
        tabs::SidePanel::Automations => tabs::automation_panel_label(window),
        tabs::SidePanel::Scripting => tabs::scripting_panel_label(window),
        tabs::SidePanel::Host | tabs::SidePanel::Agent | tabs::SidePanel::None => {
            tabs::agent_panel_label(window)
        }
    }
}

fn ensure_kind(app: &AppHandle, window: &str, kind: tabs::SidePanel) -> Result<(), String> {
    match kind {
        tabs::SidePanel::Agent => {
            ensure_view(app, window, &tabs::agent_panel_label(window), "agent-panel")
        }
        tabs::SidePanel::Automations => ensure_view(
            app,
            window,
            &tabs::automation_panel_label(window),
            "automations-panel",
        ),
        tabs::SidePanel::Scripting => ensure_view(
            app,
            window,
            &tabs::scripting_panel_label(window),
            "scripting-panel",
        ),
        tabs::SidePanel::Host => Ok(()),
        tabs::SidePanel::None => Ok(()),
    }
}

fn saved_width(app: &AppHandle) -> f64 {
    settings::get(app).agent_panel_width as f64
}

/// The scripting editor needs more than the agent column's usual width.
const SCRIPTING_MIN: f64 = 480.0;

/// Saved column width, limited to what this panel may take in `window`.
fn fitted_width(app: &AppHandle, window: &str, kind: tabs::SidePanel, width: f64) -> f64 {
    let others = tabs::other_columns(app, window, kind.dock_id());
    let Some(geom) = geometry(app, window) else {
        let cap = if wide_column(kind) { 8192.0 } else { AGENT_MAX };
        return width.clamp(panel_min(kind), cap);
    };
    let span =
        (geom.client - geom.right.max(0.0) - geom.left.max(0.0) - others - tabs::PANEL_PAGE_MIN)
            .max(0.0);
    let floor = f64::from(settings::AGENT_PANEL_WIDTH_PX.0);
    let min = panel_min(kind).min(span.max(floor));
    width_for_edge(
        geom.client,
        geom.left,
        geom.right,
        geom.client - geom.right - width,
        0.0,
        min,
        column_max(kind, &geom),
        tabs::PanelEdge::Right,
        width.max(min),
        0.0,
        others,
    )
}

fn panel_min(kind: tabs::SidePanel) -> f64 {
    if kind == tabs::SidePanel::Scripting {
        SCRIPTING_MIN
    } else {
        f64::from(settings::AGENT_PANEL_WIDTH_PX.0)
    }
}

fn wide_column(kind: tabs::SidePanel) -> bool {
    kind == tabs::SidePanel::Automations || kind == tabs::SidePanel::Scripting
}

fn wide_id(id: &str) -> bool {
    id == tabs::AUTOMATIONS_PANEL_ID || id == tabs::SCRIPTING_PANEL_ID
}

/// Automations and scripting may cover half the area the page and the column share.
/// The agent panel stays narrower.
fn column_max(kind: tabs::SidePanel, geom: &Geom) -> f64 {
    column_max_id(kind.dock_id(), geom)
}

fn column_max_id(id: &str, geom: &Geom) -> f64 {
    if id == tabs::AGENT_PANEL_ID {
        return AGENT_MAX;
    }
    let span = (geom.client - geom.right.max(0.0) - geom.left.max(0.0)).max(0.0);
    if wide_id(id) {
        span * 0.5
    } else {
        span.max(AGENT_MAX)
    }
}

/// Follows the left button and resizes the agent panel until the button comes up.
pub fn track_resize(app: &AppHandle, window: &str) -> Result<(), String> {
    track_panel_resize(app, window, tabs::AGENT_PANEL_ID)
}

/// Follows the left button and resizes the docked panel `id`.
pub fn track_panel_resize(app: &AppHandle, window: &str, id: &str) -> Result<(), String> {
    let Some(entry) = tabs::dock_entry(app, window, id) else {
        return Ok(());
    };
    if entry.collapsed || !entry.resizable {
        return Ok(());
    }
    let (min, max, remember, fallback) = resize_limits(app, window, id, &entry);
    match track_column(
        app,
        window,
        ColumnDrag {
            id: id.to_string(),
            edge: entry.edge,
            min,
            max,
            remember_agent: remember,
            fallback,
        },
    ) {
        Ok(_) => Ok(()),
        Err(error) if error == PANEL_CLOSED || error == PANEL_BUSY => Ok(()),
        Err(error) => Err(error),
    }
}

fn resize_limits(
    app: &AppHandle,
    window: &str,
    id: &str,
    entry: &tabs::DockEntry,
) -> (f64, f64, bool, f64) {
    if let Some(extension) = tabs::extension_id_of(id) {
        let (min, max, fallback) = crate::host_extensions::extension_panel_limits(app, extension)
            .unwrap_or((entry.min, 8192.0, entry.width));
        return (min, max.max(min), false, fallback);
    }
    let max = geometry(app, window)
        .map(|geom| column_max_id(id, &geom))
        .unwrap_or(AGENT_MAX);
    let min = if id == tabs::SCRIPTING_PANEL_ID {
        SCRIPTING_MIN
    } else {
        f64::from(settings::AGENT_PANEL_WIDTH_PX.0)
    };
    (min, max.max(min), id == tabs::AGENT_PANEL_ID, entry.width)
}

/// Follows the pointer and reorders `id` among the panels on its edge.
pub fn track_reorder(app: &AppHandle, window: &str, id: &str) -> Result<(), String> {
    let Some(entry) = tabs::dock_entry(app, window, id) else {
        return Ok(());
    };
    if RESIZING.swap(true, Ordering::AcqRel) {
        return Ok(());
    }
    let _guard = ResizeGuard;
    {
        let mut slot = DRAG.lock().expect("panel reorder");
        *slot = Some(Drag {
            window: window.to_string(),
            id: id.to_string(),
            grab: None,
            edge: entry.edge,
            min: entry.min,
            max: entry.width,
            fallback: entry.width,
            reorder: true,
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
    tabs::emit_tabs(app);
    Ok(())
}

/// Follows the pointer until the left button comes up. Returns the width that was applied.
pub fn track_column(app: &AppHandle, window: &str, drag: ColumnDrag) -> Result<f64, String> {
    if tabs::dock_panel_width(app, window, &drag.id) <= 0.0 {
        return Err(PANEL_CLOSED.into());
    }
    if RESIZING.swap(true, Ordering::AcqRel) {
        return Err(PANEL_BUSY.into());
    }
    let _guard = ResizeGuard;
    {
        let mut slot = DRAG.lock().expect("panel resize");
        *slot = Some(Drag {
            window: window.to_string(),
            id: drag.id.clone(),
            grab: None,
            edge: drag.edge,
            min: drag.min,
            max: drag.max,
            fallback: drag.fallback,
            reorder: false,
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
    if drag.remember_agent {
        let (sender, receiver) = mpsc::channel();
        let finish = app.clone();
        app.run_on_main_thread(move || {
            remember_current(&finish);
            let _ = sender.send(());
        })
        .map_err(|error| error.to_string())?;
        receiver
            .recv_timeout(Duration::from_secs(2))
            .map_err(|_| "The panel resize did not finish.".to_string())?;
    }
    tabs::emit_tabs(app);
    Ok(tabs::dock_panel_width(app, window, &drag.id))
}

/// Sets one expanded panel to `width` CSS pixels. The agent panel's width is also saved in settings.
pub fn set_width(app: &AppHandle, window: &str, id: &str, width: f64) -> Result<(), String> {
    let Some(width) = clamped_width(app, window, id, width) else {
        return Ok(());
    };
    if let Some(applied) = tabs::resize_dock_panel(app, window, id, width) {
        if id == tabs::AGENT_PANEL_ID {
            remember(app, applied);
        }
        tabs::emit_tabs(app);
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
    let width = tabs::dock_panel_width(app, &window, tabs::AGENT_PANEL_ID);
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
    let (window, id, edge, min, max, fallback, reorder) = {
        let Ok(drag) = DRAG.lock() else {
            return;
        };
        let Some(drag) = drag.as_ref() else {
            return;
        };
        (
            drag.window.clone(),
            drag.id.clone(),
            drag.edge,
            drag.min,
            drag.max,
            drag.fallback,
            drag.reorder,
        )
    };
    if reorder {
        apply_reorder(app, &window, &id, edge);
        return;
    }
    let Some(geom) = geometry(app, &window) else {
        return;
    };
    let Some(cursor) = cursor_x(&geom) else {
        return;
    };
    let current = tabs::dock_panel_width(app, &window, &id);
    if current <= 0.0 {
        return;
    }
    let outer = tabs::outer_columns(app, &window, &id);
    let others = tabs::other_columns(app, &window, &id);
    let grab = {
        let Ok(mut drag) = DRAG.lock() else {
            return;
        };
        let Some(state) = drag.as_mut() else {
            return;
        };
        if state.grab.is_none() {
            state.grab = Some(match edge {
                tabs::PanelEdge::Right => {
                    let left = geom.client - geom.right - outer - current;
                    cursor - left
                }
                tabs::PanelEdge::Left => {
                    let panel_right = geom.left + outer + current;
                    cursor - panel_right
                }
            });
        }
        state.grab.unwrap_or(0.0)
    };
    let next = width_for_edge(
        geom.client,
        geom.left,
        geom.right,
        cursor,
        grab,
        min,
        max,
        edge,
        fallback,
        outer,
        others,
    );
    tabs::resize_dock_panel(app, &window, &id, next);
}

fn apply_reorder(app: &AppHandle, window: &str, id: &str, edge: tabs::PanelEdge) {
    let Some(geom) = geometry(app, window) else {
        return;
    };
    let Some(cursor) = cursor_x(&geom) else {
        return;
    };
    let panels = tabs::dock_panels(app, window);
    let others: Vec<&tabs::DockEntry> = panels
        .iter()
        .filter(|entry| entry.edge == edge && entry.id != id)
        .collect();
    if others.is_empty() {
        return;
    }
    let centers = edge_centers(edge, &others, &geom);
    let index = match edge {
        tabs::PanelEdge::Right => centers.iter().filter(|center| **center < cursor).count(),
        tabs::PanelEdge::Left => centers.iter().filter(|center| **center > cursor).count(),
    };
    tabs::reorder_dock_panel(app, window, id, index);
}

/// Centers of `others` in nearest-to-page order, in CSS pixels from the window's left.
fn edge_centers(edge: tabs::PanelEdge, others: &[&tabs::DockEntry], geom: &Geom) -> Vec<f64> {
    match edge {
        tabs::PanelEdge::Right => {
            let sum: f64 = others.iter().map(|entry| tabs::column_width(entry)).sum();
            let mut x = geom.client - geom.right.max(0.0) - sum;
            let mut centers = Vec::with_capacity(others.len());
            for entry in others {
                let width = tabs::column_width(entry);
                centers.push(x + width / 2.0);
                x += width;
            }
            centers
        }
        tabs::PanelEdge::Left => {
            let mut x = geom.left.max(0.0);
            let mut from_far = Vec::with_capacity(others.len());
            for entry in others.iter().rev() {
                let width = tabs::column_width(entry);
                from_far.push(x + width / 2.0);
                x += width;
            }
            from_far.reverse();
            from_far
        }
    }
}

fn clamped_width(app: &AppHandle, window: &str, id: &str, width: f64) -> Option<f64> {
    if tabs::dock_panel_width(app, window, id) <= 0.0 {
        return None;
    }
    let geom = geometry(app, window)?;
    let others = tabs::other_columns(app, window, id);
    let outer = tabs::outer_columns(app, window, id);
    let max = column_max_id(id, &geom);
    Some(width_for_edge(
        geom.client,
        geom.left,
        geom.right,
        geom.client - geom.right - outer - width,
        0.0,
        f64::from(settings::AGENT_PANEL_WIDTH_PX.0),
        max,
        tabs::PanelEdge::Right,
        width,
        outer,
        others,
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
#[cfg(test)]
fn width_beside_pointer(
    client: f64,
    left: f64,
    right: f64,
    cursor: f64,
    grab: f64,
    max: f64,
) -> f64 {
    width_for_edge(
        client,
        left,
        right,
        cursor,
        grab,
        f64::from(settings::AGENT_PANEL_WIDTH_PX.0),
        max,
        tabs::PanelEdge::Right,
        f64::from(settings::AGENT_PANEL_DEFAULT_WIDTH),
        0.0,
        0.0,
    )
}

/// Width of a column docked on `edge`. `grab` is how far the pointer sits past the dragged edge.
fn width_for_edge(
    client: f64,
    left: f64,
    right: f64,
    cursor: f64,
    grab: f64,
    min: f64,
    max: f64,
    edge: tabs::PanelEdge,
    fallback: f64,
    outer: f64,
    others: f64,
) -> f64 {
    let min = if min.is_finite() {
        min.max(0.0)
    } else {
        f64::from(settings::AGENT_PANEL_WIDTH_PX.0)
    };
    let max = if max.is_finite() {
        max.max(min)
    } else {
        AGENT_MAX.max(min)
    };
    let fallback = if fallback.is_finite() {
        fallback
    } else {
        f64::from(settings::AGENT_PANEL_DEFAULT_WIDTH)
    };
    if ![client, left, right, cursor, grab]
        .iter()
        .all(|value| value.is_finite())
    {
        return fallback.clamp(min, max);
    }
    let raw = match edge {
        tabs::PanelEdge::Right => {
            let panel_right = client - right.max(0.0) - outer.max(0.0);
            panel_right - (cursor - grab)
        }
        tabs::PanelEdge::Left => (cursor - grab) - left.max(0.0) - outer.max(0.0),
    };
    let room = client - right.max(0.0) - left.max(0.0) - others.max(0.0) - tabs::PANEL_PAGE_MIN;
    let cap = room.min(max).max(min);
    if !raw.is_finite() {
        return fallback.clamp(min, cap);
    }
    raw.clamp(min, cap)
}

fn ensure_view(app: &AppHandle, window: &str, label: &str, page: &str) -> Result<(), String> {
    if app.get_webview(label).is_some() {
        return Ok(());
    }
    let host = app
        .get_window(window)
        .ok_or("The browser window is missing.")?;
    let (r, g, b) = settings::popup_surface(app);
    let url = paths::internal_url(app, page);
    let builder = tabs::base_builder(app, label, WebviewUrl::External(url))
        .background_color(Color(r, g, b, 255))
        .focused(false);
    let view = host
        .add_child(
            builder,
            LogicalPosition::new(-10000.0, -10000.0),
            LogicalSize::new(saved_width(app), 400.0),
        )
        .map_err(|error| error.to_string())?;
    webview2::attach_panel_hooks(app, &view);
    Ok(())
}

const SCRIPTING_WINDOW: &str = "popup-scripting";

/// Opens the scripting panel in its own window. A second request brings that window forward.
pub fn open_scripting_window(app: &AppHandle, owner: &str) -> Result<(), String> {
    if let Some(existing) = app.get_window(SCRIPTING_WINDOW) {
        let _ = existing.unminimize();
        let _ = existing.show();
        let _ = existing.set_focus();
    } else {
        let window = WindowBuilder::new(app, SCRIPTING_WINDOW)
            .title("Scripting")
            .inner_size(960.0, 720.0)
            .min_inner_size(640.0, 420.0)
            .resizable(true)
            .decorations(true)
            .focused(true)
            .shadow(true)
            .build()
            .map_err(|error| error.to_string())?;
        crate::profiles::apply_window_icon(app, &window);
        if let Some(owner_window) = app.get_window(owner)
            && let Ok(position) = owner_window.outer_position()
        {
            let _ = window.set_position(PhysicalPosition::new(position.x + 48, position.y + 48));
        }
        let url = paths::internal_url(app, "scripting-panel");
        let host = app.clone();
        let view = tabs::base_builder(app, SCRIPTING_WINDOW, WebviewUrl::External(url))
            .auto_resize()
            .disable_drag_drop_handler()
            .background_color(tabs::popup_surface_color(app))
            .focused(true)
            .on_new_window(move |url, features| tabs::new_window(&host, None, url, features));
        let size = match window.inner_size() {
            Ok(size) => size,
            Err(error) => {
                let _ = window.close();
                return Err(error.to_string());
            }
        };
        let webview = match window.add_child(view, PhysicalPosition::new(0, 0), size) {
            Ok(webview) => webview,
            Err(error) => {
                let _ = window.close();
                return Err(error.to_string());
            }
        };
        webview2::attach_panel_hooks(app, &webview);
        let _ = window.set_focus();
    }
    if tabs::is_browser_label(owner) {
        let _ = toggle_scripting(app, owner, Some(false));
    }
    Ok(())
}

/// Reopens the dock saved in the session. Missing extensions are skipped.
pub fn restore_dock(app: &AppHandle, panels: &[tabs::DockRecord]) {
    for panel in panels {
        let id = panel.id.as_str();
        let width = if panel.width.is_finite() && panel.width >= 1.0 {
            panel.width
        } else {
            saved_width(app)
        };
        let opened = if let Some(kind) = builtin_kind(id) {
            ensure_kind(app, tabs::WINDOW, kind).is_ok() && {
                tabs::open_dock_panel(
                    app,
                    tabs::WINDOW,
                    id,
                    kind.title(),
                    tabs::PanelEdge::Right,
                    width,
                    panel_min(kind),
                    true,
                );
                true
            }
        } else if let Some(extension) = tabs::extension_id_of(id) {
            let edge = tabs::parse_panel_edge(&panel.edge);
            crate::host_extensions::reopen_extension_panel(
                app,
                tabs::WINDOW,
                extension,
                edge,
                width,
            )
            .is_ok()
        } else {
            false
        };
        if opened && panel.collapsed {
            tabs::set_dock_collapsed(app, tabs::WINDOW, id, true);
        }
    }
}

fn builtin_kind(id: &str) -> Option<tabs::SidePanel> {
    match id {
        tabs::AGENT_PANEL_ID => Some(tabs::SidePanel::Agent),
        tabs::AUTOMATIONS_PANEL_ID => Some(tabs::SidePanel::Automations),
        tabs::SCRIPTING_PANEL_ID => Some(tabs::SidePanel::Scripting),
        _ => None,
    }
}

/// Closes the popped-out scripting window so the process can exit with the last browser window.
pub fn close_scripting_window(app: &AppHandle) {
    if let Some(window) = app.get_window(SCRIPTING_WINDOW) {
        let _ = window.close();
    }
}

#[cfg(test)]
mod tests {
    use super::width_beside_pointer;

    #[test]
    fn the_left_edge_follows_the_pointer() {
        assert_eq!(
            width_beside_pointer(1200.0, 0.0, 48.0, 800.0, 0.0, 720.0),
            352.0
        );
    }

    #[test]
    fn the_grab_keeps_the_edge_from_jumping() {
        assert_eq!(
            width_beside_pointer(1200.0, 0.0, 48.0, 776.0, 4.0, 720.0),
            380.0
        );
    }

    #[test]
    fn the_width_stays_between_the_limits() {
        assert_eq!(
            width_beside_pointer(1600.0, 0.0, 0.0, 0.0, 0.0, 720.0),
            720.0
        );
        assert_eq!(
            width_beside_pointer(1600.0, 0.0, 0.0, 1500.0, 0.0, 720.0),
            280.0
        );
    }

    #[test]
    fn the_automations_column_may_cover_half_the_page() {
        assert_eq!(
            width_beside_pointer(2000.0, 0.0, 0.0, 0.0, 0.0, 1000.0),
            1000.0
        );
    }

    #[test]
    fn a_narrow_window_keeps_the_minimum() {
        assert_eq!(
            width_beside_pointer(500.0, 0.0, 0.0, 0.0, 0.0, 250.0),
            280.0
        );
    }

    #[test]
    fn the_right_edge_of_a_left_panel_follows_the_pointer() {
        assert_eq!(
            super::width_for_edge(
                1200.0,
                0.0,
                48.0,
                400.0,
                0.0,
                240.0,
                720.0,
                crate::tabs::PanelEdge::Left,
                360.0,
                0.0,
                0.0,
            ),
            400.0
        );
        assert_eq!(
            super::width_for_edge(
                1200.0,
                80.0,
                48.0,
                490.0,
                10.0,
                240.0,
                720.0,
                crate::tabs::PanelEdge::Left,
                360.0,
                0.0,
                0.0,
            ),
            400.0
        );
    }

    #[test]
    fn a_bad_pointer_keeps_the_default() {
        assert_eq!(
            width_beside_pointer(f64::NAN, 0.0, 0.0, 0.0, 0.0, 720.0),
            380.0
        );
    }
}
