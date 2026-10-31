use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::webview::{
    Color, NewWindowFeatures, NewWindowResponse, PageLoadEvent, WebviewWindowBuilder,
};
use tauri::window::WindowBuilder;
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, PhysicalPosition, PhysicalSize,
    Webview, WebviewBuilder, WebviewUrl, Wry,
};
use url::Url;

use crate::{extensions, history, paths, session, settings, webview2};

pub const WINDOW: &str = "main";
pub const UI: &str = "ui";
const MAX_CLOSED: usize = 25;
const DEFAULT_UI_HEIGHT: f64 = 84.0;
const POPUP_TOGGLE_WINDOW: Duration = Duration::from_millis(400);
/// Popups that stay hidden until `PopupFrame` reports the content height.
const MEASURED_POPUPS: &[&str] = &["bookmark-folder", "print-pdf", "page-capture"];
const ZOOM_LEVELS: &[f64] = &[
    0.25, 0.33, 0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0,
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabInfo {
    pub id: u32,
    pub url: String,
    pub title: String,
    pub favicon: Option<String>,
    pub pinned: bool,
    pub loading: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub zoom: f64,
    /// False for restored tabs whose page view has not been created yet.
    pub loaded: bool,
    /// Browser window that shows this tab. `main` is the first window.
    pub window: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowTabs {
    pub label: String,
    pub active_id: Option<u32>,
    pub fullscreen: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabsSnapshot {
    pub tabs: Vec<TabInfo>,
    pub active_id: Option<u32>,
    /// Selected tabs in strip order. Includes the active tab whenever one is open.
    pub selected_ids: Vec<u32>,
    pub can_reopen: bool,
    pub fullscreen: bool,
    pub windows: Vec<WindowTabs>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PopupChange {
    pub kind: Option<String>,
    pub params: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PopupRequest {
    pub kind: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub params: String,
    #[serde(default = "default_true")]
    pub focus: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone)]
struct ClosedTab {
    url: String,
    title: String,
    pinned: bool,
    index: usize,
    window: String,
}

struct Tab {
    info: TabInfo,
    /// Loaded when the page view is created: restored tabs wait until they are selected.
    pending_url: Option<Url>,
    webview: Option<String>,
    /// The first navigation finished, so the webview can cover the page without a white frame.
    painted: bool,
    /// The current title belongs to `info.url`, not the page this tab showed before.
    title_ready: bool,
}

struct WindowChrome {
    label: String,
    ui: String,
    active: Option<u32>,
    ui_height: f64,
    fullscreen: bool,
    popup: Popup,
    shown: Option<String>,
}

impl WindowChrome {
    fn new(label: &str, ui: &str) -> Self {
        Self {
            label: label.to_string(),
            ui: ui.to_string(),
            active: None,
            ui_height: DEFAULT_UI_HEIGHT,
            fullscreen: false,
            popup: Popup::default(),
            shown: None,
        }
    }
}

#[derive(Default)]
struct Popup {
    label: Option<String>,
    kind: Option<String>,
    params: Option<String>,
    focus: bool,
    anchor_x: f64,
    anchor_y: f64,
    /// The webview has been hidden at least once, so showing it will not race the first paint.
    concealed: bool,
    /// The page finished loading, or the content reported its size.
    ready: bool,
    /// The placeholder size is safe to show. Folder menus stay false until content is measured.
    sized: bool,
    revealed: bool,
    closed_key: Option<String>,
    closed_at: Option<Instant>,
}

impl Popup {
    fn key(&self) -> Option<String> {
        Some(format!(
            "{}?{}",
            self.kind.as_deref()?,
            self.params.as_deref().unwrap_or_default()
        ))
    }
}

pub struct TabsState {
    tabs: Vec<Tab>,
    windows: Vec<WindowChrome>,
    /// Tabs in the multi-selection. The active tab is added when the selection is read.
    selected: Vec<u32>,
    /// Tab that a shift-click range starts from.
    anchor: Option<u32>,
    next_id: u32,
    next_window: u32,
    next_popup: u32,
    closed: Vec<ClosedTab>,
}

impl TabsState {
    fn index_of(&self, id: u32) -> Option<usize> {
        self.tabs.iter().position(|tab| tab.info.id == id)
    }

    fn tab_mut(&mut self, id: u32) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|tab| tab.info.id == id)
    }

    fn chrome(&self, label: &str) -> Option<&WindowChrome> {
        self.windows.iter().find(|window| window.label == label)
    }

    fn chrome_mut(&mut self, label: &str) -> Option<&mut WindowChrome> {
        self.windows.iter_mut().find(|window| window.label == label)
    }

    fn tab_window(&self, id: u32) -> Option<String> {
        self.tabs
            .iter()
            .find(|tab| tab.info.id == id)
            .map(|tab| tab.info.window.clone())
    }

    /// Global indexes of one window's tabs, in strip order.
    fn indexes_in(&self, window: &str) -> Vec<usize> {
        self.tabs
            .iter()
            .enumerate()
            .filter(|(_, tab)| tab.info.window == window)
            .map(|(index, _)| index)
            .collect()
    }

    fn pinned_count_in(&self, window: &str) -> usize {
        self.tabs
            .iter()
            .filter(|tab| tab.info.window == window)
            .take_while(|tab| tab.info.pinned)
            .count()
    }

    /// `local` is an index among this window's tabs. Returns where that sits in `self.tabs`.
    fn global_index(&self, window: &str, local: usize) -> usize {
        let indexes = self.indexes_in(window);
        indexes
            .get(local)
            .copied()
            .or_else(|| indexes.last().map(|index| index + 1))
            .unwrap_or(self.tabs.len())
    }

    fn active_of(&self, window: &str) -> Option<u32> {
        self.chrome(window).and_then(|chrome| chrome.active)
    }

    fn set_window_active(&mut self, window: &str, id: Option<u32>) {
        if let Some(chrome) = self.chrome_mut(window) {
            chrome.active = id;
        }
    }

    fn popup_for_label_mut(&mut self, label: &str) -> Option<&mut Popup> {
        self.windows
            .iter_mut()
            .find(|window| window.popup.label.as_deref() == Some(label))
            .map(|window| &mut window.popup)
    }

    fn snapshot(&self) -> TabsSnapshot {
        let main = self.chrome(WINDOW);
        TabsSnapshot {
            tabs: self.tabs.iter().map(|tab| tab.info.clone()).collect(),
            active_id: main.and_then(|chrome| chrome.active),
            selected_ids: self.selection_in_order(None),
            can_reopen: !self.closed.is_empty(),
            fullscreen: main.is_some_and(|chrome| chrome.fullscreen),
            windows: self
                .windows
                .iter()
                .map(|chrome| WindowTabs {
                    label: chrome.label.clone(),
                    active_id: chrome.active,
                    fullscreen: chrome.fullscreen,
                })
                .collect(),
        }
    }

    /// Selected tabs in strip order. `window` limits the list to one window and includes its active tab.
    fn selection_in_order(&self, window: Option<&str>) -> Vec<u32> {
        let selected = &self.selected;
        self.tabs
            .iter()
            .filter(|tab| window.is_none_or(|label| tab.info.window == label))
            .map(|tab| tab.info.id)
            .filter(|id| {
                let active = self
                    .tab_window(*id)
                    .and_then(|label| self.active_of(&label));
                selected.contains(id) || active == Some(*id)
            })
            .collect()
    }

    fn select_only(&mut self, id: u32) {
        self.selected.clear();
        self.selected.push(id);
        self.anchor = Some(id);
    }

    fn forget_tab(&mut self, id: u32, window: &str) {
        self.selected.retain(|selected| *selected != id);
        let active = self.active_of(window);
        if self.anchor == Some(id) {
            self.anchor = active.filter(|active| *active != id);
        }
        if let Some(active) = active.filter(|active| *active != id) {
            if !self.selected.contains(&active) {
                self.selected.push(active);
            }
        }
    }

    fn page_labels(&self) -> Vec<String> {
        self.tabs
            .iter()
            .filter_map(|tab| tab.webview.clone())
            .collect()
    }
}

pub struct Tabs(Mutex<TabsState>);

pub fn init(app: &AppHandle) {
    app.manage(Tabs(Mutex::new(TabsState {
        tabs: Vec::new(),
        windows: vec![WindowChrome::new(WINDOW, UI)],
        selected: Vec::new(),
        anchor: None,
        next_id: 1,
        next_window: 1,
        next_popup: 0,
        closed: Vec::new(),
    })));
}

/// Never hold this lock while calling into a webview or window getter: those wait on the
/// main thread, and WebView2 event handlers on the main thread take this lock too.
fn lock(app: &AppHandle) -> MutexGuard<'_, TabsState> {
    app.state::<Tabs>().inner().0.lock().expect("tabs lock")
}

fn with_state<T>(app: &AppHandle, change: impl FnOnce(&mut TabsState) -> T) -> T {
    let state = app.state::<Tabs>();
    let mut guard = state.inner().0.lock().expect("tabs lock");
    change(&mut guard)
}

pub fn snapshot(app: &AppHandle) -> TabsSnapshot {
    lock(app).snapshot()
}

pub fn is_browser_label(label: &str) -> bool {
    label == WINDOW || label.starts_with("main-")
}

/// The browser window the user is typing in, or the first window.
pub fn focused_window(app: &AppHandle) -> String {
    if let Some(window) = app.get_focused_window() {
        let label = window.label().to_string();
        if is_browser_label(&label) {
            return label;
        }
    }
    WINDOW.to_string()
}

pub fn browser_labels(app: &AppHandle) -> Vec<String> {
    lock(app)
        .windows
        .iter()
        .map(|window| window.label.clone())
        .collect()
}

pub fn any_ui(app: &AppHandle) -> Option<Webview> {
    let labels: Vec<String> = lock(app).windows.iter().map(|window| window.ui.clone()).collect();
    labels.iter().find_map(|label| app.get_webview(label))
}

pub fn active_id(app: &AppHandle) -> Option<u32> {
    let window = focused_window(app);
    lock(app).active_of(&window)
}

pub fn emit_tabs(app: &AppHandle) {
    let snapshot = snapshot(app);
    for chrome in &snapshot.windows {
        let Some(window) = app.get_window(&chrome.label) else {
            continue;
        };
        let title = chrome
            .active_id
            .and_then(|id| snapshot.tabs.iter().find(|tab| tab.id == id))
            .map(|tab| format!("{} - ce-browser", tab.title))
            .unwrap_or_else(|| "ce-browser".to_string());
        let _ = window.set_title(&title);
    }
    let _ = app.emit("tabs-changed", &snapshot);
    session::schedule(app);
}

/// Settings every webview shares. WebView2 refuses to create a webview whose environment
/// options differ from another webview that uses the same profile folder.
pub fn base_builder(app: &AppHandle, label: &str, url: WebviewUrl) -> WebviewBuilder<Wry> {
    let mut builder = WebviewBuilder::new(label, url)
        .data_directory(paths::profile_dir(app))
        .browser_extensions_enabled(true)
        .zoom_hotkeys_enabled(false);
    if let Some(environment) = any_ui(app).and_then(|ui| webview2::environment_of(&ui)) {
        builder = builder.with_environment(environment);
    }
    builder
}

pub fn resolve(app: &AppHandle, input: &str) -> Url {
    let search = settings::get(app).search_url();
    paths::resolve_input(app, input, &search)
}

pub fn home_url(app: &AppHandle) -> Url {
    let home = settings::get(app).home_page;
    resolve(app, &home)
}

pub struct OpenTab {
    pub url: Url,
    pub background: bool,
    pub index: Option<usize>,
    pub pinned: bool,
    pub title: Option<String>,
    /// Restored tabs keep their entry but create the page view on first activation.
    pub lazy: bool,
    /// Window that owns the new tab. Empty uses the focused browser window.
    pub window: String,
}

impl OpenTab {
    pub fn new(url: Url) -> Self {
        Self {
            url,
            background: false,
            index: None,
            pinned: false,
            title: None,
            lazy: false,
            window: String::new(),
        }
    }
}

pub fn create_tab(
    app: &AppHandle,
    url: Option<String>,
    background: bool,
    index: Option<usize>,
) -> Result<u32, String> {
    create_tab_on(app, &focused_window(app), url, background, index)
}

pub fn create_tab_on(
    app: &AppHandle,
    window: &str,
    url: Option<String>,
    background: bool,
    index: Option<usize>,
) -> Result<u32, String> {
    let url = match url
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        Some(text) => resolve(app, text),
        None => resolve(app, paths::NEW_TAB),
    };
    open(
        app,
        OpenTab {
            background,
            index,
            window: window.to_string(),
            ..OpenTab::new(url)
        },
    )
}

