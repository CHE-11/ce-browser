use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use webview2_com::{BytesReceivedChangedEventHandler, StateChangedEventHandler};
use windows::core::HSTRING;

use crate::webview2::{MainThread, read_bool, read_pwstr};
use crate::{files, paths, settings};

const FILE: &str = "downloads.json";
const MAX_HISTORY: usize = 500;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

pub const IN_PROGRESS: &str = "inProgress";
pub const PAUSED: &str = "paused";
pub const COMPLETED: &str = "completed";
pub const CANCELLED: &str = "cancelled";
pub const INTERRUPTED: &str = "interrupted";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DownloadItem {
    pub id: u32,
    pub url: String,
    pub file_name: String,
    pub path: String,
    pub mime_type: String,
    pub total_bytes: Option<u64>,
    pub received_bytes: u64,
    /// `inProgress`, `paused`, `completed`, `cancelled` or `interrupted`.
    pub state: String,
    pub started_at: u64,
    pub finished_at: Option<u64>,
    pub can_resume: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct DownloadsFile {
    items: Vec<DownloadItem>,
}

struct DownloadsState {
    items: Vec<DownloadItem>,
    next_id: u32,
    last_emit: HashMap<u32, Instant>,
}

pub struct Downloads(Mutex<DownloadsState>);

thread_local! {
    /// Live WebView2 operations by download id. Only the main thread touches these.
    static OPERATIONS: RefCell<HashMap<u32, ICoreWebView2DownloadOperation>> = RefCell::new(HashMap::new());
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

pub fn init(app: &AppHandle) {
    let file: DownloadsFile = files::read_json(&paths::data_file(app, FILE));
    let mut items = file.items;
    for item in &mut items {
        if item.state == IN_PROGRESS || item.state == PAUSED || item.state.is_empty() {
            item.state = INTERRUPTED.into();
            item.can_resume = false;
        }
    }
    let next_id = items.iter().map(|item| item.id).max().unwrap_or(0) + 1;
    app.manage(Downloads(Mutex::new(DownloadsState {
        items,
        next_id,
        last_emit: HashMap::new(),
    })));
}

pub fn reload(app: &AppHandle) {
    let file: DownloadsFile = files::read_json(&paths::data_file(app, FILE));
    let mut items = file.items;
    for item in &mut items {
        if item.state == IN_PROGRESS || item.state == PAUSED || item.state.is_empty() {
            item.state = INTERRUPTED.into();
            item.can_resume = false;
        }
    }
    let next_id = items.iter().map(|item| item.id).max().unwrap_or(0) + 1;
    with_state(app, |state| {
        state.items = items.clone();
        state.next_id = next_id;
    });
    let _ = app.emit("downloads-changed", &items);
}

fn with_state<T>(app: &AppHandle, change: impl FnOnce(&mut DownloadsState) -> T) -> T {
    let state = app.state::<Downloads>();
    let mut guard = state.inner().0.lock().expect("downloads lock");
    change(&mut guard)
}

pub fn list(app: &AppHandle) -> Vec<DownloadItem> {
    with_state(app, |state| state.items.clone())
}

fn changed(app: &AppHandle, persist: bool) {
    let items = list(app);
    if persist {
        let saved: Vec<DownloadItem> = items.iter().take(MAX_HISTORY).cloned().collect();
        let _ = files::write_json(
            &paths::data_file(app, FILE),
            &DownloadsFile { items: saved },
        );
    }
    let _ = app.emit("downloads-changed", &items);
}

/// Runs on the main thread for every download a page view starts. The default WebView2
/// flyout is suppressed and the file goes to the configured folder under a unique name.
pub fn on_download_starting(
    app: &AppHandle,
    args: ICoreWebView2DownloadStartingEventArgs,
) -> windows::core::Result<()> {
    unsafe {
        args.SetHandled(true)?;
        let operation = args.DownloadOperation()?;
        let suggested = read_pwstr(|value| args.ResultFilePath(value));
        let name = Path::new(&suggested)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "download".to_string());
        let settings = settings::get(app);
        let dir = settings.download_dir(app);

        if settings.ask_where_to_save {
            let deferral = args.GetDeferral()?;
            let held = MainThread::new((args.clone(), deferral, operation));
            let app = app.clone();
            std::thread::spawn(move || {
                let chosen = app
                    .dialog()
                    .file()
                    .set_directory(&dir)
                    .set_file_name(&name)
                    .blocking_save_file()
                    .and_then(|path| path.into_path().ok());
                let main_app = app.clone();
                let _ = app.run_on_main_thread(move || {
                    let (args, deferral, operation) = held.into_inner();
                    match chosen {
                        Some(path) => {
                            let _ = args
                                .SetResultFilePath(&HSTRING::from(path.to_string_lossy().as_ref()));
                            let _ = track(&main_app, &operation, path);
                        }
                        None => {
                            let _ = args.SetCancel(true);
                        }
                    }
                    let _ = deferral.Complete();
                });
            });
            return Ok(());
        }

        let _ = fs::create_dir_all(&dir);
        let path = files::unique_path(&dir, &name);
        args.SetResultFilePath(&HSTRING::from(path.to_string_lossy().as_ref()))?;
        track(app, &operation, path)
    }
}

fn track(
    app: &AppHandle,
    operation: &ICoreWebView2DownloadOperation,
    path: PathBuf,
) -> windows::core::Result<()> {
    unsafe {
        let url = read_pwstr(|value| operation.Uri(value));
        let mime_type = read_pwstr(|value| operation.MimeType(value));
        let mut total = 0i64;
        let _ = operation.TotalBytesToReceive(&mut total);
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let id = with_state(app, |state| {
            let id = state.next_id;
            state.next_id += 1;
            state.items.insert(
                0,
                DownloadItem {
                    id,
                    url,
                    file_name,
                    path: path.to_string_lossy().into_owned(),
                    mime_type,
                    total_bytes: (total > 0).then_some(total as u64),
                    received_bytes: 0,
                    state: IN_PROGRESS.into(),
                    started_at: now_ms(),
                    finished_at: None,
                    can_resume: false,
                },
            );
            id
        });
        OPERATIONS.with(|operations| operations.borrow_mut().insert(id, operation.clone()));

        let mut token = 0i64;
        let progress_app = app.clone();
        operation.add_BytesReceivedChanged(
            &BytesReceivedChangedEventHandler::create(Box::new(move |sender, _| {
                if let Some(operation) = sender {
                    let mut received = 0i64;
                    let mut total = 0i64;
                    operation.BytesReceived(&mut received)?;
                    operation.TotalBytesToReceive(&mut total)?;
                    progress(&progress_app, id, received, total);
                }
                Ok(())
            })),
            &mut token,
        )?;
        let state_app = app.clone();
        operation.add_StateChanged(
            &StateChangedEventHandler::create(Box::new(move |sender, _| {
                if let Some(operation) = sender {
                    state_changed(&state_app, id, &operation);
                }
                Ok(())
            })),
            &mut token,
        )?;
    }
    changed(app, true);
    let _ = app.emit("download-started", ());
    Ok(())
}

fn progress(app: &AppHandle, id: u32, received: i64, total: i64) {
    let due = with_state(app, |state| {
        if let Some(item) = state.items.iter_mut().find(|item| item.id == id) {
            item.received_bytes = received.max(0) as u64;
            if total > 0 {
                item.total_bytes = Some(total as u64);
            }
        }
        let now = Instant::now();
        let due = state
            .last_emit
            .get(&id)
            .is_none_or(|last| now.duration_since(*last) >= PROGRESS_INTERVAL);
        if due {
            state.last_emit.insert(id, now);
        }
        due
    });
    if due {
        changed(app, false);
    }
}

fn state_changed(app: &AppHandle, id: u32, operation: &ICoreWebView2DownloadOperation) {
    let (state, reason, can_resume, received, path) = unsafe {
        let mut state = COREWEBVIEW2_DOWNLOAD_STATE::default();
        let _ = operation.State(&mut state);
        let mut reason = COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON::default();
        let _ = operation.InterruptReason(&mut reason);
        let can_resume = read_bool(|value| operation.CanResume(value));
        let mut received = 0i64;
        let _ = operation.BytesReceived(&mut received);
        let path = read_pwstr(|value| operation.ResultFilePath(value));
        (state, reason, can_resume, received, path)
    };
    let next = if state == COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED {
        COMPLETED
    } else if state == COREWEBVIEW2_DOWNLOAD_STATE_INTERRUPTED {
        if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_CANCELED {
            CANCELLED
        } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_PAUSED {
            PAUSED
        } else {
            INTERRUPTED
        }
    } else {
        IN_PROGRESS
    };
    let finished = next == COMPLETED || next == CANCELLED || (next == INTERRUPTED && !can_resume);
    with_state(app, |state| {
        if let Some(item) = state.items.iter_mut().find(|item| item.id == id) {
            item.state = next.into();
            item.can_resume = can_resume;
            item.received_bytes = received.max(0) as u64;
            if next == COMPLETED && item.total_bytes.is_none() {
                item.total_bytes = Some(item.received_bytes);
            }
            if !path.is_empty() {
                item.path = path.clone();
                if let Some(name) = Path::new(&path).file_name() {
                    item.file_name = name.to_string_lossy().into_owned();
                }
            }
            if finished {
                item.finished_at = Some(now_ms());
            }
        }
        state.last_emit.remove(&id);
    });
    if finished {
        OPERATIONS.with(|operations| operations.borrow_mut().remove(&id));
    }
    changed(app, true);
}

fn run_operation(
    app: &AppHandle,
    id: u32,
    work: fn(&ICoreWebView2DownloadOperation) -> windows::core::Result<()>,
) -> Result<(), String> {
    let (tx, rx) = mpsc::channel();
    app.run_on_main_thread(move || {
        let operation = OPERATIONS.with(|operations| operations.borrow().get(&id).cloned());
        let result = match operation {
            Some(operation) => work(&operation).map_err(|error| error.message()),
            None => Err("This download is no longer active.".to_string()),
        };
        let _ = tx.send(result);
    })
    .map_err(|error| error.to_string())?;
    rx.recv_timeout(Duration::from_secs(5))
        .map_err(|_| "WebView2 did not answer.".to_string())?
}

fn set_state(app: &AppHandle, id: u32, next: &str) {
    with_state(app, |state| {
        if let Some(item) = state.items.iter_mut().find(|item| item.id == id) {
            item.state = next.into();
        }
    });
    changed(app, true);
}

pub fn pause(app: &AppHandle, id: u32) -> Result<(), String> {
    run_operation(app, id, |operation| unsafe { operation.Pause() })?;
    set_state(app, id, PAUSED);
    Ok(())
}

pub fn resume(app: &AppHandle, id: u32) -> Result<(), String> {
    run_operation(app, id, |operation| unsafe { operation.Resume() })?;
    set_state(app, id, IN_PROGRESS);
    Ok(())
}

pub fn cancel(app: &AppHandle, id: u32) -> Result<(), String> {
    run_operation(app, id, |operation| unsafe { operation.Cancel() })
}

fn item(app: &AppHandle, id: u32) -> Result<DownloadItem, String> {
    with_state(app, |state| {
        state.items.iter().find(|item| item.id == id).cloned()
    })
    .ok_or_else(|| "That download is not in the list.".to_string())
}

pub fn open(app: &AppHandle, id: u32) -> Result<(), String> {
    let item = item(app, id)?;
    if item.state != COMPLETED {
        return Err("The download has not finished.".into());
    }
    files::open_with_default(Path::new(&item.path))
}

pub fn show_in_folder(app: &AppHandle, id: u32) -> Result<(), String> {
    let item = item(app, id)?;
    files::reveal_path(Path::new(&item.path))
}

pub fn remove(app: &AppHandle, id: u32) -> Result<(), String> {
    let item = item(app, id)?;
    if item.state == IN_PROGRESS || item.state == PAUSED {
        let _ = cancel(app, id);
    }
    with_state(app, |state| state.items.retain(|item| item.id != id));
    changed(app, true);
    Ok(())
}

pub fn clear(app: &AppHandle) {
    with_state(app, |state| {
        state
            .items
            .retain(|item| item.state == IN_PROGRESS || item.state == PAUSED)
    });
    changed(app, true);
}
