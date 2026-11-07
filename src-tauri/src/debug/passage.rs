//! Passage: a dev-only MCP and HTTP server that lets a coding agent inspect and drive the
//! running app. It reads the debug feed, runs scripts and real input in any webview, calls
//! commands through the browser UI and takes screenshots.
//!
//! It listens on 127.0.0.1 only and refuses requests from web pages. There is no token, so
//! any local process can use it while a dev build runs. Release builds never start it.

use std::io::Read;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine;
use serde_json::{Value, json};
use tauri::{AppHandle, Manager, Webview};
use tiny_http::{Method, Request, Server};

use super::{DebugIpcRecord, DebugLogLine, webviews};
use crate::automation::driver;
use crate::automation::mcp::{header, origin_allowed, reply, rpc_error, rpc_result};
use crate::{capture, profiles, settings, tabs, webview2};

const DEFAULT_PORT: u16 = 1427;
const PORT_ENV: &str = "CE_PASSAGE_PORT";
const PROTOCOL_VERSION: &str = "2025-06-18";
const BODY_LIMIT: u64 = 4_000_000;
const FAILURE: &str = "Passage:";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
/// Screenshots kept in `src-tauri/target/passage`. Older ones are deleted.
const KEEP_SHOTS: usize = 40;
/// Widest image sent back inline over MCP. The saved PNG keeps full size.
const INLINE_WIDTH: u32 = 1600;

static STARTED_AT: OnceLock<f64> = OnceLock::new();
static PORT: OnceLock<u16> = OnceLock::new();

pub fn start(app: &AppHandle) {
    let port = std::env::var(PORT_ENV)
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(DEFAULT_PORT);
    let _ = STARTED_AT.set(super::now_ms());
    let server = match Server::http(("127.0.0.1", port)) {
        Ok(server) => server,
        Err(error) => {
            eprintln!("passage: couldn't listen on port {port}: {error}");
            return;
        }
    };
    let _ = PORT.set(port);
    eprintln!("passage: listening on http://127.0.0.1:{port}/mcp");
    let app = app.clone();
    let _ = thread::Builder::new()
        .name("passage".into())
        .spawn(move || {
            for request in server.incoming_requests() {
                let app = app.clone();
                thread::spawn(move || handle(&app, request));
            }
        });
}

// HTTP

enum Route {
    Mcp,
    Health,
    Call(String),
    Unknown,
}

fn route_of(url: &str) -> Route {
    let path = url.split('?').next().unwrap_or("").trim_end_matches('/');
    match path {
        "" | "/mcp" => Route::Mcp,
        "/health" => Route::Health,
        _ => match path.strip_prefix("/call/") {
            Some(tool) if !tool.is_empty() && !tool.contains('/') => Route::Call(tool.to_string()),
            _ => Route::Unknown,
        },
    }
}

/// Browsers can only send `application/json` after a CORS preflight, which is never answered.
fn json_content(content_type: Option<&str>) -> bool {
    content_type.is_some_and(|value| {
        value
            .split(';')
            .next()
            .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("application/json"))
    })
}

fn handle(app: &AppHandle, mut request: Request) {
    if !origin_allowed(header(&request, "Origin").as_deref()) {
        return reply(request, 403, Some(json!({ "error": "Web pages can't use passage." })));
    }
    let route = route_of(request.url());
    if let Route::Health = route {
        return reply(request, 200, Some(health()));
    }
    if let Route::Unknown = route {
        return reply(
            request,
            404,
            Some(json!({ "error": "Not found. Use /mcp, /health or POST /call/<tool>." })),
        );
    }
    match request.method() {
        Method::Post => {}
        Method::Delete if matches!(route, Route::Mcp) => return reply(request, 200, None),
        _ => return reply(request, 405, None),
    }
    if !json_content(header(&request, "Content-Type").as_deref()) {
        return reply(
            request,
            415,
            Some(json!({ "error": "Send Content-Type: application/json." })),
        );
    }
    let mut body = String::new();
    if request
        .as_reader()
        .take(BODY_LIMIT)
        .read_to_string(&mut body)
        .is_err()
    {
        return reply(request, 400, Some(json!({ "error": "The body is not UTF-8 text." })));
    }
    let parsed: Value = if body.trim().is_empty() {
        json!({})
    } else {
        match serde_json::from_str(&body) {
            Ok(value) => value,
            Err(error) => {
                return reply(request, 400, Some(json!({ "error": format!("Parse error: {error}") })));
            }
        }
    };
    match route {
        Route::Call(tool) => match run(app, &tool, &parsed) {
            Ok(output) => reply(request, 200, Some(json!({ "ok": true, "result": output.value }))),
            Err(error) => reply(request, 400, Some(json!({ "ok": false, "error": error }))),
        },
        _ => match parsed {
            Value::Array(batch) => {
                let answers: Vec<Value> = batch
                    .iter()
                    .filter_map(|message| dispatch(app, message))
                    .collect();
                if answers.is_empty() {
                    reply(request, 202, None)
                } else {
                    reply(request, 200, Some(Value::Array(answers)))
                }
            }
            message => match dispatch(app, &message) {
                Some(answer) => reply(request, 200, Some(answer)),
                None => reply(request, 202, None),
            },
        },
    }
}

