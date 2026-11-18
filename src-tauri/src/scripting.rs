//! Per-site scripts in `scripting.json`. JavaScript and TypeScript run in the page.
//! Python runs there too, through a Pyodide runtime cached under local app data.
//! A manual run can be stopped. JavaScript is cut off immediately. Python stops when it
//! next returns to the page, and sooner when the runtime accepts an interrupt.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_HOST_RESOURCE_ACCESS_KIND_ALLOW, ICoreWebView2, ICoreWebView2_3,
};
use windows::core::{HSTRING, Interface};

use crate::{files, paths, settings, tabs, webview2};

const FILE: &str = "scripting.json";
const MAX_SCRIPTS: usize = 200;
const MAX_CODE: usize = 256 * 1024;
const MAX_NAME: usize = 80;
const PYODIDE_VERSION: &str = "0.27.7";
const PYODIDE_HOST: &str = "ce-pyodide.localhost";
/// The wasm is about 8 MB. A smaller file is a partial download.
const MIN_WASM: u64 = 1_000_000;
const PYODIDE_FILES: &[&str] = &[
    "pyodide.mjs",
    "pyodide.asm.js",
    "pyodide.asm.wasm",
    "python_stdlib.zip",
    "pyodide-lock.json",
];

static FETCH: Mutex<()> = Mutex::new(());
/// Run id the panel is executing. `0` means nothing is running.
static ACTIVE_RUN: AtomicU64 = AtomicU64::new(0);
/// Run id the panel asked to stop. Compared with [`ACTIVE_RUN`].
static CANCEL_RUN: AtomicU64 = AtomicU64::new(0);

struct ActiveRun(u64);

impl ActiveRun {
    fn enter(run_id: u64) -> Self {
        ACTIVE_RUN.store(run_id, Ordering::SeqCst);
        Self(run_id)
    }
}

impl Drop for ActiveRun {
    fn drop(&mut self) {
        if ACTIVE_RUN.load(Ordering::SeqCst) == self.0 {
            ACTIVE_RUN.store(0, Ordering::SeqCst);
        }
    }
}

fn cancelled() -> bool {
    let active = ACTIVE_RUN.load(Ordering::SeqCst);
    active != 0 && CANCEL_RUN.load(Ordering::SeqCst) == active
}

