//! Keeps one frozen page from freezing the whole browser.
//!
//! WebView2 parents windows owned by `msedgewebview2.exe` under ours, and moving, resizing or
//! destroying a page view sends them synchronous messages. When one of those processes stops
//! answering, the main thread waits with it and Windows offers to close the app. A watchdog
//! thread asks Windows what the stuck main thread is waiting on and ends that page or GPU
//! process instead. The tab shows its crash page and everything else keeps working.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::AppHandle;

use crate::{page_errors, status};

const TICK: Duration = Duration::from_millis(500);
/// Windows shows "Not responding" after five seconds, so act before that.
const STALL: Duration = Duration::from_secs(3);
const RETRY: Duration = Duration::from_secs(3);
/// When the chain stops at the browser process, each guess needs a longer stall than the
/// last: hung pages first, then the GPU process, then the pages on screen.
const GUESSES: [Duration; 3] = [
    Duration::from_secs(3),
    Duration::from_secs(6),
    Duration::from_secs(10),
];
/// A crash page within this long of the watchdog ending a process credits it.
const CREDIT: Duration = Duration::from_secs(10);
const WEBVIEW_EXE: &str = "msedgewebview2.exe";

const WCT_MAX_NODE_COUNT: usize = 16;
const WCT_OUT_OF_PROC_FLAG: u32 = 0x1;
const WCT_THREAD_TYPE: u32 = 8;
const WCT_STATUS_BLOCKED: u32 = 3;
const WCT_STATUS_PID_ONLY: u32 = 4;
const WCT_STATUS_PID_ONLY_RPCSS: u32 = 5;

static MAIN_THREAD: AtomicU32 = AtomicU32::new(0);
static ENDED_AT: Mutex<Option<Instant>> = Mutex::new(None);

/// Starts the watchdog. Call it on the main thread.
pub fn init(app: &AppHandle) {
    MAIN_THREAD.store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);
    let app = app.clone();
    std::thread::spawn(move || watch(&app));
}

/// True on the thread that runs the event loop.
pub fn on_main_thread() -> bool {
    MAIN_THREAD.load(Ordering::SeqCst) == unsafe { GetCurrentThreadId() }
}

/// The watchdog ended a page process a moment ago, so a crash page can say why.
pub fn ended_recently() -> bool {
    ENDED_AT
        .lock()
        .ok()
        .and_then(|at| *at)
        .is_some_and(|at| at.elapsed() < CREDIT)
}

/// Ends the renderer of tab `id` when WebView2 reported it hung, so destroying its page view
/// does not wait on it.
pub fn end_frozen_page(id: u32) {
    let pids = page_errors::unresponsive_pids(id);
    if pids.is_empty() {
        return;
    }
    let processes = webview_processes();
    for pid in pids {
        if role_of(&processes, pid) == Some("renderer") {
            end(pid, "renderer");
        }
    }
}

fn watch(app: &AppHandle) {
    let answered = Arc::new(AtomicBool::new(true));
    let mut posted = Instant::now();
    let mut last_tick = Instant::now();
    let mut last_rescue: Option<Instant> = None;
    let mut guess = 0;
    loop {
        std::thread::sleep(TICK);
        let now = Instant::now();
        // After sleep or hibernation the beat in flight is late, not lost.
        if now.duration_since(last_tick) > TICK * 4 {
            posted = now;
        }
        last_tick = now;
        if answered.swap(false, Ordering::SeqCst) {
            guess = 0;
            posted = now;
            let beat = answered.clone();
            if app
                .run_on_main_thread(move || beat.store(true, Ordering::SeqCst))
                .is_err()
            {
                return;
            }
            continue;
        }
        let stalled = now.duration_since(posted);
        if stalled < STALL || last_rescue.is_some_and(|at| now.duration_since(at) < RETRY) {
            continue;
        }
        if rescue(stalled, &mut guess) {
            last_rescue = Some(now);
        }
    }
}

