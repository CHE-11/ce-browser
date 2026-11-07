use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Window};
use tauri_plugin_dialog::DialogExt;
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use webview2_com::{BytesReceivedChangedEventHandler, StateChangedEventHandler};
use windows::core::HSTRING;

use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2;

use crate::accelerator::{self, ByteRange, CHUNK_BYTES, CONNECTIONS, MIN_BYTES};
use crate::webview2::{self, MainThread, read_bool, read_pwstr};
use crate::{files, paths, popup, settings, system, tabs};

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
    /// This file was saved with range requests instead of WebView2.
    #[serde(default)]
    pub accelerated: bool,
    /// Inclusive byte ranges. Empty for a normal WebView2 download.
    #[serde(default)]
    pub ranges: Vec<ByteRange>,
    /// Windows name of the computer that saved this file.
    #[serde(default)]
    pub computer_name: String,
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
    let items = read_items(app);
    let next_id = items.iter().map(|item| item.id).max().unwrap_or(0) + 1;
    app.manage(Downloads(Mutex::new(DownloadsState {
        items,
        next_id,
        last_emit: HashMap::new(),
    })));
}

pub fn reload(app: &AppHandle) {
    let items = read_items(app);
    let next_id = items.iter().map(|item| item.id).max().unwrap_or(0) + 1;
    with_state(app, |state| {
        state.items = items.clone();
        state.next_id = next_id;
    });
    let _ = app.emit("downloads-changed", &items);
}

fn read_items(app: &AppHandle) -> Vec<DownloadItem> {
    let file: DownloadsFile = files::read_json(&paths::data_file(app, FILE));
    let mut items = file.items;
    for item in &mut items {
        settle_unfinished(item);
    }
    if adopt_local(&mut items) {
        let saved: Vec<DownloadItem> = items.iter().take(MAX_HISTORY).cloned().collect();
        let _ = files::write_json(
            &paths::data_file(app, FILE),
            &DownloadsFile { items: saved },
        );
    }
    items
}

/// Name Windows shows for this PC. Downloads from another computer stay in the list and do nothing here.
pub fn computer_name() -> String {
    std::env::var("COMPUTERNAME")
        .ok()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "This computer".to_string())
}

fn on_this_computer(item: &DownloadItem) -> bool {
    let current = computer_name();
    !item.computer_name.is_empty() && item.computer_name.eq_ignore_ascii_case(&current)
}

fn not_here(item: &DownloadItem) -> String {
    if item.computer_name.is_empty() {
        "That file was saved on another computer.".into()
    } else {
        format!("That file was saved on {}.", item.computer_name)
    }
}

fn require_here(app: &AppHandle, id: u32) -> Result<DownloadItem, String> {
    let item = item(app, id)?;
    if on_this_computer(&item) {
        Ok(item)
    } else {
        Err(not_here(&item))
    }
}

/// Older lists have no computer name. A file that is still on this disk belongs to this computer.
fn adopt_local(items: &mut [DownloadItem]) -> bool {
    let current = computer_name();
    let mut changed = false;
    for item in items.iter_mut() {
        if !item.computer_name.is_empty() || !Path::new(&item.path).is_file() {
            continue;
        }
        item.computer_name = current.clone();
        changed = true;
    }
    changed
}

fn with_state<T>(app: &AppHandle, change: impl FnOnce(&mut DownloadsState) -> T) -> T {
    let state = app.state::<Downloads>();
    let mut guard = state.inner().0.lock().expect("downloads lock");
    change(&mut guard)
}

pub fn list(app: &AppHandle) -> Vec<DownloadItem> {
    with_state(app, |state| state.items.clone())
}

/// A file this browser wrote itself, such as a saved PDF. It joins the profile's download list.
pub fn record_completed(app: &AppHandle, url: &str, path: &Path, mime_type: &str) {
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "download".to_string());
    let bytes = fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
    let now = now_ms();
    with_state(app, |state| {
        let id = state.next_id;
        state.next_id += 1;
        state.items.insert(
            0,
            DownloadItem {
                id,
                url: url.to_string(),
                file_name,
                path: path.to_string_lossy().into_owned(),
                mime_type: mime_type.to_string(),
                total_bytes: Some(bytes),
                received_bytes: bytes,
                state: COMPLETED.into(),
                started_at: now,
                finished_at: Some(now),
                can_resume: false,
                accelerated: false,
                ranges: Vec::new(),
                computer_name: computer_name(),
            },
        );
    });
    changed(app, true);
    let _ = app.emit("download-started", ());
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