fn stopped_run() -> ScriptRun {
    ScriptRun {
        logs: Vec::new(),
        value: None,
        error: Some("Stopped".into()),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct PageScript {
    pub id: String,
    pub name: String,
    pub domain: String,
    /// Full page address, without a hash or a trailing slash.
    #[serde(default)]
    pub url: String,
    /// `domain` runs on the whole site. `url` runs only on `url`.
    /// Older files omit this and run on the whole domain.
    #[serde(default = "default_scope")]
    pub scope: String,
    /// `javascript`, `typescript`, `python`, or `css`.
    pub language: String,
    pub enabled: bool,
    pub run_on_load: bool,
    pub source: String,
    /// JavaScript emitted for JavaScript and TypeScript. Empty for Python.
    pub compiled: String,
}

impl Default for PageScript {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            domain: String::new(),
            url: String::new(),
            scope: default_scope(),
            language: "javascript".into(),
            enabled: true,
            run_on_load: false,
            source: String::new(),
            compiled: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ScriptFile {
    pub scripts: Vec<PageScript>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptLog {
    pub level: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptRun {
    pub logs: Vec<ScriptLog>,
    pub value: Option<String>,
    pub error: Option<String>,
}

struct ScriptState {
    scripts: Mutex<ScriptFile>,
}

pub fn init(app: &AppHandle) {
    let loaded: ScriptFile = files::read_json(&paths::data_file(app, FILE));
    app.manage(ScriptState {
        scripts: Mutex::new(loaded.normalize()),
    });
}

fn state(app: &AppHandle) -> tauri::State<'_, ScriptState> {
    app.state::<ScriptState>()
}

pub fn get(app: &AppHandle) -> ScriptFile {
    state(app).scripts.lock().expect("scripting").clone()
}

fn write(app: &AppHandle, file: &ScriptFile) -> Result<(), String> {
    files::write_json(&paths::data_file(app, FILE), file)
}

fn publish(app: &AppHandle, file: &ScriptFile) {
    let _ = app.emit("scripting-changed", file);
    tabs::reinject_scripting(app);
}

fn save_file(app: &AppHandle, file: ScriptFile) -> Result<ScriptFile, String> {
    let file = file.normalize();
    write(app, &file)?;
    *state(app).scripts.lock().expect("scripting") = file.clone();
    publish(app, &file);
    Ok(file)
}

pub fn save_script(app: &AppHandle, mut script: PageScript) -> Result<ScriptFile, String> {
    let is_new = script.id.trim().is_empty();
    script = normalize_script(script)?;
    let mut file = get(app);
    if is_new {
        let base = script.id.clone();
        let mut suffix = 2;
        while file.scripts.iter().any(|item| item.id == script.id) {
            script.id = format!("{base}-{suffix}");
            suffix += 1;
        }
    }
    if let Some(existing) = file.scripts.iter_mut().find(|item| item.id == script.id) {
        *existing = script;
    } else {
        if file.scripts.len() >= MAX_SCRIPTS {
            return Err(format!("You can save up to {MAX_SCRIPTS} scripts."));
        }
        file.scripts.push(script);
    }
    save_file(app, file)
}

pub fn delete_script(app: &AppHandle, id: &str) -> Result<ScriptFile, String> {
    let mut file = get(app);
    file.scripts.retain(|script| script.id != id);
    save_file(app, file)
}

/// JavaScript injected as a matching http(s) document loads. Empty when nothing should run.
pub fn injection(app: &AppHandle, url: &str) -> String {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return String::new();
    }
    if app.try_state::<ScriptState>().is_none() {
        return String::new();
    }
    let file = get(app);
    let javascript: Vec<Value> = file
        .scripts
        .iter()
        .filter(|script| {
            script.enabled
                && script.run_on_load
                && script.language != "python"
                && script.language != "css"
                && !script.compiled.is_empty()
                && script_matches(script, url)
        })
        .map(|script| {
            json!({
                "name": script.name,
                "js": script.compiled,
            })
        })
        .collect();
    let python: Vec<Value> = if pyodide_ready(app) {
        file.scripts
            .iter()
            .filter(|script| {
                script.enabled
                    && script.run_on_load
                    && script.language == "python"
                    && !script.source.is_empty()
                    && script_matches(script, url)
            })
            .map(|script| {
                json!({
                    "name": script.name,
                    "source": script.source,
                })
            })
            .collect()
    } else {
        Vec::new()
    };
    let css: Vec<Value> = file
        .scripts
        .iter()
        .filter(|script| {
            script.enabled
                && script.run_on_load
                && script.language == "css"
                && !script.compiled.is_empty()
                && script_matches(script, url)
        })
        .map(|script| {
            json!({
                "name": script.name,
                "css": script.compiled,
            })
        })
        .collect();
    if javascript.is_empty() && css.is_empty() && python.is_empty() {
        return String::new();
    }
    let javascript = serde_json::to_string(&javascript).unwrap_or_else(|_| "[]".into());
    let css = serde_json::to_string(&css).unwrap_or_else(|_| "[]".into());
    let python = serde_json::to_string(&python).unwrap_or_else(|_| "[]".into());
    fill_once(
        &fill_once(
            &fill_once(ONLOAD, "__CE_JS__", &javascript),
            "__CE_CSS__",
            &css,
        ),
        "__CE_PY__",
        &python,
    )
}

/// Replaces the first placeholder and leaves the inserted text alone.
fn fill_once(template: &str, key: &str, value: &str) -> String {
    match template.split_once(key) {
        Some((head, tail)) => format!("{head}{value}{tail}"),
        None => template.to_string(),
    }
}

/// Runs `source` in the page of tab `tab_id`. TypeScript arrives already compiled to JavaScript.
/// `run_id` is the panel's token for this attempt. [`stop`] with the same id cancels it.
pub fn run(
    app: &AppHandle,
    tab_id: u32,
    language: &str,
    source: &str,
    expression: bool,
    run_id: u64,
) -> Result<ScriptRun, String> {
    let _active = ActiveRun::enter(run_id);
    if cancelled() {
        return Ok(stopped_run());
    }
    let url = tabs::tab_page_url(app, tab_id).ok_or("This tab is not open.")?;
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("Scripts run on a website. Open an http or https page.".into());
    }
    if source.len() > MAX_CODE {
        return Err(format!("A script can hold up to {} KB.", MAX_CODE / 1024));
    }
    let webview = tabs::page_view(app, tab_id).ok_or("This page is not loaded yet.")?;
    let language = canonical_language(language);
    let (program, timeout) = if language == "python" {
        if let Err(error) = ensure_pyodide(app) {
            if cancelled() || error == "Stopped" {
                return Ok(stopped_run());
            }
            return Err(error);
        }
        if cancelled() {
            return Ok(stopped_run());
        }
        map_webview(app, &webview)?;
        (python_runner(source), Duration::from_secs(120))
    } else if language == "css" {
        (css_runner(source), Duration::from_secs(20))
    } else {
        (
            javascript_runner(source, expression),
            Duration::from_secs(20),
        )
    };
    if cancelled() {
        return Ok(stopped_run());
    }
    // Wait for the page to actually finish. Bailing out when stop is requested reports
    // success while a JavaScript loop is still running.
    let value = match webview2::evaluate(&webview, &program, timeout, "The script") {
        Ok(value) => value,
        Err(_) if cancelled() => return Ok(read_stopped(&webview)),
        Err(error) => return Err(error),
    };
    let mut run = parse_run(value);
    if cancelled() && run.error.is_some() {
        run.error = Some("Stopped".into());
    }
    Ok(run)
}

/// Cancels the run identified by `run_id`.
///
/// `Runtime.terminateExecution` has to go out before any other page script. A stop hook
/// queued first runs at the next safepoint, returns, and the JavaScript loop resumes.
/// The hook is sent twice because that termination can consume the next script when the
/// page was idle.
pub fn stop(app: &AppHandle, tab_id: u32, run_id: u64) -> Result<(), String> {
    if run_id != 0 {
        CANCEL_RUN.store(run_id, Ordering::SeqCst);
    }
    let Some(webview) = tabs::page_view(app, tab_id) else {
        return Ok(());
    };
    let _ = webview2::call_cdp_json(
        &webview,
        "Runtime.terminateExecution",
        "{}",
        Duration::from_secs(2),
        "Stop",
    );
    if webview2::evaluate(&webview, STOP_JS, Duration::from_secs(2), "Stop").is_err() {
        let _ = webview2::evaluate(&webview, STOP_JS, Duration::from_secs(2), "Stop");
    }
    Ok(())
}

fn read_stopped(webview: &tauri::Webview) -> ScriptRun {
    match webview2::evaluate(webview, HARVEST_JS, Duration::from_millis(500), "Stop") {
        Ok(value) => {
            let mut run = parse_run(value);
            if run.error.is_none() {
                run.error = Some("Stopped".into());
            }
            run
        }
        Err(_) => stopped_run(),
    }
}

pub fn pyodide_ready(app: &AppHandle) -> bool {
    let dir = cache_dir(app);
    PYODIDE_FILES.iter().all(|name| {
        let path = dir.join(name);
        let Ok(meta) = fs::metadata(&path) else {
            return false;
        };
        if *name == "pyodide.asm.wasm" {
            meta.len() >= MIN_WASM
        } else {
            meta.len() > 0
        }
    })
}

/// Serves the cached runtime at `https://ce-pyodide.localhost/` for this webview.
pub fn map_pyodide(app: &AppHandle, core: &ICoreWebView2) {
    let folder = cache_dir(app);
    if !folder.is_dir() {
        return;
    }
    let Ok(core3) = core.clone().cast::<ICoreWebView2_3>() else {
        return;
    };
    let host = HSTRING::from(PYODIDE_HOST);
    let path = HSTRING::from(folder.to_string_lossy().as_ref());
    let _ = unsafe {
        core3.SetVirtualHostNameToFolderMapping(
            &host,
            &path,
            COREWEBVIEW2_HOST_RESOURCE_ACCESS_KIND_ALLOW,
        )
    };
}

fn map_webview(app: &AppHandle, webview: &tauri::Webview) -> Result<(), String> {
    let app = app.clone();
    webview
        .with_webview(move |platform| unsafe {
            if let Ok(core) = platform.controller().CoreWebView2() {
                map_pyodide(&app, &core);
            }
        })
        .map_err(|error| error.to_string())
}

fn remap_all(app: &AppHandle) {
    let labels: Vec<String> = app.webviews().keys().cloned().collect();
    for label in labels {
        let Some(view) = app.get_webview(&label) else {
            continue;
        };
        let _ = map_webview(app, &view);
    }
}

pub fn ensure_pyodide(app: &AppHandle) -> Result<(), String> {
    let _guard = FETCH
        .lock()
        .map_err(|_| "The Python runtime is busy.".to_string())?;
    if pyodide_ready(app) {
        return Ok(());
    }
    if cancelled() {
        return Err("Stopped".into());
    }
    let dir = cache_dir(app);
    fs::create_dir_all(&dir).map_err(|error| {
        format!(
            "Could not create the Python runtime folder {}: {error}",
            dir.display()
        )
    })?;
    for name in PYODIDE_FILES {
        if cancelled() {
            return Err("Stopped".into());
        }
        let dest = dir.join(name);
        if cached(&dest, name) {
            continue;
        }
        let url = format!("https://cdn.jsdelivr.net/pyodide/v{PYODIDE_VERSION}/full/{name}");
        download(&url, &dest)?;
    }
    if !pyodide_ready(app) {
        return Err("The Python runtime download was incomplete.".into());
    }
    remap_all(app);
    Ok(())
}

fn cached(path: &Path, name: &str) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    if name == "pyodide.asm.wasm" {
        meta.len() >= MIN_WASM
    } else {
        meta.len() > 0
    }
}

fn download(url: &str, dest: &Path) -> Result<(), String> {
    let temp = dest.with_extension("partial");
    let response = ureq::get(url)
        .timeout(Duration::from_secs(180))
        .call()
        .map_err(|error| format!("Could not download the Python runtime: {error}"))?;
    let mut file = File::create(&temp).map_err(|error| {
        format!(
            "Could not write the Python runtime {}: {error}",
            temp.display()
        )
    })?;
    let mut reader = response.into_reader();
    let mut buf = [0u8; 64 * 1024];
    loop {
        if cancelled() {
            drop(file);
            let _ = fs::remove_file(&temp);
            return Err("Stopped".into());
        }
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(count) => {
                if let Err(error) = file.write_all(&buf[..count]) {
                    drop(file);
                    let _ = fs::remove_file(&temp);
                    return Err(format!("Could not download the Python runtime: {error}"));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => {
                drop(file);
                let _ = fs::remove_file(&temp);
                return Err(format!("Could not download the Python runtime: {error}"));
            }
        }
    }
    file.flush().ok();
    drop(file);
    fs::rename(&temp, dest).map_err(|error| {
        format!(
            "Could not write the Python runtime {}: {error}",
            dest.display()
        )
    })?;
    if dest
        .file_name()
        .is_some_and(|name| name == "pyodide.asm.wasm")
        && fs::metadata(dest).map(|meta| meta.len()).unwrap_or(0) < MIN_WASM
    {
        let _ = fs::remove_file(dest);
        return Err("The Python runtime download was incomplete.".into());
    }
    Ok(())
}

fn cache_dir(app: &AppHandle) -> PathBuf {
    paths::root_local_dir(app).join("pyodide")
}

fn parse_run(value: Value) -> ScriptRun {
    let logs = value
        .get("logs")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some(ScriptLog {
                        level: item.get("level")?.as_str()?.to_string(),
                        text: item.get("text")?.as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let error = value
        .get("error")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_string);
    let shown = match value.get("value") {
        Some(Value::Null) | None => None,
        Some(Value::String(text)) => Some(text.clone()),
        Some(other) => Some(other.to_string()),
    };
    ScriptRun {
        logs,
        value: shown,
        error,
    }
}

fn javascript_runner(source: &str, expression: bool) -> String {
    let encoded = serde_json::to_string(source).unwrap_or_else(|_| "\"\"".into());
    JS_RUNNER
        .replacen("__CE_CONTROL__", SCRIPT_CONTROL, 1)
        .replacen("__CE_EXPR__", if expression { "true" } else { "false" }, 1)
        .replacen("__CE_SOURCE__", &encoded, 1)
}

fn python_runner(source: &str) -> String {
    let encoded = serde_json::to_string(source).unwrap_or_else(|_| "\"\"".into());
    PY_RUNNER
        .replacen("__CE_CONTROL__", SCRIPT_CONTROL, 1)
        .replacen("__CE_SOURCE__", &encoded, 1)
}

fn css_runner(source: &str) -> String {
    let encoded = serde_json::to_string(source).unwrap_or_else(|_| "\"\"".into());
    CSS_RUNNER.replacen("__CE_SOURCE__", &encoded, 1)
}

/// Arms a stop hook for one run. Timers, animation frames, fetches, and XHR calls made
/// while it is armed are cancelled when the panel asks the page to stop.
const SCRIPT_CONTROL: &str = r#"(() => {
  const slot = globalThis.__ceScriptRun || (globalThis.__ceScriptRun = { stop: null, logs: [] });
  const logs = [];
  slot.logs = logs;
  const state = { stopped: false };
  const timers = new Set();
  const intervals = new Set();
  const frames = new Set();
  const fetches = new Set();
  const requests = new Set();
  const orig = {
    setTimeout: globalThis.setTimeout.bind(globalThis),
    clearTimeout: globalThis.clearTimeout.bind(globalThis),
    setInterval: globalThis.setInterval.bind(globalThis),
    clearInterval: globalThis.clearInterval.bind(globalThis),
    requestAnimationFrame: globalThis.requestAnimationFrame.bind(globalThis),
    cancelAnimationFrame: globalThis.cancelAnimationFrame.bind(globalThis),
    fetch: globalThis.fetch.bind(globalThis),
  };
  const xhrSend = XMLHttpRequest.prototype.send;
  let rejectStop = function () {};
  const stoppedPromise = new Promise(function (_, reject) {
    rejectStop = function () { reject(new Error("Stopped")); };
  });
  stoppedPromise.catch(function () {});
  const halt = function () {
    if (state.stopped) return;
    state.stopped = true;
    for (const id of timers) orig.clearTimeout(id);
    timers.clear();
    for (const id of intervals) orig.clearInterval(id);
    intervals.clear();
    for (const id of frames) orig.cancelAnimationFrame(id);
    frames.clear();
    for (const controller of fetches) {
      try { controller.abort(); } catch (error) {}
    }
    fetches.clear();
    for (const xhr of requests) {
      try { xhr.abort(); } catch (error) {}
    }
    requests.clear();
    const py = globalThis.__cePyodide;
    if (py && py.__ceInterrupt) py.__ceInterrupt[0] = 2;
    if (py && py.__ceInterruptWorker) {
      try { py.__ceInterruptWorker.postMessage("stop"); } catch (error) {}
    }
    rejectStop();
  };
  const restore = function () {
    if (slot.stop === halt) slot.stop = null;
    globalThis.setTimeout = orig.setTimeout;
    globalThis.clearTimeout = orig.clearTimeout;
    globalThis.setInterval = orig.setInterval;
    globalThis.clearInterval = orig.clearInterval;
    globalThis.requestAnimationFrame = orig.requestAnimationFrame;
    globalThis.cancelAnimationFrame = orig.cancelAnimationFrame;
    globalThis.fetch = orig.fetch;
    XMLHttpRequest.prototype.send = xhrSend;
  };
  slot.stop = halt;
  slot.restore = restore;
  globalThis.setTimeout = function (fn, delay) {
    if (state.stopped) return 0;
    const args = Array.prototype.slice.call(arguments, 2);
    const id = orig.setTimeout(function () {
      timers.delete(id);
      if (state.stopped || typeof fn !== "function") return;
      fn.apply(globalThis, args);
    }, delay);
    timers.add(id);
    return id;
  };
  globalThis.clearTimeout = function (id) {
    timers.delete(id);
    return orig.clearTimeout(id);
  };
  globalThis.setInterval = function (fn, delay) {
    if (state.stopped) return 0;
    const args = Array.prototype.slice.call(arguments, 2);
    const id = orig.setInterval(function () {
      if (state.stopped) {
        orig.clearInterval(id);
        intervals.delete(id);
        return;
      }
      if (typeof fn === "function") fn.apply(globalThis, args);
    }, delay);
    intervals.add(id);
    return id;
  };
  globalThis.clearInterval = function (id) {
    intervals.delete(id);
    return orig.clearInterval(id);
  };
  globalThis.requestAnimationFrame = function (fn) {
    if (state.stopped) return 0;
    const id = orig.requestAnimationFrame(function (time) {
      frames.delete(id);
      if (!state.stopped && typeof fn === "function") fn(time);
    });
    frames.add(id);
    return id;
  };
  globalThis.cancelAnimationFrame = function (id) {
    frames.delete(id);
    return orig.cancelAnimationFrame(id);
  };
  globalThis.fetch = function (input, init) {
    if (state.stopped) return Promise.reject(new Error("Stopped"));
    const controller = new AbortController();
    fetches.add(controller);
    const next = init ? Object.assign({}, init) : {};
    if (next.signal && typeof AbortSignal.any === "function") {
      next.signal = AbortSignal.any([next.signal, controller.signal]);
    } else {
      next.signal = controller.signal;
    }
    return orig.fetch(input, next).finally(function () { fetches.delete(controller); });
  };
  XMLHttpRequest.prototype.send = function () {
    if (state.stopped) throw new Error("Stopped");
    requests.add(this);
    this.addEventListener("loadend", function () { requests.delete(this); }, { once: true });
    return xhrSend.apply(this, arguments);
  };
  return {
    logs: logs,
    stoppedPromise: stoppedPromise,
    stopped: function () { return state.stopped; },
    errorText: function (error) {
      if (state.stopped || (error && error.message === "Stopped")) return "Stopped";
      const text = error && error.stack ? String(error.stack) : String(error);
      return text.slice(0, 8000);
    },
    restore: restore,
  };
})()"#;

const STOP_JS: &str = r#"(() => {
  const slot = globalThis.__ceScriptRun;
  if (!slot) return false;
  if (typeof slot.stop === "function") slot.stop();
  if (typeof slot.restore === "function") slot.restore();
  return true;
})()"#;

