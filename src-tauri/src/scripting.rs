//! Per-site scripts in `scripting.json`. JavaScript and TypeScript run in the page.
//! Python runs there too, through a Pyodide runtime cached under local app data.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct PageScript {
    pub id: String,
    pub name: String,
    pub domain: String,
    /// `javascript`, `typescript`, or `python`.
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
    let host = settings::sleep_site_key(url);
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
                && domain_matches(&host, &script.domain)
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
                    && domain_matches(&host, &script.domain)
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
                && domain_matches(&host, &script.domain)
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
    fill_once(&fill_once(&fill_once(ONLOAD, "__CE_JS__", &javascript), "__CE_CSS__", &css), "__CE_PY__", &python)
}

/// Replaces the first placeholder and leaves the inserted text alone.
fn fill_once(template: &str, key: &str, value: &str) -> String {
    match template.split_once(key) {
        Some((head, tail)) => format!("{head}{value}{tail}"),
        None => template.to_string(),
    }
}

/// Runs `source` in the page of tab `tab_id`. TypeScript arrives already compiled to JavaScript.
pub fn run(
    app: &AppHandle,
    tab_id: u32,
    language: &str,
    source: &str,
    expression: bool,
) -> Result<ScriptRun, String> {
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
        ensure_pyodide(app)?;
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
    let value = webview2::evaluate(&webview, &program, timeout, "The script")?;
    Ok(parse_run(value))
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
    let dir = cache_dir(app);
    fs::create_dir_all(&dir).map_err(|error| {
        format!(
            "Could not create the Python runtime folder {}: {error}",
            dir.display()
        )
    })?;
    for name in PYODIDE_FILES {
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
    io::copy(&mut response.into_reader(), &mut file).map_err(|error| {
        let _ = fs::remove_file(&temp);
        format!("Could not download the Python runtime: {error}")
    })?;
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
        .replacen("__CE_EXPR__", if expression { "true" } else { "false" }, 1)
        .replacen("__CE_SOURCE__", &encoded, 1)
}

fn python_runner(source: &str) -> String {
    let encoded = serde_json::to_string(source).unwrap_or_else(|_| "\"\"".into());
    PY_RUNNER.replacen("__CE_SOURCE__", &encoded, 1)
}

fn css_runner(source: &str) -> String {
    let encoded = serde_json::to_string(source).unwrap_or_else(|_| "\"\"".into());
    CSS_RUNNER.replacen("__CE_SOURCE__", &encoded, 1)
}

const JS_RUNNER: &str = r#"(async () => {
  const expression = __CE_EXPR__;
  const source = __CE_SOURCE__;
  const logs = [];
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
  try {
    let value;
    if (expression) {
      const expressionSource = source.trim().replace(/;+\s*$/, "");
      try {
        value = await (0, eval)("(" + expressionSource + ")");
      } catch (error) {
        if (!(error instanceof SyntaxError)) throw error;
        value = await (0, eval)("(async () => {\n" + source + "\n})()");
      }
    } else {
      value = await (0, eval)("(async () => {\n" + source + "\n})()");
    }
    return { logs, value: show(value), error: null };
  } catch (error) {
    const text = error && error.stack ? String(error.stack) : String(error);
    return { logs, value: null, error: text.slice(0, 8000) };
  } finally {
    for (const level of levels) console[level] = original[level];
  }
})()"#;

const PY_RUNNER: &str = r#"(async () => {
  const source = __CE_SOURCE__;
  const logs = [];
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
    const pyodide = globalThis.__cePyodide;
    if (typeof pyodide.setStdout === "function") {
      pyodide.setStdout({ batched: (text) => note("log", text) });
    }
    if (typeof pyodide.setStderr === "function") {
      pyodide.setStderr({ batched: (text) => note("error", text) });
    }
    const value = await pyodide.runPythonAsync(source);
    return { logs, value: show(value), error: null };
  } catch (error) {
    const text = error && error.stack ? String(error.stack) : String(error);
    return { logs, value: null, error: text.slice(0, 8000) };
  } finally {
    for (const level of levels) console[level] = original[level];
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
    }

    #[test]
    fn python_does_not_keep_compiled_javascript() {
        let script = PageScript {
            id: "py".into(),
            name: "Title".into(),
            domain: "example.com".into(),
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