pub fn open(app: &AppHandle, tab: OpenTab) -> Result<u32, String> {
    let display = paths::display_url(app, tab.url.as_str());
    let zoom = page_zoom_for(app, &display);
    let window = if tab.window.is_empty() {
        focused_window(app)
    } else {
        tab.window.clone()
    };
    let id = with_state(app, |state| {
        let id = state.next_id;
        state.next_id += 1;
        let owned = state.indexes_in(&window).len();
        let pinned_count = state.pinned_count_in(&window);
        let local = if tab.pinned {
            tab.index.unwrap_or(pinned_count).min(pinned_count)
        } else {
            tab.index.unwrap_or(owned).clamp(pinned_count, owned)
        };
        let index = state.global_index(&window, local);
        let title = tab
            .title
            .clone()
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| display.clone());
        state.tabs.insert(
            index,
            Tab {
                info: TabInfo {
                    id,
                    url: display.clone(),
                    title,
                    favicon: None,
                    pinned: tab.pinned,
                    loading: false,
                    can_go_back: false,
                    can_go_forward: false,
                    zoom,
                    loaded: false,
                    window: window.clone(),
                },
                pending_url: Some(tab.url.clone()),
                webview: None,
                painted: false,
                title_ready: false,
            },
        );
        let none_active = state.active_of(&window).is_none();
        if !tab.background || (none_active && !tab.lazy) {
            state.set_window_active(&window, Some(id));
        }
        id
    });
    let is_active = lock(app).active_of(&window) == Some(id);
    if is_active {
        activate(app, id)?;
    } else {
        if !tab.lazy {
            ensure_webview(app, id)?;
        }
        emit_tabs(app);
    }
    Ok(id)
}

fn ensure_webview(app: &AppHandle, id: u32) -> Result<(), String> {
    let fallback = paths::internal_url(app, "newtab");
    let pending = with_state(app, |state| {
        let tab = state.tab_mut(id)?;
        if tab.webview.is_some() {
            return None;
        }
        let label = format!("tab-{id}");
        tab.webview = Some(label.clone());
        tab.info.loaded = true;
        tab.info.loading = true;
        Some((label, tab.pending_url.take().unwrap_or(fallback)))
    });
    let Some((label, url)) = pending else {
        return Ok(());
    };
    if let Some(window) = with_state(app, |state| state.tab_window(id)) {
        close_window_popup(app, &window);
    }
    if let Err(error) = build_page_view(app, id, &label, url.clone()) {
        with_state(app, |state| {
            if let Some(tab) = state.tab_mut(id) {
                tab.webview = None;
                tab.info.loaded = false;
                tab.info.loading = false;
                tab.pending_url = Some(url);
            }
        });
        return Err(error);
    }
    Ok(())
}