const HARVEST_JS: &str = r#"(() => {
  const slot = globalThis.__ceScriptRun || {};
  const logs = Array.isArray(slot.logs) ? slot.logs : [];
  return { logs: logs, value: null, error: "Stopped" };
})()"#;

const JS_RUNNER: &str = r#"(async () => {
  const expression = __CE_EXPR__;
  const source = __CE_SOURCE__;
  const control = __CE_CONTROL__;
  const logs = control.logs;
  const levels = ["log", "info", "warn", "error", "debug"];
  const original = {};
  const show = (value) => {
    if (value === undefined || value === null) return null;
    if (typeof value === "string") return value;
    if (typeof value === "number" || typeof value === "boolean") return String(value);
    try { return JSON.stringify(value); } catch (error) { return String(value); }
  };
  for (const level of levels) {
    original[level] = console[level].bind(console);
    console[level] = (...args) => {
      const text = args.map((item) => show(item) ?? "null").join(" ").slice(0, 8000);
      if (logs.length < 200) logs.push({ level, text });
      original[level](...args);
    };
  }
  const task = (async () => {
    if (expression) {
      const expressionSource = source.trim().replace(/;+\s*$/, "");
      try {
        return await (0, eval)("(" + expressionSource + ")");
      } catch (error) {
        if (!(error instanceof SyntaxError)) throw error;
        return await (0, eval)("(async () => {\n" + source + "\n})()");
      }
    }
    return await (0, eval)("(async () => {\n" + source + "\n})()");
  })();
  task.catch(function () {});
  try {
    const value = await Promise.race([task, control.stoppedPromise]);
    return { logs, value: show(value), error: null };
  } catch (error) {
    return { logs, value: null, error: control.errorText(error) };
  } finally {
    for (const level of levels) console[level] = original[level];
    control.restore();
  }
})()"#;

