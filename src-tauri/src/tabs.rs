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

use crate::{devtools, history, paths, session, settings, webview2};

pub const WINDOW: &str = "main";
pub const UI: &str = "ui";
const MAX_CLOSED: usize = 25;
const DEFAULT_UI_HEIGHT: f64 = 84.0;
/// Extension popouts (`chrome.windows.create`) often arrive with no size, or with the narrow
/// toolbar width. A wider frame fits the vault UI.
const EXTENSION_POPOUT_WIDTH: f64 = 480.0;
const EXTENSION_POPOUT_HEIGHT: f64 = 680.0;
/// Restored pinned tabs wait this long, so the selected page loads first.
const PIN_LOAD_DELAY: Duration = Duration::from_millis(1500);
/// Gap between two restored pinned tabs starting to load.
const PIN_LOAD_GAP: Duration = Duration::from_millis(750);
/// Longest wait for a loading page before the next restored pinned tab starts anyway.
const PIN_LOAD_WAIT: Duration = Duration::from_secs(6);
const PIN_QUIET_POLL: Duration = Duration::from_millis(200);
const ZOOM_LEVELS: &[f64] = &[
    0.25, 0.33, 0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0,
];
pub const DEFAULT_WORKSPACE: &str = "default";
pub const GROUP_COLORS: &[&str] = &[
    "grey", "blue", "red", "yellow", "green", "pink", "purple", "cyan", "orange",
];
/// Chromium args for every WebView2 environment. Setting them replaces wry's defaults,
/// including `--autoplay-policy=no-user-gesture-required`, so that flag is repeated here.
/// `user-gesture-required` drops speaker audio that starts after the click returns, which
/// is how a call plays the other person. `AUTOPLAY_GUARD_SCRIPT` is what keeps a page quiet
/// until a click or key press inside it. Selecting the tab does not count.
///
/// `--remote-debugging-port=0` lets each profile pick a free localhost port and write it to
/// `DevToolsActivePort`. The docked inspector loads from that port. Any process on this machine
/// can attach for the life of the session. The same argument string is required on every webview
/// that shares a profile: WebView2 rejects a second webview whose environment options differ.
const WEBVIEW_BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --autoplay-policy=no-user-gesture-required --remote-debugging-port=0";
/// Logical px between the two split panes. The browser UI shows through it as the divider.
const SPLIT_GAP: f64 = 6.0;
/// Share of the page area kept for pages when developer tools are docked. The tools take the rest.
/// The divider in the browser UI uses the same limits.
const DEVTOOLS_RATIO: f64 = 0.62;
const DEVTOOLS_RATIO_MIN: f64 = 0.35;
const DEVTOOLS_RATIO_MAX: f64 = 0.8;
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
    /// The page stopped answering. Not written into the session.
    pub unresponsive: bool,
    /// `#rrggbb` from the site's theme color or favicon. Not written into the session.
    pub accent: Option<String>,
    /// Registrable domain of an http(s) page, such as `example.co.uk`. Empty for other pages.
    pub site: String,
    /// When the tab was last selected, in milliseconds since 1970. Zero until it is shown.
    pub last_active: u64,
    /// Load progress from 0 to 100. 100 once the page has finished.
    pub progress: u8,
    /// Why the page can't be shown. The browser UI draws this in place of the page.
    pub failure: Option<TabFailure>,
    /// Bytes the page used just before it went to sleep. Zero while awake or when unknown.
    pub sleep_saved: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TabFailure {
    /// `crashed`, `offline`, `dns`, `certificate`, `timeout`, `refused` or `other`.
    pub kind: String,
    /// Address that failed, so Retry can load it again.
    pub url: String,
    /// WebView2 error name, for example `HOST_NAME_NOT_RESOLVED`.
    pub code: String,
    /// Extra text, such as the certificate issuer and expiry.
    pub detail: String,
}

/// One tab as it was when it closed.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ClosedTab {
    pub url: String,
    pub title: String,
    pub favicon: Option<String>,
    pub pinned: bool,
    /// Position in its strip when it closed.
    pub index: usize,
    pub window: String,
    pub workspace: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ClosedGroup {
    pub name: String,
    pub color: String,
    pub container: String,
}

/// A closed tab, a closed group, or tabs closed together, restored as one.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ClosedEntry {
    pub id: u32,
    /// Milliseconds since 1970.
    pub closed_at: u64,
    /// `tab`, `group` or `batch`.
    pub kind: String,
    pub group: Option<ClosedGroup>,
    pub tabs: Vec<ClosedTab>,
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitInfo {
    pub left: u32,
    pub right: u32,
    pub ratio: f64,
}

/// Docked inspector for one window. Absent until the user opens it.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DevtoolsDock {
    /// Share of the page area the pages keep. Tools take the rest.
    pub ratio: f64,
    /// The column is on screen. Fullscreen keeps the dock open with this false.
    pub visible: bool,
}

/// CSS pixel size of the page view. The webview itself is this size, centered under the device bar.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageViewport {
    pub width: u32,
    pub height: u32,
    /// Black cutout over the top of the page, like an iPhone island.
    pub cutout: bool,
    /// Home indicator along the bottom of the page.
    pub home_bar: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowTabs {
    pub label: String,
    pub active_id: Option<u32>,
    pub fullscreen: bool,
    pub workspace: String,
    pub split: Option<SplitInfo>,
    /// Docked page inspector. Empty until the user opens it.
    pub devtools: Option<DevtoolsDock>,
    /// Address of the open peek, if any.
    pub peek_url: Option<String>,
    /// Set while the page view is a fixed CSS size. Empty uses the whole page area.
    pub viewport: Option<PageViewport>,
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
    /// Resident bookmarks page parked off the strip. The webview stays loaded.
    shelf: bool,
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

struct DevtoolsState {
    open: bool,
    pending: bool,
    ready: bool,
    ratio: f64,
    tab: Option<u32>,
    page: Option<String>,
    epoch: u64,
    job: u64,
    inflight: bool,
}

impl DevtoolsState {
    fn new() -> Self {
        Self {
            open: false,
            pending: false,
            ready: false,
            ratio: DEVTOOLS_RATIO,
            tab: None,
            page: None,
            epoch: 0,
            job: 0,
            inflight: false,
        }
    }

    fn engaged(&self) -> bool {
        self.open || self.pending
    }

    /// Drops in-flight inspector work. The returned epoch is the only one allowed to finish.
    fn cancel_work(&mut self) -> u64 {
        self.job = self.job.wrapping_add(1);
        self.epoch = self.epoch.wrapping_add(1);
        self.inflight = false;
        self.epoch
    }
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
    /// Toast view drawn over the bottom of the pages. Menus open and close without touching it.
    toast: Option<String>,
    /// Page views on screen and where each one sits.
    shown: Vec<(String, Slot)>,
    split: Option<Split>,
    devtools: DevtoolsState,
    workspace: String,
    /// Active tab of each workspace this window has left, so switching back returns to it.
    active_by_workspace: HashMap<String, u32>,
    /// CSS size of the page view. Empty uses the whole page area.
    viewport: Option<(u32, u32)>,
    viewport_cutout: bool,
    viewport_home: bool,
    /// Screencast frames just after the bar was closed are the same session, not a new request.
    screencast_suppress: bool,
    screencast_at: Instant,
    /// Resident bookmarks tab for this window. Its page stays loaded while shelved.
    bookmarks: Option<u32>,
    /// The window is closing, so the resident bookmarks tab is destroyed with the rest.
    discarding: bool,
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
            toast: None,
            shown: Vec::new(),
            split: None,
            devtools: DevtoolsState::new(),
            workspace: DEFAULT_WORKSPACE.to_string(),
            active_by_workspace: HashMap::new(),
            viewport: None,
            viewport_cutout: true,
            viewport_home: true,
            screencast_suppress: false,
            screencast_at: Instant::now()
                .checked_sub(Duration::from_secs(60))
                .unwrap_or_else(Instant::now),
            bookmarks: None,
            discarding: false,
        }
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
    /// Oldest first. Capped at `MAX_CLOSED` entries.
    closed: Vec<ClosedEntry>,
    next_closed: u32,
    groups: Vec<TabGroup>,
    next_group: u32,
    workspaces: Vec<Workspace>,
}

impl TabsState {
    fn push_closed(
        &mut self,
        kind: &str,
        group: Option<ClosedGroup>,
        tabs: Vec<ClosedTab>,
    ) -> Option<u32> {
        if tabs.is_empty() {
            return None;
        }
        self.next_closed += 1;
        let id = self.next_closed;
        self.closed.push(ClosedEntry {
            id,
            closed_at: now_ms(),
            kind: kind.to_string(),
            group,
            tabs,
        });
        if self.closed.len() > MAX_CLOSED {
            let extra = self.closed.len() - MAX_CLOSED;
            self.closed.drain(..extra);
        }
        Some(id)
    }

    fn take_closed(&mut self, id: Option<u32>) -> Option<ClosedEntry> {
        let index = match id {
            Some(id) => self.closed.iter().position(|entry| entry.id == id)?,
            None => self.closed.len().checked_sub(1)?,
        };
        Some(self.closed.remove(index))
    }

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
            .filter(|(_, tab)| {
                tab.info.window == window && tab.info.workspace == workspace && !tab.shelf
            })
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

    fn page_loading(&self) -> bool {
        self.tabs
            .iter()
            .any(|tab| !tab.shelf && tab.webview.is_some() && tab.info.loading)
    }

    /// First pinned tab, in strip order, that has no page view and is not in `skip`.
    fn unloaded_pin(&self, skip: &[u32]) -> Option<u32> {
        self.tabs
            .iter()
            .find(|tab| tab.info.pinned && tab.webview.is_none() && !skip.contains(&tab.info.id))
            .map(|tab| tab.info.id)
    }

    /// The window's resident bookmarks tab, when that tab is still open.
    fn resident_bookmarks(&self, window: &str) -> Option<u32> {
        let id = self.chrome(window)?.bookmarks?;
        self.index_of(id)?;
        Some(id)
    }

