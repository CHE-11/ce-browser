//! Windows integration: site windows, a summon hotkey, toasts, the default browser, the tray, and battery saver.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::{files, paths, profiles, settings, tabs};

const LAUNCH_FILE: &str = "launch.json";
const APPS_FILE: &str = "apps.json";
pub const DEFAULT_HOTKEY: &str = "ctrl+alt+space";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launch {
    None,
    Url(String),
    App(String),
    Open(String),
    /// Local files to open with a file handler. Not kept in `LAUNCH`: those wait until
    /// session restore has finished so the new tab stays in front.
    Files(Vec<String>),
}

struct PendingFiles {
    ready: bool,
    paths: Vec<String>,
}

#[derive(Clone)]
struct FileAttach {
    dir: PathBuf,
    /// `https://file-<id>.ce-browser.example/document.<ext>`. The page fetches this.
    /// `.example` is a reserved name, so it never hits the network.
    fetch: String,
    file_name: String,
    staged: PathBuf,
    content_type: String,
    input_id: String,
    editor_url: String,
}

/// Bytes for a file fetch from a handler page.
pub struct ServedFile {
    pub bytes: Vec<u8>,
    pub content_type: String,
}

static LAUNCH: Mutex<Launch> = Mutex::new(Launch::None);
static FILE_LAUNCH: Mutex<PendingFiles> = Mutex::new(PendingFiles {
    ready: false,
    paths: Vec::new(),
});
static FILE_ARMED: Mutex<Option<FileAttach>> = Mutex::new(None);
static FILE_TABS: Mutex<Vec<(u32, FileAttach)>> = Mutex::new(Vec::new());
static TRAY: OnceLock<tauri::tray::TrayIcon> = OnceLock::new();
static FRAMES_CAPPED: Mutex<bool> = Mutex::new(false);

pub fn read_args() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match launch_from_args(&args) {
        Launch::Files(paths) => {
            FILE_LAUNCH.lock().expect("file launch").paths.extend(paths);
        }
        other => *LAUNCH.lock().expect("launch") = other,
    }
}

pub fn launch_from_args(args: &[String]) -> Launch {
    let mut files = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        if let Some(id) = arg.strip_prefix("--app=") {
            return Launch::App(id.to_string());
        }
        if arg == "--app" {
            if let Some(id) = args.get(index + 1) {
                return Launch::App(id.clone());
            }
        }
        if let Some(path) = arg.strip_prefix("--open=") {
            return Launch::Open(path.to_string());
        }
        if arg == "--open" {
            if let Some(path) = args.get(index + 1) {
                return Launch::Open(path.clone());
            }
        }
        if arg == "--profile" {
            index += 2;
            continue;
        }
        if arg.starts_with("--profile=") {
            index += 1;
            continue;
        }
        if is_website(arg) {
            return Launch::Url(arg.to_string());
        }
        if let Some(path) = file_argument(arg) {
            files.push(path);
        }
        index += 1;
    }
    if files.is_empty() {
        Launch::None
    } else {
        Launch::Files(files)
    }
}

/// A command-line file from a file association or a `file://` URL. Flags are left alone.
pub fn file_argument(arg: &str) -> Option<String> {
    let arg = arg.trim().trim_matches('"');
    if arg.is_empty() || arg.starts_with('-') {
        return None;
    }
    let path = if arg.len() >= 7 && arg[..7].eq_ignore_ascii_case("file://") {
        url::Url::parse(arg).ok()?.to_file_path().ok()?
    } else {
        PathBuf::from(arg)
    };
    let extension = path.extension().and_then(|ext| ext.to_str())?;
    if clean_launch_extension(extension).is_none() {
        return None;
    }
    Some(path.to_string_lossy().to_string())
}

fn clean_launch_extension(extension: &str) -> Option<String> {
    let text = extension
        .trim()
        .trim_start_matches('.')
        .to_ascii_lowercase();
    if (1..=16).contains(&text.len())
        && text
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
    {
        Some(text)
    } else {
        None
    }
}

/// Host and path match. A query string on the loaded page is ignored.
pub fn pages_match(page: &str, editor: &str) -> bool {
    let Ok(page) = url::Url::parse(page) else {
        return false;
    };
    let Ok(editor) = url::Url::parse(editor) else {
        return false;
    };
    page.host_str() == editor.host_str()
        && page.path().trim_end_matches('/') == editor.path().trim_end_matches('/')
}

pub fn handler_matches(url: &str, handlers: &[settings::FileHandler]) -> bool {
    handlers
        .iter()
        .any(|handler| pages_match(url, &handler.url))
}

pub fn content_type(extension: &str) -> &'static str {
    match extension.to_ascii_lowercase().as_str() {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "txt" => "text/plain",
        "htm" | "html" => "text/html",
        "css" => "text/css",
        "csv" => "text/csv",
        "json" => "application/json",
        "xml" => "application/xml",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        "wav" => "audio/wav",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    }
}

pub fn is_website(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

pub fn site_id(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return None;
    }
    let host = parsed.host_str()?.trim_start_matches("www.");
    let id: String = host
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let id = id.trim_matches('-').to_string();
    if id.is_empty() { None } else { Some(id) }
}

pub fn hotkey_mods(token: &str) -> Option<(u32, u32)> {
    let mut mods = 0u32;
    let mut key = None;
    for part in token
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => mods |= 0x0002,
            "alt" => mods |= 0x0001,
            "shift" => mods |= 0x0004,
            "space" => key = Some(0x20),
            other if other.len() == 1 => {
                let character = other.chars().next()?.to_ascii_uppercase();
                if character.is_ascii_alphabetic() {
                    key = Some(character as u32);
                }
            }
            _ => {}
        }
    }
    key.map(|key| (mods, key))
}

pub fn power_is_battery(ac_line: u8) -> bool {
    ac_line == 0
}

pub fn write_handoff(app: &AppHandle, profile: &str) {
    let launch = LAUNCH.lock().expect("launch").clone();
    let files = FILE_LAUNCH.lock().expect("file launch").paths.clone();
    if matches!(launch, Launch::None) && files.is_empty() {
        return;
    }
    let mut value = launch_file(&launch);
    if !files.is_empty() {
        value["files"] = serde_json::json!(files);
    }
    if !profile.is_empty() {
        value["profile"] = serde_json::json!(profile);
    }
    let path = paths::root_local_dir(app).join(LAUNCH_FILE);
    let _ = files::write_json(&path, &value);
}

pub fn start(app: &AppHandle) {
    let _ = register_browser(app);
    start_hotkey(app.clone());
    let _ = install_tray(app);
    apply_launch(app);
}