const PY_RUNNER: &str = r#"(async () => {
  const source = __CE_SOURCE__;
  const control = __CE_CONTROL__;
  const logs = control.logs;
  const levels = ["log", "info", "warn", "error", "debug"];
  const original = {};
  const note = (level, text) => {
    if (logs.length < 200) logs.push({ level, text: String(text).slice(0, 8000) });
  };
  const show = (value) => {
    if (value === undefined || value === null) return null;
    let plain = value;
    if (value && typeof value.toJs === "function") {
      try { plain = value.toJs(); } catch (error) { plain = String(value); }
      if (typeof value.destroy === "function") value.destroy();
    }
    if (typeof plain === "string") return plain;
    if (typeof plain === "number" || typeof plain === "boolean") return String(plain);
    try { return JSON.stringify(plain); } catch (error) { return String(plain); }
  };
  const armInterrupt = (pyodide) => {
    try {
      if (pyodide.__ceInterrupt) {
        if (!control.stopped()) pyodide.__ceInterrupt[0] = 0;
        return;
      }
      if (typeof SharedArrayBuffer !== "function" || typeof pyodide.setInterruptBuffer !== "function") return;
      const buffer = new SharedArrayBuffer(1);
      const bytes = new Uint8Array(buffer);
      pyodide.setInterruptBuffer(bytes);
      pyodide.__ceInterrupt = bytes;
      const url = URL.createObjectURL(new Blob(["let bytes;self.onmessage=function(e){if(e.data&&e.data.buffer){bytes=new Uint8Array(e.data.buffer);}else if(bytes){bytes[0]=2;}};"], { type: "text/javascript" }));
      const worker = new Worker(url);
      URL.revokeObjectURL(url);
      worker.postMessage({ buffer: buffer });
      pyodide.__ceInterruptWorker = worker;
      if (control.stopped()) bytes[0] = 2;
    } catch (error) {}
  };
  for (const level of levels) {
    original[level] = console[level].bind(console);
    console[level] = (...args) => {
      note(level, args.map((item) => {
        try { return typeof item === "string" ? item : JSON.stringify(item); }
        catch (error) { return String(item); }
      }).join(" "));
      original[level](...args);
    };
  }
  try {
    if (!globalThis.__cePyodide) {
      const mod = await import("https://ce-pyodide.localhost/pyodide.mjs");
      globalThis.__cePyodide = await mod.loadPyodide({ indexURL: "https://ce-pyodide.localhost/" });
    }
    if (control.stopped()) throw new Error("Stopped");
    const pyodide = globalThis.__cePyodide;
    armInterrupt(pyodide);
    if (control.stopped()) throw new Error("Stopped");
    if (typeof pyodide.setStdout === "function") {
      pyodide.setStdout({ batched: (text) => note("log", text) });
    }
    if (typeof pyodide.setStderr === "function") {
      pyodide.setStderr({ batched: (text) => note("error", text) });
    }
    const task = pyodide.runPythonAsync(source);
    task.catch(function () {});
    const value = await Promise.race([task, control.stoppedPromise]);
    return { logs, value: show(value), error: null };
  } catch (error) {
    return { logs, value: null, error: control.errorText(error) };
  } finally {
    for (const level of levels) console[level] = original[level];
    control.restore();
  }
})()"#;

