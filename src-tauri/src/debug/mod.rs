//! Debug HUD: an IPC and event feed, a webview inspector, a state viewer and the backend log.
//!
//! It is a built-in extension, so it runs in a dev build and in a release build when that
//! extension is on. Passage stays dev-only. The commands stay registered either way and refuse
//! while the extension is off.

mod hud;
mod outline;
mod passage;
pub mod stderr;
mod webviews;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Window, WindowEvent};
use windows::Win32::Foundation::{CloseHandle, HWND, POINT};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GA_ROOT, GetAncestor, GetWindowTextW, GetWindowThreadProcessId, WindowFromPoint,
};
use windows::core::PWSTR;

use crate::{files, paths, popup, profiles, tabs};

pub use hud::{contains_point as hud_contains_point, follow};
pub use webviews::DebugWebview;

/// Records and log lines each HUD can scroll back through.
const RING_SIZE: usize = 2000;
/// The feed is sent in batches so a busy page does not raise one event per call.
const FLUSH_EVERY: Duration = Duration::from_millis(150);
const FEED_EVENT: &str = "debug-feed";

/// Keep in sync with the Debug HUD id in `frontend/src/features/BuiltInExtensions/builtin.ts`.
pub const DEBUG_HUD_ID: &str = "debug-hud";

static HUD_ON: AtomicBool = AtomicBool::new(false);

/// The Debug HUD extension is on. Settings apply this during startup: on in a dev build, off in a release build until it is enabled.
pub fn hud_enabled() -> bool {
    HUD_ON.load(Ordering::Relaxed)
}

/// Applies the extension toggle. Turning it on starts the log capture. Turning it off hides the HUD.
pub fn apply_tool(app: &AppHandle, disabled_builtin_tools: &[String]) {
    let on = !disabled_builtin_tools.iter().any(|id| id == DEBUG_HUD_ID);
    let was = HUD_ON.swap(on, Ordering::Relaxed);
    if on {
        stderr::capture();
        return;
    }
    if was {
        hud::hide_all(app);
        outline::clear(app);
    }
}

pub fn require() -> Result<(), String> {
    if hud_enabled() {
        Ok(())
    } else {
        Err("Debug HUD is turned off. Enable it on the Extensions page.".into())
    }
}

/// The HUD (`debug-<window>`) and the outline (`debug-outline-<window>`).
pub fn is_debug_label(label: &str) -> bool {
    label.starts_with("debug-")
}