fn launch_file(launch: &Launch) -> serde_json::Value {
    match launch {
        Launch::Url(url) => serde_json::json!({ "url": url }),
        Launch::App(id) => serde_json::json!({ "app": id }),
        Launch::Open(path) => serde_json::json!({ "open": path }),
        Launch::Files(paths) => serde_json::json!({ "files": paths }),
        Launch::None => serde_json::json!({}),
    }
}

pub fn apply_launch(app: &AppHandle) {
    let pending = {
        let mut guard = LAUNCH.lock().expect("launch");
        std::mem::replace(&mut *guard, Launch::None)
    };
    open_launch(app, pending);
    enqueue_files(app, &take_routed_files(app));
    let path = paths::root_local_dir(app).join(LAUNCH_FILE);
    if let Ok(text) = fs::read_to_string(&path) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
            let owner = value
                .get("profile")
                .and_then(|item| item.as_str())
                .unwrap_or("");
            if !owner.is_empty() && owner != profiles::active_id() {
                return;
            }
            let _ = fs::remove_file(&path);
            let launch = if let Some(url) = value.get("url").and_then(|item| item.as_str()) {
                Launch::Url(url.to_string())
            } else if let Some(id) = value.get("app").and_then(|item| item.as_str()) {
                Launch::App(id.to_string())
            } else if let Some(path) = value.get("open").and_then(|item| item.as_str()) {
                Launch::Open(path.to_string())
            } else {
                Launch::None
            };
            let queued = queued_files(&value);
            open_launch(app, launch);
            enqueue_files(app, &queued);
        } else {
            let _ = fs::remove_file(&path);
        }
    }
}

fn open_launch(app: &AppHandle, launch: Launch) {
    match launch {
        Launch::Url(url) if is_website(&url) => {
            let _ = tabs::create_tab(app, Some(url), false, None);
            tabs::show_main(app);
        }
        Launch::Url(_) => {}
        Launch::App(id) => {
            if let Some(site) = find_app(app, &id) {
                let _ = tabs::open_site_window(app, &site.name, &site.url);
                mark_open_site(app, &site);
            }
        }
        Launch::Open(path) => {
            let _ = crate::files::open_with_default(Path::new(&path));
        }
        Launch::Files(paths) => enqueue_files(app, &paths),
        Launch::None => {}
    }
}

/// Opens queued files once session restore has finished, so the handler tab stays selected.
pub fn open_pending_files(app: &AppHandle) {
    let paths = {
        let mut pending = FILE_LAUNCH.lock().expect("file launch");
        pending.ready = true;
        std::mem::take(&mut pending.paths)
    };
    open_files(app, &paths);
}

fn enqueue_files(app: &AppHandle, paths: &[String]) {
    if paths.is_empty() {
        return;
    }
    let mut pending = FILE_LAUNCH.lock().expect("file launch");
    if pending.ready {
        drop(pending);
        open_files(app, paths);
    } else {
        pending.paths.extend(paths.iter().cloned());
    }
}

/// Files handed to this profile by another process.
fn take_routed_files(app: &AppHandle) -> Vec<String> {
    let path = paths::root_local_dir(app).join(routed_file_name(&profiles::active_id()));
    let Ok(text) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    let _ = fs::remove_file(&path);
    serde_json::from_str::<serde_json::Value>(&text)
        .map(|value| queued_files(&value))
        .unwrap_or_default()
}