const CSS_RUNNER: &str = r#"(async () => {
  const source = __CE_SOURCE__;
  const id = "ce-scripting-css";
  let style = document.getElementById(id);
  if (!style) {
    style = document.createElement("style");
    style.id = id;
    (document.head || document.documentElement).appendChild(style);
  }
  style.textContent = source;
  return { logs: [], value: "Applied CSS", error: null };
})()"#;

const ONLOAD: &str = r#"(function () {
  var css = __CE_CSS__;
  var previous = document.querySelectorAll("style[data-ce-scripting]");
  for (var index = 0; index < previous.length; index += 1) previous[index].remove();
  for (var index = 0; index < css.length; index += 1) {
    try {
      var style = document.createElement("style");
      style.setAttribute("data-ce-scripting", css[index].name);
      style.textContent = css[index].css;
      (document.head || document.documentElement).appendChild(style);
    } catch (error) { console.error("[Scripting] " + css[index].name, error); }
  }
  var scripts = __CE_JS__;
  for (var index = 0; index < scripts.length; index += 1) {
    try { (0, eval)(scripts[index].js); }
    catch (error) { console.error("[Scripting] " + scripts[index].name, error); }
  }
  var python = __CE_PY__;
  if (!python.length) return;
  var run = async function () {
    try {
      if (!globalThis.__cePyodide) {
        var mod = await import("https://ce-pyodide.localhost/pyodide.mjs");
        globalThis.__cePyodide = await mod.loadPyodide({ indexURL: "https://ce-pyodide.localhost/" });
      }
      for (var index = 0; index < python.length; index += 1) {
        try { await globalThis.__cePyodide.runPythonAsync(python[index].source); }
        catch (error) { console.error("[Scripting] " + python[index].name, error); }
      }
    } catch (error) {
      console.error("[Scripting]", error);
    }
  };
  void run();
})();"#;

