//! Menus, panels and extension toolbar popups.
//!
//! Each browser window owns a few borderless top-level windows, owned by it so they stay above
//! it and close with it. They stay loaded, parked off screen. Opening one sends it the menu,
//! the page measures itself and reports its size, and only then does the window move next to
//! its button. A popup stays inside the browser window, and inside the screen when the
//! window runs past it.
//!
//! Submenus are a stack: a popup opened from a popup with `child` sits one level above it.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::webview::PageLoadEvent;
use tauri::window::WindowBuilder;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Position, Rect, Size, Webview,
    WebviewUrl, Window,
};
use url::Url;

use crate::{extensions, hang_guard, paths, settings, tabs, webview2};

/// A click on the button of a popup that just closed (by losing focus to that click) closes it.
const TOGGLE_WINDOW: Duration = Duration::from_millis(400);
/// A page that has not reported a ready size by then is shown at the size it has.
const SHOW_FALLBACK: Duration = Duration::from_millis(150);
/// Extension pages load from disk and run their own scripts before they can be measured.
const EXTENSION_FALLBACK: Duration = Duration::from_millis(600);
/// Focus moving from one window of a stack to another reports the loss before the gain.
const BLUR_SETTLE: Duration = Duration::from_millis(40);
const POOL_SIZE: usize = 2;
/// Logical size a parked app popup lays out at, so its content measures at its natural size.
const PARK_WIDTH: f64 = 900.0;
const PARK_HEIGHT: f64 = 900.0;
/// Extension pages size themselves from the viewport, so they lay out at a toolbar popup's width.
const EXTENSION_PARK_WIDTH: f64 = 380.0;
const EXTENSION_PARK_HEIGHT: f64 = 40.0;
/// Physical px. Left of and above every monitor.
const PARK_AT: i32 = -20000;
const MIN_WIDTH: f64 = 120.0;
const MIN_HEIGHT: f64 = 40.0;
/// Chrome's limit for a toolbar popup. A larger document scrolls inside the popup.
const EXTENSION_MAX_WIDTH: f64 = 800.0;
const EXTENSION_MAX_HEIGHT: f64 = 600.0;
/// Shown when the content has not reported any size.
const DEFAULT_WIDTH: f64 = 360.0;
const DEFAULT_HEIGHT: f64 = 320.0;
/// Logical px. Matches `::-webkit-scrollbar` in index.css.
const SCROLLBAR_WIDTH: f64 = 8.0;
/// Logical px between a button and the popup below it.
const GAP: f64 = 4.0;
/// Logical px between a menu and a submenu beside it.
const SIDE_GAP: f64 = 2.0;
/// Logical px a popup keeps from the edge of the browser window.
const SCREEN_INSET: f64 = 4.0;

/// Logical px in the webview that asked for the popup.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PopupRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PopupRequest {
    pub kind: String,
    /// URL query string read by the popup page. For `extension` it is the extension id.
    #[serde(default)]
    pub params: String,
    /// Button or point the popup hangs from. A point is a zero-size rectangle.
    #[serde(default)]
    pub anchor: PopupRect,
    /// `bottom-start`, `bottom-end`, `bottom-center`, `right-start`, `left-start`, `point` or `center`.
    #[serde(default)]
    pub placement: String,
    /// Popups that take focus close when focus leaves them.
    #[serde(default = "default_true")]
    pub focus: bool,
    /// Opens one level above the popup that asked, instead of replacing every popup.
    #[serde(default)]
    pub child: bool,
    /// Logical px. The omnibox is at least as wide as the address field.
    #[serde(default)]
    pub min_width: f64,
}

fn default_true() -> bool {
    true
}

/// Sent to a popup page: what to render, and the token its size report must carry.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PopupShow {
    pub kind: String,
    pub params: String,
    pub token: u64,
    pub level: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PopupChange {
    pub kind: Option<String>,
    pub params: Option<String>,
}

