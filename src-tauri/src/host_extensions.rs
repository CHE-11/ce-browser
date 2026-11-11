//! Host extensions: folders of HTML, CSS, and JavaScript the browser loads at runtime.
//!
//! Each enabled package is served at `https://<id>.ce-ext.localhost/`. Its pages call `window.ce`.
//! That object posts to the host, which checks the calling page and the granted permissions
//! before touching tabs, bookmarks, downloads, or the desktop.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tauri::webview::Color;
use tauri::window::WindowBuilder;
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, PhysicalPosition, Webview,
    WebviewUrl,
};
use url::Url;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_HOST_RESOURCE_ACCESS_KIND_DENY_CORS, COREWEBVIEW2_WEB_ERROR_STATUS, ICoreWebView2,
    ICoreWebView2_3,
};
use webview2_com::{
    AddScriptToExecuteOnDocumentCreatedCompletedHandler, ExecuteScriptCompletedHandler,
    NavigationCompletedEventHandler, WebMessageReceivedEventHandler,
};
use windows::core::{HSTRING, Interface};

use crate::{bookmarks, downloads, files, paths, settings, tabs, undo, webview2};

const FILE: &str = "host-extensions.json";
const HOST_SUFFIX: &str = ".ce-ext.localhost";
const PERMISSIONS: &[&str] = &[
    "tabs",
    "page",
    "desktop",
    "bookmarks",
    "downloads",
    "storage",
];
const MAX_SCRIPT: usize = 1024 * 1024;
const MAX_ICON: usize = 512 * 1024;
const MAX_MESSAGE: usize = 64 * 1024;
const MAX_STORAGE_VALUE: usize = 256 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct HostRecord {
    id: String,
    enabled: bool,
    granted: Vec<String>,
    /// `left`, `right`, or empty to use the manifest. Size and whether the panel
    /// can be resized come from the manifest, not this record.
    panel_side: String,
    /// Last dragged width. Zero uses the manifest default width.
    panel_width: u32,
    /// `top`, `right`, `bottom`, `left`, or empty to use the manifest.
    toolbar_side: String,
    /// The page toolbar is showing. Missing in older files, which stay open.
    #[serde(default = "default_true")]
    toolbar_open: bool,
}

fn default_true() -> bool {
    true
}

