use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::mem::size_of;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use url::Url;

use crate::{files, paths, system, tabs, webview2};

const FILE: &str = "stats.json";
const EMIT_EVERY: Duration = Duration::from_secs(2);
const SAVE_EVERY: Duration = Duration::from_secs(30);
/// A stalled tick must not add the hours the machine was asleep.
const MAX_TICK: u64 = 15_000;

static STARTED: LazyLock<SystemTime> = LazyLock::new(SystemTime::now);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusPage {
    pub id: u32,
    pub tab_used: Option<u64>,
    pub tab_shared: bool,
    pub tab_heap: Option<u64>,
    pub site_host: String,
    pub site_all_time_ms: u64,
    pub tab_session_ms: u64,
    /// The page is on screen, so the clocks keep counting from this snapshot.
    pub shown: bool,
}

/// Memory and time for the status bar. The flat fields describe the focused window's active tab.
/// `pages` covers every window, including both panes of a split.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusSnapshot {
    pub version: String,
    pub started_at: u64,
    pub system_used: u64,
    pub system_total: u64,
    pub app_used: u64,
    pub tab_used: Option<u64>,
    pub tab_shared: bool,
    pub tab_heap: Option<u64>,
    pub site_host: String,
    pub site_all_time_ms: u64,
    pub tab_session_ms: u64,
    pub battery_saver: bool,
    pub pages: Vec<StatusPage>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusText {
    pub tab_id: u32,
    pub text: String,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct UsageFile {
    sites: HashMap<String, u64>,
}

struct TabSample {
    used: Option<u64>,
    heap: Option<u64>,
    shared: bool,
}

struct Usage {
    sites: HashMap<String, u64>,
    session: HashMap<u32, u64>,
    dirty: bool,
    loaded: bool,
    last_tick: Instant,
    last_save: Instant,
    last_emit: Instant,
}

static USAGE: LazyLock<Mutex<Usage>> = LazyLock::new(|| {
    let now = Instant::now();
    Mutex::new(Usage {
        sites: HashMap::new(),
        session: HashMap::new(),
        dirty: false,
        loaded: false,
        last_tick: now,
        last_save: now,
        last_emit: now.checked_sub(EMIT_EVERY).unwrap_or(now),
    })
});

fn lock() -> std::sync::MutexGuard<'static, Usage> {
    USAGE.lock().expect("usage stats")
}

pub fn init(app: &AppHandle) {
    let _ = STARTED.elapsed();
    let mut state = lock();
    state.ensure_loaded(app);
}

impl Usage {
    fn ensure_loaded(&mut self, app: &AppHandle) {
        if self.loaded {
            return;
        }
        let file: UsageFile = files::read_json(&paths::data_file(app, FILE));
        self.sites = file.sites;
        self.loaded = true;
    }
}

/// Hovered link text from the page. An empty string clears it.
pub fn emit_status_text(app: &AppHandle, tab_id: u32, text: String) {
    let _ = app.emit("status-text", StatusText { tab_id, text });
}

/// Adds foreground time, and every couple of seconds publishes memory and the clocks.
pub fn tick(app: &AppHandle) {
    let targets = tabs::status_targets(app);
    let open = tabs::open_page_urls(app);
    let now = Instant::now();
    let emit = {
        let mut state = lock();
        state.ensure_loaded(app);
        let delta = now.saturating_duration_since(state.last_tick).as_millis() as u64;
        state.last_tick = now;
        if (1..MAX_TICK).contains(&delta) {
            for target in targets.iter().filter(|target| target.shown) {
                let key = site_key(&target.url);
                if key.is_empty() {
                    continue;
                }
                *state.session.entry(target.id).or_insert(0) += delta;
                *state.sites.entry(key).or_insert(0) += delta;
                state.dirty = true;
            }
        }
        let save = state.dirty && now.saturating_duration_since(state.last_save) >= SAVE_EVERY;
        let sites = save.then(|| state.sites.clone());
        if save {
            state.dirty = false;
            state.last_save = now;
        }
        let due = now.saturating_duration_since(state.last_emit) >= EMIT_EVERY;
        if due {
            state.last_emit = now;
        }
        (sites, due)
    };
    if let Some(sites) = emit.0 {
        store(app, sites);
    }
    if emit.1 {
        publish(app, &targets, &open);
    }
}

