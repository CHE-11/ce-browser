use std::collections::{HashMap, HashSet};
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
/// A popup that never reports its first frame is shown anyway after this long.
const POPUP_REVEAL_FALLBACK: Duration = Duration::from_millis(1500);
/// Extension pages cannot report a painted frame. Wait a couple of frames after load instead.
const EXTENSION_POPUP_SETTLE: Duration = Duration::from_millis(32);
const ZOOM_LEVELS: &[f64] = &[
    0.25, 0.33, 0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0,
];
pub const DEFAULT_WORKSPACE: &str = "default";
pub const GROUP_COLORS: &[&str] = &[
    "grey", "blue", "red", "yellow", "green", "pink", "purple", "cyan", "orange",
];
/// Logical px between the two split panes. The browser UI shows through it as the divider.
const SPLIT_GAP: f64 = 6.0;
const MAX_WORKSPACES: usize = 20;

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
    /// The page is playing sound.
    pub audible: bool,
    /// Muted by the user. Background muting from settings does not set this.
    pub muted: bool,
    /// The page view is suspended until the tab is shown again.
    pub sleeping: bool,
    pub group_id: Option<u32>,
    pub workspace: String,
    /// Seconds between automatic reloads. Zero is off.
    pub refresh_secs: u32,
    /// Compare the page text before each reload and mark the tab when it differs.
    pub watch_changes: bool,
    /// The watched page changed since this tab was last selected.
    pub changed: bool,
    /// An InPrivate tab. It is not saved and it is left out of history.
    pub ephemeral: bool,
    /// Page errors since the last navigation. Not written into the session.
    pub errors: u32,
    /// `#rrggbb` from the site's theme color or favicon. Not written into the session.
    pub accent: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitInfo {
    pub left: u32,
    pub right: u32,
    pub ratio: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowTabs {
    pub label: String,
    pub active_id: Option<u32>,
    pub fullscreen: bool,
    pub workspace: String,
    pub split: Option<SplitInfo>,
    /// Address of the open peek, if any.
    pub peek_url: Option<String>,
    /// `phone`, `tablet`, `laptop`, or empty for the full page.
    pub device: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabGroup {
    pub id: u32,
    pub name: String,
    pub color: String,
    pub collapsed: bool,
    pub window: String,
    /// Empty uses the default profile. Otherwise a container id.
    pub container: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub color: String,
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
    pub groups: Vec<TabGroup>,
    pub workspaces: Vec<Workspace>,
}

/// Drops blank and duplicate workspaces and keeps `default` first.
pub fn normalize_workspaces(list: Vec<Workspace>) -> Vec<Workspace> {
    let mut kept: Vec<Workspace> = Vec::new();
    for mut workspace in list {
        workspace.id = workspace.id.trim().to_string();
        workspace.name = workspace.name.trim().chars().take(40).collect();
        if workspace.id.is_empty() || kept.iter().any(|item| item.id == workspace.id) {
            continue;
        }
        if workspace.name.is_empty() {
            workspace.name = "Workspace".into();
        }
        workspace.color = group_color(&workspace.color);
        kept.push(workspace);
        if kept.len() >= MAX_WORKSPACES {
            break;
        }
    }
    if !kept.iter().any(|item| item.id == DEFAULT_WORKSPACE) {
        kept.insert(
            0,
            Workspace {
                id: DEFAULT_WORKSPACE.into(),
                name: "Default".into(),
                color: "blue".into(),
            },
        );
    }
    kept
}

fn group_color(raw: &str) -> String {
    if GROUP_COLORS.contains(&raw) {
        raw.to_string()
    } else {
        "grey".to_string()
    }
}

pub fn group_color_owned(raw: &str) -> String {
    group_color(raw)
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
    /// Last time the page was on screen. The sleep timer counts from here.
    last_shown: Instant,
    /// The controller was made invisible to suspend the page. It must be made visible before it is shown.
    hidden: bool,
    /// Mute state last sent to the page view, so unchanged tabs skip the COM call.
    applied_mute: Option<bool>,
    /// When the auto-refresh interval last fired.
    last_refresh: Instant,
    /// Text fingerprint from the previous watched reload.
    content_mark: Option<String>,
    /// The accent came from theme-color, so a later favicon does not replace it.
    accent_locked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Slot {
    Full,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy)]
struct Split {
    left: u32,
    right: u32,
    ratio: f64,
}

struct WindowChrome {
    label: String,
    ui: String,
    active: Option<u32>,
    ui_height: f64,
    /// Width of the vertical tab sidebar. Zero with horizontal tabs.
    ui_left: f64,
    /// Width of the vertical tool strip. Zero when that strip is hidden.
    ui_right: f64,
    /// Height of the status bar. Zero when it is hidden.
    ui_bottom: f64,
    fullscreen: bool,
    popup: Popup,
    /// Page views on screen and where each one sits.
    shown: Vec<(String, Slot)>,
    split: Option<Split>,
    workspace: String,
    /// Active tab of each workspace this window has left, so switching back returns to it.
    active_by_workspace: HashMap<String, u32>,
    /// `phone`, `tablet`, `laptop`, or empty.
    device: String,
}

impl WindowChrome {
    fn new(label: &str, ui: &str) -> Self {
        Self {
            label: label.to_string(),
            ui: ui.to_string(),
            active: None,
            ui_height: DEFAULT_UI_HEIGHT,
            ui_left: 0.0,
            ui_right: 0.0,
            ui_bottom: 0.0,
            fullscreen: false,
            popup: Popup::default(),
            shown: Vec::new(),
            split: None,
            workspace: DEFAULT_WORKSPACE.to_string(),
            active_by_workspace: HashMap::new(),
            device: String::new(),
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
    groups: Vec<TabGroup>,
    next_group: u32,
    workspaces: Vec<Workspace>,
}

impl TabsState {
    fn workspace_of(&self, window: &str) -> String {
        self.chrome(window)
            .map(|chrome| chrome.workspace.clone())
            .unwrap_or_else(|| DEFAULT_WORKSPACE.to_string())
    }

    /// Global indexes of the tabs one window shows for `workspace`, in strip order.
    fn strip_of(&self, window: &str, workspace: &str) -> Vec<usize> {
        self.tabs
            .iter()
            .enumerate()
            .filter(|(_, tab)| tab.info.window == window && tab.info.workspace == workspace)
            .map(|(index, _)| index)
            .collect()
    }

    /// The tabs in a window's strip: its current workspace.
    fn strip_indexes(&self, window: &str) -> Vec<usize> {
        self.strip_of(window, &self.workspace_of(window))
    }

    fn pinned_count_of(&self, window: &str, workspace: &str) -> usize {
        self.strip_of(window, workspace)
            .into_iter()
            .take_while(|index| self.tabs[*index].info.pinned)
            .count()
    }

    /// `local` is an index in one workspace's strip. Returns where that sits in `self.tabs`.
    fn global_index_of(&self, window: &str, workspace: &str, local: usize) -> usize {
        let indexes = self.strip_of(window, workspace);
        indexes
            .get(local)
            .copied()
            .or_else(|| indexes.last().map(|index| index + 1))
            .or_else(|| {
                self.tabs
                    .iter()
                    .rposition(|tab| tab.info.window == window)
                    .map(|index| index + 1)
            })
            .unwrap_or(self.tabs.len())
    }

    fn local_index(&self, id: u32) -> Option<(String, String, usize)> {
        let index = self.index_of(id)?;
        let tab = &self.tabs[index];
        let window = tab.info.window.clone();
        let workspace = tab.info.workspace.clone();
        let local = self
            .strip_of(&window, &workspace)
            .iter()
            .position(|item| *item == index)?;
        Some((window, workspace, local))
    }

    fn shown_labels(&self) -> HashSet<String> {
        self.windows
            .iter()
            .flat_map(|window| window.shown.iter().map(|(label, _)| label.clone()))
            .collect()
    }

    /// Active tab, or the other half of its split. Background muting and sleep leave these alone.
    fn is_foreground(&self, id: u32) -> bool {
        let Some(window) = self.tab_window(id) else {
            return false;
        };
        let Some(chrome) = self.chrome(&window) else {
            return false;
        };
        if chrome.active == Some(id) {
            return true;
        }
        chrome.split.is_some_and(|split| {
            (split.left == id || split.right == id)
                && chrome
                    .active
                    .is_some_and(|active| active == split.left || active == split.right)
        })
    }

    fn end_split_with(&mut self, id: u32) -> Option<String> {
        for chrome in &mut self.windows {
            if chrome
                .split
                .is_some_and(|split| split.left == id || split.right == id)
            {
                chrome.split = None;
                return Some(chrome.label.clone());
            }
        }
        None
    }

    /// Pinned tabs leave their group, every group's tabs sit together, and empty groups go.
    fn normalize_groups(&mut self) {
        let groups: Vec<(u32, String)> = self
            .groups
            .iter()
            .map(|group| (group.id, group.window.clone()))
            .collect();
        for tab in &mut self.tabs {
            if tab.info.pinned {
                tab.info.group_id = None;
            }
            if let Some(id) = tab.info.group_id {
                if !groups
                    .iter()
                    .any(|(group, window)| *group == id && *window == tab.info.window)
                {
                    tab.info.group_id = None;
                }
            }
        }
        let mut order = Vec::with_capacity(self.tabs.len());
        let mut placed = HashSet::new();
        for index in 0..self.tabs.len() {
            match self.tabs[index].info.group_id {
                None => order.push(index),
                Some(group) => {
                    if placed.insert(group) {
                        let key = (
                            self.tabs[index].info.window.clone(),
                            self.tabs[index].info.workspace.clone(),
                        );
                        for other in index..self.tabs.len() {
                            let tab = &self.tabs[other];
                            if tab.info.group_id == Some(group)
                                && tab.info.window == key.0
                                && tab.info.workspace == key.1
                            {
                                order.push(other);
                            }
                        }
                        for other in index..self.tabs.len() {
                            let tab = &self.tabs[other];
                            if tab.info.group_id == Some(group)
                                && (tab.info.window != key.0 || tab.info.workspace != key.1)
                                && !order.contains(&other)
                            {
                                order.push(other);
                            }
                        }
                    }
                }
            }
        }
        if order.len() == self.tabs.len()
            && order.iter().enumerate().any(|(at, index)| at != *index)
        {
            let mut slots: Vec<Option<Tab>> = self.tabs.drain(..).map(Some).collect();
            self.tabs = order
                .into_iter()
                .filter_map(|index| slots[index].take())
                .collect();
        }
        let tabs = &self.tabs;
        self.groups
            .retain(|group| tabs.iter().any(|tab| tab.info.group_id == Some(group.id)));
    }

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
                    workspace: chrome.workspace.clone(),
                    split: chrome.split.map(|split| SplitInfo {
                        left: split.left,
                        right: split.right,
                        ratio: split.ratio,
                    }),
                    peek_url: None,
                    device: chrome.device.clone(),
                })
                .collect(),
            groups: self.groups.clone(),
            workspaces: self.workspaces.clone(),
        }
    }

    /// Selected tabs in strip order. `window` limits the list to one window's strip and includes its active tab.
    fn selection_in_order(&self, window: Option<&str>) -> Vec<u32> {
        let selected = &self.selected;
        let workspace = window.map(|label| self.workspace_of(label));
        self.tabs
            .iter()
            .filter(|tab| window.is_none_or(|label| tab.info.window == label))
            .filter(|tab| {
                workspace
                    .as_ref()
                    .is_none_or(|workspace| tab.info.workspace == *workspace)
            })
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
        groups: Vec::new(),
        next_group: 1,
        workspaces: normalize_workspaces(Vec::new()),
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
    let mut snapshot = lock(app).snapshot();
    for window in &mut snapshot.windows {
        window.peek_url = crate::peek::url_of(app, &window.label);
    }
    snapshot
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
    let labels: Vec<String> = lock(app)
        .windows
        .iter()
        .map(|window| window.ui.clone())
        .collect();
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
/// Page view for one tab. A container uses its own user-data folder. An ephemeral tab is InPrivate.
fn page_builder(app: &AppHandle, id: u32, label: &str, url: WebviewUrl) -> WebviewBuilder<Wry> {
    let (container, ephemeral) = with_state(app, |state| {
        let Some(tab) = state.tabs.iter().find(|tab| tab.info.id == id) else {
            return (None, false);
        };
        let container = tab.info.group_id.and_then(|group| {
            state
                .groups
                .iter()
                .find(|item| item.id == group)
                .map(|item| item.container.clone())
                .filter(|container| !container.is_empty())
        });
        (container, tab.info.ephemeral)
    });
    if let Some(container) = container {
        let dir = crate::privacy::container_dir(app, &container);
        let mut builder = WebviewBuilder::new(label, url)
            .data_directory(dir)
            .browser_extensions_enabled(false)
            .zoom_hotkeys_enabled(false);
        if let Some(environment) = container_environment(app, &container) {
            builder = builder.with_environment(environment);
        }
        return builder;
    }
    let mut builder = base_builder(app, label, url);
    if ephemeral {
        builder = builder.incognito(true);
    }
    builder
}

fn container_environment(app: &AppHandle, container: &str) -> Option<webview2::WebViewEnvironment> {
    let labels = with_state(app, |state| {
        state
            .tabs
            .iter()
            .filter(|tab| {
                tab.info.group_id.is_some_and(|group| {
                    state
                        .groups
                        .iter()
                        .any(|item| item.id == group && item.container == container)
                })
            })
            .filter_map(|tab| tab.webview.clone())
            .collect::<Vec<_>>()
    });
    labels.iter().find_map(|label| {
        app.get_webview(label)
            .as_ref()
            .and_then(webview2::environment_of)
    })
}

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
    /// Workspace of the new tab. Empty uses the window's current workspace.
    pub workspace: String,
    pub group: Option<u32>,
    pub muted: bool,
    pub ephemeral: bool,
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
            workspace: String::new(),
            group: None,
            muted: false,
            ephemeral: false,
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
        let workspace = if tab.workspace.is_empty()
            || !state.workspaces.iter().any(|item| item.id == tab.workspace)
        {
            state.workspace_of(&window)
        } else {
            tab.workspace.clone()
        };
        let owned = state.strip_of(&window, &workspace).len();
        let pinned_count = state.pinned_count_of(&window, &workspace);
        let local = if tab.pinned {
            tab.index.unwrap_or(pinned_count).min(pinned_count)
        } else {
            tab.index.unwrap_or(owned).clamp(pinned_count, owned)
        };
        let index = state.global_index_of(&window, &workspace, local);
        let group = if tab.pinned || tab.group.is_some() {
            tab.group
        } else {
            // A tab opened between two tabs of a group joins it.
            let before = index.checked_sub(1).and_then(|at| state.tabs.get(at));
            let after = state.tabs.get(index);
            match (before, after) {
                (Some(before), Some(after))
                    if before.info.group_id.is_some()
                        && before.info.group_id == after.info.group_id
                        && before.info.window == window =>
                {
                    before.info.group_id
                }
                _ => None,
            }
        };
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
                    audible: false,
                    muted: tab.muted,
                    sleeping: false,
                    group_id: if tab.pinned { None } else { group },
                    workspace: workspace.clone(),
                    refresh_secs: 0,
                    watch_changes: false,
                    changed: false,
                    ephemeral: tab.ephemeral,
                    errors: 0,
                    accent: None,
                },
                pending_url: Some(tab.url.clone()),
                webview: None,
                painted: false,
                title_ready: false,
                last_shown: Instant::now(),
                hidden: false,
                applied_mute: None,
                last_refresh: Instant::now(),
                content_mark: None,
                accent_locked: false,
            },
        );
        state.normalize_groups();
        let current = workspace == state.workspace_of(&window);
        let none_active = state.active_of(&window).is_none();
        if current && (!tab.background || (none_active && !tab.lazy)) {
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
    if let Some(window) = with_state(app, |state| state.tab_window(id)) {
        apply_audio(app, &window);
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
    let builder = page_builder(app, id, label, WebviewUrl::External(url))
        .background_color(color)
        .focused(false)
        .initialization_script(page_background_script(app))
        .initialization_script(PEEK_CLICK_SCRIPT)
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

/// Shift+click on a web link asks the browser for a peek. `page_hooks` reads the message.
const PEEK_CLICK_SCRIPT: &str = r#"(function () {
  if (!window.chrome || !window.chrome.webview) return;
  if (!/^https?:$/.test(location.protocol)) return;
  document.addEventListener("mousedown", function (event) {
    if (event.button !== 2) return;
    window.__ceGestureStart = { x: event.clientX, y: event.clientY, moved: false };
  }, true);
  document.addEventListener("mousemove", function (event) {
    var start = window.__ceGestureStart;
    if (!start || (event.buttons & 2) === 0) return;
    if (Math.hypot(event.clientX - start.x, event.clientY - start.y) >= 80) start.moved = true;
  }, true);
  document.addEventListener("contextmenu", function (event) {
    if (window.__ceGestureDrag || (window.__ceGestureStart && window.__ceGestureStart.moved)) {
      window.__ceGestureDrag = false;
      event.preventDefault();
      event.stopPropagation();
    }
  }, true);
  document.addEventListener("mouseup", function (event) {
    var start = window.__ceGestureStart;
    window.__ceGestureStart = null;
    if (!start || event.button !== 2 || !start.moved) return;
    window.__ceGestureDrag = true;
    var dx = event.clientX - start.x;
    var dy = event.clientY - start.y;
    if (Math.hypot(dx, dy) < 80) return;
    var direction = Math.abs(dx) > Math.abs(dy) ? (dx < 0 ? "left" : "right") : (dy < 0 ? "up" : "down");
    if (window.chrome && window.chrome.webview) {
      window.chrome.webview.postMessage({ type: "ce-gesture", direction: direction });
    }
  }, true);
  document.addEventListener("click", function (event) {
    if (!event.shiftKey || event.ctrlKey || event.metaKey || event.altKey) return;
    if (event.button !== 0 || event.defaultPrevented) return;
    var target = event.target;
    var link = target && target.closest ? target.closest("a[href]") : null;
    if (!link || !/^https?:/i.test(link.href)) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    window.chrome.webview.postMessage({ type: "ce-peek", url: link.href });
  }, true);
})();"#;

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
        let (window, index, workspace, group) = match opener {
            Some(id) => {
                let state = lock(&app);
                match state.local_index(id) {
                    Some((window, workspace, local)) => {
                        let group = state
                            .index_of(id)
                            .and_then(|index| state.tabs[index].info.group_id);
                        (window, Some(local + 1), workspace, group)
                    }
                    None => (
                        host.unwrap_or_else(|| WINDOW.to_string()),
                        None,
                        String::new(),
                        None,
                    ),
                }
            }
            None => (
                host.unwrap_or_else(|| focused_window(&app)),
                None,
                String::new(),
                None,
            ),
        };
        let _ = open(
            &app,
            OpenTab {
                index,
                window,
                workspace,
                group,
                ..OpenTab::new(url)
            },
        );
    });
    NewWindowResponse::Deny
}