fn build_page_view(app: &AppHandle, id: u32, label: &str, url: Url) -> Result<Webview, String> {
    let window_label =
        with_state(app, |state| state.tab_window(id)).unwrap_or_else(|| WINDOW.to_string());
    let window = app
        .get_window(&window_label)
        .ok_or("The browser window is missing.")?;
    let (_, size) = page_rect(app, &window_label);
    let color = settings::page_color(app);
    let load_app = app.clone();
    let title_app = app.clone();
    let window_app = app.clone();
    let builder = base_builder(app, label, WebviewUrl::External(url))
        .background_color(color)
        .focused(false)
        .initialization_script(page_background_script(app))
        .on_page_load(move |_, payload| {
            let finished = matches!(payload.event(), PageLoadEvent::Finished);
            let loading = matches!(payload.event(), PageLoadEvent::Started);
            let url = paths::display_url(&load_app, payload.url().as_str());
            let new_tab_page = url == paths::NEW_TAB;
            let fresh = with_state(&load_app, |state| {
                state.tab_mut(id).is_some_and(|tab| !tab.painted)
            });
            let first = finished && fresh;
            update(&load_app, id, |tab| {
                if tab.info.url != url {
                    tab.title_ready = false;
                }
                tab.info.loading = loading;
                tab.info.url = url;
                if finished {
                    tab.painted = true;
                }
            });
            let owner = with_state(&load_app, |state| state.tab_window(id))
                .unwrap_or_else(|| WINDOW.to_string());
            if finished {
                remember_visit(&load_app, id);
                place_pages(&load_app, &owner, false);
            }
            let active = lock(&load_app).active_of(&owner) == Some(id);
            // The new page view takes focus as soon as it starts loading. Take it back
            // for the new tab page then, instead of waiting until the document finishes.
            if active && new_tab_page && fresh && (loading || finished) {
                focus_active(&load_app, &owner);
            } else if first && active {
                focus_active(&load_app, &owner);
            }
        })
        .on_document_title_changed(move |_, title| {
            let url = with_state(&title_app, |state| {
                let Some(tab) = state.tab_mut(id) else {
                    return None;
                };
                tab.title_ready = true;
                tab.info.title = if title.trim().is_empty() {
                    tab.info.url.clone()
                } else {
                    title
                };
                Some((tab.info.url.clone(), tab.info.title.clone()))
            });
            if let Some((url, title)) = url {
                history::set_title(&title_app, &url, &title);
                emit_tabs(&title_app);
            }
        })
        .on_new_window(move |url, features| new_window(&window_app, Some(id), url, features));
    // A visible webview's first frame is white. Keep it off the page until the document has painted.
    let webview = window
        .add_child(builder, parked_position(), size)
        .map_err(|error| error.to_string())?;
    webview2::attach_page_hooks(app, &webview, id);
    let zoom = with_state(app, |state| {
        state
            .index_of(id)
            .map(|index| state.tabs[index].info.zoom)
            .unwrap_or(1.0)
    });
    if (zoom - 1.0).abs() > 0.001 {
        let _ = webview.set_zoom(zoom);
    }
    Ok(webview)
}

fn page_zoom_for(app: &AppHandle, url: &str) -> f64 {
    if url.starts_with(paths::INTERNAL_SCHEME) {
        1.0
    } else {
        settings::get(app).page_zoom_factor()
    }
}

/// Paints app pages before their CSS loads. The app stylesheet comes later and overrides this.
pub fn page_background_script(app: &AppHandle) -> String {
    let origin = paths::app_origin(app).origin().ascii_serialization();
    let color = settings::page_color(app);
    format!(
        r##"(function () {{
  if (location.origin !== "{origin}") return;
  if (location.hash.indexOf("#/popup/") === 0) return;
  var style = document.createElement("style");
  style.id = "ce-page-background";
  style.textContent = "html,body{{background-color:#{r:02x}{g:02x}{b:02x}}}";
  document.documentElement.appendChild(style);
}})();"##,
        origin = origin,
        r = color.0,
        g = color.1,
        b = color.2,
    )
}

/// `target="_blank"` links become tabs. Script popups that ask for a size (OAuth sign-in,
/// payment windows) open as real windows so `window.opener` keeps working.
pub fn new_window(
    app: &AppHandle,
    opener: Option<u32>,
    url: Url,
    features: NewWindowFeatures,
) -> NewWindowResponse<Wry> {
    blank_tab(app, None, opener, url, features)
}

/// A sized `window.open` has to be a top-level window that shares the opener's WebView2
/// environment. Letting WebView2 build the default popup freezes this process and leaves
/// a window with no title bar, because every page view here is a child of the browser window.
fn script_window(app: &AppHandle, url: Url, features: NewWindowFeatures) -> NewWindowResponse<Wry> {
    let Some(size) = features.size() else {
        return NewWindowResponse::Deny;
    };
    let label = with_state(app, |state| {
        state.next_popup += 1;
        format!("script-{}", state.next_popup)
    });
    let environment = features.opener().environment.clone();
    let position = features.position();
    let host = app.clone();
    let mut builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(url))
        .title("ce-browser")
        .inner_size(size.width.max(240.0), size.height.max(160.0))
        .min_inner_size(240.0, 160.0)
        .decorations(true)
        .shadow(true)
        .focused(true)
        .with_environment(environment)
        .on_new_window(move |url, features| new_window(&host, None, url, features));
    if let Some(position) = position {
        builder = builder.position(position.x, position.y);
    }
    match builder.build() {
        Ok(window) => NewWindowResponse::Create { window },
        Err(error) => {
            eprintln!("script window: {error}");
            NewWindowResponse::Deny
        }
    }
}

fn blank_tab(
    app: &AppHandle,
    host: Option<String>,
    opener: Option<u32>,
    url: Url,
    features: NewWindowFeatures,
) -> NewWindowResponse<Wry> {
    if features.size().is_some() {
        return script_window(app, url, features);
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let (window, index) = match opener {
            Some(id) => {
                let state = lock(&app);
                let window = state
                    .tab_window(id)
                    .unwrap_or_else(|| host.unwrap_or_else(|| WINDOW.to_string()));
                let local = state
                    .indexes_in(&window)
                    .iter()
                    .position(|index| state.tabs[*index].info.id == id)
                    .map(|index| index + 1);
                (window, local)
            }
            None => (host.unwrap_or_else(|| focused_window(&app)), None),
        };
        let _ = open(
            &app,
            OpenTab {
                index,
                window,
                ..OpenTab::new(url)
            },
        );
    });
    NewWindowResponse::Deny
}

/// Opens another browser window in this process. Page views share the profile folder.
pub fn create_browser_window(app: &AppHandle, label: &str) -> Result<(), String> {
    let ui = with_state(app, |state| {
        state.chrome(label).map(|chrome| chrome.ui.clone())
    })
    .ok_or("The browser window is missing.")?;
    let window = WindowBuilder::new(app, label)
        .title(crate::profiles::window_title(app))
        .inner_size(1280.0, 840.0)
        .min_inner_size(640.0, 420.0)
        .decorations(false)
        .shadow(true)
        .build()
        .map_err(|error| error.to_string())?;
    crate::profiles::apply_window_icon(app, &window);
    let size = window.inner_size().map_err(|error| error.to_string())?;
    let (r, g, b) = settings::page_surface(app);
    let background = Color(r, g, b, 255);
    let _ = window.set_background_color(Some(background));
    let ui_app = app.clone();
    let host = label.to_string();
    let ui_view = window
        .add_child(
            base_builder(app, &ui, WebviewUrl::App("index.html".into()))
                .background_color(background)
                .initialization_script(page_background_script(app))
                .on_new_window(move |url, features| {
                    blank_tab(&ui_app, Some(host.clone()), None, url, features)
                }),
            LogicalPosition::new(0.0, 0.0),
            size,
        )
        .map_err(|error| error.to_string())?;
    webview2::attach_ui_hooks(app, &ui_view);
    let _ = window.set_focus();
    Ok(())
}

fn allocate_window(app: &AppHandle) -> String {
    with_state(app, |state| {
        state.next_window += 1;
        let number = state.next_window;
        let label = format!("main-{number}");
        let ui = format!("ui-{number}");
        state.windows.push(WindowChrome::new(&label, &ui));
        label
    })
}