impl ScriptFile {
    fn normalize(mut self) -> Self {
        self.scripts = self
            .scripts
            .into_iter()
            .filter_map(|script| normalize_script(script).ok())
            .take(MAX_SCRIPTS)
            .collect();
        self
    }
}

fn normalize_script(mut script: PageScript) -> Result<PageScript, String> {
    script.name = script.name.trim().chars().take(MAX_NAME).collect();
    if script.name.is_empty() {
        script.name = "Script".into();
    }
    script.domain = settings::sleep_site_key(&script.domain);
    if script.domain.is_empty() {
        return Err("Choose a site for this script.".into());
    }
    script.url = crate::page_rules::page_address(&script.url);
    script.scope = canonical_scope(&script.scope, &script.url);
    if script.source.len() > MAX_CODE || script.compiled.len() > MAX_CODE {
        return Err(format!("A script can hold up to {} KB.", MAX_CODE / 1024));
    }
    script.language = canonical_language(&script.language);
    if script.language == "python" {
        script.compiled.clear();
    } else if script.language != "typescript" || script.compiled.is_empty() {
        script.compiled = script.source.clone();
    }
    script.id = script_id(&script.id, &script.name);
    Ok(script)
}

fn default_scope() -> String {
    "domain".into()
}

/// `url` only sticks when the script has an address. Anything else runs on the domain.
fn canonical_scope(scope: &str, url: &str) -> String {
    if scope.trim().eq_ignore_ascii_case("url") && !url.is_empty() {
        "url".into()
    } else {
        default_scope()
    }
}