fn settle_unfinished(item: &mut DownloadItem) {
    if item.state != IN_PROGRESS && item.state != PAUSED && !item.state.is_empty() {
        return;
    }
    let total = item.total_bytes.unwrap_or(0);
    let resumable =
        item.accelerated && total > item.received_bytes && Path::new(&item.path).is_file();
    item.state = INTERRUPTED.into();
    item.can_resume = resumable;
}

/// Runs on the main thread for every download a page view starts. The default WebView2
/// flyout is suppressed and the file goes to the configured folder under a unique name.
pub fn on_download_starting(
    app: &AppHandle,
    core: &ICoreWebView2,
    args: ICoreWebView2DownloadStartingEventArgs,
) -> windows::core::Result<()> {
    unsafe {
        args.SetHandled(true)?;
        let operation = args.DownloadOperation()?;
        let url = read_pwstr(|value| operation.Uri(value));
        let suggested = read_pwstr(|value| args.ResultFilePath(value));
        let raw_name = Path::new(&suggested)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "download".to_string());
        let settings = settings::get(app);
        let name = accelerator::download_name(
            &settings.download_name_template,
            &url,
            &raw_name,
            &accelerator::local_date(),
        );
        let dir = settings.download_dir(app);

        if settings.ask_where_to_save {
            let deferral = args.GetDeferral()?;
            let held = MainThread::new((args.clone(), deferral, operation, core.clone()));
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
                    let (args, deferral, operation, core) = held.into_inner();
                    match chosen {
                        Some(path) => place(&main_app, &core, args, deferral, &operation, path),
                        None => {
                            let _ = args.SetCancel(true);
                            let _ = deferral.Complete();
                        }
                    }
                });
            });
            return Ok(());
        }

        let _ = fs::create_dir_all(&dir);
        let path = files::unique_path(&dir, &name);
        if !worth_probing(&settings, &operation) {
            args.SetResultFilePath(&HSTRING::from(path.to_string_lossy().as_ref()))?;
            return track(app, &operation, path);
        }
        let deferral = args.GetDeferral()?;
        place(app, core, args, deferral, &operation, path);
        Ok(())
    }
}

fn worth_probing(
    settings: &settings::BrowserSettings,
    operation: &ICoreWebView2DownloadOperation,
) -> bool {
    if !settings.accelerate_downloads {
        return false;
    }
    let mut total = 0i64;
    let _ = unsafe { operation.TotalBytesToReceive(&mut total) };
    total <= 0 || (total as u64) >= MIN_BYTES
}