fn health() -> Value {
    json!({
        "ok": true,
        "pid": std::process::id(),
        "startedAt": STARTED_AT.get().copied().unwrap_or(0.0),
        "version": env!("CARGO_PKG_VERSION"),
        "port": PORT.get().copied().unwrap_or(DEFAULT_PORT),
    })
}

// MCP

const INSTRUCTIONS: &str = "Passage inspects and drives the running ce-browser dev build. \
Webviews are addressed by label: `ui` is the React browser UI, `tab-<id>` a page, `popup-<window>-<n>` a popup. \
Read `logs` (backend stdout/stderr and frontend console errors) and `ipc` after reproducing a problem. \
Use `query` and `eval` to inspect the DOM, `click`/`type`/`press` for real input, `invoke` to call Tauri commands, \
and `screenshot` to see the window. Pass `sinceId` from a previous `logs` result to read only new lines.";

/// One JSON-RPC message. Notifications get no answer.
fn dispatch(app: &AppHandle, message: &Value) -> Option<Value> {
    let id = message.get("id").cloned()?;
    let method = message.get("method").and_then(Value::as_str).unwrap_or("");
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    Some(match method {
        "initialize" => {
            let requested = params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or(PROTOCOL_VERSION);
            rpc_result(
                id,
                json!({
                    "protocolVersion": requested,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": { "name": "ce-passage", "version": env!("CARGO_PKG_VERSION") },
                    "instructions": INSTRUCTIONS,
                }),
            )
        }
        "ping" => rpc_result(id, json!({})),
        "tools/list" => rpc_result(id, json!({ "tools": tool_list() })),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            let result = match run(app, name, &args) {
                Ok(output) => json!({ "content": output.content(), "isError": false }),
                Err(error) => {
                    json!({ "content": [{ "type": "text", "text": error }], "isError": true })
                }
            };
            rpc_result(id, result)
        }
        "resources/list" => rpc_result(id, json!({ "resources": [] })),
        "prompts/list" => rpc_result(id, json!({ "prompts": [] })),
        _ => rpc_error(id, -32601, &format!("Unknown method \"{method}\".")),
    })
}

fn schema(properties: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": properties, "required": required })
}