    fn is_resident_bookmarks(&self, id: u32) -> bool {
        self.tab_window(id)
            .is_some_and(|window| self.resident_bookmarks(&window) == Some(id))
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

    fn snapshot(&self) -> TabsSnapshot {
        let main = self.chrome(WINDOW);
        TabsSnapshot {
            tabs: self
                .tabs
                .iter()
                .filter(|tab| !tab.shelf)
                .map(|tab| {
                    let mut info = tab.info.clone();
                    info.site = paths::site_of(&info.url);
                    info
                })
                .collect(),
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
                    devtools: published_devtools(chrome),
                    peek_url: None,
                    viewport: chrome.viewport.map(|(width, height)| PageViewport {
                        width,
                        height,
                        cutout: chrome.viewport_cutout,
                        home_bar: chrome.viewport_home,
                    }),
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
            .filter(|tab| !tab.shelf)
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
        next_closed: 0,
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

fn published_devtools(chrome: &WindowChrome) -> Option<DevtoolsDock> {
    if !chrome.devtools.engaged() {
        return None;
    }
    Some(DevtoolsDock {
        ratio: chrome.devtools.ratio,
        visible: chrome.devtools.open && chrome.devtools.ready && !chrome.fullscreen,
    })
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

/// The browser window the user is typing in, or the first window. A focused popup counts as
/// the window it belongs to.
///
/// `AppHandle::get_focused_window` holds Tauri's window lock while it asks the main thread
/// about each window, which deadlocks against a main thread looking up a window.
pub fn focused_window(app: &AppHandle) -> String {
    let foreground = webview2::foreground_root_owner();
    if foreground != 0 {
        for label in browser_labels(app) {
            let hwnd = app
                .get_window(&label)
                .and_then(|window| window.hwnd().ok())
                .map_or(0, |hwnd| hwnd.0 as isize);
            if hwnd == foreground {
                return label;
            }
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
            .additional_browser_args(WEBVIEW_BROWSER_ARGS)
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
        .additional_browser_args(WEBVIEW_BROWSER_ARGS)
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
    /// Session restore opens a bookmarks tab instead of revealing the resident page.
    pub keep_bookmarks: bool,
    /// Park the tab off the strip. The resident bookmarks page is created this way.
    pub shelf: bool,
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
            keep_bookmarks: false,
            shelf: false,
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
    if !tab.keep_bookmarks
        && let Some(target) = bookmarks_target(&display)
    {
        let window = if tab.window.is_empty() {
            focused_window(app)
        } else {
            tab.window
        };
        return reveal_bookmarks(app, &window, target.folder.as_deref());
    }
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
                    unresponsive: false,
                    accent: None,
                    site: String::new(),
                    last_active: 0,
                    progress: 0,
                    failure: None,
                    sleep_saved: 0,
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
                shelf: tab.shelf,
            },
        );
        state.normalize_groups();
        let current = workspace == state.workspace_of(&window);
        let none_active = state.active_of(&window).is_none();
        if !tab.shelf && current && (!tab.background || (none_active && !tab.lazy)) {
            state.set_window_active(&window, Some(id));
        }
        id
    });
    if crate::system::handler_matches(tab.url.as_str(), &crate::settings::get(app).file_handlers) {
        crate::system::claim_file(id);
    }
    let is_active = lock(app).active_of(&window) == Some(id);
    if is_active {
        activate(app, id)?;
    } else {
        if !tab.lazy {
            // Leave an open menu up. The click that asked for a background tab did not dismiss it.
            open_page_view(app, id, false)?;
        }
        emit_tabs(app);
    }
    Ok(id)
}

fn ensure_webview(app: &AppHandle, id: u32) -> Result<(), String> {
    open_page_view(app, id, true)
}

/// Builds the page view for `id`. `dismiss` closes menus; a background tab leaves them open.
fn open_page_view(app: &AppHandle, id: u32, dismiss: bool) -> Result<(), String> {
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
    if dismiss {
        if let Some(window) = with_state(app, |state| state.tab_window(id)) {
            crate::popup::close_all(app, &window, true);
        }
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
        if !dismiss {
            crate::popup::refocus(app, &window);
        }
    }
    Ok(())
}

fn build_page_view(app: &AppHandle, id: u32, label: &str, url: Url) -> Result<Webview, String> {
    let extension_page = is_extension_url(url.as_str());
    let window_label =
        with_state(app, |state| state.tab_window(id)).unwrap_or_else(|| WINDOW.to_string());
    let window = app
        .get_window(&window_label)
        .ok_or("The browser window is missing.")?;
    let (_, size) = page_rect(app, &window_label);
    let color = document_color(app, url.as_str());
    let load_app = app.clone();
    let title_app = app.clone();
    let window_app = app.clone();
    let builder = page_builder(app, id, label, WebviewUrl::External(url))
        // Tauri's handler consumes file drops on Windows. The page needs them for its own drop zones.
        .disable_drag_drop_handler()
        .background_color(color)
        .focused(false)
        .initialization_script(AUTOPLAY_GUARD_SCRIPT)
        .initialization_script(page_background_script(app))
        .initialization_script(PEEK_CLICK_SCRIPT)
        .initialization_script(ZOOM_WHEEL_SCRIPT)
        .on_page_load(move |_, payload| {
            let finished = matches!(payload.event(), PageLoadEvent::Finished);
            let loading = matches!(payload.event(), PageLoadEvent::Started);
            let url = paths::display_url(&load_app, payload.url().as_str());
            let extension_page = is_extension_url(&url);
            let new_tab_page = url == paths::NEW_TAB;
            let fresh = with_state(&load_app, |state| {
                state.tab_mut(id).is_some_and(|tab| !tab.painted)
            });
            let first = finished && fresh;
            let extension_zoom =
                (finished && extension_page).then(|| settings::get(&load_app).ui_zoom_factor());
            update(&load_app, id, |tab| {
                if tab.info.url != url {
                    tab.title_ready = false;
                }
                tab.info.loading = loading;
                tab.info.url = url;
                if finished {
                    tab.painted = true;
                }
                if let Some(factor) = extension_zoom {
                    tab.info.zoom = factor;
                }
            });
            let owner = with_state(&load_app, |state| state.tab_window(id))
                .unwrap_or_else(|| WINDOW.to_string());
            if finished {
                remember_visit(&load_app, id);
                place_pages(&load_app, &owner, false);
                apply_device_chrome(&load_app, &owner);
                crate::system::attach_open_file(&load_app, id, payload.url().as_str());
            }
            let active = lock(&load_app).active_of(&owner) == Some(id);
            // The new page view takes focus as soon as it starts loading. Take it back
            // for the new tab page then, instead of waiting until the document finishes.
            // A background tab gives it back to the menu that opened the tab.
            if active && new_tab_page && fresh && (loading || finished) {
                focus_active(&load_app, &owner);
            } else if first && active {
                focus_active(&load_app, &owner);
            } else if !active && (loading || first) {
                crate::popup::refocus(&load_app, &owner);
            }
            // A reload restores the profile's per-site zoom. Extension pages stay on Browser UI size.
            if let Some(factor) = extension_zoom
                && let Some(webview) = page_view(&load_app, id)
            {
                let _ = webview.set_zoom(factor);
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
    if extension_page || (zoom - 1.0).abs() > 0.001 {
        let _ = webview.set_zoom(zoom);
    }
    Ok(webview)
}

fn is_extension_url(url: &str) -> bool {
    url.starts_with("chrome-extension:")
}

/// Zoom a page opens at: 100% for `ce://` pages, Browser UI size for extension pages,
/// the site's remembered zoom, or the default page zoom.
fn page_zoom_for(app: &AppHandle, url: &str) -> f64 {
    if url.starts_with(paths::INTERNAL_SCHEME) {
        1.0
    } else if is_extension_url(url) {
        settings::get(app).ui_zoom_factor()
    } else {
        crate::page_rules::zoom_for(app, url)
            .unwrap_or_else(|| settings::get(app).page_zoom_factor())
    }
}

/// Pauses video and audio that start before the user clicks or presses a key in this document.
/// The click that opened the tab, and selecting the tab, do not count: both can satisfy
/// Chromium's autoplay policy without a gesture inside this page.
///
/// A call plays the dial tone from the click, then the other person through an `AudioContext`
/// or a frame that did not receive that click. `suspend()` stays in progress after it is called,
/// and `resume()` while the context is still running does nothing, so the context was left
/// suspended and the answered call was silent. Resume waits until that suspend finishes.
/// Ringback and the microphone resume that context. Ordinary video stays paused until a click.
const AUTOPLAY_GUARD_SCRIPT: &str = r#"(function () {
  if (window.__ceAutoplayGuard || !window.HTMLMediaElement || !HTMLMediaElement.prototype) return;
  window.__ceAutoplayGuard = true;
  var allowed = false;
  var audioLive = false;
  var nativePlay = HTMLMediaElement.prototype.play;
  var nativePause = HTMLMediaElement.prototype.pause;
  var roots = [];
  var contexts = [];
  var resumeWaiters = [];
  var nativeResume = null;
  var nativeSuspend = null;
  var timer = 0;
  var scan = 0;

  function finish() {
    if (timer) clearInterval(timer);
    if (scan) clearInterval(scan);
    timer = 0;
    scan = 0;
  }
  function notifyFrames() {
    var frames = [];
    function collect(root, depth) {
      if (!root || depth > 6 || !root.querySelectorAll) return;
      var found;
      try { found = root.querySelectorAll("iframe, frame"); } catch (error) { found = []; }
      for (var i = 0; i < found.length; i++) frames.push(found[i]);
      var nodes;
      try { nodes = root.querySelectorAll("*"); } catch (error) { return; }
      for (var n = 0; n < nodes.length; n++) {
        if (nodes[n].shadowRoot) collect(nodes[n].shadowRoot, depth + 1);
      }
    }
    collect(document, 0);
    for (var i = 0; i < frames.length; i++) {
      try {
        if (frames[i].contentWindow) frames[i].contentWindow.postMessage({ type: "ce-autoplay-release" }, "*");
      } catch (error) {}
    }
  }
  function notifyParent() {
    if (!window.parent || window.parent === window) return;
    try { window.parent.postMessage({ type: "ce-autoplay-release-up" }, "*"); } catch (error) {}
  }
  function resumeContext(ctx) {
    if (!nativeResume) return Promise.resolve();
    var pending = ctx.__ceSuspend;
    ctx.__ceSuspend = null;
    var run = function () { return nativeResume.call(ctx); };
    if (pending && pending.then) return pending.then(run, run);
    return run();
  }
  function resumeParked() {
    audioLive = true;
    var pending = contexts.slice();
    for (var i = 0; i < pending.length; i++) {
      try {
        var resumed = resumeContext(pending[i]);
        if (resumed && resumed.catch) resumed.catch(function () {});
      } catch (error) {}
    }
    var waiting = resumeWaiters;
    resumeWaiters = [];
    for (var n = 0; n < waiting.length; n++) {
      try { waiting[n](); } catch (error) {}
    }
  }
  function release() {
    if (allowed) {
      finish();
      return;
    }
    allowed = true;
    finish();
    resumeParked();
    notifyFrames();
    notifyParent();
  }
  function arm(event) {
    if (event && event.isTrusted) release();
  }
  ["pointerdown", "mousedown", "touchstart", "keydown", "click"].forEach(function (type) {
    window.addEventListener(type, arm, true);
  });

  var NativeAudio = window.AudioContext || window.webkitAudioContext;
  if (NativeAudio && NativeAudio.prototype && NativeAudio.prototype.resume && NativeAudio.prototype.suspend) {
    nativeResume = NativeAudio.prototype.resume;
    nativeSuspend = NativeAudio.prototype.suspend;
    function park(ctx) {
      if (contexts.indexOf(ctx) === -1) contexts.push(ctx);
      var op;
      try { op = nativeSuspend.call(ctx); } catch (error) { op = Promise.resolve(); }
      if (!op || !op.then) op = Promise.resolve();
      ctx.__ceSuspend = op;
      op.then(function () {
        if (allowed) return nativeResume.call(ctx);
      }).catch(function () {});
    }
    var WrappedAudio = function AudioContext(options) {
      var ctx = options === undefined ? new NativeAudio() : new NativeAudio(options);
      if (!allowed && !audioLive) park(ctx);
      return ctx;
    };
    WrappedAudio.prototype = NativeAudio.prototype;
    if (window.AudioContext) window.AudioContext = WrappedAudio;
    if (window.webkitAudioContext) window.webkitAudioContext = WrappedAudio;
    NativeAudio.prototype.resume = function () {
      var ctx = this;
      if (!allowed && !audioLive) {
        return new Promise(function (resolve, reject) {
          resumeWaiters.push(function () {
            resumeContext(ctx).then(resolve, reject);
          });
        });
      }
      return resumeContext(ctx);
    };
  }

  function streamOnly(media) {
    try {
      var value = media && media.srcObject;
      if (!value) return false;
      if (typeof MediaStream !== "undefined" && value instanceof MediaStream) return true;
      return typeof value.getTracks === "function";
    } catch (error) {
      return false;
    }
  }

  function stop(media) {
    if (allowed || !media || media.nodeType !== 1) return;
    var tag = media.tagName;
    if (tag !== "VIDEO" && tag !== "AUDIO") return;
    if (streamOnly(media)) return;
    try {
      if (media.autoplay || (media.hasAttribute && media.hasAttribute("autoplay"))) media.autoplay = false;
      if (!media.paused && !media.ended) nativePause.call(media);
    } catch (error) {}
  }

  function watch(root) {
    if (!root || root.__ceAutoplayWatch) return;
    try { root.__ceAutoplayWatch = true; } catch (error) { return; }
    function onPlay(event) {
      if (streamOnly(event.target)) resumeParked();
      else stop(event.target);
    }
    root.addEventListener("play", onPlay, true);
    root.addEventListener("playing", onPlay, true);
  }

  function remember(root) {
    if (!root || roots.indexOf(root) !== -1) return;
    roots.push(root);
    watch(root);
  }

  function discover(root, depth) {
    if (allowed || !root || depth > 6 || !root.querySelectorAll) return;
    remember(root);
    var nodes;
    try { nodes = root.querySelectorAll("*"); } catch (error) { return; }
    for (var n = 0; n < nodes.length; n++) {
      if (nodes[n].shadowRoot) discover(nodes[n].shadowRoot, depth + 1);
    }
  }

  function tick() {
    if (allowed) {
      release();
      return;
    }
    for (var i = 0; i < roots.length; i++) {
      var media;
      try { media = roots[i].querySelectorAll("video, audio"); } catch (error) { continue; }
      for (var n = 0; n < media.length; n++) stop(media[n]);
    }
  }

  HTMLMediaElement.prototype.play = function () {
    if (streamOnly(this)) {
      resumeParked();
      return nativePlay.apply(this, arguments);
    }
    if (!allowed) {
      var media = this;
      media.__ceWantPlay = true;
      var settled = new Promise(function (resolve, reject) {
        queueMicrotask(function () {
          if (streamOnly(media) || allowed) {
            if (streamOnly(media)) resumeParked();
            try { nativePlay.call(media).then(resolve, reject); } catch (error) { reject(error); }
            return;
          }
          try { media.autoplay = false; } catch (error) {}
          stop(media);
          reject(new DOMException("Playback needs a click in the page.", "NotAllowedError"));
        });
      });
      settled.catch(function () {});
      return settled;
    }
    return nativePlay.apply(this, arguments);
  };

  var autoplay = Object.getOwnPropertyDescriptor(HTMLMediaElement.prototype, "autoplay");
  if (autoplay && autoplay.get && autoplay.set) {
    Object.defineProperty(HTMLMediaElement.prototype, "autoplay", {
      configurable: true,
      enumerable: autoplay.enumerable,
      get: function () { return autoplay.get.call(this); },
      set: function (value) {
        if (value) this.__ceWantAutoplay = true;
        if (value && streamOnly(this)) resumeParked();
        autoplay.set.call(this, !!value && (allowed || streamOnly(this)));
      }
    });
  }

  var srcObject = Object.getOwnPropertyDescriptor(HTMLMediaElement.prototype, "srcObject");
  if (srcObject && srcObject.get && srcObject.set) {
    Object.defineProperty(HTMLMediaElement.prototype, "srcObject", {
      configurable: true,
      enumerable: srcObject.enumerable,
      get: function () { return srcObject.get.call(this); },
      set: function (value) {
        srcObject.set.call(this, value);
        if (!streamOnly(this)) return;
        var wanted = this.__ceWantPlay || this.__ceWantAutoplay;
        resumeParked();
        if (!wanted) return;
        this.__ceWantPlay = false;
        try {
          var started = nativePlay.call(this);
          if (started && started.catch) started.catch(function () {});
        } catch (error) {}
      }
    });
  }

  var devices = navigator.mediaDevices;
  if (devices && devices.getUserMedia) {
    var nativeCapture = devices.getUserMedia.bind(devices);
    devices.getUserMedia = function (constraints) {
      return nativeCapture(constraints).then(function (stream) {
        try {
          if (stream && stream.getAudioTracks && stream.getAudioTracks().length) resumeParked();
        } catch (error) {}
        return stream;
      });
    };
  }

  var nativeAttach = Element.prototype.attachShadow;
  if (nativeAttach) {
    Element.prototype.attachShadow = function () {
      var root = nativeAttach.apply(this, arguments);
      remember(root);
      return root;
    };
  }

  remember(document);
  window.__ceStopAutoplay = function () {
    discover(document, 0);
    tick();
  };
  window.__cePlayMedia = function (media) {
    release();
    return nativePlay.call(media);
  };
  tick();
  timer = setInterval(tick, 250);
  scan = setInterval(function () { discover(document, 0); }, 2000);
  document.addEventListener("DOMContentLoaded", function () {
    discover(document, 0);
    tick();
  });
  window.addEventListener("message", function (event) {
    var data = event.data;
    if (!data || typeof data !== "object") return;
    if (data.type === "ce-autoplay-query") {
      if (allowed && event.source && event.source !== window) {
        try { event.source.postMessage({ type: "ce-autoplay-release" }, "*"); } catch (error) {}
      }
      return;
    }
    if (data.type === "ce-autoplay-release" && event.source === window.parent && window.parent !== window) {
      release();
      return;
    }
    var fromChild = event.source && event.source !== window && event.source.parent === window;
    if (data.type === "ce-autoplay-release-up" && fromChild && event.origin === location.origin) release();
  });
  if (window.parent && window.parent !== window) {
    try { window.parent.postMessage({ type: "ce-autoplay-query" }, "*"); } catch (error) {}
  }
})();"#;

/// Ctrl+scroll zooms the page. A site that already handled the wheel, such as a map, is left alone.
const ZOOM_WHEEL_SCRIPT: &str = r#"(function () {
  var bucket = 0;
  window.addEventListener("wheel", function (event) {
    if (!event.ctrlKey || event.defaultPrevented || event.deltaY === 0) {
      if (!event.ctrlKey) bucket = 0;
      return;
    }
    event.preventDefault();
    if (event.deltaMode === 2) {
      bucket = 0;
      post(event.deltaY < 0 ? "in" : "out");
      return;
    }
    bucket += event.deltaY;
    var notch = event.deltaMode === 1 ? 1 : 100;
    if (Math.abs(bucket) < notch) return;
    var change = bucket < 0 ? "in" : "out";
    bucket = 0;
    post(change);
  }, { passive: false });
  function post(change) {
    if (window.chrome && window.chrome.webview) {
      window.chrome.webview.postMessage({ type: "ce-zoom", change: change });
    }
  }
})();"#;

/// Shift+click on a web link asks the browser for a peek. `page_hooks` reads the message.
/// The click itself is left alone: WebView2 also raises a new-window request for it, and
/// cancelling the click here drops that request when the message never arrives.
const PEEK_CLICK_SCRIPT: &str = r#"(function () {
  if (!/^https?:$/.test(location.protocol)) return;
  function linkHref(event) {
    var path = event.composedPath ? event.composedPath() : [event.target];
    for (var i = 0; i < path.length; i++) {
      var node = path[i];
      if (!node || node.nodeType !== 1) continue;
      var link = node.tagName === "A" ? node : (node.closest ? node.closest("a[href]") : null);
      if (!link) continue;
      var href = typeof link.href === "string" ? link.href : "";
      if (/^https?:/i.test(href)) return href;
    }
    return "";
  }
  document.addEventListener("click", function (event) {
    if (!event.shiftKey || event.ctrlKey || event.metaKey || event.altKey || event.button !== 0) return;
    var href = linkHref(event);
    if (!href || !window.chrome || !window.chrome.webview) return;
    window.chrome.webview.postMessage({ type: "ce-peek", url: href });
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
/// payment windows) open as real windows so `window.opener` keeps working. A
/// `chrome-extension:` popout does too, even when the request leaves the size out.
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
///
/// The window starts at `about:blank` and is handed to `SetNewWindow` before anything navigates
/// it. WebView2 commits the popout. Navigating or resizing from that callback never returns, so
/// the follow-up runs on a later turn of the main thread.
fn script_window(app: &AppHandle, url: Url, features: NewWindowFeatures) -> NewWindowResponse<Wry> {
    let extension = url.scheme() == "chrome-extension";
    let Some(size) = features.size().or_else(|| {
        extension.then_some(LogicalSize::new(
            EXTENSION_POPOUT_WIDTH,
            EXTENSION_POPOUT_HEIGHT,
        ))
    }) else {
        return NewWindowResponse::Deny;
    };
    let Ok(blank) = Url::parse("about:blank") else {
        return NewWindowResponse::Deny;
    };
    let factor = if extension {
        settings::get(app).ui_zoom_factor()
    } else {
        1.0
    };
    let width = if extension {
        size.width.max(EXTENSION_POPOUT_WIDTH)
    } else {
        size.width.max(240.0)
    } * factor;
    let height = size.height.max(if extension { 320.0 } else { 160.0 }) * factor;
    let label = with_state(app, |state| {
        state.next_popup += 1;
        format!("script-{}", state.next_popup)
    });
    let environment = features.opener().environment.clone();
    let position = features.position();
    let host = app.clone();
    let mut builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(blank))
        .title(if extension { "Extension" } else { "ce-browser" })
        .inner_size(width, height)
        .min_inner_size(
            (if extension { 420.0 } else { 240.0 }) * factor,
            (if extension { 320.0 } else { 160.0 }) * factor,
        )
        .decorations(true)
        .shadow(true)
        .focused(true)
        .browser_extensions_enabled(true)
        .with_environment(environment)
        .on_new_window(move |url, features| new_window(&host, None, url, features));
    if extension {
        builder = builder
            .background_color(settings::page_color(app))
            .on_document_title_changed(|window, title| {
                let title = title.trim();
                if title.is_empty() || title.eq_ignore_ascii_case("about:blank") {
                    return;
                }
                let _ = window.set_title(title);
            });
    }
    if let Some(position) = position {
        builder = builder.position(position.x, position.y);
    } else if extension && let Some((x, y)) = centered_on_browser(app, width, height) {
        builder = builder.position(x, y);
    }
    match builder.build() {
        Ok(window) => {
            webview2::close_window_when_requested(app, window.label());
            if extension {
                let _ = window.set_zoom(factor);
                open_extension_popout(app, window.label(), url, width, height);
            }
            NewWindowResponse::Create { window }
        }
        Err(error) => {
            eprintln!("script window: {error}");
            NewWindowResponse::Deny
        }
    }
}

/// Sizes the popout after `SetNewWindow` has returned. The page lays out against a zero-size view
/// until the frame is resized, so this nudges the window once the document is up. Navigating from
/// the navigation callback freezes the UI thread, so the blank-page load is here too.
fn open_extension_popout(app: &AppHandle, label: &str, target: Url, width: f64, height: f64) {
    let app = app.clone();
    let label = label.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(250));
        settle_extension_popout(&app, &label, Some(target), width, height);
        std::thread::sleep(Duration::from_millis(500));
        settle_extension_popout(&app, &label, None, width, height);
    });
}