fn place(
    app: &AppHandle,
    core: &ICoreWebView2,
    args: ICoreWebView2DownloadStartingEventArgs,
    deferral: ICoreWebView2Deferral,
    operation: &ICoreWebView2DownloadOperation,
    path: PathBuf,
) {
    if !worth_probing(&settings::get(app), operation) {
        let _ = unsafe { args.SetResultFilePath(&HSTRING::from(path.to_string_lossy().as_ref())) };
        let _ = track(app, operation, path);
        let _ = unsafe { deferral.Complete() };
        return;
    }
    let url = unsafe { read_pwstr(|value| operation.Uri(value)) };
    let agent = webview2::user_agent(core);
    let held = MainThread::new((args, deferral, operation.clone()));
    let app = app.clone();
    let probe_url = url.clone();
    webview2::request_cookies(core, &url, move |cookie| {
        let app = app.clone();
        std::thread::spawn(move || {
            let probed = accelerator::probe(&probe_url, &cookie, &agent);
            let main_app = app.clone();
            let _ = app.run_on_main_thread(move || {
                let (args, deferral, operation) = held.into_inner();
                match probed {
                    Ok(total) => {
                        let _ = unsafe { args.SetCancel(true) };
                        let _ = unsafe { deferral.Complete() };
                        start_accelerated(&main_app, &probe_url, path, total, cookie, agent);
                    }
                    Err(_) => {
                        let _ = unsafe {
                            args.SetResultFilePath(&HSTRING::from(path.to_string_lossy().as_ref()))
                        };
                        let _ = track(&main_app, &operation, path);
                        let _ = unsafe { deferral.Complete() };
                    }
                }
            });
        });
    });
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
                    accelerated: false,
                    ranges: Vec::new(),
                    computer_name: computer_name(),
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
    if next == COMPLETED {
        let name = Path::new(&path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or(path.clone());
        crate::system::toast_download(&name, &path);
        crate::automation::scheduler::on_download(app, &name, &path);
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
    require_here(app, id)?;
    if accelerated(app, id) {
        if let Some(job) = jobs().get(&id) {
            job.pause.store(true, Ordering::Relaxed);
        }
        set_state(app, id, PAUSED);
        return Ok(());
    }
    run_operation(app, id, |operation| unsafe { operation.Pause() })?;
    set_state(app, id, PAUSED);
    Ok(())
}

pub fn resume(app: &AppHandle, id: u32) -> Result<(), String> {
    require_here(app, id)?;
    if accelerated(app, id) {
        let item = item(app, id)?;
        let needs_cookie = jobs()
            .get(&id)
            .is_none_or(|job| job.cookie.lock().expect("cookie").is_empty());
        let session = if needs_cookie {
            Some(refresh_session(app, &item.url))
        } else {
            None
        };
        {
            let mut map = jobs();
            let job = map.entry(id).or_insert_with(AccelJob::new);
            if let Some((cookie, agent)) = session {
                *job.cookie.lock().expect("cookie") = cookie;
                if !agent.is_empty() {
                    *job.agent.lock().expect("agent") = agent;
                }
            }
            job.pause.store(false, Ordering::Relaxed);
            job.cancel.store(false, Ordering::Relaxed);
        }
        set_state(app, id, IN_PROGRESS);
        launch(app, id);
        return Ok(());
    }
    run_operation(app, id, |operation| unsafe { operation.Resume() })?;
    set_state(app, id, IN_PROGRESS);
    Ok(())
}

pub fn cancel(app: &AppHandle, id: u32) -> Result<(), String> {
    require_here(app, id)?;
    if accelerated(app, id) {
        let running = jobs().get(&id).is_some_and(|job| {
            job.cancel.store(true, Ordering::Relaxed);
            job.pause.store(false, Ordering::Relaxed);
            job.running.load(Ordering::Relaxed)
        });
        if !running {
            finish_cancel(app, id);
        }
        return Ok(());
    }
    run_operation(app, id, |operation| unsafe { operation.Cancel() })
}

fn item(app: &AppHandle, id: u32) -> Result<DownloadItem, String> {
    with_state(app, |state| {
        state.items.iter().find(|item| item.id == id).cloned()
    })
    .ok_or_else(|| "That download is not in the list.".to_string())
}

pub fn open(app: &AppHandle, id: u32, window: &str) -> Result<(), String> {
    let item = require_here(app, id)?;
    if item.state != COMPLETED {
        return Err("The download has not finished.".into());
    }
    let owner = popup::owner_of(window);
    if system::has_handler(app, &item.path) {
        return system::open_file_in_window(app, &item.path, &owner);
    }
    files::open_with_default(Path::new(&item.path))
}

pub fn show_in_folder(app: &AppHandle, id: u32) -> Result<(), String> {
    let item = require_here(app, id)?;
    files::reveal_path(Path::new(&item.path))
}

static FILE_DRAG: AtomicBool = AtomicBool::new(false);
static CLOSE_POPUP_AFTER_DRAG: AtomicBool = AtomicBool::new(false);

/// True while `DoDragDrop` is running on the UI thread.
pub fn dragging_file() -> bool {
    FILE_DRAG.load(Ordering::Acquire)
}

/// The downloads popup lost focus because a file drag started. Close it once the drag ends.
pub fn close_popup_when_drag_ends() {
    CLOSE_POPUP_AFTER_DRAG.store(true, Ordering::Release);
}

struct FileDragFlag;

impl FileDragFlag {
    fn enter() -> Self {
        CLOSE_POPUP_AFTER_DRAG.store(false, Ordering::Release);
        FILE_DRAG.store(true, Ordering::Release);
        Self
    }
}

impl Drop for FileDragFlag {
    fn drop(&mut self) {
        FILE_DRAG.store(false, Ordering::Release);
    }
}

/// Drags a finished download as a real file so page drop zones and other programs can take it.
pub fn drag_file(app: &AppHandle, window: &Window, id: u32) -> Result<(), String> {
    let found = require_here(app, id)?;
    if found.state != COMPLETED {
        return Err("The download has not finished.".into());
    }
    let path = shell_path(Path::new(&found.path));
    if !path.is_file() {
        return Err("That file is no longer on disk.".into());
    }
    let _flag = FileDragFlag::enter();
    // The webview still owns the mouse. Release it so the drag can leave the window.
    unsafe {
        let _ = windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture();
    }
    // The downloads panel sits above the page. Move it aside once the drag is watching the
    // mouse, so a drop lands on the page underneath. Close it when the drag ends. Destroying
    // it here would run under DoDragDrop.
    let popup_drag = popup::is_popup_label(window.label());
    if popup_drag {
        CLOSE_POPUP_AFTER_DRAG.store(true, Ordering::Release);
    }
    let hwnd = popup_drag.then(|| window.hwnd().ok()).flatten();
    let result = shell_drag(&path, move || {
        let Some(hwnd) = hwnd else {
            return;
        };
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{
                SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos,
            };
            let _ = SetWindowPos(
                hwnd,
                None,
                -20_000,
                -20_000,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOSIZE,
            );
        }
    });
    let close = CLOSE_POPUP_AFTER_DRAG.swap(false, Ordering::AcqRel);
    drop(_flag);
    if close {
        let app = app.clone();
        let label = window.label().to_string();
        let _ = app.clone().run_on_main_thread(move || {
            crate::popup::close_all(&app, &label, true);
        });
    }
    result
}

/// `canonicalize` adds a `\\?\` prefix that page drop zones and other programs reject.
fn shell_path(path: &Path) -> PathBuf {
    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let text = canonical.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        return PathBuf::from(rest);
    }
    canonical
}