impl Default for HostRecord {
    fn default() -> Self {
        Self {
            id: String::new(),
            enabled: false,
            granted: Vec::new(),
            panel_side: String::new(),
            panel_width: 0,
            toolbar_side: String::new(),
            toolbar_open: true,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct HostFile {
    extensions: Vec<HostRecord>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    #[serde(default)]
    manifest_version: u32,
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    permissions: Vec<String>,
    #[serde(default)]
    icon: String,
    #[serde(default)]
    background: String,
    #[serde(default)]
    toolbar: Option<ToolbarManifest>,
    #[serde(default)]
    panel: PanelSpec,
    #[serde(default)]
    popout: Option<PopoutSpec>,
    #[serde(default)]
    pages: Vec<PageManifest>,
    #[serde(default)]
    bookmarks_bar: String,
    /// A strip around the page. A string is the HTML path. An object can also set the starting edge and thickness.
    #[serde(default)]
    page_toolbar: PageToolbarSpec,
    #[serde(default)]
    downloads: String,
    #[serde(default)]
    content_scripts: Vec<ContentManifest>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum PageToolbarSpec {
    Path(String),
    Detail(PageToolbarDetail),
}

impl Default for PageToolbarSpec {
    fn default() -> Self {
        Self::Path(String::new())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct PageToolbarDetail {
    path: String,
    /// `top`, `right`, `bottom`, or `left`. Empty starts on the bottom edge.
    side: String,
    /// CSS pixels. Height on the top or bottom, width on the left or right.
    thickness: u32,
}

impl Default for PageToolbarDetail {
    fn default() -> Self {
        Self {
            path: String::new(),
            side: String::new(),
            thickness: 0,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolbarManifest {
    #[serde(default)]
    title: String,
    #[serde(default)]
    popup: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum PanelSpec {
    Path(String),
    Detail(PanelDetail),
}

impl Default for PanelSpec {
    fn default() -> Self {
        Self::Path(String::new())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct PanelDetail {
    path: String,
    side: String,
    resizable: bool,
    min_width: u32,
    max_width: u32,
    default_width: u32,
}

impl Default for PanelDetail {
    fn default() -> Self {
        Self {
            path: String::new(),
            side: String::new(),
            resizable: false,
            min_width: 0,
            max_width: 0,
            default_width: 0,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum PopoutSpec {
    Path(String),
    Detail(PopoutDetail),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct PopoutDetail {
    path: String,
    title: String,
    width: u32,
    height: u32,
    min_width: u32,
    min_height: u32,
    resizable: bool,
}

impl Default for PopoutDetail {
    fn default() -> Self {
        Self {
            path: String::new(),
            title: String::new(),
            width: 480,
            height: 640,
            min_width: 320,
            min_height: 240,
            resizable: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageManifest {
    #[serde(default)]
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContentManifest {
    #[serde(default)]
    matches: Vec<String>,
    #[serde(default)]
    js: Vec<String>,
    #[serde(default)]
    css: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostPageInfo {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostSurfaces {
    pub toolbar: bool,
    pub popup: bool,
    pub panel: bool,
    pub pages: Vec<HostPageInfo>,
    pub bookmarks_bar: bool,
    pub downloads: bool,
    pub content_scripts: bool,
    pub background: bool,
    pub popout: bool,
    pub page_toolbar: bool,
}

/// Where an extension's page toolbar sits, and how thick it is in CSS pixels.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostPageToolbar {
    pub side: String,
    pub thickness: u32,
    /// False after the toolbar's own close control hides it. The extension stays enabled.
    pub open: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostPanelLayout {
    pub side: String,
    pub resizable: bool,
    pub min_width: u32,
    pub max_width: u32,
    pub default_width: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostExtensionInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub enabled: bool,
    pub icon: Option<String>,
    pub permissions: Vec<String>,
    pub surfaces: HostSurfaces,
    /// The side the panel opens on, and the size declared in the manifest. Empty when the extension has no panel.
    pub panel: Option<HostPanelLayout>,
    /// The edge and thickness of the page toolbar. Empty when the extension has none.
    pub page_toolbar: Option<HostPageToolbar>,
}

struct Loaded {
    record: HostRecord,
    manifest: Manifest,
    root: PathBuf,
}

#[derive(Default)]
struct ScriptSlot {
    script_id: Option<String>,
    stamp: Option<String>,
    inflight: bool,
}

struct HostState {
    records: Vec<HostRecord>,
}

pub struct HostExtensions(Mutex<HostState>);

fn state(app: &AppHandle) -> Vec<HostRecord> {
    app.state::<HostExtensions>()
        .0
        .lock()
        .expect("host extensions lock")
        .records
        .clone()
}

fn save_records(app: &AppHandle, records: Vec<HostRecord>) -> Result<(), String> {
    {
        let host = app.state::<HostExtensions>();
        host.0.lock().expect("host extensions lock").records = records.clone();
    }
    invalidate_content_cache();
    toolbar_generation().fetch_add(1, Ordering::AcqRel);
    crate::sync_conflicts::store(
        app,
        &paths::data_file(app, FILE),
        &HostFile {
            extensions: records,
        },
    )?;
    emit(app);
    Ok(())
}

fn emit(app: &AppHandle) {
    let _ = app.emit("host-extensions-changed", list(app));
}

pub fn init(app: &AppHandle) {
    crate::sync_conflicts::note(app, &paths::data_file(app, FILE));
    let file: HostFile = crate::sync_conflicts::read_json(app, &paths::data_file(app, FILE));
    app.manage(HostExtensions(Mutex::new(HostState {
        records: file.extensions,
    })));
}

/// Starts background pages after the browser window exists. `add_child` waits on the main
/// thread, so this must not run on that thread.
pub fn start(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        reconcile_permissions(&app);
        sync_backgrounds(&app);
        sync_page_toolbars(&app);
        refresh_webviews(&app);
    });
}

pub fn adopt_saved(app: &AppHandle) {
    let file: HostFile = crate::sync_conflicts::read_json(app, &paths::data_file(app, FILE));
    {
        let host = app.state::<HostExtensions>();
        host.0.lock().expect("host extensions lock").records = file.extensions;
    }
    invalidate_content_cache();
    emit(app);
    let app = app.clone();
    std::thread::spawn(move || {
        reconcile_permissions(&app);
        sync_backgrounds(&app);
        sync_page_toolbars(&app);
        refresh_webviews(&app);
    });
}

fn reconcile_permissions(app: &AppHandle) {
    let mut records = state(app);
    let mut changed = false;
    for record in &mut records {
        let Some(loaded) = load_record(app, record) else {
            if record.enabled {
                record.enabled = false;
                changed = true;
            }
            continue;
        };
        if record.enabled && !grants_cover(&record.granted, &loaded.manifest.permissions) {
            record.enabled = false;
            changed = true;
        }
    }
    if changed {
        let _ = save_records(app, records);
    }
}

pub fn list(app: &AppHandle) -> Vec<HostExtensionInfo> {
    state(app)
        .iter()
        .filter_map(|record| info_of(app, record))
        .collect()
}

fn info_of(app: &AppHandle, record: &HostRecord) -> Option<HostExtensionInfo> {
    let loaded = load_record(app, record)?;
    let manifest = &loaded.manifest;
    let enabled = record.enabled && grants_cover(&record.granted, &manifest.permissions);
    let panel_path = manifest.panel_path();
    let has_panel = !panel_path.is_empty();
    let has_popout = resolve_popout(manifest).is_some();
    let layout = has_panel.then(|| publish_panel(&resolve_panel(record, manifest)));
    let page_toolbar = resolve_page_toolbar(record, manifest).map(|toolbar| HostPageToolbar {
        side: toolbar.side,
        thickness: toolbar.thickness.round() as u32,
        open: record.toolbar_open,
    });
    Some(HostExtensionInfo {
        id: record.id.clone(),
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        description: manifest.description.clone(),
        enabled,
        icon: icon_data(&loaded.root, &manifest.icon),
        permissions: manifest.permissions.clone(),
        surfaces: HostSurfaces {
            toolbar: manifest
                .toolbar
                .as_ref()
                .is_some_and(|toolbar| !toolbar.popup.is_empty() || has_panel || has_popout),
            popup: manifest
                .toolbar
                .as_ref()
                .is_some_and(|toolbar| !toolbar.popup.is_empty()),
            panel: has_panel,
            pages: manifest
                .pages
                .iter()
                .map(|page| HostPageInfo {
                    id: page.id.clone(),
                    title: if page.title.is_empty() {
                        page.id.clone()
                    } else {
                        page.title.clone()
                    },
                })
                .collect(),
            bookmarks_bar: !manifest.bookmarks_bar.is_empty(),
            downloads: !manifest.downloads.is_empty(),
            content_scripts: !manifest.content_scripts.is_empty(),
            background: !manifest.background.is_empty(),
            popout: has_popout,
            page_toolbar: page_toolbar.is_some(),
        },
        panel: layout,
        page_toolbar,
    })
}

fn load_record(app: &AppHandle, record: &HostRecord) -> Option<Loaded> {
    let root = package_dir(app, &record.id);
    let manifest = read_manifest(&root).ok()?;
    if manifest.id != record.id {
        return None;
    }
    Some(Loaded {
        record: record.clone(),
        manifest,
        root,
    })
}

fn package_dir(app: &AppHandle, id: &str) -> PathBuf {
    paths::data_dir(app).join("host-extensions").join(id)
}

fn storage_path(app: &AppHandle, id: &str) -> PathBuf {
    paths::data_dir(app)
        .join("host-storage")
        .join(format!("{id}.json"))
}

pub fn install_folder(app: &AppHandle, path: &Path) -> Result<HostExtensionInfo, String> {
    let manifest = read_manifest(path)?;
    validate_manifest(&manifest)?;
    let root = package_dir(app, &manifest.id);
    if same_dir(path, &root)? {
        return Err("That folder is already installed.".into());
    }
    if root.exists() {
        fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
    }
    files::copy_dir(path, &root)?;
    let mut records = state(app);
    let previous = records
        .iter()
        .find(|record| record.id == manifest.id)
        .cloned();
    let enabled = match &previous {
        None => true,
        Some(record) => record.enabled && grants_cover(&record.granted, &manifest.permissions),
    };
    records.retain(|record| record.id != manifest.id);
    let mut next = HostRecord {
        id: manifest.id.clone(),
        enabled,
        granted: manifest.permissions.clone(),
        ..HostRecord::default()
    };
    if let Some(previous) = previous {
        next.panel_side = previous.panel_side;
        next.panel_width = previous.panel_width;
        next.toolbar_side = previous.toolbar_side;
        next.toolbar_open = previous.toolbar_open;
    }
    records.push(next);
    save_records(app, records)?;
    refresh_after_change(app);
    list(app)
        .into_iter()
        .find(|item| item.id == manifest.id)
        .ok_or_else(|| "That extension could not be read after install.".into())
}

pub fn set_enabled(app: &AppHandle, id: &str, enabled: bool) -> Result<(), String> {
    let id = canonical_id(id)?;
    let mut records = state(app);
    let record = records
        .iter_mut()
        .find(|record| record.id == id)
        .ok_or("That extension is not installed.")?;
    if enabled {
        let loaded = load_record(app, record).ok_or("That extension folder could not be read.")?;
        if !grants_cover(&record.granted, &loaded.manifest.permissions) {
            record.granted = loaded.manifest.permissions.clone();
        }
    }
    record.enabled = enabled;
    save_records(app, records)?;
    if !enabled {
        clear_slots(app, &id);
        close_background(app, &id);
        close_popout(app, &id);
        close_panels_for(app, &id);
        close_page_toolbars(app, &id);
    }
    refresh_after_change(app);
    Ok(())
}

pub fn remove(app: &AppHandle, id: &str) -> Result<(), String> {
    let id = canonical_id(id)?;
    let mut records = state(app);
    if !records.iter().any(|record| record.id == id) {
        return Err("That extension is not installed.".into());
    }
    records.retain(|record| record.id != id);
    save_records(app, records)?;
    clear_slots(app, &id);
    close_background(app, &id);
    close_popout(app, &id);
    close_panels_for(app, &id);
    close_page_toolbars(app, &id);
    let root = package_dir(app, &id);
    if root.exists() {
        fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
    }
    let storage = storage_path(app, &id);
    if storage.exists() {
        let _ = fs::remove_file(&storage);
    }
    refresh_after_change(app);
    Ok(())
}

fn clear_slots(app: &AppHandle, id: &str) {
    let _ = settings::update(app, |settings| {
        if settings.host_bookmarks_bar == id {
            settings.host_bookmarks_bar.clear();
        }
        if settings.host_downloads == id {
            settings.host_downloads.clear();
        }
    });
}

fn refresh_after_change(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        sync_backgrounds(&app);
        sync_page_toolbars(&app);
        refresh_webviews(&app);
    });
}

pub fn popup_url(app: &AppHandle, params: &str) -> Result<Url, String> {
    let id = canonical_id(params.trim().trim_start_matches("id=").trim())?;
    let loaded = require_enabled(app, &id)?;
    let popup = loaded
        .manifest
        .toolbar
        .as_ref()
        .map(|toolbar| toolbar.popup.as_str())
        .filter(|popup| !popup.is_empty())
        .ok_or("That extension has no toolbar popup.")?;
    file_url(&id, popup)
}

pub fn downloads_url(app: &AppHandle) -> Option<Url> {
    let id = settings::get(app).host_downloads;
    if id.is_empty() {
        return None;
    }
    let loaded = require_enabled(app, &id).ok()?;
    if loaded.manifest.downloads.is_empty() {
        return None;
    }
    file_url(&id, &loaded.manifest.downloads).ok()
}

/// `ce://ext/<id>/<page>` and `ce://downloads` when a host extension owns downloads.
pub fn redirect_internal(app: &AppHandle, page: &str) -> Option<Url> {
    let bare = page
        .split(['?', '#'])
        .next()
        .unwrap_or(page)
        .trim_matches('/');
    if bare == "downloads" {
        return downloads_url(app);
    }
    let rest = bare.strip_prefix("ext/")?;
    let (id, target) = rest.split_once('/')?;
    if !valid_id(id) {
        return None;
    }
    page_target(app, id, target).ok()
}

/// Address bar text for a host-extension document.
pub fn display_for(app: &AppHandle, raw: &str) -> Option<String> {
    let url = Url::parse(raw).ok()?;
    let id = virtual_id(&url)?;
    if !state(app).iter().any(|record| record.id == id) {
        return None;
    }
    let path = url.path().trim_start_matches('/');
    if let Some(loaded) = require_enabled(app, &id).ok()
        && settings::get(app).host_downloads == id
        && !loaded.manifest.downloads.is_empty()
        && path == loaded.manifest.downloads.trim_start_matches('/')
    {
        return Some("ce://downloads".into());
    }
    Some(format!("ce://ext/{id}/{path}"))
}

fn page_target(app: &AppHandle, id: &str, target: &str) -> Result<Url, String> {
    let loaded = require_enabled(app, id)?;
    if let Some(page) = loaded.manifest.pages.iter().find(|page| page.id == target) {
        return file_url(id, &page.path);
    }
    file_url(id, target)
}

pub fn toggle_panel(app: &AppHandle, window: &str, id: &str) -> Result<bool, String> {
    let id = canonical_id(id.trim().trim_start_matches("host:"))?;
    let loaded = require_enabled(app, &id)?;
    let panel_path = loaded.manifest.panel_path();
    if panel_path.is_empty() {
        return Err("That extension has no side panel.".into());
    }
    let showing = tabs::side_panel(app, window) == tabs::SidePanel::Host
        && tabs::host_panel_extension(app, window) == id
        && tabs::agent_panel_width(app, window) > 0.0;
    if showing {
        tabs::set_side_panel(app, window, tabs::SidePanel::None, 0.0);
        return Ok(false);
    }
    let layout = resolve_panel(&loaded.record, &loaded.manifest);
    let url = file_url(&id, panel_path)?;
    let label = tabs::host_panel_label(window);
    eprintln!("panel: open host extension {id} in {window} at {url}");
    ensure_child(app, window, &label, url.clone()).map_err(|error| {
        eprintln!("panel: {label} was not created: {error}");
        error
    })?;
    remember_panel_target(&label, url.as_str(), true);
    let wanted = if layout.resizable && loaded.record.panel_width > 0 {
        f64::from(loaded.record.panel_width)
    } else {
        f64::from(layout.default_width)
    };
    let width = fitted_panel_width(app, window, wanted, &layout);
    // Place the column before navigating. The extension lays out against the size it
    // first sees, and a navigation started while the view is still parked often loses
    // to the initial about:blank load.
    tabs::open_host_panel(app, window, &id, width, layout.edge);
    reveal_panel_document(app, &label, &url);
    post_panel_chrome(app, window, &layout);
    if let Some(view) = app.get_webview(&label) {
        let _ = view.set_focus();
    }
    Ok(true)
}

pub fn place_bookmarks_bar(app: &AppHandle, window: &str, x: f64, y: f64, width: f64, height: f64) {
    let label = format!("cext-bar-{window}");
    let Some(view) = bookmarks_view(app, window) else {
        if let Some(view) = app.get_webview(&label) {
            park(&view);
        }
        return;
    };
    if width < 1.0 || height < 1.0 {
        park(&view);
        return;
    }
    let scale = app
        .get_window(window)
        .and_then(|host| host.scale_factor().ok())
        .unwrap_or(1.0);
    let _ = view.set_position(tauri::PhysicalPosition::new(
        (x * scale).round() as i32,
        (y * scale).round() as i32,
    ));
    let _ = view.set_size(tauri::PhysicalSize::new(
        ((width * scale).round() as u32).max(1),
        ((height * scale).round() as u32).max(1),
    ));
}

const TOOLBAR_MIN: u32 = 28;
const TOOLBAR_MAX: u32 = 96;
const TOOLBAR_DEFAULT: u32 = 40;

struct ResolvedPageToolbar {
    path: String,
    side: String,
    thickness: f64,
}

#[derive(Clone)]
struct CachedToolbar {
    id: String,
    path: String,
    side: String,
    thickness: f64,
}

struct ToolbarCache {
    generation: u64,
    bars: Vec<CachedToolbar>,
}

fn toolbar_generation() -> &'static AtomicU64 {
    static GENERATION: AtomicU64 = AtomicU64::new(0);
    &GENERATION
}

fn toolbar_cache() -> &'static Mutex<ToolbarCache> {
    static CACHE: Mutex<ToolbarCache> = Mutex::new(ToolbarCache {
        generation: u64::MAX,
        bars: Vec::new(),
    });
    &CACHE
}

fn toolbar_syncing() -> &'static AtomicBool {
    static SYNCING: AtomicBool = AtomicBool::new(false);
    &SYNCING
}

/// CSS pixels reserved around the page for extension toolbars.
pub struct PageToolbarInsets {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

pub fn page_toolbar_insets(app: &AppHandle) -> PageToolbarInsets {
    let mut insets = PageToolbarInsets {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };
    let zoom = settings::get(app).ui_zoom_factor();
    for bar in cached_toolbars(app) {
        let thickness = bar.thickness * zoom;
        match bar.side.as_str() {
            "top" => insets.top += thickness,
            "right" => insets.right += thickness,
            "left" => insets.left += thickness,
            _ => insets.bottom += thickness,
        }
    }
    insets
}

fn cached_toolbars(app: &AppHandle) -> Vec<CachedToolbar> {
    let generation = toolbar_generation().load(Ordering::Acquire);
    {
        let cache = toolbar_cache().lock().expect("page toolbars");
        if cache.generation == generation {
            return cache.bars.clone();
        }
    }
    let bars = enabled_page_toolbars(app);
    let mut cache = toolbar_cache().lock().expect("page toolbars");
    cache.generation = generation;
    cache.bars = bars.clone();
    bars
}

fn enabled_page_toolbars(app: &AppHandle) -> Vec<CachedToolbar> {
    let mut bars = Vec::new();
    for record in state(app) {
        if !record.enabled {
            continue;
        }
        let Some(loaded) = load_record(app, &record) else {
            continue;
        };
        if !grants_cover(&record.granted, &loaded.manifest.permissions) {
            continue;
        }
        let Some(toolbar) = page_toolbar_for_layout(&record, &loaded.manifest) else {
            continue;
        };
        bars.push(CachedToolbar {
            id: record.id.clone(),
            path: toolbar.path,
            side: toolbar.side,
            thickness: toolbar.thickness,
        });
    }
    bars.sort_by(|left, right| left.id.cmp(&right.id));
    bars
}

/// A toolbar that should take space around the page. A closed toolbar still resolves, so the
/// extensions page can show it again, but it is left out of the layout.
fn page_toolbar_for_layout(
    record: &HostRecord,
    manifest: &Manifest,
) -> Option<ResolvedPageToolbar> {
    if !record.toolbar_open {
        return None;
    }
    resolve_page_toolbar(record, manifest)
}

fn resolve_page_toolbar(record: &HostRecord, manifest: &Manifest) -> Option<ResolvedPageToolbar> {
    let detail = match &manifest.page_toolbar {
        PageToolbarSpec::Path(path) => {
            if path.trim().is_empty() {
                return None;
            }
            PageToolbarDetail {
                path: path.clone(),
                ..PageToolbarDetail::default()
            }
        }
        PageToolbarSpec::Detail(detail) => {
            if detail.path.trim().is_empty() {
                return None;
            }
            detail.clone()
        }
    };
    let side = match record.toolbar_side.as_str() {
        "top" | "right" | "bottom" | "left" => record.toolbar_side.clone(),
        _ => match detail.side.as_str() {
            "top" | "right" | "bottom" | "left" => detail.side.clone(),
            _ => "bottom".to_string(),
        },
    };
    let thickness = if detail.thickness == 0 {
        TOOLBAR_DEFAULT
    } else {
        detail.thickness
    }
    .clamp(TOOLBAR_MIN, TOOLBAR_MAX);
    Some(ResolvedPageToolbar {
        path: detail.path,
        side,
        thickness: f64::from(thickness),
    })
}

fn toolbar_label(window: &str, id: &str) -> String {
    format!("cext-toolbar-{window}-{}", id.replace('.', "_"))
}

fn close_page_toolbars(app: &AppHandle, id: &str) {
    let suffix = format!("-{}", id.replace('.', "_"));
    let labels: Vec<String> = app
        .webviews()
        .keys()
        .filter(|label| label.starts_with("cext-toolbar-") && label.ends_with(&suffix))
        .cloned()
        .collect();
    for label in labels {
        if let Some(view) = app.get_webview(&label) {
            let _ = view.close();
        }
    }
    relayout_windows(app);
}

pub fn set_page_toolbar_side(app: &AppHandle, id: &str, side: &str) -> Result<(), String> {
    if !matches!(side, "top" | "right" | "bottom" | "left") {
        return Err("The toolbar side must be top, right, bottom, or left.".into());
    }
    let id = canonical_id(id)?;
    let mut records = state(app);
    let record = records
        .iter_mut()
        .find(|record| record.id == id)
        .ok_or("That extension is not installed.")?;
    let loaded = load_record(app, record).ok_or("That extension folder could not be read.")?;
    if resolve_page_toolbar(record, &loaded.manifest).is_none() {
        return Err("That extension has no page toolbar.".into());
    }
    if record.toolbar_side == side {
        return Ok(());
    }
    record.toolbar_side = side.to_string();
    save_records(app, records)?;
    relayout_windows(app);
    let payload = json!({ "type": "ce-event", "event": "pageToolbar", "side": side });
    for window in tabs::snapshot(app).windows {
        post_label(app, &toolbar_label(&window.label, &id), &payload);
    }
    Ok(())
}

pub fn set_page_toolbar_open(app: &AppHandle, id: &str, open: bool) -> Result<(), String> {
    let id = canonical_id(id)?;
    let mut records = state(app);
    let record = records
        .iter_mut()
        .find(|record| record.id == id)
        .ok_or("That extension is not installed.")?;
    let loaded = load_record(app, record).ok_or("That extension folder could not be read.")?;
    if resolve_page_toolbar(record, &loaded.manifest).is_none() {
        return Err("That extension has no page toolbar.".into());
    }
    if record.toolbar_open == open {
        return Ok(());
    }
    record.toolbar_open = open;
    save_records(app, records)?;
    relayout_windows(app);
    Ok(())
}

fn relayout_windows(app: &AppHandle) {
    for window in tabs::snapshot(app).windows {
        tabs::layout(app, &window.label);
    }
}

/// Moves extension toolbars into the strip around the page. Missing webviews are created off this thread.
pub fn place_page_toolbars(app: &AppHandle, window: &str) {
    let prefix = format!("cext-toolbar-{window}-");
    let fullscreen = tabs::is_fullscreen(app, window);
    let bars = if fullscreen {
        Vec::new()
    } else {
        cached_toolbars(app)
    };
    let wanted: HashSet<String> = bars
        .iter()
        .map(|bar| toolbar_label(window, &bar.id))
        .collect();
    let mut missing = false;
    if !bars.is_empty() {
        let (origin, size) = tabs::page_area(app, window);
        let scale = app
            .get_window(window)
            .and_then(|host| host.scale_factor().ok())
            .unwrap_or(1.0)
            .max(0.01);
        let insets = page_toolbar_insets(app);
        let zoom = settings::get(app).ui_zoom_factor();
        let top = (insets.top * scale).round() as i32;
        let right = (insets.right * scale).round() as i32;
        let bottom = (insets.bottom * scale).round() as i32;
        let left = (insets.left * scale).round() as i32;
        let box_x = origin.x - left;
        let box_y = origin.y - top;
        let box_w = (size.width as i32 + left + right).max(1);
        let box_h = (size.height as i32 + top + bottom).max(1);
        let mut tops = Vec::new();
        let mut rights = Vec::new();
        let mut bottoms = Vec::new();
        let mut lefts = Vec::new();
        for bar in &bars {
            match bar.side.as_str() {
                "top" => tops.push(bar),
                "right" => rights.push(bar),
                "left" => lefts.push(bar),
                _ => bottoms.push(bar),
            }
        }
        let mut y = box_y;
        for bar in tops {
            let height = (bar.thickness * zoom * scale).round() as i32;
            missing |= !place_toolbar(app, window, bar, box_x, y, box_w, height.max(1));
            y += height;
        }
        let mut cursor = box_y + box_h;
        for bar in &bottoms {
            let height = (bar.thickness * zoom * scale).round() as i32;
            cursor -= height;
            missing |= !place_toolbar(app, window, bar, box_x, cursor, box_w, height.max(1));
        }
        let mid_y = box_y + top;
        let mid_h = (box_h - top - bottom).max(1);
        let mut x = box_x;
        for bar in lefts {
            let width = (bar.thickness * zoom * scale).round() as i32;
            missing |= !place_toolbar(app, window, bar, x, mid_y, width.max(1), mid_h);
            x += width;
        }
        let mut x = box_x + box_w;
        for bar in rights {
            let width = (bar.thickness * zoom * scale).round() as i32;
            x -= width;
            missing |= !place_toolbar(app, window, bar, x, mid_y, width.max(1), mid_h);
        }
    }
    for (label, view) in app.webviews() {
        if label.starts_with(&prefix) && !wanted.contains(label.as_str()) {
            park(&view);
        }
    }
    if missing && !toolbar_syncing().swap(true, Ordering::AcqRel) {
        let app = app.clone();
        std::thread::spawn(move || {
            sync_page_toolbars(&app);
            toolbar_syncing().store(false, Ordering::Release);
        });
    }
}

fn place_toolbar(
    app: &AppHandle,
    window: &str,
    bar: &CachedToolbar,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> bool {
    let Some(view) = app.get_webview(&toolbar_label(window, &bar.id)) else {
        return false;
    };
    let _ = view.set_position(PhysicalPosition::new(x, y));
    let _ = view.set_size(tauri::PhysicalSize::new(
        width.max(1) as u32,
        height.max(1) as u32,
    ));
    true
}

fn sync_page_toolbars(app: &AppHandle) {
    let bars = cached_toolbars(app);
    let windows: Vec<String> = tabs::snapshot(app)
        .windows
        .into_iter()
        .map(|window| window.label)
        .collect();
    for window in &windows {
        for bar in &bars {
            let Ok(url) = file_url(&bar.id, &bar.path) else {
                continue;
            };
            let label = toolbar_label(window, &bar.id);
            if ensure_child(app, window, &label, url.clone()).is_ok()
                && let Some(view) = app.get_webview(&label)
            {
                navigate_if_needed(&view, &url);
            }
        }
    }
    for window in windows {
        tabs::layout(app, &window);
    }
}

fn bookmarks_view(app: &AppHandle, window: &str) -> Option<Webview> {
    let id = settings::get(app).host_bookmarks_bar;
    if id.is_empty() {
        return None;
    }
    let loaded = require_enabled(app, &id).ok()?;
    if loaded.manifest.bookmarks_bar.is_empty() {
        return None;
    }
    let url = file_url(&id, &loaded.manifest.bookmarks_bar).ok()?;
    let label = format!("cext-bar-{window}");
    let view = ensure_child(app, window, &label, url.clone()).ok()?;
    navigate_if_needed(&view, &url);
    Some(view)
}

fn ensure_child(app: &AppHandle, window: &str, label: &str, url: Url) -> Result<Webview, String> {
    if let Some(view) = app.get_webview(label) {
        if !label.starts_with("cext-panel-") || panel_view_alive(&view) {
            return Ok(view);
        }
        eprintln!("panel: {label} stopped answering, so it is being created again");
        let _ = view.close();
        for _ in 0..30 {
            if app.get_webview(label).is_none() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        if app.get_webview(label).is_some() {
            return Err(format!("{label} closed and could not be created again."));
        }
    }
    let host = app
        .get_window(window)
        .ok_or("The browser window is missing.")?;
    let (r, g, b) = settings::popup_surface(app);
    // The real page is a virtual host. Mapping that host has to exist before the navigation,
    // or WebView2 treats `*.ce-ext.localhost` as a refused connection and the document stays
    // on the error page.
    let blank = Url::parse("about:blank").map_err(|error| error.to_string())?;
    let mut builder = tabs::base_builder(app, label, WebviewUrl::External(blank))
        .background_color(Color(r, g, b, 255))
        .focused(false);
    if label.starts_with("cext-panel-") {
        // A store panel is a child webview, so WebView2 denies `chrome.windows.create` unless
        // this handler builds the popout. That is the button that opens the extension's window.
        let host = app.clone();
        builder = builder
            .initialization_script(EXTENSION_TAB_JS)
            .initialization_script(PANEL_HANDLE_JS)
            .initialization_script(PANEL_CHROME_JS)
            .on_new_window(move |url, features| tabs::new_window(&host, None, url, features));
    } else if label.starts_with("cext-toolbar-") {
        builder = builder.initialization_script(TOOLBAR_CHROME_JS);
    }
    let view = host
        .add_child(
            builder,
            LogicalPosition::new(-10000.0, -10000.0),
            LogicalSize::new(360.0, 480.0),
        )
        .map_err(|error| error.to_string())?;
    attach_webview(app, &view);
    let _ = view.set_zoom(settings::get(app).ui_zoom_factor());
    if label.starts_with("cext-panel-") {
        watch_panel_navigation(app, &view, label);
        eprintln!("panel: created {label}");
    } else {
        let _ = view.navigate(url);
    }
    Ok(view)
}

/// Side panels, page toolbars, bookmark bars and host popouts follow Browser UI size.
pub fn apply_ui_zoom(app: &AppHandle, zoom: f64, ratio: f64) {
    for (label, webview) in app.webviews() {
        let chrome = label.starts_with("cext-panel-")
            || label.starts_with("cext-toolbar-")
            || label.starts_with("cext-bar-")
            || label.starts_with("cext-out-");
        if !chrome {
            continue;
        }
        let _ = webview.set_zoom(zoom);
        if !label.starts_with("cext-out-") {
            continue;
        }
        let Some(window) = app.get_window(&label) else {
            continue;
        };
        let _ = window.set_min_size(Some(LogicalSize::new(240.0 * zoom, 180.0 * zoom)));
        if let (Ok(size), Ok(scale)) = (window.inner_size(), window.scale_factor())
            && scale > f64::EPSILON
        {
            let _ = window.set_size(LogicalSize::new(
                size.width as f64 / scale * ratio,
                size.height as f64 / scale * ratio,
            ));
        }
    }
}

fn navigate_if_needed(view: &Webview, url: &Url) {
    let current = view.url().ok();
    let failed = current
        .as_ref()
        .is_some_and(|current| panel_placeholder(current.as_str()));
    let same = current.is_some_and(|current| current.as_str() == url.as_str());
    if failed || !same {
        let _ = view.navigate(url.clone());
    }
}

const PANEL_CHECK_DELAY: Duration = Duration::from_millis(350);
const PANEL_CONTENT_DELAY: Duration = Duration::from_millis(500);
const PANEL_RETRIES: u8 = 2;

struct PanelLoad {
    url: String,
    retries: u8,
    logged_content: bool,
    waiting: bool,
}

fn panel_loads() -> &'static Mutex<HashMap<String, PanelLoad>> {
    static LOADS: std::sync::OnceLock<Mutex<HashMap<String, PanelLoad>>> =
        std::sync::OnceLock::new();
    LOADS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn panel_view_alive(view: &Webview) -> bool {
    match view.url() {
        Ok(_) => true,
        Err(error) => {
            eprintln!("panel: {} is not answering ({error})", view.label());
            false
        }
    }
}

/// A blank or error document. The close and side buttons can paint here, with no extension.
fn panel_placeholder(url: &str) -> bool {
    let url = url.trim();
    url.is_empty()
        || url.starts_with("about:blank")
        || url.starts_with("chrome-error:")
        || url.starts_with("edge-error:")
}

/// Same extension document, ignoring a hash or query the page adds after it loads.
fn panel_document_ready(current: &str, wanted: &str) -> bool {
    if panel_placeholder(current) {
        return false;
    }
    match (Url::parse(current), Url::parse(wanted)) {
        (Ok(current), Ok(wanted)) => {
            current.scheme() == wanted.scheme()
                && current.host_str() == wanted.host_str()
                && current.path().trim_end_matches('/') == wanted.path().trim_end_matches('/')
        }
        _ => false,
    }
}

fn panel_target_url(label: &str) -> Option<String> {
    panel_loads()
        .lock()
        .expect("panel loads")
        .get(label)
        .map(|load| load.url.clone())
}

fn remember_panel_target(label: &str, url: &str, fresh: bool) {
    let mut loads = panel_loads().lock().expect("panel loads");
    if let Some(load) = loads.get_mut(label) {
        let same = load.url == url;
        load.url = url.to_string();
        if fresh || !same {
            load.retries = 0;
            load.logged_content = false;
            load.waiting = false;
        }
        return;
    }
    loads.insert(
        label.to_string(),
        PanelLoad {
            url: url.to_string(),
            retries: 0,
            logged_content: false,
            waiting: false,
        },
    );
}

fn panel_column_open(app: &AppHandle, label: &str) -> bool {
    let Some(window) = label.strip_prefix("cext-panel-") else {
        return false;
    };
    tabs::side_panel(app, window) == tabs::SidePanel::Host
        && tabs::agent_panel_width(app, window) > 0.0
}

fn panel_bounds(view: &Webview) -> String {
    match (view.position(), view.size()) {
        (Ok(at), Ok(size)) => format!("{},{} {}x{}", at.x, at.y, size.width, size.height),
        (Err(error), _) | (_, Err(error)) => format!("unreadable ({error})"),
    }
}

fn panel_is_parked(view: &Webview) -> bool {
    view.position()
        .is_ok_and(|at| at.x <= -4_000 || at.y <= -4_000)
}

/// Navigates the panel after the column has a real size, then checks that the extension arrived.
fn reveal_panel_document(app: &AppHandle, label: &str, url: &Url) {
    let Some(view) = app.get_webview(label) else {
        eprintln!("panel: {label} is missing, so {url} cannot load");
        return;
    };
    webview2::repaint_child(&view);
    let current = view
        .url()
        .map(|current| current.to_string())
        .unwrap_or_else(|error| format!("unreadable ({error})"));
    if panel_document_ready(&current, url.as_str()) {
        eprintln!(
            "panel: {label} already showing {current} at {}",
            panel_bounds(&view)
        );
        schedule_panel_content(app, label);
        return;
    }
    eprintln!(
        "panel: load {label} from {current} to {url} at {}",
        panel_bounds(&view)
    );
    if let Err(error) = view.navigate(url.clone()) {
        eprintln!("panel: navigate {label} failed: {error}");
    }
    schedule_panel_check(app, label);
}

fn schedule_panel_check(app: &AppHandle, label: &str) {
    {
        let mut loads = panel_loads().lock().expect("panel loads");
        let Some(load) = loads.get_mut(label) else {
            return;
        };
        if load.waiting {
            return;
        }
        load.waiting = true;
    }
    let app = app.clone();
    let label = label.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(PANEL_CHECK_DELAY);
        let app_for_check = app.clone();
        let _ = app.run_on_main_thread(move || {
            {
                let mut loads = panel_loads().lock().expect("panel loads");
                if let Some(load) = loads.get_mut(&label) {
                    load.waiting = false;
                }
            }
            check_panel_document(&app_for_check, &label);
        });
    });
}

fn check_panel_document(app: &AppHandle, label: &str) {
    let Some(wanted) = panel_target_url(label) else {
        return;
    };
    if !panel_column_open(app, label) {
        return;
    }
    let Some(view) = app.get_webview(label) else {
        eprintln!("panel: {label} disappeared before {wanted} loaded");
        return;
    };
    let current = view.url().map(|url| url.to_string()).unwrap_or_default();
    let place = panel_bounds(&view);
    eprintln!("panel: check {label} at {place} url={current}");
    if panel_is_parked(&view)
        && let Some(window) = label.strip_prefix("cext-panel-")
    {
        eprintln!("panel: {label} is off screen while its column is open");
        tabs::layout(app, window);
    }
    if panel_document_ready(&current, &wanted) {
        repost_panel_chrome(app, label);
        schedule_panel_content(app, label);
        return;
    }
    retry_panel_document(app, label, &wanted, &current);
}

fn retry_panel_document(app: &AppHandle, label: &str, wanted: &str, current: &str) {
    {
        let mut loads = panel_loads().lock().expect("panel loads");
        let Some(load) = loads.get_mut(label) else {
            return;
        };
        if load.retries >= PANEL_RETRIES {
            eprintln!("panel: gave up on {label} at {current}; wanted {wanted}");
            return;
        }
        load.retries += 1;
        eprintln!(
            "panel: {label} is {current}, loading {wanted} again ({}/{})",
            load.retries, PANEL_RETRIES
        );
    }
    let Some(view) = app.get_webview(label) else {
        eprintln!("panel: {label} is missing, so the retry cannot load {wanted}");
        return;
    };
    let Ok(url) = Url::parse(wanted) else {
        eprintln!("panel: {wanted} is not an address");
        return;
    };
    if let Err(error) = view.navigate(url) {
        eprintln!("panel: navigate {label} failed: {error}");
    }
    schedule_panel_check(app, label);
}

fn schedule_panel_content(app: &AppHandle, label: &str) {
    {
        let mut loads = panel_loads().lock().expect("panel loads");
        let Some(load) = loads.get_mut(label) else {
            return;
        };
        if load.logged_content {
            return;
        }
        load.logged_content = true;
    }
    let app = app.clone();
    let label = label.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(PANEL_CONTENT_DELAY);
        let app_for_log = app.clone();
        let _ = app.run_on_main_thread(move || log_panel_content(&app_for_log, &label));
    });
}

const PANEL_SNAPSHOT_JS: &str = r#"(function () {
  var body = document.body;
  var text = "";
  var nodes = 0;
  if (body) {
    text = String(body.innerText || "").replace(/\s+/g, " ").trim().slice(0, 160);
    var children = body.children || [];
    for (var i = 0; i < children.length; i++) {
      var id = children[i] && children[i].id || "";
      if (id.indexOf("ce-panel") === 0) continue;
      nodes++;
    }
  }
  return {
    href: String(location.href || ""),
    ready: document.readyState,
    nodes: nodes,
    text: text,
    chrome: !!document.getElementById("ce-panel-chrome")
  };
})()"#;

fn log_panel_content(app: &AppHandle, label: &str) {
    if !panel_column_open(app, label) {
        return;
    }
    let Some(view) = app.get_webview(label) else {
        eprintln!("panel: {label} is gone before its contents could be checked");
        return;
    };
    let label = label.to_string();
    let _ = view.with_webview(move |platform| unsafe {
        let Ok(core) = platform.controller().CoreWebView2() else {
            eprintln!("panel: {label} has no document to check");
            return;
        };
        let script_label = label.clone();
        let handler = ExecuteScriptCompletedHandler::create(Box::new(move |result, value| {
            match result {
                Ok(()) => report_panel_content(&script_label, &value),
                Err(error) => eprintln!("panel: {script_label} content check failed: {error}"),
            }
            Ok(())
        }));
        if let Err(error) = core.ExecuteScript(&HSTRING::from(PANEL_SNAPSHOT_JS), &handler) {
            eprintln!("panel: {label} content check failed: {error}");
        }
    });
}

fn report_panel_content(label: &str, raw: &str) {
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        eprintln!("panel: {label} content {raw}");
        return;
    };
    let href = value.get("href").and_then(Value::as_str).unwrap_or("");
    let ready = value.get("ready").and_then(Value::as_str).unwrap_or("");
    let nodes = value.get("nodes").and_then(Value::as_u64).unwrap_or(0);
    let text = value.get("text").and_then(Value::as_str).unwrap_or("");
    let chrome = value
        .get("chrome")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if panel_placeholder(href) {
        eprintln!("panel: {label} chrome is on {href} and the extension document is not");
        return;
    }
    if nodes == 0 && text.is_empty() {
        eprintln!(
            "panel: {label} loaded {href} ({ready}) but the extension drew nothing; chrome={chrome}"
        );
        return;
    }
    eprintln!("panel: {label} is showing {href} ({nodes} nodes, chrome={chrome}, {ready})");
}

fn watch_panel_navigation(app: &AppHandle, view: &Webview, label: &str) {
    let app = app.clone();
    let label = label.to_string();
    let watched = label.clone();
    if let Err(error) = view.with_webview(move |platform| unsafe {
        let Ok(core) = platform.controller().CoreWebView2() else {
            eprintln!("panel: {label} has no webview to watch");
            return;
        };
        let watch_app = app.clone();
        let watch_label = label.clone();
        let mut token = 0i64;
        let handler = NavigationCompletedEventHandler::create(Box::new(move |sender, args| {
            let Some(args) = args else {
                return Ok(());
            };
            let success = webview2::read_bool(|value| args.IsSuccess(value));
            let mut status = COREWEBVIEW2_WEB_ERROR_STATUS::default();
            let _ = args.WebErrorStatus(&mut status);
            let code = crate::page_errors::web_error(status)
                .map(|(_, code)| code)
                .unwrap_or(if success { "OK" } else { "CANCELED" });
            let source = sender
                .as_ref()
                .map(|core| webview2::read_pwstr(|value| core.Source(value)))
                .unwrap_or_default();
            let app = watch_app.clone();
            let label = watch_label.clone();
            let code = code.to_string();
            std::thread::spawn(move || {
                let app_for_settle = app.clone();
                let _ = app.run_on_main_thread(move || {
                    settle_panel_navigation(&app_for_settle, &label, success, &code, &source);
                });
            });
            Ok(())
        }));
        if core.add_NavigationCompleted(&handler, &mut token).is_err() {
            eprintln!("panel: could not watch navigation for {label}");
        }
    }) {
        eprintln!("panel: could not watch {watched}: {error}");
    }
}

fn repost_panel_chrome(app: &AppHandle, label: &str) {
    let Some(window) = label.strip_prefix("cext-panel-") else {
        return;
    };
    let id = tabs::host_panel_extension(app, window);
    if id.is_empty() {
        return;
    }
    if state(app).iter().any(|record| record.id == id) {
        let Ok(loaded) = require_enabled(app, &id) else {
            eprintln!("panel: {label} belongs to {id}, which is not enabled");
            return;
        };
        let layout = resolve_panel(&loaded.record, &loaded.manifest);
        post_panel_chrome(app, window, &layout);
        return;
    }
    post_store_panel_chrome(
        app,
        window,
        store_panel_edge(app, &id),
        store_panel_resizable(app, &id),
    );
}

fn settle_panel_navigation(app: &AppHandle, label: &str, success: bool, code: &str, source: &str) {
    let Some(wanted) = panel_target_url(label) else {
        return;
    };
    if !panel_column_open(app, label) {
        return;
    }
    eprintln!("panel: {label} navigation finished success={success} status={code} url={source}");
    if success && panel_document_ready(source, &wanted) {
        repost_panel_chrome(app, label);
        return;
    }
    if !success && (code == "CANCELED" || code == "CONNECTION_ABORTED") {
        eprintln!("panel: {label} navigation was cancelled");
        return;
    }
    retry_panel_document(app, label, &wanted, source);
}

/// The browser window a store popover or side panel belongs to.
fn extension_surface_window(label: &str) -> String {
    if let Some(window) = label.strip_prefix("cext-panel-") {
        return window.to_string();
    }
    crate::popup::owner_of(label)
}

/// Browser window that owns this extension page. Options tabs and popouts are included.
fn surface_window(app: &AppHandle, label: &str) -> String {
    if let Some(id) = label
        .strip_prefix("tab-")
        .and_then(|rest| rest.parse::<u32>().ok())
    {
        let snapshot = tabs::snapshot(app);
        if let Some(tab) = snapshot.tabs.iter().find(|tab| tab.id == id) {
            return tab.window.clone();
        }
    }
    if label.starts_with("script-") {
        return tabs::focused_window(app);
    }
    let owner = extension_surface_window(label);
    let snapshot = tabs::snapshot(app);
    if snapshot.windows.iter().any(|window| window.label == owner) {
        return owner;
    }
    tabs::focused_window(app)
}

/// Stable positive id for a browser window. Chromium's current-window sentinel is negative.
fn window_number(label: &str) -> i64 {
    let mut hash: u32 = 2_166_136_261;
    for byte in label.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(16_777_619);
    }
    i64::from((hash % 900_000) + 1)
}

/// Address Chromium reports for a page view. Store popups match logins against this, and a
/// `ce://` tab is shown to the extension as the app URL loaded in that view.
fn extension_page_url(app: &AppHandle, id: u32, logical: &str) -> String {
    let Some(view) = app.get_webview(&format!("tab-{id}")) else {
        return logical.to_string();
    };
    let Ok(url) = view.url() else {
        return logical.to_string();
    };
    let text = url.to_string();
    if text.starts_with("http://") || text.starts_with("https://") || text.starts_with("file:") {
        text
    } else {
        logical.to_string()
    }
}

/// Tells a store popover or panel which page tab is open, so `chrome.tabs.query` for the
/// current window can return that page instead of the extension view.
fn reply_extension_active_tab(app: &AppHandle, label: &str, token: i64) {
    let window = surface_window(app, label);
    let snapshot = tabs::snapshot(app);
    let active = snapshot
        .windows
        .iter()
        .find(|item| item.label == window)
        .and_then(|item| item.active_id);
    let Some(id) = active else {
        post_label(
            app,
            label,
            &json!({ "type": "ce-active-tab", "token": token }),
        );
        return;
    };
    let tab = snapshot.tabs.iter().find(|tab| tab.id == id);
    let logical = tab.map(|tab| tab.url.clone()).unwrap_or_default();
    let title = tab.map(|tab| tab.title.clone()).unwrap_or_default();
    let url = extension_page_url(app, id, &logical);
    post_label(
        app,
        label,
        &json!({
            "type": "ce-active-tab",
            "token": token,
            "url": url,
            "title": title,
        }),
    );
}

/// Screen DIP. `chrome.windows` left, top, width and height have to be integers.
/// A fractional or missing value makes `windows.create` throw, so a popout never opens.
fn dip_px(physical: i32, scale: f64) -> i64 {
    if scale <= f64::EPSILON {
        return i64::from(physical);
    }
    (f64::from(physical) / scale).round() as i64
}

/// Outer frame of a browser window, in the units `chrome.windows` uses.
fn window_frame(app: &AppHandle, label: &str) -> Option<Value> {
    let window = app.get_window(label)?;
    let scale = window.scale_factor().unwrap_or(1.0);
    let origin = window.outer_position().ok()?;
    let size = window.outer_size().ok()?;
    let state = if window.is_minimized().unwrap_or(false) {
        "minimized"
    } else if tabs::is_fullscreen(app, label) {
        "fullscreen"
    } else if window.is_maximized().unwrap_or(false) {
        "maximized"
    } else {
        "normal"
    };
    let width = dip_px(size.width as i32, scale).max(1);
    let height = dip_px(size.height as i32, scale).max(1);
    Some(json!({
        "left": dip_px(origin.x, scale),
        "top": dip_px(origin.y, scale),
        "width": width,
        "height": height,
        "state": state,
    }))
}

/// The browser's tabs and windows, so an extension page can answer `chrome.tabs.query`
/// and `chrome.windows.getAll` with one window per browser window. Bounds are included
/// because a toolbar popout, such as Bitwarden's, positions `chrome.windows.create`
/// from `getCurrent`.
fn reply_extension_tabs(app: &AppHandle, label: &str, token: i64) {
    let snapshot = tabs::snapshot(app);
    let current = surface_window(app, label);
    let focused = tabs::focused_window(app);
    let windows: Vec<Value> = snapshot
        .windows
        .iter()
        .map(|window| {
            let mut entry = json!({
                "id": window_number(&window.label),
                "focused": window.label == focused,
            });
            if let Some(frame) = window_frame(app, &window.label)
                && let (Some(entry), Some(frame)) = (entry.as_object_mut(), frame.as_object())
            {
                for (key, value) in frame {
                    entry.insert(key.clone(), value.clone());
                }
            }
            entry
        })
        .collect();
    let mut indexes: HashMap<String, i64> = HashMap::new();
    let mut listed = Vec::new();
    for tab in &snapshot.tabs {
        let index = indexes.entry(tab.window.clone()).or_insert(0);
        let at = *index;
        *index += 1;
        let active = snapshot
            .windows
            .iter()
            .any(|window| window.label == tab.window && window.active_id == Some(tab.id));
        listed.push(json!({
            "id": tab.id,
            "windowId": window_number(&tab.window),
            "index": at,
            "url": extension_page_url(app, tab.id, &tab.url),
            "title": tab.title,
            "active": active,
            "pinned": tab.pinned,
            "status": if tab.loading { "loading" } else { "complete" },
            "audible": tab.audible,
            "muted": tab.muted,
            "discarded": tab.sleeping,
        }));
    }
    post_label(
        app,
        label,
        &json!({
            "type": "ce-tabs",
            "token": token,
            "currentWindowId": window_number(&current),
            "lastFocusedWindowId": window_number(&focused),
            "windows": windows,
            "tabs": listed,
        }),
    );
}

/// `chrome.sidePanel.open` from an extension page this browser hosts.
fn open_called_side_panel(app: &AppHandle, label: &str) {
    let Some(view) = app.get_webview(label) else {
        return;
    };
    let Ok(current) = view.url() else {
        return;
    };
    let Some(id) = chrome_extension_id(current.as_str()) else {
        return;
    };
    let Ok(page) = crate::extensions::side_panel_url(app, &id) else {
        return;
    };
    let window = surface_window(app, label);
    if let Err(error) = open_store_panel(app, &window, &id, page, store_panel_edge(app, &id)) {
        eprintln!("panel: {error}");
    }
}

/// Browser controls on a panel, popover, or page toolbar. The document may be the browser's
/// error page, which has no `window.ce`, so the webview label says which surface this is.
fn apply_chrome_action(app: &AppHandle, label: &str, action: &str, side: &str) {
    if label.starts_with("extpopup-") {
        apply_popup_chrome(app, label, action, side);
        return;
    }
    if let Some(window) = label.strip_prefix("cext-panel-") {
        let id = tabs::host_panel_extension(app, window);
        if id.is_empty() {
            return;
        }
        if !state(app).iter().any(|record| record.id == id) {
            apply_store_panel_chrome(app, window, &id, action, side);
            return;
        }
        let result = if action == "close" {
            toggle_panel(app, window, &id).map(|_| ())
        } else if action == "side" {
            set_panel_layout(app, &id, side)
        } else {
            Ok(())
        };
        if let Err(error) = result {
            eprintln!("panel chrome: {error}");
        }
        return;
    }
    let Some(id) = toolbar_owner(app, label) else {
        return;
    };
    let result = if action == "close" {
        set_page_toolbar_open(app, &id, false)
    } else if action == "side" {
        set_page_toolbar_side(app, &id, side)
    } else {
        Ok(())
    };
    if let Err(error) = result {
        eprintln!("toolbar chrome: {error}");
    }
}

fn toolbar_owner(app: &AppHandle, label: &str) -> Option<String> {
    if !label.starts_with("cext-toolbar-") {
        return None;
    }
    let windows: Vec<String> = tabs::snapshot(app)
        .windows
        .into_iter()
        .map(|window| window.label)
        .collect();
    state(app).into_iter().find_map(|record| {
        windows
            .iter()
            .any(|window| toolbar_label(window, &record.id) == label)
            .then_some(record.id)
    })
}

fn park(view: &Webview) {
    let _ = view.set_position(tauri::PhysicalPosition::new(-10000, -10000));
}

fn sync_backgrounds(app: &AppHandle) {
    let wanted: HashSet<String> = state(app)
        .iter()
        .filter_map(|record| {
            let loaded = load_record(app, record)?;
            if record.enabled && !loaded.manifest.background.is_empty() {
                Some(record.id.clone())
            } else {
                None
            }
        })
        .collect();
    for record in state(app) {
        if !wanted.contains(&record.id) {
            close_background(app, &record.id);
        }
    }
    let window = tabs::focused_window(app);
    for id in wanted {
        let Ok(loaded) = require_enabled(app, &id) else {
            continue;
        };
        let Ok(url) = file_url(&id, &loaded.manifest.background) else {
            continue;
        };
        let label = background_label(&id);
        if ensure_child(app, &window, &label, url.clone()).is_ok()
            && let Some(view) = app.get_webview(&label)
        {
            navigate_if_needed(&view, &url);
            park(&view);
        }
    }
}

fn close_background(app: &AppHandle, id: &str) {
    if let Some(view) = app.get_webview(&background_label(id)) {
        let _ = view.close();
    }
}

fn background_label(id: &str) -> String {
    format!("cext-bg-{}", id.replace('.', "_"))
}

pub fn note_tabs(app: &AppHandle) {
    broadcast(app, "tabs");
}

pub fn note_downloads(app: &AppHandle) {
    broadcast(app, "downloads");
}

fn broadcast(app: &AppHandle, event: &str) {
    let backgrounds: Vec<String> = state(app)
        .iter()
        .filter(|record| record.enabled)
        .map(|record| background_label(&record.id))
        .filter(|label| app.get_webview(label).is_some())
        .collect();
    if backgrounds.is_empty() {
        return;
    }
    {
        let mut inflight = broadcasts().lock().expect("host broadcasts");
        if !inflight.insert(event.to_string()) {
            return;
        }
    }
    let app = app.clone();
    let event = event.to_string();
    std::thread::spawn(move || {
        let payload = json!({ "type": "ce-event", "event": event });
        for label in &backgrounds {
            post_label(&app, label, &payload);
        }
        broadcasts().lock().expect("host broadcasts").remove(&event);
    });
}

/// A document posted `ce-rpc` or `ce-content`. `tab_id` is set for page views.
pub fn handle_message(app: &AppHandle, tab_id: Option<u32>, label: &str, source: &str, raw: &str) {
    let Ok(mut message) = serde_json::from_str::<Value>(raw) else {
        return;
    };
    if let Some(text) = message.as_str()
        && let Ok(inner) = serde_json::from_str::<Value>(text)
    {
        message = inner;
    }
    let kind = message
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if kind == "ce-chrome" {
        let action = message
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let side = message
            .get("side")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let token = message.get("token").and_then(Value::as_i64).unwrap_or(0);
        let app = app.clone();
        let label = label.to_string();
        std::thread::spawn(move || {
            if action == "active-tab" {
                reply_extension_active_tab(&app, &label, token);
            } else if action == "tabs" {
                reply_extension_tabs(&app, &label, token);
            } else if action == "side-panel" {
                open_called_side_panel(&app, &label);
            } else {
                apply_chrome_action(&app, &label, &action, &side);
            }
        });
        return;
    }
    if kind != "ce-rpc" && kind != "ce-content" {
        return;
    }
    if raw.len() > MAX_MESSAGE {
        return;
    }
    let app = app.clone();
    let label = label.to_string();
    let source = source.to_string();
    std::thread::spawn(move || {
        if kind == "ce-content" {
            deliver_content(&app, tab_id, &message);
            return;
        }
        let rpc_id = message.get("id").and_then(Value::as_i64).unwrap_or(0);
        let method = message
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
        let result = match caller(&app, tab_id, &source) {
            Ok(id) => dispatch(&app, &id, &method, &params),
            Err(error) => Err(error),
        };
        let payload = match result {
            Ok(value) => json!({ "type": "ce-rpc-result", "id": rpc_id, "result": value }),
            Err(error) => json!({ "type": "ce-rpc-result", "id": rpc_id, "error": error }),
        };
        if let Some(tab_id) = tab_id {
            post_label(&app, &format!("tab-{tab_id}"), &payload);
        } else {
            post_label(&app, &label, &payload);
        }
    });
}

fn caller(app: &AppHandle, tab_id: Option<u32>, source: &str) -> Result<String, String> {
    if let Some(id) = extension_from_location(app, source) {
        return require_enabled(app, &id).map(|loaded| loaded.record.id);
    }
    if let Some(tab_id) = tab_id {
        if let Some(url) = tabs::tab_page_url(app, tab_id)
            && let Some(id) = extension_from_location(app, &url)
        {
            return require_enabled(app, &id).map(|loaded| loaded.record.id);
        }
        if let Some(view) = app.get_webview(&format!("tab-{tab_id}"))
            && let Ok(url) = view.url()
            && let Some(id) = extension_from_location(app, url.as_str())
        {
            return require_enabled(app, &id).map(|loaded| loaded.record.id);
        }
    }
    Err("This page cannot use ce.".into())
}

fn deliver_content(app: &AppHandle, tab_id: Option<u32>, message: &Value) {
    let Some(id) = message.get("extensionId").and_then(Value::as_str) else {
        return;
    };
    if require_enabled(app, id).is_err() {
        return;
    }
    let sender = json!({
        "tabId": tab_id,
        "url": tab_id.and_then(|id| tabs::tab_page_url(app, id)).unwrap_or_default(),
    });
    let payload = json!({
        "type": "ce-event",
        "event": "message",
        "message": message.get("message").cloned().unwrap_or(Value::Null),
        "sender": sender,
    });
    post_label(app, &background_label(id), &payload);
}

fn post_label(app: &AppHandle, label: &str, payload: &Value) {
    let Some(view) = app.get_webview(label) else {
        return;
    };
    let json = payload.to_string();
    let _ = view.with_webview(move |platform| unsafe {
        if let Ok(core) = platform.controller().CoreWebView2() {
            let _ = core.PostWebMessageAsJson(&HSTRING::from(json.as_str()));
        }
    });
}

fn dispatch(app: &AppHandle, id: &str, method: &str, params: &Value) -> Result<Value, String> {
    let loaded = require_enabled(app, id)?;
    let allows = |permission: &str| loaded.record.granted.iter().any(|item| item == permission);
    let need = |permission: &str| -> Result<(), String> {
        if allows(permission) {
            Ok(())
        } else {
            Err(format!(
                "This extension does not have the {permission} permission."
            ))
        }
    };
    match method {
        "tabs.list" => {
            need("tabs")?;
            Ok(tabs_payload(app))
        }
        "tabs.create" => {
            need("tabs")?;
            let url = params.get("url").and_then(Value::as_str).unwrap_or("");
            let background = params
                .get("background")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let input = if url.is_empty() {
                None
            } else {
                Some(url.to_string())
            };
            let id = tabs::create_tab(app, input, background, None)?;
            Ok(json!({ "id": id }))
        }
        "tabs.close" => {
            need("tabs")?;
            tabs::close_tab(app, number_field(params, "id")?)?;
            Ok(Value::Null)
        }
        "tabs.activate" => {
            need("tabs")?;
            tabs::activate(app, number_field(params, "id")?)?;
            Ok(Value::Null)
        }
        "tabs.navigate" => {
            need("tabs")?;
            let url = text_field(params, "url")?;
            tabs::navigate(app, number_field(params, "id")?, &url)?;
            Ok(Value::Null)
        }
        "tabs.back" => {
            need("tabs")?;
            tabs::go_back(app, number_field(params, "id")?);
            Ok(Value::Null)
        }
        "tabs.forward" => {
            need("tabs")?;
            tabs::go_forward(app, number_field(params, "id")?);
            Ok(Value::Null)
        }
        "tabs.reload" => {
            need("tabs")?;
            tabs::reload(app, number_field(params, "id")?)?;
            Ok(Value::Null)
        }
        "page.executeScript" => {
            need("page")?;
            let code = text_field(params, "code")?;
            let tab = tab_field(params)?;
            let view = tab_view(app, tab)?;
            webview2::evaluate(
                &view,
                &code,
                Duration::from_secs(15),
                "The page script failed.",
            )
        }
        "page.insertCSS" => {
            need("page")?;
            let css = serde_json::to_string(&text_field(params, "css")?)
                .unwrap_or_else(|_| "\"\"".into());
            let script = format!(
                "(function(){{var style=document.createElement('style');style.textContent={css};(document.head||document.documentElement).appendChild(style);return true;}})()"
            );
            let view = tab_view(app, tab_field(params)?)?;
            webview2::evaluate(
                &view,
                &script,
                Duration::from_secs(15),
                "The page style failed.",
            )
        }
        "page.capture" => {
            need("page")?;
            let (image, truncated) =
                crate::capture::page_image(app, tab_field(params)?, false, false)?;
            let png = crate::capture::encode_png(&image)?;
            Ok(json!({
                "width": image.width(),
                "height": image.height(),
                "truncated": truncated,
                "dataUrl": format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)),
            }))
        }
        "page.click" => {
            need("page")?;
            let tab = tab_field(params)?;
            let x = float_field(params, "x")?;
            let y = float_field(params, "y")?;
            crate::automation::driver::click_at(app, tab, x, y, "left", false)?;
            Ok(Value::Null)
        }
        "page.type" => {
            need("page")?;
            crate::automation::driver::type_text(
                app,
                tab_field(params)?,
                None,
                &text_field(params, "text")?,
                false,
                false,
            )?;
            Ok(Value::Null)
        }
        "page.scroll" => {
            need("page")?;
            let dy = float_field(params, "dy").unwrap_or(1.0);
            crate::automation::driver::scroll(app, tab_field(params)?, None, dy, false)?;
            Ok(Value::Null)
        }
        "desktop.move" => {
            need_desktop(app, &loaded)?;
            crate::automation::desktop::move_to(
                float_field(params, "x")?,
                float_field(params, "y")?,
            )?;
            Ok(Value::Null)
        }
        "desktop.click" => {
            need_desktop(app, &loaded)?;
            crate::automation::desktop::click(
                float_field(params, "x")?,
                float_field(params, "y")?,
            )?;
            Ok(Value::Null)
        }
        "desktop.type" => {
            need_desktop(app, &loaded)?;
            crate::automation::desktop::type_text(&text_field(params, "text")?)?;
            Ok(Value::Null)
        }
        "bookmarks.list" => {
            need("bookmarks")?;
            Ok(serde_json::to_value(bookmarks::tree(app)).unwrap_or(Value::Null))
        }
        "bookmarks.add" => {
            need("bookmarks")?;
            let parent = params
                .get("parentId")
                .and_then(Value::as_str)
                .unwrap_or(bookmarks::BAR);
            let parent = if parent.is_empty() {
                bookmarks::BAR
            } else {
                parent
            };
            let url = params
                .get("url")
                .and_then(Value::as_str)
                .filter(|url| !url.is_empty());
            let node = bookmarks::add(
                app,
                parent,
                &text_field(params, "title").unwrap_or_default(),
                url.map(str::to_string),
                None,
                false,
            )?;
            Ok(serde_json::to_value(node).unwrap_or(Value::Null))
        }
        "bookmarks.update" => {
            need("bookmarks")?;
            let url = params
                .get("url")
                .and_then(Value::as_str)
                .map(str::to_string);
            bookmarks::update(
                app,
                &text_field(params, "id")?,
                &text_field(params, "title").unwrap_or_default(),
                url,
            )?;
            Ok(Value::Null)
        }
        "bookmarks.remove" => {
            need("bookmarks")?;
            bookmarks::delete_returning(app, &text_field(params, "id")?)?;
            Ok(Value::Null)
        }
        "downloads.list" => {
            need("downloads")?;
            Ok(serde_json::to_value(downloads::list(app)).unwrap_or(Value::Null))
        }
        "downloads.pause" => {
            need("downloads")?;
            downloads::pause(app, number_field(params, "id")?)?;
            Ok(Value::Null)
        }
        "downloads.resume" => {
            need("downloads")?;
            downloads::resume(app, number_field(params, "id")?)?;
            Ok(Value::Null)
        }
        "downloads.cancel" => {
            need("downloads")?;
            downloads::cancel(app, number_field(params, "id")?)?;
            Ok(Value::Null)
        }
        "downloads.open" => {
            need("downloads")?;
            downloads::open(app, number_field(params, "id")?, &tabs::focused_window(app))?;
            Ok(Value::Null)
        }
        "downloads.show" => {
            need("downloads")?;
            downloads::show_in_folder(app, number_field(params, "id")?)?;
            Ok(Value::Null)
        }
        "storage.get" => {
            need("storage")?;
            let key = text_field(params, "key")?;
            Ok(storage_values(app, id)
                .get(&key)
                .cloned()
                .unwrap_or(Value::Null))
        }
        "storage.set" => {
            need("storage")?;
            let key = text_field(params, "key")?;
            if key.len() > 256 {
                return Err("That storage key is too long.".into());
            }
            let value = params.get("value").cloned().unwrap_or(Value::Null);
            if serde_json::to_string(&value)
                .map(|text| text.len())
                .unwrap_or(0)
                > MAX_STORAGE_VALUE
            {
                return Err("That stored value is too large.".into());
            }
            let mut values = storage_values(app, id);
            if value.is_null() {
                values.remove(&key);
            } else {
                values.insert(key, value);
            }
            write_storage(app, id, &values)?;
            Ok(Value::Null)
        }
        "ui.togglePanel" => {
            let open = toggle_panel(app, &tabs::focused_window(app), id)?;
            Ok(json!({ "open": open }))
        }
        "ui.openPage" => {
            let page_id = text_field(params, "pageId")?;
            let input = format!("ce://ext/{id}/{page_id}");
            let tab = tabs::create_tab(app, Some(input), false, None)?;
            Ok(json!({ "id": tab }))
        }
        "ui.notify" => {
            let message = text_field(params, "message").unwrap_or_default();
            let message = message.chars().take(240).collect::<String>();
            undo::notice(app, &tabs::focused_window(app), &message);
            Ok(Value::Null)
        }
        "ui.panelChrome" => Ok(panel_chrome_payload(&loaded)),
        "ui.setPanelSide" => {
            let side = text_field(params, "side")?;
            set_panel_layout(app, id, &side)?;
            Ok(json!({ "side": side }))
        }
        "ui.resizePanel" => resize_open_panel(app, &loaded),
        "ui.openPopout" => {
            open_popout(app, id)?;
            Ok(Value::Null)
        }
        "ui.closePopout" => {
            close_popout(app, id);
            Ok(Value::Null)
        }
        "ui.pageToolbar" => {
            let toolbar = resolve_page_toolbar(&loaded.record, &loaded.manifest)
                .ok_or("This extension has no page toolbar.")?;
            Ok(json!({ "side": toolbar.side, "thickness": toolbar.thickness }))
        }
        "ui.setPageToolbarSide" => {
            let side = text_field(params, "side")?;
            set_page_toolbar_side(app, id, &side)?;
            Ok(json!({ "side": side }))
        }
        "ui.closePageToolbar" => {
            set_page_toolbar_open(app, id, false)?;
            Ok(Value::Null)
        }
        _ => Err(format!("Unknown ce method {method}.")),
    }
}

fn need_desktop(app: &AppHandle, loaded: &Loaded) -> Result<(), String> {
    if !loaded.record.granted.iter().any(|item| item == "desktop") {
        return Err("This extension does not have the desktop permission.".into());
    }
    if !settings::get(app).automation.desktop_control {
        return Err("Desktop control is turned off in Automation settings.".into());
    }
    Ok(())
}

fn tabs_payload(app: &AppHandle) -> Value {
    let snapshot = tabs::snapshot(app);
    let active: HashSet<u32> = snapshot
        .windows
        .iter()
        .filter_map(|window| window.active_id)
        .collect();
    json!(
        snapshot
            .tabs
            .iter()
            .map(|tab| json!({
                "id": tab.id,
                "url": tab.url,
                "title": tab.title,
                "window": tab.window,
                "active": active.contains(&tab.id),
            }))
            .collect::<Vec<_>>()
    )
}

fn tab_view(app: &AppHandle, id: u32) -> Result<Webview, String> {
    app.get_webview(&format!("tab-{id}"))
        .ok_or_else(|| "That tab is not loaded.".into())
}

fn tab_field(params: &Value) -> Result<u32, String> {
    number_field(params, "tabId").or_else(|_| number_field(params, "id"))
}

fn number_field(params: &Value, name: &str) -> Result<u32, String> {
    params
        .get(name)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| format!("Missing {name}."))
}

fn float_field(params: &Value, name: &str) -> Result<f64, String> {
    params
        .get(name)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("Missing {name}."))
}

fn text_field(params: &Value, name: &str) -> Result<String, String> {
    params
        .get(name)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("Missing {name}."))
}

fn storage_values(app: &AppHandle, id: &str) -> BTreeMap<String, Value> {
    let file: StorageFile = files::read_json(&storage_path(app, id));
    file.values
}

fn write_storage(
    app: &AppHandle,
    id: &str,
    values: &BTreeMap<String, Value>,
) -> Result<(), String> {
    files::write_json(
        &storage_path(app, id),
        &StorageFile {
            values: values.clone(),
        },
    )
}

#[derive(Default, Serialize, Deserialize)]
struct StorageFile {
    values: BTreeMap<String, Value>,
}

fn require_enabled(app: &AppHandle, id: &str) -> Result<Loaded, String> {
    let id = canonical_id(id)?;
    let record = state(app)
        .into_iter()
        .find(|record| record.id == id)
        .ok_or("That extension is not installed.")?;
    if !record.enabled {
        return Err("That extension is turned off.".into());
    }
    let loaded = load_record(app, &record).ok_or("That extension folder could not be read.")?;
    if !grants_cover(&record.granted, &loaded.manifest.permissions) {
        return Err(
            "This extension asked for new permissions. Turn it on again to allow them.".into(),
        );
    }
    Ok(loaded)
}

fn extension_from_location(app: &AppHandle, raw: &str) -> Option<String> {
    if let Ok(url) = Url::parse(raw)
        && let Some(id) = virtual_id(&url)
        && state(app).iter().any(|record| record.id == id)
    {
        return Some(id);
    }
    let bare = raw.split(['?', '#']).next().unwrap_or(raw);
    if let Some(rest) = bare.strip_prefix("ce://ext/") {
        let id = rest.split('/').next().unwrap_or("");
        if valid_id(id) && state(app).iter().any(|record| record.id == id) {
            return Some(id.to_string());
        }
    }
    if bare == "ce://downloads" || bare.starts_with("ce://downloads/") {
        let id = settings::get(app).host_downloads;
        if !id.is_empty() && state(app).iter().any(|record| record.id == id) {
            return Some(id);
        }
    }
    None
}

fn virtual_id(url: &Url) -> Option<String> {
    if url.scheme() != "https" {
        return None;
    }
    let host = url.host_str()?.to_ascii_lowercase();
    let id = host.strip_suffix(HOST_SUFFIX)?;
    valid_id(id).then(|| id.to_string())
}

fn file_url(id: &str, path: &str) -> Result<Url, String> {
    let relative = safe_relative(path)?;
    let text = relative.to_string_lossy().replace('\\', "/");
    Url::parse(&format!("https://{id}{HOST_SUFFIX}/{text}")).map_err(|error| error.to_string())
}

const PANEL_FLOOR: u32 = 160;
const PANEL_CEILING: u32 = 1600;
const PANEL_MIN: u32 = 240;
const PANEL_MAX: u32 = 720;
const PANEL_DEFAULT: u32 = 360;
const PAGE_RESERVE: f64 = 320.0;

struct ResolvedPanel {
    edge: tabs::PanelEdge,
    resizable: bool,
    min_width: u32,
    max_width: u32,
    default_width: u32,
}

struct ResolvedPopout {
    path: String,
    title: String,
    width: f64,
    height: f64,
    min_width: f64,
    min_height: f64,
    resizable: bool,
}

impl Manifest {
    fn panel_path(&self) -> &str {
        match &self.panel {
            PanelSpec::Path(path) => path,
            PanelSpec::Detail(detail) => &detail.path,
        }
    }

    fn panel_detail(&self) -> PanelDetail {
        match &self.panel {
            PanelSpec::Path(_) => PanelDetail::default(),
            PanelSpec::Detail(detail) => detail.clone(),
        }
    }
}

fn resolve_panel(record: &HostRecord, manifest: &Manifest) -> ResolvedPanel {
    let detail = manifest.panel_detail();
    let side = match record.panel_side.as_str() {
        "left" | "right" => record.panel_side.as_str(),
        _ => match detail.side.as_str() {
            "left" => "left",
            _ => "right",
        },
    };
    let min_width = if detail.min_width > 0 {
        detail.min_width
    } else {
        PANEL_MIN
    };
    let max_width = if detail.max_width > 0 {
        detail.max_width
    } else {
        PANEL_MAX
    };
    let default_width = if detail.default_width > 0 {
        detail.default_width
    } else {
        PANEL_DEFAULT
    };
    normalize_panel(side, detail.resizable, min_width, max_width, default_width)
}

fn normalize_panel(
    side: &str,
    resizable: bool,
    min_width: u32,
    max_width: u32,
    default_width: u32,
) -> ResolvedPanel {
    let min_width = min_width.clamp(PANEL_FLOOR, PANEL_CEILING);
    let mut max_width = max_width.clamp(PANEL_FLOOR, PANEL_CEILING);
    if max_width < min_width {
        max_width = min_width;
    }
    let default_width = if resizable {
        default_width.clamp(min_width, max_width)
    } else {
        default_width.clamp(PANEL_FLOOR, PANEL_CEILING)
    };
    ResolvedPanel {
        edge: if side == "left" {
            tabs::PanelEdge::Left
        } else {
            tabs::PanelEdge::Right
        },
        resizable,
        min_width,
        max_width,
        default_width,
    }
}

fn publish_panel(layout: &ResolvedPanel) -> HostPanelLayout {
    HostPanelLayout {
        side: if layout.edge == tabs::PanelEdge::Left {
            "left".into()
        } else {
            "right".into()
        },
        resizable: layout.resizable,
        min_width: layout.min_width,
        max_width: layout.max_width,
        default_width: layout.default_width,
    }
}

fn panel_chrome_payload(loaded: &Loaded) -> Value {
    let layout = resolve_panel(&loaded.record, &loaded.manifest);
    json!({
        "side": if layout.edge == tabs::PanelEdge::Left { "left" } else { "right" },
        "resizable": layout.resizable,
        "minWidth": layout.min_width,
        "maxWidth": layout.max_width,
        "defaultWidth": layout.default_width,
    })
}

fn fitted_panel_width(app: &AppHandle, window: &str, wanted: f64, layout: &ResolvedPanel) -> f64 {
    let min = if layout.resizable {
        f64::from(layout.min_width)
    } else {
        f64::from(PANEL_FLOOR)
    };
    let max = if layout.resizable {
        f64::from(layout.max_width)
    } else {
        f64::from(PANEL_CEILING)
    }
    .max(min);
    let Some(host) = app.get_window(window) else {
        return wanted.clamp(min, max);
    };
    let scale = host.scale_factor().unwrap_or(1.0).max(0.01);
    let client = host
        .inner_size()
        .map(|size| size.width as f64 / scale)
        .unwrap_or(1200.0);
    let (left, right) = tabs::page_insets(app, window)
        .map(|(_, left, right, _)| (left, right))
        .unwrap_or((0.0, 0.0));
    let room = client - left.max(0.0) - right.max(0.0) - PAGE_RESERVE;
    let cap = if room.is_finite() {
        room.min(max).max(min)
    } else {
        max
    };
    if !wanted.is_finite() {
        return f64::from(PANEL_DEFAULT).clamp(min, cap);
    }
    wanted.clamp(min, cap)
}

/// Close, panel side, and toolbar side for a Chrome Web Store popup.
fn apply_popup_chrome(app: &AppHandle, label: &str, action: &str, side: &str) {
    let window = crate::popup::owner_of(label);
    match action {
        "close" => crate::popup::close_level(app, label),
        "panel" => {
            if side != "left" && side != "right" {
                return;
            }
            let Some(document) = crate::popup::extension_document(app, label) else {
                return;
            };
            let Some(id) = chrome_extension_id(&document) else {
                return;
            };
            if showing_store_panel(app, &window, &id) == side {
                tabs::set_side_panel(app, &window, tabs::SidePanel::None, 0.0);
                announce_popup_chrome(app, label);
                return;
            }
            remember_store_panel_side(app, &id, side);
            let Ok(url) = Url::parse(&document) else {
                return;
            };
            let edge = if side == "left" {
                tabs::PanelEdge::Left
            } else {
                tabs::PanelEdge::Right
            };
            if let Err(error) = open_store_panel(app, &window, &id, url, edge) {
                eprintln!("panel: {error}");
            }
            crate::popup::close_level(app, label);
        }
        "toolbar" => {
            if !matches!(side, "top" | "right" | "bottom" | "left") {
                return;
            }
            let _ = settings::update(app, |settings| {
                settings.toolbar_side = side.to_string();
            });
            announce_popup_chrome(app, label);
        }
        _ => {}
    }
}

fn apply_store_panel_chrome(app: &AppHandle, window: &str, id: &str, action: &str, side: &str) {
    if action == "close" {
        tabs::set_side_panel(app, window, tabs::SidePanel::None, 0.0);
        return;
    }
    if action == "resize" {
        resize_store_panel(app, window, id);
        return;
    }
    if action == "chrome" {
        if showing_store_panel(app, window, id).is_empty() {
            return;
        }
        post_store_panel_chrome(
            app,
            window,
            store_panel_edge(app, id),
            store_panel_resizable(app, id),
        );
        return;
    }
    if action != "side" || (side != "left" && side != "right") {
        return;
    }
    remember_store_panel_side(app, id, side);
    let edge = if side == "left" {
        tabs::PanelEdge::Left
    } else {
        tabs::PanelEdge::Right
    };
    let width = store_panel_width(app, id);
    tabs::open_host_panel(app, window, id, width, edge);
    let label = tabs::host_panel_label(window);
    if let Some(wanted) = panel_target_url(&label)
        && let Ok(url) = Url::parse(&wanted)
    {
        reveal_panel_document(app, &label, &url);
    }
    post_store_panel_chrome(app, window, edge, store_panel_resizable(app, id));
}

/// Opens a store extension's popup page in the side panel, or closes it when that panel is already showing.
pub fn toggle_store_extension_panel(
    app: &AppHandle,
    window: &str,
    id: &str,
) -> Result<bool, String> {
    if !showing_store_panel(app, window, id).is_empty() {
        tabs::set_side_panel(app, window, tabs::SidePanel::None, 0.0);
        return Ok(false);
    }
    let url = crate::extensions::store_panel_url(app, id)?;
    open_store_panel(app, window, id, url, store_panel_edge(app, id))?;
    Ok(true)
}

/// Moves an open store panel onto the side, width, and resize setting saved for that extension.
pub fn place_store_extension_panel(app: &AppHandle, window: &str, id: &str) -> Result<(), String> {
    if showing_store_panel(app, window, id).is_empty() {
        return Ok(());
    }
    let url = crate::extensions::store_panel_url(app, id)?;
    open_store_panel(app, window, id, url, store_panel_edge(app, id))?;
    Ok(())
}

fn store_panel_edge(app: &AppHandle, id: &str) -> tabs::PanelEdge {
    match settings::get(app)
        .extension_panel_side
        .get(id)
        .map(String::as_str)
    {
        Some("left") => tabs::PanelEdge::Left,
        _ => tabs::PanelEdge::Right,
    }
}

const STORE_PANEL_DEFAULT: u32 = 380;

fn store_panel_width(app: &AppHandle, id: &str) -> f64 {
    let width = settings::get(app)
        .extension_panel_width
        .get(id)
        .copied()
        .unwrap_or(STORE_PANEL_DEFAULT);
    f64::from(width.clamp(PANEL_MIN, PANEL_MAX))
}

fn store_panel_resizable(app: &AppHandle, id: &str) -> bool {
    settings::get(app)
        .extension_panel_resizable
        .get(id)
        .copied()
        .unwrap_or(true)
}

fn remember_store_panel_width(app: &AppHandle, id: &str, width: u32) {
    if id.is_empty() {
        return;
    }
    let width = width.clamp(PANEL_MIN, PANEL_MAX);
    let _ = settings::update(app, |settings| {
        if width == STORE_PANEL_DEFAULT {
            settings.extension_panel_width.remove(id);
        } else {
            settings.extension_panel_width.insert(id.to_string(), width);
        }
    });
}

fn resize_store_panel(app: &AppHandle, window: &str, id: &str) {
    if showing_store_panel(app, window, id).is_empty() || !store_panel_resizable(app, id) {
        return;
    }
    let Ok(width) = crate::automation::panel::track_column(
        app,
        window,
        crate::automation::panel::ColumnDrag {
            edge: store_panel_edge(app, id),
            min: f64::from(PANEL_MIN),
            max: f64::from(PANEL_MAX),
            remember_agent: false,
            fallback: store_panel_width(app, id),
        },
    ) else {
        return;
    };
    let rounded = width
        .round()
        .clamp(f64::from(PANEL_MIN), f64::from(PANEL_MAX)) as u32;
    remember_store_panel_width(app, id, rounded);
}

fn remember_store_panel_side(app: &AppHandle, id: &str, side: &str) {
    if id.is_empty() || (side != "left" && side != "right") {
        return;
    }
    let _ = settings::update(app, |settings| {
        if side == "right" {
            settings.extension_panel_side.remove(id);
        } else {
            settings
                .extension_panel_side
                .insert(id.to_string(), side.to_string());
        }
    });
}

fn open_store_panel(
    app: &AppHandle,
    window: &str,
    id: &str,
    url: Url,
    edge: tabs::PanelEdge,
) -> Result<(), String> {
    let label = tabs::host_panel_label(window);
    eprintln!("panel: open store extension {id} in {window} at {url}");
    ensure_child(app, window, &label, url.clone()).map_err(|error| {
        eprintln!("panel: {label} was not created: {error}");
        error
    })?;
    remember_panel_target(&label, url.as_str(), true);
    let width = store_panel_width(app, id);
    tabs::open_host_panel(app, window, id, width, edge);
    reveal_panel_document(app, &label, &url);
    post_store_panel_chrome(app, window, edge, store_panel_resizable(app, id));
    Ok(())
}

fn post_store_panel_chrome(app: &AppHandle, window: &str, edge: tabs::PanelEdge, resizable: bool) {
    post_label(
        app,
        &tabs::host_panel_label(window),
        &json!({
            "type": "ce-panel",
            "side": if edge == tabs::PanelEdge::Left { "left" } else { "right" },
            "resizable": resizable,
        }),
    );
}

fn chrome_extension_id(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    if parsed.scheme() != "chrome-extension" {
        return None;
    }
    parsed
        .host_str()
        .filter(|id| !id.is_empty())
        .map(str::to_string)
}

fn showing_store_panel(app: &AppHandle, window: &str, id: &str) -> String {
    if id.is_empty()
        || tabs::side_panel(app, window) != tabs::SidePanel::Host
        || tabs::host_panel_extension(app, window) != id
        || tabs::agent_panel_width(app, window) <= 0.0
    {
        return String::new();
    }
    match tabs::host_panel_edge(app, window) {
        tabs::PanelEdge::Left => "left".to_string(),
        tabs::PanelEdge::Right => "right".to_string(),
    }
}

/// Tells a store popup which toolbar side and panel edge are on.
pub fn announce_popup_chrome(app: &AppHandle, label: &str) {
    let window = crate::popup::owner_of(label);
    let document = crate::popup::extension_document(app, label).unwrap_or_default();
    let id = chrome_extension_id(&document).unwrap_or_default();
    let toolbar = settings::get(app).toolbar_side;
    let panel = showing_store_panel(app, &window, &id);
    post_label(
        app,
        label,
        &json!({
            "type": "ce-popup-chrome",
            "toolbar": toolbar,
            "panel": panel,
        }),
    );
}

fn post_panel_chrome(app: &AppHandle, window: &str, layout: &ResolvedPanel) {
    post_label(
        app,
        &tabs::host_panel_label(window),
        &json!({
            "type": "ce-panel",
            "side": if layout.edge == tabs::PanelEdge::Left { "left" } else { "right" },
            "resizable": layout.resizable,
        }),
    );
}

fn close_panels_for(app: &AppHandle, id: &str) {
    for window in tabs::snapshot(app).windows {
        if tabs::host_panel_extension(app, &window.label) == id {
            tabs::set_side_panel(app, &window.label, tabs::SidePanel::None, 0.0);
        }
    }
}

pub fn set_panel_layout(app: &AppHandle, id: &str, side: &str) -> Result<(), String> {
    if side != "left" && side != "right" {
        return Err("The panel side must be left or right.".into());
    }
    let id = canonical_id(id)?;
    let mut records = state(app);
    let record = records
        .iter_mut()
        .find(|record| record.id == id)
        .ok_or("That extension is not installed.")?;
    let loaded = load_record(app, record).ok_or("That extension folder could not be read.")?;
    if loaded.manifest.panel_path().is_empty() {
        return Err("That extension has no side panel.".into());
    }
    if record.panel_side == side {
        return Ok(());
    }
    record.panel_side = side.to_string();
    save_records(app, records)?;
    apply_open_panel(app, &id);
    Ok(())
}

fn apply_open_panel(app: &AppHandle, id: &str) {
    let Ok(loaded) = require_enabled(app, id) else {
        return;
    };
    let layout = resolve_panel(&loaded.record, &loaded.manifest);
    let wanted = if layout.resizable && loaded.record.panel_width > 0 {
        f64::from(loaded.record.panel_width)
    } else {
        f64::from(layout.default_width)
    };
    let panel_path = loaded.manifest.panel_path().to_string();
    let url = if panel_path.is_empty() {
        None
    } else {
        file_url(id, &panel_path).ok()
    };
    for window in tabs::snapshot(app).windows {
        if tabs::host_panel_extension(app, &window.label) != id {
            continue;
        }
        let width = fitted_panel_width(app, &window.label, wanted, &layout);
        tabs::open_host_panel(app, &window.label, id, width, layout.edge);
        if let Some(url) = &url {
            let label = tabs::host_panel_label(&window.label);
            remember_panel_target(&label, url.as_str(), false);
            reveal_panel_document(app, &label, url);
        }
        post_panel_chrome(app, &window.label, &layout);
    }
}

fn resize_open_panel(app: &AppHandle, loaded: &Loaded) -> Result<Value, String> {
    let layout = resolve_panel(&loaded.record, &loaded.manifest);
    if !layout.resizable {
        return Err("This panel is not resizable.".into());
    }
    let window = tabs::focused_window(app);
    if tabs::host_panel_extension(app, &window) != loaded.record.id {
        return Err("This extension's panel is not open.".into());
    }
    let width = crate::automation::panel::track_column(
        app,
        &window,
        crate::automation::panel::ColumnDrag {
            edge: layout.edge,
            min: f64::from(layout.min_width),
            max: f64::from(layout.max_width),
            remember_agent: false,
            fallback: f64::from(layout.default_width),
        },
    )?;
    let rounded = width.round().clamp(0.0, u32::MAX as f64) as u32;
    let mut records = state(app);
    let changed = records
        .iter_mut()
        .find(|record| record.id == loaded.record.id)
        .is_some_and(|record| {
            if record.panel_width == rounded {
                false
            } else {
                record.panel_width = rounded;
                true
            }
        });
    if changed {
        save_records(app, records)?;
    }
    Ok(json!({ "width": width }))
}

fn resolve_popout(manifest: &Manifest) -> Option<ResolvedPopout> {
    let detail = match &manifest.popout {
        None => return None,
        Some(PopoutSpec::Path(path)) => {
            if path.trim().is_empty() {
                return None;
            }
            PopoutDetail {
                path: path.clone(),
                ..PopoutDetail::default()
            }
        }
        Some(PopoutSpec::Detail(detail)) => {
            if detail.path.trim().is_empty() {
                return None;
            }
            detail.clone()
        }
    };
    let width = if detail.width == 0 { 480 } else { detail.width }.clamp(240, 2400);
    let height = if detail.height == 0 {
        640
    } else {
        detail.height
    }
    .clamp(180, 1600);
    let min_width = if detail.min_width == 0 {
        320
    } else {
        detail.min_width
    }
    .clamp(240, width);
    let min_height = if detail.min_height == 0 {
        240
    } else {
        detail.min_height
    }
    .clamp(180, height);
    Some(ResolvedPopout {
        path: detail.path,
        title: detail.title,
        width: f64::from(width),
        height: f64::from(height),
        min_width: f64::from(min_width),
        min_height: f64::from(min_height),
        resizable: detail.resizable,
    })
}

fn popout_label(id: &str) -> String {
    format!("cext-out-{}", id.replace('.', "_"))
}

pub fn open_popout(app: &AppHandle, id: &str) -> Result<(), String> {
    let loaded = require_enabled(app, id)?;
    let popout = resolve_popout(&loaded.manifest).ok_or("That extension has no popout window.")?;
    let label = popout_label(&loaded.record.id);
    if let Some(existing) = app.get_window(&label) {
        let _ = existing.unminimize();
        let _ = existing.show();
        let _ = existing.set_focus();
        return Ok(());
    }
    let url = file_url(&loaded.record.id, &popout.path)?;
    let title = if popout.title.trim().is_empty() {
        loaded.manifest.name.clone()
    } else {
        popout.title.clone()
    };
    let factor = settings::get(app).ui_zoom_factor();
    let window = WindowBuilder::new(app, &label)
        .title(&title)
        .inner_size(popout.width * factor, popout.height * factor)
        .min_inner_size(popout.min_width * factor, popout.min_height * factor)
        .resizable(popout.resizable)
        .decorations(true)
        .focused(true)
        .shadow(true)
        .build()
        .map_err(|error| error.to_string())?;
    crate::profiles::apply_window_icon(app, &window);
    let owner = tabs::focused_window(app);
    if let Some(owner_window) = app.get_window(&owner)
        && let Ok(position) = owner_window.outer_position()
    {
        let _ = window.set_position(PhysicalPosition::new(position.x + 48, position.y + 48));
    }
    let (r, g, b) = settings::popup_surface(app);
    let _ = window.set_background_color(Some(Color(r, g, b, 255)));
    let size = match window.inner_size() {
        Ok(size) => size,
        Err(error) => {
            let _ = window.close();
            return Err(error.to_string());
        }
    };
    let host = app.clone();
    let builder = tabs::base_builder(app, &label, WebviewUrl::External(url))
        .auto_resize()
        .background_color(Color(r, g, b, 255))
        .focused(true)
        .on_document_title_changed(|webview, title| {
            let title = title.trim();
            if title.is_empty() || title.eq_ignore_ascii_case("about:blank") {
                return;
            }
            let _ = webview.window().set_title(title);
        })
        .on_new_window(move |url, features| tabs::new_window(&host, None, url, features));
    let webview = match window.add_child(builder, PhysicalPosition::new(0, 0), size) {
        Ok(webview) => webview,
        Err(error) => {
            let _ = window.close();
            return Err(error.to_string());
        }
    };
    let _ = webview.set_zoom(settings::get(app).ui_zoom_factor());
    attach_webview(app, &webview);
    let _ = window.set_focus();
    Ok(())
}

fn close_popout(app: &AppHandle, id: &str) {
    if let Some(window) = app.get_window(&popout_label(id)) {
        let _ = window.close();
    }
}

fn read_manifest(root: &Path) -> Result<Manifest, String> {
    let path = root.join("manifest.json");
    let text =
        fs::read_to_string(&path).map_err(|_| format!("Could not read {}.", path.display()))?;
    serde_json::from_str(&text).map_err(|error| format!("Could not read manifest.json. {error}"))
}

fn validate_manifest(manifest: &Manifest) -> Result<(), String> {
    if manifest.manifest_version != 1 {
        return Err("This host extension needs manifestVersion 1.".into());
    }
    if !valid_id(&manifest.id) {
        return Err(
            "The extension id must be 1 to 64 lowercase letters, digits, dots, or hyphens.".into(),
        );
    }
    if manifest.name.trim().is_empty() {
        return Err("The extension needs a name.".into());
    }
    if manifest.version.trim().is_empty() {
        return Err("The extension needs a version.".into());
    }
    for permission in &manifest.permissions {
        if !PERMISSIONS.contains(&permission.as_str()) {
            return Err(format!("Unknown permission {permission}."));
        }
    }
    for path in manifest_paths(manifest) {
        safe_relative(&path)?;
    }
    for page in &manifest.pages {
        if page.id.is_empty() || page.id.contains('/') || page.id.contains('\\') {
            return Err("Each page needs an id without slashes.".into());
        }
    }
    Ok(())
}

fn manifest_paths(manifest: &Manifest) -> Vec<String> {
    let mut paths = Vec::new();
    if !manifest.icon.is_empty() {
        paths.push(manifest.icon.clone());
    }
    if !manifest.background.is_empty() {
        paths.push(manifest.background.clone());
    }
    if !manifest.panel_path().is_empty() {
        paths.push(manifest.panel_path().to_string());
    }
    if let Some(popout) = resolve_popout(manifest) {
        paths.push(popout.path);
    }
    if !manifest.bookmarks_bar.is_empty() {
        paths.push(manifest.bookmarks_bar.clone());
    }
    if !manifest.downloads.is_empty() {
        paths.push(manifest.downloads.clone());
    }
    if let Some(toolbar) = &manifest.toolbar
        && !toolbar.popup.is_empty()
    {
        paths.push(toolbar.popup.clone());
    }
    if let Some(page_toolbar) = resolve_page_toolbar(&HostRecord::default(), manifest) {
        paths.push(page_toolbar.path);
    }
    for page in &manifest.pages {
        paths.push(page.path.clone());
    }
    for script in &manifest.content_scripts {
        paths.extend(script.js.iter().cloned());
        paths.extend(script.css.iter().cloned());
    }
    paths
}

fn safe_relative(path: &str) -> Result<PathBuf, String> {
    if path.is_empty() || path.contains('\0') {
        return Err("That file path is empty.".into());
    }
    let trimmed = path.trim_start_matches(['/', '\\']);
    let relative = Path::new(trimmed);
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err("That file path is not inside the extension.".into());
    }
    Ok(relative.to_path_buf())
}

pub fn valid_id(id: &str) -> bool {
    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return false;
    }
    if id.len() > 64 {
        return false;
    }
    id.chars().all(|character| {
        character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || character == '.'
            || character == '-'
    }) && !id.ends_with('.')
        && !id.ends_with('-')
        && !id.contains("..")
}

fn canonical_id(id: &str) -> Result<String, String> {
    let id = id.trim();
    if valid_id(id) {
        Ok(id.to_string())
    } else {
        Err("That extension id is not valid.".into())
    }
}

fn grants_cover(granted: &[String], requested: &[String]) -> bool {
    requested
        .iter()
        .all(|permission| granted.iter().any(|item| item == permission))
}

fn same_dir(left: &Path, right: &Path) -> Result<bool, String> {
    let left = fs::canonicalize(left).unwrap_or_else(|_| left.to_path_buf());
    let right = if right.exists() {
        fs::canonicalize(right).unwrap_or_else(|_| right.to_path_buf())
    } else {
        right.to_path_buf()
    };
    Ok(left == right)
}

fn icon_data(root: &Path, icon: &str) -> Option<String> {
    if icon.is_empty() {
        return None;
    }
    let path = root.join(safe_relative(icon).ok()?);
    let bytes = fs::read(&path).ok()?;
    if bytes.len() > MAX_ICON {
        return None;
    }
    let kind = match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
    {
        "svg" => "image/svg+xml",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "image/png",
    };
    Some(format!(
        "data:{kind};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

fn listeners() -> &'static Mutex<HashSet<String>> {
    static LABELS: std::sync::OnceLock<Mutex<HashSet<String>>> = std::sync::OnceLock::new();
    LABELS.get_or_init(Default::default)
}

fn bridges() -> &'static Mutex<HashSet<String>> {
    static LABELS: std::sync::OnceLock<Mutex<HashSet<String>>> = std::sync::OnceLock::new();
    LABELS.get_or_init(Default::default)
}

fn script_slots() -> &'static Mutex<HashMap<String, ScriptSlot>> {
    static SLOTS: std::sync::OnceLock<Mutex<HashMap<String, ScriptSlot>>> =
        std::sync::OnceLock::new();
    SLOTS.get_or_init(Default::default)
}

fn broadcasts() -> &'static Mutex<HashSet<String>> {
    static INFLIGHT: std::sync::OnceLock<Mutex<HashSet<String>>> = std::sync::OnceLock::new();
    INFLIGHT.get_or_init(Default::default)
}

struct HostMapping {
    key: String,
    ids: Vec<String>,
}

fn mappings() -> &'static Mutex<HashMap<String, HostMapping>> {
    static MAPPINGS: std::sync::OnceLock<Mutex<HashMap<String, HostMapping>>> =
        std::sync::OnceLock::new();
    MAPPINGS.get_or_init(Default::default)
}

struct ContentCache {
    generation: u64,
    stamp: String,
    script: String,
}

fn content_generation() -> &'static AtomicU64 {
    static GENERATION: AtomicU64 = AtomicU64::new(1);
    &GENERATION
}