fn queued_files(value: &serde_json::Value) -> Vec<String> {
    value
        .get("files")
        .or_else(|| value.get("pdfs"))
        .and_then(|item| item.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str())
                .filter_map(file_argument)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn routed_file_name(profile: &str) -> String {
    let safe: String = profile
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
        .collect();
    format!("open-{safe}.json")
}

/// Sends files to a profile that is already open. That profile reads them on its next launch check.
fn handoff_paths(app: &AppHandle, profile: &str, paths: &[String]) {
    let path = paths::root_local_dir(app).join(routed_file_name(profile));
    let mut files = fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .map(|value| queued_files(&value))
        .unwrap_or_default();
    for item in paths {
        if !files.iter().any(|existing| existing == item) {
            files.push(item.clone());
        }
    }
    let _ = files::write_json(&path, &serde_json::json!({ "files": files }));
}

pub fn pending_files() -> Vec<String> {
    FILE_LAUNCH.lock().expect("file launch").paths.clone()
}

/// Profile this process should become when a file is opened and no `--profile` was passed.
/// A selected handler wins. Otherwise an already open profile is used, preferring the default.
pub fn launch_profile(app: &AppHandle) -> Option<String> {
    let files = pending_files();
    if files.is_empty() {
        return None;
    }
    if let Some(profile) = files.iter().find_map(|path| selected_profile(app, path)) {
        return Some(profile);
    }
    let ids = profiles::ids(app);
    if ids
        .iter()
        .any(|id| id == profiles::DEFAULT_ID && profiles::is_open(id))
    {
        return Some(profiles::DEFAULT_ID.to_string());
    }
    ids.into_iter().find(|id| profiles::is_open(id))
}

fn selected_profile(app: &AppHandle, path: &str) -> Option<String> {
    let extension = Path::new(path).extension().and_then(|ext| ext.to_str())?;
    let handler = chosen_handler(app, extension)?;
    if handler.profile.is_empty() {
        None
    } else {
        Some(handler.profile)
    }
}

/// The handler row that names a profile for this extension. The default profile's row wins.
fn chosen_handler(app: &AppHandle, extension: &str) -> Option<settings::FileHandler> {
    let ids = profiles::ids(app);
    let groups = ids
        .iter()
        .map(|id| (id.clone(), profile_handlers(app, id)))
        .collect::<Vec<_>>();
    let views = groups
        .iter()
        .map(|(id, handlers)| (id.as_str(), handlers.as_slice()))
        .collect::<Vec<_>>();
    chosen_handler_in(&views, extension, &ids)
}

fn profile_handlers(app: &AppHandle, id: &str) -> Vec<settings::FileHandler> {
    let path = paths::profile_data_dir(app, id).join("settings.json");
    let Ok(text) = fs::read_to_string(path) else {
        return settings::default_file_handlers();
    };
    serde_json::from_str::<settings::BrowserSettings>(&text)
        .map(|settings| settings.normalize().file_handlers)
        .unwrap_or_default()
}

/// `groups` is `(profile that stored the row, that profile's handlers)`, default profile first when it is first.
pub fn chosen_handler_in(
    groups: &[(&str, &[settings::FileHandler])],
    extension: &str,
    known: &[String],
) -> Option<settings::FileHandler> {
    let extension = extension
        .trim()
        .trim_start_matches('.')
        .to_ascii_lowercase();
    let mut first = None;
    for (owner, handlers) in groups {
        let Some(handler) = handlers
            .iter()
            .find(|handler| handler.extension == extension)
        else {
            continue;
        };
        if handler.profile.is_empty() || !known.iter().any(|id| id == &handler.profile) {
            continue;
        }
        if *owner == profiles::DEFAULT_ID {
            return Some(handler.clone());
        }
        if first.is_none() {
            first = Some(handler.clone());
        }
    }
    first
}

fn deliver(app: &AppHandle, profile: &str, path: &str) -> Result<(), String> {
    if profiles::is_open(profile) {
        handoff_paths(app, profile, &[path.to_string()]);
        profiles::focus(profile);
        return Ok(());
    }
    profiles::launch(profile, &[path.to_string()])
}

fn handler_for_path(app: &AppHandle, path: &str) -> Option<settings::FileHandler> {
    let extension = Path::new(path).extension().and_then(|ext| ext.to_str())?;
    if let Some(handler) = chosen_handler(app, extension) {
        return Some(handler);
    }
    settings::get(app).handler_for(extension).cloned()
}

/// True when this profile opens that file type in a tab.
pub fn has_handler(app: &AppHandle, path: &str) -> bool {
    handler_for_path(app, path).is_some()
}

/// Opens one handled file in `window`. A download clicked in a browser window stays in that window.
pub fn open_file_in_window(app: &AppHandle, path: &str, window: &str) -> Result<(), String> {
    let path = file_argument(path).ok_or_else(|| "That file can't be opened here.".to_string())?;
    if let Some(profile) = selected_profile(app, &path)
        && profile != profiles::active_id()
    {
        return deliver(app, &profile, &path);
    }
    let handler = handler_for_path(app, &path)
        .ok_or_else(|| "No file handler is set for that type.".to_string())?;
    sweep_old_files();
    open_one_file(app, &path, &handler, Some(window))?;
    reveal_window(app, Some(window));
    Ok(())
}

fn open_files(app: &AppHandle, paths: &[String]) {
    if paths.is_empty() {
        return;
    }
    sweep_old_files();
    let mut opened = false;
    for path in paths {
        if let Some(profile) = selected_profile(app, path)
            && profile != profiles::active_id()
        {
            if let Err(error) = deliver(app, &profile, path) {
                eprintln!("file: {error}");
            }
            continue;
        }
        let Some(handler) = handler_for_path(app, path) else {
            continue;
        };
        match open_one_file(app, path, &handler, None) {
            Ok(()) => opened = true,
            Err(error) => eprintln!("file: {error}"),
        }
    }
    if opened {
        reveal_window(app, None);
    }
}

fn open_one_file(
    app: &AppHandle,
    path: &str,
    handler: &settings::FileHandler,
    window: Option<&str>,
) -> Result<(), String> {
    if handler.native {
        return open_native_pdf(app, path, window);
    }
    let attach = stage_file(path, handler)?;
    let title = attach.file_name.clone();
    arm_file(attach);
    let url = tabs::resolve(app, &handler.url);
    let mut tab = tabs::OpenTab::new(url);
    tab.title = Some(title);
    if let Some(window) = window {
        tab.window = window.to_string();
    }
    if let Err(error) = tabs::open(app, tab) {
        if let Some(attach) = disarm_file() {
            let _ = fs::remove_dir_all(attach.dir);
        }
        return Err(error);
    }
    Ok(())
}

/// Loads the PDF at `path` in Edge's built-in PDF viewer.
fn open_native_pdf(app: &AppHandle, path: &str, window: Option<&str>) -> Result<(), String> {
    let file = PathBuf::from(path);
    if !file.is_file() {
        return Err(format!("Couldn't find {path}."));
    }
    let url = url::Url::from_file_path(&file).map_err(|_| format!("Couldn't open {path}."))?;
    let title = file
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("PDF")
        .to_string();
    let mut tab = tabs::OpenTab::new(url);
    tab.title = Some(title);
    if let Some(window) = window {
        tab.window = window.to_string();
    }
    tabs::open(app, tab)?;
    Ok(())
}

fn reveal_window(app: &AppHandle, window: Option<&str>) {
    let Some(label) = window.filter(|label| tabs::is_browser_label(label)) else {
        tabs::show_main(app);
        return;
    };
    let Some(browser) = app.get_window(label) else {
        tabs::show_main(app);
        return;
    };
    let _ = browser.unminimize();
    let _ = browser.show();
    let _ = browser.set_focus();
}

fn stage_file(source: &str, handler: &settings::FileHandler) -> Result<FileAttach, String> {
    let path = PathBuf::from(source);
    if !path.is_file() {
        return Err(format!("Couldn't find {source}."));
    }
    let id = rand::random::<u64>();
    let dir = std::env::temp_dir()
        .join("ce-browser-file")
        .join(format!("{id:x}"));
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let staged_name = format!("document.{}", handler.extension);
    let dest = dir.join(&staged_name);
    // A hard link keeps one copy of a large file. Another drive has to be copied.
    if fs::hard_link(&path, &dest).is_err() {
        fs::copy(&path, &dest).map_err(|error| format!("Couldn't read {source}. {error}"))?;
    }
    let fallback = format!("document.{}", handler.extension);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(fallback.as_str())
        .to_string();
    Ok(FileAttach {
        dir,
        fetch: format!("https://file-{id:x}.ce-browser.example/{staged_name}"),
        file_name,
        staged: dest,
        content_type: content_type(&handler.extension).to_string(),
        input_id: handler.input_id.clone(),
        editor_url: handler.url.clone(),
    })
}

fn sweep_old_files() {
    sweep_old_dir("ce-browser-file");
    sweep_old_dir("ce-browser-pdf");
}

fn sweep_old_dir(name: &str) {
    let root = std::env::temp_dir().join(name);
    let Ok(entries) = fs::read_dir(&root) else {
        return;
    };
    let Some(cutoff) = SystemTime::now().checked_sub(Duration::from_secs(24 * 60 * 60)) else {
        return;
    };
    for entry in entries.flatten() {
        let stale = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .is_ok_and(|modified| modified < cutoff);
        if stale {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

fn arm_file(attach: FileAttach) {
    *FILE_ARMED.lock().expect("file") = Some(attach);
}

fn disarm_file() -> Option<FileAttach> {
    FILE_ARMED.lock().expect("file").take()
}

/// The tab just created takes the file that was armed for it, before its page starts loading.
pub fn claim_file(id: u32) {
    if let Some(attach) = disarm_file() {
        FILE_TABS.lock().expect("file").push((id, attach));
    }
}

/// After a handler page finishes loading, give it the file from this open.
pub fn attach_open_file(app: &AppHandle, id: u32, url: &str) {
    let Some(attach) = FILE_TABS
        .lock()
        .expect("file")
        .iter()
        .find(|(tab, _)| *tab == id)
        .map(|(_, attach)| attach.clone())
    else {
        return;
    };
    if !pages_match(url, &attach.editor_url) {
        return;
    }
    let Some(webview) = tabs::page_view(app, id) else {
        return;
    };
    crate::webview2::attach_local_file(
        &webview,
        &attach.fetch,
        &attach.file_name,
        &attach.input_id,
        &attach.content_type,
    );
}

/// Bytes for a file fetch from a handler page. The address only exists inside this app.
pub fn file_bytes(url: &str) -> Option<ServedFile> {
    let attach = FILE_TABS
        .lock()
        .expect("file")
        .iter()
        .find_map(|(_, attach)| same_file_url(url, &attach.fetch).then(|| attach.clone()))?;
    let bytes = fs::read(&attach.staged).ok()?;
    Some(ServedFile {
        bytes,
        content_type: header_type(&attach.content_type),
    })
}

fn header_type(content_type: &str) -> String {
    let clean: String = content_type
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric()
                || *character == '/'
                || *character == '-'
                || *character == '+'
                || *character == '.'
        })
        .collect();
    if clean.is_empty() {
        "application/octet-stream".into()
    } else {
        clean
    }
}

fn same_file_url(request: &str, fetch: &str) -> bool {
    request.trim_end_matches('/') == fetch.trim_end_matches('/')
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct SiteApp {
    id: String,
    name: String,
    url: String,
}

impl Default for SiteApp {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            url: String::new(),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct AppFile {
    apps: Vec<SiteApp>,
}

pub fn install_site(app: &AppHandle, id: u32) -> Result<(), String> {
    let url = tabs::tab_page_url(app, id).ok_or("That tab is gone.")?;
    let site_id = site_id(&url).ok_or("Only a website can be installed.")?;
    let snapshot = tabs::snapshot(app);
    let name = snapshot
        .tabs
        .iter()
        .find(|tab| tab.id == id)
        .map(|tab| {
            if tab.title.is_empty() {
                site_id.clone()
            } else {
                tab.title.clone()
            }
        })
        .unwrap_or_else(|| site_id.clone());
    let icon = save_site_icon(
        app,
        &site_id,
        snapshot
            .tabs
            .iter()
            .find(|tab| tab.id == id)
            .and_then(|tab| tab.favicon.as_deref()),
    );
    let model = format!("com.ce.browser.site.{site_id}");
    let arguments = format!("--app {site_id}");
    let _ = profiles::install_link(
        &format!("{name}.lnk"),
        &arguments,
        &name,
        icon.as_deref(),
        &model,
    )?;
    let mut file: AppFile = files::read_json(&paths::data_file(app, APPS_FILE));
    file.apps.retain(|site| site.id != site_id);
    file.apps.insert(
        0,
        SiteApp {
            id: site_id,
            name,
            url,
        },
    );
    files::write_json(&paths::data_file(app, APPS_FILE), &file)
}

fn find_app(app: &AppHandle, id: &str) -> Option<SiteApp> {
    let file: AppFile = files::read_json(&paths::data_file(app, APPS_FILE));
    file.apps.into_iter().find(|site| site.id == id)
}

fn save_site_icon(app: &AppHandle, id: &str, favicon: Option<&str>) -> Option<PathBuf> {
    use std::io::Read;
    let fetched = favicon.and_then(|url| {
        if !is_website(url) {
            return None;
        }
        let response = ureq::get(url).timeout(Duration::from_secs(8)).call().ok()?;
        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(1_000_000)
            .read_to_end(&mut bytes)
            .ok()?;
        image::load_from_memory(&bytes).ok()
    });
    let image = fetched
        .or_else(|| image::load_from_memory(crate::profile_icons::default_png()).ok())?
        .to_rgba8();
    let dir = paths::data_dir(app).join("apps");
    fs::create_dir_all(&dir).ok()?;
    let ico = crate::profile_icons::encode_ico(&image).ok()?;
    let path = dir.join(format!("{id}.ico"));
    fs::write(&path, ico).ok()?;
    Some(path)
}

pub fn mark_site_window(app: &AppHandle, window: &tauri::Window, id: &str, name: &str) {
    let model = format!("com.ce.browser.site.{id}");
    let relaunch = paths::launch_exe()
        .map(|exe| format!("\"{}\" --app {id}", exe.display()))
        .unwrap_or_default();
    let icon = paths::data_dir(app).join("apps").join(format!("{id}.ico"));
    let icon = if icon.is_file() {
        icon.display().to_string()
    } else {
        String::new()
    };
    let _ = profiles::mark_model(window, &model, &relaunch, name, &icon);
    if icon_file_ok(&icon) {
        if let Ok(image) = tauri::image::Image::from_path(&icon) {
            let _ = window.set_icon(image);
        }
    }
}

fn icon_file_ok(path: &str) -> bool {
    !path.is_empty() && Path::new(path).is_file()
}

fn mark_open_site(app: &AppHandle, site: &SiteApp) {
    let labels = tabs::browser_labels(app);
    if let Some(label) = labels.last() {
        if let Some(window) = app.get_window(label) {
            let _ = window.set_title(&site.name);
            mark_site_window(app, &window, &site.id, &site.name);
        }
    }
}

pub fn summon(app: &AppHandle) {
    tabs::show_main(app);
}

fn start_hotkey(app: AppHandle) {
    std::thread::spawn(move || {
        let token = settings::get(&app).summon_hotkey;
        let Some((mods, key)) = hotkey_mods(if token.trim().is_empty() {
            DEFAULT_HOTKEY
        } else {
            token.as_str()
        }) else {
            return;
        };
        unsafe { hotkey_loop(&app, mods, key) };
    });
}

unsafe fn hotkey_loop(app: &AppHandle, mods: u32, key: u32) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{HOT_KEY_MODIFIERS, RegisterHotKey};
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, MSG, TranslateMessage,
    };
    unsafe {
        if RegisterHotKey(None, 1, HOT_KEY_MODIFIERS(mods), key).is_err() {
            return;
        }
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            if message.message == 0x0312 {
                summon(app);
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

fn install_tray(app: &AppHandle) -> Result<(), String> {
    use tauri::image::Image;
    use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
    let icon = Image::from_bytes(crate::profile_icons::default_png())
        .map_err(|error| error.to_string())?;
    let menu = tray_menu(app)?;
    let tray = TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            if id == "show" || id == "search" {
                summon(app);
            } else if id == "quit" {
                app.exit(0);
            } else if let Some(tab) = id.strip_prefix("tab-") {
                if let Ok(tab) = tab.parse::<u32>() {
                    let _ = tabs::activate(app, tab);
                    summon(app);
                }
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                ..
            } = event
            {
                summon(tray.app_handle());
            }
        })
        .build(app)
        .map_err(|error| error.to_string())?;
    let _ = TRAY.set(tray);
    Ok(())
}

fn tray_menu(app: &AppHandle) -> Result<tauri::menu::Menu<tauri::Wry>, String> {
    use tauri::menu::{Menu, MenuItem};
    let mut items: Vec<MenuItem<tauri::Wry>> = Vec::new();
    for (id, title) in tabs::recent_tabs(app) {
        let label: String = title.chars().take(80).collect();
        if let Ok(item) = MenuItem::with_id(app, format!("tab-{id}"), label, true, None::<&str>) {
            items.push(item);
        }
    }
    let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>)
        .map_err(|error| error.to_string())?;
    let search = MenuItem::with_id(app, "search", "Quick search", true, None::<&str>)
        .map_err(|error| error.to_string())?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)
        .map_err(|error| error.to_string())?;
    let mut refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> =
        items.iter().map(|item| item as _).collect();
    refs.push(&show);
    refs.push(&search);
    refs.push(&quit);
    Menu::with_items(app, &refs).map_err(|error| error.to_string())
}

pub fn refresh_tray(app: &AppHandle) {
    let Some(tray) = TRAY.get() else {
        return;
    };
    if let Ok(menu) = tray_menu(app) {
        let _ = tray.set_menu(Some(menu));
    }
}

/// How a Windows toast should sound. WebView2's own notification is silent, so site
/// notifications go through this instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastSound {
    /// One system notification chime.
    Default,
    /// Banner only.
    Silent,
    /// Rings until the toast is dismissed. For an incoming call.
    Call,
}