/// The browser window a debug, popup or browser window belongs to.
pub fn owner_of(label: &str) -> String {
    if let Some(window) = label.strip_prefix("debug-outline-") {
        return window.to_string();
    }
    if let Some(window) = label.strip_prefix("debug-") {
        return window.to_string();
    }
    popup::owner_of(label)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct DebugIpcRecord {
    /// Given by the backend, so records from different webviews never share one.
    pub id: u64,
    pub webview: String,
    /// `invoke` or `event`.
    pub kind: String,
    pub name: String,
    /// Unix ms.
    pub started_at: f64,
    pub duration_ms: Option<f64>,
    pub ok: bool,
    pub args_preview: String,
    pub result_preview: String,
    pub error: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DebugLogLine {
    pub id: u64,
    /// Unix ms.
    pub at: f64,
    /// `stdout`, `stderr` or `console`.
    pub stream: String,
    /// The `area` in `eprintln!("area: message")`. Empty when the line has none.
    pub area: String,
    pub text: String,
}

/// A console message or uncaught error from a frontend webview.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct DebugConsoleEntry {
    /// `error`, `warn`, `uncaught` or `rejection`.
    pub level: String,
    pub text: String,
    /// Unix ms.
    pub at: f64,
    /// `file:line:column` when the browser gave one.
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugSnapshot {
    pub ipc: Vec<DebugIpcRecord>,
    pub logs: Vec<DebugLogLine>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DebugFeed {
    Ipc {
        records: Vec<DebugIpcRecord>,
    },
    Log {
        lines: Vec<DebugLogLine>,
    },
    /// `ipc` or `logs`.
    Cleared {
        target: String,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugStoreFile {
    pub name: String,
    pub path: String,
    pub bytes: u64,
    /// Unix ms. 0 when the file system does not say.
    pub modified: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugPaths {
    pub profile: String,
    pub root_data_dir: String,
    pub data_root: String,
    pub data_dir: String,
    pub profile_dir: String,
    /// Files directly inside `data_dir`, such as `settings.json` and `session.json`.
    pub files: Vec<DebugStoreFile>,
}

/// Oldest items fall off the front once `capacity` is reached.
#[derive(Debug)]
struct Ring<T> {
    items: VecDeque<T>,
    capacity: usize,
}

impl<T: Clone> Ring<T> {
    fn new(capacity: usize) -> Self {
        Self {
            items: VecDeque::new(),
            capacity,
        }
    }

    fn push(&mut self, item: T) {
        if self.capacity == 0 {
            return;
        }
        while self.items.len() >= self.capacity {
            self.items.pop_front();
        }
        self.items.push_back(item);
    }

    fn to_vec(&self) -> Vec<T> {
        self.items.iter().cloned().collect()
    }

    fn take(&mut self) -> Vec<T> {
        self.items.drain(..).collect()
    }

    fn clear(&mut self) {
        self.items.clear();
    }

    fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

struct Feed {
    next_id: u64,
    ipc: Ring<DebugIpcRecord>,
    logs: Ring<DebugLogLine>,
    /// Not yet sent to the HUD.
    unsent_ipc: Ring<DebugIpcRecord>,
    unsent_logs: Ring<DebugLogLine>,
}

static FEED: LazyLock<Mutex<Feed>> = LazyLock::new(|| {
    Mutex::new(Feed {
        next_id: 1,
        ipc: Ring::new(RING_SIZE),
        logs: Ring::new(RING_SIZE),
        unsent_ipc: Ring::new(RING_SIZE),
        unsent_logs: Ring::new(RING_SIZE),
    })
});

static APP: OnceLock<AppHandle> = OnceLock::new();

/// The log reader must never panic: a dead reader would make every later `eprintln!` fail.
fn feed() -> MutexGuard<'static, Feed> {
    FEED.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

pub fn init(app: &AppHandle) {
    if APP.set(app.clone()).is_err() {
        return;
    }
    if tauri::is_dev() {
        passage::start(app);
    }
    let app = app.clone();
    let _ = std::thread::Builder::new()
        .name("debug-feed".into())
        .spawn(move || {
            loop {
                std::thread::sleep(FLUSH_EVERY);
                flush(&app);
            }
        });
}

fn flush(app: &AppHandle) {
    let (records, lines) = {
        let mut feed = feed();
        if feed.unsent_ipc.is_empty() && feed.unsent_logs.is_empty() {
            return;
        }
        (feed.unsent_ipc.take(), feed.unsent_logs.take())
    };
    let huds = hud::labels(app);
    if huds.is_empty() {
        return;
    }
    let mut messages = Vec::new();
    if !records.is_empty() {
        messages.push(DebugFeed::Ipc { records });
    }
    if !lines.is_empty() {
        messages.push(DebugFeed::Log { lines });
    }
    for label in &huds {
        for message in &messages {
            let _ = app.emit_to(label.as_str(), FEED_EVENT, message);
        }
    }
}

pub(crate) fn push_log(stream: &str, text: &str) {
    push_line(stream, stderr::area_of(text), text, now_ms());
}

/// `text` starts with `area: ` when `area` is not empty, as the HUD strips that prefix.
fn push_line(stream: &str, area: &str, text: &str, at: f64) {
    if text.trim().is_empty() {
        return;
    }
    let mut feed = feed();
    let line = DebugLogLine {
        id: feed.next_id,
        at,
        stream: stream.to_string(),
        area: area.to_string(),
        text: text.to_string(),
    };
    feed.next_id += 1;
    feed.logs.push(line.clone());
    feed.unsent_logs.push(line);
}

/// The area and line a console entry from webview `webview` is logged as.
fn console_line(webview: &str, entry: &DebugConsoleEntry) -> (String, String) {
    let level = if entry.level.trim().is_empty() {
        "error"
    } else {
        entry.level.trim()
    };
    let area = format!("{webview} {level}");
    let mut text = format!("{area}: {}", entry.text.trim_end());
    if !entry.source.is_empty() {
        text.push_str(&format!(" ({})", entry.source));
    }
    (area, text)
}

fn push_console(webview: &str, batch: Vec<DebugConsoleEntry>) {
    for entry in batch {
        let (area, text) = console_line(webview, &entry);
        let at = if entry.at > 0.0 { entry.at } else { now_ms() };
        push_line("console", &area, &text, at);
    }
}

/// Log lines in the feed, oldest first.
pub(crate) fn log_lines() -> Vec<DebugLogLine> {
    feed().logs.to_vec()
}

/// IPC records in the feed, oldest first.
pub(crate) fn ipc_records() -> Vec<DebugIpcRecord> {
    feed().ipc.to_vec()
}

/// The id the next record or line will get.
pub(crate) fn next_id() -> u64 {
    feed().next_id
}

fn push_ipc(batch: Vec<DebugIpcRecord>) {
    let mut feed = feed();
    for mut record in batch {
        record.id = feed.next_id;
        feed.next_id += 1;
        feed.ipc.push(record.clone());
        feed.unsent_ipc.push(record);
    }
}

pub(crate) fn clear(app: &AppHandle, target: &str) -> Result<(), String> {
    {
        let mut feed = feed();
        match target {
            "ipc" => {
                feed.ipc.clear();
                feed.unsent_ipc.clear();
            }
            "logs" => {
                feed.logs.clear();
                feed.unsent_logs.clear();
            }
            _ => return Err(format!("Unknown debug feed: {target}")),
        }
    }
    let message = DebugFeed::Cleared {
        target: target.to_string(),
    };
    for label in hud::labels(app) {
        let _ = app.emit_to(label.as_str(), FEED_EVENT, &message);
    }
    Ok(())
}

/// Browser windows whose popup stack stays open only because focus went to the HUD or to
/// DevTools. A click anywhere else closes it.
static HELD_POPUPS: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn held_popups() -> MutexGuard<'static, Vec<String>> {
    HELD_POPUPS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Focus moving to the HUD or to a WebView2 DevTools window does not dismiss a popup, so
/// the popup can be inspected. Only while the Debug HUD extension is on.
pub fn keeps_popups_open(app: &AppHandle, hwnd: isize) -> bool {
    hud_enabled() && (hud::is_hud_hwnd(app, hwnd) || is_webview_devtools(hwnd))
}

pub fn hold_popups(window: &str) {
    let mut held = held_popups();
    if !held.iter().any(|item| item == window) {
        held.push(window.to_string());
    }
}

/// A mouse button went down at screen px. Returns the browser windows whose held popups that
/// click dismisses: every one, unless it landed in the HUD or in a DevTools window.
pub fn release_held_popups(app: &AppHandle, x: i32, y: i32) -> Vec<String> {
    if held_popups().is_empty() {
        return Vec::new();
    }
    if hud::contains_point(app, x, y) || is_webview_devtools(root_at(x, y)) {
        return Vec::new();
    }
    std::mem::take(&mut *held_popups())
}

fn root_at(x: i32, y: i32) -> isize {
    let hwnd = unsafe { WindowFromPoint(POINT { x, y }) };
    if hwnd.0.is_null() {
        return 0;
    }
    unsafe { GetAncestor(hwnd, GA_ROOT) }.0 as isize
}

/// A top-level window titled `DevTools…` that belongs to a WebView2 browser process.
fn is_webview_devtools(hwnd: isize) -> bool {
    if hwnd == 0 {
        return false;
    }
    let hwnd = HWND(hwnd as *mut std::ffi::c_void);
    let mut title = [0u16; 16];
    let length = unsafe { GetWindowTextW(hwnd, &mut title) }.max(0) as usize;
    if !String::from_utf16_lossy(&title[..length]).starts_with("DevTools") {
        return false;
    }
    let mut process_id = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)) };
    if process_id == 0 {
        return false;
    }
    let Ok(process) =
        (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) })
    else {
        return false;
    };
    let mut path = [0u16; 1024];
    let mut size = path.len() as u32;
    let named = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(path.as_mut_ptr()),
            &mut size,
        )
    }
    .is_ok();
    let _ = unsafe { CloseHandle(process) };
    named
        && String::from_utf16_lossy(&path[..size as usize])
            .to_ascii_lowercase()
            .ends_with("msedgewebview2.exe")
}