/// Ends the WebView2 process the main thread is stuck on. True when it ended one.
fn rescue(stalled: Duration, guess: &mut usize) -> bool {
    let Some(waits_on) = blocking_processes(MAIN_THREAD.load(Ordering::SeqCst)) else {
        return false;
    };
    let processes = webview_processes();
    let chain: Vec<(u32, &str)> = waits_on
        .iter()
        .filter_map(|pid| Some((*pid, role_of(&processes, *pid)?)))
        .collect();
    if chain.is_empty() {
        return false;
    }
    if let Some(&(pid, role)) = chain.iter().rev().find(|(_, role)| endable(role)) {
        end(pid, role);
        return true;
    }
    // Only the browser process shows up: it waits on a helper in a way the chain can't follow.
    while *guess < GUESSES.len() && stalled >= GUESSES[*guess] {
        let (role, candidates) = match *guess {
            0 => ("renderer", page_errors::all_unresponsive_pids()),
            1 => ("gpu-process", with_role(&processes, "gpu-process")),
            _ => ("renderer", status::shown_renderers()),
        };
        *guess += 1;
        let targets: Vec<u32> = candidates
            .into_iter()
            .filter(|pid| role_of(&processes, *pid) == Some(role))
            .collect();
        if targets.is_empty() {
            continue;
        }
        for pid in targets {
            end(pid, role);
        }
        return true;
    }
    false
}

fn end(pid: u32, role: &str) {
    if let Ok(mut at) = ENDED_AT.lock() {
        *at = Some(Instant::now());
    }
    let ended = status::terminate(pid);
    eprintln!("hang guard: ended WebView2 {role} process {pid}: {ended}");
}

fn endable(role: &str) -> bool {
    matches!(role, "renderer" | "gpu-process" | "utility")
}

/// This app's WebView2 processes with their Chromium role: `renderer`, `gpu-process`,
/// `utility`, or empty for the browser process itself.
fn webview_processes() -> Vec<(u32, String)> {
    status::child_processes()
        .into_iter()
        .filter(|(_, exe)| exe.eq_ignore_ascii_case(WEBVIEW_EXE))
        .filter_map(|(pid, _)| Some((pid, process_type(&command_line(pid)?).to_string())))
        .collect()
}

fn role_of(processes: &[(u32, String)], pid: u32) -> Option<&str> {
    processes
        .iter()
        .find(|(known, _)| *known == pid)
        .map(|(_, role)| role.as_str())
}

fn with_role(processes: &[(u32, String)], role: &str) -> Vec<u32> {
    processes
        .iter()
        .filter(|(_, known)| known == role)
        .map(|(pid, _)| *pid)
        .collect()
}

/// The `--type=` switch Chromium starts its helper processes with. Empty for the browser process.
fn process_type(command_line: &str) -> &str {
    command_line
        .split_whitespace()
        .find_map(|part| part.trim_matches('"').strip_prefix("--type="))
        .unwrap_or("")
}

/// Processes the thread is blocked on, nearest first. `None` when it is running.
fn blocking_processes(thread: u32) -> Option<Vec<u32>> {
    if thread == 0 {
        return None;
    }
    let session = unsafe { OpenThreadWaitChainSession(0, std::ptr::null()) };
    if session.is_null() {
        return None;
    }
    let mut nodes = [unsafe { std::mem::zeroed::<WaitChainNode>() }; WCT_MAX_NODE_COUNT];
    let mut count = nodes.len() as u32;
    let mut cycle = 0i32;
    let ok = unsafe {
        GetThreadWaitChain(
            session,
            0,
            WCT_OUT_OF_PROC_FLAG,
            thread,
            &mut count,
            nodes.as_mut_ptr(),
            &mut cycle,
        )
    } != 0;
    unsafe { CloseThreadWaitChainSession(session) };
    if !ok {
        return None;
    }
    let nodes = &nodes[..(count as usize).min(WCT_MAX_NODE_COUNT)];
    if nodes
        .first()
        .is_none_or(|node| node.object_status != WCT_STATUS_BLOCKED)
    {
        return None;
    }
    let ours = unsafe { GetCurrentProcessId() };
    let mut pids = Vec::new();
    for node in nodes.iter().skip(1) {
        let names_process = node.object_type == WCT_THREAD_TYPE
            || matches!(
                node.object_status,
                WCT_STATUS_PID_ONLY | WCT_STATUS_PID_ONLY_RPCSS
            );
        if !names_process {
            continue;
        }
        let pid = unsafe { node.data.thread.process_id };
        if pid != 0 && pid != ours && !pids.contains(&pid) {
            pids.push(pid);
        }
    }
    Some(pids)
}