/// Moves a tab into a new browser window. An empty source window closes.
pub fn detach_tab(app: &AppHandle, id: u32) -> Result<(), String> {
    let (source, alone) = with_state(app, |state| {
        let source = state.tab_window(id)?;
        let alone = state.indexes_in(&source).len() <= 1;
        Some((source, alone))
    })
    .ok_or("That tab is no longer open.")?;
    let label = allocate_window(app);
    if let Err(error) = create_browser_window(app, &label) {
        with_state(app, |state| {
            state.windows.retain(|window| window.label != label);
        });
        return Err(error);
    }
    if let (Some(source_window), Some(created)) = (app.get_window(&source), app.get_window(&label))
        && let Ok(position) = source_window.outer_position()
    {
        let _ = created.set_position(PhysicalPosition::new(position.x + 48, position.y + 48));
    }
    let next = with_state(app, |state| {
        let next = state
            .indexes_in(&source)
            .into_iter()
            .find(|index| state.tabs[*index].info.id != id)
            .map(|index| state.tabs[index].info.id);
        if let Some(tab) = state.tab_mut(id) {
            tab.info.window = label.clone();
        }
        if state.active_of(&source) == Some(id) {
            state.set_window_active(&source, next);
        }
        state.set_window_active(&label, Some(id));
        next
    });
    // Closing the source window destroys webviews Tauri still owns, so the last tab is
    // rebuilt on the new window instead of reparented.
    if alone || !reparent_page(app, id, &label) {
        let saved = with_state(app, |state| {
            let tab = state.tab_mut(id)?;
            let pending = tab.pending_url.clone();
            let display = tab.info.url.clone();
            let webview = tab.webview.take();
            tab.info.loaded = false;
            tab.info.loading = false;
            tab.painted = false;
            Some((webview, pending, display))
        });
        if let Some((webview, pending, display)) = saved {
            retire_webview(app, webview);
            let url = pending.unwrap_or_else(|| resolve(app, &display));
            with_state(app, |state| {
                if let Some(tab) = state.tab_mut(id) {
                    tab.pending_url = Some(url);
                }
            });
        }
    }
    let source_empty = with_state(app, |state| state.indexes_in(&source).is_empty());
    if source_empty {
        close_window_popup(app, &source);
        with_state(app, |state| {
            state.windows.retain(|window| window.label != source);
        });
        if let Some(window) = app.get_window(&source) {
            let _ = window.close();
        }
    } else if let Some(next) = next {
        ensure_webview(app, next)?;
        place_pages(app, &source, false);
        layout(app, &source);
    }
    ensure_webview(app, id)?;
    place_pages(app, &label, true);
    layout(app, &label);
    emit_tabs(app);
    Ok(())
}

fn reparent_page(app: &AppHandle, id: u32, window_label: &str) -> bool {
    let Some(webview) = page_view(app, id) else {
        return true;
    };
    let Some(window) = app.get_window(window_label) else {
        return false;
    };
    // Move wry's child container. SetParentWindow on the controller pulls the page out of
    // that container and stretches it over the whole window, which hides the title buttons.
    webview.reparent(&window).is_ok()
}

/// Drops a window's tabs while it is closing. The last window keeps them for the session save.
pub fn discard_window(app: &AppHandle, label: &str) {
    if !browser_labels(app).iter().any(|item| item == label) {
        return;
    }
    let ids = with_state(app, |state| {
        state
            .indexes_in(label)
            .into_iter()
            .map(|index| state.tabs[index].info.id)
            .collect::<Vec<_>>()
    });
    for id in ids {
        let _ = close_tab(app, id);
    }
    close_window_popup(app, label);
    with_state(app, |state| {
        state.windows.retain(|window| window.label != label);
    });
    emit_tabs(app);
}

fn update(app: &AppHandle, id: u32, change: impl FnOnce(&mut Tab)) {
    let found = with_state(app, |state| match state.tab_mut(id) {
        Some(tab) => {
            change(tab);
            true
        }
        None => false,
    });
    if found {
        emit_tabs(app);
    }
}

pub fn update_history(app: &AppHandle, id: u32, back: bool, forward: bool) {
    update(app, id, |tab| {
        tab.info.can_go_back = back;
        tab.info.can_go_forward = forward;
    });
}

pub fn update_url(app: &AppHandle, id: u32, raw: &str) {
    let url = paths::display_url(app, raw);
    let default_zoom = settings::get(app).page_zoom_factor();
    let change = with_state(app, |state| {
        let Some(tab) = state.tab_mut(id) else {
            return None;
        };
        if tab.info.url == url {
            return Some((false, None));
        }
        let was_internal = tab.info.url.starts_with(paths::INTERNAL_SCHEME);
        let now_internal = url.starts_with(paths::INTERNAL_SCHEME);
        let loading = tab.info.loading;
        tab.title_ready = false;
        tab.info.url = url.clone();
        let zoom = match (was_internal, now_internal) {
            (_, true) if !was_internal => Some(1.0),
            (true, false) => Some(default_zoom),
            _ => None,
        };
        if let Some(zoom) = zoom {
            tab.info.zoom = zoom;
        }
        Some((!loading, zoom))
    });
    let Some((record, zoom)) = change else {
        return;
    };
    if let Some(zoom) = zoom
        && let Some(webview) = page_view(app, id)
    {
        let _ = webview.set_zoom(zoom);
    }
    if record {
        remember_visit(app, id);
    }
    emit_tabs(app);
}

fn remember_visit(app: &AppHandle, id: u32) {
    let Some((url, title)) = with_state(app, |state| {
        let index = state.index_of(id)?;
        let tab = &state.tabs[index];
        let title = if tab.title_ready {
            tab.info.title.clone()
        } else {
            String::new()
        };
        Some((tab.info.url.clone(), title))
    }) else {
        return;
    };
    history::visit(app, &url, &title);
}

pub fn update_favicon(app: &AppHandle, id: u32, favicon: Option<String>) {
    update(app, id, |tab| tab.info.favicon = favicon);
}

pub fn activate(app: &AppHandle, id: u32) -> Result<(), String> {
    set_active(app, id, true)
}

/// `collapse` replaces the selection with this tab. Shift- and ctrl-clicks keep the group.
fn set_active(app: &AppHandle, id: u32, collapse: bool) -> Result<(), String> {
    let window = with_state(app, |state| {
        let window = state.tab_window(id)?;
        state.set_window_active(&window, Some(id));
        if collapse {
            state.select_only(id);
        } else if !state.selected.contains(&id) {
            state.selected.push(id);
        }
        Some(window)
    });
    let Some(window) = window else {
        return Err("That tab is no longer open.".into());
    };
    ensure_webview(app, id)?;
    place_pages(app, &window, false);
    emit_tabs(app);
    focus_active(app, &window);
    Ok(())
}

/// Ctrl-click: add or remove one tab. The active tab stays put unless it was removed.
pub fn toggle_selected(app: &AppHandle, id: u32) -> Result<(), String> {
    enum Edit {
        Missing,
        Same,
        Emit,
        Show(u32),
    }
    let edit = with_state(app, |state| {
        let Some(index) = state.index_of(id) else {
            return Edit::Missing;
        };
        let Some(window) = state.tab_window(id) else {
            return Edit::Missing;
        };
        let selected = state.selection_in_order(Some(&window));
        let active = state.active_of(&window);
        if selected.contains(&id) {
            if selected.len() <= 1 {
                return Edit::Same;
            }
            state.selected.retain(|selected_id| *selected_id != id);
            state.anchor = Some(id);
            if active != Some(id) {
                return Edit::Emit;
            }
            let remaining = state.selected.clone();
            let replacement = state
                .indexes_in(&window)
                .into_iter()
                .filter(|tab_index| remaining.contains(&state.tabs[*tab_index].info.id))
                .min_by_key(|tab_index| tab_index.abs_diff(index))
                .map(|tab_index| state.tabs[tab_index].info.id);
            return match replacement {
                Some(next) => Edit::Show(next),
                None => Edit::Same,
            };
        }
        state.selected.push(id);
        if let Some(active) = active {
            if !state.selected.contains(&active) {
                state.selected.push(active);
            }
        }
        state.anchor = Some(id);
        Edit::Emit
    });
    match edit {
        Edit::Missing => Err("That tab is no longer open.".into()),
        Edit::Same => Ok(()),
        Edit::Emit => {
            emit_tabs(app);
            Ok(())
        }
        Edit::Show(next) => set_active(app, next, false),
    }
}