fn settle_extension_popout(
    app: &AppHandle,
    label: &str,
    target: Option<Url>,
    width: f64,
    height: f64,
) {
    let host = app.clone();
    let label = label.to_string();
    let _ = app.run_on_main_thread(move || {
        if let Some(target) = target
            && let Some(webview) = host.get_webview(&label)
        {
            webview2::fill_client_area(&webview);
            let blank = webview
                .url()
                .ok()
                .is_none_or(|current| current.as_str().starts_with("about:blank"));
            if blank {
                let _ = webview.navigate(target);
            }
        }
        let Some(window) = host.get_window(&label) else {
            return;
        };
        let _ = window.set_size(LogicalSize::new(width + 1.0, height));
        let again = host.clone();
        let label = label.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(40));
            let window_host = again.clone();
            let _ = again.run_on_main_thread(move || {
                if let Some(window) = window_host.get_window(&label) {
                    let _ = window.set_size(LogicalSize::new(width, height));
                }
            });
        });
    });
}

/// Screen position that puts a `width` by `height` window over the browser the user is in.
fn centered_on_browser(app: &AppHandle, width: f64, height: f64) -> Option<(f64, f64)> {
    let window = app.get_window(&focused_window(app))?;
    let scale = window.scale_factor().ok()?;
    if scale <= f64::EPSILON {
        return None;
    }
    let origin = window.outer_position().ok()?;
    let outer = window.outer_size().ok()?;
    let x = origin.x as f64 / scale + (outer.width as f64 / scale - width) / 2.0;
    let y = origin.y as f64 / scale + (outer.height as f64 / scale - height) / 2.0;
    Some((x, y))
}

fn blank_tab(
    app: &AppHandle,
    host: Option<String>,
    opener: Option<u32>,
    url: Url,
    features: NewWindowFeatures,
) -> NewWindowResponse<Wry> {
    if features.size().is_some() || url.scheme() == "chrome-extension" {
        return script_window(app, url, features);
    }
    // Shift+click is a new-window request. Opening a tab for it focuses that tab and
    // closes the peek the page script just asked for.
    let (shift, ctrl, alt) = webview2::pointer_modifiers();
    let remembered = opener.is_some_and(|id| crate::peek::remembers_click(id, url.as_str()));
    if let Some(id) = opener
        && (crate::peek::from_new_window(shift, ctrl, alt, url.scheme()) || remembered)
    {
        let app = app.clone();
        let raw = url.to_string();
        if settings::get(&app).peek_on_shift_click {
            crate::peek::hold_focus(&app, id);
        }
        tauri::async_runtime::spawn(async move {
            crate::peek::request(&app, id, &raw);
        });
        return NewWindowResponse::Deny;
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
    let _ = ensure_bookmarks_host(app, &label);
    Ok(label)
}

/// Opens `url` in a new browser window, offset from the window in front.
pub fn open_url_window(app: &AppHandle, url: &str) -> Result<(), String> {
    let source = focused_window(app);
    let label = open_site_window(app, "ce-browser", url)?;
    if let (Some(source_window), Some(created)) = (app.get_window(&source), app.get_window(&label))
        && let Ok(position) = source_window.outer_position()
    {
        let _ = created.set_position(PhysicalPosition::new(position.x + 48, position.y + 48));
    }
    Ok(())
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
    // Mica and Acrylic only show through a window created transparent.
    let backdrop = crate::backdrop::wanted(app).is_some();
    let saved = (label == WINDOW)
        .then(|| crate::placement::saved(app))
        .flatten();
    let restoring = saved.is_some();
    let mut builder = WindowBuilder::new(app, label)
        .title(crate::profiles::window_title(app))
        .inner_size(1280.0, 840.0)
        .min_inner_size(640.0, 420.0)
        .decorations(false)
        .shadow(true)
        .transparent(backdrop);
    if restoring {
        builder = builder.visible(false);
    }
    let window = builder.build().map_err(|error| error.to_string())?;
    crate::profiles::apply_window_icon(app, &window);
    if let Some(saved) = saved {
        crate::placement::apply(app, &window, saved);
    }
    let size = window.inner_size().map_err(|error| error.to_string())?;
    if backdrop {
        crate::backdrop::note_transparent(label);
    }
    let (r, g, b) = settings::page_surface(app);
    let background = if backdrop {
        Color(0, 0, 0, 0)
    } else {
        Color(r, g, b, 255)
    };
    let _ = window.set_background_color(Some(background));
    let ui_app = app.clone();
    let host = label.to_string();
    let ui_view = window
        .add_child(
            base_builder(app, &ui, WebviewUrl::App("index.html".into()))
                .disable_drag_drop_handler()
                .background_color(background)
                .initialization_script(page_background_script(app))
                .on_new_window(move |url, features| {
                    blank_tab(&ui_app, Some(host.clone()), None, url, features)
                }),
            LogicalPosition::new(0.0, 0.0),
            size,
        )
        .map_err(|error| error.to_string())?;
    webview2::attach_ui_hooks(app, &ui_view, label);
    if backdrop {
        crate::backdrop::apply_all(app);
    }
    crate::popup::ensure_pool(app, label);
    if restoring {
        let _ = window.show();
    }
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
        if state.chrome(&source).and_then(|chrome| chrome.bookmarks) == Some(id) {
            if let Some(chrome) = state.chrome_mut(&source) {
                chrome.bookmarks = None;
            }
            if let Some(chrome) = state.chrome_mut(&label) {
                chrome.bookmarks = Some(id);
            }
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
        crate::popup::forget(app, &source);
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
    if with_state(app, |state| state.chrome(&source).is_some()) {
        let _ = ensure_bookmarks_host(app, &source);
    }
    let _ = ensure_bookmarks_host(app, &label);
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
    with_state(app, |state| {
        if let Some(chrome) = state.chrome_mut(label) {
            chrome.discarding = true;
        }
    });
    crate::peek::dismiss(app, label);
    crate::backdrop::forget(label);
    let ids = with_state(app, |state| {
        state
            .indexes_in(label)
            .into_iter()
            .map(|index| state.tabs[index].info.id)
            .collect::<Vec<_>>()
    });
    let _ = close_many(app, &ids, "batch", None);
    crate::popup::forget(app, label);
    crate::undo::close_window(app, label);
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

/// Height of the device bar drawn above the page, in CSS pixels. The page view starts below it.
const VIEWPORT_BAR: f64 = 36.0;
const VIEWPORT_MIN: u32 = 200;
const VIEWPORT_MAX: u32 = 3840;

fn clamp_viewport(width: u32, height: u32) -> Option<(u32, u32)> {
    if width < VIEWPORT_MIN || height < VIEWPORT_MIN {
        return None;
    }
    Some((width.min(VIEWPORT_MAX), height.min(VIEWPORT_MAX)))
}

/// Centers a CSS viewport in the page column, under the device bar.
fn viewport_frame(
    viewport: Option<(u32, u32)>,
    page_w: u32,
    page_h: u32,
    scale: f64,
) -> Option<(i32, i32, u32, u32)> {
    let (css_w, css_h) = viewport?;
    let bar = (VIEWPORT_BAR * scale).round() as u32;
    let avail_h = page_h.saturating_sub(bar).max(1);
    let want_w = (f64::from(css_w) * scale).round().max(1.0) as u32;
    let want_h = (f64::from(css_h) * scale).round().max(1.0) as u32;
    let (x, y, width, height) = fit_device(page_w, avail_h, want_w, want_h);
    Some((x as i32, y as i32 + bar as i32, width, height))
}

pub fn set_viewport(
    app: &AppHandle,
    window: &str,
    width: u32,
    height: u32,
    cutout: bool,
    home_bar: bool,
) {
    let Some(size) = clamp_viewport(width, height) else {
        clear_viewport(app, window);
        return;
    };
    let found = with_state(app, |state| {
        let chrome = state.chrome_mut(window)?;
        chrome.screencast_suppress = false;
        let same = chrome.viewport == Some(size)
            && chrome.viewport_cutout == cutout
            && chrome.viewport_home == home_bar;
        if same {
            return None;
        }
        let resized = chrome.viewport != Some(size);
        chrome.viewport = Some(size);
        chrome.viewport_cutout = cutout;
        chrome.viewport_home = home_bar;
        Some(resized)
    });
    let Some(resized) = found else {
        return;
    };
    if resized {
        place_pages(app, window, true);
        release_viewport_emulation(app, window);
    }
    emit_tabs(app);
    apply_device_chrome(app, window);
}

pub fn clear_viewport(app: &AppHandle, window: &str) {
    let found = with_state(app, |state| {
        let chrome = state.chrome_mut(window)?;
        chrome.screencast_suppress = true;
        chrome.screencast_at = Instant::now();
        chrome.viewport.take().map(|_| ())
    });
    if found.is_none() {
        return;
    }
    place_pages(app, window, true);
    emit_tabs(app);
    apply_device_chrome(app, window);
    release_viewport_emulation(app, window);
}

fn device_chrome_script(cutout: bool, home: bool, portrait: bool) -> String {
    format!(
        r#"(function(cutout, home, portrait) {{
  var root = document.getElementById("ce-device-chrome");
  if (!cutout && !home) {{
    if (root) root.remove();
    return;
  }}
  if (!root) {{
    root = document.createElement("div");
    root.id = "ce-device-chrome";
    var parent = document.documentElement || document.body;
    if (!parent) return;
    parent.appendChild(root);
  }}
  root.setAttribute("style", "position:fixed;inset:0;z-index:2147483646;pointer-events:none;");
  var island = "";
  if (cutout) {{
    island = portrait
      ? '<div id="ce-device-cutout" style="position:absolute;top:11px;left:50%;width:min(126px,34vw);height:37px;transform:translateX(-50%);background:#000;border-radius:20px;"></div>'
      : '<div id="ce-device-cutout" style="position:absolute;left:11px;top:50%;width:37px;height:min(126px,34vh);transform:translateY(-50%);background:#000;border-radius:20px;"></div>';
  }}
  var bar = home
    ? '<div id="ce-device-home" style="position:absolute;left:50%;bottom:8px;width:min(134px,36vw);height:5px;transform:translateX(-50%);border-radius:999px;background:rgba(235,235,235,0.92);box-shadow:0 0 0 0.5px rgba(0,0,0,0.45);"></div>'
    : "";
  root.innerHTML = island + bar;
}})({cutout}, {home}, {portrait})"#,
        cutout = cutout,
        home = home,
        portrait = portrait,
    )
}

fn apply_device_chrome(app: &AppHandle, window: &str) {
    let plan = with_state(app, |state| {
        let chrome = state.chrome(window)?;
        let id = chrome.active?;
        let Some((width, height)) = chrome.viewport.filter(|_| !chrome.fullscreen) else {
            return Some((id, false, false, true));
        };
        Some((
            id,
            chrome.viewport_cutout,
            chrome.viewport_home,
            height >= width,
        ))
    });
    let Some((id, cutout, home, portrait)) = plan else {
        return;
    };
    let Some(webview) = page_view(app, id) else {
        return;
    };
    let _ = webview.eval(device_chrome_script(cutout, home, portrait));
}

/// DevTools started a screencast of this tab. The second view is stopped, and the page webview
/// takes that size instead. A burst of frames after the bar was closed does not open it again.
pub fn adopt_screencast(app: &AppHandle, id: u32, width: u32, height: u32) {
    let Some(size) = clamp_viewport(width, height) else {
        return;
    };
    let Some(window) = with_state(app, |state| state.tab_window(id)) else {
        return;
    };
    let apply = with_state(app, |state| {
        let chrome = state.chrome_mut(&window)?;
        let gap = chrome.screencast_at.elapsed();
        chrome.screencast_at = Instant::now();
        if chrome.screencast_suppress && gap < Duration::from_millis(700) {
            return None;
        }
        chrome.screencast_suppress = false;
        if chrome.viewport == Some(size) {
            return None;
        }
        chrome.viewport = Some(size);
        Some(())
    });
    if apply.is_none() {
        return;
    }
    place_pages(app, &window, true);
    emit_tabs(app);
    apply_device_chrome(app, &window);
}

fn release_viewport_emulation(app: &AppHandle, window: &str) {
    let Some(id) = with_state(app, |state| state.active_of(window)) else {
        return;
    };
    let app = app.clone();
    std::thread::spawn(move || {
        let Some(webview) = page_view(&app, id) else {
            return;
        };
        webview2::release_page_emulation(&webview);
    });
}

fn watch_screencast(app: &AppHandle, id: u32) {
    let Some(webview) = page_view(app, id) else {
        return;
    };
    webview2::watch_screencast(app.clone(), id, &webview);
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

/// Marks a tab whose page stopped answering. The tab strip shows that until it recovers.
pub fn set_unresponsive(app: &AppHandle, id: u32, hung: bool) {
    let changed = with_state(app, |state| {
        let tab = state.tab_mut(id)?;
        if tab.info.unresponsive == hung {
            return Some(false);
        }
        tab.info.unresponsive = hung;
        Some(true)
    });
    if changed == Some(true) {
        emit_tabs(app);
    }
}

/// Copies the hung-page set onto each tab. A report older than the hang window drops off.
pub fn refresh_unresponsive(app: &AppHandle) {
    let hung = crate::page_errors::unresponsive_ids();
    let changed = with_state(app, |state| {
        let mut changed = false;
        for tab in &mut state.tabs {
            let next = hung.contains(&tab.info.id);
            if tab.info.unresponsive != next {
                tab.info.unresponsive = next;
                changed = true;
            }
        }
        changed
    });
    if changed {
        emit_tabs(app);
    }
}

/// Loaded tabs that still have a page view, so their renderer memory can be measured.
pub fn awake_ids(app: &AppHandle) -> HashSet<u32> {
    with_state(app, |state| {
        state
            .tabs
            .iter()
            .filter(|tab| tab.info.loaded && !tab.info.sleeping && tab.webview.is_some())
            .map(|tab| tab.info.id)
            .collect()
    })
}

/// A main-frame navigation began. A failure is cleared, so the page view shows again.
/// Runs inside the WebView2 event on the main thread, so moving views is left to a task.
pub fn navigation_started(app: &AppHandle, id: u32) {
    crate::page_errors::clear_unresponsive(id);
    let recovered = with_state(app, |state| {
        let tab = state.tab_mut(id)?;
        tab.info.unresponsive = false;
        tab.info.progress = 10;
        tab.info.failure.take().map(|_| tab.info.window.clone())
    });
    emit_tabs(app);
    if let Some(window) = recovered {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            place_pages(&app, &window, true);
            emit_tabs(&app);
        });
    }
}