/// A click on a site toast. The id is the one embedded in the toast argument.
#[derive(Debug, PartialEq, Eq)]
pub enum NoteAction {
    Open(u64),
    Dismiss(u64),
}

pub fn note_open_arg(id: u64) -> String {
    format!("ce-note-open:{id}")
}

pub fn note_dismiss_arg(id: u64) -> String {
    format!("ce-note-dismiss:{id}")
}

pub fn note_action(argument: &str) -> Option<NoteAction> {
    let (kind, id) = argument.split_once(':')?;
    let id = id.parse().ok()?;
    match kind {
        "ce-note-open" => Some(NoteAction::Open(id)),
        "ce-note-dismiss" => Some(NoteAction::Dismiss(id)),
        _ => None,
    }
}

/// An incoming call, as opposed to a message that merely mentions calling.
/// Missed-call alerts stay ordinary notifications so they do not keep ringing.
pub fn notification_is_call(title: &str, body: &str, tag: &str) -> bool {
    let tag = tag.trim().to_ascii_lowercase();
    let tagged = tag == "call"
        || tag.contains("incoming-call")
        || tag.contains("incoming_call")
        || tag.contains("phonecall")
        || tag.contains("voice-call")
        || tag.contains("video-call");
    if tagged {
        return true;
    }
    let text = format!("{title} {body}").to_lowercase();
    if text.contains("missed call")
        || text.contains("missed voice")
        || text.contains("missed video")
    {
        return false;
    }
    const PHRASES: &[&str] = &[
        "incoming call",
        "incoming voice",
        "incoming video",
        "video call",
        "voice call",
        "phone call",
        "is calling",
        "calling you",
        "wants to call",
        "call from",
        "started a call",
    ];
    PHRASES.iter().any(|phrase| text.contains(phrase))
}