/// Shift-click: select every tab from the anchor through `id`.
pub fn select_range(app: &AppHandle, id: u32) -> Result<(), String> {
    let found = with_state(app, |state| {
        let Some(window) = state.tab_window(id) else {
            return false;
        };
        let indexes = state.indexes_in(&window);
        let Some(target) = indexes.iter().position(|index| state.tabs[*index].info.id == id) else {
            return false;
        };
        let anchor = state
            .anchor
            .filter(|anchor| state.tab_window(*anchor).as_deref() == Some(window.as_str()))
            .and_then(|anchor| indexes.iter().position(|index| state.tabs[*index].info.id == anchor))
            .or_else(|| {
                state
                    .active_of(&window)
                    .and_then(|active| indexes.iter().position(|index| state.tabs[*index].info.id == active))
            })
            .unwrap_or(target);
        let start = anchor.min(target);
        let end = anchor.max(target);
        state.selected = indexes[start..=end]
            .iter()
            .map(|index| state.tabs[*index].info.id)
            .collect();
        if let Some(active) = state.active_of(&window) {
            if !state.selected.contains(&active) {
                state.selected.push(active);
            }
        }
        true
    });
    if !found {
        return Err("That tab is no longer open.".into());
    }
    emit_tabs(app);
    Ok(())
}

fn parked_position() -> LogicalPosition<f64> {
    LogicalPosition::new(-10000.0, -10000.0)
}

/// Puts the active page on screen once it has painted. Until then the previous page stays,
/// so opening or closing a tab does not flash the window through.
fn place_pages(app: &AppHandle, window: &str, resize: bool) {
    let (position, page_size) = page_rect(app, window);
    let (shown, parked, move_shown) = with_state(app, |state| {
        let active = state.active_of(window);
        let ready = active
            .and_then(|id| state.tabs.iter().find(|tab| tab.info.id == id))
            .filter(|tab| tab.painted)
            .and_then(|tab| tab.webview.clone());
        let previous = state.chrome(window).and_then(|chrome| chrome.shown.clone());
        let shown = ready.or_else(|| {
            previous.clone().filter(|label| {
                state.tabs.iter().any(|tab| {
                    tab.info.window == window && tab.painted && tab.webview.as_ref() == Some(label)
                })
            })
        });
        let move_shown = resize || previous != shown;
        if let Some(chrome) = state.chrome_mut(window) {
            chrome.shown = shown.clone();
        }
        let parked: Vec<String> = state
            .indexes_in(window)
            .into_iter()
            .filter_map(|index| state.tabs[index].webview.clone())
            .filter(|label| Some(label) != shown.as_ref())
            .collect();
        (shown, parked, move_shown)
    });
    if !resize && !move_shown {
        return;
    }
    if move_shown {
        if let Some(webview) = shown.and_then(|label| app.get_webview(&label)) {
            let _ = webview.set_size(page_size);
            let _ = webview.set_position(position);
        }
    }
    for label in parked {
        if let Some(webview) = app.get_webview(&label) {
            let _ = webview.set_size(page_size);
            let _ = webview.set_position(parked_position());
        }
    }
}

fn retire_webview(app: &AppHandle, label: Option<String>) {
    let Some(webview) = label.and_then(|label| app.get_webview(&label)) else {
        return;
    };
    let _ = webview.set_position(parked_position());
    let _ = webview.close();
}

fn focus_active(app: &AppHandle, window: &str) {
    let target = with_state(app, |state| {
        let id = state.active_of(window)?;
        let tab = state.tabs.iter().find(|tab| tab.info.id == id)?;
        let shown = state.chrome(window).and_then(|chrome| chrome.shown.clone());
        let on_screen = tab.webview.is_some() && shown == tab.webview;
        Some((tab.webview.clone(), tab.info.url.clone(), on_screen))
    });
    let Some((label, url, on_screen)) = target else {
        return;
    };
    if url == paths::NEW_TAB {
        send_command(app, window, "focus-address");
        let app = app.clone();
        let window = window.to_string();
        let later = app.clone();
        let _ = app.run_on_main_thread(move || {
            let still = with_state(&later, |state| {
                let id = state.active_of(&window)?;
                state
                    .tabs
                    .iter()
                    .find(|tab| tab.info.id == id)
                    .map(|tab| tab.info.url.clone())
            });
            if still.as_deref() == Some(paths::NEW_TAB) {
                send_command(&later, &window, "focus-address");
            }
        });
        return;
    }
    if !on_screen {
        return;
    }
    if let Some(webview) = label.and_then(|label| app.get_webview(&label)) {
        let _ = webview.set_focus();
    }
}

/// Focuses one window's browser UI and tells it what to do (`focus-address`, `find`, `bookmark`...).
pub fn send_command(app: &AppHandle, window: &str, command: &str) {
    let ui = with_state(app, |state| {
        state.chrome(window).map(|chrome| chrome.ui.clone())
    });
    if let Some(ui) = ui.and_then(|label| app.get_webview(&label)) {
        let _ = ui.set_focus();
        let _ = ui.emit("browser-command", command);
    }
}

pub fn close_tab(app: &AppHandle, id: u32) -> Result<(), String> {
    let result = with_state(app, |state| {
        let index = state.index_of(id)?;
        let window = state.tabs[index].info.window.clone();
        let local = state
            .indexes_in(&window)
            .iter()
            .position(|tab_index| *tab_index == index)
            .unwrap_or(0);
        let tab = state.tabs.remove(index);
        if tab.info.url != paths::NEW_TAB {
            state.closed.push(ClosedTab {
                url: tab.info.url.clone(),
                title: tab.info.title.clone(),
                pinned: tab.info.pinned,
                index: local,
                window: window.clone(),
            });
            if state.closed.len() > MAX_CLOSED {
                state.closed.remove(0);
            }
        }
        let was_active = state.active_of(&window) == Some(id);
        if was_active {
            let indexes = state.indexes_in(&window);
            let next = indexes
                .get(local)
                .or_else(|| local.checked_sub(1).and_then(|previous| indexes.get(previous)))
                .map(|tab_index| state.tabs[*tab_index].info.id);
            state.set_window_active(&window, next);
        }
        let next = state.active_of(&window);
        let empty = state.indexes_in(&window).is_empty();
        let last_window = state.windows.len() <= 1;
        state.forget_tab(id, &window);
        Some((tab.webview, was_active, next, empty, last_window, window))
    });
    let Some((label, was_active, next, empty, last_window, window)) = result else {
        return Ok(());
    };
    if empty {
        retire_webview(app, label);
        if last_window {
            create_tab_on(app, &window, None, false, None)?;
        }
        return Ok(());
    }
    let ensured = if was_active {
        match next {
            Some(next) => ensure_webview(app, next),
            None => Ok(()),
        }
    } else {
        Ok(())
    };
    // Move the replacement on screen before the closed view is destroyed.
    place_pages(app, &window, false);
    retire_webview(app, label);
    ensured?;
    emit_tabs(app);
    if was_active {
        focus_active(app, &window);
    }
    Ok(())
}

/// Closes every selected tab. A single selection closes the active tab.
pub fn close_selected(app: &AppHandle) -> Result<(), String> {
    let window = focused_window(app);
    let (ids, active) = with_state(app, |state| {
        (
            state.selection_in_order(Some(&window)),
            state.active_of(&window),
        )
    });
    let targets = if ids.len() > 1 {
        ids
    } else {
        active.into_iter().collect()
    };
    for id in targets.iter().copied().filter(|id| Some(*id) != active) {
        close_tab(app, id)?;
    }
    if let Some(id) = active.filter(|id| targets.contains(id)) {
        close_tab(app, id)?;
    }
    Ok(())
}

pub fn close_other_tabs(app: &AppHandle, id: u32) -> Result<(), String> {
    let ids: Vec<u32> = with_state(app, |state| {
        let Some(window) = state.tab_window(id) else {
            return Vec::new();
        };
        state
            .tabs
            .iter()
            .filter(|tab| tab.info.window == window && tab.info.id != id && !tab.info.pinned)
            .map(|tab| tab.info.id)
            .collect()
    });
    activate(app, id)?;
    for other in ids {
        close_tab(app, other)?;
    }
    Ok(())
}

pub fn close_tabs_to_right(app: &AppHandle, id: u32) -> Result<(), String> {
    let ids: Vec<u32> = with_state(app, |state| {
        let Some(window) = state.tab_window(id) else {
            return Vec::new();
        };
        let Some(local) = state
            .indexes_in(&window)
            .iter()
            .position(|index| state.tabs[*index].info.id == id)
        else {
            return Vec::new();
        };
        state
            .indexes_in(&window)
            .into_iter()
            .skip(local + 1)
            .map(|index| &state.tabs[index])
            .filter(|tab| !tab.info.pinned)
            .map(|tab| tab.info.id)
            .collect()
    });
    for other in ids {
        close_tab(app, other)?;
    }
    Ok(())
}