fn tool_list() -> Vec<Value> {
    let label = json!({ "type": "string", "description": "Webview label, such as ui, tab-3 or popup-main-0. Defaults to ui." });
    let selector = json!({ "type": "string", "description": "CSS selector. The first match is used." });
    let timeout = json!({ "type": "number", "description": "Seconds to wait. Defaults to 10." });
    let tools = [
        ("status", "Process id, start time, version, profile, data folders and browser windows.", schema(json!({}), &[])),
        ("webviews", "Every native webview with its kind, address, bounds and whether it is parked off screen.", schema(json!({}), &[])),
        ("tabs", "The full tab snapshot: tabs, groups, windows and workspaces.", schema(json!({}), &[])),
        ("settings", "The active profile's settings.", schema(json!({}), &[])),
        (
            "logs",
            "Backend stdout and stderr and frontend console errors and warnings, oldest first. Returns nextId; pass it as sinceId to read only newer lines.",
            schema(
                json!({
                    "sinceId": { "type": "integer" },
                    "stream": { "type": "string", "enum": ["stdout", "stderr", "console"] },
                    "area": { "type": "string", "description": "Text the line's area contains, such as tabs or ui error." },
                    "contains": { "type": "string", "description": "Text the line contains. Not case sensitive." },
                    "limit": { "type": "integer", "description": "Newest lines returned. Defaults to 200." }
                }),
                &[],
            ),
        ),
        (
            "ipc",
            "Commands and events recorded from the frontend webviews, oldest first. Returns nextId.",
            schema(
                json!({
                    "sinceId": { "type": "integer" },
                    "name": { "type": "string", "description": "Text the command or event name contains." },
                    "webview": { "type": "string" },
                    "failedOnly": { "type": "boolean" },
                    "limit": { "type": "integer", "description": "Newest records returned. Defaults to 100." }
                }),
                &[],
            ),
        ),
        (
            "eval",
            "Run a JavaScript expression in a webview and return its JSON value. Promises are awaited.",
            schema(json!({ "label": label, "expression": { "type": "string" }, "timeoutSeconds": timeout }), &["expression"]),
        ),
        (
            "query",
            "Elements matching a CSS selector with their id, tag, classes, text, CSS-pixel box and visibility.",
            schema(
                json!({
                    "label": label,
                    "selector": { "type": "string" },
                    "styles": { "type": "array", "items": { "type": "string" }, "description": "Computed style properties to include, such as color or z-index." },
                    "html": { "type": "boolean", "description": "Include outerHTML, cut to 2000 characters." },
                    "limit": { "type": "integer", "description": "Elements returned. Defaults to 20." }
                }),
                &["selector"],
            ),
        ),
        (
            "click",
            "A real mouse click on an element, or at x and y in CSS pixels.",
            schema(
                json!({
                    "label": label,
                    "selector": selector,
                    "x": { "type": "number" },
                    "y": { "type": "number" },
                    "button": { "type": "string", "enum": ["left", "right", "middle"] },
                    "double": { "type": "boolean" }
                }),
                &[],
            ),
        ),
        (
            "type",
            "Type text with real key input. With a selector, clicks that element first and replaces its value unless clear is false.",
            schema(
                json!({
                    "label": label,
                    "selector": selector,
                    "text": { "type": "string" },
                    "clear": { "type": "boolean" },
                    "submit": { "type": "boolean", "description": "Press Enter after typing." }
                }),
                &["text"],
            ),
        ),
        (
            "press",
            "Press a key or chord, such as Enter, Escape, Control+L or Shift+Tab.",
            schema(json!({ "label": label, "keys": { "type": "string" } }), &["keys"]),
        ),
        (
            "invoke",
            "Call a Tauri command through the browser UI webview, as the frontend would. Recorded in the ipc feed.",
            schema(
                json!({
                    "command": { "type": "string" },
                    "args": { "type": "object", "description": "Arguments with camelCase names." },
                    "label": { "type": "string", "description": "Webview to call from. Defaults to ui." },
                    "timeoutSeconds": timeout
                }),
                &["command"],
            ),
        ),
        (
            "screenshot",
            "A PNG saved under src-tauri/target/passage. Without webview: the browser window (mode window), or what the screen shows over it including popups (mode screen). With webview: that webview only.",
            schema(
                json!({
                    "webview": { "type": "string", "description": "Capture only this webview." },
                    "window": { "type": "string", "description": "Browser window label. Defaults to the focused one." },
                    "mode": { "type": "string", "enum": ["window", "screen"] }
                }),
                &[],
            ),
        ),
        ("reload", "Reload a webview.", schema(json!({ "label": label }), &[])),
        ("devtools", "Open DevTools for a webview.", schema(json!({ "label": label }), &[])),
        (
            "wait",
            "Wait until a selector appears (or is gone) in a webview, or until a log line contains text.",
            schema(
                json!({
                    "label": label,
                    "selector": { "type": "string" },
                    "gone": { "type": "boolean" },
                    "log": { "type": "string", "description": "Text a new log line contains." },
                    "sinceId": { "type": "integer", "description": "For log: lines from this id on. Defaults to lines after the call starts." },
                    "timeoutSeconds": timeout
                }),
                &[],
            ),
        ),
        (
            "clear",
            "Clear the logs or ipc feed.",
            schema(json!({ "target": { "type": "string", "enum": ["logs", "ipc"] } }), &["target"]),
        ),
    ];
    tools
        .into_iter()
        .map(|(name, description, input)| {
            json!({ "name": name, "description": description, "inputSchema": input })
        })
        .collect()
}

// Tools

struct Output {
    value: Value,
    /// Inline JPEG for MCP clients.
    image: Option<Vec<u8>>,
}

impl Output {
    fn json(value: Value) -> Self {
        Self { value, image: None }
    }

    fn content(&self) -> Vec<Value> {
        let mut content = Vec::new();
        if let Some(image) = &self.image {
            let data = base64::engine::general_purpose::STANDARD.encode(image);
            content.push(json!({ "type": "image", "data": data, "mimeType": "image/jpeg" }));
        }
        let text = match &self.value {
            Value::String(text) => text.clone(),
            other => serde_json::to_string_pretty(other).unwrap_or_default(),
        };
        content.push(json!({ "type": "text", "text": text }));
        content
    }
}