/// `page_audible` means the tab is already making sound the user can hear. A call
/// then keeps the page's own ringtone. Otherwise the toast rings, including when
/// the site marked the notification silent and plays a ringtone this browser cannot.
pub fn notification_sound(silent: bool, call: bool, page_audible: bool) -> ToastSound {
    if call && !page_audible {
        ToastSound::Call
    } else if silent {
        ToastSound::Silent
    } else {
        ToastSound::Default
    }
}

pub(crate) fn toast_markup(
    launch: &str,
    title: &str,
    body: &str,
    sound: ToastSound,
    decline: &str,
) -> String {
    let audio = match sound {
        ToastSound::Default => r#"<audio src="ms-winsoundevent:Notification.Default"/>"#,
        ToastSound::Silent => r#"<audio silent="true"/>"#,
        ToastSound::Call => {
            r#"<audio src="ms-winsoundevent:Notification.Looping.Call" loop="true"/>"#
        }
    };
    let scenario = if sound == ToastSound::Call {
        r#" scenario="incomingCall""#
    } else {
        ""
    };
    let actions = if sound == ToastSound::Call {
        format!(
            r#"<actions><action content="Answer" arguments="{}"/><action content="Decline" arguments="{}"/></actions>"#,
            xml_text(launch),
            xml_text(decline),
        )
    } else {
        String::new()
    };
    format!(
        r#"<toast launch="{}"{scenario}><visual><binding template="ToastGeneric"><text>{}</text><text>{}</text></binding></visual>{actions}{audio}</toast>"#,
        xml_text(launch),
        xml_text(title),
        xml_text(body),
    )
}

pub fn toast(title: &str, body: &str, launch: &str) {
    let _ = present_toast(&toast_markup(launch, title, body, ToastSound::Default, ""));
}

pub(crate) fn create_toast(markup: &str) -> Option<windows::UI::Notifications::ToastNotification> {
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::UI::Notifications::ToastNotification;
    let document = XmlDocument::new().ok()?;
    document
        .LoadXml(&windows::core::HSTRING::from(markup))
        .ok()?;
    ToastNotification::CreateToastNotification(&document).ok()
}

pub(crate) fn show_created(toast: &windows::UI::Notifications::ToastNotification) -> bool {
    notifier().is_some_and(|notifier| notifier.Show(toast).is_ok())
}

pub(crate) fn hide_created(toast: &windows::UI::Notifications::ToastNotification) {
    if let Some(notifier) = notifier() {
        let _ = notifier.Hide(toast);
    }
}

fn present_toast(markup: &str) -> bool {
    create_toast(markup).is_some_and(|toast| show_created(&toast))
}

