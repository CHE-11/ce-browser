use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use serde::Serialize;
use tauri::webview::{Color, PageLoadEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl};
use url::Url;

use crate::{paths, settings, tabs, webview2};

/// Logical px of the card header the frame draws above the page.
const HEADER: f64 = 40.0;
/// Logical px of frame left around the page on the sides and bottom.
const BORDER: f64 = 6.0;
/// Logical px between the page area and the card, top and bottom.
const INSET_Y: f64 = 24.0;
/// Share of the page area width left free on each side of the card.
const INSET_X: f64 = 0.06;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeekInfo {
    pub window: String,
    pub url: String,
    pub title: String,
    pub favicon: Option<String>,
}

struct Peek {
    frame: String,
    page: String,
    info: PeekInfo,
}

#[derive(Default)]
pub struct PeekState(Mutex<HashMap<String, Peek>>);

static NEXT: AtomicU32 = AtomicU32::new(1);

pub fn init(app: &AppHandle) {
    app.manage(PeekState::default());
}

/// Never call into tabs or a webview while holding this lock: tabs reads it from `snapshot`.
fn with_peeks<T>(app: &AppHandle, work: impl FnOnce(&mut HashMap<String, Peek>) -> T) -> T {
    let state = app.state::<PeekState>();
    let mut guard = state.0.lock().expect("peek lock");
    work(&mut guard)
}

pub fn url_of(app: &AppHandle, window: &str) -> Option<String> {
    app.try_state::<PeekState>()?;
    with_peeks(app, |peeks| {
        peeks.get(window).map(|peek| peek.info.url.clone())
    })
}

pub fn info(app: &AppHandle, window: &str) -> Option<PeekInfo> {
    with_peeks(app, |peeks| peeks.get(window).map(|peek| peek.info.clone()))
}

fn emit_changed(app: &AppHandle, window: &str) {
    let info = info(app, window);
    let _ = app.emit("peek-changed", info);
}

/// Shift+click on a link in tab `opener`. Opens a peek, or a background tab when peeks are off.
pub fn request(app: &AppHandle, opener: u32, raw: &str) {
    let Ok(url) = Url::parse(raw) else {
        return;
    };
    if !matches!(url.scheme(), "http" | "https") {
        return;
    }
    if !settings::get(app).peek_on_shift_click {
        let _ = tabs::open_beside(app, opener, url, true);
        return;
    }
    open_for_tab(app, opener, raw);
}

/// Opens a peek over the window that shows tab `opener`.
pub fn open_for_tab(app: &AppHandle, opener: u32, raw: &str) {
    let Ok(url) = Url::parse(raw) else {
        return;
    };
    let window = tabs::snapshot(app)
        .tabs
        .iter()
        .find(|tab| tab.id == opener)
        .map(|tab| tab.window.clone())
        .unwrap_or_else(|| tabs::WINDOW.to_string());
    if let Err(error) = open(app, &window, url) {
        eprintln!("peek: {error}");
    }
}

/// Card and page rectangles in physical pixels.
fn rects(
    app: &AppHandle,
    window: &str,
) -> (
    (PhysicalPosition<i32>, PhysicalSize<u32>),
    (PhysicalPosition<i32>, PhysicalSize<u32>),
) {
    let (area, size) = tabs::page_area(app, window);
    let scale = app
        .get_window(window)
        .and_then(|window| window.scale_factor().ok())
        .unwrap_or(1.0);
    let px = |value: f64| (value * scale).round() as i32;
    let width = size.width as i32;
    let height = size.height as i32;
    let inset_x = ((width as f64) * INSET_X).round() as i32;
    let inset_y = px(INSET_Y).min(height / 6);
    let card_width = (width - inset_x * 2).max(200);
    let card_height = (height - inset_y * 2).max(160);
    let card = (
        PhysicalPosition::new(area.x + inset_x, area.y + inset_y),
        PhysicalSize::new(card_width as u32, card_height as u32),
    );
    let border = px(BORDER);
    let header = px(HEADER);
    let page = (
        PhysicalPosition::new(card.0.x + border, card.0.y + header),
        PhysicalSize::new(
            (card_width - border * 2).max(1) as u32,
            (card_height - header - border).max(1) as u32,
        ),
    );
    (card, page)
}