fn content_cache() -> &'static Mutex<Option<ContentCache>> {
    static CACHE: std::sync::OnceLock<Mutex<Option<ContentCache>>> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

fn invalidate_content_cache() {
    content_generation().fetch_add(1, Ordering::AcqRel);
}

/// Assembled content-script text. Disk is read only after the catalog changes.
fn cached_content(app: &AppHandle) -> (String, String) {
    let generation = content_generation().load(Ordering::Acquire);
    if let Some(hit) = content_cache().lock().expect("host content cache").as_ref()
        && hit.generation == generation
    {
        return (hit.stamp.clone(), hit.script.clone());
    }
    let script = content_script(app);
    let stamp = content_stamp(&script);
    if content_generation().load(Ordering::Acquire) == generation {
        *content_cache().lock().expect("host content cache") = Some(ContentCache {
            generation,
            stamp: stamp.clone(),
            script: script.clone(),
        });
    }
    (stamp, script)
}

pub fn attach_webview(app: &AppHandle, webview: &Webview) {
    let app = app.clone();
    let label = webview.label().to_string();
    let _ = webview.with_webview(move |platform| unsafe {
        let Ok(core) = platform.controller().CoreWebView2() else {
            return;
        };
        prepare_core(&app, &core, &label);
        let mut labels = listeners().lock().expect("host listeners");
        if !labels.insert(label.clone()) {
            return;
        }
        drop(labels);
        let listen_app = app.clone();
        let listen_label = label.clone();
        let handler = WebMessageReceivedEventHandler::create(Box::new(move |_, args| {
            let Some(args) = args else {
                return Ok(());
            };
            let json = webview2::read_pwstr(|value| unsafe { args.WebMessageAsJson(value) });
            let source = webview2::read_pwstr(|value| unsafe { args.Source(value) });
            handle_message(&listen_app, None, &listen_label, &source, &json);
            Ok(())
        }));
        let mut token = 0i64;
        if core.add_WebMessageReceived(&handler, &mut token).is_err() {
            listeners().lock().expect("host listeners").remove(&label);
        }
    });
}