fn arg_str(args: &Value, key: &str) -> String {
    args.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

fn arg_bool(args: &Value, key: &str, fallback: bool) -> bool {
    args.get(key).and_then(Value::as_bool).unwrap_or(fallback)
}

fn arg_f64(args: &Value, key: &str) -> Option<f64> {
    args.get(key)
        .and_then(|value| value.as_f64().or_else(|| value.as_str().and_then(|s| s.parse().ok())))
}

fn arg_u64(args: &Value, key: &str) -> Option<u64> {
    arg_f64(args, key).filter(|value| *value >= 0.0).map(|value| value as u64)
}

fn arg_label(args: &Value) -> String {
    let label = arg_str(args, "label");
    if label.trim().is_empty() { "ui".into() } else { label.trim().to_string() }
}

fn arg_timeout(args: &Value) -> Duration {
    arg_f64(args, "timeoutSeconds")
        .map(|seconds| Duration::from_secs_f64(seconds.clamp(0.5, 120.0)))
        .unwrap_or(DEFAULT_TIMEOUT)
}

fn to_json(value: impl serde::Serialize) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|error| error.to_string())
}

fn run(app: &AppHandle, name: &str, args: &Value) -> Result<Output, String> {
    match name {
        "status" => Ok(Output::json(status(app)?)),
        "webviews" => Ok(Output::json(to_json(webviews::list(app))?)),
        "tabs" => Ok(Output::json(to_json(tabs::snapshot(app))?)),
        "settings" => Ok(Output::json(to_json(settings::get(app))?)),
        "logs" => Ok(Output::json(logs(args))),
        "ipc" => Ok(Output::json(ipc(args))),
        "eval" => {
            let expression = arg_str(args, "expression");
            if expression.trim().is_empty() {
                return Err("Give an expression.".into());
            }
            let webview = webview_of(app, &arg_label(args))?;
            Ok(Output::json(evaluate(&webview, &expression, arg_timeout(args))?))
        }
        "query" => query(app, args).map(Output::json),
        "click" => click(app, args).map(Output::json),
        "type" => type_text(app, args).map(Output::json),
        "press" => {
            let webview = webview_of(app, &arg_label(args))?;
            press(&webview, &arg_str(args, "keys"))?;
            Ok(Output::json(json!("Pressed.")))
        }
        "invoke" => invoke(app, args).map(Output::json),
        "screenshot" => screenshot(app, args),
        "reload" => {
            super::reload_webview(app, &arg_label(args))?;
            Ok(Output::json(json!("Reloading.")))
        }
        "devtools" => {
            super::open_devtools(app, &arg_label(args))?;
            Ok(Output::json(json!("DevTools opened.")))
        }
        "wait" => wait(app, args).map(Output::json),
        "clear" => {
            super::clear(app, &arg_str(args, "target"))?;
            Ok(Output::json(json!("Cleared.")))
        }
        other => Err(format!("There is no tool named \"{other}\".")),
    }
}

fn status(app: &AppHandle) -> Result<Value, String> {
    let mut value = health();
    value["profile"] = json!(profiles::active_id());
    value["paths"] = to_json(super::debug_paths_of(app))?;
    value["windows"] = json!(tabs::browser_labels(app));
    value["focusedWindow"] = json!(tabs::focused_window(app));
    value["screenshots"] = json!(shots_dir().display().to_string());
    Ok(value)
}

/// A page view is created for a sleeping tab, so its label always resolves.
fn webview_of(app: &AppHandle, label: &str) -> Result<Webview, String> {
    if let Some(id) = webviews::tab_id(label) {
        return driver::view(app, id);
    }
    app.get_webview(label).ok_or_else(|| {
        format!("No webview is labelled \"{label}\". Call webviews to list them.")
    })
}

fn evaluate(webview: &Webview, expression: &str, timeout: Duration) -> Result<Value, String> {
    webview2::evaluate(webview, expression, timeout, FAILURE)
}

fn cdp(webview: &Webview, method: &str, params: Value) -> Result<Value, String> {
    webview2::call_cdp_json(webview, method, &params.to_string(), DEFAULT_TIMEOUT, FAILURE)
}

// Feed

struct LogFilter {
    since_id: u64,
    stream: String,
    area: String,
    contains: String,
    limit: usize,
}

fn filter_logs(lines: Vec<DebugLogLine>, filter: &LogFilter) -> Vec<DebugLogLine> {
    let area = filter.area.to_lowercase();
    let needle = filter.contains.to_lowercase();
    let mut matches: Vec<DebugLogLine> = lines
        .into_iter()
        .filter(|line| line.id >= filter.since_id)
        .filter(|line| filter.stream.is_empty() || line.stream == filter.stream)
        .filter(|line| area.is_empty() || line.area.to_lowercase().contains(&area))
        .filter(|line| needle.is_empty() || line.text.to_lowercase().contains(&needle))
        .collect();
    let skip = matches.len().saturating_sub(filter.limit);
    matches.drain(..skip);
    matches
}