pub fn reopen_closed(app: &AppHandle) -> Result<Option<u32>, String> {
    let Some(closed) = with_state(app, |state| state.closed.pop()) else {
        return Ok(None);
    };
    let url = resolve(app, &closed.url);
    let window = if browser_labels(app).iter().any(|label| label == &closed.window) {
        closed.window
    } else {
        focused_window(app)
    };
    open(
        app,
        OpenTab {
            index: Some(closed.index),
            pinned: closed.pinned,
            title: Some(closed.title),
            window,
            ..OpenTab::new(url)
        },
    )
    .map(Some)
}

pub fn page_view(app: &AppHandle, id: u32) -> Option<Webview> {
    let label = with_state(app, |state| {
        state.tab_mut(id).and_then(|tab| tab.webview.clone())
    })?;
    app.get_webview(&label)
}

pub fn navigate(app: &AppHandle, id: u32, input: &str) -> Result<(), String> {
    let url = resolve(app, input);
    let display = paths::display_url(app, url.as_str());
    let label = with_state(app, |state| {
        let tab = state.tab_mut(id)?;
        tab.info.url = display.clone();
        tab.info.loading = true;
        if tab.webview.is_none() {
            tab.pending_url = Some(url.clone());
        }
        Some(tab.webview.clone())
    })
    .ok_or("That tab is no longer open.")?;
    match label.and_then(|label| app.get_webview(&label)) {
        Some(webview) => {
            webview.navigate(url).map_err(|error| error.to_string())?;
            if display != paths::NEW_TAB {
                let _ = webview.set_focus();
            }
        }
        None => ensure_webview(app, id)?,
    }
    emit_tabs(app);
    Ok(())
}

pub fn go_back(app: &AppHandle, id: u32) {
    if let Some(webview) = page_view(app, id) {
        webview2::go_back(&webview);
    }
}

pub fn go_forward(app: &AppHandle, id: u32) {
    if let Some(webview) = page_view(app, id) {
        webview2::go_forward(&webview);
    }
}

pub fn stop(app: &AppHandle, id: u32) {
    if let Some(webview) = page_view(app, id) {
        webview2::stop(&webview);
    }
}

pub fn reload(app: &AppHandle, id: u32) -> Result<(), String> {
    match page_view(app, id) {
        Some(webview) => webview.reload().map_err(|error| error.to_string()),
        None => activate(app, id),
    }
}

pub fn clear_site_cache(app: &AppHandle, id: u32) -> Result<(), String> {
    let url = with_state(app, |state| {
        state
            .tabs
            .iter()
            .find(|tab| tab.info.id == id)
            .map(|tab| tab.info.url.clone())
    })
    .ok_or("That tab is no longer open.")?;
    let origin = paths::http_origin(&url).ok_or("This page has no site cache to clear.")?;
    let webview = page_view(app, id).ok_or("This page is not loaded yet.")?;
    webview2::clear_site_cache(&webview, &origin)
}

pub fn move_tab(app: &AppHandle, id: u32, to: usize) {
    let moved = with_state(app, |state| {
        let Some(from) = state.index_of(id) else {
            return false;
        };
        let window = state.tabs[from].info.window.clone();
        let tab = state.tabs.remove(from);
        let owned = state.indexes_in(&window).len();
        let pinned_count = state.pinned_count_in(&window);
        let local = if tab.info.pinned {
            to.min(pinned_count)
        } else {
            to.clamp(pinned_count, owned)
        };
        let index = state.global_index(&window, local);
        state.tabs.insert(index, tab);
        true
    });
    if moved {
        emit_tabs(app);
    }
}

pub fn set_pinned(app: &AppHandle, id: u32, pinned: bool) {
    let changed = with_state(app, |state| {
        let Some(from) = state.index_of(id) else {
            return false;
        };
        let window = state.tabs[from].info.window.clone();
        let mut tab = state.tabs.remove(from);
        tab.info.pinned = pinned;
        let local = state.pinned_count_in(&window);
        let to = state.global_index(&window, local);
        state.tabs.insert(to, tab);
        true
    });
    if changed {
        emit_tabs(app);
    }
}

pub fn duplicate_tab(app: &AppHandle, id: u32) -> Result<u32, String> {
    let (url, index, window) = with_state(app, |state| {
        let index = state.index_of(id)?;
        let window = state.tabs[index].info.window.clone();
        let local = state
            .indexes_in(&window)
            .iter()
            .position(|tab_index| *tab_index == index)?;
        Some((state.tabs[index].info.url.clone(), local, window))
    })
    .ok_or("That tab is no longer open.")?;
    let url = resolve(app, &url);
    open(
        app,
        OpenTab {
            index: Some(index + 1),
            window,
            ..OpenTab::new(url)
        },
    )
}

pub fn cycle(app: &AppHandle, step: isize) -> Result<(), String> {
    let window = focused_window(app);
    let next = with_state(app, |state| {
        let indexes = state.indexes_in(&window);
        let count = indexes.len() as isize;
        if count == 0 {
            return None;
        }
        let current = state
            .active_of(&window)
            .and_then(|id| indexes.iter().position(|index| state.tabs[*index].info.id == id))
            .unwrap_or(0) as isize;
        let index = indexes[(current + step).rem_euclid(count) as usize];
        Some(state.tabs[index].info.id)
    });
    match next {
        Some(id) => activate(app, id),
        None => Ok(()),
    }
}

/// `None` selects the last tab (Ctrl+9).
pub fn select_index(app: &AppHandle, index: Option<usize>) -> Result<(), String> {
    let window = focused_window(app);
    let id = with_state(app, |state| {
        let indexes = state.indexes_in(&window);
        let global = match index {
            Some(index) => indexes.get(index).copied(),
            None => indexes.last().copied(),
        }?;
        Some(state.tabs[global].info.id)
    });
    match id {
        Some(id) => activate(app, id),
        None => Ok(()),
    }
}

/// `change` is `in`, `out` or `reset`.
pub fn zoom(app: &AppHandle, id: u32, change: &str) -> Result<f64, String> {
    let default_zoom = settings::get(app).page_zoom_factor();
    let next = with_state(app, |state| {
        let tab = state.tab_mut(id)?;
        let current = tab.info.zoom;
        let reset = if tab.info.url.starts_with(paths::INTERNAL_SCHEME) {
            1.0
        } else {
            default_zoom
        };
        let next = match change {
            "in" => ZOOM_LEVELS
                .iter()
                .copied()
                .find(|level| *level > current + 0.001),
            "out" => ZOOM_LEVELS
                .iter()
                .rev()
                .copied()
                .find(|level| *level < current - 0.001),
            _ => Some(reset),
        }
        .unwrap_or(current);
        tab.info.zoom = next;
        Some(next)
    })
    .ok_or("That tab is no longer open.")?;
    if let Some(webview) = page_view(app, id) {
        webview.set_zoom(next).map_err(|error| error.to_string())?;
    }
    emit_tabs(app);
    Ok(next)
}

/// Pages still on the previous default follow a new default. A page zoomed with Ctrl +/- stays put.
pub fn apply_default_page_zoom(app: &AppHandle, previous_percent: u32, next_percent: u32) {
    if previous_percent == next_percent {
        return;
    }
    let previous = f64::from(previous_percent) / 100.0;
    let next = f64::from(next_percent) / 100.0;
    let ids = with_state(app, |state| {
        let mut ids = Vec::new();
        for tab in &mut state.tabs {
            if tab.info.url.starts_with(paths::INTERNAL_SCHEME) {
                continue;
            }
            if (tab.info.zoom - previous).abs() > 0.02 {
                continue;
            }
            tab.info.zoom = next;
            ids.push(tab.info.id);
        }
        ids
    });
    if ids.is_empty() {
        return;
    }
    for id in ids {
        if let Some(webview) = page_view(app, id) {
            let _ = webview.set_zoom(next);
        }
    }
    emit_tabs(app);
}