fn notifier() -> Option<windows::UI::Notifications::ToastNotifier> {
    windows::UI::Notifications::ToastNotificationManager::CreateToastNotifierWithId(
        &windows::core::HSTRING::from(profiles::current_model_id()),
    )
    .ok()
}

pub fn toast_download(name: &str, path: &str) {
    toast("Download finished", name, &format!("--open={path}"));
}

fn xml_text(text: &str) -> String {
    let cleaned: String = text
        .chars()
        .filter(|c| matches!(c, '\t' | '\n' | '\r') || !c.is_control())
        .collect();
    xml_escape(&cleaned)
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const FILE_ASSOCIATIONS: &str =
    r"Software\Clients\StartMenuInternet\ce-browser\Capabilities\FileAssociations";

pub fn register_browser(app: &AppHandle) -> Result<(), String> {
    let exe = paths::launch_exe().map_err(|error| error.to_string())?;
    let command = format!("\"{}\" \"%1\"", exe.display());
    set_key(r"Software\Classes\ce-browser", "", "ce-browser")?;
    set_key(
        r"Software\Classes\ce-browser\shell\open\command",
        "",
        &command,
    )?;
    set_key(r"Software\Classes\ce-browserHTML", "", "HTML document")?;
    set_key(
        r"Software\Classes\ce-browserHTML\shell\open\command",
        "",
        &command,
    )?;
    set_key(
        r"Software\Clients\StartMenuInternet\ce-browser",
        "",
        "ce-browser",
    )?;
    set_key(
        r"Software\Clients\StartMenuInternet\ce-browser\Capabilities",
        "ApplicationName",
        "ce-browser",
    )?;
    set_key(
        r"Software\Clients\StartMenuInternet\ce-browser\Capabilities\URLAssociations",
        "http",
        "ce-browser",
    )?;
    set_key(
        r"Software\Clients\StartMenuInternet\ce-browser\Capabilities\URLAssociations",
        "https",
        "ce-browser",
    )?;
    set_key(
        r"Software\Clients\StartMenuInternet\ce-browser\Capabilities\FileAssociations",
        ".html",
        "ce-browserHTML",
    )?;
    let handlers = settings::get(app).file_handlers.clone();
    let exe_name = exe
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string);
    if let Some(name) = &exe_name {
        let apps = format!(r"Software\Classes\Applications\{name}");
        set_key(&apps, "FriendlyAppName", "ce-browser")?;
        set_key(&format!(r"{apps}\shell\open\command"), "", &command)?;
    }
    for handler in &handlers {
        register_handler(&exe, &command, handler, exe_name.as_deref())?;
    }
    prune_file_handlers(&handlers, exe_name.as_deref());
    set_key(
        r"Software\RegisteredApplications",
        "ce-browser",
        r"Software\Clients\StartMenuInternet\ce-browser\Capabilities",
    )?;
    Ok(())
}

fn handler_progid(extension: &str) -> String {
    if extension.eq_ignore_ascii_case("pdf") {
        "ce-browserPDF".into()
    } else {
        format!("ce-browser.{extension}")
    }
}

fn handler_label(extension: &str) -> String {
    if extension.eq_ignore_ascii_case("pdf") {
        "PDF Document".into()
    } else {
        format!("{} file", extension.to_ascii_uppercase())
    }
}

fn register_handler(
    exe: &Path,
    command: &str,
    handler: &settings::FileHandler,
    exe_name: Option<&str>,
) -> Result<(), String> {
    let progid = handler_progid(&handler.extension);
    let dot = format!(".{}", handler.extension);
    let class = format!(r"Software\Classes\{progid}");
    set_key(&class, "", &handler_label(&handler.extension))?;
    set_key(&class, "Content Type", content_type(&handler.extension))?;
    set_key(
        &format!(r"{class}\DefaultIcon"),
        "",
        &format!("\"{}\",0", exe.display()),
    )?;
    set_key(&format!(r"{class}\shell\open\command"), "", command)?;
    set_key(
        &format!(r"Software\Classes\{dot}\OpenWithProgids"),
        &progid,
        "",
    )?;
    set_key(FILE_ASSOCIATIONS, &dot, &progid)?;
    if let Some(name) = exe_name {
        set_key(
            &format!(r"Software\Classes\Applications\{name}\SupportedTypes"),
            &dot,
            "",
        )?;
    }
    Ok(())
}

/// Drops Open With and file-association entries for extensions this profile no longer handles.
fn prune_file_handlers(handlers: &[settings::FileHandler], exe_name: Option<&str>) {
    let wanted: Vec<String> = handlers
        .iter()
        .map(|handler| format!(".{}", handler.extension))
        .collect();
    let retired = |name: &str| {
        !name.eq_ignore_ascii_case(".html")
            && !wanted.iter().any(|ext| ext.eq_ignore_ascii_case(name))
    };
    for (name, value) in list_values(FILE_ASSOCIATIONS) {
        if !retired(&name) || !value.starts_with("ce-browser") {
            continue;
        }
        delete_value(FILE_ASSOCIATIONS, &name);
        delete_value(&format!(r"Software\Classes\{name}\OpenWithProgids"), &value);
    }
    if let Some(name) = exe_name {
        let supported = format!(r"Software\Classes\Applications\{name}\SupportedTypes");
        for (entry, _) in list_values(&supported) {
            if retired(&entry) {
                delete_value(&supported, &entry);
            }
        }
    }
}

pub fn open_default_settings() -> Result<(), String> {
    std::process::Command::new("cmd")
        .args(["/C", "start", "", "ms-settings:defaultapps"])
        .spawn()
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn set_key(path: &str, name: &str, value: &str) -> Result<(), String> {
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegCreateKeyExW(
            key: isize,
            sub: *const u16,
            reserved: u32,
            class: *const u16,
            options: u32,
            access: u32,
            security: *mut core::ffi::c_void,
            result: *mut isize,
            disposition: *mut u32,
        ) -> i32;
        fn RegSetValueExW(
            key: isize,
            name: *const u16,
            reserved: u32,
            kind: u32,
            data: *const u8,
            bytes: u32,
        ) -> i32;
        fn RegCloseKey(key: isize) -> i32;
    }
    let sub = wide(path);
    let name = wide(name);
    let mut data = wide(value);
    data.pop();
    let bytes = (data.len() * 2) as u32;
    unsafe {
        let mut key = 0isize;
        let mut disposition = 0u32;
        let status = RegCreateKeyExW(
            0x8000_0001u32 as isize,
            sub.as_ptr(),
            0,
            std::ptr::null(),
            0,
            0x20006,
            std::ptr::null_mut(),
            &mut key,
            &mut disposition,
        );
        if status != 0 {
            return Err(format!("Couldn't register the browser ({status})."));
        }
        let status = RegSetValueExW(key, name.as_ptr(), 0, 1, data.as_ptr().cast(), bytes + 2);
        let _ = RegCloseKey(key);
        if status != 0 {
            return Err(format!("Couldn't register the browser ({status})."));
        }
    }
    Ok(())
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn open_key(path: &str, access: u32) -> Option<isize> {
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegOpenKeyExW(
            key: isize,
            sub: *const u16,
            options: u32,
            access: u32,
            result: *mut isize,
        ) -> i32;
    }
    let sub = wide(path);
    let mut key = 0isize;
    let status = unsafe { RegOpenKeyExW(0x8000_0001, sub.as_ptr(), 0, access, &mut key) };
    if status == 0 { Some(key) } else { None }
}