/// WebView2 does not report the mouse button to `DoDragDrop`, so a normal drop source ends
/// the drag at once. This one follows the physical button, and a low-level hook when that
/// state is missing, until the button actually comes up.
static DRAG_BUTTON_HELD: AtomicBool = AtomicBool::new(false);
static DRAG_BUTTON_UP: AtomicBool = AtomicBool::new(false);

#[windows::core::implement(windows::Win32::System::Ole::IDropSource)]
struct DownloadDropSource;

impl windows::Win32::System::Ole::IDropSource_Impl for DownloadDropSource_Impl {
    fn QueryContinueDrag(
        &self,
        escape: windows::core::BOOL,
        keys: windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS,
    ) -> windows::core::HRESULT {
        use windows::Win32::Foundation::{DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, S_OK};
        use windows::Win32::System::SystemServices::MK_LBUTTON;
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};

        if escape.as_bool() {
            return DRAGDROP_S_CANCEL;
        }
        let physical = unsafe { (GetAsyncKeyState(i32::from(VK_LBUTTON.0)) as u16) & 0x8000 != 0 };
        let message =
            (keys & MK_LBUTTON) != windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS(0);
        if physical || message {
            DRAG_BUTTON_HELD.store(true, Ordering::Release);
            return S_OK;
        }
        if DRAG_BUTTON_HELD.load(Ordering::Acquire) || DRAG_BUTTON_UP.load(Ordering::Acquire) {
            return DRAGDROP_S_DROP;
        }
        S_OK
    }

    fn GiveFeedback(
        &self,
        _effect: windows::Win32::System::Ole::DROPEFFECT,
    ) -> windows::core::HRESULT {
        windows::Win32::Foundation::DRAGDROP_S_USEDEFAULTCURSORS
    }
}