/// Shows or hides the HUD of browser window `window`. Resolves to whether it is now shown.
pub fn toggle_hud(app: &AppHandle, window: &str) -> Result<bool, String> {
    require()?;
    hud::toggle(app, window)
}

/// Window events for the HUD and the outline. Browser windows go through `lib.rs`.
pub fn window_event(window: &Window, event: &WindowEvent) {
    hud::window_event(window, event);
}

fn debug_paths_of(app: &AppHandle) -> DebugPaths {
    let data_dir = paths::data_dir(app);
    let mut files: Vec<DebugStoreFile> = std::fs::read_dir(&data_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            if !meta.is_file() {
                return None;
            }
            let modified = meta
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|elapsed| elapsed.as_secs_f64() * 1000.0)
                .unwrap_or(0.0);
            Some(DebugStoreFile {
                name: entry.file_name().to_string_lossy().into_owned(),
                path: entry.path().display().to_string(),
                bytes: meta.len(),
                modified,
            })
        })
        .collect();
    files.sort_by(|left, right| left.name.cmp(&right.name));
    DebugPaths {
        profile: profiles::active_id(),
        root_data_dir: paths::root_data_dir(app).display().to_string(),
        data_root: paths::data_root(app).display().to_string(),
        data_dir: data_dir.display().to_string(),
        profile_dir: paths::profile_dir(app).display().to_string(),
        files,
    }
}