fn close_key(key: isize) {
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegCloseKey(key: isize) -> i32;
    }
    let _ = unsafe { RegCloseKey(key) };
}

fn list_values(path: &str) -> Vec<(String, String)> {
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegEnumValueW(
            key: isize,
            index: u32,
            name: *mut u16,
            name_len: *mut u32,
            reserved: *mut u32,
            kind: *mut u32,
            data: *mut u8,
            data_len: *mut u32,
        ) -> i32;
    }
    let Some(key) = open_key(path, 0x20019) else {
        return Vec::new();
    };
    let mut values = Vec::new();
    let mut index = 0u32;
    loop {
        let mut name = vec![0u16; 256];
        let mut name_len = name.len() as u32;
        let mut kind = 0u32;
        let mut data = vec![0u8; 1024];
        let mut data_len = data.len() as u32;
        let status = unsafe {
            RegEnumValueW(
                key,
                index,
                name.as_mut_ptr(),
                &mut name_len,
                std::ptr::null_mut(),
                &mut kind,
                data.as_mut_ptr(),
                &mut data_len,
            )
        };
        if status == 234 {
            index += 1;
            continue;
        }
        if status != 0 {
            break;
        }
        index += 1;
        if kind != 1 {
            continue;
        }
        let name = String::from_utf16_lossy(&name[..name_len as usize]);
        let units: Vec<u16> = data[..data_len as usize]
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .take_while(|unit| *unit != 0)
            .collect();
        values.push((name, String::from_utf16_lossy(&units)));
    }
    close_key(key);
    values
}

fn delete_value(path: &str, name: &str) {
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegDeleteValueW(key: isize, name: *const u16) -> i32;
    }
    let Some(key) = open_key(path, 0x0002) else {
        return;
    };
    let name = wide(name);
    let _ = unsafe { RegDeleteValueW(key, name.as_ptr()) };
    close_key(key);
}

pub fn saver_active(app: &AppHandle) -> bool {
    settings::get(app).battery_saver && power_is_battery(ac_line())
}

pub fn on_battery() -> bool {
    power_is_battery(ac_line())
}

fn ac_line() -> u8 {
    #[repr(C)]
    struct Power {
        ac: u8,
        flag: u8,
        percent: u8,
        status: u8,
        life: u32,
        full: u32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetSystemPowerStatus(status: *mut Power) -> i32;
    }
    let mut status = Power {
        ac: 255,
        flag: 0,
        percent: 0,
        status: 0,
        life: 0,
        full: 0,
    };
    if unsafe { GetSystemPowerStatus(&mut status) } == 0 {
        return 255;
    }
    status.ac
}

pub fn sync_frames(app: &AppHandle) {
    let on = saver_active(app);
    let changed = {
        let mut guard = FRAMES_CAPPED.lock().expect("frames");
        if *guard == on {
            false
        } else {
            *guard = on;
            true
        }
    };
    if !changed {
        return;
    }
    let script = format!(
        r#"(function(on) {{
          if (on) {{
            if (window.__ceFps) return;
            window.__ceFpsNative = window.requestAnimationFrame.bind(window);
            var last = 0;
            window.requestAnimationFrame = function(callback) {{
              return window.__ceFpsNative(function(time) {{
                if (time - last < 33) {{ window.requestAnimationFrame(callback); return; }}
                last = time;
                callback(time);
              }});
            }};
            window.__ceFps = true;
          }} else if (window.__ceFpsNative) {{
            window.requestAnimationFrame = window.__ceFpsNative;
            window.__ceFps = false;
          }}
        }})({on})"#
    );
    for (id, _) in tabs::recent_tabs(app) {
        if let Some(webview) = tabs::page_view(app, id) {
            let _ = webview.eval(&script);
        }
    }
}

/// Keeps `Notification.permission` granted for a site that was allowed, and tells the host
/// about the notification. A current WebView2 raises `NotificationReceived` and shows one
/// toast with sound, so the host ignores this message then. Older runtimes use it as the
/// only path, which is why `silent` and `tag` are included.
pub const NOTIFY_JS: &str = r#"(function () {
  if (window.__ceNotify || !window.Notification) return;
  window.__ceNotify = true;
  var Native = window.Notification;
  function callText(title, options) {
    var body = options && options.body ? String(options.body) : "";
    var tag = options && options.tag ? String(options.tag) : "";
    var text = (String(title || "") + " " + body).toLowerCase();
    var mark = tag.toLowerCase();
    if (mark === "call" || mark.indexOf("incoming-call") >= 0 || mark.indexOf("incoming_call") >= 0 || mark.indexOf("voice-call") >= 0 || mark.indexOf("video-call") >= 0) return true;
    if (text.indexOf("missed call") >= 0 || text.indexOf("missed voice") >= 0) return false;
    return text.indexOf("incoming call") >= 0 || text.indexOf("incoming voice") >= 0 || text.indexOf("incoming video") >= 0 || text.indexOf("voice call") >= 0 || text.indexOf("video call") >= 0 || text.indexOf("is calling") >= 0 || text.indexOf("calling you") >= 0;
  }
  function Wrapped(title, options) {
    try {
      if (callText(title, options) && window.__ceAllowPlayback) window.__ceAllowPlayback();
      var host = window.__ceHost || (window.chrome && window.chrome.webview);
      if (host) host.postMessage({
        type: "ce-notify",
        title: String(title || ""),
        body: options && options.body ? String(options.body) : "",
        silent: !!(options && options.silent),
        tag: options && options.tag ? String(options.tag) : ""
      });
    } catch (error) {}
    return new Native(title, options);
  }
  Wrapped.permission = "granted";
  Wrapped.requestPermission = function () { return Promise.resolve("granted"); };
  window.Notification = Wrapped;
})();"#;