pub fn flush(app: &AppHandle) {
    let sites = {
        let mut state = lock();
        state.ensure_loaded(app);
        if !state.dirty {
            return;
        }
        state.dirty = false;
        state.last_save = Instant::now();
        state.sites.clone()
    };
    store(app, sites);
}

fn store(app: &AppHandle, sites: HashMap<String, u64>) {
    let file = UsageFile { sites };
    if let Err(error) = files::write_json(&paths::data_file(app, FILE), &file) {
        eprintln!("usage stats: {error}");
        if let Ok(mut state) = USAGE.lock() {
            state.dirty = true;
        }
    }
}

fn publish(app: &AppHandle, targets: &[tabs::StatusPageTarget], open: &[(u32, String)]) {
    let samples = tab_samples(app, targets, open);
    let (system_used, system_total) = system_memory();
    let app_used = app_working_set();
    let (sites, session) = {
        let state = lock();
        (state.sites.clone(), state.session.clone())
    };
    let pages: Vec<StatusPage> = targets
        .iter()
        .map(|target| {
            let key = site_key(&target.url);
            let sample = samples.get(&target.id);
            StatusPage {
                id: target.id,
                tab_used: sample.and_then(|item| item.used),
                tab_shared: sample.is_some_and(|item| item.shared),
                tab_heap: sample.and_then(|item| item.heap),
                site_host: key.clone(),
                site_all_time_ms: sites.get(&key).copied().unwrap_or(0),
                tab_session_ms: session.get(&target.id).copied().unwrap_or(0),
                shown: target.shown,
            }
        })
        .collect();
    let focused =
        tabs::active_id(app).and_then(|id| pages.iter().find(|page| page.id == id).cloned());
    let focused = focused.unwrap_or(StatusPage {
        id: 0,
        tab_used: None,
        tab_shared: false,
        tab_heap: None,
        site_host: String::new(),
        site_all_time_ms: 0,
        tab_session_ms: 0,
        shown: false,
    });
    let _ = app.emit(
        "status-changed",
        StatusSnapshot {
            version: env!("CARGO_PKG_VERSION").to_string(),
            started_at: started_at_ms(),
            system_used,
            system_total,
            app_used,
            tab_used: focused.tab_used,
            tab_shared: focused.tab_shared,
            tab_heap: focused.tab_heap,
            site_host: focused.site_host,
            site_all_time_ms: focused.site_all_time_ms,
            tab_session_ms: focused.tab_session_ms,
            battery_saver: system::saver_active(app),
            pages,
        },
    );
}