/// Only the folders and files `debug_paths` reports can be shown in Explorer.
fn known_path(app: &AppHandle, path: &str) -> Option<PathBuf> {
    let known = debug_paths_of(app);
    let allowed = [
        &known.root_data_dir,
        &known.data_root,
        &known.data_dir,
        &known.profile_dir,
    ]
    .into_iter()
    .any(|item| item == path)
        || known.files.iter().any(|file| file.path == path);
    allowed.then(|| PathBuf::from(path))
}

// Commands

#[tauri::command]
pub async fn debug_toggle_hud(app: AppHandle, window: Window) -> Result<bool, String> {
    toggle_hud(&app, &owner_of(window.label()))
}

#[tauri::command]
pub async fn debug_snapshot() -> Result<DebugSnapshot, String> {
    require()?;
    let feed = feed();
    Ok(DebugSnapshot {
        ipc: feed.ipc.to_vec(),
        logs: feed.logs.to_vec(),
    })
}

#[tauri::command]
pub async fn debug_record_ipc(batch: Vec<DebugIpcRecord>) -> Result<(), String> {
    require()?;
    push_ipc(batch);
    Ok(())
}

#[tauri::command]
pub async fn debug_record_console(
    webview: tauri::Webview,
    batch: Vec<DebugConsoleEntry>,
) -> Result<(), String> {
    require()?;
    push_console(webview.label(), batch);
    Ok(())
}

#[tauri::command]
pub async fn debug_clear(app: AppHandle, target: String) -> Result<(), String> {
    require()?;
    clear(&app, &target)
}

#[tauri::command]
pub async fn debug_webviews(app: AppHandle) -> Result<Vec<DebugWebview>, String> {
    require()?;
    let mut rows = webviews::list(&app);
    outline::sync(&app, &rows);
    let outlined = outline::outlined_label();
    for row in &mut rows {
        row.outlined = outlined.as_deref() == Some(row.label.as_str());
    }
    Ok(rows)
}

/// Outlines `label`, or clears the outline when it is already on `label`. Resolves to whether
/// `label` is outlined now.
#[tauri::command]
pub async fn debug_outline(app: AppHandle, label: String) -> Result<bool, String> {
    require()?;
    outline::toggle(&app, &label)
}

#[tauri::command]
pub async fn debug_clear_outline(app: AppHandle) -> Result<(), String> {
    require()?;
    outline::clear(&app);
    Ok(())
}