#[cfg(test)]
mod tests {
    use super::{
        Launch, chosen_handler_in, handler_matches, hotkey_mods, is_website, launch_from_args,
        power_is_battery, site_id,
    };
    use crate::settings::{FileHandler, default_file_handlers};

    #[test]
    fn a_site_id_comes_from_the_host() {
        assert_eq!(
            site_id("https://www.example.com/a").as_deref(),
            Some("example-com")
        );
        assert_eq!(site_id("ce://settings"), None);
    }

    #[test]
    fn a_command_line_website_is_kept() {
        assert_eq!(
            launch_from_args(&["https://example.com".into()]),
            Launch::Url("https://example.com".into())
        );
        assert_eq!(launch_from_args(&["ce://settings".into()]), Launch::None);
        assert!(!is_website("ce://settings"));
    }

    #[test]
    fn a_command_line_file_is_queued_for_a_handler() {
        assert_eq!(
            launch_from_args(&[
                "--profile".into(),
                "default".into(),
                r"C:\docs\note.PDF".into()
            ]),
            Launch::Files(vec![r"C:\docs\note.PDF".into()])
        );
        assert_eq!(
            launch_from_args(&["file:///C:/docs/note.pdf".into()]),
            Launch::Files(vec![r"C:\docs\note.pdf".into()])
        );
        assert_eq!(
            launch_from_args(&[r"C:\docs\photo.png".into()]),
            Launch::Files(vec![r"C:\docs\photo.png".into()])
        );
        assert_eq!(launch_from_args(&[r"C:\docs\README".into()]), Launch::None);
        assert_eq!(
            launch_from_args(&[r"--open=C:\docs\note.pdf".into()]),
            Launch::Open(r"C:\docs\note.pdf".into())
        );
        let defaults = default_file_handlers();
        assert!(defaults[0].native);
        assert!(!handler_matches(
            "https://ce-file-tools.vercel.app/pdf/view-and-markup",
            &defaults
        ));
        let custom = vec![FileHandler {
            extension: "pdf".into(),
            url: "https://example.com/pdf/view-and-markup".into(),
            input_id: "pdf-dropzone-input".into(),
            profile: String::new(),
            native: false,
        }];
        assert!(handler_matches(
            "https://example.com/pdf/view-and-markup",
            &custom
        ));
        assert!(!handler_matches(
            "https://ce-file-tools.vercel.app/pdf/view-and-markup",
            &custom
        ));
    }

    #[test]
    fn the_default_profile_picks_where_a_file_type_opens() {
        let stock = FileHandler {
            extension: "pdf".into(),
            url: "https://example.com/default".into(),
            input_id: "pdf-dropzone-input".into(),
            profile: String::new(),
            native: false,
        };
        let work = FileHandler {
            extension: "pdf".into(),
            url: "https://example.com/work".into(),
            input_id: "drop".into(),
            profile: "personal".into(),
            native: false,
        };
        let known = [
            "default".to_string(),
            "work".to_string(),
            "personal".to_string(),
        ];
        let chosen = chosen_handler_in(
            &[("default", &[stock.clone()]), ("work", &[work.clone()])],
            "pdf",
            &known,
        );
        assert_eq!(chosen.unwrap().url, "https://example.com/work");
        let assigned = FileHandler {
            profile: "work".into(),
            ..stock.clone()
        };
        let chosen = chosen_handler_in(
            &[("default", &[assigned]), ("work", &[work])],
            "PDF",
            &known,
        );
        assert_eq!(chosen.unwrap().profile, "work");
        let missing = FileHandler {
            profile: "gone".into(),
            ..stock
        };
        assert!(chosen_handler_in(&[("default", &[missing])], "pdf", &known).is_none());
    }

    #[test]
    fn the_summon_hotkey_is_ctrl_alt_space() {
        assert_eq!(hotkey_mods("ctrl+alt+space"), Some((0x0002 | 0x0001, 0x20)));
        assert_eq!(hotkey_mods("nope"), None);
    }

    #[test]
    fn battery_power_is_unplugged_only() {
        assert!(power_is_battery(0));
        assert!(!power_is_battery(1));
        assert!(!power_is_battery(255));
    }

    #[test]
    fn a_ringing_call_is_not_a_missed_call_or_a_mention() {
        use super::{
            NoteAction, ToastSound, note_action, note_dismiss_arg, note_open_arg,
            notification_is_call, notification_sound,
        };
        assert!(notification_is_call("Alice", "Incoming voice call", ""));
        assert!(notification_is_call("Bob", "Bob is calling you", ""));
        assert!(notification_is_call("Meet", "Call from Dana", "ping"));
        assert!(notification_is_call("Phone", "", "incoming-call"));
        assert!(!notification_is_call("Alice", "Missed voice call", ""));
        assert!(!notification_is_call("Alice", "I'll call you later", ""));
        assert_eq!(notification_sound(true, true, false), ToastSound::Call);
        assert_eq!(notification_sound(true, true, true), ToastSound::Silent);
        assert_eq!(notification_sound(false, false, false), ToastSound::Default);
        assert_eq!(notification_sound(true, false, false), ToastSound::Silent);
        assert_eq!(note_action(&note_open_arg(7)), Some(NoteAction::Open(7)));
        assert_eq!(
            note_action(&note_dismiss_arg(7)),
            Some(NoteAction::Dismiss(7))
        );
        assert_eq!(note_action("--open=C:\\a.pdf"), None);
    }

    #[test]
    fn a_call_toast_loops_and_a_normal_one_chimes() {
        use super::{ToastSound, toast_markup};
        let call = toast_markup(
            "ce-note-open:4",
            "Alice & Bob",
            "Incoming call",
            ToastSound::Call,
            "ce-note-dismiss:4",
        );
        assert!(call.contains(r#"scenario="incomingCall""#));
        assert!(call.contains("ms-winsoundevent:Notification.Looping.Call"));
        assert!(call.contains(r#"loop="true""#));
        assert!(call.contains("Answer"));
        assert!(call.contains("Decline"));
        assert!(call.contains("Alice &amp; Bob"));
        assert!(!call.contains("Alice & Bob"));
        let chime = toast_markup("open", "Done", "Saved", ToastSound::Default, "");
        assert!(chime.contains("ms-winsoundevent:Notification.Default"));
        assert!(!chime.contains("incomingCall"));
        let quiet = toast_markup("open", "Done", "Saved", ToastSound::Silent, "");
        assert!(quiet.contains(r#"silent="true""#));
    }
}