unsafe extern "system" fn download_drag_hook(
    code: i32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{CallNextHookEx, HC_ACTION, WM_LBUTTONUP};

    if code == HC_ACTION as i32 && wparam.0 as u32 == WM_LBUTTONUP {
        DRAG_BUTTON_UP.store(true, Ordering::Release);
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn shell_drag(path: &Path, prepare: impl FnOnce()) -> Result<(), String> {
    use windows::Win32::Foundation::DRAGDROP_S_DROP;
    use windows::Win32::System::Com::IDataObject;
    use windows::Win32::System::Ole::{DROPEFFECT_COPY, DoDragDrop, IDropSource, OleInitialize};
    use windows::Win32::UI::Shell::{
        BHID_DataObject, IShellItem, IShellItemArray, SHCreateItemFromParsingName,
        SHCreateShellItemArrayFromShellItem,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowsHookExW, UnhookWindowsHookEx, WH_MOUSE_LL,
    };
    use windows::core::HSTRING;

    let wide = HSTRING::from(path.as_os_str());
    let data: IDataObject = unsafe {
        let _ = OleInitialize(None);
        let item: IShellItem = SHCreateItemFromParsingName(&wide, None)
            .map_err(|_| "The file could not be dragged.".to_string())?;
        let items: IShellItemArray = SHCreateShellItemArrayFromShellItem(&item)
            .map_err(|_| "The file could not be dragged.".to_string())?;
        items
            .BindToHandler(None, &BHID_DataObject)
            .map_err(|_| "The file could not be dragged.".to_string())?
    };
    let source: IDropSource = DownloadDropSource.into();
    DRAG_BUTTON_HELD.store(false, Ordering::Release);
    DRAG_BUTTON_UP.store(false, Ordering::Release);
    let hook = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(download_drag_hook), None, 0) }.ok();
    prepare();
    let mut effect = windows::Win32::System::Ole::DROPEFFECT::default();
    let result = unsafe { DoDragDrop(&data, &source, DROPEFFECT_COPY, &mut effect) };
    if let Some(hook) = hook {
        unsafe {
            let _ = UnhookWindowsHookEx(hook);
        }
    }
    if result == DRAGDROP_S_DROP || result.is_ok() {
        Ok(())
    } else {
        Err("The file could not be dragged.".into())
    }
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

#[derive(Clone)]
struct AccelJob {
    pause: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
    cookie: Arc<Mutex<String>>,
    agent: Arc<Mutex<String>>,
}

impl AccelJob {
    fn new() -> Self {
        Self {
            pause: Arc::new(AtomicBool::new(false)),
            cancel: Arc::new(AtomicBool::new(false)),
            running: Arc::new(AtomicBool::new(false)),
            failed: Arc::new(AtomicBool::new(false)),
            cookie: Arc::new(Mutex::new(String::new())),
            agent: Arc::new(Mutex::new(String::new())),
        }
    }
}

static JOBS: LazyLock<Mutex<HashMap<u32, AccelJob>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn jobs() -> std::sync::MutexGuard<'static, HashMap<u32, AccelJob>> {
    JOBS.lock().expect("download jobs")
}

fn accelerated(app: &AppHandle, id: u32) -> bool {
    item(app, id).is_ok_and(|item| item.accelerated)
}

fn start_accelerated(
    app: &AppHandle,
    url: &str,
    path: PathBuf,
    total: u64,
    cookie: String,
    agent: String,
) {
    let ranges = accelerator::split_ranges(total, CONNECTIONS);
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
                url: url.to_string(),
                file_name,
                path: path.to_string_lossy().into_owned(),
                mime_type: String::new(),
                total_bytes: Some(total),
                received_bytes: 0,
                state: IN_PROGRESS.into(),
                started_at: now_ms(),
                finished_at: None,
                can_resume: true,
                accelerated: true,
                ranges,
                computer_name: computer_name(),
            },
        );
        id
    });
    {
        let mut map = jobs();
        let job = map.entry(id).or_insert_with(AccelJob::new);
        *job.cookie.lock().expect("cookie") = cookie;
        *job.agent.lock().expect("agent") = agent;
    }
    changed(app, true);
    let _ = app.emit("download-started", ());
    launch(app, id);
}