/// Maps package folders and installs `window.ce` on a webview that already has a core.
pub fn prepare_core(app: &AppHandle, core: &ICoreWebView2, label: &str) {
    sync_mappings(app, core, label);
    ensure_bridge(core, label);
    sync_content_script(app, core, label);
}

fn refresh_webviews(app: &AppHandle) {
    let _ = cached_content(app);
    let labels: Vec<String> = app.webviews().keys().cloned().collect();
    for label in labels {
        let Some(view) = app.get_webview(&label) else {
            continue;
        };
        let app = app.clone();
        let _ = view.with_webview(move |platform| unsafe {
            if let Ok(core) = platform.controller().CoreWebView2() {
                prepare_core(&app, &core, &label);
            }
        });
    }
}

fn sync_mappings(app: &AppHandle, core: &ICoreWebView2, label: &str) {
    let Ok(core3) = core.clone().cast::<ICoreWebView2_3>() else {
        return;
    };
    let mut enabled: Vec<(String, PathBuf)> = state(app)
        .iter()
        .filter(|record| record.enabled)
        .map(|record| (record.id.clone(), package_dir(app, &record.id)))
        .collect();
    enabled.sort_by(|left, right| left.0.cmp(&right.0));
    let mut key = String::new();
    for (id, folder) in &enabled {
        key.push_str(id);
        key.push('\n');
        key.push_str(&folder.to_string_lossy());
        key.push('\n');
    }
    let next_ids: Vec<String> = enabled.iter().map(|(id, _)| id.clone()).collect();
    let previous = {
        let mapped = mappings().lock().expect("host mappings");
        if mapped.get(label).is_some_and(|state| state.key == key) {
            return;
        }
        mapped
            .get(label)
            .map(|state| state.ids.clone())
            .unwrap_or_default()
    };
    for id in &previous {
        if next_ids.iter().any(|next| next == id) {
            continue;
        }
        let host = HSTRING::from(format!("{id}{HOST_SUFFIX}"));
        let _ = unsafe { core3.ClearVirtualHostNameToFolderMapping(&host) };
    }
    for (id, folder) in &enabled {
        let host = HSTRING::from(format!("{id}{HOST_SUFFIX}"));
        let folder = HSTRING::from(folder.to_string_lossy().as_ref());
        let _ = unsafe {
            core3.SetVirtualHostNameToFolderMapping(
                &host,
                &folder,
                COREWEBVIEW2_HOST_RESOURCE_ACCESS_KIND_DENY_CORS,
            )
        };
    }
    mappings()
        .lock()
        .expect("host mappings")
        .insert(label.to_string(), HostMapping { key, ids: next_ids });
}