pub fn open_devtools(app: &AppHandle, id: u32) {
    if let Some(webview) = page_view(app, id) {
        webview.open_devtools();
    }
}

pub fn find_in_page(
    app: &AppHandle,
    id: u32,
    query: &str,
    backwards: bool,
) -> Result<bool, String> {
    let webview = page_view(app, id).ok_or("That tab has no page loaded.")?;
    let query = serde_json::to_string(query).map_err(|error| error.to_string())?;
    let script = format!("window.find({query}, false, {backwards}, true, false, false, false)");
    let (tx, rx) = std::sync::mpsc::channel();
    webview
        .eval_with_callback(script, move |result| {
            let _ = tx.send(result.trim() == "true");
        })
        .map_err(|error| error.to_string())?;
    Ok(rx.recv_timeout(Duration::from_secs(2)).unwrap_or(false))
}

pub fn clear_find(app: &AppHandle, id: u32) {
    if let Some(webview) = page_view(app, id) {
        let _ = webview.eval("window.getSelection()?.removeAllRanges()");
    }
}

pub fn set_fullscreen(app: &AppHandle, id: u32, fullscreen: bool) {
    let window_label = with_state(app, |state| {
        let window = state.tab_window(id)?;
        let chrome = state.chrome_mut(&window)?;
        if chrome.active != Some(id) || chrome.fullscreen == fullscreen {
            return None;
        }
        chrome.fullscreen = fullscreen;
        Some(window)
    });
    let Some(window_label) = window_label else {
        return;
    };
    let app = app.clone();
    let layout_label = window_label.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(window) = app.get_window(&layout_label) {
            let _ = window.set_fullscreen(fullscreen);
        }
        layout(&app, &layout_label);
        emit_tabs(&app);
    });
}

pub fn set_ui_height(app: &AppHandle, window: &str, height: f64) {
    let changed = with_state(app, |state| {
        // A zero report is the header before it has laid out. Treating it as the real
        // height parks every page at the top of the window and covers the window buttons.
        if height < 1.0 {
            return None;
        }
        let chrome = state.chrome_mut(window)?;
        if (chrome.ui_height - height).abs() < 0.5 {
            return None;
        }
        chrome.ui_height = height;
        Some(())
    });
    if changed.is_some() {
        layout(app, window);
    }
}

/// Physical rectangle for page views: below the browser UI, or the whole window in fullscreen.
fn page_rect(app: &AppHandle, window_label: &str) -> (PhysicalPosition<i32>, PhysicalSize<u32>) {
    let (ui_height, fullscreen) = with_state(app, |state| {
        state
            .chrome(window_label)
            .map(|chrome| (chrome.ui_height, chrome.fullscreen))
            .unwrap_or((DEFAULT_UI_HEIGHT, false))
    });
    let Some(window) = app.get_window(window_label) else {
        return (PhysicalPosition::new(0, 0), PhysicalSize::new(800, 600));
    };
    let size = window.inner_size().unwrap_or(PhysicalSize::new(800, 600));
    if fullscreen {
        return (PhysicalPosition::new(0, 0), size);
    }
    let scale = window.scale_factor().unwrap_or(1.0);
    let top = ((ui_height * scale).round() as u32).min(size.height);
    (
        PhysicalPosition::new(0, top as i32),
        PhysicalSize::new(size.width, size.height - top),
    )
}

pub fn layout(app: &AppHandle, window_label: &str) {
    let Some(window) = app.get_window(window_label) else {
        return;
    };
    let Ok(size) = window.inner_size() else {
        return;
    };
    let ui_label = with_state(app, |state| {
        state.chrome(window_label).map(|chrome| chrome.ui.clone())
    });
    if let Some(ui) = ui_label.and_then(|label| app.get_webview(&label)) {
        let _ = ui.set_position(PhysicalPosition::new(0, 0));
        let _ = ui.set_size(size);
    }
    place_pages(app, window_label, true);
    close_window_popup(app, window_label);
}

/// Window and webview fill, so a gap while a page view is created or destroyed matches the page.
pub fn apply_page_background(app: &AppHandle) {
    let color = settings::page_color(app);
    let chrome = with_state(app, |state| {
        state
            .windows
            .iter()
            .map(|window| (window.label.clone(), window.ui.clone()))
            .collect::<Vec<_>>()
    });
    for (label, ui) in chrome {
        if let Some(window) = app.get_window(&label) {
            let _ = window.set_background_color(Some(color));
        }
        if let Some(webview) = app.get_webview(&ui) {
            let _ = webview.set_background_color(Some(color));
        }
    }
    if app.try_state::<Tabs>().is_none() {
        return;
    }
    let labels = with_state(app, |state| state.page_labels());
    for label in labels {
        if let Some(webview) = app.get_webview(&label) {
            let _ = webview.set_background_color(Some(color));
        }
    }
}

/// Returns false when the request closed an already open popup of the same kind instead.
pub fn open_popup(app: &AppHandle, window_label: &str, request: PopupRequest) -> Result<bool, String> {
    let key = format!("{}?{}", request.kind, request.params);
    let toggled = request.focus
        && with_state(app, |state| {
            let Some(popup) = state.chrome(window_label).map(|chrome| &chrome.popup) else {
                return false;
            };
            let open_same = popup.label.is_some() && popup.key().as_deref() == Some(key.as_str());
            let just_closed = popup.closed_key.as_deref() == Some(key.as_str())
                && popup
                    .closed_at
                    .is_some_and(|at| at.elapsed() < POPUP_TOGGLE_WINDOW);
            open_same || just_closed
        });
    if toggled {
        close_window_popup(app, window_label);
        with_state(app, |state| {
            if let Some(popup) = state.chrome_mut(window_label).map(|chrome| &mut chrome.popup) {
                popup.closed_at = None;
            }
        });
        return Ok(false);
    }
    close_window_popup(app, window_label);

    let url = if request.kind == "extension" {
        extensions::popup_url(app, &request.params)?
    } else if request.params.is_empty() {
        paths::internal_url(app, &format!("popup/{}", request.kind))
    } else {
        paths::internal_url(app, &format!("popup/{}?{}", request.kind, request.params))
    };
    let window = app
        .get_window(window_label)
        .ok_or("The browser window is missing.")?;
    let size = window.inner_size().map_err(|error| error.to_string())?;
    let scale = window.scale_factor().unwrap_or(1.0);
    let max_width = size.width as f64 / scale;
    let max_height = size.height as f64 / scale;
    let width = request.width.min(max_width - 8.0).max(120.0);
    let y = request.y.clamp(0.0, (max_height - 60.0).max(0.0));
    let height = request.height.min(max_height - y - 8.0).max(60.0);
    let x = request.x.clamp(4.0, (max_width - width - 4.0).max(4.0));

    let label = with_state(app, |state| {
        state.next_popup += 1;
        let label = format!("popup-{}", state.next_popup);
        let popup = &mut state.chrome_mut(window_label)?.popup;
        popup.label = Some(label.clone());
        popup.kind = Some(request.kind.clone());
        popup.params = Some(request.params.clone());
        popup.focus = request.focus;
        popup.anchor_x = x;
        popup.anchor_y = y;
        popup.concealed = false;
        popup.ready = false;
        // These popups' requested height is only a placeholder. Showing one before its content
        // is measured paints a panel at the wrong height that then snaps to the real one.
        popup.sized = !MEASURED_POPUPS.contains(&request.kind.as_str());
        popup.revealed = false;
        Some(label)
    })
    .ok_or("The browser window is missing.")?;
    let surface = popup_surface_color(app);
    let popup_app = app.clone();
    let reveal_app = app.clone();
    let reveal_label = label.clone();
    let popup_window = window_label.to_string();
    let mut builder = base_builder(app, &label, WebviewUrl::External(url))
        .background_color(surface)
        .focused(false)
        .on_page_load(move |_, payload| {
            if matches!(payload.event(), PageLoadEvent::Finished) {
                popup_ready(&reveal_app, &reveal_label);
            }
        })
        .on_new_window(move |url, features| {
            let response = new_window(&popup_app, None, url, features);
            close_window_popup(&popup_app, &popup_window);
            response
        });
    if request.kind != "extension" {
        builder = builder.initialization_script(popup_paint_script(surface));
    }
    // Park it off the window until the page has painted. A visible webview starts white.
    let webview = match window.add_child(
        builder,
        LogicalPosition::new(-10000.0, -10000.0),
        LogicalSize::new(width, height),
    ) {
        Ok(webview) => webview,
        Err(error) => {
            with_state(app, |state| {
                if let Some(popup) = state.popup_for_label_mut(&label) {
                    popup.label = None;
                    popup.kind = None;
                    popup.params = None;
                }
            });
            return Err(error.to_string());
        }
    };
    webview2::conceal_popup(app, &webview, &label);
    webview2::attach_popup_hooks(app, &webview, &label);
    let _ = app.emit(
        "popup-changed",
        PopupChange {
            kind: Some(request.kind),
            params: Some(request.params),
        },
    );
    Ok(true)
}

