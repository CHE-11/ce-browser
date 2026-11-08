//! Host extensions: folders of HTML, CSS, and JavaScript the browser loads at runtime.
//!
//! Each enabled package is served at `https://<id>.ce-ext.localhost/`. Its pages call `window.ce`.
//! That object posts to the host, which checks the calling page and the granted permissions
//! before touching tabs, bookmarks, downloads, or the desktop.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
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
    COREWEBVIEW2_HOST_RESOURCE_ACCESS_KIND_DENY_CORS, ICoreWebView2, ICoreWebView2_3,
};
use webview2_com::{
    AddScriptToExecuteOnDocumentCreatedCompletedHandler, WebMessageReceivedEventHandler,
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
    /// `left`, `right`, or empty to use the manifest.
    panel_side: String,
    /// `None` uses the manifest. The extensions page writes `Some` when the user chooses.
    panel_resizable: Option<bool>,
    /// Zero uses the manifest, then the built-in default.
    panel_min_width: u32,
    panel_max_width: u32,
    panel_default_width: u32,
    /// Last dragged width. Zero uses the default width.
    panel_width: u32,
}

impl Default for HostRecord {
    fn default() -> Self {
        Self {
            id: String::new(),
            enabled: false,
            granted: Vec::new(),
            panel_side: String::new(),
            panel_resizable: None,
            panel_min_width: 0,
            panel_max_width: 0,
            panel_default_width: 0,
            panel_width: 0,
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
    #[serde(default)]
    downloads: String,
    #[serde(default)]
    content_scripts: Vec<ContentManifest>,
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
    /// The side and size the panel opens with. Empty when the extension has no panel.
    pub panel: Option<HostPanelLayout>,
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
        },
        panel: layout,
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
        next.panel_resizable = previous.panel_resizable;
        next.panel_min_width = previous.panel_min_width;
        next.panel_max_width = previous.panel_max_width;
        next.panel_default_width = previous.panel_default_width;
        next.panel_width = previous.panel_width;
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
    ensure_child(app, window, &label, url.clone())?;
    if let Some(view) = app.get_webview(&label) {
        navigate_if_needed(&view, &url);
    }
    let wanted = if layout.resizable && loaded.record.panel_width > 0 {
        f64::from(loaded.record.panel_width)
    } else {
        f64::from(layout.default_width)
    };
    let width = fitted_panel_width(app, window, wanted, &layout);
    tabs::open_host_panel(app, window, &id, width, layout.edge);
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
        return Ok(view);
    }
    let host = app
        .get_window(window)
        .ok_or("The browser window is missing.")?;
    let (r, g, b) = settings::popup_surface(app);
    let mut builder = tabs::base_builder(app, label, WebviewUrl::External(url))
        .background_color(Color(r, g, b, 255))
        .focused(false);
    if label.starts_with("cext-panel-") {
        builder = builder.initialization_script(PANEL_HANDLE_JS);
    }
    let view = host
        .add_child(
            builder,
            LogicalPosition::new(-10000.0, -10000.0),
            LogicalSize::new(360.0, 480.0),
        )
        .map_err(|error| error.to_string())?;
    attach_webview(app, &view);
    Ok(view)
}