fn started_at_ms() -> u64 {
    STARTED
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn tab_samples(
    app: &AppHandle,
    targets: &[tabs::StatusPageTarget],
    open: &[(u32, String)],
) -> HashMap<u32, TabSample> {
    let mut frames: Vec<(u32, Vec<String>)> = Vec::new();
    let mut matched: HashMap<u32, Vec<u32>> = HashMap::new();
    for target in targets.iter().filter(|target| target.shown) {
        let candidates = page_candidates(app, &target.url);
        let mut pids = pids_for(&frames, &candidates);
        if pids.is_empty()
            && let Some(webview) = tabs::page_view(app, target.id)
        {
            for (pid, sources) in webview2::renderer_frames(&webview) {
                if frames.iter().any(|(known, _)| *known == pid) {
                    continue;
                }
                frames.push((pid, sources));
            }
            pids = pids_for(&frames, &candidates);
        }
        matched.insert(target.id, pids);
    }

    let mut samples = HashMap::new();
    for target in targets.iter().filter(|target| target.shown) {
        let key = site_key(&target.url);
        let pids = matched.get(&target.id).cloned().unwrap_or_default();
        let shared = (!key.is_empty()
            && open
                .iter()
                .any(|(id, url)| *id != target.id && site_key(url) == key))
            || pids.iter().any(|pid| {
                matched
                    .iter()
                    .any(|(other, other_pids)| *other != target.id && other_pids.contains(pid))
            });
        let bytes = sum_working_set(&pids);
        let (used, heap) = if bytes > 0 {
            (Some(bytes), None)
        } else {
            let heap = tabs::page_view(app, target.id)
                .and_then(|webview| webview2::js_heap_used(&webview));
            (None, heap)
        };
        samples.insert(target.id, TabSample { used, heap, shared });
    }
    samples
}

fn pids_for(frames: &[(u32, Vec<String>)], candidates: &[String]) -> Vec<u32> {
    frames
        .iter()
        .filter(|(_, sources)| {
            sources
                .iter()
                .any(|source| source_matches(source, candidates))
        })
        .map(|(pid, _)| *pid)
        .collect()
}

/// Display URL plus the real address of a `ce://` page, so a renderer frame can be matched.
fn page_candidates(app: &AppHandle, display: &str) -> Vec<String> {
    let mut urls = vec![normalize_match(display)];
    if let Some(rest) = display.trim().strip_prefix("ce://") {
        let page = rest
            .split(['?', '#', '/'])
            .next()
            .filter(|page| !page.is_empty())
            .unwrap_or("newtab");
        urls.push(normalize_match(&paths::internal_url(app, page).to_string()));
        urls.push(format!("#/{page}").to_ascii_lowercase());
    }
    urls.retain(|url| !url.is_empty());
    urls
}

fn source_matches(source: &str, candidates: &[String]) -> bool {
    let source = normalize_match(source);
    candidates.iter().any(|candidate| {
        if let Some(page) = candidate.strip_prefix("#/") {
            fragment_matches(&source, page)
        } else {
            source == *candidate
        }
    })
}

fn fragment_matches(source: &str, page: &str) -> bool {
    if page.is_empty() {
        return false;
    }
    let marker = format!("#/{page}");
    let Some(index) = source.find(&marker) else {
        return false;
    };
    let rest = &source[index + marker.len()..];
    rest.is_empty() || rest.starts_with(['?', '/'])
}

fn normalize_match(value: &str) -> String {
    value.trim().trim_end_matches('/').to_ascii_lowercase()
}

/// `www.example.com` and `example.com` share one key. Internal pages keep `ce://<page>`.
fn site_key(url: &str) -> String {
    let url = url.trim();
    if url.is_empty() {
        return String::new();
    }
    if let Some(rest) = url.strip_prefix("ce://") {
        let page = rest
            .split(['?', '#', '/'])
            .next()
            .filter(|page| !page.is_empty())
            .unwrap_or("newtab");
        return format!("ce://{page}");
    }
    let Ok(parsed) = Url::parse(url) else {
        return String::new();
    };
    let Some(host) = parsed.host_str() else {
        return String::new();
    };
    let host = host.to_ascii_lowercase();
    site_host(&host).to_string()
}

/// Drops a single leading `www` label when a real domain remains, so `www.example.com`
/// matches `example.com` without turning `www.com` into `com`.
fn site_host(host: &str) -> &str {
    match host.split_once('.') {
        Some(("www", rest)) if rest.contains('.') => rest,
        _ => host,
    }
}

fn system_memory() -> (u64, u64) {
    let mut status = unsafe { std::mem::zeroed::<MemoryStatusEx>() };
    status.length = size_of::<MemoryStatusEx>() as u32;
    if unsafe { GlobalMemoryStatusEx(&mut status) } == 0 {
        return (0, 0);
    }
    let total = status.total_phys;
    (total.saturating_sub(status.avail_phys), total)
}

fn app_working_set() -> u64 {
    let mut rows = Vec::new();
    let root = unsafe { GetCurrentProcessId() };
    if !snapshot_processes(&mut rows) {
        return working_set(root);
    }
    let mut keep = HashSet::new();
    keep.insert(root);
    let mut grew = true;
    while grew {
        grew = false;
        for &(pid, parent) in &rows {
            if parent != 0 && keep.contains(&parent) && keep.insert(pid) {
                grew = true;
            }
        }
    }
    sum_working_set(&keep.into_iter().collect::<Vec<_>>())
}

fn sum_working_set(pids: &[u32]) -> u64 {
    let mut seen = HashSet::new();
    pids.iter()
        .copied()
        .filter(|pid| seen.insert(*pid))
        .map(working_set)
        .sum()
}

fn snapshot_processes(rows: &mut Vec<(u32, u32)>) -> bool {
    let snap = unsafe { CreateToolhelp32Snapshot(0x0000_0002, 0) };
    if invalid_handle(snap) {
        return false;
    }
    let mut entry: ProcessEntry = unsafe { std::mem::zeroed() };
    entry.size = size_of::<ProcessEntry>() as u32;
    if unsafe { Process32FirstW(snap, &mut entry) } == 0 {
        unsafe { CloseHandle(snap) };
        return false;
    }
    loop {
        rows.push((entry.pid, entry.parent));
        entry.size = size_of::<ProcessEntry>() as u32;
        if unsafe { Process32NextW(snap, &mut entry) } == 0 {
            break;
        }
    }
    unsafe { CloseHandle(snap) };
    true
}

fn working_set(pid: u32) -> u64 {
    const ACCESS: u32 = 0x1000 | 0x0010;
    let handle = unsafe { OpenProcess(ACCESS, 0, pid) };
    if invalid_handle(handle) {
        return 0;
    }
    let mut counters: ProcessMemoryCountersEx = unsafe { std::mem::zeroed() };
    counters.cb = size_of::<ProcessMemoryCountersEx>() as u32;
    let ok = unsafe { GetProcessMemoryInfo(handle, &mut counters, counters.cb) };
    unsafe { CloseHandle(handle) };
    if ok == 0 {
        0
    } else {
        counters.working_set as u64
    }
}

fn invalid_handle(handle: *mut c_void) -> bool {
    handle.is_null() || handle == usize::MAX as *mut c_void
}

#[repr(C)]
struct MemoryStatusEx {
    length: u32,
    memory_load: u32,
    total_phys: u64,
    avail_phys: u64,
    total_page: u64,
    avail_page: u64,
    total_virtual: u64,
    avail_virtual: u64,
    avail_extended_virtual: u64,
}

#[repr(C)]
struct ProcessEntry {
    size: u32,
    usage: u32,
    pid: u32,
    heap: usize,
    module: u32,
    threads: u32,
    parent: u32,
    pri_class_base: i32,
    flags: u32,
    exe: [u16; 260],
}

#[repr(C)]
struct ProcessMemoryCountersEx {
    cb: u32,
    page_fault_count: u32,
    peak_working_set: usize,
    working_set: usize,
    quota_peak_paged_pool: usize,
    quota_paged_pool: usize,
    quota_peak_non_paged_pool: usize,
    quota_non_paged_pool: usize,
    pagefile: usize,
    peak_pagefile: usize,
    private_usage: usize,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GlobalMemoryStatusEx(buffer: *mut MemoryStatusEx) -> i32;
    fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> *mut c_void;
    fn Process32FirstW(snapshot: *mut c_void, entry: *mut ProcessEntry) -> i32;
    fn Process32NextW(snapshot: *mut c_void, entry: *mut ProcessEntry) -> i32;
    fn CloseHandle(handle: *mut c_void) -> i32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
    fn GetCurrentProcessId() -> u32;
}

#[link(name = "psapi")]
unsafe extern "system" {
    fn GetProcessMemoryInfo(
        process: *mut c_void,
        counters: *mut ProcessMemoryCountersEx,
        cb: u32,
    ) -> i32;
}

#[cfg(test)]
mod tests {
    use super::{
        MemoryStatusEx, ProcessEntry, ProcessMemoryCountersEx, fragment_matches, site_key,
        source_matches,
    };

    #[test]
    fn site_key_strips_www_and_keeps_ce_pages() {
        assert_eq!(site_key("https://www.Example.com/a"), "example.com");
        assert_eq!(site_key("https://example.com"), "example.com");
        assert_eq!(site_key("https://www.example.co.uk/"), "example.co.uk");
        assert_eq!(site_key("https://www.com/"), "www.com");
        assert_eq!(site_key("ce://newtab"), "ce://newtab");
        assert_eq!(site_key("ce://settings?x=1"), "ce://settings");
        assert_eq!(site_key(""), "");
    }

    #[test]
    fn a_frame_matches_its_page_without_swallowing_a_longer_name() {
        assert!(source_matches(
            "https://www.example.com/a",
            &["https://www.example.com/a".to_string()],
        ));
        assert!(source_matches(
            "http://localhost:1420/#/newtab",
            &["#/newtab".to_string()]
        ));
        assert!(!source_matches(
            "http://localhost:1420/#/settings",
            &["#/newtab".to_string()]
        ));
        assert!(!fragment_matches("http://localhost:1420/#/newtab", "new"));
    }

    #[test]
    fn process_structs_match_the_windows_layout() {
        if cfg!(target_pointer_width = "64") {
            assert_eq!(std::mem::size_of::<ProcessEntry>(), 568);
            assert_eq!(std::mem::size_of::<ProcessMemoryCountersEx>(), 80);
            assert_eq!(std::mem::size_of::<MemoryStatusEx>(), 64);
        }
    }
}