fn launch(app: &AppHandle, id: u32) {
    let Ok(item) = item(app, id) else {
        return;
    };
    let Some(job) = jobs().get(&id).cloned() else {
        return;
    };
    if job
        .running
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_err()
    {
        return;
    }
    job.failed.store(false, Ordering::Relaxed);
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .read(true)
        .open(&item.path);
    let Ok(file) = file else {
        job.running.store(false, Ordering::Relaxed);
        mark_interrupted(app, id, false);
        return;
    };
    if let Some(total) = item.total_bytes {
        let _ = file.set_len(total);
    }
    let indexes: Vec<usize> = item
        .ranges
        .iter()
        .enumerate()
        .filter(|(_, range)| !range.done())
        .map(|(index, _)| index)
        .collect();
    if indexes.is_empty() {
        job.running.store(false, Ordering::Relaxed);
        mark_completed(app, id);
        return;
    }
    let file = Arc::new(Mutex::new(file));
    let left = Arc::new(AtomicUsize::new(indexes.len()));
    for index in indexes {
        let app = app.clone();
        let job = job.clone();
        let file = file.clone();
        let left = left.clone();
        std::thread::spawn(move || {
            pump(&app, id, index, &file, &job);
            if left.fetch_sub(1, Ordering::AcqRel) == 1 {
                job.running.store(false, Ordering::Relaxed);
                settle_job(&app, id);
            }
        });
    }
}

fn pump(app: &AppHandle, id: u32, index: usize, file: &Mutex<std::fs::File>, job: &AccelJob) {
    loop {
        if job.cancel.load(Ordering::Relaxed) || job.pause.load(Ordering::Relaxed) {
            return;
        }
        let next = item(app, id).ok().and_then(|item| {
            let range = item.ranges.get(index)?;
            if range.done() {
                return None;
            }
            let start = range.start + range.received;
            let end = (start + CHUNK_BYTES - 1).min(range.end);
            Some((item.url, start, end))
        });
        let Some((url, start, end)) = next else {
            return;
        };
        let cookie = job.cookie.lock().expect("cookie").clone();
        let agent = job.agent.lock().expect("agent").clone();
        let bytes = accelerator::read_range(&url, &cookie, &agent, start, end)
            .or_else(|_| accelerator::read_range(&url, &cookie, &agent, start, end));
        let Ok(bytes) = bytes else {
            job.failed.store(true, Ordering::Relaxed);
            return;
        };
        if job.cancel.load(Ordering::Relaxed) {
            return;
        }
        let wrote = {
            let mut file = file.lock().expect("download file");
            file.seek(SeekFrom::Start(start))
                .and_then(|_| file.write_all(&bytes))
                .is_ok()
        };
        if !wrote {
            job.failed.store(true, Ordering::Relaxed);
            return;
        }
        note(app, id, index, bytes.len() as u64);
    }
}