fn navigate_if_needed(view: &Webview, url: &Url) {
    let same = view
        .url()
        .ok()
        .is_some_and(|current| current.as_str() == url.as_str());
    if !same {
        let _ = view.navigate(url.clone());
    }
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
        "ui.resizePanel" => resize_open_panel(app, &loaded),
        "ui.openPopout" => {
            open_popout(app, id)?;
            Ok(Value::Null)
        }
        "ui.closePopout" => {
            close_popout(app, id);
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
    let resizable = record.panel_resizable.unwrap_or(detail.resizable);
    let min_width = pick_width(record.panel_min_width, detail.min_width, PANEL_MIN);
    let max_width = pick_width(record.panel_max_width, detail.max_width, PANEL_MAX);
    let default_width = pick_width(
        record.panel_default_width,
        detail.default_width,
        PANEL_DEFAULT,
    );
    normalize_panel(side, resizable, min_width, max_width, default_width)
}

fn pick_width(chosen: u32, suggested: u32, fallback: u32) -> u32 {
    if chosen > 0 {
        chosen
    } else if suggested > 0 {
        suggested
    } else {
        fallback
    }
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

pub fn set_panel_layout(
    app: &AppHandle,
    id: &str,
    side: &str,
    resizable: bool,
    min_width: u32,
    max_width: u32,
    default_width: u32,
) -> Result<(), String> {
    if side != "left" && side != "right" {
        return Err("The panel side must be left or right.".into());
    }
    for (value, label) in [
        (min_width, "Min width"),
        (max_width, "Max width"),
        (default_width, "Default width"),
    ] {
        if !(PANEL_FLOOR..=PANEL_CEILING).contains(&value) {
            return Err(format!(
                "{label} must be between {PANEL_FLOOR} and {PANEL_CEILING} pixels."
            ));
        }
    }
    if min_width > max_width {
        return Err("The minimum width cannot be larger than the maximum.".into());
    }
    if resizable && (default_width < min_width || default_width > max_width) {
        return Err("Default width must sit between the minimum and the maximum.".into());
    }
    let id = canonical_id(id)?;
    let mut records = state(app);
    {
        let record = records
            .iter_mut()
            .find(|record| record.id == id)
            .ok_or("That extension is not installed.")?;
        let loaded = load_record(app, record).ok_or("That extension folder could not be read.")?;
        if loaded.manifest.panel_path().is_empty() {
            return Err("That extension has no side panel.".into());
        }
        let current = resolve_panel(record, &loaded.manifest);
        let reset_width = current.resizable != resizable
            || current.min_width != min_width
            || current.max_width != max_width
            || current.default_width != default_width;
        record.panel_side = side.to_string();
        record.panel_resizable = Some(resizable);
        record.panel_min_width = min_width;
        record.panel_max_width = max_width;
        record.panel_default_width = default_width;
        if reset_width {
            record.panel_width = 0;
        }
    }
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
    for window in tabs::snapshot(app).windows {
        if tabs::host_panel_extension(app, &window.label) != id {
            continue;
        }
        let width = fitted_panel_width(app, &window.label, wanted, &layout);
        tabs::open_host_panel(app, &window.label, id, width, layout.edge);
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
    let window = WindowBuilder::new(app, &label)
        .title(&title)
        .inner_size(popout.width, popout.height)
        .min_inner_size(popout.min_width, popout.min_height)
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

const PANEL_HANDLE_JS: &str = r#"(function () {
  var handleId = "ce-panel-resize";
  function ensure() {
    var handle = document.getElementById(handleId);
    if (handle) return handle;
    handle = document.createElement("div");
    handle.id = handleId;
    handle.hidden = true;
    handle.title = "Drag to resize";
    handle.style.position = "fixed";
    handle.style.top = "0";
    handle.style.bottom = "0";
    handle.style.width = "6px";
    handle.style.zIndex = "2147483646";
    handle.style.cursor = "col-resize";
    handle.style.background = "transparent";
    var dragging = false;
    handle.addEventListener("pointerdown", function (event) {
      if (handle.hidden || dragging) return;
      if (!window.ce || !window.ce.ui || !window.ce.ui.resizePanel) return;
      dragging = true;
      event.preventDefault();
      event.stopPropagation();
      var done = function () { dragging = false; };
      try {
        var pending = window.ce.ui.resizePanel();
        if (pending && pending.then) pending.then(done, done);
        else done();
      } catch (error) { done(); }
    });
    handle.addEventListener("mouseenter", function () {
      if (!handle.hidden) handle.style.background = "rgba(14, 165, 233, 0.45)";
    });
    handle.addEventListener("mouseleave", function () { handle.style.background = "transparent"; });
    (document.documentElement || document.body).appendChild(handle);
    return handle;
  }
  function apply(detail) {
    var handle = ensure();
    var side = detail && detail.side === "left" ? "left" : "right";
    var resizable = !!(detail && detail.resizable);
    handle.hidden = !resizable;
    handle.style.left = side === "left" ? "auto" : "0";
    handle.style.right = side === "left" ? "0" : "auto";
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
  function pull() {
    listen();
    if (!window.ce || !window.ce.ui || !window.ce.ui.panelChrome) {
      setTimeout(pull, 60);
      return;
    }
    window.ce.ui.panelChrome().then(apply).catch(function () {});
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
      resizePanel: function () { return rpc("ui.resizePanel"); },
      openPopout: function () { return rpc("ui.openPopout"); },
      closePopout: function () { return rpc("ui.closePopout"); }
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