/// Opens `url` right after tab `opener`, in the same window, workspace and group.
pub fn open_beside(
    app: &AppHandle,
    opener: u32,
    url: Url,
    background: bool,
) -> Result<u32, String> {
    let place = with_state(app, |state| {
        let (window, workspace, local) = state.local_index(opener)?;
        let group = state
            .index_of(opener)
            .and_then(|index| state.tabs[index].info.group_id);
        Some((window, workspace, local, group))
    });
    let Some((window, workspace, local, group)) = place else {
        return open(
            app,
            OpenTab {
                background,
                ..OpenTab::new(url)
            },
        );
    };
    open(
        app,
        OpenTab {
            background,
            index: Some(local + 1),
            window,
            workspace,
            group,
            ..OpenTab::new(url)
        },
    )
}

/// Opens another browser window in this process. Page views share the profile folder.
/// A window of its own for an installed site, then one tab at that address.
pub fn open_site_window(app: &AppHandle, title: &str, url: &str) -> Result<String, String> {
    let label = allocate_window(app);
    if let Err(error) = create_browser_window(app, &label) {
        with_state(app, |state| {
            state.windows.retain(|window| window.label != label);
        });
        return Err(error);
    }
    if let Some(window) = app.get_window(&label) {
        let _ = window.set_title(title);
    }
    let _ = create_tab_on(app, &label, Some(url.to_string()), false, None);
    Ok(label)
}

pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_window(WINDOW) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
    send_command(app, WINDOW, "focus-address");
}