/// Moves the load bar forward. It never goes back while one navigation runs.
pub fn set_progress(app: &AppHandle, id: u32, progress: u8) {
    let changed = with_state(app, |state| {
        let tab = state.tab_mut(id)?;
        if tab.info.progress >= progress || tab.info.progress >= 100 {
            return None;
        }
        tab.info.progress = progress;
        Some(())
    });
    if changed.is_some() {
        emit_tabs(app);
    }
}

/// The page view can't show this tab's page. The browser UI draws `kind` in its place.
pub fn set_failure(app: &AppHandle, id: u32, kind: &str, code: &str, detail: String) {
    let window = with_state(app, |state| {
        let tab = state.tab_mut(id)?;
        tab.info.unresponsive = false;
        tab.info.failure = Some(TabFailure {
            kind: kind.to_string(),
            url: tab.info.url.clone(),
            code: code.to_string(),
            detail,
        });
        tab.info.progress = 100;
        tab.info.loading = false;
        Some(tab.info.window.clone())
    });
    if let Some(window) = window {
        crate::thumbnails::forget(id);
        if kind == "crashed" && crate::popup::kind(app, &window).as_deref() == Some("unresponsive")
        {
            crate::popup::close_all(app, &window, true);
        }
        place_pages(app, &window, true);
        emit_tabs(app);
    }
}

pub fn failure_of(app: &AppHandle, id: u32) -> Option<TabFailure> {
    with_state(app, |state| {
        state
            .tabs
            .iter()
            .find(|tab| tab.info.id == id)
            .and_then(|tab| tab.info.failure.clone())
    })
}

/// Asks whether to wait for or end a page that stopped answering. Only the page on screen asks.
pub fn show_unresponsive(app: &AppHandle, id: u32) -> Result<(), String> {
    let window = with_state(app, |state| {
        let window = state.tab_window(id)?;
        state.is_foreground(id).then_some(window)
    });
    let Some(window) = window else {
        return Ok(());
    };
    if crate::popup::kind(app, &window).as_deref() == Some("unresponsive") {
        return Ok(());
    }
    let handle = app
        .get_window(&window)
        .ok_or("The browser window is missing.")?;
    let ui = with_state(app, |state| {
        state.chrome(&window).map(|chrome| chrome.ui.clone())
    })
    .and_then(|label| app.get_webview(&label))
    .ok_or("The browser window is missing.")?;
    let size = handle.inner_size().map_err(|error| error.to_string())?;
    let scale = handle.scale_factor().unwrap_or(1.0);
    crate::popup::open(
        app,
        &ui,
        crate::popup::PopupRequest {
            kind: "unresponsive".into(),
            params: format!("id={id}"),
            anchor: crate::popup::PopupRect {
                x: 0.0,
                y: 0.0,
                width: f64::from(size.width) / scale,
                height: 116.0,
            },
            placement: "bottom-center".into(),
            focus: true,
            child: false,
            min_width: 0.0,
        },
    )
    .map(|_| ())
}

/// Asks for a permission just under the toolbar, over the page that requested it.
pub fn show_permission_prompt(app: &AppHandle, id: u32, params: &str) -> Result<(), String> {
    let window = with_state(app, |state| state.tab_window(id)).ok_or("That tab is not open.")?;
    if crate::popup::kind(app, &window).as_deref() == Some("permission") {
        return Err("open".into());
    }
    let handle = app
        .get_window(&window)
        .ok_or("The browser window is missing.")?;
    let ui = with_state(app, |state| {
        state.chrome(&window).map(|chrome| chrome.ui.clone())
    })
    .and_then(|label| app.get_webview(&label))
    .ok_or("The browser window is missing.")?;
    let (position, _) = page_area(app, &window);
    let scale = handle.scale_factor().unwrap_or(1.0);
    crate::popup::open(
        app,
        &ui,
        crate::popup::PopupRequest {
            kind: "permission".into(),
            params: params.to_string(),
            anchor: crate::popup::PopupRect {
                x: f64::from(position.x) / scale + 16.0,
                y: f64::from(position.y) / scale,
                width: 1.0,
                height: 1.0,
            },
            placement: "bottom-start".into(),
            focus: true,
            child: false,
            min_width: 0.0,
        },
    )
    .map(|_| ())
}