fn logs(args: &Value) -> Value {
    let filter = LogFilter {
        since_id: arg_u64(args, "sinceId").unwrap_or(0),
        stream: arg_str(args, "stream"),
        area: arg_str(args, "area"),
        contains: arg_str(args, "contains"),
        limit: arg_u64(args, "limit").unwrap_or(200).clamp(1, 2000) as usize,
    };
    let next_id = super::next_id();
    let lines = filter_logs(super::log_lines(), &filter);
    json!({ "nextId": next_id, "lines": lines })
}

struct IpcFilter {
    since_id: u64,
    name: String,
    webview: String,
    failed_only: bool,
    limit: usize,
}

fn filter_ipc(records: Vec<DebugIpcRecord>, filter: &IpcFilter) -> Vec<DebugIpcRecord> {
    let name = filter.name.to_lowercase();
    let mut matches: Vec<DebugIpcRecord> = records
        .into_iter()
        .filter(|record| record.id >= filter.since_id)
        .filter(|record| name.is_empty() || record.name.to_lowercase().contains(&name))
        .filter(|record| filter.webview.is_empty() || record.webview == filter.webview)
        .filter(|record| !filter.failed_only || !record.ok)
        .collect();
    let skip = matches.len().saturating_sub(filter.limit);
    matches.drain(..skip);
    matches
}

fn ipc(args: &Value) -> Value {
    let filter = IpcFilter {
        since_id: arg_u64(args, "sinceId").unwrap_or(0),
        name: arg_str(args, "name"),
        webview: arg_str(args, "webview"),
        failed_only: arg_bool(args, "failedOnly", false),
        limit: arg_u64(args, "limit").unwrap_or(100).clamp(1, 2000) as usize,
    };
    let next_id = super::next_id();
    let records = filter_ipc(super::ipc_records(), &filter);
    json!({ "nextId": next_id, "records": records })
}

// DOM and input

const QUERY_SCRIPT: &str = r#"((selector, styles, html, limit) => {
  const all = Array.from(document.querySelectorAll(selector));
  const elements = all.slice(0, limit).map((el) => {
    const r = el.getBoundingClientRect();
    const cs = getComputedStyle(el);
    const item = {
      id: el.id || "",
      tag: el.tagName.toLowerCase(),
      classes: el.getAttribute("class") || "",
      text: ((el.innerText ?? el.textContent) || "").trim().slice(0, 200),
      rect: { x: Math.round(r.x), y: Math.round(r.y), width: Math.round(r.width), height: Math.round(r.height) },
      visible: r.width > 0 && r.height > 0 && cs.visibility !== "hidden" && cs.display !== "none" && cs.opacity !== "0",
    };
    if (styles.length) item.styles = Object.fromEntries(styles.map((name) => [name, cs.getPropertyValue(name)]));
    if (html) item.html = el.outerHTML.slice(0, 2000);
    return item;
  });
  return { count: all.length, elements };
})"#;

fn query(app: &AppHandle, args: &Value) -> Result<Value, String> {
    let selector = arg_str(args, "selector");
    if selector.trim().is_empty() {
        return Err("Give a selector.".into());
    }
    let styles: Vec<String> = match args.get("styles") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.as_str().map(String::from))
            .collect(),
        _ => Vec::new(),
    };
    let limit = arg_u64(args, "limit").unwrap_or(20).clamp(1, 500);
    let expression = format!(
        "{QUERY_SCRIPT}({}, {}, {}, {limit})",
        json!(selector),
        json!(styles),
        arg_bool(args, "html", false)
    );
    let webview = webview_of(app, &arg_label(args))?;
    evaluate(&webview, &expression, DEFAULT_TIMEOUT)
}

/// The centre of the first element `selector` matches, in CSS pixels, after scrolling to it.
fn center_of(webview: &Webview, selector: &str) -> Result<(f64, f64), String> {
    let expression = format!(
        r#"((selector) => {{
  const el = document.querySelector(selector);
  if (!el) return null;
  el.scrollIntoView({{ block: "center", inline: "center" }});
  const r = el.getBoundingClientRect();
  return [r.x + r.width / 2, r.y + r.height / 2];
}})({})"#,
        json!(selector)
    );
    let value = evaluate(webview, &expression, DEFAULT_TIMEOUT)?;
    match (value.get(0).and_then(Value::as_f64), value.get(1).and_then(Value::as_f64)) {
        (Some(x), Some(y)) => Ok((x, y)),
        _ => Err(format!("Nothing matches \"{selector}\".")),
    }
}