pub fn recent_tabs(app: &AppHandle) -> Vec<(u32, String)> {
    snapshot(app)
        .tabs
        .into_iter()
        .filter(|tab| tab.url.starts_with("http://") || tab.url.starts_with("https://"))
        .take(8)
        .map(|tab| {
            let title = if tab.title.is_empty() {
                tab.url
            } else {
                tab.title
            };
            (tab.id, title)
        })
        .collect()
}

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
        // `main-*` is listed in capabilities/default.json. A different prefix cannot drag or invoke.
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
    crate::peek::close(app, &source);
    let next = with_state(app, |state| {
        let next = state
            .strip_indexes(&source)
            .into_iter()
            .find(|index| state.tabs[*index].info.id != id)
            .map(|index| state.tabs[index].info.id);
        state.end_split_with(id);
        let mut workspace = DEFAULT_WORKSPACE.to_string();
        if let Some(tab) = state.tab_mut(id) {
            tab.info.window = label.clone();
            tab.info.group_id = None;
            workspace = tab.info.workspace.clone();
        }
        if let Some(chrome) = state.chrome_mut(&label) {
            chrome.workspace = workspace;
        }
        state.normalize_groups();
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
    } else {
        create_tab_on(app, &source, None, false, None)?;
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
    crate::peek::dismiss(app, label);
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

/// Keeps a device frame inside the page area and centers it.
pub fn fit_device(page_w: u32, page_h: u32, want_w: u32, want_h: u32) -> (u32, u32, u32, u32) {
    let page_w = page_w.max(1);
    let page_h = page_h.max(1);
    let width = want_w.min(page_w).max(1);
    let height = want_h.min(page_h).max(1);
    let x = page_w.saturating_sub(width) / 2;
    let y = page_h.saturating_sub(height) / 2;
    (x, y, width, height)
}

fn device_pixels(
    device: &str,
    page: PhysicalSize<u32>,
    scale: f64,
) -> Option<(u32, u32, u32, u32)> {
    let (css_w, css_h) = match device {
        "phone" => (390u32, 844u32),
        "tablet" => (768, 1024),
        "laptop" => (1280, 800),
        _ => return None,
    };
    let want_w = (f64::from(css_w) * scale).round().max(1.0) as u32;
    let want_h = (f64::from(css_h) * scale).round().max(1.0) as u32;
    Some(fit_device(page.width, page.height, want_w, want_h))
}

pub fn set_device(app: &AppHandle, window: &str, device: &str) -> Result<(), String> {
    let device = match device {
        "" | "off" => String::new(),
        "phone" | "tablet" | "laptop" => device.to_string(),
        _ => return Err("Unknown device.".into()),
    };
    let found = with_state(app, |state| {
        state
            .chrome_mut(window)
            .map(|chrome| chrome.device = device)
    });
    if found.is_none() {
        return Err("That window is not open.".into());
    }
    place_pages(app, window, true);
    emit_tabs(app);
    Ok(())
}

pub fn note_error(app: &AppHandle, id: u32) {
    update(app, id, |tab| {
        tab.info.errors = tab.info.errors.saturating_add(1);
    });
}

pub fn set_theme_color(app: &AppHandle, id: u32, raw: &str) {
    let color = crate::accent::parse_color(raw);
    update(app, id, move |tab| {
        if let Some(color) = color {
            tab.info.accent = Some(color);
            tab.accent_locked = true;
        } else {
            tab.accent_locked = false;
        }
    });
}

pub fn set_favicon_accent(app: &AppHandle, id: u32, color: String) {
    update(app, id, |tab| {
        if !tab.accent_locked {
            tab.info.accent = Some(color);
        }
    });
}

pub fn clear_accent(app: &AppHandle, id: u32) {
    let changed = with_state(app, |state| {
        let tab = state.tab_mut(id)?;
        let changed = tab.info.accent.is_some() || tab.accent_locked;
        tab.info.accent = None;
        tab.accent_locked = false;
        Some(changed)
    });
    if changed == Some(true) {
        emit_tabs(app);
    }
}

pub fn clear_errors(app: &AppHandle, id: u32) {
    let changed = with_state(app, |state| {
        let tab = state.tab_mut(id)?;
        if tab.info.errors == 0 {
            return Some(false);
        }
        tab.info.errors = 0;
        Some(true)
    });
    if changed == Some(true) {
        emit_tabs(app);
    }
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
        if tab.info.ephemeral {
            return None;
        }
        Some((tab.info.url.clone(), title))
    }) else {
        return;
    };
    history::visit(app, &url, &title);
    crate::history_index::queue(app, id, &url);
}

pub fn update_favicon(app: &AppHandle, id: u32, favicon: Option<String>) {
    update(app, id, |tab| tab.info.favicon = favicon);
}

pub fn activate(app: &AppHandle, id: u32) -> Result<(), String> {
    set_active(app, id, true)
}