fn open_devtools(app: &AppHandle, label: &str) -> Result<(), String> {
    if let Some(id) = webviews::tab_id(label) {
        tabs::open_devtools(app, id);
        return Ok(());
    }
    let webview = app
        .get_webview(label)
        .ok_or_else(|| format!("No webview is labelled {label}."))?;
    webview.open_devtools();
    Ok(())
}

fn reload_webview(app: &AppHandle, label: &str) -> Result<(), String> {
    if let Some(id) = webviews::tab_id(label) {
        return tabs::reload(app, id);
    }
    let webview = app
        .get_webview(label)
        .ok_or_else(|| format!("No webview is labelled {label}."))?;
    webview.reload().map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn debug_open_devtools(app: AppHandle, label: String) -> Result<(), String> {
    require()?;
    open_devtools(&app, &label)
}

#[tauri::command]
pub async fn debug_reload_webview(app: AppHandle, label: String) -> Result<(), String> {
    require()?;
    reload_webview(&app, &label)
}

#[tauri::command]
pub async fn debug_paths(app: AppHandle) -> Result<DebugPaths, String> {
    require()?;
    Ok(debug_paths_of(&app))
}

#[tauri::command]
pub async fn debug_reveal_path(app: AppHandle, path: String) -> Result<(), String> {
    require()?;
    let path = known_path(&app, &path).ok_or("That path is not one the debug HUD lists.")?;
    files::reveal_path(&path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ring_drops_the_oldest_items_at_capacity() {
        let mut ring = Ring::new(3);
        for item in 0..5 {
            ring.push(item);
        }
        assert_eq!(ring.to_vec(), vec![2, 3, 4]);
        assert_eq!(ring.take(), vec![2, 3, 4]);
        assert!(ring.is_empty());
    }

    #[test]
    fn a_zero_ring_holds_nothing() {
        let mut ring = Ring::new(0);
        ring.push(1);
        assert!(ring.is_empty());
    }

    #[test]
    fn debug_windows_belong_to_their_browser_window() {
        assert_eq!(owner_of("debug-main"), "main");
        assert_eq!(owner_of("debug-main-3"), "main-3");
        assert_eq!(owner_of("debug-outline-main-2"), "main-2");
        assert_eq!(owner_of("popup-main-0"), "main");
        assert_eq!(owner_of("main"), "main");
        assert!(is_debug_label("debug-outline-main"));
        assert!(!is_debug_label("main"));
    }

    #[test]
    fn the_feed_is_tagged_by_kind() {
        let json = serde_json::to_value(DebugFeed::Cleared {
            target: "ipc".into(),
        })
        .unwrap();
        assert_eq!(json["kind"], "cleared");
        assert_eq!(json["target"], "ipc");
        let json = serde_json::to_value(DebugFeed::Log { lines: vec![] }).unwrap();
        assert_eq!(json["kind"], "log");
    }

    #[test]
    fn console_lines_carry_the_webview_and_level_as_their_area() {
        let entry = DebugConsoleEntry {
            level: "warn".into(),
            text: "Key prop missing\n".into(),
            at: 1.0,
            source: "TabStrip.tsx:12:4".into(),
        };
        let (area, text) = console_line("ui", &entry);
        assert_eq!(area, "ui warn");
        assert_eq!(text, "ui warn: Key prop missing (TabStrip.tsx:12:4)");
        let (area, text) = console_line("popup-main-0", &DebugConsoleEntry::default());
        assert_eq!(area, "popup-main-0 error");
        assert_eq!(text, "popup-main-0 error: ");
    }

    #[test]
    fn a_console_entry_with_missing_fields_deserializes() {
        let entry: DebugConsoleEntry = serde_json::from_str(r#"{"text":"boom"}"#).unwrap();
        assert_eq!(entry.text, "boom");
        assert_eq!(entry.level, "");
    }

    #[test]
    fn a_record_without_an_id_deserializes() {
        let record: DebugIpcRecord = serde_json::from_str(
            r#"{"webview":"ui","kind":"invoke","name":"list_tabs","ok":true}"#,
        )
        .unwrap();
        assert_eq!(record.id, 0);
        assert_eq!(record.name, "list_tabs");
        assert_eq!(record.duration_ms, None);
    }
}