fn ensure_bridge(core: &ICoreWebView2, label: &str) {
    let mut installed = bridges().lock().expect("host bridge");
    if !installed.insert(label.to_string()) {
        return;
    }
    drop(installed);
    let script = HSTRING::from(BRIDGE_JS);
    let label = label.to_string();
    let failed = label.clone();
    let handler = AddScriptToExecuteOnDocumentCreatedCompletedHandler::create(Box::new(
        move |result, _script_id| {
            if result.is_err() {
                bridges().lock().expect("host bridge").remove(&label);
            }
            Ok(())
        },
    ));
    if unsafe { core.AddScriptToExecuteOnDocumentCreated(&script, &handler) }.is_err() {
        bridges().lock().expect("host bridge").remove(&failed);
    }
}

fn sync_content_script(app: &AppHandle, core: &ICoreWebView2, label: &str) {
    let (stamp, script) = cached_content(app);
    {
        let mut slots = script_slots().lock().expect("host content script");
        let slot = slots.entry(label.to_string()).or_default();
        if slot.inflight {
            return;
        }
        if slot.stamp.as_deref() == Some(stamp.as_str()) && slot.script_id.is_some() {
            return;
        }
        if let Some(script_id) = slot.script_id.take() {
            let _ =
                unsafe { core.RemoveScriptToExecuteOnDocumentCreated(&HSTRING::from(script_id)) };
        }
        slot.inflight = true;
        slot.stamp = Some(stamp);
    }
    let label_key = label.to_string();
    let handler = AddScriptToExecuteOnDocumentCreatedCompletedHandler::create(Box::new(
        move |result, script_id| {
            let mut slots = script_slots().lock().expect("host content script");
            let slot = slots.entry(label_key).or_default();
            slot.inflight = false;
            if result.is_ok() {
                slot.script_id = Some(script_id);
            } else {
                slot.stamp = None;
            }
            Ok(())
        },
    ));
    if unsafe { core.AddScriptToExecuteOnDocumentCreated(&HSTRING::from(script), &handler) }
        .is_err()
    {
        let mut slots = script_slots().lock().expect("host content script");
        if let Some(slot) = slots.get_mut(label) {
            slot.inflight = false;
            slot.stamp = None;
        }
    }
}