/// `collapse` replaces the selection with this tab. Shift- and ctrl-clicks keep the group.
fn set_active(app: &AppHandle, id: u32, collapse: bool) -> Result<(), String> {
    if let Some(window) = with_state(app, |state| state.tab_window(id)) {
        crate::peek::close(app, &window);
    }
    let window = with_state(app, |state| {
        let window = state.tab_window(id)?;
        let workspace = state.tabs[state.index_of(id)?].info.workspace.clone();
        if workspace != state.workspace_of(&window) {
            let previous = state.active_of(&window);
            if let Some(chrome) = state.chrome_mut(&window) {
                if let Some(previous) = previous {
                    chrome
                        .active_by_workspace
                        .insert(chrome.workspace.clone(), previous);
                }
                chrome.workspace = workspace;
            }
            state.selected.clear();
        }
        state.set_window_active(&window, Some(id));
        if let Some(tab) = state.tab_mut(id) {
            tab.info.changed = false;
        }
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
    let partner = with_state(app, |state| {
        let split = state.chrome(&window)?.split?;
        if split.left == id {
            Some(split.right)
        } else if split.right == id {
            Some(split.left)
        } else {
            None
        }
    });
    if let Some(partner) = partner {
        ensure_webview(app, partner)?;
    }
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
                .strip_indexes(&window)
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
        let indexes = state.strip_indexes(&window);
        let Some(target) = indexes
            .iter()
            .position(|index| state.tabs[*index].info.id == id)
        else {
            return false;
        };
        let anchor = state
            .anchor
            .filter(|anchor| state.tab_window(*anchor).as_deref() == Some(window.as_str()))
            .and_then(|anchor| {
                indexes
                    .iter()
                    .position(|index| state.tabs[*index].info.id == anchor)
            })
            .or_else(|| {
                state.active_of(&window).and_then(|active| {
                    indexes
                        .iter()
                        .position(|index| state.tabs[*index].info.id == active)
                })
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

/// Puts the active page on screen once it has painted, beside its split partner when it has one.
/// Until then the previous page stays, so opening or closing a tab does not flash the window through.
fn place_pages(app: &AppHandle, window: &str, resize: bool) {
    let (position, page_size) = page_rect(app, window);
    let scale = app
        .get_window(window)
        .and_then(|window| window.scale_factor().ok())
        .unwrap_or(1.0);
    let plan = with_state(app, |state| {
        let active = state.active_of(window);
        let chrome = state.chrome(window)?;
        let fullscreen = chrome.fullscreen;
        let device = chrome.device.clone();
        let previous = chrome.shown.clone();
        let split = chrome.split.filter(|split| {
            device.is_empty()
                && !fullscreen
                && active.is_some_and(|id| id == split.left || id == split.right)
        });
        let ratio = split.map(|split| split.ratio).unwrap_or(0.5);
        let painted_view = |id: u32| {
            state
                .tabs
                .iter()
                .find(|tab| tab.info.id == id && tab.painted)
                .and_then(|tab| tab.webview.clone())
        };
        let paired = split.and_then(|split| {
            Some(vec![
                (painted_view(split.left)?, Slot::Left),
                (painted_view(split.right)?, Slot::Right),
            ])
        });
        let shown = paired.unwrap_or_else(|| {
            let ready = active.and_then(painted_view);
            let fallback = || {
                previous
                    .first()
                    .map(|(label, _)| label.clone())
                    .filter(|label| {
                        state.tabs.iter().any(|tab| {
                            tab.info.window == window
                                && tab.painted
                                && tab.webview.as_ref() == Some(label)
                        })
                    })
            };
            ready
                .or_else(fallback)
                .map(|label| vec![(label, Slot::Full)])
                .unwrap_or_default()
        });
        let changed = previous != shown;
        let parked: Vec<String> = state
            .indexes_in(window)
            .into_iter()
            .filter_map(|index| state.tabs[index].webview.clone())
            .filter(|label| !shown.iter().any(|(item, _)| item == label))
            .collect();
        let now = Instant::now();
        let mut wake = Vec::new();
        for tab in &mut state.tabs {
            let Some(label) = tab.webview.as_ref() else {
                continue;
            };
            if shown.iter().any(|(item, _)| item == label) {
                tab.last_shown = now;
                if tab.hidden || tab.info.sleeping {
                    tab.hidden = false;
                    tab.info.sleeping = false;
                    wake.push(label.clone());
                }
            }
        }
        if let Some(chrome) = state.chrome_mut(window) {
            chrome.shown = shown.clone();
        }
        Some((shown, parked, changed, ratio, wake, device))
    });
    let Some((shown, parked, changed, ratio, wake, device)) = plan else {
        return;
    };
    for label in &wake {
        if let Some(webview) = app.get_webview(label) {
            webview2::wake(&webview);
        }
    }
    if !resize && !changed {
        if !wake.is_empty() {
            emit_tabs(app);
        }
        return;
    }
    let gap = (SPLIT_GAP * scale).round() as i32;
    let left_width = (((page_size.width as f64) * ratio).round() as i32 - gap / 2).max(1);
    let right_x = left_width + gap;
    let right_width = (page_size.width as i32 - right_x).max(1);
    let fitted = device_pixels(&device, page_size, scale);
    for (label, slot) in &shown {
        let Some(webview) = app.get_webview(label) else {
            continue;
        };
        let (x, y, width, height) =
            if let Some((dx, dy, width, height)) = fitted.filter(|_| matches!(slot, Slot::Full)) {
                (dx as i32, dy as i32, width, height)
            } else {
                let (x, width) = match slot {
                    Slot::Full => (0, page_size.width as i32),
                    Slot::Left => (0, left_width),
                    Slot::Right => (right_x, right_width),
                };
                (x, 0, width as u32, page_size.height)
            };
        let _ = webview.set_size(PhysicalSize::new(width, height));
        let _ = webview.set_position(PhysicalPosition::new(position.x + x, position.y + y));
    }
    for label in parked {
        if let Some(webview) = app.get_webview(&label) {
            let _ = webview.set_size(page_size);
            let _ = webview.set_position(parked_position());
        }
    }
    apply_audio(app, window);
    if !wake.is_empty() {
        emit_tabs(app);
    }
}

fn retire_webview(app: &AppHandle, label: Option<String>) {
    if let Some(id) = label
        .as_deref()
        .and_then(|label| label.strip_prefix("tab-"))
        .and_then(|id| id.parse().ok())
    {
        crate::adblock::forget(id);
    }
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
        let on_screen = tab.webview.as_ref().is_some_and(|label| {
            state
                .chrome(window)
                .is_some_and(|chrome| chrome.shown.iter().any(|(shown, _)| shown == label))
        });
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
        let (window, workspace, local) = state.local_index(id)?;
        let index = state.index_of(id)?;
        // Closing one half of a split shows the other half on its own.
        let partner = state.chrome(&window).and_then(|chrome| {
            let split = chrome.split?;
            if split.left == id {
                Some(split.right)
            } else if split.right == id {
                Some(split.left)
            } else {
                None
            }
        });
        state.end_split_with(id);
        let tab = state.tabs.remove(index);
        state.normalize_groups();
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
            let indexes = state.strip_of(&window, &workspace);
            let next = partner
                .filter(|partner| state.index_of(*partner).is_some())
                .or_else(|| {
                    indexes
                        .get(local)
                        .or_else(|| {
                            local
                                .checked_sub(1)
                                .and_then(|previous| indexes.get(previous))
                        })
                        .map(|tab_index| state.tabs[*tab_index].info.id)
                });
            state.set_window_active(&window, next);
        }
        let next = state.active_of(&window);
        let empty = state.indexes_in(&window).is_empty();
        let strip_empty = state.strip_indexes(&window).is_empty();
        let last_window = state.windows.len() <= 1;
        state.forget_tab(id, &window);
        Some((
            tab.webview,
            was_active,
            next,
            empty,
            strip_empty,
            last_window,
            window,
        ))
    });
    let Some((label, was_active, next, empty, strip_empty, last_window, window)) = result else {
        return Ok(());
    };
    if empty || strip_empty {
        retire_webview(app, label);
        if last_window || !empty {
            create_tab_on(app, &window, None, false, None)?;
        } else {
            emit_tabs(app);
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
            .strip_indexes(&window)
            .into_iter()
            .map(|index| &state.tabs[index])
            .filter(|tab| tab.info.id != id && !tab.info.pinned)
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
        let strip = state.strip_indexes(&window);
        let Some(local) = strip
            .iter()
            .position(|index| state.tabs[*index].info.id == id)
        else {
            return Vec::new();
        };
        strip
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
    let window = if browser_labels(app)
        .iter()
        .any(|label| label == &closed.window)
    {
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

/// A page the status bar should describe. Shown pages are on screen, including both sides of a split.
#[derive(Clone)]
pub struct StatusPageTarget {
    pub id: u32,
    pub url: String,
    pub shown: bool,
}

/// On-screen pages, plus each window's active tab when it is not on screen yet.
pub fn status_targets(app: &AppHandle) -> Vec<StatusPageTarget> {
    with_state(app, |state| {
        let shown: Vec<String> = state
            .windows
            .iter()
            .flat_map(|chrome| chrome.shown.iter().map(|(label, _)| label.clone()))
            .collect();
        let actives: Vec<u32> = state
            .windows
            .iter()
            .filter_map(|chrome| chrome.active)
            .collect();
        let mut pages = Vec::new();
        let mut seen = HashSet::new();
        for label in shown {
            let Some(tab) = state
                .tabs
                .iter()
                .find(|tab| tab.webview.as_deref() == Some(label.as_str()))
            else {
                continue;
            };
            if seen.insert(tab.info.id) {
                pages.push(StatusPageTarget {
                    id: tab.info.id,
                    url: tab.info.url.clone(),
                    shown: true,
                });
            }
        }
        for id in actives {
            if !seen.insert(id) {
                continue;
            }
            let Some(tab) = state.tabs.iter().find(|tab| tab.info.id == id) else {
                continue;
            };
            pages.push(StatusPageTarget {
                id,
                url: tab.info.url.clone(),
                shown: false,
            });
        }
        pages
    })
}

pub fn open_page_urls(app: &AppHandle) -> Vec<(u32, String)> {
    with_state(app, |state| {
        state
            .tabs
            .iter()
            .map(|tab| (tab.info.id, tab.info.url.clone()))
            .collect()
    })
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
        let workspace = state.tabs[from].info.workspace.clone();
        let mut tab = state.tabs.remove(from);
        let owned = state.strip_of(&window, &workspace).len();
        let pinned_count = state.pinned_count_of(&window, &workspace);
        let local = if tab.info.pinned {
            to.min(pinned_count)
        } else {
            to.clamp(pinned_count, owned)
        };
        let index = state.global_index_of(&window, &workspace, local);
        if !tab.info.pinned {
            let strip = state.strip_of(&window, &workspace);
            let before = local
                .checked_sub(1)
                .and_then(|at| strip.get(at))
                .and_then(|at| state.tabs[*at].info.group_id);
            let after = strip
                .get(local)
                .and_then(|at| state.tabs[*at].info.group_id);
            tab.info.group_id = match (before, after) {
                (Some(before), Some(after)) if before == after => Some(before),
                _ if tab.info.group_id.is_some()
                    && (tab.info.group_id == before || tab.info.group_id == after) =>
                {
                    tab.info.group_id
                }
                _ => None,
            };
        }
        state.tabs.insert(index, tab);
        state.normalize_groups();
        true
    });
    if moved {
        emit_tabs(app);
    }
}

pub fn set_pinned(app: &AppHandle, id: u32, pinned: bool) {
    let changed = with_state(app, |state| {
        let from = state.index_of(id)?;
        if pinned {
            state.end_split_with(id);
        }
        let window = state.tabs[from].info.window.clone();
        let workspace = state.tabs[from].info.workspace.clone();
        let mut tab = state.tabs.remove(from);
        tab.info.pinned = pinned;
        if pinned {
            tab.info.group_id = None;
        }
        let local = state.pinned_count_of(&window, &workspace);
        let to = state.global_index_of(&window, &workspace, local);
        state.tabs.insert(to, tab);
        state.normalize_groups();
        Some(window)
    });
    if let Some(window) = changed {
        place_pages(app, &window, true);
        emit_tabs(app);
    }
}

pub fn duplicate_tab(app: &AppHandle, id: u32) -> Result<u32, String> {
    let (url, index, window, workspace, group) = with_state(app, |state| {
        let (window, workspace, local) = state.local_index(id)?;
        let tab = &state.tabs[state.index_of(id)?];
        Some((
            tab.info.url.clone(),
            local,
            window,
            workspace,
            tab.info.group_id,
        ))
    })
    .ok_or("That tab is no longer open.")?;
    let url = resolve(app, &url);
    open(
        app,
        OpenTab {
            index: Some(index + 1),
            window,
            workspace,
            group,
            ..OpenTab::new(url)
        },
    )
}

pub fn cycle(app: &AppHandle, step: isize) -> Result<(), String> {
    let window = focused_window(app);
    let next = with_state(app, |state| {
        let indexes = state.strip_indexes(&window);
        let count = indexes.len() as isize;
        if count == 0 {
            return None;
        }
        let current = state
            .active_of(&window)
            .and_then(|id| {
                indexes
                    .iter()
                    .position(|index| state.tabs[*index].info.id == id)
            })
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
        let indexes = state.strip_indexes(&window);
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

/// `top` is the header height, `left` the vertical tab sidebar, `right` the tool strip,
/// and `bottom` the status bar, in CSS pixels. A zero bottom means the bar is hidden.
pub fn set_ui_insets(app: &AppHandle, window: &str, top: f64, left: f64, right: f64, bottom: f64) {
    let changed = with_state(app, |state| {
        // A zero report is the header before it has laid out. Treating it as the real
        // height parks every page at the top of the window and covers the window buttons.
        // A zero bottom is real: the status bar is hidden, or the window is fullscreen.
        if top < 1.0 {
            return None;
        }
        let left = left.max(0.0);
        let right = right.max(0.0);
        let bottom = bottom.max(0.0);
        let chrome = state.chrome_mut(window)?;
        if (chrome.ui_height - top).abs() < 0.5
            && (chrome.ui_left - left).abs() < 0.5
            && (chrome.ui_right - right).abs() < 0.5
            && (chrome.ui_bottom - bottom).abs() < 0.5
        {
            return None;
        }
        chrome.ui_height = top;
        chrome.ui_left = left;
        chrome.ui_right = right;
        chrome.ui_bottom = bottom;
        Some(())
    });
    if changed.is_some() {
        layout(app, window);
    }
}

/// Physical rectangle for page views: below the browser UI, right of the tab sidebar,
/// left of the tool strip, and above the status bar, or the whole window in fullscreen.
fn page_rect(app: &AppHandle, window_label: &str) -> (PhysicalPosition<i32>, PhysicalSize<u32>) {
    let (ui_height, ui_left, ui_right, ui_bottom, fullscreen) = with_state(app, |state| {
        state
            .chrome(window_label)
            .map(|chrome| {
                (
                    chrome.ui_height,
                    chrome.ui_left,
                    chrome.ui_right,
                    chrome.ui_bottom,
                    chrome.fullscreen,
                )
            })
            .unwrap_or((DEFAULT_UI_HEIGHT, 0.0, 0.0, 0.0, false))
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
    let left = ((ui_left * scale).round() as u32).min(size.width.saturating_sub(1));
    let right =
        ((ui_right * scale).round() as u32).min(size.width.saturating_sub(left).saturating_sub(1));
    let bottom = ((ui_bottom * scale).round() as u32).min(size.height.saturating_sub(top));
    (
        PhysicalPosition::new(left as i32, top as i32),
        PhysicalSize::new(size.width - left - right, size.height - top - bottom),
    )
}

/// The page area of a window in physical pixels, for views drawn over the pages.
pub fn page_area(
    app: &AppHandle,
    window_label: &str,
) -> (PhysicalPosition<i32>, PhysicalSize<u32>) {
    page_rect(app, window_label)
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
    crate::peek::relayout(app, window_label);
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
pub fn open_popup(
    app: &AppHandle,
    window_label: &str,
    request: PopupRequest,
) -> Result<bool, String> {
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
            if let Some(popup) = state
                .chrome_mut(window_label)
                .map(|chrome| &mut chrome.popup)
            {
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
        popup.revealed = false;
        Some(label)
    })
    .ok_or("The browser window is missing.")?;
    let extension = request.kind == "extension";
    let surface = popup_surface_color(app);
    let popup_app = app.clone();
    let reveal_app = app.clone();
    let reveal_label = label.clone();
    let popup_window = window_label.to_string();
    let mut builder = base_builder(app, &label, WebviewUrl::External(url))
        .background_color(surface)
        .focused(false)
        .on_page_load(move |_, payload| {
            // Internal popups report their own first frame through `popup_painted`.
            if extension && matches!(payload.event(), PageLoadEvent::Finished) {
                reveal_popup_after(&reveal_app, &reveal_label, EXTENSION_POPUP_SETTLE);
            }
        })
        .on_new_window(move |url, features| {
            let response = new_window(&popup_app, None, url, features);
            close_window_popup(&popup_app, &popup_window);
            response
        });
    if !extension {
        builder = builder.initialization_script(popup_paint_script(surface));
    }
    // Park it off the window until the page has painted. A new webview starts white, and it
    // must stay visible here so the page keeps rendering and `requestAnimationFrame` fires.
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
    webview2::attach_popup_hooks(app, &webview, &label);
    reveal_popup_after(app, &label, POPUP_REVEAL_FALLBACK);
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

pub fn resize_popup(
    app: &AppHandle,
    window_label: &str,
    width: Option<f64>,
    height: f64,
) -> Result<(), String> {
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
    round_open_popup(app, &webview);
    Ok(())
}

/// The popup page has drawn its first complete frame offscreen: data loaded, theme applied
/// and height reported. Move it into place.
pub fn popup_painted(app: &AppHandle, window_label: &str) {
    let label = with_state(app, |state| state.chrome(window_label)?.popup.label.clone());
    if let Some(label) = label {
        show_popup(app, &label);
    }
}

/// Shows the popup after `delay` unless it was replaced or already shown.
fn reveal_popup_after(app: &AppHandle, label: &str, delay: Duration) {
    let app = app.clone();
    let label = label.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        show_popup(&app, &label);
    });
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
    round_open_popup(app, &webview);
    let _ = webview.set_position(LogicalPosition::new(x, y));
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
    pub muted: bool,
    pub group_id: Option<u32>,
    pub workspace: String,
    pub refresh_secs: u32,
    pub watch_changes: bool,
}

pub struct SessionState {
    pub tabs: Vec<SessionTab>,
    pub active: Option<usize>,
    pub groups: Vec<TabGroup>,
    pub workspaces: Vec<Workspace>,
    pub active_workspace: String,
    /// Indexes into `tabs` and the divider ratio.
    pub split: Option<(usize, usize, f64)>,
}

pub fn session_tabs(app: &AppHandle) -> SessionState {
    with_state(app, |state| {
        let tabs = state
            .tabs
            .iter()
            .filter(|tab| !tab.info.ephemeral)
            .map(|tab| SessionTab {
                url: tab.info.url.clone(),
                title: tab.info.title.clone(),
                pinned: tab.info.pinned,
                favicon: tab.info.favicon.clone(),
                muted: tab.info.muted,
                group_id: tab.info.group_id,
                workspace: tab.info.workspace.clone(),
                refresh_secs: tab.info.refresh_secs,
                watch_changes: tab.info.watch_changes,
            })
            .collect();
        let active = state.active_of(WINDOW).and_then(|id| state.index_of(id));
        let split = state.chrome(WINDOW).and_then(|chrome| {
            let split = chrome.split?;
            Some((
                state.index_of(split.left)?,
                state.index_of(split.right)?,
                split.ratio,
            ))
        });
        SessionState {
            tabs,
            active,
            groups: state.groups.clone(),
            workspaces: state.workspaces.clone(),
            active_workspace: state.workspace_of(WINDOW),
            split,
        }
    })
}

pub struct RestoreState {
    pub tabs: Vec<SessionTab>,
    pub active: Option<usize>,
    pub groups: Vec<TabGroup>,
    pub workspaces: Vec<Workspace>,
    pub active_workspace: String,
    pub split: Option<(usize, usize, f64)>,
}

/// Restored tabs get their entry back but no page view until they are selected.
pub fn restore(app: &AppHandle, restored: RestoreState) -> Result<(), String> {
    with_state(app, |state| {
        state.workspaces = normalize_workspaces(restored.workspaces);
        let active_workspace = if state
            .workspaces
            .iter()
            .any(|workspace| workspace.id == restored.active_workspace)
        {
            restored.active_workspace.clone()
        } else {
            DEFAULT_WORKSPACE.to_string()
        };
        if let Some(chrome) = state.chrome_mut(WINDOW) {
            chrome.workspace = active_workspace;
        }
    });
    let mut ids = Vec::new();
    let mut memberships = Vec::new();
    for tab in restored.tabs {
        let url = resolve(app, &tab.url);
        let id = open(
            app,
            OpenTab {
                background: true,
                pinned: tab.pinned,
                title: Some(tab.title),
                lazy: true,
                window: WINDOW.to_string(),
                workspace: tab.workspace,
                muted: tab.muted,
                ..OpenTab::new(url)
            },
        )?;
        let refresh_secs = tab.refresh_secs;
        let watch_changes = tab.watch_changes;
        update(app, id, |entry| {
            entry.info.favicon = tab.favicon;
            entry.info.refresh_secs = refresh_secs;
            entry.info.watch_changes = watch_changes;
        });
        memberships.push((id, tab.group_id));
        ids.push(id);
    }
    // Groups are attached once every tab exists. Attaching them while tabs open would drop
    // each group that has no tab yet.
    with_state(app, |state| {
        state.groups = restored
            .groups
            .into_iter()
            .map(|mut group| {
                group.color = group_color(&group.color);
                group.window = WINDOW.to_string();
                if !crate::privacy::known_container(app, &group.container) {
                    group.container.clear();
                }
                group
            })
            .collect();
        state.next_group = state.groups.iter().map(|group| group.id).max().unwrap_or(0) + 1;
        for (id, group) in &memberships {
            if let Some(tab) = state.tab_mut(*id) {
                tab.info.group_id = *group;
            }
        }
        state.normalize_groups();
        if let Some((left, right, ratio)) = restored.split {
            if let (Some(left), Some(right)) = (ids.get(left), ids.get(right)) {
                let same_strip = state
                    .index_of(*left)
                    .zip(state.index_of(*right))
                    .is_some_and(|(a, b)| {
                        state.tabs[a].info.workspace == state.tabs[b].info.workspace
                            && !state.tabs[a].info.pinned
                            && !state.tabs[b].info.pinned
                    });
                if same_strip && left != right {
                    if let Some(chrome) = state.chrome_mut(WINDOW) {
                        chrome.split = Some(Split {
                            left: *left,
                            right: *right,
                            ratio: clamp_ratio(ratio),
                        });
                    }
                }
            }
        }
    });
    let target = with_state(app, |state| {
        let workspace = state.workspace_of(WINDOW);
        let in_workspace = |id: &u32| {
            state
                .index_of(*id)
                .is_some_and(|index| state.tabs[index].info.workspace == workspace)
        };
        restored
            .active
            .and_then(|index| ids.get(index))
            .filter(|id| in_workspace(id))
            .or_else(|| ids.iter().find(|id| in_workspace(id)))
            .copied()
    });
    match target {
        Some(id) => activate(app, id),
        None => create_tab(app, None, false, None).map(|_| ()),
    }
}

fn clamp_ratio(ratio: f64) -> f64 {
    if ratio.is_finite() {
        ratio.clamp(0.2, 0.8)
    } else {
        0.5
    }
}

// Audio

/// Mutes or unmutes each page view in `window` to match its tab and the background-mute setting.
pub fn apply_audio(app: &AppHandle, window: &str) {
    let background = settings::get(app).mute_background_tabs;
    let changes = with_state(app, |state| {
        let ids: Vec<u32> = state
            .indexes_in(window)
            .into_iter()
            .map(|index| state.tabs[index].info.id)
            .collect();
        let mut changes = Vec::new();
        for id in ids {
            let foreground = state.is_foreground(id);
            let Some(tab) = state.tab_mut(id) else {
                continue;
            };
            let Some(label) = tab.webview.clone() else {
                continue;
            };
            let muted = tab.info.muted || (background && !foreground);
            if tab.applied_mute != Some(muted) {
                tab.applied_mute = Some(muted);
                changes.push((label, muted));
            }
        }
        changes
    });
    for (label, muted) in changes {
        if let Some(webview) = app.get_webview(&label) {
            webview2::set_muted(&webview, muted);
        }
    }
}

pub fn apply_audio_all(app: &AppHandle) {
    if app.try_state::<Tabs>().is_none() {
        return;
    }
    for window in browser_labels(app) {
        apply_audio(app, &window);
    }
}

pub fn update_audio(app: &AppHandle, id: u32, audible: bool) {
    let changed = with_state(app, |state| {
        let tab = state.tab_mut(id)?;
        if tab.info.audible == audible {
            return None;
        }
        tab.info.audible = audible;
        Some(())
    });
    if changed.is_some() {
        emit_tabs(app);
    }
}

pub fn set_muted(app: &AppHandle, ids: &[u32], muted: bool) -> Result<(), String> {
    let windows = with_state(app, |state| {
        let mut windows = Vec::new();
        for id in ids {
            if let Some(tab) = state.tab_mut(*id) {
                tab.info.muted = muted;
                if !windows.contains(&tab.info.window) {
                    windows.push(tab.info.window.clone());
                }
            }
        }
        windows
    });
    if windows.is_empty() {
        return Err("That tab is no longer open.".into());
    }
    for window in windows {
        apply_audio(app, &window);
    }
    emit_tabs(app);
    Ok(())
}

/// Mutes the selected tabs, or unmutes them when every one is already muted.
pub fn toggle_selected_muted(app: &AppHandle) -> Result<(), String> {
    let window = focused_window(app);
    let (ids, all_muted) = with_state(app, |state| {
        let ids = state.selection_in_order(Some(&window));
        let all_muted = ids.iter().all(|id| {
            state
                .index_of(*id)
                .is_some_and(|index| state.tabs[index].info.muted)
        });
        (ids, all_muted)
    });
    if ids.is_empty() {
        return Ok(());
    }
    set_muted(app, &ids, !all_muted)
}

// Sleep

/// Tabs the sleep timer may suspend now.
pub fn sleep_candidates(app: &AppHandle) -> Vec<u32> {
    let sleep = settings::get(app).tab_sleep;
    if !sleep.enabled {
        return Vec::new();
    }
    let after = if crate::system::saver_active(app) {
        Duration::from_secs(5)
    } else {
        Duration::from_secs(u64::from(sleep.minutes) * 60)
    };
    with_state(app, |state| {
        let shown = state.shown_labels();
        state
            .tabs
            .iter()
            .filter(|tab| {
                tab.webview
                    .as_ref()
                    .is_some_and(|label| !shown.contains(label))
                    && !tab.hidden
                    && !tab.info.sleeping
                    && !tab.info.audible
                    && !tab.info.loading
                    && (sleep.sleep_pinned || !tab.info.pinned)
                    && tab.last_shown.elapsed() >= after
                    && !sleep.never_sleeps(&tab.info.url)
            })
            .map(|tab| tab.info.id)
            .filter(|id| !state.is_foreground(*id))
            .collect()
    })
}

pub fn sleep_tab(app: &AppHandle, id: u32) -> Result<(), String> {
    let label = with_state(app, |state| {
        if state.is_foreground(id) {
            return Err("The tab on screen can't sleep.".to_string());
        }
        let tab = state.tab_mut(id).ok_or("That tab is no longer open.")?;
        if tab.info.sleeping || tab.hidden {
            return Ok(None);
        }
        let Some(label) = tab.webview.clone() else {
            return Ok(None);
        };
        tab.hidden = true;
        Ok(Some(label))
    })?;
    if let Some(webview) = label.and_then(|label| app.get_webview(&label)) {
        webview2::suspend(app, &webview, id);
    }
    Ok(())
}

pub fn sleep_other_tabs(app: &AppHandle, id: u32) -> Result<(), String> {
    let ids = with_state(app, |state| {
        let Some(window) = state.tab_window(id) else {
            return Vec::new();
        };
        state
            .indexes_in(&window)
            .into_iter()
            .map(|index| state.tabs[index].info.id)
            .filter(|other| *other != id && !state.is_foreground(*other))
            .collect::<Vec<_>>()
    });
    for other in ids {
        let _ = sleep_tab(app, other);
    }
    Ok(())
}

/// Called when WebView2 answers a suspend request.
pub fn finish_sleep(app: &AppHandle, id: u32, suspended: bool) {
    let result = with_state(app, |state| {
        let foreground = state.is_foreground(id);
        let tab = state.tab_mut(id)?;
        let label = tab.webview.clone()?;
        if suspended && tab.hidden && !foreground {
            tab.info.sleeping = true;
            return Some((label, false));
        }
        // Shown again while the request ran, or WebView2 refused: make the page visible again.
        tab.hidden = false;
        tab.info.sleeping = false;
        Some((label, true))
    });
    let Some((label, wake)) = result else {
        return;
    };
    if wake {
        if let Some(webview) = app.get_webview(&label) {
            webview2::wake(&webview);
        }
    }
    emit_tabs(app);
}

/// Adds the tab's site to the never-sleep list.
pub fn never_sleep_site(app: &AppHandle, id: u32) -> Result<(), String> {
    let url = with_state(app, |state| {
        state
            .index_of(id)
            .map(|index| state.tabs[index].info.url.clone())
    })
    .ok_or("That tab is no longer open.")?;
    let site = settings::sleep_site_key(&url);
    if site.is_empty() {
        return Err("This page has no site to remember.".into());
    }
    settings::update(app, |settings| {
        if !settings.tab_sleep.never_sleep_sites.contains(&site) {
            settings.tab_sleep.never_sleep_sites.push(site);
        }
    })?;
    let sleeping = with_state(app, |state| {
        state
            .index_of(id)
            .is_some_and(|index| state.tabs[index].info.sleeping || state.tabs[index].hidden)
    });
    if sleeping {
        finish_sleep(app, id, false);
    }
    Ok(())
}

// Split view

/// Shows `id` beside the active tab, active on the left.
pub fn split_with(app: &AppHandle, id: u32) -> Result<(), String> {
    let window = with_state(app, |state| {
        let window = state.tab_window(id).ok_or("That tab is no longer open.")?;
        let active = state
            .active_of(&window)
            .ok_or("Open a tab to split with first.")?;
        if active == id {
            return Err("Choose a different tab to split with.".to_string());
        }
        let (Some(left_index), Some(right_index)) = (state.index_of(active), state.index_of(id))
        else {
            return Err("That tab is no longer open.".to_string());
        };
        let (left_tab, right_tab) = (&state.tabs[left_index].info, &state.tabs[right_index].info);
        if left_tab.pinned || right_tab.pinned {
            return Err("Pinned tabs can't be split. Unpin the tab first.".to_string());
        }
        if left_tab.workspace != right_tab.workspace {
            return Err("Both tabs must be in the same workspace.".to_string());
        }
        let group = left_tab.group_id;
        let mut tab = state.tabs.remove(right_index);
        tab.info.group_id = group;
        let left_index = state.index_of(active).unwrap_or(0);
        state.tabs.insert(left_index + 1, tab);
        state.normalize_groups();
        state.end_split_with(active);
        state.end_split_with(id);
        if let Some(chrome) = state.chrome_mut(&window) {
            chrome.split = Some(Split {
                left: active,
                right: id,
                ratio: 0.5,
            });
        }
        Ok(window)
    })?;
    ensure_webview(app, id)?;
    place_pages(app, &window, true);
    emit_tabs(app);
    focus_active(app, &window);
    Ok(())
}

/// Opens `url` in a new tab on the right half of a split with the active tab.
pub fn open_in_split(app: &AppHandle, window: &str, url: &str) -> Result<u32, String> {
    let index = with_state(app, |state| {
        let active = state.active_of(window)?;
        state.local_index(active).map(|(_, _, local)| local + 1)
    });
    let id = create_tab_on(app, window, Some(url.to_string()), true, index)?;
    split_with(app, id)?;
    Ok(id)
}

pub fn swap_split(app: &AppHandle, window: &str) -> Result<(), String> {
    let changed = with_state(app, |state| {
        let chrome = state.chrome_mut(window)?;
        let split = chrome.split.as_mut()?;
        std::mem::swap(&mut split.left, &mut split.right);
        split.ratio = 1.0 - split.ratio;
        Some(())
    });
    if changed.is_some() {
        place_pages(app, window, true);
        emit_tabs(app);
    }
    Ok(())
}

pub fn exit_split(app: &AppHandle, window: &str) -> Result<(), String> {
    let changed = with_state(app, |state| state.chrome_mut(window)?.split.take());
    if changed.is_some() {
        place_pages(app, window, true);
        emit_tabs(app);
        focus_active(app, window);
    }
    Ok(())
}

pub fn set_split_ratio(app: &AppHandle, window: &str, ratio: f64) {
    let changed = with_state(app, |state| {
        let split = state.chrome_mut(window)?.split.as_mut()?;
        let ratio = clamp_ratio(ratio);
        if (split.ratio - ratio).abs() < 0.001 {
            return None;
        }
        split.ratio = ratio;
        Some(())
    });
    if changed.is_some() {
        place_pages(app, window, true);
        emit_tabs(app);
    }
}

/// A page view took focus. Clicking into the other half of a split makes that tab active,
/// and clicking a tab while a peek is open closes the peek.
pub fn page_focused(app: &AppHandle, id: u32) {
    let Some(window) = with_state(app, |state| state.tab_window(id)) else {
        return;
    };
    crate::peek::dismiss(app, &window);
    let changed = with_state(app, |state| {
        let chrome = state.chrome(&window)?;
        let split = chrome.split?;
        let active = chrome.active?;
        let in_split = |tab: u32| tab == split.left || tab == split.right;
        if active == id || !in_split(id) || !in_split(active) {
            return None;
        }
        state.set_window_active(&window, Some(id));
        state.select_only(id);
        Some(())
    });
    if changed.is_some() {
        emit_tabs(app);
    }
}

/// Re-focuses the window's active page. Used after a view drawn over the pages closes.
pub fn refocus(app: &AppHandle, window: &str) {
    focus_active(app, window);
}

// Groups

pub fn create_group(app: &AppHandle, ids: &[u32]) -> Result<u32, String> {
    let group = with_state(app, |state| {
        let first = ids
            .iter()
            .find_map(|id| state.index_of(*id))
            .ok_or("Those tabs are no longer open.")?;
        let window = state.tabs[first].info.window.clone();
        let workspace = state.tabs[first].info.workspace.clone();
        let members: Vec<u32> = ids
            .iter()
            .copied()
            .filter(|id| {
                state.index_of(*id).is_some_and(|index| {
                    let tab = &state.tabs[index].info;
                    tab.window == window && tab.workspace == workspace && !tab.pinned
                })
            })
            .collect();
        if members.is_empty() {
            return Err("Pinned tabs can't be grouped.".to_string());
        }
        let id = state.next_group;
        state.next_group += 1;
        let used: Vec<String> = state
            .groups
            .iter()
            .map(|group| group.color.clone())
            .collect();
        let color = GROUP_COLORS
            .iter()
            .skip(1)
            .find(|color| !used.iter().any(|used| used == *color))
            .unwrap_or(&GROUP_COLORS[1]);
        state.groups.push(TabGroup {
            id,
            name: String::new(),
            color: (*color).to_string(),
            collapsed: false,
            window,
            container: String::new(),
        });
        for member in &members {
            if let Some(tab) = state.tab_mut(*member) {
                tab.info.group_id = Some(id);
            }
        }
        state.normalize_groups();
        Ok(id)
    })?;
    emit_tabs(app);
    Ok(group)
}

pub fn add_to_group(app: &AppHandle, ids: &[u32], group: u32) -> Result<(), String> {
    with_state(app, |state| {
        let window = state
            .groups
            .iter()
            .find(|item| item.id == group)
            .map(|item| item.window.clone())
            .ok_or("That group no longer exists.")?;
        let workspace = state
            .tabs
            .iter()
            .find(|tab| tab.info.group_id == Some(group))
            .map(|tab| tab.info.workspace.clone());
        for id in ids {
            if let Some(tab) = state.tab_mut(*id) {
                let same_strip = workspace
                    .as_ref()
                    .is_none_or(|workspace| tab.info.workspace == *workspace);
                if tab.info.window == window && same_strip && !tab.info.pinned {
                    tab.info.group_id = Some(group);
                }
            }
        }
        state.normalize_groups();
        Ok::<(), String>(())
    })?;
    emit_tabs(app);
    Ok(())
}

pub fn remove_from_group(app: &AppHandle, ids: &[u32]) -> Result<(), String> {
    with_state(app, |state| {
        for id in ids {
            let Some(index) = state.index_of(*id) else {
                continue;
            };
            let Some(group) = state.tabs[index].info.group_id else {
                continue;
            };
            // The tab leaves at the end of its group so the rest stays together.
            let mut tab = state.tabs.remove(index);
            tab.info.group_id = None;
            let last = state
                .tabs
                .iter()
                .rposition(|other| other.info.group_id == Some(group))
                .map(|at| at + 1)
                .unwrap_or(index.min(state.tabs.len()));
            state.tabs.insert(last, tab);
        }
        state.normalize_groups();
    });
    emit_tabs(app);
    Ok(())
}

pub fn update_group(
    app: &AppHandle,
    id: u32,
    name: Option<String>,
    color: Option<String>,
    collapsed: Option<bool>,
) -> Result<(), String> {
    with_state(app, |state| {
        let group = state
            .groups
            .iter_mut()
            .find(|group| group.id == id)
            .ok_or("That group no longer exists.")?;
        if let Some(name) = name {
            group.name = name.trim().chars().take(60).collect();
        }
        if let Some(color) = color {
            group.color = group_color(&color);
        }
        if let Some(collapsed) = collapsed {
            group.collapsed = collapsed;
        }
        Ok::<(), String>(())
    })?;
    emit_tabs(app);
    Ok(())
}

pub fn ungroup(app: &AppHandle, id: u32) -> Result<(), String> {
    with_state(app, |state| {
        for tab in &mut state.tabs {
            if tab.info.group_id == Some(id) {
                tab.info.group_id = None;
            }
        }
        state.normalize_groups();
    });
    emit_tabs(app);
    Ok(())
}

pub fn close_group(app: &AppHandle, id: u32) -> Result<(), String> {
    let ids: Vec<u32> = with_state(app, |state| {
        state
            .tabs
            .iter()
            .filter(|tab| tab.info.group_id == Some(id))
            .map(|tab| tab.info.id)
            .collect()
    });
    for tab in ids {
        close_tab(app, tab)?;
    }
    Ok(())
}

pub fn new_tab_in_group(app: &AppHandle, id: u32) -> Result<u32, String> {
    let (window, workspace, index) = with_state(app, |state| {
        let last = state
            .tabs
            .iter()
            .rposition(|tab| tab.info.group_id == Some(id))?;
        let tab_id = state.tabs[last].info.id;
        let (window, workspace, local) = state.local_index(tab_id)?;
        if let Some(group) = state.groups.iter_mut().find(|group| group.id == id) {
            group.collapsed = false;
        }
        Some((window, workspace, local + 1))
    })
    .ok_or("That group no longer exists.")?;
    let url = resolve(app, paths::NEW_TAB);
    open(
        app,
        OpenTab {
            index: Some(index),
            window,
            workspace,
            group: Some(id),
            ..OpenTab::new(url)
        },
    )
}

// Workspaces

pub fn create_workspace(app: &AppHandle, name: &str, color: &str) -> Result<String, String> {
    let id = with_state(app, |state| {
        if state.workspaces.len() >= MAX_WORKSPACES {
            return Err(format!("You can have up to {MAX_WORKSPACES} workspaces."));
        }
        let mut number = state.workspaces.len();
        let id = loop {
            let candidate = format!("workspace-{number}");
            if !state
                .workspaces
                .iter()
                .any(|workspace| workspace.id == candidate)
            {
                break candidate;
            }
            number += 1;
        };
        let name = name.trim();
        state.workspaces.push(Workspace {
            id: id.clone(),
            name: if name.is_empty() {
                format!("Workspace {}", state.workspaces.len() + 1)
            } else {
                name.chars().take(40).collect()
            },
            color: group_color(color),
        });
        Ok(id)
    })?;
    emit_tabs(app);
    Ok(id)
}

pub fn rename_workspace(app: &AppHandle, id: &str, name: &str, color: &str) -> Result<(), String> {
    with_state(app, |state| {
        let workspace = state
            .workspaces
            .iter_mut()
            .find(|workspace| workspace.id == id)
            .ok_or("That workspace no longer exists.")?;
        let name = name.trim();
        if !name.is_empty() {
            workspace.name = name.chars().take(40).collect();
        }
        workspace.color = group_color(color);
        Ok::<(), String>(())
    })?;
    emit_tabs(app);
    Ok(())
}

/// Closes the workspace's tabs. Windows showing it move to another workspace first.
pub fn delete_workspace(app: &AppHandle, id: &str) -> Result<(), String> {
    let (fallback, windows) = with_state(app, |state| {
        if state.workspaces.len() <= 1 {
            return Err("The last workspace can't be deleted.".to_string());
        }
        let fallback = state
            .workspaces
            .iter()
            .find(|workspace| workspace.id != id)
            .map(|workspace| workspace.id.clone())
            .ok_or("The last workspace can't be deleted.")?;
        let windows: Vec<String> = state
            .windows
            .iter()
            .filter(|chrome| chrome.workspace == id)
            .map(|chrome| chrome.label.clone())
            .collect();
        Ok((fallback, windows))
    })?;
    for window in windows {
        switch_workspace(app, &window, &fallback)?;
    }
    let ids: Vec<u32> = with_state(app, |state| {
        state
            .tabs
            .iter()
            .filter(|tab| tab.info.workspace == id)
            .map(|tab| tab.info.id)
            .collect()
    });
    for tab in ids {
        close_tab(app, tab)?;
    }
    with_state(app, |state| {
        state.workspaces.retain(|workspace| workspace.id != id);
        for chrome in &mut state.windows {
            chrome.active_by_workspace.remove(id);
        }
    });
    emit_tabs(app);
    Ok(())
}

pub fn switch_workspace(app: &AppHandle, window: &str, id: &str) -> Result<(), String> {
    crate::peek::close(app, window);
    let target = with_state(app, |state| {
        if !state.workspaces.iter().any(|workspace| workspace.id == id) {
            return Err("That workspace no longer exists.".to_string());
        }
        let active = state.active_of(window);
        let chrome = state
            .chrome_mut(window)
            .ok_or("The browser window is missing.")?;
        if chrome.workspace == id {
            return Ok(None);
        }
        if let Some(active) = active {
            chrome
                .active_by_workspace
                .insert(chrome.workspace.clone(), active);
        }
        chrome.workspace = id.to_string();
        let remembered = chrome.active_by_workspace.get(id).copied();
        chrome.active = None;
        state.selected.clear();
        let strip = state.strip_of(window, id);
        let target = remembered
            .filter(|tab| strip.iter().any(|index| state.tabs[*index].info.id == *tab))
            .or_else(|| strip.first().map(|index| state.tabs[*index].info.id));
        Ok(Some(target))
    })?;
    match target {
        None => Ok(()),
        Some(Some(tab)) => activate(app, tab),
        Some(None) => create_tab_on(app, window, None, false, None).map(|_| ()),
    }
}

/// Switches to the workspace at `index` in the list (Ctrl+Alt+1 to 9).
pub fn switch_workspace_index(app: &AppHandle, index: usize) -> Result<(), String> {
    let window = focused_window(app);
    let id = with_state(app, |state| {
        state
            .workspaces
            .get(index)
            .map(|workspace| workspace.id.clone())
    });
    match id {
        Some(id) => switch_workspace(app, &window, &id),
        None => Ok(()),
    }
}

pub fn move_tabs_to_workspace(app: &AppHandle, ids: &[u32], workspace: &str) -> Result<(), String> {
    let windows = with_state(app, |state| {
        if !state.workspaces.iter().any(|item| item.id == workspace) {
            return Err("That workspace no longer exists.".to_string());
        }
        let mut windows: Vec<String> = Vec::new();
        for id in ids {
            let Some((window, current, local)) = state.local_index(*id) else {
                continue;
            };
            if current == workspace {
                continue;
            }
            state.end_split_with(*id);
            if state.active_of(&window) == Some(*id) {
                let strip = state.strip_of(&window, &current);
                let next = strip
                    .iter()
                    .map(|index| state.tabs[*index].info.id)
                    .filter(|other| other != id && !ids.contains(other))
                    .min_by_key(|other| {
                        strip
                            .iter()
                            .position(|index| state.tabs[*index].info.id == *other)
                            .unwrap_or(0)
                            .abs_diff(local)
                    });
                state.set_window_active(&window, next);
            }
            let Some(index) = state.index_of(*id) else {
                continue;
            };
            let mut tab = state.tabs.remove(index);
            tab.info.group_id = None;
            tab.info.workspace = workspace.to_string();
            let end = state.global_index_of(&window, workspace, usize::MAX);
            let pinned = state.pinned_count_of(&window, workspace);
            let at = if tab.info.pinned {
                state.global_index_of(&window, workspace, pinned)
            } else {
                end
            };
            state.tabs.insert(at.min(state.tabs.len()), tab);
            state.selected.retain(|selected| selected != id);
            if !windows.contains(&window) {
                windows.push(window);
            }
        }
        state.normalize_groups();
        Ok(windows)
    })?;
    for window in windows {
        let active = with_state(app, |state| state.active_of(&window));
        match active {
            Some(id) => {
                ensure_webview(app, id)?;
                place_pages(app, &window, true);
                focus_active(app, &window);
            }
            None => {
                create_tab_on(app, &window, None, false, None)?;
            }
        }
    }
    emit_tabs(app);
    Ok(())
}

pub fn create_ephemeral_tab(app: &AppHandle, window: &str) -> Result<u32, String> {
    let url = resolve(app, paths::NEW_TAB);
    open(
        app,
        OpenTab {
            ephemeral: true,
            window: window.to_string(),
            ..OpenTab::new(url)
        },
    )
}

/// Tabs in this group load again, because a page view cannot change profile after it is created.
pub fn set_group_container(app: &AppHandle, group_id: u32, container: &str) -> Result<(), String> {
    if !crate::privacy::known_container(app, container) {
        return Err("That container no longer exists.".into());
    }
    let ids = with_state(app, |state| {
        let group = state
            .groups
            .iter_mut()
            .find(|group| group.id == group_id)
            .ok_or("That group no longer exists.")?;
        group.container = container.to_string();
        Ok::<Vec<u32>, String>(
            state
                .tabs
                .iter()
                .filter(|tab| tab.info.group_id == Some(group_id))
                .map(|tab| tab.info.id)
                .collect(),
        )
    })?;
    for id in ids {
        rebuild_page(app, id)?;
    }
    emit_tabs(app);
    Ok(())
}

pub fn clear_container(app: &AppHandle, container: &str) {
    let ids = with_state(app, |state| {
        let group_ids: Vec<u32> = state
            .groups
            .iter()
            .filter(|group| group.container == container)
            .map(|group| group.id)
            .collect();
        for group in &mut state.groups {
            if group.container == container {
                group.container.clear();
            }
        }
        state
            .tabs
            .iter()
            .filter(|tab| tab.info.group_id.is_some_and(|id| group_ids.contains(&id)))
            .map(|tab| tab.info.id)
            .collect::<Vec<_>>()
    });
    for id in ids {
        let _ = rebuild_page(app, id);
    }
    emit_tabs(app);
}

fn rebuild_page(app: &AppHandle, id: u32) -> Result<(), String> {
    let saved = with_state(app, |state| {
        let tab = state.tab_mut(id)?;
        let webview = tab.webview.take();
        let url = tab.info.url.clone();
        let window = tab.info.window.clone();
        tab.info.loaded = false;
        tab.info.loading = false;
        tab.painted = false;
        tab.hidden = false;
        tab.info.sleeping = false;
        Some((webview, url, window))
    });
    let Some((webview, url, window)) = saved else {
        return Ok(());
    };
    retire_webview(app, webview);
    let resolved = resolve(app, &url);
    with_state(app, |state| {
        if let Some(tab) = state.tab_mut(id) {
            tab.pending_url = Some(resolved);
        }
    });
    let active = with_state(app, |state| state.active_of(&window) == Some(id));
    if active {
        ensure_webview(app, id)?;
        place_pages(app, &window, true);
        focus_active(app, &window);
    }
    Ok(())
}

const REFRESH_CHOICES: &[u32] = &[0, 15, 30, 60, 300, 900];

pub fn set_refresh(app: &AppHandle, id: u32, secs: u32, watch: bool) -> Result<(), String> {
    let secs = if REFRESH_CHOICES.contains(&secs) {
        secs
    } else {
        0
    };
    let found = with_state(app, |state| {
        let tab = state.tab_mut(id)?;
        tab.info.refresh_secs = secs;
        tab.info.watch_changes = watch && secs > 0;
        tab.last_refresh = Instant::now();
        tab.content_mark = None;
        if secs == 0 {
            tab.info.changed = false;
        }
        Some(())
    });
    if found.is_none() {
        return Err("That tab is no longer open.".into());
    }
    emit_tabs(app);
    Ok(())
}

/// Tabs whose reload interval has elapsed. The bool is whether to compare page text first.
pub fn refresh_due(app: &AppHandle) -> Vec<(u32, bool)> {
    if app.try_state::<Tabs>().is_none() {
        return Vec::new();
    }
    with_state(app, |state| {
        state
            .tabs
            .iter()
            .filter(|tab| {
                tab.info.refresh_secs > 0
                    && tab.webview.is_some()
                    && !tab.info.sleeping
                    && !tab.hidden
                    && !tab.info.loading
                    && tab.info.url.starts_with("http")
                    && tab.last_refresh.elapsed()
                        >= Duration::from_secs(u64::from(tab.info.refresh_secs))
            })
            .map(|tab| (tab.info.id, tab.info.watch_changes))
            .collect()
    })
}

pub fn stamp_refresh(app: &AppHandle, id: u32) {
    with_state(app, |state| {
        if let Some(tab) = state.tab_mut(id) {
            tab.last_refresh = Instant::now();
        }
    });
}

/// Remembers the page text fingerprint. Returns true the first time it differs from a stored one.
pub fn note_content(app: &AppHandle, id: u32, mark: &str) -> bool {
    with_state(app, |state| {
        let Some(tab) = state.tab_mut(id) else {
            return false;
        };
        let changed = tab
            .content_mark
            .as_deref()
            .is_some_and(|previous| previous != mark);
        tab.content_mark = Some(mark.to_string());
        if changed {
            tab.info.changed = true;
        }
        changed
    })
}

/// Runs the page-control script again in every open http(s) tab.
pub fn reinject_page_rules(app: &AppHandle) {
    if app.try_state::<Tabs>().is_none() {
        return;
    }
    let pages = with_state(app, |state| {
        state
            .tabs
            .iter()
            .filter_map(|tab| {
                let label = tab.webview.clone()?;
                (tab.info.url.starts_with("http://") || tab.info.url.starts_with("https://"))
                    .then(|| (label, tab.info.url.clone()))
            })
            .collect::<Vec<_>>()
    });
    for (label, url) in pages {
        let script = crate::page_rules::injection(app, &url);
        if script.is_empty() {
            continue;
        }
        if let Some(webview) = app.get_webview(&label) {
            let _ = webview.eval(script);
        }
    }
}

pub fn run_zapper(app: &AppHandle, id: u32) -> Result<(), String> {
    let webview = page_view(app, id).ok_or("This page is not loaded yet.")?;
    webview
        .eval(crate::page_rules::zapper_script())
        .map_err(|error| error.to_string())
}

pub fn run_inspector(app: &AppHandle, id: u32) -> Result<(), String> {
    let webview = page_view(app, id).ok_or("This page is not loaded yet.")?;
    webview
        .eval(crate::page_rules::inspector_script())
        .map_err(|error| error.to_string())
}

pub fn tab_page_url(app: &AppHandle, id: u32) -> Option<String> {
    with_state(app, |state| {
        state
            .index_of(id)
            .map(|index| state.tabs[index].info.url.clone())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab(id: u32, group: Option<u32>, workspace: &str) -> Tab {
        Tab {
            info: TabInfo {
                id,
                url: format!("https://{id}.example"),
                title: String::new(),
                favicon: None,
                pinned: false,
                loading: false,
                can_go_back: false,
                can_go_forward: false,
                zoom: 1.0,
                loaded: false,
                window: WINDOW.into(),
                audible: false,
                muted: false,
                sleeping: false,
                group_id: group,
                workspace: workspace.into(),
                refresh_secs: 0,
                watch_changes: false,
                changed: false,
                ephemeral: false,
                errors: 0,
                accent: None,
            },
            pending_url: None,
            webview: None,
            painted: false,
            title_ready: false,
            last_shown: Instant::now(),
            hidden: false,
            applied_mute: None,
            last_refresh: Instant::now(),
            content_mark: None,
            accent_locked: false,
        }
    }

    fn state(tabs: Vec<Tab>, groups: &[u32]) -> TabsState {
        TabsState {
            tabs,
            windows: vec![WindowChrome::new(WINDOW, UI)],
            selected: Vec::new(),
            anchor: None,
            next_id: 100,
            next_window: 1,
            next_popup: 0,
            closed: Vec::new(),
            groups: groups
                .iter()
                .map(|id| TabGroup {
                    id: *id,
                    name: String::new(),
                    color: "blue".into(),
                    collapsed: false,
                    window: WINDOW.into(),
                    container: String::new(),
                })
                .collect(),
            next_group: 10,
            workspaces: normalize_workspaces(vec![Workspace {
                id: "work".into(),
                name: "Work".into(),
                color: "red".into(),
            }]),
        }
    }

    fn ids(state: &TabsState) -> Vec<u32> {
        state.tabs.iter().map(|tab| tab.info.id).collect()
    }

    #[test]
    fn a_group_keeps_its_tabs_together() {
        let mut state = state(
            vec![
                tab(1, Some(7), DEFAULT_WORKSPACE),
                tab(2, None, DEFAULT_WORKSPACE),
                tab(3, Some(7), DEFAULT_WORKSPACE),
            ],
            &[7],
        );
        state.normalize_groups();
        assert_eq!(ids(&state), vec![1, 3, 2]);
    }

    #[test]
    fn empty_groups_and_unknown_ids_are_dropped() {
        let mut state = state(vec![tab(1, Some(9), DEFAULT_WORKSPACE)], &[7]);
        state.normalize_groups();
        assert!(state.tabs[0].info.group_id.is_none());
        assert!(state.groups.is_empty());
    }

    #[test]
    fn a_device_frame_stays_inside_the_page() {
        let (x, y, width, height) = fit_device(1000, 700, 1280, 800);
        assert!(x + width <= 1000);
        assert!(y + height <= 700);
        assert_eq!(width, 1000);
        assert_eq!(height, 700);
        let (x, y, width, height) = fit_device(2000, 1400, 390, 844);
        assert_eq!((width, height), (390, 844));
        assert_eq!(x, (2000 - 390) / 2);
        assert_eq!(y, (1400 - 844) / 2);
    }

    #[test]
    fn the_strip_shows_one_workspace() {
        let state = state(
            vec![
                tab(1, None, DEFAULT_WORKSPACE),
                tab(2, None, "work"),
                tab(3, None, DEFAULT_WORKSPACE),
            ],
            &[],
        );
        let strip: Vec<u32> = state
            .strip_indexes(WINDOW)
            .into_iter()
            .map(|index| state.tabs[index].info.id)
            .collect();
        assert_eq!(strip, vec![1, 3]);
        assert_eq!(state.global_index_of(WINDOW, "work", 1), 2);
    }

    #[test]
    fn default_workspace_is_always_present() {
        let list = normalize_workspaces(vec![
            Workspace {
                id: "a".into(),
                name: " ".into(),
                color: "neon".into(),
            },
            Workspace {
                id: "a".into(),
                name: "Dup".into(),
                color: "red".into(),
            },
        ]);
        assert_eq!(list[0].id, DEFAULT_WORKSPACE);
        assert_eq!(list.len(), 2);
        assert_eq!(list[1].name, "Workspace");
        assert_eq!(list[1].color, "grey");
    }
}