fn mouse_click(webview: &Webview, x: f64, y: f64, button: &str, double: bool) -> Result<(), String> {
    let button = match button {
        "right" | "middle" => button,
        _ => "left",
    };
    cdp(webview, "Input.dispatchMouseEvent", json!({ "type": "mouseMoved", "x": x, "y": y }))?;
    let clicks = if double { 2 } else { 1 };
    for count in 1..=clicks {
        for kind in ["mousePressed", "mouseReleased"] {
            cdp(
                webview,
                "Input.dispatchMouseEvent",
                json!({ "type": kind, "x": x, "y": y, "button": button, "clickCount": count }),
            )?;
        }
    }
    Ok(())
}

fn click(app: &AppHandle, args: &Value) -> Result<Value, String> {
    let webview = webview_of(app, &arg_label(args))?;
    let selector = arg_str(args, "selector");
    let (x, y) = if !selector.trim().is_empty() {
        center_of(&webview, &selector)?
    } else {
        match (arg_f64(args, "x"), arg_f64(args, "y")) {
            (Some(x), Some(y)) => (x, y),
            _ => return Err("Give a selector, or x and y.".into()),
        }
    };
    mouse_click(&webview, x, y, &arg_str(args, "button"), arg_bool(args, "double", false))?;
    Ok(json!({ "clicked": [x.round(), y.round()] }))
}

fn press(webview: &Webview, keys: &str) -> Result<(), String> {
    let (down, up) = driver::key_events(keys)?;
    cdp(webview, "Input.dispatchKeyEvent", down)?;
    cdp(webview, "Input.dispatchKeyEvent", up)?;
    Ok(())
}

fn type_text(app: &AppHandle, args: &Value) -> Result<Value, String> {
    let webview = webview_of(app, &arg_label(args))?;
    let selector = arg_str(args, "selector");
    if !selector.trim().is_empty() {
        let (x, y) = center_of(&webview, &selector)?;
        mouse_click(&webview, x, y, "left", false)?;
        thread::sleep(Duration::from_millis(60));
        if arg_bool(args, "clear", true) {
            press(&webview, "Control+A")?;
        }
    }
    let text = arg_str(args, "text");
    if !text.is_empty() {
        cdp(&webview, "Input.insertText", json!({ "text": text }))?;
    }
    if arg_bool(args, "submit", false) {
        thread::sleep(Duration::from_millis(60));
        press(&webview, "Enter")?;
    }
    Ok(json!("Typed."))
}

// Commands

fn invoke_script(command: &str, args: &Value) -> String {
    format!(
        r#"(async (command, args) => {{
  const started = performance.now();
  const describe = (error) => {{
    if (typeof error === "string") return error;
    if (error instanceof Error) return error.message;
    try {{ return JSON.stringify(error); }} catch {{ return String(error); }}
  }};
  try {{
    const value = await window.__TAURI_INTERNALS__.invoke(command, args);
    return {{ ok: true, value: value === undefined ? null : value, ms: performance.now() - started }};
  }} catch (error) {{
    return {{ ok: false, error: describe(error), ms: performance.now() - started }};
  }}
}})({}, {})"#,
        json!(command),
        args
    )
}

fn preview(value: &Value) -> (String, u64) {
    const LIMIT: usize = 2048;
    let text = value.to_string();
    let bytes = text.len() as u64;
    if text.len() <= LIMIT {
        return (text, bytes);
    }
    let mut end = LIMIT;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (format!("{}…", &text[..end]), bytes)
}

fn invoke(app: &AppHandle, args: &Value) -> Result<Value, String> {
    let command = arg_str(args, "command");
    if command.trim().is_empty() {
        return Err("Give a command name.".into());
    }
    let call_args = match args.get("args") {
        Some(value @ Value::Object(_)) => value.clone(),
        _ => json!({}),
    };
    let label = arg_label(args);
    let webview = webview_of(app, &label)?;
    let started_at = super::now_ms();
    let answer = evaluate(&webview, &invoke_script(&command, &call_args), arg_timeout(args))?;
    let ok = answer.get("ok").and_then(Value::as_bool).unwrap_or(false);
    let value = answer.get("value").cloned().unwrap_or(Value::Null);
    let error = answer.get("error").and_then(Value::as_str).unwrap_or("").to_string();
    let (args_preview, _) = preview(&call_args);
    let (result_preview, bytes) = if ok { preview(&value) } else { (String::new(), 0) };
    super::push_ipc(vec![DebugIpcRecord {
        webview: format!("passage via {label}"),
        kind: "invoke".into(),
        name: command,
        started_at,
        duration_ms: answer.get("ms").and_then(Value::as_f64),
        ok,
        args_preview,
        result_preview,
        error: error.clone(),
        bytes,
        ..DebugIpcRecord::default()
    }]);
    if ok { Ok(value) } else { Err(error) }
}