pub fn close_window_popup(app: &AppHandle, window_label: &str) {
    let label = with_state(app, |state| {
        let popup = &mut state.chrome_mut(window_label)?.popup;
        let label = popup.label.take()?;
        popup.closed_key = if popup.focus { popup.key() } else { None };
        popup.closed_at = Some(Instant::now());
        popup.kind = None;
        popup.params = None;
        popup.concealed = false;
        popup.ready = false;
        popup.sized = false;
        popup.revealed = false;
        Some(label)
    });
    if let Some(label) = label {
        if let Some(webview) = app.get_webview(&label) {
            let _ = webview.close();
        }
        let _ = app.emit(
            "popup-changed",
            PopupChange {
                kind: None,
                params: None,
            },
        );
    }
}

/// Called when a popup loses focus. A newer popup may already have replaced it.
pub fn close_popup_label(app: &AppHandle, label: &str) {
    let window = with_state(app, |state| {
        state
            .windows
            .iter()
            .find(|window| window.popup.label.as_deref() == Some(label))
            .map(|window| window.label.clone())
    });
    if let Some(window) = window {
        close_window_popup(app, &window);
    }
}

pub fn resize_popup(app: &AppHandle, window_label: &str, width: Option<f64>, height: f64) -> Result<(), String> {
    let (label, anchor_y, revealed) = with_state(app, |state| {
        let popup = &state.chrome(window_label)?.popup;
        let label = popup.label.clone()?;
        Some((label, popup.anchor_y, popup.revealed))
    })
    .ok_or("No popup is open.")?;
    let webview = app.get_webview(&label).ok_or("No popup is open.")?;
    let window = app
        .get_window(window_label)
        .ok_or("The browser window is missing.")?;
    let size = window.inner_size().map_err(|error| error.to_string())?;
    let scale = window.scale_factor().unwrap_or(1.0);
    let position = webview.position().map_err(|error| error.to_string())?;
    let current = webview.size().map_err(|error| error.to_string())?;
    let top = if revealed {
        position.y as f64 / scale
    } else {
        anchor_y
    };
    let max_height = size.height as f64 / scale - top - 8.0;
    let width = width.unwrap_or(current.width as f64 / scale);
    webview
        .set_size(LogicalSize::new(width, height.min(max_height).max(40.0)))
        .map_err(|error| error.to_string())?;
    with_state(app, |state| {
        if let Some(popup) = state.popup_for_label_mut(&label) {
            popup.sized = true;
        }
    });
    round_open_popup(app, &webview);
    popup_ready(app, &label);
    Ok(())
}

/// The popup webview is now hidden. Show it if the page was already ready.
pub fn popup_concealed(app: &AppHandle, label: &str) {
    let show = with_state(app, |state| {
        let popup = state.popup_for_label_mut(label)?;
        if popup.revealed || popup.label.as_deref() != Some(label) {
            return None;
        }
        popup.concealed = true;
        Some(popup_can_reveal(popup))
    });
    if show == Some(true) {
        show_popup(app, label);
    }
}

/// The page has loaded or reported its size. Show it once the initial hide has landed
/// and, for a folder menu, the content height has replaced the placeholder.
fn popup_ready(app: &AppHandle, label: &str) {
    let show = with_state(app, |state| {
        let popup = state.popup_for_label_mut(label)?;
        if popup.revealed || popup.label.as_deref() != Some(label) {
            return None;
        }
        popup.ready = true;
        Some(popup_can_reveal(popup))
    });
    if show == Some(true) {
        show_popup(app, label);
    }
}

fn popup_can_reveal(popup: &Popup) -> bool {
    popup.concealed && popup.ready && popup.sized
}

fn show_popup(app: &AppHandle, label: &str) {
    let place = with_state(app, |state| {
        let popup = state.popup_for_label_mut(label)?;
        if popup.revealed || popup.label.as_deref() != Some(label) {
            return None;
        }
        popup.revealed = true;
        Some((popup.focus, popup.anchor_x, popup.anchor_y))
    });
    let Some((focus, x, y)) = place else {
        return;
    };
    let Some(webview) = app.get_webview(label) else {
        return;
    };
    let _ = webview.set_position(LogicalPosition::new(x, y));
    let _ = webview.show();
    round_open_popup(app, &webview);
    if focus {
        webview2::attach_popup_blur(app, &webview, label);
        let _ = webview.set_focus();
    }
}

fn round_open_popup(app: &AppHandle, webview: &Webview) {
    let owner = with_state(app, |state| {
        state.windows.iter().find_map(|window| {
            let popup_label = window.popup.label.as_deref()?;
            (webview.label() == popup_label).then(|| {
                (
                    window.label.clone(),
                    window.popup.kind.as_deref() == Some("extension"),
                )
            })
        })
    });
    let Some((window_label, extension)) = owner else {
        return;
    };
    if extension {
        return;
    }
    let Some(window) = app.get_window(&window_label) else {
        return;
    };
    let Ok(main) = window.hwnd() else {
        return;
    };
    let scale = window.scale_factor().unwrap_or(1.0);
    webview2::round_popup(
        webview,
        main.0 as isize,
        (12.0 * scale).round().max(1.0) as i32,
    );
}

fn popup_surface_color(app: &AppHandle) -> Color {
    let (r, g, b) = settings::popup_surface(app);
    Color(r, g, b, 255)
}

/// Paints the document the same color as the webview before React mounts.
fn popup_paint_script(color: Color) -> String {
    format!(
        r##"(function () {{
  if (location.hash.indexOf("#/popup/") !== 0) return;
  var color = "#{r:02x}{g:02x}{b:02x}";
  var root = document.documentElement;
  root.dataset.popup = "true";
  var paint = function () {{
    root.style.backgroundColor = color;
    if (document.body) document.body.style.backgroundColor = color;
  }};
  paint();
  document.addEventListener("DOMContentLoaded", paint);
}})();"##,
        r = color.0,
        g = color.1,
        b = color.2,
    )
}

pub struct SessionTab {
    pub url: String,
    pub title: String,
    pub pinned: bool,
    pub favicon: Option<String>,
}

pub fn session_tabs(app: &AppHandle) -> (Vec<SessionTab>, Option<usize>) {
    with_state(app, |state| {
        let tabs = state
            .tabs
            .iter()
            .map(|tab| SessionTab {
                url: tab.info.url.clone(),
                title: tab.info.title.clone(),
                pinned: tab.info.pinned,
                favicon: tab.info.favicon.clone(),
            })
            .collect();
        let active = state
            .active_of(WINDOW)
            .and_then(|id| state.index_of(id));
        (tabs, active)
    })
}

/// Restored tabs get their entry back but no page view until they are selected.
pub fn restore(
    app: &AppHandle,
    tabs: Vec<SessionTab>,
    active: Option<usize>,
) -> Result<(), String> {
    let mut ids = Vec::new();
    for tab in tabs {
        let url = resolve(app, &tab.url);
        let id = open(
            app,
            OpenTab {
                background: true,
                pinned: tab.pinned,
                title: Some(tab.title),
                lazy: true,
                ..OpenTab::new(url)
            },
        )?;
        update(app, id, |entry| entry.info.favicon = tab.favicon);
        ids.push(id);
    }
    match active.and_then(|index| ids.get(index)).or(ids.first()) {
        Some(id) => activate(app, *id),
        None => create_tab(app, None, false, None).map(|_| ()),
    }
}