pub fn update_url(app: &AppHandle, id: u32, raw: &str) {
    let url = paths::display_url(app, raw);
    let opening_zoom = page_zoom_for(app, &url);
    let new_site = settings::sleep_site_key(&url);
    let change = with_state(app, |state| {
        let Some(tab) = state.tab_mut(id) else {
            return None;
        };
        if tab.info.url == url {
            return Some((false, None));
        }
        let was_internal = tab.info.url.starts_with(paths::INTERNAL_SCHEME);
        let now_internal = url.starts_with(paths::INTERNAL_SCHEME);
        let site_changed = settings::sleep_site_key(&tab.info.url) != new_site;
        let loading = tab.info.loading;
        tab.title_ready = false;
        tab.info.url = url.clone();
        // A different site opens at its own zoom. Moving around one site keeps Ctrl +/- changes.
        let zoom = match (was_internal, now_internal) {
            (_, true) if !was_internal => Some(1.0),
            (true, false) => Some(opening_zoom),
            (false, false) if site_changed && (tab.info.zoom - opening_zoom).abs() > 0.001 => {
                Some(opening_zoom)
            }
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

pub fn is_ephemeral(app: &AppHandle, id: u32) -> bool {
    with_state(app, |state| {
        state
            .index_of(id)
            .map(|index| state.tabs[index].info.ephemeral)
    })
    .unwrap_or(false)
}

pub fn activate(app: &AppHandle, id: u32) -> Result<(), String> {
    set_active(app, id, true)
}

/// `collapse` replaces the selection with this tab. Shift- and ctrl-clicks keep the group.
fn set_active(app: &AppHandle, id: u32, collapse: bool) -> Result<(), String> {
    if let Some(window) = with_state(app, |state| state.tab_window(id)) {
        crate::peek::close(app, &window);
        let previous = with_state(app, |state| state.active_of(&window));
        if let Some(previous) = previous.filter(|previous| *previous != id) {
            crate::thumbnails::capture_later(app, previous);
        }
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
            tab.info.last_active = now_ms();
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
    devtools_follow(app, window);
    let (position, page_size) = page_rect(app, window);
    let scale = app
        .get_window(window)
        .and_then(|window| window.scale_factor().ok())
        .unwrap_or(1.0);
    let plan = with_state(app, |state| {
        let active = state.active_of(window);
        let chrome = state.chrome(window)?;
        let fullscreen = chrome.fullscreen;
        let viewport = chrome.viewport.filter(|_| !fullscreen);
        let previous = chrome.shown.clone();
        let split = chrome.split.filter(|split| {
            viewport.is_none()
                && !fullscreen
                && active.is_some_and(|id| id == split.left || id == split.right)
        });
        let ratio = split.map(|split| split.ratio).unwrap_or(0.5);
        let dock_ratio = (chrome.devtools.open && chrome.devtools.ready && !fullscreen)
            .then_some(chrome.devtools.ratio);
        let failed = |id: u32| {
            state
                .tabs
                .iter()
                .any(|tab| tab.info.id == id && tab.info.failure.is_some())
        };
        // A tab that failed keeps its view off screen, so the browser UI behind can explain why.
        let painted_view = |id: u32| {
            state
                .tabs
                .iter()
                .find(|tab| tab.info.id == id && tab.painted && tab.info.failure.is_none())
                .and_then(|tab| tab.webview.clone())
        };
        let paired =
            split.and_then(
                |split| match (painted_view(split.left), painted_view(split.right)) {
                    (Some(left), Some(right)) => {
                        Some(vec![(left, Slot::Left), (right, Slot::Right)])
                    }
                    (Some(left), None) if failed(split.right) => Some(vec![(left, Slot::Left)]),
                    (None, Some(right)) if failed(split.left) => Some(vec![(right, Slot::Right)]),
                    (None, None) if failed(split.left) && failed(split.right) => Some(Vec::new()),
                    _ => None,
                },
            );
        let shown = paired.unwrap_or_else(|| {
            if active.is_some_and(failed) {
                return Vec::new();
            }
            let ready = active.and_then(painted_view);
            let fallback = || {
                previous
                    .first()
                    .map(|(label, _)| label.clone())
                    .filter(|label| {
                        state.tabs.iter().any(|tab| {
                            tab.info.window == window
                                && tab.painted
                                && tab.info.failure.is_none()
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
                    tab.info.sleep_saved = 0;
                    wake.push(label.clone());
                }
            }
        }
        if let Some(chrome) = state.chrome_mut(window) {
            chrome.shown = shown.clone();
        }
        Some((shown, parked, changed, ratio, wake, viewport, dock_ratio))
    });
    let Some((shown, parked, changed, ratio, wake, viewport, dock_ratio)) = plan else {
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
    let (content_width, tools_slot) = match dock_ratio {
        Some(dock_ratio) => {
            let pages = (((page_size.width as f64) * dock_ratio).round() as i32 - gap / 2).max(1);
            let tools_x = pages + gap;
            let tools_width = (page_size.width as i32 - tools_x).max(1);
            (pages as u32, Some((tools_x, tools_width as u32)))
        }
        None => (page_size.width, None),
    };
    let left_width = (((content_width as f64) * ratio).round() as i32 - gap / 2).max(1);
    let right_x = left_width + gap;
    let right_width = (content_width as i32 - right_x).max(1);
    // The device bar is drawn above the page whenever tools are docked or a size is set.
    // The tools column stays full height; only the page view moves down.
    let show_bar = dock_ratio.is_some() || viewport.is_some();
    let bar_px = if show_bar {
        (VIEWPORT_BAR * scale).round() as i32
    } else {
        0
    };
    let page_height = (page_size.height as i32 - bar_px).max(1) as u32;
    let fitted = viewport_frame(viewport, content_width, page_size.height, scale);
    for (label, slot) in &shown {
        let Some(webview) = app.get_webview(label) else {
            continue;
        };
        let (x, y, width, height) =
            if let Some((dx, dy, width, height)) = fitted.filter(|_| matches!(slot, Slot::Full)) {
                (dx as i32, dy as i32, width, height)
            } else {
                let (x, width) = match slot {
                    Slot::Full => (0, content_width as i32),
                    Slot::Left => (0, left_width),
                    Slot::Right => (right_x, right_width),
                };
                (x, bar_px, width as u32, page_height)
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
    if let Some(webview) = app.get_webview(&devtools::label(window)) {
        if let Some((x, width)) = tools_slot {
            let _ = webview.set_size(PhysicalSize::new(width, page_size.height));
            let _ = webview.set_position(PhysicalPosition::new(position.x + x, position.y));
        } else {
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
        crate::webview2::forget_early_script(id);
        crate::microphone::forget_tab(app, id);
        crate::hang_guard::end_frozen_page(id);
        crate::page_errors::forget_tab(id);
    }
    let Some(webview) = label.and_then(|label| app.get_webview(&label)) else {
        return;
    };
    let _ = webview.set_position(parked_position());
    let _ = webview.close();
}

/// Puts the keyboard back on `window`'s page after a popup that had it closed.
pub fn refocus_window(app: &AppHandle, window: &str) {
    if app.try_state::<Tabs>().is_some() {
        focus_active(app, window);
    }
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
    let closed = close_tab_inner(app, id)?;
    if let Some(tab) = closed {
        with_state(app, |state| state.push_closed("tab", None, vec![tab]));
        emit_tabs(app);
    }
    Ok(())
}

/// Closes tabs and remembers them as one entry, so they come back together.
/// Returns the entry id when anything worth restoring closed.
pub fn close_many(
    app: &AppHandle,
    ids: &[u32],
    kind: &str,
    group: Option<ClosedGroup>,
) -> Result<Option<u32>, String> {
    let mut closed = Vec::new();
    let mut failure = None;
    for id in ids {
        match close_tab_inner(app, *id) {
            Ok(Some(tab)) => closed.push(tab),
            Ok(None) => {}
            Err(error) => {
                failure = Some(error);
                break;
            }
        }
    }
    let kind = if closed.len() == 1 && group.is_none() {
        "tab"
    } else {
        kind
    };
    let entry = with_state(app, |state| state.push_closed(kind, group, closed));
    if entry.is_some() {
        emit_tabs(app);
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(entry),
    }
}

/// Closes one tab without adding it to the recently closed list. Returns what to remember.
fn close_tab_inner(app: &AppHandle, id: u32) -> Result<Option<ClosedTab>, String> {
    let plan = with_state(app, |state| {
        let window = state.tab_window(id)?;
        let resident = state.resident_bookmarks(&window) == Some(id);
        let discarding = state
            .chrome(&window)
            .is_some_and(|chrome| chrome.discarding);
        Some(resident && !discarding)
    });
    match plan {
        None => return Ok(None),
        Some(true) => {
            shelf_bookmarks(app, id)?;
            return Ok(None);
        }
        Some(false) => {}
    }
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
        crate::webview2::forget_page_scheme(id);
        state.normalize_groups();
        let remembered = (tab.info.url != paths::NEW_TAB && !tab.info.ephemeral && !tab.shelf)
            .then(|| ClosedTab {
                url: tab.info.url.clone(),
                title: tab.info.title.clone(),
                favicon: tab.info.favicon.clone(),
                pinned: tab.info.pinned,
                index: local,
                window: window.clone(),
                workspace: workspace.clone(),
            });
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
            remembered,
        ))
    });
    let Some((label, was_active, next, empty, strip_empty, last_window, window, remembered)) =
        result
    else {
        return Ok(None);
    };
    crate::thumbnails::forget(id);
    if empty || strip_empty {
        retire_webview(app, label);
        if last_window || !empty {
            create_tab_on(app, &window, None, false, None)?;
        } else {
            emit_tabs(app);
        }
        return Ok(remembered);
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
    Ok(remembered)
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
    let mut order: Vec<u32> = targets
        .iter()
        .copied()
        .filter(|id| Some(*id) != active)
        .collect();
    if let Some(id) = active.filter(|id| targets.contains(id)) {
        order.push(id);
    }
    if order.len() <= 1 {
        return match order.first() {
            Some(id) => close_tab(app, *id),
            None => Ok(()),
        };
    }
    let entry = close_many(app, &order, "batch", None)?;
    crate::undo::offer_closed(app, &window, entry);
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
    let window = with_state(app, |state| state.tab_window(id)).unwrap_or_else(|| WINDOW.into());
    let entry = close_many(app, &ids, "batch", None)?;
    crate::undo::offer_closed(app, &window, entry);
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
    let window = with_state(app, |state| state.tab_window(id)).unwrap_or_else(|| WINDOW.into());
    let entry = close_many(app, &ids, "batch", None)?;
    crate::undo::offer_closed(app, &window, entry);
    Ok(())
}

pub fn reopen_closed(app: &AppHandle) -> Result<Option<u32>, String> {
    restore_closed(app, None)
}

/// Toast text for a closed entry, such as `Closed group "Work"` or `Closed 4 tabs`.
pub fn closed_summary(app: &AppHandle, id: u32) -> Option<String> {
    with_state(app, |state| {
        let entry = state.closed.iter().find(|entry| entry.id == id)?;
        let count = entry.tabs.len();
        let tabs = if count == 1 {
            "1 tab".to_string()
        } else {
            format!("{count} tabs")
        };
        Some(match &entry.group {
            Some(group) if !group.name.trim().is_empty() => {
                format!(
                    "Closed group \u{201c}{}\u{201d} ({tabs})",
                    group.name.trim()
                )
            }
            Some(_) => format!("Closed a group of {tabs}"),
            None => format!("Closed {tabs}"),
        })
    })
}

/// Recently closed tabs and groups, newest first.
pub fn recently_closed(app: &AppHandle) -> Vec<ClosedEntry> {
    with_state(app, |state| state.closed.iter().rev().cloned().collect())
}

/// Opens a recently closed entry again. `None` takes the newest. A group comes back as a group.
/// Returns the first tab it opened.
pub fn restore_closed(app: &AppHandle, id: Option<u32>) -> Result<Option<u32>, String> {
    let Some(entry) = with_state(app, |state| state.take_closed(id)) else {
        return Ok(None);
    };
    let windows = browser_labels(app);
    let focused = focused_window(app);
    let mut opened = Vec::new();
    let many = entry.tabs.len() > 1;
    for (position, closed) in entry.tabs.iter().enumerate() {
        let url = resolve(app, &closed.url);
        let window = if windows.iter().any(|label| label == &closed.window) {
            closed.window.clone()
        } else {
            focused.clone()
        };
        let id = open(
            app,
            OpenTab {
                index: Some(closed.index),
                pinned: closed.pinned && entry.group.is_none(),
                title: Some(closed.title.clone()),
                background: many && position > 0,
                lazy: many && position > 0,
                window,
                workspace: closed.workspace.clone(),
                ..OpenTab::new(url)
            },
        )?;
        if closed.favicon.is_some() {
            let favicon = closed.favicon.clone();
            update(app, id, |tab| tab.info.favicon = favicon);
        }
        opened.push(id);
    }
    if let Some(group) = entry.group.filter(|_| !opened.is_empty()) {
        let container = if crate::privacy::known_container(app, &group.container) {
            group.container
        } else {
            String::new()
        };
        with_state(app, |state| {
            let Some(window) = opened.first().and_then(|id| state.tab_window(*id)) else {
                return;
            };
            let id = state.next_group;
            state.next_group += 1;
            state.groups.push(TabGroup {
                id,
                name: group.name,
                color: group_color(&group.color),
                collapsed: false,
                window,
                container: container.clone(),
            });
            for tab in &opened {
                if let Some(tab) = state.tab_mut(*tab) {
                    tab.info.group_id = Some(id);
                }
            }
            state.normalize_groups();
        });
        if !container.is_empty() {
            for id in &opened {
                rebuild_page(app, *id)?;
            }
        }
    }
    emit_tabs(app);
    Ok(opened.first().copied())
}

/// Recently closed entries to keep in the session file.
pub fn closed_for_session(app: &AppHandle) -> Vec<ClosedEntry> {
    with_state(app, |state| state.closed.clone())
}

/// Puts the saved recently closed list back. Ids are renumbered.
pub fn restore_closed_list(app: &AppHandle, entries: Vec<ClosedEntry>) {
    with_state(app, |state| {
        for entry in entries {
            let tabs: Vec<ClosedTab> = entry
                .tabs
                .into_iter()
                .filter(|tab| !tab.url.trim().is_empty())
                .collect();
            let kind = match entry.kind.as_str() {
                "group" if entry.group.is_some() => "group",
                _ if tabs.len() > 1 => "batch",
                _ => "tab",
            };
            let group = if kind == "group" { entry.group } else { None };
            if state.push_closed(kind, group, tabs).is_some()
                && let Some(last) = state.closed.last_mut()
                && entry.closed_at > 0
            {
                last.closed_at = entry.closed_at;
            }
        }
    });
}

/// Runs `script` in every open tab whose page origin is `origin` (`https://example.com`).
pub fn eval_on_origin(app: &AppHandle, origin: &str, script: &str) {
    if origin.is_empty() || script.is_empty() || app.try_state::<Tabs>().is_none() {
        return;
    }
    let labels = with_state(app, |state| {
        state
            .tabs
            .iter()
            .filter(|tab| crate::paths::http_origin(&tab.info.url).as_deref() == Some(origin))
            .filter_map(|tab| tab.webview.clone())
            .collect::<Vec<_>>()
    });
    for label in labels {
        if let Some(webview) = app.get_webview(&label) {
            let _ = webview.eval(script);
        }
    }
}

pub fn page_view(app: &AppHandle, id: u32) -> Option<Webview> {
    let label = with_state(app, |state| {
        state.tab_mut(id).and_then(|tab| tab.webview.clone())
    })?;
    app.get_webview(&label)
}

/// The page view exists, has drawn, and is not asleep or showing a failure, so it can be captured.
pub fn capturable(app: &AppHandle, id: u32) -> bool {
    with_state(app, |state| {
        state.tabs.iter().any(|tab| {
            tab.info.id == id
                && tab.webview.is_some()
                && tab.painted
                && !tab.hidden
                && !tab.info.sleeping
                && tab.info.failure.is_none()
        })
    })
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

struct BookmarksTarget {
    folder: Option<String>,
}

/// `ce://bookmarks` and `ce://bookmarks?folder=...`. Other addresses are `None`.
fn bookmarks_target(url: &str) -> Option<BookmarksTarget> {
    let rest = url.strip_prefix(paths::BOOKMARKS)?;
    if rest.is_empty() {
        return Some(BookmarksTarget { folder: None });
    }
    let query = rest.strip_prefix('?')?;
    let folder = query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        (key == "folder" && !value.is_empty()).then(|| value.to_string())
    });
    Some(BookmarksTarget { folder })
}

/// Loads the window's bookmarks page in the background when it does not have one yet.
/// A bookmarks tab already in the window is adopted, including one restored from the session.
pub fn ensure_bookmarks_host(app: &AppHandle, window: &str) -> Result<(), String> {
    let existing = with_state(app, |state| state.resident_bookmarks(window));
    if let Some(id) = existing {
        let needs_view = with_state(app, |state| {
            state
                .index_of(id)
                .is_some_and(|index| state.tabs[index].webview.is_none())
        });
        if needs_view {
            ensure_webview(app, id)?;
        }
        return Ok(());
    }
    let adopted = with_state(app, |state| {
        state
            .tabs
            .iter()
            .find(|tab| tab.info.window == window && bookmarks_target(&tab.info.url).is_some())
            .map(|tab| tab.info.id)
    });
    if let Some(id) = adopted {
        with_state(app, |state| {
            if let Some(chrome) = state.chrome_mut(window) {
                chrome.bookmarks = Some(id);
            }
        });
        let needs_view = with_state(app, |state| {
            state
                .index_of(id)
                .is_some_and(|index| state.tabs[index].webview.is_none())
        });
        if needs_view {
            ensure_webview(app, id)?;
        }
        return Ok(());
    }
    let url = paths::internal_url(app, "bookmarks");
    let id = open(
        app,
        OpenTab {
            background: true,
            window: window.to_string(),
            keep_bookmarks: true,
            shelf: true,
            ..OpenTab::new(url)
        },
    )?;
    with_state(app, |state| {
        if let Some(chrome) = state.chrome_mut(window) {
            chrome.bookmarks = Some(id);
        }
    });
    Ok(())
}

/// Shows the resident bookmarks page. `folder` selects that folder in the page.
pub fn reveal_bookmarks(
    app: &AppHandle,
    window: &str,
    folder: Option<&str>,
) -> Result<u32, String> {
    ensure_bookmarks_host(app, window)?;
    let id = with_state(app, |state| state.resident_bookmarks(window))
        .ok_or("Bookmarks is not ready.")?;
    let moved = with_state(app, |state| {
        let workspace = state.workspace_of(window);
        let tab = state.tab_mut(id)?;
        let moved = tab.info.workspace != workspace;
        tab.shelf = false;
        tab.info.window = window.to_string();
        tab.info.workspace = workspace;
        if let Some(folder) = folder.filter(|folder| !folder.is_empty()) {
            tab.info.url = format!("{}?folder={folder}", paths::BOOKMARKS);
        }
        Some(moved)
    })
    .unwrap_or(false);
    if moved {
        with_state(app, |state| {
            state.relocate(id, usize::MAX);
        });
    }
    activate(app, id)?;
    show_bookmarks_folder(app, id, folder);
    Ok(id)
}

fn show_bookmarks_folder(app: &AppHandle, id: u32, folder: Option<&str>) {
    let Some(folder) = folder.filter(|folder| !folder.is_empty()) else {
        return;
    };
    let hash = format!("#/bookmarks?folder={folder}");
    let painted = with_state(app, |state| {
        state
            .index_of(id)
            .is_some_and(|index| state.tabs[index].painted)
    });
    let Some(webview) = page_view(app, id) else {
        return;
    };
    if painted {
        let _ = webview.eval(hash_script(&hash));
        return;
    }
    let url = paths::internal_url(app, &format!("bookmarks?folder={folder}"));
    let _ = webview.navigate(url);
}

/// Swaps a painted React page to `hash` without a reload or a history entry.
fn hash_script(hash: &str) -> String {
    let hash = serde_json::to_string(hash).unwrap_or_else(|_| "\"#/\"".to_string());
    format!(
        "history.replaceState(null,\"\",{hash});window.dispatchEvent(new HashChangeEvent(\"hashchange\"));"
    )
}

/// Parks the resident bookmarks page without destroying its webview.
fn shelf_bookmarks(app: &AppHandle, id: u32) -> Result<(), String> {
    let already = with_state(app, |state| {
        state
            .index_of(id)
            .is_some_and(|index| state.tabs[index].shelf)
    });
    if already {
        return Ok(());
    }
    let plan = with_state(app, |state| {
        let (window, workspace, local) = state.local_index(id)?;
        state.end_split_with(id);
        if let Some(tab) = state.tab_mut(id) {
            tab.shelf = true;
            tab.info.group_id = None;
        }
        let was_active = state.active_of(&window) == Some(id);
        if was_active {
            let indexes = state.strip_of(&window, &workspace);
            let next = indexes
                .get(local)
                .or_else(|| {
                    local
                        .checked_sub(1)
                        .and_then(|previous| indexes.get(previous))
                })
                .map(|tab_index| state.tabs[*tab_index].info.id);
            state.set_window_active(&window, next);
        }
        state.forget_tab(id, &window);
        let next = state.active_of(&window);
        let strip_empty = state.strip_indexes(&window).is_empty();
        Some((window, was_active, next, strip_empty))
    });
    let Some((window, was_active, next, strip_empty)) = plan else {
        return Ok(());
    };
    if strip_empty {
        create_tab_on(app, &window, None, false, None)?;
    } else if was_active {
        if let Some(next) = next {
            ensure_webview(app, next)?;
        }
        place_pages(app, &window, false);
        focus_active(app, &window);
    } else {
        place_pages(app, &window, false);
    }
    emit_tabs(app);
    Ok(())
}

/// Opens `url` where the bookmarks tab sits, and parks the bookmarks page again.
fn leave_bookmarks(app: &AppHandle, id: u32, url: Url) -> Result<(), String> {
    let (window, workspace, local) =
        with_state(app, |state| state.local_index(id)).ok_or("That tab is no longer open.")?;
    open(
        app,
        OpenTab {
            index: Some(local),
            window,
            workspace,
            ..OpenTab::new(url)
        },
    )?;
    shelf_bookmarks(app, id)?;
    Ok(())
}

pub fn navigate(app: &AppHandle, id: u32, input: &str) -> Result<(), String> {
    let url = resolve(app, input);
    let display = paths::display_url(app, url.as_str());
    if let Some(target) = bookmarks_target(&display) {
        let window =
            with_state(app, |state| state.tab_window(id)).unwrap_or_else(|| focused_window(app));
        reveal_bookmarks(app, &window, target.folder.as_deref())?;
        return Ok(());
    }
    if with_state(app, |state| state.is_resident_bookmarks(id)) {
        return leave_bookmarks(app, id, url);
    }
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
    let moved = with_state(app, |state| state.relocate(id, to));
    if moved {
        emit_tabs(app);
    }
}

#[derive(Clone)]
enum PinBind {
    /// An open tab already stands for this pin.
    Tab(u32),
    /// Another saved pin already claimed that same address.
    Duplicate,
    /// Nothing open matches, so the pin should be created.
    Missing,
}

/// Pairs each saved pin with an open tab in `window`. See `paths::match_pins`.
fn bind_pins(state: &TabsState, window: &str, pins: &[String]) -> Vec<PinBind> {
    let mut candidates: Vec<&Tab> = state
        .tabs
        .iter()
        .filter(|tab| tab.info.window == window)
        .collect();
    candidates.sort_by_key(|tab| tab.info.id);
    let open: Vec<(&str, bool)> = candidates
        .iter()
        .map(|tab| (tab.info.url.as_str(), tab.info.pinned))
        .collect();
    paths::match_pins(pins, &open)
        .into_iter()
        .map(|found| match found {
            paths::PinMatch::Open(at) => PinBind::Tab(candidates[at].info.id),
            paths::PinMatch::Duplicate => PinBind::Duplicate,
            paths::PinMatch::Missing => PinBind::Missing,
        })
        .collect()
}

fn pinned_local(state: &TabsState, window: &str, workspace: &str, id: u32) -> Option<usize> {
    state.strip_of(window, workspace).iter().position(|global| {
        let tab = &state.tabs[*global];
        tab.info.id == id && tab.info.pinned
    })
}

/// Opens settings pins that were closed, lines the saved pins up in list order, and reloads the ones already open.
pub fn restore_saved_pins(app: &AppHandle, window: &str) -> Result<(), String> {
    let pins = settings::get(app).pinned_tabs;
    let (workspace, bound) = with_state(app, |state| {
        let workspace = state.workspace_of(window);
        let bound = bind_pins(state, window, &pins);
        (workspace, bound)
    });
    let mut next_local = 0usize;
    let mut refresh = Vec::new();
    for (index, pin) in pins.iter().enumerate() {
        match bound.get(index) {
            Some(PinBind::Tab(id)) => {
                let id = *id;
                let local = with_state(app, |state| pinned_local(state, window, &workspace, id));
                if let Some(local) = local {
                    if local != next_local {
                        move_tab(app, id, next_local);
                    }
                    next_local += 1;
                }
                let pinned = with_state(app, |state| {
                    state
                        .tabs
                        .iter()
                        .any(|tab| tab.info.id == id && tab.info.pinned)
                });
                if pinned {
                    refresh.push(id);
                }
            }
            Some(PinBind::Missing) => {
                let url = resolve(app, pin);
                open(
                    app,
                    OpenTab {
                        background: true,
                        index: Some(next_local),
                        pinned: true,
                        window: window.to_string(),
                        workspace: workspace.clone(),
                        ..OpenTab::new(url)
                    },
                )?;
                next_local += 1;
            }
            _ => {}
        }
    }
    for id in refresh {
        refresh_pinned_tab(app, id);
    }
    Ok(())
}

/// Reloads a pinned tab that is already open. One that has no page view yet is loaded instead.
fn refresh_pinned_tab(app: &AppHandle, id: u32) {
    let needs_view = with_state(app, |state| {
        state
            .tabs
            .iter()
            .find(|tab| tab.info.id == id && tab.info.pinned)
            .map(|tab| tab.webview.is_none())
    });
    let Some(needs_view) = needs_view else {
        return;
    };
    if needs_view {
        let _ = ready_pinned(app, id);
        return;
    }
    let _ = ready_pinned(app, id);
    let _ = reload(app, id);
}

impl TabsState {
    fn relocate(&mut self, id: u32, to: usize) -> bool {
        let Some(from) = self.index_of(id) else {
            return false;
        };
        let window = self.tabs[from].info.window.clone();
        let workspace = self.tabs[from].info.workspace.clone();
        let mut tab = self.tabs.remove(from);
        let owned = self.strip_of(&window, &workspace).len();
        let pinned_count = self.pinned_count_of(&window, &workspace);
        let local = if tab.info.pinned {
            to.min(pinned_count)
        } else {
            to.clamp(pinned_count, owned)
        };
        let index = self.global_index_of(&window, &workspace, local);
        if !tab.info.pinned {
            let strip = self.strip_of(&window, &workspace);
            let before = local
                .checked_sub(1)
                .and_then(|at| strip.get(at))
                .and_then(|at| self.tabs[*at].info.group_id);
            let after = strip.get(local).and_then(|at| self.tabs[*at].info.group_id);
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
        self.tabs.insert(index, tab);
        self.normalize_groups();
        true
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
        if pinned {
            ready_pinned(app, id);
        }
        place_pages(app, &window, true);
        emit_tabs(app);
    }
}

/// Wakes every pinned tab that was suspended. A pinned tab with no page view yet loads too, one per
/// call and only while no page is loading: each new page view holds up the window while it is built.
pub fn wake_pinned(app: &AppHandle) {
    let (sleeping, missing) = with_state(app, |state| {
        let sleeping: Vec<u32> = state
            .tabs
            .iter()
            .filter(|tab| tab.info.pinned && tab.webview.is_some())
            .filter(|tab| tab.info.sleeping || tab.hidden)
            .map(|tab| tab.info.id)
            .collect();
        let missing = if state.page_loading() {
            None
        } else {
            state.unloaded_pin(&[])
        };
        (sleeping, missing)
    });
    let mut changed = false;
    for id in sleeping.into_iter().chain(missing) {
        changed |= ready_pinned(app, id);
    }
    if changed {
        emit_tabs(app);
    }
}

/// Restored pinned tabs start without a page view so the window opens at once. They load here in
/// the background, one at a time, after the selected page has had its turn.
fn load_pinned_later(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(PIN_LOAD_DELAY);
        let mut tried = Vec::new();
        loop {
            wait_for_quiet(&app);
            let Some(id) = with_state(&app, |state| state.unloaded_pin(&tried)) else {
                return;
            };
            tried.push(id);
            if ready_pinned(&app, id) {
                emit_tabs(&app);
            }
            std::thread::sleep(PIN_LOAD_GAP);
        }
    });
}

/// Waits while a popup is open, since building a page view closes it, and while a page is still
/// loading, up to `PIN_LOAD_WAIT`.
fn wait_for_quiet(app: &AppHandle) {
    let started = Instant::now();
    loop {
        let popup = crate::popup::any_open(app);
        let loading = with_state(app, |state| state.page_loading());
        if !popup && (!loading || started.elapsed() >= PIN_LOAD_WAIT) {
            return;
        }
        std::thread::sleep(PIN_QUIET_POLL);
    }
}

/// A pinned tab keeps its page view and comes back if it was suspended.
fn ready_pinned(app: &AppHandle, id: u32) -> bool {
    let plan = with_state(app, |state| {
        let tab = state.tab_mut(id)?;
        if !tab.info.pinned {
            return None;
        }
        let needs_view = tab.webview.is_none();
        let sleeping = tab.info.sleeping || tab.hidden;
        if !needs_view && !sleeping {
            return None;
        }
        if sleeping {
            tab.hidden = false;
            tab.info.sleeping = false;
            tab.info.sleep_saved = 0;
        }
        Some((tab.webview.clone(), needs_view, sleeping))
    });
    let Some((label, needs_view, sleeping)) = plan else {
        return false;
    };
    if sleeping {
        if let Some(webview) = label.and_then(|label| app.get_webview(&label)) {
            webview2::wake(&webview);
        }
    }
    if needs_view {
        let _ = ensure_webview(app, id);
    }
    true
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

/// Presses of Ctrl+Tab this close together walk one frozen most-recent-first list, so holding
/// Ctrl and tapping Tab reaches older tabs instead of bouncing between the last two.
const RECENT_CYCLE_WINDOW: Duration = Duration::from_millis(1500);

struct RecentCycle {
    window: String,
    order: Vec<u32>,
    index: usize,
    at: Instant,
}

static RECENT_CYCLE: Mutex<Option<RecentCycle>> = Mutex::new(None);

/// The window's current strip, most recently selected first.
pub fn recent_order(app: &AppHandle, window: &str) -> Vec<u32> {
    with_state(app, |state| {
        let active = state.active_of(window);
        let mut tabs: Vec<(u32, u64)> = state
            .strip_indexes(window)
            .into_iter()
            .map(|index| {
                (
                    state.tabs[index].info.id,
                    state.tabs[index].info.last_active,
                )
            })
            .collect();
        tabs.sort_by(|left, right| {
            (Some(right.0) == active)
                .cmp(&(Some(left.0) == active))
                .then(right.1.cmp(&left.1))
        });
        tabs.into_iter().map(|(id, _)| id).collect()
    })
}

/// Ctrl+Tab in recent-use order without the switcher.
pub fn cycle_recent(app: &AppHandle, step: isize) -> Result<(), String> {
    let window = focused_window(app);
    let target = {
        let mut cycle = RECENT_CYCLE.lock().expect("recent cycle");
        let continuing = cycle.as_ref().is_some_and(|cycle| {
            cycle.window == window && cycle.at.elapsed() < RECENT_CYCLE_WINDOW
        });
        if !continuing {
            *cycle = Some(RecentCycle {
                window: window.clone(),
                order: recent_order(app, &window),
                index: 0,
                at: Instant::now(),
            });
        }
        let cycle = cycle.as_mut().expect("recent cycle");
        let count = cycle.order.len() as isize;
        if count == 0 {
            return Ok(());
        }
        cycle.index = (cycle.index as isize + step).rem_euclid(count) as usize;
        cycle.at = Instant::now();
        cycle.order[cycle.index]
    };
    if with_state(app, |state| state.index_of(target).is_none()) {
        *RECENT_CYCLE.lock().expect("recent cycle") = None;
        return cycle_recent(app, step);
    }
    activate(app, target)
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

/// `change` is `in`, `out` or `reset`. Extension pages stay on Browser UI size.
pub fn zoom(app: &AppHandle, id: u32, change: &str) -> Result<f64, String> {
    let extension_zoom = with_state(app, |state| {
        state
            .tab_mut(id)
            .and_then(|tab| is_extension_url(&tab.info.url).then_some(tab.info.zoom))
    });
    if let Some(zoom) = extension_zoom {
        return Ok(zoom);
    }
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
        Some((next, tab.info.url.clone()))
    })
    .ok_or("That tab is no longer open.")?;
    let (next, url) = next;
    if let Some(webview) = page_view(app, id) {
        webview.set_zoom(next).map_err(|error| error.to_string())?;
    }
    if url.starts_with("http://") || url.starts_with("https://") {
        let percent = (next * 100.0).round() as u32;
        let default_percent = (default_zoom * 100.0).round() as u32;
        let remembered = if change == "reset" || percent == default_percent {
            0
        } else {
            percent
        };
        crate::page_rules::remember_zoom(app, &url, remembered);
        apply_site_zoom(app, &url, next, id);
    }
    emit_tabs(app);
    Ok(next)
}

/// Other open tabs on the same site follow a zoom change.
fn apply_site_zoom(app: &AppHandle, url: &str, zoom: f64, except: u32) {
    let site = settings::sleep_site_key(url);
    if site.is_empty() {
        return;
    }
    let ids = with_state(app, |state| {
        let mut ids = Vec::new();
        for tab in &mut state.tabs {
            if tab.info.id != except
                && settings::sleep_site_key(&tab.info.url) == site
                && (tab.info.zoom - zoom).abs() > 0.001
            {
                tab.info.zoom = zoom;
                ids.push(tab.info.id);
            }
        }
        ids
    });
    for id in ids {
        if let Some(webview) = page_view(app, id) {
            let _ = webview.set_zoom(zoom);
        }
    }
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
            if tab.info.url.starts_with(paths::INTERNAL_SCHEME) || is_extension_url(&tab.info.url) {
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

/// Extension popups, popouts and pages follow Browser UI size. Websites keep their page zoom.
pub fn apply_ui_zoom(app: &AppHandle, previous_percent: u32, next_percent: u32) {
    if previous_percent == next_percent || previous_percent == 0 {
        return;
    }
    let previous = f64::from(previous_percent) / 100.0;
    let next = f64::from(next_percent) / 100.0;
    let ratio = next / previous;
    if app.try_state::<Tabs>().is_some() {
        apply_extension_tab_zoom(app, next);
        crate::popup::apply_ui_zoom(app, next);
    }
    apply_extension_popout_zoom(app, next, ratio);
}

fn apply_extension_tab_zoom(app: &AppHandle, zoom: f64) {
    let ids = with_state(app, |state| {
        let mut ids = Vec::new();
        for tab in &mut state.tabs {
            if !is_extension_url(&tab.info.url) {
                continue;
            }
            tab.info.zoom = zoom;
            ids.push(tab.info.id);
        }
        ids
    });
    if ids.is_empty() {
        return;
    }
    for id in ids {
        if let Some(webview) = page_view(app, id) {
            let _ = webview.set_zoom(zoom);
        }
    }
    emit_tabs(app);
}

fn apply_extension_popout_zoom(app: &AppHandle, zoom: f64, ratio: f64) {
    let min_width = 240.0 * zoom;
    let min_height = 160.0 * zoom;
    for (label, webview) in app.webviews() {
        if !label.starts_with("script-") {
            continue;
        }
        let Ok(url) = webview.url() else {
            continue;
        };
        if !is_extension_url(url.as_str()) {
            continue;
        }
        let Some(window) = app.get_window(&label) else {
            continue;
        };
        let _ = window.set_min_size(Some(LogicalSize::new(min_width, min_height)));
        if let (Ok(size), Ok(scale)) = (window.inner_size(), window.scale_factor()) {
            if scale > f64::EPSILON {
                let width = size.width as f64 / scale * ratio;
                let height = size.height as f64 / scale * ratio;
                let _ = window.set_size(LogicalSize::new(width, height));
            }
        }
        let _ = webview.set_zoom(zoom);
    }
}

pub fn open_devtools(app: &AppHandle, id: u32) {
    let Some(window) = with_state(app, |state| state.tab_window(id)) else {
        return;
    };
    let close = with_state(app, |state| {
        state
            .chrome(&window)
            .is_some_and(|chrome| chrome.devtools.engaged())
    });
    if close {
        close_devtools(app, &window);
        return;
    }
    let Some(epoch) = with_state(app, |state| {
        let devtools = &mut state.chrome_mut(&window)?.devtools;
        devtools.pending = true;
        devtools.ready = false;
        Some(devtools.cancel_work())
    }) else {
        return;
    };
    emit_tabs(app);
    let app = app.clone();
    std::thread::spawn(move || open_devtools_worker(&app, &window, id, epoch));
}

fn close_devtools(app: &AppHandle, window: &str) {
    let label = devtools::label(window);
    with_state(app, |state| {
        let Some(chrome) = state.chrome_mut(window) else {
            return;
        };
        chrome.devtools.cancel_work();
        chrome.devtools.open = false;
        chrome.devtools.pending = false;
        chrome.devtools.ready = false;
        chrome.devtools.tab = None;
        chrome.devtools.page = None;
        chrome.screencast_suppress = true;
        chrome.screencast_at = Instant::now();
        chrome.viewport = None;
    });
    if let Some(webview) = app.get_webview(&label) {
        let _ = webview.close();
    }
    place_pages(app, window, true);
    emit_tabs(app);
    apply_device_chrome(app, window);
    release_viewport_emulation(app, window);
    focus_active(app, window);
}

fn open_devtools_worker(app: &AppHandle, window: &str, id: u32, epoch: u64) {
    if !devtools_epoch_is(app, window, epoch) {
        return;
    }
    let Some(target) = devtools_target(app, window, id) else {
        fail_devtools_open(app, window, epoch, id);
        return;
    };
    let url = match devtools::inspector_for(&target.webview, &target.profile) {
        Ok(url) => url,
        Err(error) => {
            eprintln!("docked devtools: {error}");
            fail_devtools_open(app, window, epoch, id);
            return;
        }
    };
    if !devtools_epoch_is(app, window, epoch) {
        return;
    }
    if let Err(error) = mount_devtools(app, window, &url) {
        eprintln!("docked devtools: {error}");
        fail_devtools_open(app, window, epoch, id);
        return;
    }
    if !devtools_epoch_is(app, window, epoch) {
        if let Some(webview) = app.get_webview(&devtools::label(window)) {
            let _ = webview.close();
        }
        return;
    }
    let watched = target.id;
    let applied = with_state(app, |state| {
        let Some(devtools) = state.chrome_mut(window).map(|chrome| &mut chrome.devtools) else {
            return false;
        };
        if devtools.epoch != epoch {
            return false;
        }
        devtools.pending = false;
        devtools.open = true;
        devtools.ready = true;
        devtools.tab = Some(target.id);
        devtools.page = Some(target.page);
        true
    });
    if !applied {
        if let Some(webview) = app.get_webview(&devtools::label(window)) {
            let _ = webview.close();
        }
        return;
    }
    place_pages(app, window, true);
    emit_tabs(app);
    watch_screencast(app, watched);
    if let Some(webview) = app.get_webview(&devtools::label(window)) {
        let _ = webview.set_focus();
    }
}

fn fail_devtools_open(app: &AppHandle, window: &str, epoch: u64, id: u32) {
    let popout = with_state(app, |state| {
        let Some(devtools) = state.chrome_mut(window).map(|chrome| &mut chrome.devtools) else {
            return false;
        };
        if devtools.epoch != epoch {
            return false;
        }
        devtools.pending = false;
        devtools.open = false;
        true
    });
    if !popout {
        return;
    }
    emit_tabs(app);
    if let Some(webview) = page_view(app, id) {
        webview.open_devtools();
    }
}

struct DevtoolsTarget {
    id: u32,
    webview: Webview,
    profile: std::path::PathBuf,
    page: String,
}

struct FollowJob {
    job: u64,
    epoch: u64,
    tab: u32,
}

enum FollowEnd {
    Ignore,
    Layout,
    Again,
}

fn devtools_target(app: &AppHandle, window: &str, prefer: u32) -> Option<DevtoolsTarget> {
    let id = with_state(app, |state| {
        let in_window = state.tab_window(prefer).as_deref() == Some(window);
        if in_window {
            Some(prefer)
        } else {
            state.active_of(window)
        }
    })?;
    let _ = ensure_webview(app, id);
    let page = with_state(app, |state| {
        state
            .tabs
            .iter()
            .find(|tab| tab.info.id == id)
            .and_then(|tab| tab.webview.clone())
    })?;
    let webview = app.get_webview(&page)?;
    Some(DevtoolsTarget {
        id,
        webview,
        profile: tab_profile_dir(app, id),
        page,
    })
}

fn tab_profile_dir(app: &AppHandle, id: u32) -> std::path::PathBuf {
    let container = with_state(app, |state| {
        let tab = state.tabs.iter().find(|tab| tab.info.id == id)?;
        let group = tab.info.group_id?;
        state
            .groups
            .iter()
            .find(|item| item.id == group)
            .map(|item| item.container.clone())
            .filter(|container| !container.is_empty())
    });
    match container {
        Some(container) => crate::privacy::container_dir(app, &container),
        None => paths::profile_dir(app),
    }
}

fn devtools_epoch_is(app: &AppHandle, window: &str, epoch: u64) -> bool {
    with_state(app, |state| {
        state
            .chrome(window)
            .is_some_and(|chrome| chrome.devtools.epoch == epoch)
    })
}

fn mount_devtools(app: &AppHandle, window: &str, url: &str) -> Result<(), String> {
    let label = devtools::label(window);
    let parsed = Url::parse(url).map_err(|error| error.to_string())?;
    if let Some(webview) = app.get_webview(&label) {
        return webview.navigate(parsed).map_err(|error| error.to_string());
    }
    let host = app
        .get_window(window)
        .ok_or("The browser window is missing.")?;
    let (_, size) = page_rect(app, window);
    let painted_app = app.clone();
    let builder = base_builder(app, &label, WebviewUrl::External(parsed))
        .disable_drag_drop_handler()
        .background_color(Color(24, 24, 27, 255))
        .focused(false)
        .on_new_window(|_url, _features| NewWindowResponse::Deny)
        .on_page_load(move |webview, payload| {
            devtools_painted(
                &painted_app,
                webview.label(),
                matches!(payload.event(), PageLoadEvent::Finished),
                payload.url().as_str(),
            );
        });
    let webview = host
        .add_child(builder, parked_position(), size)
        .map_err(|error| error.to_string())?;
    webview2::attach_devtools_hooks(app, &webview);
    Ok(())
}

fn is_inspector_document(url: &str) -> bool {
    url.contains("/devtools/devtools_app.html") || url.contains("/devtools/inspector.html")
}

fn devtools_painted(app: &AppHandle, label: &str, finished: bool, url: &str) {
    if !finished || !is_inspector_document(url) {
        return;
    }
    let Some(window) = label.strip_prefix(devtools::LABEL_PREFIX) else {
        return;
    };
    let window = window.to_string();
    let show = with_state(app, |state| {
        let devtools = &mut state.chrome_mut(&window)?.devtools;
        if !devtools.engaged() || devtools.ready {
            return None;
        }
        devtools.ready = true;
        Some(())
    });
    if show.is_some() {
        place_pages(app, &window, true);
        emit_tabs(app);
    }
}

/// Points an open dock at the window's active tab when that tab is no longer the one it shows.
fn devtools_follow(app: &AppHandle, window: &str) {
    let Some(job) = with_state(app, |state| {
        let (open, inflight, active, epoch, aimed_tab, aimed_page) = {
            let chrome = state.chrome(window)?;
            (
                chrome.devtools.open,
                chrome.devtools.inflight,
                chrome.active,
                chrome.devtools.epoch,
                chrome.devtools.tab,
                chrome.devtools.page.clone(),
            )
        };
        if !open || inflight {
            return None;
        }
        let active = active?;
        let page = state
            .tabs
            .iter()
            .find(|tab| tab.info.id == active)
            .and_then(|tab| tab.webview.clone())?;
        if aimed_tab == Some(active) && aimed_page.as_deref() == Some(page.as_str()) {
            return None;
        }
        let devtools = &mut state.chrome_mut(window)?.devtools;
        devtools.job = devtools.job.wrapping_add(1);
        devtools.inflight = true;
        Some(FollowJob {
            job: devtools.job,
            epoch,
            tab: active,
        })
    }) else {
        return;
    };
    let app = app.clone();
    let window = window.to_string();
    std::thread::spawn(move || run_devtools_follow(&app, &window, job));
}

fn run_devtools_follow(app: &AppHandle, window: &str, job: FollowJob) {
    let outcome = (|| -> Result<(u32, String), String> {
        if !devtools_job_is(app, window, job.job) {
            return Err("cancelled".into());
        }
        let target = devtools_target(app, window, job.tab).ok_or("This page is not loaded yet.")?;
        let url = devtools::inspector_for(&target.webview, &target.profile)?;
        if !devtools_job_is(app, window, job.job) {
            return Err("cancelled".into());
        }
        mount_devtools(app, window, &url)?;
        if !devtools_job_is(app, window, job.job) {
            if let Some(webview) = app.get_webview(&devtools::label(window)) {
                let _ = webview.close();
            }
            return Err("cancelled".into());
        }
        Ok((target.id, target.page))
    })();
    match outcome {
        Ok(applied) => finish_devtools_follow(app, window, &job, Some(applied)),
        Err(error) => {
            if error != "cancelled" {
                eprintln!("docked devtools: {error}");
            }
            finish_devtools_follow(app, window, &job, None);
        }
    }
}

fn finish_devtools_follow(
    app: &AppHandle,
    window: &str,
    job: &FollowJob,
    applied: Option<(u32, String)>,
) {
    let watched = applied.as_ref().map(|(id, _)| *id);
    let end = with_state(app, |state| {
        let Some(devtools) = state.chrome_mut(window).map(|chrome| &mut chrome.devtools) else {
            return FollowEnd::Ignore;
        };
        if devtools.job != job.job {
            return FollowEnd::Ignore;
        }
        devtools.inflight = false;
        if devtools.epoch != job.epoch {
            return if devtools.open {
                FollowEnd::Again
            } else {
                FollowEnd::Ignore
            };
        }
        if let Some((id, page)) = applied {
            devtools.tab = Some(id);
            devtools.page = Some(page);
            return FollowEnd::Layout;
        }
        FollowEnd::Ignore
    });
    match end {
        FollowEnd::Layout => {
            place_pages(app, window, true);
            if let Some(id) = watched {
                watch_screencast(app, id);
            }
        }
        FollowEnd::Again => devtools_follow(app, window),
        FollowEnd::Ignore => {}
    }
}

fn devtools_job_is(app: &AppHandle, window: &str, job: u64) -> bool {
    with_state(app, |state| {
        state
            .chrome(window)
            .is_some_and(|chrome| chrome.devtools.job == job)
    })
}

fn clamp_devtools_ratio(ratio: f64) -> f64 {
    if ratio.is_finite() {
        ratio.clamp(DEVTOOLS_RATIO_MIN, DEVTOOLS_RATIO_MAX)
    } else {
        DEVTOOLS_RATIO
    }
}

/// Ctrl+Shift+I. A website under the pointer opens its docked tools. The browser chrome and
/// built-in pages open the native tools for the webview being hovered.
pub fn inspect_hovered(app: &AppHandle) {
    match webview2::hovered_view() {
        Some(webview2::HoveredView::Chrome(window)) => open_ui_devtools(app, &window),
        Some(webview2::HoveredView::Page(id)) => {
            if page_is_app(app, id) {
                open_popout_devtools(app, id);
            } else {
                open_devtools(app, id);
            }
        }
        None => {
            if let Some(id) = active_id(app) {
                if page_is_app(app, id) {
                    open_popout_devtools(app, id);
                } else {
                    open_devtools(app, id);
                }
            }
        }
    }
}

fn page_is_app(app: &AppHandle, id: u32) -> bool {
    tab_page_url(app, id).is_some_and(|url| url.starts_with(paths::INTERNAL_SCHEME))
}

fn open_popout_devtools(app: &AppHandle, id: u32) {
    if let Some(webview) = page_view(app, id) {
        webview.open_devtools();
    }
}

/// Developer tools for the browser chrome of `window`, the React UI webview.
pub fn open_ui_devtools(app: &AppHandle, window: &str) {
    let label = with_state(app, |state| {
        state.chrome(window).map(|chrome| chrome.ui.clone())
    });
    if let Some(webview) = label.and_then(|label| app.get_webview(&label)) {
        webview.open_devtools();
    }
}

/// One tab in a window's current strip, for searches across tabs.
pub struct StripTab {
    pub id: u32,
    pub title: String,
    pub url: String,
    pub loaded: bool,
    pub sleeping: bool,
    pub failed: bool,
}

pub fn strip_tabs(app: &AppHandle, window: &str) -> Vec<StripTab> {
    with_state(app, |state| {
        state
            .strip_indexes(window)
            .into_iter()
            .map(|index| {
                let tab = &state.tabs[index];
                StripTab {
                    id: tab.info.id,
                    title: tab.info.title.clone(),
                    url: tab.info.url.clone(),
                    loaded: tab.webview.is_some() && tab.painted,
                    sleeping: tab.info.sleeping || tab.hidden,
                    failed: tab.info.failure.is_some(),
                }
            })
            .collect()
    })
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

/// Active tab in one browser window. `None` when the tab list is busy, so a mouse hook
/// can skip the notch instead of waiting on the UI thread.
pub fn active_in(app: &AppHandle, window: &str) -> Option<u32> {
    let state = app.try_state::<Tabs>()?;
    let guard = state.inner().0.try_lock().ok()?;
    guard.active_of(window)
}

/// Page area insets in CSS pixels: top, left, right, bottom.
/// Fullscreen is the whole window. `None` until the header has been measured, or while the
/// tab list is busy.
pub fn page_insets(app: &AppHandle, window: &str) -> Option<(f64, f64, f64, f64)> {
    let state = app.try_state::<Tabs>()?;
    let guard = state.inner().0.try_lock().ok()?;
    let chrome = guard.chrome(window)?;
    if chrome.fullscreen {
        return Some((0.0, 0.0, 0.0, 0.0));
    }
    if chrome.ui_height < 1.0 {
        return None;
    }
    Some((
        chrome.ui_height,
        chrome.ui_left,
        chrome.ui_right,
        chrome.ui_bottom,
    ))
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
    crate::popup::close_all(app, window_label, false);
    place_toast(app, window_label);
}

/// Window and chrome fill use the theme. A remote page uses the Windows canvas instead.
pub fn apply_page_background(app: &AppHandle) {
    let color = settings::page_color(app);
    if app.try_state::<Tabs>().is_none() {
        return;
    }
    crate::backdrop::apply_all(app);
    let chrome = with_state(app, |state| {
        state
            .windows
            .iter()
            .map(|window| (window.label.clone(), window.ui.clone()))
            .collect::<Vec<_>>()
    });
    for (label, ui) in chrome {
        let surface = if crate::backdrop::active(app, &label) {
            Color(0, 0, 0, 0)
        } else {
            color
        };
        if let Some(window) = app.get_window(&label) {
            let _ = window.set_background_color(Some(surface));
        }
        if let Some(webview) = app.get_webview(&ui) {
            let _ = webview.set_background_color(Some(surface));
        }
    }
    if app.try_state::<Tabs>().is_none() {
        return;
    }
    let pages = with_state(app, |state| {
        state
            .tabs
            .iter()
            .filter_map(|tab| {
                tab.webview
                    .as_ref()
                    .map(|label| (label.clone(), tab.info.url.clone()))
            })
            .collect::<Vec<_>>()
    });
    for (label, url) in pages {
        if let Some(webview) = app.get_webview(&label) {
            let _ = webview.set_background_color(Some(document_color(app, &url)));
        }
    }
}

/// UI and page webviews. A container tab keeps its own WebView2 profile.
pub fn open_surface_views(app: &AppHandle) -> Vec<Webview> {
    if app.try_state::<Tabs>().is_none() {
        return Vec::new();
    }
    let labels = with_state(app, |state| {
        let mut labels = state.page_labels();
        for window in &state.windows {
            labels.push(window.ui.clone());
        }
        labels
    });
    labels
        .into_iter()
        .filter_map(|label| app.get_webview(&label))
        .collect()
}

/// The gap around a document. Remote pages follow Windows. App pages use the theme.
pub fn paint_document_background(app: &AppHandle, id: u32, url: &str) {
    let Some(webview) = page_view(app, id) else {
        return;
    };
    let _ = webview.set_background_color(Some(document_color(app, url)));
}

fn document_color(app: &AppHandle, url: &str) -> Color {
    if remote_document(app, url) {
        if crate::webview2::page_keeps_light_scheme(url) {
            Color(0xff, 0xff, 0xff, 255)
        } else {
            settings::site_canvas(app)
        }
    } else {
        settings::page_color(app)
    }
}

/// `http`, `https`, and `file` pages. `ce://` pages and extensions keep the browser theme.
fn remote_document(app: &AppHandle, raw: &str) -> bool {
    remote_scheme(&paths::display_url(app, raw))
}

fn remote_scheme(shown: &str) -> bool {
    if shown.starts_with(paths::INTERNAL_SCHEME) || shown.starts_with("chrome-extension:") {
        return false;
    }
    let scheme = shown.split(':').next().unwrap_or("").to_ascii_lowercase();
    matches!(scheme.as_str(), "http" | "https" | "file")
}

pub(crate) fn popup_surface_color(app: &AppHandle) -> Color {
    let (r, g, b) = settings::popup_surface(app);
    Color(r, g, b, 255)
}

/// Paints the document the same color as the webview before React mounts.
pub(crate) fn popup_paint_script(color: Color) -> String {
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

// Toasts

const TOAST_WIDTH: f64 = 400.0;
const TOAST_HEIGHT: f64 = 52.0;
const TOAST_MARGIN: f64 = 16.0;

/// Where the toast sits in the page area, in logical pixels. The `toastPosition` setting picks the
/// corner or edge.
fn toast_rect(app: &AppHandle, window_label: &str) -> (LogicalPosition<f64>, LogicalSize<f64>) {
    let (position, size) = page_rect(app, window_label);
    let scale = app
        .get_window(window_label)
        .and_then(|window| window.scale_factor().ok())
        .unwrap_or(1.0);
    let left = position.x as f64 / scale;
    let top = position.y as f64 / scale;
    let width = size.width as f64 / scale;
    let height = size.height as f64 / scale;
    let toast_width = TOAST_WIDTH.min((width - 16.0).max(200.0));
    let placement = settings::get(app).toast_position;
    let (vertical, horizontal) = placement.split_once('-').unwrap_or(("bottom", "right"));
    let x = match horizontal {
        "left" => TOAST_MARGIN,
        "center" => ((width - toast_width) / 2.0).max(8.0),
        _ => (width - toast_width - TOAST_MARGIN).max(8.0),
    };
    let y = match vertical {
        "top" => TOAST_MARGIN,
        _ => (height - TOAST_HEIGHT - TOAST_MARGIN).max(8.0),
    };
    (
        LogicalPosition::new(left + x, top + y),
        LogicalSize::new(toast_width, TOAST_HEIGHT),
    )
}

/// Moves every open toast to where the current settings place it.
pub fn place_all_toasts(app: &AppHandle) {
    if app.try_state::<Tabs>().is_none() {
        return;
    }
    let windows: Vec<String> = with_state(app, |state| {
        state
            .windows
            .iter()
            .map(|chrome| chrome.label.clone())
            .collect()
    });
    for window in windows {
        place_toast(app, &window);
    }
}

/// Shows a toast over the window's pages. A newer toast replaces the old one.
/// `params` is the query string the toast page reads.
pub fn show_toast(app: &AppHandle, window_label: &str, params: &str) -> Result<(), String> {
    close_toast(app, window_label);
    let window = app
        .get_window(window_label)
        .ok_or("The browser window is missing.")?;
    let label = with_state(app, |state| {
        state.next_popup += 1;
        let label = format!("toast-{}", state.next_popup);
        state.chrome_mut(window_label)?.toast = Some(label.clone());
        Some(label)
    })
    .ok_or("The browser window is missing.")?;
    let url = paths::internal_url(app, &format!("popup/toast?{params}"));
    let surface = popup_surface_color(app);
    let (position, size) = toast_rect(app, window_label);
    let builder = base_builder(app, &label, WebviewUrl::External(url))
        .disable_drag_drop_handler()
        .background_color(surface)
        .focused(false)
        .initialization_script(popup_paint_script(surface));
    match window.add_child(builder, position, size) {
        Ok(webview) => {
            if let Ok(main) = window.hwnd() {
                webview2::round_popup(
                    &webview,
                    main.0 as isize,
                    size.width,
                    size.height,
                    10.0,
                    true,
                );
            }
            Ok(())
        }
        Err(error) => {
            with_state(app, |state| {
                if let Some(chrome) = state.chrome_mut(window_label) {
                    chrome.toast = None;
                }
            });
            Err(error.to_string())
        }
    }
}

pub fn close_toast(app: &AppHandle, window_label: &str) {
    let label = with_state(app, |state| state.chrome_mut(window_label)?.toast.take());
    if let Some(webview) = label.and_then(|label| app.get_webview(&label)) {
        let _ = webview.close();
    }
}

fn place_toast(app: &AppHandle, window_label: &str) {
    let label = with_state(app, |state| state.chrome(window_label)?.toast.clone());
    let Some(webview) = label.and_then(|label| app.get_webview(&label)) else {
        return;
    };
    let (position, size) = toast_rect(app, window_label);
    let _ = webview.set_size(size);
    let _ = webview.set_position(position);
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
        let position = |id: u32| -> Option<usize> {
            let index = state.index_of(id)?;
            if state.tabs[index].info.ephemeral || state.tabs[index].shelf {
                return None;
            }
            let skipped = state.tabs[..index]
                .iter()
                .filter(|tab| tab.shelf || tab.info.ephemeral)
                .count();
            Some(index - skipped)
        };
        let tabs = state
            .tabs
            .iter()
            .filter(|tab| !tab.info.ephemeral && !tab.shelf)
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
        let active = state.active_of(WINDOW).and_then(position);
        let split = state.chrome(WINDOW).and_then(|chrome| {
            let split = chrome.split?;
            Some((position(split.left)?, position(split.right)?, split.ratio))
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
                keep_bookmarks: true,
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
    let shown = match target {
        Some(id) => activate(app, id),
        None => create_tab(app, None, false, None).map(|_| ()),
    };
    load_pinned_later(app);
    shown
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
    let heard = with_state(app, |state| {
        let tab = state.tab_mut(id)?;
        if tab.info.audible == audible {
            return None;
        }
        tab.info.audible = audible;
        // The webview can be left silent after a background mute. Starting playback asks again.
        if audible && !tab.info.muted {
            tab.applied_mute = None;
        }
        Some(state.tab_window(id))
    });
    let Some(window) = heard else {
        return;
    };
    if let Some(window) = window {
        apply_audio(app, &window);
    }
    emit_tabs(app);
    // Muted video never raises this. The page script pauses that on its own.
    if audible {
        if let Some(webview) = page_view(app, id) {
            let _ = webview.eval("if (window.__ceStopAutoplay) window.__ceStopAutoplay()");
        }
    }
}

pub fn set_muted(app: &AppHandle, ids: &[u32], muted: bool) -> Result<(), String> {
    let windows = with_state(app, |state| {
        let mut windows = Vec::new();
        for id in ids {
            if let Some(tab) = state.tab_mut(*id) {
                tab.info.muted = muted;
                tab.applied_mute = None;
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
        let residents: Vec<u32> = state
            .windows
            .iter()
            .filter_map(|chrome| chrome.bookmarks)
            .collect();
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
                    && !residents.contains(&tab.info.id)
                    && !tab.info.loading
                    && !tab.info.pinned
                    && tab.last_shown.elapsed() >= after
                    && !sleep.never_sleeps(&tab.info.url)
            })
            .map(|tab| tab.info.id)
            .filter(|id| !state.is_foreground(*id))
            .collect()
    })
}

pub fn sleep_tab(app: &AppHandle, id: u32) -> Result<(), String> {
    let eligible = with_state(app, |state| {
        if state.is_foreground(id) {
            return Err("The tab on screen can't sleep.".to_string());
        }
        let tab = state
            .tabs
            .iter()
            .find(|tab| tab.info.id == id)
            .ok_or("That tab is no longer open.")?;
        let ready = !tab.info.pinned && !tab.info.sleeping && !tab.hidden && tab.webview.is_some();
        Ok(ready && !state.is_resident_bookmarks(id))
    })?;
    if !eligible {
        return Ok(());
    }
    let used = crate::status::tab_memory(app, id).unwrap_or(0);
    let label = with_state(app, |state| {
        if state.is_foreground(id) {
            return None;
        }
        let tab = state.tab_mut(id)?;
        if tab.info.pinned || tab.info.sleeping || tab.hidden {
            return None;
        }
        let label = tab.webview.clone()?;
        tab.hidden = true;
        tab.info.sleep_saved = used;
        Some(label)
    });
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

/// Sleeps every selected tab that is not on screen.
pub fn sleep_selected(app: &AppHandle, window: &str) -> Result<(), String> {
    let ids = with_state(app, |state| {
        state
            .selection_in_order(Some(window))
            .into_iter()
            .filter(|id| !state.is_foreground(*id))
            .collect::<Vec<_>>()
    });
    for id in ids {
        let _ = sleep_tab(app, id);
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
        tab.info.sleep_saved = 0;
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

pub fn set_devtools_ratio(app: &AppHandle, window: &str, ratio: f64) {
    let changed = with_state(app, |state| {
        let devtools = &mut state.chrome_mut(window)?.devtools;
        if !devtools.open {
            return None;
        }
        let ratio = clamp_devtools_ratio(ratio);
        if (devtools.ratio - ratio).abs() < 0.001 {
            return None;
        }
        devtools.ratio = ratio;
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
    if !crate::peek::ignore_page_focus(&window) {
        crate::peek::dismiss(app, &window);
    }
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
        devtools_follow(app, &window);
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
    let (ids, group) = with_state(app, |state| {
        let ids: Vec<u32> = state
            .tabs
            .iter()
            .filter(|tab| tab.info.group_id == Some(id))
            .map(|tab| tab.info.id)
            .collect();
        let group = state
            .groups
            .iter()
            .find(|group| group.id == id)
            .map(|group| {
                (
                    group.window.clone(),
                    ClosedGroup {
                        name: group.name.clone(),
                        color: group.color.clone(),
                        container: group.container.clone(),
                    },
                )
            });
        (ids, group)
    });
    let Some((window, group)) = group else {
        return Err("That group no longer exists.".into());
    };
    let entry = close_many(app, &ids, "group", Some(group))?;
    crate::undo::offer_closed(app, &window, entry);
    Ok(())
}

/// A group's name and the title and address of each of its tabs, for bookmarking.
pub fn group_links(app: &AppHandle, id: u32) -> Result<(String, Vec<(String, String)>), String> {
    with_state(app, |state| {
        let group = state
            .groups
            .iter()
            .find(|group| group.id == id)
            .ok_or("That group no longer exists.")?;
        let links = state
            .tabs
            .iter()
            .filter(|tab| tab.info.group_id == Some(id) && tab.info.url != paths::NEW_TAB)
            .map(|tab| (tab.info.title.clone(), tab.info.url.clone()))
            .collect::<Vec<_>>();
        if links.is_empty() {
            return Err("This group has no pages to save.".to_string());
        }
        Ok((group.name.clone(), links))
    })
}

const MAX_GROUP_OPEN: usize = 50;

/// Opens links as a new group named `name` at the end of the window's strip. The first tab is shown.
pub fn open_as_group(
    app: &AppHandle,
    window: &str,
    name: &str,
    links: &[(String, String)],
) -> Result<u32, String> {
    let links: Vec<&(String, String)> = links
        .iter()
        .filter(|(_, url)| !url.trim().is_empty())
        .take(MAX_GROUP_OPEN)
        .collect();
    if links.is_empty() {
        return Err("That folder has no links to open.".into());
    }
    let mut ids = Vec::new();
    for (position, (title, url)) in links.iter().enumerate() {
        let id = open(
            app,
            OpenTab {
                background: position > 0,
                lazy: position > 0,
                title: Some(title.clone()),
                window: window.to_string(),
                ..OpenTab::new(resolve(app, url))
            },
        )?;
        ids.push(id);
    }
    let group = create_group(app, &ids)?;
    update_group(app, group, Some(name.to_string()), None, None)?;
    Ok(group)
}

/// Groups the window's loose tabs by site. Sites with one tab stay as they are.
/// Returns how many groups were made or grew.
pub fn group_by_site(app: &AppHandle, window: &str) -> Result<usize, String> {
    let plan = with_state(app, |state| {
        let mut by_site: Vec<(String, Vec<u32>)> = Vec::new();
        for index in state.strip_indexes(window) {
            let tab = &state.tabs[index].info;
            if tab.pinned || tab.group_id.is_some() {
                continue;
            }
            let site = paths::site_of(&tab.url);
            if site.is_empty() {
                continue;
            }
            match by_site.iter_mut().find(|(key, _)| *key == site) {
                Some((_, ids)) => ids.push(tab.id),
                None => by_site.push((site, vec![tab.id])),
            }
        }
        let workspace = state.workspace_of(window);
        by_site
            .into_iter()
            .filter_map(|(site, ids)| {
                let existing = state.groups.iter().find(|group| {
                    group.window == window
                        && group.name.eq_ignore_ascii_case(&site)
                        && state.tabs.iter().any(|tab| {
                            tab.info.group_id == Some(group.id) && tab.info.workspace == workspace
                        })
                });
                match existing {
                    Some(group) => Some((site, ids, Some(group.id))),
                    None if ids.len() >= 2 => Some((site, ids, None)),
                    None => None,
                }
            })
            .collect::<Vec<_>>()
    });
    let count = plan.len();
    for (site, ids, existing) in plan {
        match existing {
            Some(group) => add_to_group(app, &ids, group)?,
            None => {
                let group = create_group(app, &ids)?;
                update_group(app, group, Some(site), None, None)?;
            }
        }
    }
    Ok(count)
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
    close_many(app, &ids, "batch", None)?;
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

/// Browser window that shows tab `id`.
pub fn tab_window_label(app: &AppHandle, id: u32) -> Option<String> {
    with_state(app, |state| state.tab_window(id))
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

    #[test]
    fn debugging_port_is_ephemeral_and_local() {
        assert!(WEBVIEW_BROWSER_ARGS.contains("--remote-debugging-port=0"));
        assert!(!WEBVIEW_BROWSER_ARGS.contains("remote-allow-origins"));
    }

    #[test]
    fn devtools_column_stays_wide_enough_to_use() {
        assert_eq!(clamp_devtools_ratio(0.0), DEVTOOLS_RATIO_MIN);
        assert_eq!(clamp_devtools_ratio(1.0), DEVTOOLS_RATIO_MAX);
        assert_eq!(clamp_devtools_ratio(0.5), 0.5);
        assert_eq!(clamp_devtools_ratio(f64::NAN), DEVTOOLS_RATIO);
    }

    #[test]
    fn remote_pages_keep_their_own_canvas() {
        assert!(remote_scheme("https://mail.google.com/"));
        assert!(remote_scheme("http://example.com"));
        assert!(remote_scheme("file:///C:/page.html"));
        assert!(!remote_scheme("ce://settings"));
        assert!(!remote_scheme("chrome-extension://abc/popup.html"));
    }

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
                unresponsive: false,
                accent: None,
                site: String::new(),
                last_active: 0,
                progress: 0,
                failure: None,
                sleep_saved: 0,
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
            shelf: false,
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
            next_closed: 0,
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
    fn bookmarks_urls_and_a_shelved_tab_stay_out_of_the_strip() {
        assert!(bookmarks_target("ce://bookmarks").is_some_and(|target| target.folder.is_none()));
        assert_eq!(
            bookmarks_target("ce://bookmarks?folder=bar")
                .and_then(|target| target.folder)
                .as_deref(),
            Some("bar")
        );
        assert!(bookmarks_target("ce://history").is_none());
        assert!(bookmarks_target("https://example.com").is_none());

        let mut shelved = tab(1, None, DEFAULT_WORKSPACE);
        shelved.shelf = true;
        shelved.info.url = "ce://bookmarks".into();
        let visible = tab(2, None, DEFAULT_WORKSPACE);
        let mut state = state(vec![shelved, visible], &[]);
        state.windows[0].bookmarks = Some(1);
        let strip = state.strip_of(WINDOW, DEFAULT_WORKSPACE);
        assert_eq!(strip, vec![1]);
        assert_eq!(state.tabs[strip[0]].info.id, 2);
        assert!(state.is_resident_bookmarks(1));
        assert!(!state.is_resident_bookmarks(2));
        let shown: Vec<u32> = state.snapshot().tabs.iter().map(|tab| tab.id).collect();
        assert_eq!(shown, vec![2]);
    }

    fn insert_pinned(state: &mut TabsState, url: &str, local: usize) {
        let mut tab = tab(state.next_id, None, DEFAULT_WORKSPACE);
        state.next_id += 1;
        tab.info.url = url.to_string();
        tab.info.pinned = true;
        let pinned_count = state.pinned_count_of(WINDOW, DEFAULT_WORKSPACE);
        let at = local.min(pinned_count);
        let index = state.global_index_of(WINDOW, DEFAULT_WORKSPACE, at);
        state.tabs.insert(index, tab);
    }

    /// Same steps as `restore_saved_pins`, with new tabs inserted instead of webviews.
    fn replay_pin_restore(state: &mut TabsState, pins: &[&str]) {
        let workspace = DEFAULT_WORKSPACE.to_string();
        let saved: Vec<String> = pins.iter().map(|pin| (*pin).to_string()).collect();
        let bound = bind_pins(state, WINDOW, &saved);
        let mut next_local = 0usize;
        for (index, pin) in saved.iter().enumerate() {
            match bound.get(index) {
                Some(PinBind::Tab(id)) => {
                    let id = *id;
                    if let Some(local) = pinned_local(state, WINDOW, &workspace, id) {
                        if local != next_local {
                            state.relocate(id, next_local);
                        }
                        next_local += 1;
                    }
                }
                Some(PinBind::Missing) => {
                    insert_pinned(state, pin, next_local);
                    next_local += 1;
                }
                _ => {}
            }
        }
    }

    fn pinned_url(url: &str, id: u32) -> Tab {
        let mut tab = tab(id, None, DEFAULT_WORKSPACE);
        tab.info.url = url.to_string();
        tab.info.pinned = true;
        tab
    }

    #[test]
    fn closed_saved_pins_reopen_in_settings_order() {
        let mut loose = tab(4, None, DEFAULT_WORKSPACE);
        loose.info.url = "https://loose.example".into();
        let mut state = state(
            vec![
                pinned_url("https://c.example/", 2),
                pinned_url("https://other.example", 3),
                pinned_url("https://a.example", 1),
                loose,
            ],
            &[],
        );
        replay_pin_restore(
            &mut state,
            &[
                "https://a.example",
                "https://b.example",
                "https://c.example",
            ],
        );
        let urls: Vec<String> = state.tabs.iter().map(|tab| tab.info.url.clone()).collect();
        assert_eq!(
            urls,
            vec![
                "https://a.example",
                "https://b.example",
                "https://c.example/",
                "https://other.example",
                "https://loose.example",
            ]
        );
        assert!(state.tabs[..4].iter().all(|tab| tab.info.pinned));
        assert!(!state.tabs[4].info.pinned);
    }

    #[test]
    fn an_open_saved_pin_is_not_opened_again() {
        let mut open = tab(1, None, DEFAULT_WORKSPACE);
        open.info.url = "https://a.example".into();
        let mut elsewhere = tab(2, None, "work");
        elsewhere.info.url = "https://b.example".into();
        elsewhere.info.pinned = true;
        let mut state = state(vec![open, elsewhere], &[]);
        replay_pin_restore(&mut state, &["https://a.example/", "https://b.example"]);
        assert_eq!(state.tabs.len(), 2);
        assert!(!state.tabs[0].info.pinned);
        assert_eq!(state.tabs[0].info.url, "https://a.example");
    }

    #[test]
    fn restored_pins_load_in_strip_order_and_skip_failures() {
        let mut loaded = pinned_url("https://a.example", 1);
        loaded.webview = Some("tab-1".into());
        let mut loose = tab(4, None, DEFAULT_WORKSPACE);
        loose.info.url = "https://loose.example".into();
        let state = state(
            vec![
                loaded,
                pinned_url("https://b.example", 2),
                pinned_url("https://c.example", 3),
                loose,
            ],
            &[],
        );
        assert_eq!(state.unloaded_pin(&[]), Some(2));
        assert_eq!(state.unloaded_pin(&[2]), Some(3));
        assert_eq!(state.unloaded_pin(&[2, 3]), None);
        assert!(!state.page_loading());
    }

    #[test]
    fn an_open_pin_that_navigated_is_not_opened_again() {
        let mut state = state(
            vec![
                pinned_url("https://www.a.example/inbox", 1),
                pinned_url("https://b.example/mail/u/0", 2),
            ],
            &[],
        );
        replay_pin_restore(
            &mut state,
            &[
                "https://a.example",
                "https://a.example/",
                "https://b.example/mail",
            ],
        );
        assert_eq!(state.tabs.len(), 2);
        assert_eq!(state.tabs[0].info.url, "https://www.a.example/inbox");
        assert_eq!(state.tabs[1].info.url, "https://b.example/mail/u/0");
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

    #[test]
    fn the_closed_list_keeps_the_newest_entries() {
        let mut state = state(Vec::new(), &[]);
        for number in 0..(MAX_CLOSED + 5) {
            state.push_closed(
                "tab",
                None,
                vec![ClosedTab {
                    url: format!("https://{number}.example"),
                    ..ClosedTab::default()
                }],
            );
        }
        assert_eq!(state.closed.len(), MAX_CLOSED);
        assert_eq!(state.closed[0].tabs[0].url, "https://5.example");
        let newest = state.take_closed(None).expect("an entry");
        assert_eq!(
            newest.tabs[0].url,
            format!("https://{}.example", MAX_CLOSED + 4)
        );
        assert!(state.push_closed("tab", None, Vec::new()).is_none());
    }

    #[test]
    fn an_older_closed_entry_without_new_keys_still_loads() {
        let entry: ClosedEntry =
            serde_json::from_str(r#"{"kind":"tab","tabs":[{"url":"https://a.example"}]}"#)
                .expect("closed entry");
        assert_eq!(entry.tabs[0].url, "https://a.example");
        assert!(entry.group.is_none());
        assert_eq!(entry.tabs[0].workspace, "");
    }
}