fn content_stamp(script: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(script.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn content_script(app: &AppHandle) -> String {
    let mut entries = Vec::new();
    for record in state(app).into_iter().filter(|record| record.enabled) {
        let Some(loaded) = load_record(app, &record) else {
            continue;
        };
        for script in &loaded.manifest.content_scripts {
            let mut code = String::new();
            for file in &script.js {
                let Ok(relative) = safe_relative(file) else {
                    continue;
                };
                let Ok(text) = fs::read_to_string(loaded.root.join(relative)) else {
                    continue;
                };
                if text.len() > MAX_SCRIPT {
                    continue;
                }
                code.push_str(&text);
                code.push('\n');
            }
            let mut css = String::new();
            for file in &script.css {
                let Ok(relative) = safe_relative(file) else {
                    continue;
                };
                let Ok(text) = fs::read_to_string(loaded.root.join(relative)) else {
                    continue;
                };
                if text.len() > MAX_SCRIPT {
                    continue;
                }
                css.push_str(&text);
                css.push('\n');
            }
            if code.is_empty() && css.is_empty() {
                continue;
            }
            entries.push(json!({
                "extensionId": record.id,
                "matches": script.matches,
                "code": code,
                "css": css,
            }));
        }
    }
    let payload = serde_json::to_string(&entries).unwrap_or_else(|_| "[]".into());
    format!("{CONTENT_JS_START}{payload}{CONTENT_JS_END}")
}

/// Each WebView2 is its own Chromium window. A store popover, side panel, options page, or
/// popout therefore answers `chrome.tabs` and `chrome.windows` with itself. This script runs
/// only on `chrome-extension:` documents. `query({ active: true, currentWindow: true })` and
/// `getCurrent` still return the open page, because a toolbar popup is not a tab and password
/// managers use whichever call returns a page first. Other queries use the browser's tab strip,
/// one window per browser window.
pub const EXTENSION_TAB_JS: &str = r##"(function () {
  if (window.__ceExtensionTabs) return;
  var protocol = "";
  try { protocol = String(location.protocol || ""); } catch (error) { protocol = ""; }
  if (protocol !== "chrome-extension:") return;
  window.__ceExtensionTabs = true;
  var pending = {};
  var next = 1;
  function host() {
    var target = window.__ceHost && window.__ceHost.postMessage ? window.__ceHost : null;
    if (!target) {
      try { target = window.chrome && window.chrome.webview; } catch (error) { target = null; }
    }
    return target && target.postMessage ? target : null;
  }
  function onMessage(event) {
    var data = event && event.data !== undefined ? event.data : event;
    if (typeof data === "string") {
      try { data = JSON.parse(data); } catch (error) { return; }
    }
    if (!data || (data.type !== "ce-active-tab" && data.type !== "ce-tabs")) return;
    var wait = pending[data.token];
    if (!wait) return;
    delete pending[data.token];
    wait(data);
  }
  function listen() {
    var target = window.chrome && window.chrome.webview;
    if (!target || !target.addEventListener || window.__ceExtensionTabsListening) return;
    window.__ceExtensionTabsListening = true;
    target.addEventListener("message", onMessage);
  }
  function ask() {
    return new Promise(function (resolve) {
      listen();
      var target = host();
      if (!target) {
        resolve(null);
        return;
      }
      var token = next++;
      var timer = setTimeout(function () {
        delete pending[token];
        resolve(null);
      }, 1500);
      pending[token] = function (page) {
        clearTimeout(timer);
        resolve(page && page.url ? page : null);
      };
      target.postMessage(JSON.stringify({ type: "ce-chrome", action: "active-tab", token: token }));
    });
  }
  function askSnapshot() {
    return new Promise(function (resolve) {
      listen();
      var target = host();
      if (!target) {
        resolve(null);
        return;
      }
      var token = next++;
      var timer = setTimeout(function () {
        delete pending[token];
        resolve(null);
      }, 1500);
      pending[token] = function (data) {
        clearTimeout(timer);
        resolve(data && data.type === "ce-tabs" ? data : null);
      };
      target.postMessage(JSON.stringify({ type: "ce-chrome", action: "tabs", token: token }));
    });
  }
  function settle(promise, callback, empty) {
    if (typeof callback === "function") {
      promise.then(function (value) { callback(value); }, function () { callback(empty); });
      return undefined;
    }
    return promise;
  }
  function matchPattern(pattern, url) {
    if (!pattern || !url) return false;
    if (pattern === "<all_urls>") return /^[a-z][a-z0-9+.-]*:/i.test(url);
    var escaped = String(pattern).replace(/[.+?^${}()|[\]\\]/g, "\\$&").replace(/\\\*/g, ".*");
    try { return new RegExp("^" + escaped + "$").test(url); } catch (error) { return false; }
  }
  function matches(tab, info, snap) {
    if (!info) return true;
    if (info.active === true && !tab.active) return false;
    if (info.active === false && tab.active) return false;
    if (info.pinned === true && !tab.pinned) return false;
    if (info.pinned === false && tab.pinned) return false;
    if (info.highlighted === true && !tab.active) return false;
    if (info.audible === true && !tab.audible) return false;
    if (info.audible === false && tab.audible) return false;
    if (info.muted === true && !(tab.mutedInfo && tab.mutedInfo.muted)) return false;
    if (info.discarded === true && !tab.discarded) return false;
    if (typeof info.index === "number" && tab.index !== info.index) return false;
    if (info.status && tab.status !== info.status) return false;
    if (info.title && String(tab.title || "").indexOf(info.title) === -1) return false;
    if (info.windowType && info.windowType !== "normal") return false;
    if (info.url) {
      var patterns = Array.isArray(info.url) ? info.url : [info.url];
      var url = tab.url || "";
      var hit = false;
      for (var i = 0; i < patterns.length; i++) if (matchPattern(patterns[i], url)) hit = true;
      if (!hit) return false;
    }
    var current = snap ? snap.currentWindowId : tab.windowId;
    var focused = snap ? snap.lastFocusedWindowId : tab.windowId;
    var windowId = info.windowId;
    if (window.chrome && chrome.windows && windowId === chrome.windows.WINDOW_ID_CURRENT) windowId = current;
    if (typeof windowId === "number" && tab.windowId !== windowId) return false;
    if (info.currentWindow === true && tab.windowId !== current) return false;
    if (info.lastFocusedWindow === true && tab.windowId !== focused) return false;
    return true;
  }
  function buildTabs(snap, chromium) {
    var used = {};
    return (snap.tabs || []).map(function (tab) {
      var match = null;
      for (var i = 0; i < chromium.length; i++) {
        if (used[chromium[i].id]) continue;
        if (same(chromium[i].url, tab.url)) {
          match = chromium[i];
          used[match.id] = true;
          break;
        }
      }
      if (match) {
        return Object.assign({}, match, {
          active: !!tab.active,
          highlighted: !!tab.active,
          windowId: tab.windowId,
          index: tab.index,
          pinned: !!tab.pinned,
          title: tab.title || match.title,
          url: tab.url || match.url,
          status: tab.status || match.status,
          audible: !!tab.audible,
          discarded: !!tab.discarded,
          mutedInfo: { muted: !!tab.muted }
        });
      }
      return {
        id: 1000000 + (tab.id || 0),
        windowId: tab.windowId,
        index: tab.index,
        url: tab.url || "",
        title: tab.title || "",
        active: !!tab.active,
        highlighted: !!tab.active,
        pinned: !!tab.pinned,
        status: tab.status || "complete",
        audible: !!tab.audible,
        discarded: !!tab.discarded,
        mutedInfo: { muted: !!tab.muted },
        incognito: false,
        autoDiscardable: true,
        groupId: -1
      };
    });
  }
  function model() {
    return askSnapshot().then(function (snap) {
      if (!snap) return null;
      return callQuery(installedQuery, {}).then(function (chromium) {
        return { snap: snap, tabs: buildTabs(snap, chromium || []) };
      });
    });
  }
  var installedQuery = null;
  function wantsPage(info) {
    if (!info || info.active !== true) return false;
    if (info.currentWindow === true || info.lastFocusedWindow === true) return true;
    var currentId = window.chrome && chrome.windows && chrome.windows.WINDOW_ID_CURRENT;
    return info.windowId === currentId;
  }
  function same(left, right) {
    if (!left || !right) return false;
    if (left === right) return true;
    try {
      var a = new URL(left);
      var b = new URL(right);
      var path = function (value) { return value.pathname.replace(/\/+$/, "") || "/"; };
      return a.origin === b.origin && path(a) === path(b) && a.search === b.search && a.hash === b.hash;
    } catch (error) {
      return false;
    }
  }
  function callQuery(query, info) {
    return new Promise(function (resolve) {
      var settled = false;
      function finish(tabs) {
        if (settled) return;
        settled = true;
        resolve(tabs || []);
      }
      var result;
      try {
        result = query(info, function (tabs) { finish(tabs); });
      } catch (error) {
        finish([]);
        return;
      }
      if (result && typeof result.then === "function") {
        result.then(function (tabs) { finish(tabs); }, function () { finish([]); });
      }
    });
  }
  function pageTab(query, page) {
    if (!page || !page.url) return Promise.resolve(null);
    return callQuery(query, {}).then(function (all) {
      for (var i = 0; i < all.length; i++) {
        if (same(all[i].url, page.url)) return Object.assign({}, all[i], { active: true });
      }
      return null;
    });
  }
  function install() {
    if (!window.chrome || !chrome.tabs || typeof chrome.tabs.query !== "function") return false;
    if (chrome.tabs.query.__ceExtensionTabs) return true;
    listen();
    var query = chrome.tabs.query.bind(chrome.tabs);
    installedQuery = query;
    function run(info) {
      if (wantsPage(info)) {
        return callQuery(query, info).then(function (tabs) {
          return ask().then(function (page) {
            return pageTab(query, page).then(function (tab) {
              return tab ? [tab] : tabs;
            });
          });
        });
      }
      return model().then(function (built) {
        if (!built) return callQuery(query, info);
        return built.tabs.filter(function (tab) { return matches(tab, info, built.snap); });
      });
    }
    function wrapped(info, callback) {
      var promise = run(info || {});
      if (typeof callback === "function") {
        promise.then(function (tabs) { callback(tabs); }, function () { callback([]); });
        return undefined;
      }
      return promise;
    }
    wrapped.__ceExtensionTabs = true;
    try { chrome.tabs.query = wrapped; } catch (error) { return false; }
    if (typeof chrome.tabs.getCurrent === "function") {
      chrome.tabs.getCurrent = function (callback) {
        var promise = ask().then(function (page) { return pageTab(query, page); });
        if (typeof callback === "function") {
          promise.then(function (tab) { callback(tab || undefined); }, function () { callback(undefined); });
          return undefined;
        }
        return promise;
      };
    }
    try {
    var get = typeof chrome.tabs.get === "function" ? chrome.tabs.get.bind(chrome.tabs) : null;
    chrome.tabs.get = function (tabId, callback) {
      var promise = model().then(function (built) {
        if (built) {
          for (var i = 0; i < built.tabs.length; i++) {
            if (built.tabs[i].id === tabId) return built.tabs[i];
          }
        }
        if (!get) return undefined;
        return new Promise(function (resolve) {
          try {
            var result = get(tabId, function (tab) { resolve(tab); });
            if (result && typeof result.then === "function") {
              result.then(resolve, function () { resolve(undefined); });
            }
          } catch (error) {
            resolve(undefined);
          }
        });
      });
      return settle(promise, callback, undefined);
    };
    if (chrome.windows && !(chrome.windows.getAll && chrome.windows.getAll.__ceExtensionTabs)) {
      function windowList(getInfo) {
        var populate = !!(getInfo && getInfo.populate);
        var types = getInfo && getInfo.windowTypes;
        return model().then(function (built) {
          if (!built) return [];
          if (Array.isArray(types) && types.indexOf("normal") === -1) return [];
          return (built.snap.windows || []).map(function (item) {
            var entry = {
              id: item.id,
              focused: !!item.focused,
              incognito: false,
              type: "normal",
              state: item.state || "normal",
              alwaysOnTop: false
            };
            ["left", "top", "width", "height"].forEach(function (key) {
              if (typeof item[key] === "number" && isFinite(item[key])) entry[key] = Math.round(item[key]);
            });
            if (populate) {
              entry.tabs = built.tabs.filter(function (tab) { return tab.windowId === item.id; });
            }
            return entry;
          });
        });
      }
      function windowOne(id, getInfo) {
        return windowList(getInfo).then(function (windows) {
          for (var i = 0; i < windows.length; i++) if (windows[i].id === id) return windows[i];
          return undefined;
        });
      }
      function getAll(getInfo, callback) {
        if (typeof getInfo === "function") {
          callback = getInfo;
          getInfo = {};
        }
        return settle(windowList(getInfo || {}), callback, []);
      }
      getAll.__ceExtensionTabs = true;
      chrome.windows.getAll = getAll;
      chrome.windows.get = function (windowId, getInfo, callback) {
        if (typeof getInfo === "function") {
          callback = getInfo;
          getInfo = {};
        }
        return settle(windowOne(windowId, getInfo || {}), callback, undefined);
      };
      chrome.windows.getCurrent = function (getInfo, callback) {
        if (typeof getInfo === "function") {
          callback = getInfo;
          getInfo = {};
        }
        var promise = model().then(function (built) {
          if (!built) return undefined;
          return windowOne(built.snap.currentWindowId, getInfo || {});
        });
        return settle(promise, callback, undefined);
      };
      chrome.windows.getLastFocused = function (getInfo, callback) {
        if (typeof getInfo === "function") {
          callback = getInfo;
          getInfo = {};
        }
        var promise = model().then(function (built) {
          if (!built) return undefined;
          return windowOne(built.snap.lastFocusedWindowId, getInfo || {});
        });
        return settle(promise, callback, undefined);
      };
      if (typeof chrome.windows.create === "function" && !chrome.windows.create.__ceExtensionTabs) {
        var nativeCreate = chrome.windows.create.bind(chrome.windows);
        function createWindow(createData, callback) {
          if (typeof createData === "function") {
            callback = createData;
            createData = {};
          }
          var data = Object.assign({}, createData || {});
          ["left", "top", "width", "height"].forEach(function (key) {
            if (data[key] == null) return;
            var value = Number(data[key]);
            if (!isFinite(value)) delete data[key];
            else data[key] = Math.round(value);
          });
          if (typeof callback === "function") return nativeCreate(data, callback);
          return nativeCreate(data);
        }
        createWindow.__ceExtensionTabs = true;
        try { chrome.windows.create = createWindow; } catch (error) {}
      }
      ["update", "remove"].forEach(function (name) {
        var native = chrome.windows[name];
        if (typeof native !== "function" || native.__ceExtensionTabs) return;
        function wrapped() {
          var args = Array.prototype.slice.call(arguments);
          var callback = args.length && typeof args[args.length - 1] === "function" ? args.pop() : null;
          function done(value) {
            if (chrome.runtime.lastError) {}
            if (typeof callback === "function") callback(value);
          }
          args.push(done);
          try {
            return native.apply(chrome.windows, args);
          } catch (error) {
            done();
          }
        }
        wrapped.__ceExtensionTabs = true;
        try { chrome.windows[name] = wrapped; } catch (error) {}
      });
    }
    var side = chrome.sidePanel;
    if (!(side && side.open && side.open.__ceExtensionTabs)) {
      if (!side) {
        try { chrome.sidePanel = {}; side = chrome.sidePanel; } catch (error) { side = null; }
      }
      if (side) {
        function openSide(options, callback) {
          var promise = new Promise(function (resolve) {
            var target = host();
            if (target) target.postMessage(JSON.stringify({ type: "ce-chrome", action: "side-panel" }));
            resolve();
          });
          return settle(promise, callback, undefined);
        }
        openSide.__ceExtensionTabs = true;
        try { side.open = openSide; } catch (error) {}
      }
    }
    } catch (error) {}
    return true;
  }
  if (!install()) {
    var timer = setInterval(function () {
      if (install()) clearInterval(timer);
    }, 0);
    setTimeout(function () { clearInterval(timer); }, 5000);
  }
})();
"##;