// Screenshots

fn shots_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("passage")
}

fn shot_name(at_ms: u64, target: &str) -> String {
    let target: String = target
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() || ch == '-' { ch } else { '-' })
        .collect();
    format!("{at_ms}-{target}.png")
}

/// Deletes the oldest screenshots so at most `KEEP_SHOTS` remain.
fn prune_shots(dir: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut shots: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "png"))
        .collect();
    if shots.len() <= KEEP_SHOTS {
        return;
    }
    shots.sort();
    for path in &shots[..shots.len() - KEEP_SHOTS] {
        let _ = std::fs::remove_file(path);
    }
}

fn screenshot(app: &AppHandle, args: &Value) -> Result<Output, String> {
    let webview_label = arg_str(args, "webview");
    let (target, png) = if !webview_label.trim().is_empty() {
        let label = webview_label.trim().to_string();
        let webview = webview_of(app, &label)?;
        let shot = cdp(
            &webview,
            "Page.captureScreenshot",
            json!({ "format": "png", "captureBeyondViewport": false }),
        )?;
        let data = shot
            .get("data")
            .and_then(Value::as_str)
            .ok_or("The webview returned no screenshot.")?;
        let png = base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|error| error.to_string())?;
        (label, png)
    } else {
        let window = match arg_str(args, "window") {
            label if label.trim().is_empty() => tabs::focused_window(app),
            label => label.trim().to_string(),
        };
        let handle = app
            .get_window(&window)
            .ok_or_else(|| format!("No window is labelled \"{window}\"."))?;
        let hwnd = handle.hwnd().map_err(|error| error.to_string())?.0 as isize;
        let mode = arg_str(args, "mode");
        let image = if mode == "screen" {
            capture::screen_image(hwnd)?
        } else {
            capture::window_image(hwnd)?
        };
        let target = if mode == "screen" { format!("{window}-screen") } else { window };
        (target, capture::encode_png(&image)?)
    };
    let dir = shots_dir();
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let path = dir.join(shot_name(super::now_ms() as u64, &target));
    std::fs::write(&path, &png).map_err(|error| error.to_string())?;
    prune_shots(&dir);
    let inline = driver::shrink_jpeg(&png, INLINE_WIDTH, 80)?;
    let size = image::load_from_memory(&png)
        .map(|image| (image.width(), image.height()))
        .unwrap_or((inline.width, inline.height));
    let shown = path.canonicalize().unwrap_or(path).display().to_string();
    let shown = shown.trim_start_matches(r"\\?\").to_string();
    Ok(Output {
        value: json!({ "path": shown, "width": size.0, "height": size.1 }),
        image: Some(inline.bytes),
    })
}

// Waiting