fn note(app: &AppHandle, id: u32, index: usize, add: u64) {
    let due = with_state(app, |state| {
        if let Some(item) = state.items.iter_mut().find(|item| item.id == id) {
            if let Some(range) = item.ranges.get_mut(index) {
                range.received = range.received.saturating_add(add).min(range.length());
            }
            item.received_bytes = item.ranges.iter().map(|range| range.received).sum();
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
        changed(app, true);
    }
}

fn settle_job(app: &AppHandle, id: u32) {
    let Some(job) = jobs().get(&id).cloned() else {
        return;
    };
    if job.cancel.load(Ordering::Relaxed) {
        finish_cancel(app, id);
        return;
    }
    let Ok(item) = item(app, id) else {
        return;
    };
    if item.ranges.iter().all(ByteRange::done) {
        mark_completed(app, id);
        return;
    }
    if job.pause.load(Ordering::Relaxed) {
        return;
    }
    if job.failed.load(Ordering::Relaxed) {
        mark_interrupted(app, id, true);
        return;
    }
    launch(app, id);
}

fn mark_completed(app: &AppHandle, id: u32) {
    with_state(app, |state| {
        if let Some(item) = state.items.iter_mut().find(|item| item.id == id) {
            item.state = COMPLETED.into();
            item.can_resume = false;
            item.received_bytes = item.total_bytes.unwrap_or(item.received_bytes);
            item.finished_at = Some(now_ms());
        }
        state.last_emit.remove(&id);
    });
    jobs().remove(&id);
    if let Ok(item) = item(app, id) {
        crate::system::toast_download(&item.file_name, &item.path);
        crate::automation::scheduler::on_download(app, &item.file_name, &item.path);
    }
    changed(app, true);
}

fn mark_interrupted(app: &AppHandle, id: u32, can_resume: bool) {
    with_state(app, |state| {
        if let Some(item) = state.items.iter_mut().find(|item| item.id == id) {
            item.state = INTERRUPTED.into();
            item.can_resume = can_resume;
            item.finished_at = Some(now_ms());
        }
    });
    changed(app, true);
}

fn finish_cancel(app: &AppHandle, id: u32) {
    let path = item(app, id).ok().map(|item| item.path);
    if let Some(path) = path {
        let _ = fs::remove_file(path);
    }
    with_state(app, |state| {
        if let Some(item) = state.items.iter_mut().find(|item| item.id == id) {
            item.state = CANCELLED.into();
            item.can_resume = false;
            item.finished_at = Some(now_ms());
        }
        state.last_emit.remove(&id);
    });
    jobs().remove(&id);
    changed(app, true);
}

fn refresh_session(app: &AppHandle, url: &str) -> (String, String) {
    let host = url::Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(|host| host.to_string()));
    let snapshot = tabs::snapshot(app);
    let matched = snapshot.tabs.iter().find(|tab| {
        tab.loaded
            && host
                .as_ref()
                .is_some_and(|host| tab.url.contains(host.as_str()))
    });
    let webview = matched
        .and_then(|tab| tabs::page_view(app, tab.id))
        .or_else(|| app.get_webview("ui"));
    webview
        .map(|webview| webview2::session_for(&webview, url))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{DownloadItem, adopt_local, computer_name, on_this_computer};

    #[test]
    fn an_older_download_file_still_loads() {
        let item: DownloadItem = serde_json::from_str(
            r#"{"id":1,"url":"https://example.com/a.zip","fileName":"a.zip","path":"a.zip","state":"completed"}"#,
        )
        .expect("download");
        assert!(!item.accelerated);
        assert!(item.ranges.is_empty());
        assert_eq!(item.file_name, "a.zip");
        assert!(item.computer_name.is_empty());
    }

    #[test]
    fn a_computer_name_round_trips() {
        let item = DownloadItem {
            computer_name: "OFFICE".into(),
            file_name: "a.zip".into(),
            ..Default::default()
        };
        let json = serde_json::to_string(&item).expect("json");
        assert!(json.contains("\"computerName\":\"OFFICE\""));
        let back: DownloadItem = serde_json::from_str(&json).expect("download");
        assert_eq!(back.computer_name, "OFFICE");
    }

    #[test]
    fn a_file_on_this_disk_adopts_this_computer() {
        let dir = std::env::temp_dir().join(format!("ce-download-adopt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("note.txt");
        std::fs::write(&path, b"hi").expect("file");
        let mut local = DownloadItem {
            path: path.to_string_lossy().into_owned(),
            ..Default::default()
        };
        assert!(adopt_local(std::slice::from_mut(&mut local)));
        assert_eq!(local.computer_name, computer_name());
        assert!(on_this_computer(&local));
        local.computer_name = computer_name().to_ascii_lowercase();
        assert!(on_this_computer(&local));

        let mut foreign = DownloadItem {
            computer_name: format!("{}-other", computer_name()),
            path: path.to_string_lossy().into_owned(),
            ..Default::default()
        };
        assert!(!adopt_local(std::slice::from_mut(&mut foreign)));
        assert!(!on_this_computer(&foreign));

        let mut missing = DownloadItem {
            path: dir.join("missing.bin").to_string_lossy().into_owned(),
            ..Default::default()
        };
        assert!(!adopt_local(std::slice::from_mut(&mut missing)));
        assert!(missing.computer_name.is_empty());
        assert!(!on_this_computer(&missing));
        let _ = std::fs::remove_dir_all(dir);
    }
}