const PANEL_CHROME_JS: &str = r##"(function () {
  function postChrome(action, side) {
    var target = window.__ceHost && window.__ceHost.postMessage ? window.__ceHost : null;
    if (!target) {
      try { target = window.chrome && window.chrome.webview; } catch (error) { target = null; }
    }
    if (!target || !target.postMessage) return;
    var message = { type: "ce-chrome", action: action };
    if (side) message.side = side;
    target.postMessage(JSON.stringify(message));
  }
  // window.close() would destroy this reusable webview and leave the column empty.
  window.close = function () {
    try { console.warn("ce panel: window.close"); } catch (error) {}
    postChrome("close");
  };
  var href = "";
  try { href = String(location.href || ""); } catch (error) { href = ""; }
  if (href.indexOf("about:blank") === 0 || href.indexOf("chrome-error:") === 0 || href.indexOf("edge-error:") === 0) return;
  if (window.__cePanelChrome) return;
  window.__cePanelChrome = true;
  var style = document.createElement("style");
  style.id = "ce-panel-chrome-style";
  style.textContent = [
    "#ce-panel-chrome{position:fixed;top:8px;right:8px;z-index:2147483647;display:flex;align-items:center;gap:2px;padding:2px;border-radius:8px;background:rgba(24,24,27,.94);border:1px solid #3f3f46;box-shadow:0 1px 2px rgba(0,0,0,.4);user-select:none}",
    "#ce-panel-chrome-side{display:flex}",
    "#ce-panel-chrome button{width:22px;height:22px;border:0;border-radius:6px;padding:0;margin:0;display:inline-flex;align-items:center;justify-content:center;cursor:pointer;background:transparent;color:#a1a1aa;font:600 11px/1 ui-sans-serif,system-ui,sans-serif}",
    "#ce-panel-chrome button:hover{background:#27272a;color:#fafafa}",
    "#ce-panel-chrome button[aria-pressed=true]{background:#3f3f46;color:#fafafa}"
  ].join("");
  function stop(event) { event.stopPropagation(); }
  function sideButton(value, text, label) {
    var button = document.createElement("button");
    button.id = "ce-panel-chrome-" + value;
    button.type = "button";
    button.textContent = text;
    button.title = label;
    button.setAttribute("aria-label", label);
    button.setAttribute("aria-pressed", "false");
    button.addEventListener("click", function () {
      paint(value);
      postChrome("side", value);
    });
    return button;
  }
  function ensure() {
    var root = document.getElementById("ce-panel-chrome");
    if (root) return root;
    if (!document.getElementById("ce-panel-chrome-style")) {
      (document.head || document.documentElement).appendChild(style);
    }
    root = document.createElement("div");
    root.id = "ce-panel-chrome";
    ["pointerdown", "pointerup", "click", "mousedown", "mouseup", "contextmenu"].forEach(function (type) {
      root.addEventListener(type, stop);
    });
    var sides = document.createElement("div");
    sides.id = "ce-panel-chrome-side";
    sides.setAttribute("role", "group");
    sides.setAttribute("aria-label", "Panel side");
    sides.appendChild(sideButton("left", "L", "Left"));
    sides.appendChild(sideButton("right", "R", "Right"));
    var close = document.createElement("button");
    close.id = "ce-panel-chrome-close";
    close.type = "button";
    close.title = "Close";
    close.setAttribute("aria-label", "Close");
    close.innerHTML = '<svg id="ce-panel-chrome-close-icon" viewBox="0 0 24 24" width="12" height="12" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round"></path></svg>';
    close.addEventListener("click", function () {
      postChrome("close");
    });
    root.appendChild(sides);
    root.appendChild(close);
    (document.documentElement || document.body).appendChild(root);
    try { console.info("ce panel: exit and side buttons on " + location.href); } catch (error) {}
    return root;
  }
  function paint(side) {
    ensure();
    ["left", "right"].forEach(function (value) {
      var button = document.getElementById("ce-panel-chrome-" + value);
      if (!button) return;
      button.setAttribute("aria-pressed", side === value ? "true" : "false");
    });
  }
  function onMessage(event) {
    var data = event && event.data !== undefined ? event.data : event;
    if (typeof data === "string") {
      try { data = JSON.parse(data); } catch (error) { return; }
    }
    if (data && data.type === "ce-panel" && data.side) paint(data.side);
  }
  function listen() {
    if (window.__cePanelChromeListening) return;
    var target = window.chrome && window.chrome.webview;
    if (!target || !target.addEventListener) return;
    window.__cePanelChromeListening = true;
    target.addEventListener("message", onMessage);
  }
  function pull() {
    listen();
    ensure();
    var ui = window.ce && window.ce.ui;
    if (!ui || !ui.panelChrome) {
      if (location.protocol === "chrome-extension:" && !window.__cePanelChromeAsked) {
        window.__cePanelChromeAsked = true;
        postChrome("chrome");
      }
      setTimeout(pull, 60);
      return;
    }
    ui.panelChrome().then(function (detail) {
      if (detail && detail.side) paint(detail.side);
    }).catch(function () {});
  }
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", pull);
  else pull();
})();
"##;

const TOOLBAR_CHROME_JS: &str = r##"(function () {
  if (window.__ceToolbarChrome) return;
  window.__ceToolbarChrome = true;
  var style = document.createElement("style");
  style.id = "ce-toolbar-chrome-style";
  style.textContent = [
    "#ce-toolbar-chrome{position:fixed;z-index:2147483647;display:flex;align-items:center;gap:4px;background:rgba(24,24,27,.96);box-sizing:border-box;user-select:none}",
    "#ce-toolbar-chrome[data-axis=horizontal]{top:0;right:0;bottom:0;left:auto;flex-direction:row;padding:0 4px;border-left:1px solid #3f3f46}",
    "#ce-toolbar-chrome[data-axis=vertical]{left:0;right:0;bottom:0;top:auto;flex-direction:column;padding:4px 0;border-top:1px solid #3f3f46}",
    "#ce-toolbar-chrome-side{display:grid;grid-template-columns:7px 7px 7px;grid-template-rows:7px 7px 7px;gap:1px}",
    "#ce-toolbar-chrome-page{grid-column:2;grid-row:2;border-radius:1px;background:#52525b}",
    "#ce-toolbar-chrome-top{grid-column:2;grid-row:1}",
    "#ce-toolbar-chrome-left{grid-column:1;grid-row:2}",
    "#ce-toolbar-chrome-right{grid-column:3;grid-row:2}",
    "#ce-toolbar-chrome-bottom{grid-column:2;grid-row:3}",
    "#ce-toolbar-chrome button{border:0;padding:0;margin:0;cursor:pointer;color:#a1a1aa;background:transparent}",
    "#ce-toolbar-chrome button[role=radio]{width:7px;height:7px;border-radius:1px;background:#3f3f46}",
    "#ce-toolbar-chrome button[role=radio]:hover{background:#71717a}",
    "#ce-toolbar-chrome button[role=radio][aria-checked=true]{background:#e4e4e7}",
    "#ce-toolbar-chrome-close{width:16px;height:16px;border-radius:4px;display:inline-flex;align-items:center;justify-content:center}",
    "#ce-toolbar-chrome-close:hover{background:#27272a;color:#fafafa}"
  ].join("");
  var sides = ["top", "right", "bottom", "left"];
  function stop(event) { event.stopPropagation(); }
  function postChrome(action, side) {
    var target = window.__ceHost && window.__ceHost.postMessage ? window.__ceHost : null;
    if (!target) {
      try { target = window.chrome && window.chrome.webview; } catch (error) { target = null; }
    }
    if (!target || !target.postMessage) return;
    var message = { type: "ce-chrome", action: action };
    if (side) message.side = side;
    target.postMessage(JSON.stringify(message));
  }
  function reserve(side) {
    var html = document.documentElement;
    if (!html) return;
    var vertical = side === "left" || side === "right";
    html.style.boxSizing = "border-box";
    html.style.height = "100%";
    html.style.paddingTop = "0px";
    html.style.paddingRight = vertical ? "0px" : "52px";
    html.style.paddingBottom = vertical ? "52px" : "0px";
    html.style.paddingLeft = "0px";
    var body = document.body;
    if (body) {
      body.style.boxSizing = "border-box";
      body.style.height = "100%";
    }
  }
  function ensure() {
    var root = document.getElementById("ce-toolbar-chrome");
    if (root) return root;
    if (!document.getElementById("ce-toolbar-chrome-style")) {
      (document.head || document.documentElement).appendChild(style);
    }
    root = document.createElement("div");
    root.id = "ce-toolbar-chrome";
    root.setAttribute("data-axis", "horizontal");
    ["pointerdown", "pointerup", "click", "mousedown", "mouseup", "contextmenu"].forEach(function (type) {
      root.addEventListener(type, stop);
    });
    var group = document.createElement("div");
    group.id = "ce-toolbar-chrome-side";
    group.setAttribute("role", "radiogroup");
    group.setAttribute("aria-label", "Toolbar side");
    var page = document.createElement("span");
    page.id = "ce-toolbar-chrome-page";
    page.setAttribute("aria-hidden", "true");
    group.appendChild(page);
    sides.forEach(function (side) {
      var button = document.createElement("button");
      button.id = "ce-toolbar-chrome-" + side;
      button.type = "button";
      button.setAttribute("role", "radio");
      button.setAttribute("aria-checked", "false");
      button.setAttribute("aria-label", side.charAt(0).toUpperCase() + side.slice(1));
      button.title = "Show on the " + side;
      button.addEventListener("click", function () {
        paint(side);
        postChrome("side", side);
      });
      group.appendChild(button);
    });
    var close = document.createElement("button");
    close.id = "ce-toolbar-chrome-close";
    close.type = "button";
    close.title = "Close";
    close.setAttribute("aria-label", "Close");
    close.innerHTML = '<svg id="ce-toolbar-chrome-close-icon" viewBox="0 0 24 24" width="10" height="10" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round"></path></svg>';
    close.addEventListener("click", function () {
      postChrome("close");
    });
    root.appendChild(group);
    root.appendChild(close);
    (document.documentElement || document.body).appendChild(root);
    return root;
  }
  function paint(side) {
    if (sides.indexOf(side) < 0) side = "bottom";
    var root = ensure();
    var vertical = side === "left" || side === "right";
    root.setAttribute("data-axis", vertical ? "vertical" : "horizontal");
    reserve(side);
    sides.forEach(function (value) {
      var button = document.getElementById("ce-toolbar-chrome-" + value);
      if (!button) return;
      button.setAttribute("aria-checked", side === value ? "true" : "false");
    });
  }
  function onEvent(event) {
    var detail = event && event.detail;
    if (detail && detail.event === "pageToolbar" && detail.side) paint(detail.side);
  }
  function pull() {
    ensure();
    var ui = window.ce && window.ce.ui;
    if (!ui || !ui.pageToolbar) {
      setTimeout(pull, 60);
      return;
    }
    ui.pageToolbar().then(function (toolbar) {
      if (toolbar && toolbar.side) paint(toolbar.side);
    }).catch(function () {});
  }
  window.addEventListener("ce-event", onEvent);
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", pull);
  else pull();
})();
"##;

const PANEL_HANDLE_JS: &str = r#"(function () {
  var handleId = "ce-panel-resize";
  var seam = "rgba(14, 165, 233, 0.85)";
  function post(message) {
    var target = window.__ceHost && window.__ceHost.postMessage ? window.__ceHost : null;
    if (!target) {
      try { target = window.chrome && window.chrome.webview; } catch (error) { target = null; }
    }
    if (!target || !target.postMessage) return;
    target.postMessage(JSON.stringify(message));
  }
  function ensure() {
    var handle = document.getElementById(handleId);
    if (handle) return handle;
    handle = document.createElement("div");
    handle.id = handleId;
    handle.hidden = true;
    handle.title = "Drag to resize";
    handle.setAttribute("role", "separator");
    handle.setAttribute("aria-orientation", "vertical");
    handle.setAttribute("aria-label", "Resize panel");
    handle.style.position = "fixed";
    handle.style.top = "0";
    handle.style.bottom = "0";
    handle.style.width = "10px";
    handle.style.zIndex = "2147483646";
    handle.style.cursor = "col-resize";
    handle.style.background = "transparent";
    handle.style.touchAction = "none";
    var dragging = false;
    handle.addEventListener("pointerdown", function (event) {
      if (handle.hidden || dragging || event.button !== 0) return;
      dragging = true;
      event.preventDefault();
      event.stopPropagation();
      if (handle.setPointerCapture) handle.setPointerCapture(event.pointerId);
      var done = function () { dragging = false; };
      var ui = window.ce && window.ce.ui;
      if (ui && ui.resizePanel) {
        try {
          var pending = ui.resizePanel();
          if (pending && pending.then) pending.then(done, done);
          else done();
        } catch (error) { done(); }
        return;
      }
      post({ type: "ce-chrome", action: "resize" });
    });
    handle.addEventListener("pointerup", function () { dragging = false; });
    handle.addEventListener("pointercancel", function () { dragging = false; });
    handle.addEventListener("mouseenter", function () {
      if (!handle.hidden) handle.style.background = "rgba(14, 165, 233, 0.28)";
    });
    handle.addEventListener("mouseleave", function () {
      if (!dragging) handle.style.background = "transparent";
    });
    (document.documentElement || document.body).appendChild(handle);
    return handle;
  }
  var last = null;
  function apply(detail) {
    last = detail;
    window.__cePanelApplied = true;
    var handle = ensure();
    var side = detail && detail.side === "left" ? "left" : "right";
    var resizable = !!(detail && detail.resizable);
    handle.hidden = !resizable;
    handle.style.left = side === "left" ? "auto" : "0";
    handle.style.right = side === "left" ? "0" : "auto";
    handle.style.boxShadow = resizable
      ? (side === "left" ? "inset -2px 0 0 " : "inset 2px 0 0 ") + seam
      : "none";
    if (!resizable) handle.style.background = "transparent";
  }
  function onMessage(event) {
    var data = event && event.data !== undefined ? event.data : event;
    if (typeof data === "string") {
      try { data = JSON.parse(data); } catch (error) { return; }
    }
    if (data && data.type === "ce-panel") apply(data);
  }
  function listen() {
    if (window.__cePanelListening) return;
    var target = window.chrome && window.chrome.webview;
    if (!target || !target.addEventListener) return;
    window.__cePanelListening = true;
    target.addEventListener("message", onMessage);
  }
  function watch() {
    if (window.__cePanelWatch || !window.MutationObserver) return;
    var root = document.documentElement;
    if (!root) return;
    window.__cePanelWatch = true;
    new MutationObserver(function () {
      if (!document.getElementById(handleId) && last) apply(last);
    }).observe(root, { childList: true });
  }
  function pull() {
    listen();
    watch();
    var ui = window.ce && window.ce.ui;
    if (ui && ui.panelChrome) {
      ui.panelChrome().then(apply).catch(function () {});
      return;
    }
    if (location.protocol === "chrome-extension:" && window.__cePanelListening) {
      if (!window.__cePanelAsked) {
        window.__cePanelAsked = true;
        post({ type: "ce-chrome", action: "chrome" });
        setTimeout(function () {
          if (!window.__cePanelApplied) apply({ side: "right", resizable: true });
        }, 400);
      }
      return;
    }
    setTimeout(pull, 60);
  }
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", pull);
  else pull();
})();
"#;