pub fn open(app: &AppHandle, window_label: &str, url: Url) -> Result<(), String> {
    // Focusing the tab here would close the new peek too, once its focus event arrives.
    close_views(app, window_label, false);
    let window = app
        .get_window(window_label)
        .ok_or("The browser window is missing.")?;
    let number = NEXT.fetch_add(1, Ordering::Relaxed);
    let frame_label = format!("peek-frame-{number}");
    let page_label = format!("peek-page-{number}");
    let display = paths::display_url(app, url.as_str());
    with_peeks(app, |peeks| {
        peeks.insert(
            window_label.to_string(),
            Peek {
                frame: frame_label.clone(),
                page: page_label.clone(),
                info: PeekInfo {
                    window: window_label.to_string(),
                    url: display.clone(),
                    title: display.clone(),
                    favicon: None,
                },
            },
        );
    });
    let ((card_position, card_size), (page_position, page_size)) = rects(app, window_label);
    let (r, g, b) = settings::popup_surface(app);
    let frame_url = paths::internal_url(app, "peek");
    let frame_builder = tabs::base_builder(app, &frame_label, WebviewUrl::External(frame_url))
        .background_color(Color(r, g, b, 255))
        .focused(false);
    let frame = match window.add_child(frame_builder, card_position, card_size) {
        Ok(frame) => frame,
        Err(error) => {
            with_peeks(app, |peeks| peeks.remove(window_label));
            return Err(error.to_string());
        }
    };
    webview2::attach_peek_hooks(app, &frame, window_label, false);
    if let Ok(hwnd) = window.hwnd() {
        let scale = window.scale_factor().unwrap_or(1.0);
        webview2::round_popup(&frame, hwnd.0 as isize, (10.0 * scale).round() as i32);
    }

    let load_app = app.clone();
    let title_app = app.clone();
    let tab_app = app.clone();
    let load_window = window_label.to_string();
    let title_window = window_label.to_string();
    let page_builder = tabs::base_builder(app, &page_label, WebviewUrl::External(url))
        .background_color(settings::page_color(app))
        .focused(true)
        .on_page_load(move |_, payload| {
            if matches!(payload.event(), PageLoadEvent::Finished) {
                let url = paths::display_url(&load_app, payload.url().as_str());
                update_url(&load_app, &load_window, &url);
            }
        })
        .on_document_title_changed(move |_, title| {
            let changed = with_peeks(&title_app, |peeks| {
                let peek = peeks.get_mut(&title_window)?;
                peek.info.title = if title.trim().is_empty() {
                    peek.info.url.clone()
                } else {
                    title
                };
                Some(())
            });
            if changed.is_some() {
                emit_changed(&title_app, &title_window);
            }
        })
        .on_new_window(move |url, features| tabs::new_window(&tab_app, None, url, features));
    let page = match window.add_child(page_builder, page_position, page_size) {
        Ok(page) => page,
        Err(error) => {
            with_peeks(app, |peeks| peeks.remove(window_label));
            let _ = frame.close();
            return Err(error.to_string());
        }
    };
    webview2::attach_peek_hooks(app, &page, window_label, true);
    let _ = page.set_focus();
    emit_changed(app, window_label);
    tabs::emit_tabs(app);
    Ok(())
}

pub fn close(app: &AppHandle, window: &str) {
    close_views(app, window, true);
}

/// Closes the peek because the user clicked into a page. Focus stays where they clicked.
pub fn dismiss(app: &AppHandle, window: &str) {
    close_views(app, window, false);
}

fn close_views(app: &AppHandle, window: &str, refocus: bool) {
    if app.try_state::<PeekState>().is_none() {
        return;
    }
    let Some(peek) = with_peeks(app, |peeks| peeks.remove(window)) else {
        return;
    };
    for label in [&peek.page, &peek.frame] {
        if let Some(webview) = app.get_webview(label) {
            let _ = webview.close();
        }
    }
    emit_changed(app, window);
    tabs::emit_tabs(app);
    if refocus {
        tabs::refocus(app, window);
    }
}

/// Keeps the peek over the page area after the window or the browser UI changes size.
pub fn relayout(app: &AppHandle, window: &str) {
    if app.try_state::<PeekState>().is_none() {
        return;
    }
    let Some((frame, page)) = with_peeks(app, |peeks| {
        peeks
            .get(window)
            .map(|peek| (peek.frame.clone(), peek.page.clone()))
    }) else {
        return;
    };
    let ((card_position, card_size), (page_position, page_size)) = rects(app, window);
    if let Some(frame) = app.get_webview(&frame) {
        let _ = frame.set_size(card_size);
        let _ = frame.set_position(card_position);
        if let Some(main) = app.get_window(window) {
            if let Ok(hwnd) = main.hwnd() {
                let scale = main.scale_factor().unwrap_or(1.0);
                webview2::round_popup(&frame, hwnd.0 as isize, (10.0 * scale).round() as i32);
            }
        }
    }
    if let Some(page) = app.get_webview(&page) {
        let _ = page.set_size(page_size);
        let _ = page.set_position(page_position);
    }
}

pub fn update_url(app: &AppHandle, window: &str, raw: &str) {
    let url = paths::display_url(app, raw);
    let changed = with_peeks(app, |peeks| {
        let peek = peeks.get_mut(window)?;
        if peek.info.url == url {
            return None;
        }
        peek.info.url = url;
        Some(())
    });
    if changed.is_some() {
        emit_changed(app, window);
        tabs::emit_tabs(app);
    }
}

pub fn update_favicon(app: &AppHandle, window: &str, favicon: Option<String>) {
    let changed = with_peeks(app, |peeks| {
        let peek = peeks.get_mut(window)?;
        peek.info.favicon = favicon;
        Some(())
    });
    if changed.is_some() {
        emit_changed(app, window);
    }
}

/// Opens the peek's page as a tab next to the active tab and closes the peek.
pub fn to_tab(app: &AppHandle, window: &str) -> Result<u32, String> {
    let url = info(app, window).ok_or("No peek is open.")?.url;
    close(app, window);
    let index = active_local(app, window);
    tabs::create_tab_on(app, window, Some(url), false, index)
}

/// Opens the peek's page beside the active tab in split view and closes the peek.
pub fn to_split(app: &AppHandle, window: &str) -> Result<u32, String> {
    let url = info(app, window).ok_or("No peek is open.")?.url;
    close(app, window);
    tabs::open_in_split(app, window, &url)
}

fn active_local(app: &AppHandle, window: &str) -> Option<usize> {
    let snapshot = tabs::snapshot(app);
    let active = snapshot
        .windows
        .iter()
        .find(|item| item.label == window)?
        .active_id?;
    let workspace = snapshot
        .windows
        .iter()
        .find(|item| item.label == window)
        .map(|item| item.workspace.clone())?;
    snapshot
        .tabs
        .iter()
        .filter(|tab| tab.window == window && tab.workspace == workspace)
        .position(|tab| tab.id == active)
        .map(|index| index + 1)
}