/// Physical px on the virtual screen.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Bounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl Bounds {
    fn right(&self) -> f64 {
        self.x + self.width
    }

    fn bottom(&self) -> f64 {
        self.y + self.height
    }

    fn inset(&self, by: f64) -> Self {
        Self {
            x: self.x + by,
            y: self.y + by,
            width: (self.width - by * 2.0).max(1.0),
            height: (self.height - by * 2.0).max(1.0),
        }
    }

    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    /// The part of `self` that also lies inside `other`.
    fn intersection(self, other: Self) -> Option<Self> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let width = self.right().min(other.right()) - x;
        let height = self.bottom().min(other.bottom()) - y;
        (width >= 1.0 && height >= 1.0).then_some(Self {
            x,
            y,
            width,
            height,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Placement {
    BottomStart,
    BottomEnd,
    BottomCenter,
    RightStart,
    LeftStart,
    Point,
    Center,
}

impl Placement {
    fn parse(raw: &str) -> Self {
        match raw {
            "bottom-start" => Self::BottomStart,
            "bottom-center" => Self::BottomCenter,
            "right-start" => Self::RightStart,
            "left-start" => Self::LeftStart,
            "point" => Self::Point,
            "center" => Self::Center,
            _ => Self::BottomEnd,
        }
    }
}

struct Level {
    label: String,
    kind: String,
    params: String,
    token: u64,
    focus: bool,
    anchor: Bounds,
    placement: Placement,
    /// Logical px.
    min_width: f64,
    /// Logical px the content last reported.
    size: Option<(f64, f64)>,
    shown: bool,
    ext_url: Option<String>,
}

impl Level {
    fn key(&self) -> String {
        format!("{}?{}", self.kind, self.params)
    }

    fn extension(&self) -> bool {
        self.ext_url.is_some()
    }
}

#[derive(Default)]
struct Stack {
    /// App popup windows. A window is reused once its level closes.
    pool: Vec<String>,
    /// Window for `chrome-extension://` toolbar popups.
    ext: Option<String>,
    /// Windows still being built. They join `pool` or `ext` once they exist.
    building: Vec<String>,
    /// Open popups, the first one opened from the browser UI and each next one beside it.
    levels: Vec<Level>,
    closed_key: Option<String>,
    closed_at: Option<Instant>,
}

#[derive(Default)]
struct PopupState {
    stacks: HashMap<String, Stack>,
    next_token: u64,
}

pub struct Popups(Mutex<PopupState>);

pub fn init(app: &AppHandle) {
    app.manage(Popups(Mutex::new(PopupState::default())));
}

/// Never hold this lock while calling into a webview or window getter.
fn with<T>(app: &AppHandle, change: impl FnOnce(&mut PopupState) -> T) -> T {
    let state = app.state::<Popups>();
    let mut guard = state.inner().0.lock().expect("popup lock");
    change(&mut guard)
}

fn app_label(window: &str, slot: usize) -> String {
    format!("popup-{window}-{slot}")
}

fn ext_label(window: &str) -> String {
    format!("extpopup-{window}")
}

pub fn is_popup_label(label: &str) -> bool {
    label.starts_with("popup-") || label.starts_with("extpopup-")
}

/// The browser window a popup belongs to. Any other label is returned as it is.
pub fn owner_of(label: &str) -> String {
    if let Some(window) = label.strip_prefix("extpopup-") {
        return window.to_string();
    }
    if let Some((window, _)) = label
        .strip_prefix("popup-")
        .and_then(|rest| rest.rsplit_once('-'))
    {
        return window.to_string();
    }
    label.to_string()
}

fn hwnd_of(window: &Window) -> isize {
    window.hwnd().map(|hwnd| hwnd.0 as isize).unwrap_or(0)
}

/// Builds the parked windows for `window` if they are not there yet. On the main thread the
/// windows are built on another thread, because building a webview waits for the main thread.
pub fn ensure_pool(app: &AppHandle, window: &str) {
    if hang_guard::on_main_thread() {
        let app = app.clone();
        let window = window.to_string();
        tauri::async_runtime::spawn(async move { ensure_pool(&app, &window) });
        return;
    }
    let (apps, ext, fresh) = with(app, |state| {
        let fresh = !state.stacks.contains_key(window);
        let stack = state.stacks.entry(window.to_string()).or_default();
        let mut apps = Vec::new();
        for slot in 0..POOL_SIZE {
            let label = app_label(window, slot);
            if !stack.pool.contains(&label) && !stack.building.contains(&label) {
                stack.building.push(label.clone());
                apps.push(label);
            }
        }
        let label = ext_label(window);
        let ext = (stack.ext.is_none() && !stack.building.contains(&label)).then(|| {
            stack.building.push(label.clone());
            label
        });
        (apps, ext, fresh)
    });
    if fresh && let Some(owner) = app.get_window(window) {
        let owner = hwnd_of(&owner);
        let _ = app.run_on_main_thread(move || webview2::keep_owner_active(owner));
    }
    for label in apps {
        let built = create_app_window(app, window, &label);
        if let Err(error) = &built {
            eprintln!("Popup window {label}: {error}");
        }
        finish_building(app, window, &label, built.is_ok(), |stack| {
            stack.pool.push(label.clone());
        });
    }
    if let Some(label) = ext {
        let built = create_ext_window(app, window, &label);
        if let Err(error) = &built {
            eprintln!("Extension popup window {label}: {error}");
        }
        finish_building(app, window, &label, built.is_ok(), |stack| {
            stack.ext = Some(label.clone());
        });
    }
}

fn finish_building(
    app: &AppHandle,
    window: &str,
    label: &str,
    built: bool,
    add: impl FnOnce(&mut Stack),
) {
    let kept = with(app, |state| {
        let Some(stack) = state.stacks.get_mut(window) else {
            return false;
        };
        stack.building.retain(|item| item != label);
        if built {
            add(stack);
        }
        true
    });
    if built
        && !kept
        && let Some(popup) = app.get_window(label)
    {
        let _ = popup.destroy();
    }
}

/// Adds one more app window, for a submenu deeper than the pool.
fn grow_pool(app: &AppHandle, window: &str) -> Result<String, String> {
    if hang_guard::on_main_thread() {
        ensure_pool(app, window);
        return Err("The popup windows are still being built.".into());
    }
    let label = with(app, |state| {
        let stack = state.stacks.get_mut(window)?;
        let mut slot = 0;
        let label = loop {
            let label = app_label(window, slot);
            if !stack.pool.contains(&label) && !stack.building.contains(&label) {
                break label;
            }
            slot += 1;
        };
        stack.building.push(label.clone());
        Some(label)
    })
    .ok_or("The browser window is missing.")?;
    let built = create_app_window(app, window, &label);
    finish_building(app, window, &label, built.is_ok(), |stack| {
        stack.pool.push(label.clone());
    });
    built.map(|()| label)
}

fn build_window(
    app: &AppHandle,
    window: &str,
    label: &str,
    width: f64,
    height: f64,
) -> Result<Window, String> {
    let owner = app
        .get_window(window)
        .ok_or("The browser window is missing.")?;
    let popup = WindowBuilder::new(app, label)
        .title("ce-browser")
        .decorations(false)
        .shadow(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .visible(false)
        .position(f64::from(PARK_AT), f64::from(PARK_AT))
        .inner_size(width, height)
        .parent(&owner)
        .map_err(|error| error.to_string())?
        .build()
        .map_err(|error| error.to_string())?;
    let _ = popup.set_background_color(Some(tabs::popup_surface_color(app)));
    // WebView2 does not paint into a hidden window, so the parked window stays shown off screen.
    let hwnd = hwnd_of(&popup);
    let scale = owner.scale_factor().unwrap_or(1.0);
    let physical_width = (width * scale).round() as i32;
    let physical_height = (height * scale).round() as i32;
    let _ = app.run_on_main_thread(move || {
        webview2::show_parked_popup(hwnd, PARK_AT, PARK_AT, physical_width, physical_height);
    });
    Ok(popup)
}

fn create_app_window(app: &AppHandle, window: &str, label: &str) -> Result<(), String> {
    let popup = build_window(app, window, label, PARK_WIDTH, PARK_HEIGHT)?;
    let url = paths::internal_url(app, "popup/host");
    let surface = tabs::popup_surface_color(app);
    let open_app = app.clone();
    let open_window = window.to_string();
    let builder = tabs::base_builder(app, label, WebviewUrl::External(url))
        .auto_resize()
        .disable_drag_drop_handler()
        .background_color(surface)
        .focused(false)
        .initialization_script(tabs::popup_paint_script(surface))
        .on_new_window(move |url, features| {
            let response = tabs::new_window(&open_app, None, url, features);
            // Close after this handler returns. WebView2 still has to adopt the new
            // window, and parking the opener first cancels that request.
            defer_close(&open_app, &open_window);
            response
        });
    let size = popup.inner_size().map_err(|error| error.to_string())?;
    let webview = popup
        .add_child(builder, PhysicalPosition::new(0, 0), size)
        .map_err(|error| error.to_string())?;
    webview2::attach_popup_hooks(app, &webview, label);
    webview2::attach_popup_blur(app, &webview, label);
    Ok(())
}

fn create_ext_window(app: &AppHandle, window: &str, label: &str) -> Result<(), String> {
    let popup = build_window(
        app,
        window,
        label,
        EXTENSION_PARK_WIDTH,
        EXTENSION_PARK_HEIGHT,
    )?;
    let url = Url::parse("about:blank").map_err(|error| error.to_string())?;
    let surface = tabs::popup_surface_color(app);
    let load_app = app.clone();
    let load_window = window.to_string();
    let open_app = app.clone();
    let open_window = window.to_string();
    let builder = tabs::base_builder(app, label, WebviewUrl::External(url))
        .auto_resize()
        .disable_drag_drop_handler()
        .background_color(surface)
        .focused(false)
        .on_page_load(move |webview, payload| {
            if !matches!(payload.event(), PageLoadEvent::Finished) {
                return;
            }
            let loaded = payload.url().as_str();
            if loaded.starts_with("about:blank") {
                return;
            }
            let label = webview.label().to_string();
            let token = with(&load_app, |state| {
                let level = state
                    .stacks
                    .get(&load_window)?
                    .levels
                    .iter()
                    .find(|level| level.label == label)?;
                same_document(loaded, level.ext_url.as_deref()?).then_some(level.token)
            });
            if let Some(token) = token {
                let factor = settings::get(&load_app).ui_zoom_factor();
                let _ = webview.set_zoom(factor);
                measure_extension(&load_app, &load_window, &webview, token, factor);
            }
        })
        .on_new_window(move |url, features| {
            let response = tabs::new_window(&open_app, None, url, features);
            defer_close(&open_app, &open_window);
            response
        });
    let size = popup.inner_size().map_err(|error| error.to_string())?;
    let webview = popup
        .add_child(builder, PhysicalPosition::new(0, 0), size)
        .map_err(|error| error.to_string())?;
    let _ = webview.set_zoom(settings::get(app).ui_zoom_factor());
    webview2::attach_popup_hooks(app, &webview, label);
    webview2::attach_popup_blur(app, &webview, label);
    Ok(())
}

/// Sizes an extension toolbar popup to its document. `factor` is the Browser UI zoom.
fn measure_extension(app: &AppHandle, window: &str, webview: &Webview, token: u64, factor: f64) {
    let app = app.clone();
    let window = window.to_string();
    webview2::measure_document(webview, move |width, height| {
        std::thread::spawn(move || {
            reveal(
                &app,
                &window,
                token,
                Some((width * factor, height * factor)),
                true,
                false,
            );
        });
    });
}

fn defer_close(app: &AppHandle, window: &str) {
    let app = app.clone();
    let window = window.to_string();
    let _ = app
        .clone()
        .run_on_main_thread(move || close_all(&app, &window, false));
}

fn same_document(current: &str, expected: &str) -> bool {
    match (Url::parse(current), Url::parse(expected)) {
        (Ok(current), Ok(expected)) => {
            current.scheme() == expected.scheme()
                && current.host_str() == expected.host_str()
                && current.path().trim_end_matches('/') == expected.path().trim_end_matches('/')
                && current.query() == expected.query()
        }
        _ => current.trim_end_matches('/') == expected.trim_end_matches('/'),
    }
}

fn emit_change(app: &AppHandle, kind: Option<String>, params: Option<String>) {
    let _ = app.emit("popup-changed", PopupChange { kind, params });
}

/// Top-left of `webview`'s page on the screen, in physical px, and its window's scale.
fn origin(webview: &Webview) -> Result<(f64, f64, f64), String> {
    let window = webview.window();
    let inner = window.inner_position().map_err(|error| error.to_string())?;
    let offset = webview.position().map_err(|error| error.to_string())?;
    let scale = window.scale_factor().unwrap_or(1.0);
    Ok((
        f64::from(inner.x + offset.x),
        f64::from(inner.y + offset.y),
        scale,
    ))
}

/// Opens a popup from `sender`, whose page measured `request.anchor`.
/// Returns false when the request closed the same popup instead.
pub fn open(app: &AppHandle, sender: &Webview, request: PopupRequest) -> Result<bool, String> {
    let sender_label = sender.label().to_string();
    let window = owner_of(sender.window().label());
    let (x, y, scale) = origin(sender)?;
    let rect = request.anchor;
    let anchor = Bounds {
        x: x + rect.x * scale,
        y: y + rect.y * scale,
        width: rect.width.max(0.0) * scale,
        height: rect.height.max(0.0) * scale,
    };
    let parent = if request.child {
        depth_of(app, &window, &sender_label)
    } else {
        None
    };
    open_at(app, &window, anchor, request, parent)
}

/// Opens a popup at a point in a page view, `x` and `y` being physical px inside the page.
pub fn open_from_page(
    app: &AppHandle,
    page: &Webview,
    x: i32,
    y: i32,
    kind: &str,
    params: String,
) -> Result<bool, String> {
    let window = owner_of(page.window().label());
    let (left, top, _) = origin(page)?;
    let anchor = Bounds {
        x: left + f64::from(x),
        y: top + f64::from(y),
        width: 0.0,
        height: 0.0,
    };
    let request = PopupRequest {
        kind: kind.to_string(),
        params,
        anchor: PopupRect::default(),
        placement: "point".into(),
        focus: true,
        child: false,
        min_width: 0.0,
    };
    open_at(app, &window, anchor, request, None)
}

fn depth_of(app: &AppHandle, window: &str, label: &str) -> Option<usize> {
    with(app, |state| {
        state
            .stacks
            .get(window)?
            .levels
            .iter()
            .position(|level| level.label == label)
    })
}

fn toggles(app: &AppHandle, window: &str, key: &str) -> bool {
    with(app, |state| {
        let Some(stack) = state.stacks.get(window) else {
            return false;
        };
        let open_same = stack.levels.first().is_some_and(|level| level.key() == key);
        let just_closed = stack.closed_key.as_deref() == Some(key)
            && stack
                .closed_at
                .is_some_and(|at| at.elapsed() < TOGGLE_WINDOW);
        open_same || just_closed
    })
}

fn open_at(
    app: &AppHandle,
    window: &str,
    anchor: Bounds,
    request: PopupRequest,
    parent: Option<usize>,
) -> Result<bool, String> {
    let depth = parent.map_or(0, |level| level + 1);
    let key = format!("{}?{}", request.kind, request.params);
    if depth == 0 && request.focus && toggles(app, window, &key) {
        close_all(app, window, true);
        with(app, |state| {
            if let Some(stack) = state.stacks.get_mut(window) {
                stack.closed_at = None;
            }
        });
        return Ok(false);
    }
    // A submenu that is already open beside its row stays. A keyboard open moves focus into it.
    let already = depth > 0
        && with(app, |state| {
            state
                .stacks
                .get(window)
                .and_then(|stack| stack.levels.get(depth))
                .is_some_and(|level| level.key() == key)
        });
    if already {
        close_from(app, window, depth + 1);
        if request.focus {
            focus_level(app, window, depth);
        }
        return Ok(true);
    }

    ensure_pool(app, window);
    let extension = request.kind == "extension";
    let ext_url = if extension {
        Some(extensions::popup_url(app, &request.params)?)
    } else {
        None
    };
    let picked = with(app, |state| {
        state.next_token += 1;
        let token = state.next_token;
        let stack = state.stacks.get_mut(window)?;
        let depth = depth.min(stack.levels.len());
        let closed: Vec<Level> = stack.levels.drain(depth..).collect();
        let label = if extension {
            stack.ext.clone()
        } else {
            let busy: Vec<&str> = stack
                .levels
                .iter()
                .map(|level| level.label.as_str())
                .collect();
            stack
                .pool
                .iter()
                .find(|label| !busy.contains(&label.as_str()))
                .cloned()
        };
        Some((closed, label, token, depth))
    });
    let Some((closed, label, token, depth)) = picked else {
        return Err("The browser window is missing.".into());
    };
    let label = match label {
        Some(label) => label,
        None if extension => {
            park_levels(app, window, &closed, None);
            return Err("The extension popup is missing.".into());
        }
        None => match grow_pool(app, window) {
            Ok(label) => label,
            Err(error) => {
                park_levels(app, window, &closed, None);
                return Err(error);
            }
        },
    };
    park_levels(app, window, &closed, Some(&label));

    let placement = Placement::parse(&request.placement);
    with(app, |state| {
        if let Some(stack) = state.stacks.get_mut(window) {
            stack.levels.truncate(depth);
            stack.levels.push(Level {
                label: label.clone(),
                kind: request.kind.clone(),
                params: request.params.clone(),
                token,
                focus: request.focus,
                anchor,
                placement,
                min_width: request.min_width.max(0.0),
                size: None,
                shown: false,
                ext_url: ext_url.as_ref().map(|url| url.as_str().to_string()),
            });
        }
    });

    if let Some(url) = ext_url {
        if let Some(webview) = app.get_webview(&label) {
            let same = webview
                .url()
                .ok()
                .is_some_and(|current| same_document(current.as_str(), url.as_str()));
            let _ = if same {
                webview.reload()
            } else {
                webview.navigate(url)
            };
        }
    } else {
        let _ = app.emit_to(
            label.as_str(),
            "popup-show",
            PopupShow {
                kind: request.kind.clone(),
                params: request.params.clone(),
                token,
                level: depth,
            },
        );
    }
    schedule_fallback(
        app,
        window,
        token,
        if extension {
            EXTENSION_FALLBACK
        } else {
            SHOW_FALLBACK
        },
    );
    if depth == 0 {
        emit_change(app, Some(request.kind), Some(request.params));
    }
    Ok(true)
}

/// Shows `token`'s popup after `delay` even if its page never reported a ready size.
fn schedule_fallback(app: &AppHandle, window: &str, token: u64, delay: Duration) {
    let app = app.clone();
    let window = window.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        reveal(&app, &window, token, None, true, true);
    });
}

/// The page of popup `label` measured itself. `ready` is false while it shows a placeholder.
pub fn measured(app: &AppHandle, label: &str, token: u64, width: f64, height: f64, ready: bool) {
    if !width.is_finite() || !height.is_finite() || width < 1.0 || height < 1.0 {
        return;
    }
    let window = owner_of(label);
    let mine = with(app, |state| {
        state.stacks.get(&window).is_some_and(|stack| {
            stack
                .levels
                .iter()
                .any(|level| level.label == label && level.token == token)
        })
    });
    if mine {
        reveal(app, &window, token, Some((width, height)), ready, false);
    }
}

struct Present {
    label: String,
    token: u64,
    anchor: Bounds,
    placement: Placement,
    size: (f64, f64),
    min_width: f64,
    focus: bool,
    first: bool,
    extension: bool,
}

/// Stores the size `token`'s page reported and puts that popup on screen at it.
fn reveal(
    app: &AppHandle,
    window: &str,
    token: u64,
    size: Option<(f64, f64)>,
    ready: bool,
    fallback: bool,
) {
    let plan = with(app, |state| {
        let stack = state.stacks.get_mut(window)?;
        let level = stack.levels.iter_mut().find(|level| level.token == token)?;
        if let Some(size) = size {
            level.size = Some(size);
        }
        if fallback && level.shown {
            return None;
        }
        if !ready && !level.shown {
            return None;
        }
        let first = !level.shown;
        level.shown = true;
        Some(Present {
            label: level.label.clone(),
            token: level.token,
            anchor: level.anchor,
            placement: level.placement,
            size: level.size.unwrap_or((DEFAULT_WIDTH, DEFAULT_HEIGHT)),
            min_width: level.min_width,
            focus: level.focus,
            first,
            extension: level.extension(),
        })
    });
    if let Some(plan) = plan {
        present(app, window, plan);
    }
}

/// Work area of the monitor under `anchor`, in physical px, and that monitor's scale.
fn work_area(app: &AppHandle, window: &str, anchor: &Bounds) -> (Bounds, f64) {
    let x = anchor.x + anchor.width / 2.0;
    let y = anchor.y + anchor.height / 2.0;
    let monitors = app.available_monitors().unwrap_or_default();
    let monitor = monitors
        .into_iter()
        .find(|monitor| {
            let position = monitor.position();
            let size = monitor.size();
            Bounds {
                x: f64::from(position.x),
                y: f64::from(position.y),
                width: f64::from(size.width),
                height: f64::from(size.height),
            }
            .contains(x, y)
        })
        .or_else(|| {
            app.get_window(window)
                .and_then(|window| window.current_monitor().ok().flatten())
        });
    match monitor {
        Some(monitor) => {
            let area = monitor.work_area();
            (
                Bounds {
                    x: f64::from(area.position.x),
                    y: f64::from(area.position.y),
                    width: f64::from(area.size.width),
                    height: f64::from(area.size.height),
                },
                monitor.scale_factor(),
            )
        }
        None => (
            Bounds {
                x: 0.0,
                y: 0.0,
                width: 1920.0,
                height: 1080.0,
            },
            1.0,
        ),
    }
}

/// Client area of the browser window, in physical px. A borderless window's outer rect
/// includes the invisible shadow frame, which is outside what is on screen.
fn browser_bounds(app: &AppHandle, window: &str) -> Option<Bounds> {
    let window = app.get_window(window)?;
    let position = window.inner_position().ok()?;
    let size = window.inner_size().ok()?;
    Some(Bounds {
        x: f64::from(position.x),
        y: f64::from(position.y),
        width: f64::from(size.width).max(1.0),
        height: f64::from(size.height).max(1.0),
    })
}

fn present(app: &AppHandle, window: &str, plan: Present) {
    let Some(popup) = app.get_window(&plan.label) else {
        return;
    };
    let (screen, scale) = work_area(app, window, &plan.anchor);
    let area = browser_bounds(app, window)
        .and_then(|browser| screen.intersection(browser))
        .unwrap_or(screen);
    let (max_width, max_height) = if plan.extension {
        (EXTENSION_MAX_WIDTH, EXTENSION_MAX_HEIGHT)
    } else {
        (f64::INFINITY, f64::INFINITY)
    };
    let width = plan.size.0.max(plan.min_width).clamp(MIN_WIDTH, max_width) * scale;
    let height = plan.size.1.clamp(MIN_HEIGHT, max_height) * scale;
    let fit = |width: f64| {
        place(
            plan.anchor,
            width,
            height,
            plan.placement,
            area.inset(SCREEN_INSET * scale),
            GAP * scale,
            SIDE_GAP * scale,
        )
    };
    let mut rect = fit(width);
    // A popup the window made shorter than its content scrolls, so it needs room for the bar.
    if !plan.extension && rect.height + 0.5 < height {
        rect = fit(width + SCROLLBAR_WIDTH * scale);
    }
    let hwnd = hwnd_of(&popup);
    if hwnd == 0 {
        return;
    }
    let webview = app.get_webview(&plan.label);
    let activate = plan.focus && plan.first;
    let no_activate = !plan.focus;
    let x = rect.x.round() as i32;
    let y = rect.y.round() as i32;
    let width = rect.width.round().max(1.0) as i32;
    let height = rect.height.round().max(1.0) as i32;
    let token = plan.token;
    let label = plan.label.clone();
    let window_name = window.to_string();
    let app_check = app.clone();
    let _ = app.run_on_main_thread(move || {
        // A search result can be chosen while this popup is still measuring. Closing it
        // parks the window, and this move would put the closed popup back on screen.
        let still_open = with(&app_check, |state| {
            state.stacks.get(&window_name).is_some_and(|stack| {
                stack
                    .levels
                    .iter()
                    .any(|level| level.label == label && level.token == token)
            })
        });
        if !still_open {
            return;
        }
        webview2::place_popup_window(hwnd, x, y, width, height, no_activate);
        if let Some(webview) = &webview {
            let _ = webview.set_bounds(Rect {
                position: Position::Physical(PhysicalPosition::new(0, 0)),
                size: Size::Physical(PhysicalSize::new(width as u32, height as u32)),
            });
        }
        if activate {
            webview2::activate_popup(hwnd);
            if let Some(webview) = &webview {
                let _ = webview.set_focus();
            }
        }
    });
}

/// Moves a popup window off screen and back to its parked size. `reset` clears its page.
fn park(app: &AppHandle, window: &str, label: &str, extension: bool, reset: bool) {
    if reset {
        if extension {
            blank(app, label);
        } else {
            let _ = app.emit_to(label, "popup-hide", ());
        }
    }
    let Some(popup) = app.get_window(label) else {
        return;
    };
    let hwnd = hwnd_of(&popup);
    if hwnd == 0 {
        return;
    }
    let owner = app
        .get_window(window)
        .map(|owner| hwnd_of(&owner))
        .unwrap_or(0);
    let scale = popup.scale_factor().unwrap_or(1.0);
    let (width, height) = if extension {
        (EXTENSION_PARK_WIDTH, EXTENSION_PARK_HEIGHT)
    } else {
        (PARK_WIDTH, PARK_HEIGHT)
    };
    let width = (width * scale).round() as i32;
    let height = (height * scale).round() as i32;
    let webview = app.get_webview(label);
    let _ = app.run_on_main_thread(move || {
        webview2::park_popup_window(hwnd, owner, PARK_AT, PARK_AT, width, height);
        if let Some(webview) = webview {
            let _ = webview.set_bounds(Rect {
                position: Position::Physical(PhysicalPosition::new(0, 0)),
                size: Size::Physical(PhysicalSize::new(width as u32, height as u32)),
            });
        }
    });
}

/// Parks `levels`. `keep` is reused for the popup opening next, so its page is not cleared.
fn park_levels(app: &AppHandle, window: &str, levels: &[Level], keep: Option<&str>) {
    for level in levels {
        let reused = keep == Some(level.label.as_str());
        park(app, window, &level.label, level.extension(), !reused);
    }
}

fn blank(app: &AppHandle, label: &str) {
    let Some(webview) = app.get_webview(label) else {
        return;
    };
    let current = webview.url().ok();
    if current
        .as_ref()
        .is_some_and(|url| url.as_str().starts_with("about:blank"))
    {
        return;
    }
    if let Ok(blank) = Url::parse("about:blank") {
        let _ = webview.navigate(blank);
    }
}

/// Closes every popup of `window` (a browser window or one of its popups). `refocus` puts the
/// keyboard back on the page when one of them had it.
pub fn close_all(app: &AppHandle, window: &str, refocus: bool) {
    let window = owner_of(window);
    if app.try_state::<Popups>().is_none() {
        return;
    }
    let closed = with(app, |state| {
        let stack = state.stacks.get_mut(&window)?;
        if stack.levels.is_empty() {
            return None;
        }
        let levels: Vec<Level> = stack.levels.drain(..).collect();
        let first = &levels[0];
        stack.closed_key = first.focus.then(|| first.key());
        stack.closed_at = Some(Instant::now());
        Some(levels)
    });
    let Some(levels) = closed else {
        return;
    };
    let focused = levels.iter().any(|level| level.focus);
    park_levels(app, &window, &levels, None);
    emit_change(app, None, None);
    if refocus && focused {
        tabs::refocus_window(app, &window);
    }
}

/// Closes the levels from `depth` up.
fn close_from(app: &AppHandle, window: &str, depth: usize) {
    let closed = with(app, |state| {
        let stack = state.stacks.get_mut(window)?;
        (stack.levels.len() > depth).then(|| stack.levels.drain(depth..).collect::<Vec<_>>())
    });
    if let Some(levels) = closed {
        park_levels(app, window, &levels, None);
    }
}

fn focus_level(app: &AppHandle, window: &str, depth: usize) {
    let label = with(app, |state| {
        let level = state.stacks.get(window)?.levels.get(depth)?;
        level.focus.then(|| level.label.clone())
    });
    let Some(label) = label else {
        return;
    };
    let Some(popup) = app.get_window(&label) else {
        return;
    };
    let hwnd = hwnd_of(&popup);
    let webview = app.get_webview(&label);
    let _ = app.run_on_main_thread(move || {
        webview2::activate_popup(hwnd);
        if let Some(webview) = webview {
            let _ = webview.set_focus();
        }
    });
}

/// Closes popup `label` and everything opened from it. Focus goes back to the menu below it,
/// or to the page when it was the first one.
pub fn close_level(app: &AppHandle, label: &str) {
    let window = owner_of(label);
    match depth_of(app, &window, label) {
        None => {}
        Some(0) => close_all(app, &window, true),
        Some(depth) => {
            close_from(app, &window, depth);
            focus_level(app, &window, depth - 1);
        }
    }
}

/// Closes the submenus opened from popup `label`. It stays open.
pub fn close_above(app: &AppHandle, label: &str) {
    let window = owner_of(label);
    if let Some(depth) = depth_of(app, &window, label) {
        close_from(app, &window, depth + 1);
    }
}

/// Escape: closes the popup on top of `window`'s stack.
pub fn close_top(app: &AppHandle, window: &str) {
    let window = owner_of(window);
    let top = with(app, |state| {
        Some(state.stacks.get(&window)?.levels.last()?.label.clone())
    });
    if let Some(label) = top {
        close_level(app, &label);
    }
}

/// A popup's page lost focus. The stack closes unless focus went to another of its windows.
/// Popups that never took focus, like the omnibox, are closed by whoever opened them.
pub fn lost_focus(app: &AppHandle, label: &str) {
    let window = owner_of(label);
    let watched = with(app, |state| {
        let stack = state.stacks.get(&window)?;
        let first = stack.levels.first()?;
        (first.focus && stack.levels.iter().any(|level| level.label == label)).then_some(())
    });
    if watched.is_none() {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(BLUR_SETTLE);
        let labels = with(&app, |state| {
            state
                .stacks
                .get(&window)
                .map(|stack| {
                    stack
                        .levels
                        .iter()
                        .map(|level| level.label.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        });
        if labels.is_empty() {
            return;
        }
        let foreground = webview2::foreground_window();
        let inside = labels.iter().any(|label| {
            app.get_window(label)
                .is_some_and(|popup| hwnd_of(&popup) == foreground)
        });
        if inside {
            return;
        }
        close_all(&app, &window, false);
        if let Some(owner) = app.get_window(&window) {
            let owner = hwnd_of(&owner);
            let _ = app.run_on_main_thread(move || webview2::release_owner(owner));
        }
    });
}

/// The popup page for `label` asks what it should show. A page that finished booting after
/// its first `popup-show` gets that request here.
pub fn current(app: &AppHandle, label: &str) -> Option<PopupShow> {
    let window = owner_of(label);
    with(app, |state| {
        let levels = &state.stacks.get(&window)?.levels;
        let depth = levels.iter().position(|level| level.label == label)?;
        let level = &levels[depth];
        (!level.extension()).then(|| PopupShow {
            kind: level.kind.clone(),
            params: level.params.clone(),
            token: level.token,
            level: depth,
        })
    })
}

/// The window moved to a monitor with another scale. Puts it back where its anchor wants it.
pub fn rescaled(app: &AppHandle, label: &str) {
    let window = owner_of(label);
    let token = with(app, |state| {
        let level = state
            .stacks
            .get(&window)?
            .levels
            .iter()
            .find(|level| level.label == label)?;
        level.shown.then_some(level.token)
    });
    if let Some(token) = token {
        reveal(app, &window, token, None, true, false);
    }
}

/// Kind of the first popup open in `window`, if any.
pub fn kind(app: &AppHandle, window: &str) -> Option<String> {
    if app.try_state::<Popups>().is_none() {
        return None;
    }
    with(app, |state| {
        Some(state.stacks.get(window)?.levels.first()?.kind.clone())
    })
}

/// Sends an event to the popup on top of `window`'s stack.
pub fn emit_to_top<S: Serialize + Clone>(app: &AppHandle, window: &str, event: &str, payload: S) {
    let label = with(app, |state| {
        Some(state.stacks.get(window)?.levels.last()?.label.clone())
    });
    if let Some(label) = label {
        let _ = app.emit_to(label.as_str(), event, payload);
    }
}

/// Browser windows showing a popup that never took the keyboard, like the omnibox. Escape
/// cannot reach those, so the page or browser UI that has the keyboard closes them.
pub fn passive_open(app: &AppHandle) -> Vec<String> {
    if app.try_state::<Popups>().is_none() {
        return Vec::new();
    }
    with(app, |state| {
        state
            .stacks
            .iter()
            .filter(|(_, stack)| stack.levels.first().is_some_and(|level| !level.focus))
            .map(|(window, _)| window.clone())
            .collect()
    })
}

pub fn any_open(app: &AppHandle) -> bool {
    if app.try_state::<Popups>().is_none() {
        return false;
    }
    with(app, |state| {
        state.stacks.values().any(|stack| !stack.levels.is_empty())
    })
}

/// The browser window is closing. Its popup windows go with it.
pub fn forget(app: &AppHandle, window: &str) {
    if app.try_state::<Popups>().is_none() {
        return;
    }
    let stack = with(app, |state| state.stacks.remove(window));
    let Some(stack) = stack else {
        return;
    };
    for label in stack.pool.iter().chain(stack.ext.iter()) {
        if let Some(popup) = app.get_window(label) {
            let _ = popup.destroy();
        }
    }
}

/// Extension popups follow the Browser UI size.
pub fn apply_ui_zoom(app: &AppHandle, zoom: f64) {
    if app.try_state::<Popups>().is_none() {
        return;
    }
    let hosts = with(app, |state| {
        state
            .stacks
            .iter()
            .filter_map(|(window, stack)| {
                let label = stack.ext.clone()?;
                let showing = stack
                    .levels
                    .iter()
                    .find(|level| level.label == label && level.shown)
                    .map(|level| level.token);
                Some((window.clone(), label, showing))
            })
            .collect::<Vec<_>>()
    });
    for (window, label, showing) in hosts {
        let Some(webview) = app.get_webview(&label) else {
            continue;
        };
        let _ = webview.set_zoom(zoom);
        if let Some(token) = showing {
            measure_extension(app, &window, &webview, token, zoom);
        }
    }
}

/// Puts a `width` by `height` popup beside `anchor` inside `area`, all in physical px.
/// It flips to the other side when its own side has less room than the popup needs and the
/// other side has more, then slides inside the area. It shrinks only when the side it ends
/// up on still cannot hold it.
fn place(
    anchor: Bounds,
    width: f64,
    height: f64,
    placement: Placement,
    area: Bounds,
    gap: f64,
    side_gap: f64,
) -> Bounds {
    let width = width.min(area.width).max(1.0);
    let mut height = height.max(1.0);
    let mut x;
    let mut y;
    match placement {
        Placement::BottomStart | Placement::BottomEnd | Placement::BottomCenter => {
            x = match placement {
                Placement::BottomStart => anchor.x,
                Placement::BottomEnd => anchor.right() - width,
                _ => anchor.x + (anchor.width - width) / 2.0,
            };
            let below = area.bottom() - (anchor.bottom() + gap);
            let above = anchor.y - gap - area.y;
            if height > below && above > below {
                height = height.min(above);
                y = anchor.y - gap - height;
            } else {
                height = height.min(below.max(1.0));
                y = anchor.bottom() + gap;
            }
        }
        Placement::RightStart | Placement::LeftStart => {
            let right = area.right() - (anchor.right() + side_gap);
            let left = anchor.x - side_gap - area.x;
            let go_right = if placement == Placement::RightStart {
                width <= right || right >= left
            } else {
                width > left && right > left
            };
            x = if go_right {
                anchor.right() + side_gap
            } else {
                anchor.x - side_gap - width
            };
            height = height.min(area.height);
            y = anchor.y.min(area.bottom() - height);
        }
        Placement::Center => {
            height = height.min(area.height);
            x = area.x + (area.width - width) / 2.0;
            y = area.y + (area.height - height) / 2.0;
        }
        Placement::Point => {
            x = if anchor.x + width > area.right() && anchor.x - width >= area.x {
                anchor.x - width
            } else {
                anchor.x
            };
            height = height.min(area.height);
            let below = area.bottom() - anchor.y;
            let above = anchor.y - area.y;
            y = if height > below && height <= above {
                anchor.y - height
            } else {
                anchor.y.min(area.bottom() - height)
            };
        }
    }
    x = x.min(area.right() - width).max(area.x);
    y = y.max(area.y);
    Bounds {
        x,
        y,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Bounds = Bounds {
        x: 0.0,
        y: 0.0,
        width: 1000.0,
        height: 800.0,
    };

    fn button(x: f64, y: f64) -> Bounds {
        Bounds {
            x,
            y,
            width: 32.0,
            height: 32.0,
        }
    }

    #[test]
    fn a_menu_hangs_from_the_right_edge_of_its_button() {
        let placed = place(
            button(700.0, 40.0),
            280.0,
            200.0,
            Placement::BottomEnd,
            SCREEN,
            4.0,
            2.0,
        );
        assert_eq!((placed.x, placed.y, placed.height), (452.0, 76.0, 200.0));
    }

    #[test]
    fn a_menu_near_the_bottom_opens_above_its_button() {
        let placed = place(
            button(100.0, 700.0),
            200.0,
            300.0,
            Placement::BottomStart,
            SCREEN,
            4.0,
            2.0,
        );
        assert_eq!((placed.x, placed.y, placed.height), (100.0, 396.0, 300.0));
    }

    #[test]
    fn a_menu_shrinks_only_when_neither_side_can_hold_it() {
        let placed = place(
            button(100.0, 300.0),
            200.0,
            1000.0,
            Placement::BottomStart,
            SCREEN,
            4.0,
            2.0,
        );
        assert_eq!((placed.y, placed.height), (336.0, 464.0));
    }

    #[test]
    fn a_popup_slides_inside_the_screen() {
        let placed = place(
            button(980.0, 40.0),
            300.0,
            100.0,
            Placement::BottomStart,
            SCREEN,
            4.0,
            2.0,
        );
        assert_eq!(placed.x, 700.0);
    }

    #[test]
    fn a_submenu_opens_on_the_left_when_the_right_has_no_room() {
        let row = Bounds {
            x: 600.0,
            y: 100.0,
            width: 300.0,
            height: 30.0,
        };
        let placed = place(row, 300.0, 200.0, Placement::RightStart, SCREEN, 4.0, 2.0);
        assert_eq!((placed.x, placed.y), (298.0, 100.0));
    }

    #[test]
    fn a_submenu_moves_up_to_stay_on_screen() {
        let row = Bounds {
            x: 100.0,
            y: 700.0,
            width: 300.0,
            height: 30.0,
        };
        let placed = place(row, 300.0, 200.0, Placement::RightStart, SCREEN, 4.0, 2.0);
        assert_eq!((placed.x, placed.y), (402.0, 600.0));
    }

    #[test]
    fn a_side_popup_stays_beside_its_button() {
        let placed = place(
            button(760.0, 400.0),
            320.0,
            200.0,
            Placement::LeftStart,
            SCREEN,
            4.0,
            2.0,
        );
        assert_eq!((placed.x, placed.y), (438.0, 400.0));
    }

    #[test]
    fn a_centered_popup_sits_in_the_middle_and_shrinks_to_the_area() {
        let anchor = Bounds {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        };
        let placed = place(anchor, 400.0, 200.0, Placement::Center, SCREEN, 4.0, 2.0);
        assert_eq!(
            (placed.x, placed.y, placed.width, placed.height),
            (300.0, 300.0, 400.0, 200.0)
        );
        let fitted = place(anchor, 2000.0, 2000.0, Placement::Center, SCREEN, 4.0, 2.0);
        assert_eq!(
            (fitted.x, fitted.y, fitted.width, fitted.height),
            (0.0, 0.0, 1000.0, 800.0)
        );
    }

    #[test]
    fn a_context_menu_flips_up_and_left_at_the_corner() {
        let point = Bounds {
            x: 900.0,
            y: 700.0,
            width: 0.0,
            height: 0.0,
        };
        let placed = place(point, 240.0, 300.0, Placement::Point, SCREEN, 4.0, 2.0);
        assert_eq!((placed.x, placed.y), (660.0, 400.0));
    }

    #[test]
    fn a_menu_stays_inside_a_window_smaller_than_the_screen() {
        let browser = Bounds {
            x: 100.0,
            y: 80.0,
            width: 900.0,
            height: 500.0,
        };
        let area = SCREEN.intersection(browser).unwrap();
        let placed = place(
            button(700.0, 100.0),
            340.0,
            900.0,
            Placement::BottomEnd,
            area,
            4.0,
            2.0,
        );
        assert!(placed.y >= area.y);
        assert!(placed.bottom() <= area.bottom() + 0.5);
        assert!(placed.right() <= area.right() + 0.5);
        assert!(placed.height < 900.0);
    }

    #[test]
    fn a_window_past_the_screen_keeps_only_the_visible_part() {
        let browser = Bounds {
            x: -100.0,
            y: 50.0,
            width: 400.0,
            height: 900.0,
        };
        assert_eq!(
            SCREEN.intersection(browser),
            Some(Bounds {
                x: 0.0,
                y: 50.0,
                width: 300.0,
                height: 750.0,
            })
        );
    }

    #[test]
    fn a_popup_label_belongs_to_its_browser_window() {
        assert_eq!(owner_of("popup-main-0"), "main");
        assert_eq!(owner_of("popup-main-3-1"), "main-3");
        assert_eq!(owner_of("extpopup-main-3"), "main-3");
        assert_eq!(owner_of("main-2"), "main-2");
    }
}