fn command_line(pid: u32) -> Option<String> {
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const PROCESS_COMMAND_LINE_INFORMATION: u32 = 60;
    const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004_u32 as i32;
    const STATUS_BUFFER_TOO_SMALL: i32 = 0xC000_0023_u32 as i32;
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }
    let mut buffer = vec![0u64; 1024];
    let status = loop {
        let size = (buffer.len() * 8) as u32;
        let mut needed = 0u32;
        let status = unsafe {
            NtQueryInformationProcess(
                handle,
                PROCESS_COMMAND_LINE_INFORMATION,
                buffer.as_mut_ptr().cast(),
                size,
                &mut needed,
            )
        };
        let short = matches!(
            status,
            STATUS_INFO_LENGTH_MISMATCH | STATUS_BUFFER_TOO_SMALL
        );
        if !short || needed <= size {
            break status;
        }
        buffer = vec![0u64; (needed as usize).div_ceil(8)];
    };
    unsafe { CloseHandle(handle) };
    if status < 0 {
        return None;
    }
    let text = unsafe { &*(buffer.as_ptr() as *const UnicodeString) };
    if text.buffer.is_null() || text.length == 0 {
        return Some(String::new());
    }
    let units = unsafe { std::slice::from_raw_parts(text.buffer, usize::from(text.length) / 2) };
    Some(String::from_utf16_lossy(units))
}

#[repr(C)]
#[derive(Clone, Copy)]
struct WaitChainNode {
    object_type: u32,
    object_status: u32,
    data: WaitChainData,
}

#[repr(C)]
#[derive(Clone, Copy)]
union WaitChainData {
    lock: LockObject,
    thread: ThreadObject,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct LockObject {
    name: [u16; 128],
    timeout: i64,
    alertable: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ThreadObject {
    process_id: u32,
    thread_id: u32,
    wait_time: u32,
    context_switches: u32,
}

#[repr(C)]
struct UnicodeString {
    length: u16,
    maximum_length: u16,
    buffer: *const u16,
}

#[link(name = "advapi32")]
unsafe extern "system" {
    fn OpenThreadWaitChainSession(flags: u32, callback: *const c_void) -> *mut c_void;
    fn CloseThreadWaitChainSession(session: *mut c_void);
    fn GetThreadWaitChain(
        session: *mut c_void,
        context: usize,
        flags: u32,
        thread: u32,
        count: *mut u32,
        nodes: *mut WaitChainNode,
        cycle: *mut i32,
    ) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentThreadId() -> u32;
    fn GetCurrentProcessId() -> u32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
    fn CloseHandle(handle: *mut c_void) -> i32;
}

#[link(name = "ntdll", kind = "raw-dylib")]
unsafe extern "system" {
    fn NtQueryInformationProcess(
        process: *mut c_void,
        class: u32,
        info: *mut c_void,
        length: u32,
        returned: *mut u32,
    ) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wait_chain_node_matches_the_windows_layout() {
        if cfg!(target_pointer_width = "64") {
            assert_eq!(std::mem::size_of::<WaitChainNode>(), 280);
            assert_eq!(std::mem::size_of::<UnicodeString>(), 16);
        }
    }

    #[test]
    fn chromium_roles_come_from_the_type_switch() {
        assert_eq!(
            process_type(r#""C:\WebView2\msedgewebview2.exe" --type=renderer --lang=en-US"#),
            "renderer"
        );
        assert_eq!(
            process_type(r#"msedgewebview2.exe --type=gpu-process --gpu-preferences=AB"#),
            "gpu-process"
        );
        assert_eq!(
            process_type(r#""msedgewebview2.exe" --embedded-browser-webview=1"#),
            ""
        );
    }

    #[test]
    fn the_main_thread_of_this_process_is_not_blocked() {
        assert!(
            blocking_processes(unsafe { GetCurrentThreadId() }).is_none_or(|pids| pids.is_empty())
        );
    }

    #[test]
    fn this_process_command_line_can_be_read() {
        let line = command_line(unsafe { GetCurrentProcessId() }).expect("command line");
        assert!(!line.is_empty());
    }
}