const BRIDGE_JS: &str = r#"(function () {
  if (window.__ceBridge) return;
  window.__ceBridge = true;
  var HOST_SUFFIX = ".ce-ext.localhost";
  var hostname = String(location.hostname || "").toLowerCase();
  var extensionId = hostname.endsWith(HOST_SUFFIX) ? hostname.slice(0, -HOST_SUFFIX.length) : "";
  var pending = {};
  var nextId = 0;
  function webview() {
    try { return window.chrome && window.chrome.webview; } catch (error) { return null; }
  }
  function post(message) {
    var host = window.__ceHost && window.__ceHost.postMessage ? window.__ceHost : webview();
    if (!host || !host.postMessage) throw new Error("ce is unavailable in this page.");
    host.postMessage(JSON.stringify(message));
  }
  function onHostMessage(event) {
    var data = event && event.data !== undefined ? event.data : event;
    if (typeof data === "string") {
      try { data = JSON.parse(data); } catch (error) { return; }
    }
    if (!data || typeof data !== "object") return;
    if (data.type === "ce-rpc-result") {
      var waiter = pending[data.id];
      if (!waiter) return;
      delete pending[data.id];
      if (data.error) waiter.reject(new Error(String(data.error)));
      else waiter.resolve(data.result);
      return;
    }
    if (data.type === "ce-event") {
      try { window.dispatchEvent(new CustomEvent("ce-event", { detail: data })); } catch (error) {}
    }
  }
  var listening = false;
  function attach() {
    if (listening) return;
    var target = webview();
    if (!target || !target.addEventListener) return;
    listening = true;
    target.addEventListener("message", onHostMessage);
  }
  attach();
  document.addEventListener("DOMContentLoaded", attach);
  function rpc(method, params) {
    if (!extensionId) return Promise.reject(new Error("This page cannot use ce."));
    var id = ++nextId;
    return new Promise(function (resolve, reject) {
      pending[id] = { resolve: resolve, reject: reject };
      try { post({ type: "ce-rpc", id: id, method: method, params: params || {} }); }
      catch (error) { delete pending[id]; reject(error); }
    });
  }
  function sendTo(id, message) {
    post({ type: "ce-content", extensionId: id, message: message });
  }
  if (!extensionId) {
    window.ce = { runtime: { sendMessage: function (id, message) { try { sendTo(String(id || ""), message); } catch (error) { console.error(error); } } } };
    return;
  }
  function onEvent(name, handler) {
    function listen(event) {
      if (event.detail && event.detail.event === name) handler(event.detail);
    }
    window.addEventListener("ce-event", listen);
    return function () { window.removeEventListener("ce-event", listen); };
  }
  window.ce = {
    runtime: {
      id: extensionId,
      sendMessage: function (message) { sendTo(extensionId, message); },
      onMessage: function (handler) { return onEvent("message", function (detail) { handler(detail.message, detail.sender || null); }); }
    },
    tabs: {
      list: function () { return rpc("tabs.list"); },
      create: function (url, background) { return rpc("tabs.create", { url: url || "", background: !!background }); },
      close: function (id) { return rpc("tabs.close", { id: id }); },
      activate: function (id) { return rpc("tabs.activate", { id: id }); },
      navigate: function (id, url) { return rpc("tabs.navigate", { id: id, url: url }); },
      back: function (id) { return rpc("tabs.back", { id: id }); },
      forward: function (id) { return rpc("tabs.forward", { id: id }); },
      reload: function (id) { return rpc("tabs.reload", { id: id }); },
      onChanged: function (handler) { return onEvent("tabs", function () { handler(); }); }
    },
    page: {
      executeScript: function (tabId, code) { return rpc("page.executeScript", { tabId: tabId, code: code }); },
      insertCSS: function (tabId, css) { return rpc("page.insertCSS", { tabId: tabId, css: css }); },
      capture: function (tabId) { return rpc("page.capture", { tabId: tabId }); },
      click: function (tabId, x, y) { return rpc("page.click", { tabId: tabId, x: x, y: y }); },
      type: function (tabId, text) { return rpc("page.type", { tabId: tabId, text: text }); },
      scroll: function (tabId, dy) { return rpc("page.scroll", { tabId: tabId, dy: dy }); }
    },
    desktop: {
      move: function (x, y) { return rpc("desktop.move", { x: x, y: y }); },
      click: function (x, y) { return rpc("desktop.click", { x: x, y: y }); },
      type: function (text) { return rpc("desktop.type", { text: text }); }
    },
    bookmarks: {
      list: function () { return rpc("bookmarks.list"); },
      add: function (parentId, title, url) { return rpc("bookmarks.add", { parentId: parentId || "", title: title || "", url: url || "" }); },
      update: function (id, title, url) { return rpc("bookmarks.update", { id: id, title: title || "", url: url || "" }); },
      remove: function (id) { return rpc("bookmarks.remove", { id: id }); }
    },
    downloads: {
      list: function () { return rpc("downloads.list"); },
      pause: function (id) { return rpc("downloads.pause", { id: id }); },
      resume: function (id) { return rpc("downloads.resume", { id: id }); },
      cancel: function (id) { return rpc("downloads.cancel", { id: id }); },
      open: function (id) { return rpc("downloads.open", { id: id }); },
      show: function (id) { return rpc("downloads.show", { id: id }); },
      onChanged: function (handler) { return onEvent("downloads", function () { handler(); }); }
    },
    storage: {
      get: function (key) { return rpc("storage.get", { key: key }); },
      set: function (key, value) { return rpc("storage.set", { key: key, value: value }); }
    },
    ui: {
      togglePanel: function () { return rpc("ui.togglePanel"); },
      openPage: function (pageId) { return rpc("ui.openPage", { pageId: pageId }); },
      notify: function (message) { return rpc("ui.notify", { message: message }); },
      panelChrome: function () { return rpc("ui.panelChrome"); },
      setPanelSide: function (side) { return rpc("ui.setPanelSide", { side: side || "" }); },
      resizePanel: function () { return rpc("ui.resizePanel"); },
      openPopout: function () { return rpc("ui.openPopout"); },
      closePopout: function () { return rpc("ui.closePopout"); },
      pageToolbar: function () { return rpc("ui.pageToolbar"); },
      setPageToolbarSide: function (side) { return rpc("ui.setPageToolbarSide", { side: side || "" }); },
      closePageToolbar: function () { return rpc("ui.closePageToolbar"); },
      onPageToolbar: function (handler) { return onEvent("pageToolbar", function (detail) { handler(detail.side || ""); }); }
    }
  };
})();
"#;

const CONTENT_JS_START: &str = r#"(function () {
  var scripts = "#;

const CONTENT_JS_END: &str = r#";
  function matches(pattern, href) {
    if (pattern === "<all_urls>" || pattern === "*://*/*") return href.indexOf("http://") === 0 || href.indexOf("https://") === 0;
    var split = pattern.indexOf("://");
    if (split < 0) return false;
    var url;
    try { url = new URL(href); } catch (error) { return false; }
    var scheme = pattern.slice(0, split);
    if (scheme !== "*" && url.protocol !== scheme + ":") return false;
    var rest = pattern.slice(split + 3);
    var slash = rest.indexOf("/");
    var host = slash < 0 ? rest : rest.slice(0, slash);
    var path = slash < 0 ? "/*" : rest.slice(slash);
    var pageHost = url.hostname;
    var hostOk = false;
    if (host === "*") hostOk = true;
    else if (host.indexOf("*.") === 0) {
      var suffix = host.slice(2);
      hostOk = pageHost === suffix || pageHost.slice(-(suffix.length + 1)) === "." + suffix;
    } else hostOk = pageHost === host;
    if (!hostOk) return false;
    if (path === "/*" || path === "*") return true;
    if (path.slice(-1) === "*") return url.pathname.indexOf(path.slice(0, -1)) === 0;
    return url.pathname === path;
  }
  var href = String(location.href || "");
  for (var index = 0; index < scripts.length; index += 1) {
    var script = scripts[index];
    var patterns = script.matches || [];
    var hit = false;
    for (var pattern = 0; pattern < patterns.length; pattern += 1) {
      if (matches(patterns[pattern], href)) { hit = true; break; }
    }
    if (!hit) continue;
    if (script.css) {
      try {
        var style = document.createElement("style");
        style.setAttribute("data-ce-extension", script.extensionId || "");
        style.textContent = script.css;
        (document.head || document.documentElement).appendChild(style);
      } catch (error) {}
    }
    if (script.code) {
      try { (0, eval)(script.code); } catch (error) { console.error("ce content script", error); }
    }
  }
})();
"#;

#[cfg(test)]
mod tests {
    use super::{grants_cover, safe_relative, valid_id};

    #[test]
    fn a_blank_panel_is_not_the_extension_document() {
        assert!(super::panel_placeholder("about:blank"));
        assert!(super::panel_placeholder("about:blank#blocked"));
        assert!(super::panel_placeholder("chrome-error://chromewebdata/"));
        assert!(!super::panel_placeholder(
            "chrome-extension://abc/popup.html"
        ));
        assert!(!super::panel_document_ready(
            "about:blank",
            "chrome-extension://abc/popup.html"
        ));
        assert!(super::panel_document_ready(
            "chrome-extension://abc/popup.html#/vault",
            "chrome-extension://abc/popup.html"
        ));
        assert!(super::panel_document_ready(
            "chrome-extension://abc/popup.html?uilocation=panel",
            "chrome-extension://abc/popup.html"
        ));
        assert!(!super::panel_document_ready(
            "chrome-extension://other/popup.html",
            "chrome-extension://abc/popup.html"
        ));
        assert!(super::panel_document_ready(
            "https://notes.ce-ext.localhost/panel.html",
            "https://notes.ce-ext.localhost/panel.html"
        ));
    }

    #[test]
    fn a_store_panel_belongs_to_its_browser_window() {
        assert_eq!(super::extension_surface_window("cext-panel-main"), "main");
        assert_eq!(
            super::extension_surface_window("cext-panel-main-2"),
            "main-2"
        );
        assert_eq!(super::extension_surface_window("extpopup-main"), "main");
    }

    #[test]
    fn ids_stay_inside_the_host_label() {
        assert!(valid_id("notes.example"));
        assert!(valid_id("a"));
        assert!(!valid_id(""));
        assert!(!valid_id("Notes"));
        assert!(!valid_id(".notes"));
        assert!(!valid_id("notes."));
        assert!(!valid_id("notes..example"));
        assert!(!valid_id("notes/example"));
        assert!(!valid_id(&"a".repeat(65)));
    }

    #[test]
    fn new_permissions_are_not_covered() {
        let granted = vec!["tabs".into()];
        assert!(grants_cover(&granted, &["tabs".into()]));
        assert!(!grants_cover(&granted, &["tabs".into(), "desktop".into()]));
        assert!(grants_cover(&granted, &[]));
    }

    #[test]
    fn a_panel_string_and_a_panel_object_both_load() {
        let path: super::Manifest = serde_json::from_str(r#"{"panel":"panel.html"}"#).unwrap();
        assert_eq!(path.panel_path(), "panel.html");
        assert!(!path.panel_detail().resizable);
        let object: super::Manifest = serde_json::from_str(
            r#"{"panel":{"path":"panel.html","side":"left","resizable":true,"minWidth":200,"maxWidth":500,"defaultWidth":320}}"#,
        )
        .unwrap();
        assert_eq!(object.panel_path(), "panel.html");
        let detail = object.panel_detail();
        assert_eq!(detail.side, "left");
        assert!(detail.resizable);
        assert_eq!(detail.min_width, 200);
    }

    #[test]
    fn window_bounds_are_whole_screen_pixels() {
        assert_eq!(super::dip_px(200, 1.0), 200);
        assert_eq!(super::dip_px(150, 1.25), 120);
        assert_eq!(super::dip_px(-11, 1.25), -9);
        assert_eq!(super::dip_px(384, 0.0), 384);
    }

    #[test]
    fn a_popout_string_uses_the_window_defaults() {
        let manifest: super::Manifest =
            serde_json::from_str(r#"{"popout":"window.html"}"#).unwrap();
        let popout = super::resolve_popout(&manifest).unwrap();
        assert_eq!(popout.path, "window.html");
        assert_eq!(popout.width, 480.0);
        assert!(popout.resizable);
        let sized: super::Manifest = serde_json::from_str(
            r#"{"popout":{"path":"window.html","width":420,"height":560,"resizable":false}}"#,
        )
        .unwrap();
        let popout = super::resolve_popout(&sized).unwrap();
        assert_eq!(popout.width, 420.0);
        assert!(!popout.resizable);
    }

    #[test]
    fn panel_size_comes_from_the_manifest() {
        let manifest: super::Manifest = serde_json::from_str(
            r#"{"panel":{"path":"panel.html","side":"right","resizable":true,"minWidth":260,"maxWidth":520,"defaultWidth":320}}"#,
        )
        .unwrap();
        let record = super::HostRecord {
            panel_side: "left".into(),
            panel_width: 400,
            ..super::HostRecord::default()
        };
        let layout = super::resolve_panel(&record, &manifest);
        assert!(matches!(layout.edge, crate::tabs::PanelEdge::Left));
        assert!(layout.resizable);
        assert_eq!(layout.min_width, 260);
        assert_eq!(layout.max_width, 520);
        assert_eq!(layout.default_width, 320);

        let bare: super::Manifest = serde_json::from_str(r#"{"panel":"panel.html"}"#).unwrap();
        let layout = super::resolve_panel(&super::HostRecord::default(), &bare);
        assert!(matches!(layout.edge, crate::tabs::PanelEdge::Right));
        assert!(!layout.resizable);
        assert_eq!(layout.min_width, 240);
        assert_eq!(layout.max_width, 720);
        assert_eq!(layout.default_width, 360);
    }

    #[test]
    fn an_older_host_record_still_loads() {
        let file: super::HostFile = serde_json::from_str(
            r#"{"extensions":[{"id":"notes.example","enabled":true,"panelSide":"left","panelResizable":false,"panelMinWidth":200,"panelMaxWidth":400,"panelDefaultWidth":300,"panelWidth":280}]}"#,
        )
        .unwrap();
        let record = &file.extensions[0];
        assert_eq!(record.id, "notes.example");
        assert!(record.enabled);
        assert_eq!(record.panel_side, "left");
        assert_eq!(record.panel_width, 280);
        assert!(record.toolbar_side.is_empty());
        assert!(record.toolbar_open);
    }

    #[test]
    fn a_page_toolbar_stays_open_until_it_is_closed() {
        let manifest: super::Manifest =
            serde_json::from_str(r#"{"pageToolbar":"strip.html"}"#).unwrap();
        let file: super::HostFile =
            serde_json::from_str(r#"{"extensions":[{"id":"sample.kitchen","enabled":true}]}"#)
                .unwrap();
        let record = &file.extensions[0];
        assert!(record.toolbar_open);
        assert!(super::page_toolbar_for_layout(record, &manifest).is_some());
        let closed = super::HostRecord {
            toolbar_open: false,
            ..super::HostRecord::default()
        };
        assert!(super::resolve_page_toolbar(&closed, &manifest).is_some());
        assert!(super::page_toolbar_for_layout(&closed, &manifest).is_none());
    }

    #[test]
    fn a_page_toolbar_string_starts_on_the_bottom() {
        let manifest: super::Manifest =
            serde_json::from_str(r#"{"pageToolbar":"strip.html"}"#).unwrap();
        let toolbar =
            super::resolve_page_toolbar(&super::HostRecord::default(), &manifest).unwrap();
        assert_eq!(toolbar.path, "strip.html");
        assert_eq!(toolbar.side, "bottom");
        assert_eq!(toolbar.thickness, 40.0);
        let moved = super::HostRecord {
            toolbar_side: "left".into(),
            ..super::HostRecord::default()
        };
        let toolbar = super::resolve_page_toolbar(&moved, &manifest).unwrap();
        assert_eq!(toolbar.side, "left");
    }

    #[test]
    fn a_page_toolbar_object_keeps_its_edge_and_thickness() {
        let manifest: super::Manifest = serde_json::from_str(
            r#"{"pageToolbar":{"path":"strip.html","side":"top","thickness":64}}"#,
        )
        .unwrap();
        let toolbar =
            super::resolve_page_toolbar(&super::HostRecord::default(), &manifest).unwrap();
        assert_eq!(toolbar.side, "top");
        assert_eq!(toolbar.thickness, 64.0);
        let huge: super::Manifest = serde_json::from_str(
            r#"{"pageToolbar":{"path":"strip.html","side":"sideways","thickness":400}}"#,
        )
        .unwrap();
        let toolbar = super::resolve_page_toolbar(&super::HostRecord::default(), &huge).unwrap();
        assert_eq!(toolbar.side, "bottom");
        assert_eq!(toolbar.thickness, 96.0);
    }

    #[test]
    fn panel_sizes_stay_inside_the_limits() {
        let layout = super::normalize_panel("left", true, 10, 9000, 20);
        assert_eq!(layout.min_width, 160);
        assert_eq!(layout.max_width, 1600);
        assert_eq!(layout.default_width, 160);
        assert!(matches!(layout.edge, crate::tabs::PanelEdge::Left));
        let swapped = super::normalize_panel("right", false, 400, 300, 500);
        assert_eq!(swapped.min_width, 400);
        assert_eq!(swapped.max_width, 400);
        assert_eq!(swapped.default_width, 400);
    }

    #[test]
    fn file_paths_cannot_leave_the_package() {
        assert!(safe_relative("popup.html").is_ok());
        assert!(safe_relative("/pages/library.html").is_ok());
        assert!(safe_relative("../secret").is_err());
        assert!(safe_relative("").is_err());
        assert!(safe_relative(r"C:\windows").is_err());
    }
}