fn canonical_language(language: &str) -> String {
    match language.trim().to_ascii_lowercase().as_str() {
        "typescript" | "ts" => "typescript".into(),
        "python" | "py" => "python".into(),
        "css" => "css".into(),
        _ => "javascript".into(),
    }
}

fn script_id(raw: &str, name: &str) -> String {
    let source = if raw.trim().is_empty() { name } else { raw };
    let mut id: String = source
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    while id.contains("--") {
        id = id.replace("--", "-");
    }
    let id = id.trim_matches('-').chars().take(40).collect::<String>();
    if id.is_empty() { "script".into() } else { id }
}

pub fn domain_matches(host: &str, domain: &str) -> bool {
    !domain.is_empty() && (host == domain || host.ends_with(&format!(".{domain}")))
}

/// A domain script matches the host and its subdomains. A url script matches that address only.
fn script_matches(script: &PageScript, url: &str) -> bool {
    let host = settings::sleep_site_key(url);
    if !domain_matches(&host, &script.domain) {
        return false;
    }
    if script.scope == "url" {
        let address = crate::page_rules::page_address(url);
        let stored = crate::page_rules::page_address(&script.url);
        !stored.is_empty() && address == stored
    } else {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{PageScript, ScriptFile, domain_matches};

    #[test]
    fn a_parent_domain_matches_its_subdomains() {
        assert!(domain_matches("news.example.com", "example.com"));
        assert!(domain_matches("example.com", "example.com"));
        assert!(!domain_matches("example.com", "news.example.com"));
        assert!(!domain_matches("notexample.com", "example.com"));
    }

    #[test]
    fn an_older_file_fills_in_the_new_fields() {
        let loaded: ScriptFile = serde_json::from_str(
            r#"{"scripts":[{"id":"prices","name":"Prices","domain":"shop.example","source":"return 1"}]}"#,
        )
        .expect("parse");
        let file = loaded.normalize();
        let script = &file.scripts[0];
        assert_eq!(script.language, "javascript");
        assert!(script.enabled);
        assert!(!script.run_on_load);
        assert_eq!(script.compiled, "return 1");
        assert_eq!(script.domain, "shop.example");
        assert!(script.url.is_empty());
        assert_eq!(script.scope, "domain");
    }

    #[test]
    fn a_url_script_matches_only_that_address() {
        let domain_script = PageScript {
            domain: "example.com".into(),
            url: "https://example.com/a".into(),
            scope: "domain".into(),
            ..PageScript::default()
        };
        let url_script = super::normalize_script(PageScript {
            domain: "example.com".into(),
            url: "https://example.com/a/#part".into(),
            scope: "url".into(),
            source: "1".into(),
            ..PageScript::default()
        })
        .expect("script");
        assert_eq!(url_script.url, "https://example.com/a");
        assert!(super::script_matches(
            &domain_script,
            "https://example.com/b"
        ));
        assert!(super::script_matches(
            &domain_script,
            "https://news.example.com/a"
        ));
        assert!(super::script_matches(&url_script, "https://example.com/a"));
        assert!(super::script_matches(&url_script, "https://example.com/a/"));
        assert!(super::script_matches(
            &url_script,
            "https://example.com/a#section"
        ));
        assert!(!super::script_matches(&url_script, "https://example.com/b"));
        assert!(!super::script_matches(&url_script, "https://other.com/a"));
    }

    #[test]
    fn a_url_scope_without_an_address_runs_on_the_domain() {
        let script = PageScript {
            domain: "example.com".into(),
            scope: "url".into(),
            source: "1".into(),
            ..PageScript::default()
        };
        let normalized = super::normalize_script(script).expect("script");
        assert_eq!(normalized.scope, "domain");
        assert!(normalized.url.is_empty());
        assert!(super::script_matches(
            &normalized,
            "https://example.com/anywhere"
        ));
    }

    #[test]
    fn python_does_not_keep_compiled_javascript() {
        let script = PageScript {
            id: "py".into(),
            name: "Title".into(),
            domain: "example.com".into(),
            url: String::new(),
            scope: "domain".into(),
            language: "python".into(),
            enabled: true,
            run_on_load: true,
            source: "print(1)".into(),
            compiled: "nope".into(),
        };
        let normalized = super::normalize_script(script).expect("script");
        assert!(normalized.compiled.is_empty());
        assert_eq!(normalized.language, "python");
    }

    #[test]
    fn css_keeps_the_stylesheet() {
        let script = PageScript {
            id: "look".into(),
            name: "Look".into(),
            domain: "example.com".into(),
            url: String::new(),
            scope: "domain".into(),
            language: "css".into(),
            enabled: true,
            run_on_load: true,
            source: "body { color: red }".into(),
            compiled: String::new(),
        };
        let normalized = super::normalize_script(script).expect("script");
        assert_eq!(normalized.language, "css");
        assert_eq!(normalized.compiled, "body { color: red }");
    }

    #[test]
    fn the_runner_keeps_a_source_that_mentions_the_control_token() {
        let program = super::javascript_runner("globalThis.__CE_CONTROL__", false);
        assert_eq!(program.matches("__CE_CONTROL__").count(), 1);
        assert!(program.contains("slot.stop"));
        assert!(!program.contains("__CE_SOURCE__"));
        assert!(!program.contains("__CE_EXPR__"));
    }

    #[test]
    fn an_unknown_language_becomes_javascript() {
        let script = PageScript {
            domain: "example.com".into(),
            language: "ruby".into(),
            source: "1".into(),
            ..PageScript::default()
        };
        let normalized = super::normalize_script(script).expect("script");
        assert_eq!(normalized.language, "javascript");
        assert_eq!(normalized.compiled, "1");
    }
}