fn wait(app: &AppHandle, args: &Value) -> Result<Value, String> {
    let limit = arg_timeout(args);
    let started = Instant::now();
    let log = arg_str(args, "log");
    if !log.trim().is_empty() {
        let since = arg_u64(args, "sinceId").unwrap_or_else(super::next_id);
        let needle = log.to_lowercase();
        while started.elapsed() < limit {
            if let Some(line) = super::log_lines()
                .into_iter()
                .find(|line| line.id >= since && line.text.to_lowercase().contains(&needle))
            {
                return Ok(json!({ "met": true, "line": line }));
            }
            thread::sleep(Duration::from_millis(200));
        }
        return Ok(json!({ "met": false }));
    }
    let selector = arg_str(args, "selector");
    if selector.trim().is_empty() {
        return Err("Give a selector or log text.".into());
    }
    let gone = arg_bool(args, "gone", false);
    let webview = webview_of(app, &arg_label(args))?;
    let expression = format!("!!document.querySelector({})", json!(selector));
    while started.elapsed() < limit {
        // A reloading page fails the script; keep waiting.
        if let Ok(found) = evaluate(&webview, &expression, DEFAULT_TIMEOUT)
            && found.as_bool() == Some(!gone)
        {
            return Ok(json!({ "met": true }));
        }
        thread::sleep(Duration::from_millis(200));
    }
    Ok(json!({ "met": false }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(id: u64, stream: &str, area: &str, text: &str) -> DebugLogLine {
        DebugLogLine {
            id,
            at: 0.0,
            stream: stream.into(),
            area: area.into(),
            text: text.into(),
        }
    }

    #[test]
    fn routes() {
        assert!(matches!(route_of("/mcp"), Route::Mcp));
        assert!(matches!(route_of("/"), Route::Mcp));
        assert!(matches!(route_of("/health?x=1"), Route::Health));
        assert!(matches!(route_of("/call/logs"), Route::Call(tool) if tool == "logs"));
        assert!(matches!(route_of("/call/"), Route::Unknown));
        assert!(matches!(route_of("/call/a/b"), Route::Unknown));
        assert!(matches!(route_of("/other"), Route::Unknown));
    }

    #[test]
    fn only_json_bodies_are_taken() {
        assert!(json_content(Some("application/json")));
        assert!(json_content(Some("Application/JSON; charset=utf-8")));
        assert!(!json_content(Some("text/plain")));
        assert!(!json_content(Some("application/x-www-form-urlencoded")));
        assert!(!json_content(None));
    }

    #[test]
    fn web_pages_are_refused() {
        assert!(origin_allowed(None));
        assert!(origin_allowed(Some("http://127.0.0.1:1427")));
        assert!(!origin_allowed(Some("https://example.com")));
        assert!(!origin_allowed(Some("http://tauri.localhost")));
    }

    #[test]
    fn every_tool_is_listed_and_runs() {
        let names: Vec<String> = tool_list()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap().to_string())
            .collect();
        for name in [
            "status", "webviews", "tabs", "settings", "logs", "ipc", "eval", "query", "click",
            "type", "press", "invoke", "screenshot", "reload", "devtools", "wait", "clear",
        ] {
            assert!(names.contains(&name.to_string()), "{name} is missing");
        }
        assert_eq!(names.len(), 17);
    }

    #[test]
    fn logs_filter_by_id_stream_area_and_text() {
        let lines = vec![
            line(1, "stderr", "tabs", "tabs: one"),
            line(2, "stdout", "", "plain two"),
            line(3, "console", "ui error", "ui error: Boom"),
            line(4, "stderr", "tabs", "tabs: four"),
        ];
        let all = LogFilter {
            since_id: 0,
            stream: String::new(),
            area: String::new(),
            contains: String::new(),
            limit: 200,
        };
        assert_eq!(filter_logs(lines.clone(), &all).len(), 4);
        let since = LogFilter { since_id: 3, ..all };
        assert_eq!(filter_logs(lines.clone(), &since).iter().map(|l| l.id).collect::<Vec<_>>(), [3, 4]);
        let stream = LogFilter { since_id: 0, stream: "console".into(), ..since };
        assert_eq!(filter_logs(lines.clone(), &stream)[0].id, 3);
        let area = LogFilter { stream: String::new(), area: "TABS".into(), ..stream };
        assert_eq!(filter_logs(lines.clone(), &area).len(), 2);
        let text = LogFilter { area: String::new(), contains: "boom".into(), ..area };
        assert_eq!(filter_logs(lines.clone(), &text)[0].id, 3);
        let newest = LogFilter { contains: String::new(), limit: 1, ..text };
        assert_eq!(filter_logs(lines, &newest)[0].id, 4);
    }

    #[test]
    fn ipc_filters_failures_and_names() {
        let record = |id: u64, name: &str, ok: bool| DebugIpcRecord {
            id,
            webview: "ui".into(),
            kind: "invoke".into(),
            name: name.into(),
            ok,
            ..DebugIpcRecord::default()
        };
        let records = vec![record(1, "list_tabs", true), record(2, "navigate", false), record(3, "list_tabs", false)];
        let filter = IpcFilter {
            since_id: 0,
            name: String::new(),
            webview: String::new(),
            failed_only: true,
            limit: 100,
        };
        assert_eq!(filter_ipc(records.clone(), &filter).iter().map(|r| r.id).collect::<Vec<_>>(), [2, 3]);
        let named = IpcFilter { name: "LIST".into(), ..filter };
        assert_eq!(filter_ipc(records.clone(), &named)[0].id, 3);
        let other = IpcFilter { webview: "popup-main-0".into(), failed_only: false, name: String::new(), ..named };
        assert!(filter_ipc(records, &other).is_empty());
    }

    #[test]
    fn screenshot_names_are_safe_file_names() {
        assert_eq!(shot_name(42, "main"), "42-main.png");
        assert_eq!(shot_name(7, "popup-main-0"), "7-popup-main-0.png");
        assert_eq!(shot_name(1, "a/b:c"), "1-a-b-c.png");
    }

    #[test]
    fn invoke_script_embeds_the_command_as_json() {
        let script = invoke_script("navigate\"x", &json!({ "id": 3 }));
        assert!(script.contains(r#"})("navigate\"x", {"id":3})"#));
    }

    #[test]
    fn previews_are_cut_on_a_character_boundary() {
        let (text, bytes) = preview(&json!("é".repeat(2000)));
        assert!(text.ends_with('…'));
        assert_eq!(bytes, 4002);
    }
}
