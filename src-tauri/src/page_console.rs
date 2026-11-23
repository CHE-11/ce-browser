//! The page console. While a window's console is open it listens to that tab's DevTools
//! events and evaluates in the page's main world, so values stay as live objects.
//! Out-of-process frames and workers are auto-attached as child sessions so their messages
//! show too. Object ids from a child session carry its session id (see [`tag_object`]).

use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Webview};
use webview2_com::DevToolsProtocolEventReceivedEventHandler;
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows::core::{HSTRING, Interface};

use crate::{tabs, webview2};

const GROUP: &str = "ce-console";
const COMPLETE_GROUP: &str = "ce-console-complete";
pub const POPOUT_LABEL: &str = "popup-console";

const EVAL_TIMEOUT: Duration = Duration::from_secs(20);
const SHORT: Duration = Duration::from_secs(3);
const COMPLETE_TIMEOUT: Duration = Duration::from_millis(800);
const FAILURE: &str = "The console could not reach the page.";

const LISTEN: [&str; 8] = [
    "Runtime.consoleAPICalled",
    "Runtime.exceptionThrown",
    "Runtime.executionContextCreated",
    "Runtime.executionContextsCleared",
    "Log.entryAdded",
    "Target.attachedToTarget",
    "Target.detachedFromTarget",
    "Overlay.inspectNodeRequested",
];

/// Starts an object id that belongs to a child session: `@<session>/<object id>`.
/// V8 object ids never start with `@`, and session ids contain no `/`.
const SESSION_MARK: char = '@';

/// Holds both buffers DevTools replays when the console attaches (up to 1000 each).
const EVENT_BACKLOG: usize = 4096;
const SEEN_LIMIT: usize = 4096;

const COMMAND_NAMES: [&str; 10] = [
    "$",
    "$$",
    "$0",
    "$_",
    "$x",
    "copy",
    "getEventListeners",
    "inspect",
    "keys",
    "values",
];

const TABLE_JS: &str = r#"function() {
  const value = this;
  const source = Array.isArray(value) ? value.slice(0, 50) : [value];
  const columns = [];
  const seen = new Set();
  const add = (name) => {
    if (columns.length >= 20 || seen.has(name)) return;
    seen.add(name);
    columns.push(name);
  };
  for (const row of source) {
    if (row && typeof row === "object") {
      for (const key of Object.keys(row)) add(String(key));
    }
  }
  if (columns.length === 0) columns.push("Value");
  const cell = (item) => {
    try {
      if (item === undefined) return "";
      if (item === null) return "null";
      if (typeof item === "string") return item.length > 200 ? item.slice(0, 200) + "…" : item;
      if (typeof item === "object") return item.constructor && item.constructor.name ? item.constructor.name : "Object";
      return String(item);
    } catch (error) {
      return "";
    }
  };
  const rows = source.map((row) => {
    if (row && typeof row === "object" && !Array.isArray(row)) {
      return columns.map((key) => cell(row[key]));
    }
    return [cell(row)];
  });
  return { columns, rows };
}"#;

const JSON_JS: &str = r#"function() {
  const seen = new WeakSet();
  const walk = (value, depth) => {
    if (value === null || typeof value !== "object") {
      if (typeof value === "string" && value.length > 200) return value.slice(0, 200) + "…";
      if (typeof value === "bigint") return value.toString();
      return value;
    }
    if (depth > 6) return "[…]";
    if (seen.has(value)) return "[Circular]";
    seen.add(value);
    if (Array.isArray(value)) return value.slice(0, 50).map((item) => walk(item, depth + 1));
    const out = {};
    let count = 0;
    for (const key of Object.keys(value)) {
      if (count >= 40) {
        out["…"] = true;
        break;
      }
      try {
        out[key] = walk(value[key], depth + 1);
      } catch (error) {
        out[key] = "[Thrown]";
      }
      count += 1;
    }
    return out;
  };
  try {
    return JSON.stringify(walk(this, 0), null, 2);
  } catch (error) {
    return String(this);
  }
}"#;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static GENERATION: AtomicU64 = AtomicU64::new(1);
static POPOUT: Mutex<Option<String>> = Mutex::new(None);
static SESSIONS: Mutex<Option<HashMap<String, u32>>> = Mutex::new(None);
static WATCHES: Mutex<Option<HashMap<u32, Watch>>> = Mutex::new(None);
static NAMES: Mutex<Option<HashMap<u32, (Instant, Vec<String>)>>> = Mutex::new(None);
static EVENTS: OnceLock<SyncSender<Incoming>> = OnceLock::new();

struct Watch {
    generation: u64,
    /// Execution context origins, keyed by session (empty for the page) and context id.
    contexts: HashMap<(String, i64), String>,
    default_origin: String,
    global_id: Option<String>,
    picking: bool,
    /// Attached child sessions and the origin each one was attached at.
    children: HashMap<String, String>,
    /// Recent message fingerprints. DevTools replays its backlog again whenever a domain is
    /// re-enabled or a child target re-attaches, and those copies must not show twice.
    seen: HashSet<u64>,
    seen_order: VecDeque<u64>,
}

struct Incoming {
    generation: u64,
    tab: u32,
    event: &'static str,
    /// Empty for the page, otherwise the child session the event came from.
    session: String,
    body: String,
}

struct Subscription {
    event: &'static str,
    token: i64,
    _handler: ICoreWebView2DevToolsProtocolEventReceivedEventHandler,
}

thread_local! {
    static SUBS: RefCell<HashMap<u32, Vec<Subscription>>> = RefCell::new(HashMap::new());
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsolePreviewEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleArg {
    #[serde(rename = "type")]
    pub kind: String,
    pub subtype: String,
    pub class_name: String,
    pub description: String,
    pub value: Value,
    pub object_id: String,
    pub overflow: bool,
    pub preview: Vec<ConsolePreviewEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleFrame {
    pub function_name: String,
    pub url: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleTable {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleMessage {
    pub tab_id: u32,
    pub id: u64,
    pub level: String,
    pub timestamp: f64,
    pub origin: String,
    pub args: Vec<ConsoleArg>,
    pub stack: Vec<ConsoleFrame>,
    pub text: String,
    pub table: Option<ConsoleTable>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleEvaluation {
    pub error: bool,
    pub value: ConsoleArg,
    pub stack: Vec<ConsoleFrame>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleProperty {
    pub name: String,
    pub value: ConsoleArg,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConsoleTab {
    tab_id: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConsoleReset {
    tab_id: u32,
    url: String,
    /// Wall time in milliseconds. Rows logged after this belong to the new document.
    at: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConsolePicked {
    tab_id: u32,
    value: ConsoleArg,
}

#[derive(Debug, PartialEq, Eq)]
enum Query {
    Global { prefix: String },
    Member { receiver: String, prefix: String },
}

fn map_new<K, V>() -> HashMap<K, V> {
    HashMap::new()
}

fn sessions() -> std::sync::MutexGuard<'static, Option<HashMap<String, u32>>> {
    let mut guard = SESSIONS.lock().expect("console sessions");
    if guard.is_none() {
        *guard = Some(map_new());
    }
    guard
}

fn watches() -> std::sync::MutexGuard<'static, Option<HashMap<u32, Watch>>> {
    let mut guard = WATCHES.lock().expect("console watches");
    if guard.is_none() {
        *guard = Some(map_new());
    }
    guard
}

fn names() -> std::sync::MutexGuard<'static, Option<HashMap<u32, (Instant, Vec<String>)>>> {
    let mut guard = NAMES.lock().expect("console names");
    if guard.is_none() {
        *guard = Some(map_new());
    }
    guard
}

/// Remembers which browser window the popped-out console belongs to.
pub fn note_popout(owner: Option<String>) {
    *POPOUT.lock().expect("console popout") = owner;
}

fn popout_is(window: &str) -> bool {
    POPOUT.lock().expect("console popout").as_deref() == Some(window)
}

/// The popped-out console closed. Stop listening when the dock is closed too.
pub fn popout_closed(app: &AppHandle) {
    let owner = POPOUT.lock().expect("console popout").take();
    if let Some(owner) = owner {
        let _ = disarm(app, &owner);
    }
}

/// Listens to `tab` for `window`. A second call for the same tab does nothing.
pub fn arm(app: &AppHandle, window: &str, tab: u32) -> Result<(), String> {
    let _ = events(app);
    let previous = {
        let mut guard = sessions();
        let map = guard.as_mut().expect("sessions");
        map.insert(window.to_string(), tab)
            .filter(|previous| *previous != tab)
    };
    if let Some(previous) = previous
        && !tab_in_use(previous)
    {
        release_tab(app, previous);
    }
    if watch_live(tab) {
        return Ok(());
    }
    attach_tab(app, tab)
}

/// Stops `window` listening, unless its console is still docked or popped out.
pub fn disarm(app: &AppHandle, window: &str) -> Result<(), String> {
    if popout_is(window) || tabs::dock_contains(app, window, tabs::CONSOLE_PANEL_ID) {
        return Ok(());
    }
    let tab = {
        let mut guard = sessions();
        guard.as_mut().expect("sessions").remove(window)
    };
    if let Some(tab) = tab
        && !tab_in_use(tab)
    {
        release_tab(app, tab);
    }
    Ok(())
}

/// A main-frame navigation started. Drop the previous document's log now, before the new
/// page logs anything. Clearing when the load finishes deletes the lines just captured.
pub fn on_navigation_started(app: &AppHandle, tab: u32, url: &str) {
    if !watch_live(tab) {
        return;
    }
    let at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0);
    let _ = app.emit(
        "console-reset",
        &ConsoleReset {
            tab_id: tab,
            url: url.to_string(),
            at,
        },
    );
}

/// The tab finished a main-frame navigation. Old object ids are dead.
pub fn on_navigated(app: &AppHandle, tab: u32) {
    if !watch_live(tab) {
        return;
    }
    let app = app.clone();
    thread::spawn(move || reset_after_navigation(&app, tab));
}

pub fn evaluate(app: &AppHandle, tab: u32, expression: &str) -> Result<ConsoleEvaluation, String> {
    if expression.len() > 100_000 {
        return Err("That expression is too long.".into());
    }
    let webview = watched_view(app, tab)?;
    let response = evaluate_raw(&webview, expression)?;
    if let Some(details) = response.get("exceptionDetails") {
        let remote = details.get("exception").cloned().unwrap_or(Value::Null);
        let value = if remote.is_object() {
            slim_remote(&remote)
        } else {
            let text = details
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("The expression failed.");
            ConsoleArg::text("error", text)
        };
        let text = if value.description.is_empty() {
            details
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("The expression failed.")
                .to_string()
        } else {
            value.description.clone()
        };
        return Ok(ConsoleEvaluation {
            error: true,
            value,
            stack: stack_of(details),
            text,
        });
    }
    let remote = response.get("result").cloned().unwrap_or(Value::Null);
    if let Err(error) = bind_name(tab, &webview, "$_", &remote) {
        eprintln!("console binding: {error}");
    }
    let value = slim_remote(&remote);
    let text = value.description.clone();
    Ok(ConsoleEvaluation {
        error: false,
        value,
        stack: Vec::new(),
        text,
    })
}

pub fn properties(
    app: &AppHandle,
    tab: u32,
    object_id: &str,
) -> Result<Vec<ConsoleProperty>, String> {
    if object_id.is_empty() {
        return Ok(Vec::new());
    }
    let webview = watched_view(app, tab)?;
    let (session, object_id) = split_object(object_id);
    let response = cdp_in(
        &webview,
        session,
        "Runtime.getProperties",
        json!({
            "objectId": object_id,
            "ownProperties": true,
            "generatePreview": true,
        }),
        SHORT,
    )?;
    let Some(list) = response.get("result").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let mut rows = Vec::new();
    for prop in list.iter().take(100) {
        let Some(name) = prop.get("name").and_then(Value::as_str) else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        let value = if let Some(remote) = prop.get("value") {
            tag_object(session, slim_remote(remote))
        } else if prop.get("get").is_some() {
            ConsoleArg::text("getter", "(getter)")
        } else {
            continue;
        };
        rows.push(ConsoleProperty {
            name: name.to_string(),
            value,
        });
    }
    Ok(rows)
}

pub fn complete(app: &AppHandle, tab: u32, expression: &str) -> Result<Vec<String>, String> {
    if !watch_live(tab) {
        return Ok(Vec::new());
    }
    let Some(query) = completion_query(expression) else {
        return Ok(Vec::new());
    };
    let names = match query {
        Query::Global { prefix } => {
            if prefix.is_empty() {
                return Ok(Vec::new());
            }
            let mut names = global_names(app, tab).unwrap_or_default();
            names.extend(COMMAND_NAMES.iter().map(|name| (*name).to_string()));
            filter_names(&names, &prefix)
        }
        Query::Member { receiver, prefix } => {
            let Ok(webview) = watched_view(app, tab) else {
                return Ok(Vec::new());
            };
            let names = member_names(&webview, &receiver).unwrap_or_default();
            filter_names(&names, &prefix)
        }
    };
    Ok(names)
}

pub fn json_of(app: &AppHandle, tab: u32, object_id: &str) -> Result<String, String> {
    if object_id.is_empty() {
        return Ok(String::new());
    }
    let webview = watched_view(app, tab)?;
    let (session, object_id) = split_object(object_id);
    let response = call_on(&webview, session, object_id, JSON_JS, true)?;
    Ok(response
        .pointer("/result/value")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string())
}

pub fn reveal(app: &AppHandle, tab: u32, object_id: &str) -> Result<(), String> {
    if object_id.is_empty() {
        return Ok(());
    }
    let webview = watched_view(app, tab)?;
    let (session, object_id) = split_object(object_id);
    call_on(
        &webview,
        session,
        object_id,
        "function(){ if (this && typeof this.scrollIntoView === 'function') this.scrollIntoView({ block: 'center', inline: 'nearest' }); }",
        false,
    )?;
    if let Some(backend) = backend_of(&webview, session, object_id) {
        flash_node(app, tab, session, backend);
    }
    Ok(())
}

pub fn pick(app: &AppHandle, tab: u32, enabled: bool) -> Result<bool, String> {
    let webview = watched_view(app, tab)?;
    if !enabled {
        set_picking(tab, false);
        let _ = stop_pick(&webview);
        return Ok(false);
    }
    set_picking(tab, true);
    match cdp(
        &webview,
        "Overlay.setInspectMode",
        json!({
            "mode": "searchForNode",
            "highlightConfig": highlight_config(),
        }),
        SHORT,
    ) {
        Ok(_) => Ok(true),
        Err(error) => {
            set_picking(tab, false);
            Err(error)
        }
    }
}

fn events(app: &AppHandle) -> SyncSender<Incoming> {
    EVENTS
        .get_or_init(|| {
            let (tx, rx) = mpsc::sync_channel(EVENT_BACKLOG);
            let app = app.clone();
            thread::spawn(move || pump(app, rx));
            tx
        })
        .clone()
}

fn pump(app: AppHandle, rx: Receiver<Incoming>) {
    while let Ok(incoming) = rx.recv() {
        if !generation_is(incoming.tab, incoming.generation) {
            continue;
        }
        match incoming.event {
            "Runtime.executionContextCreated" => note_context(&incoming),
            "Runtime.executionContextsCleared" => clear_contexts(&incoming),
            "Runtime.consoleAPICalled" => on_console(&app, &incoming),
            "Runtime.exceptionThrown" => on_exception(&app, &incoming),
            "Log.entryAdded" => on_log(&app, &incoming),
            "Target.attachedToTarget" => on_attached(&app, &incoming),
            "Target.detachedFromTarget" => on_detached(&incoming),
            "Overlay.inspectNodeRequested" => on_inspect(&app, &incoming),
            _ => {}
        }
    }
}

fn auto_attach(enabled: bool) -> Value {
    json!({
        "autoAttach": enabled,
        "waitForDebuggerOnStart": false,
        "flatten": true,
    })
}

/// Turns on the domains the console reads in the page or in one child session. Runtime and
/// Log replay the messages they buffered before the console attached.
fn enable_session(webview: &Webview, session: &str) -> Result<(), String> {
    cdp_in(webview, session, "Runtime.enable", json!({}), SHORT)?;
    if let Err(error) = cdp_in(webview, session, "Log.enable", json!({}), SHORT) {
        eprintln!("console Log.enable: {error}");
    }
    if let Err(error) = cdp_in(
        webview,
        session,
        "Target.setAutoAttach",
        auto_attach(true),
        SHORT,
    ) {
        eprintln!("console auto-attach: {error}");
    }
    Ok(())
}

fn on_attached(app: &AppHandle, incoming: &Incoming) {
    let Ok(body) = serde_json::from_str::<Value>(&incoming.body) else {
        return;
    };
    let Some(session) = body.get("sessionId").and_then(Value::as_str) else {
        return;
    };
    let url = body
        .pointer("/targetInfo/url")
        .and_then(Value::as_str)
        .unwrap_or("");
    {
        let mut guard = watches();
        let Some(watch) = guard.as_mut().expect("watches").get_mut(&incoming.tab) else {
            return;
        };
        if watch.generation != incoming.generation {
            return;
        }
        watch
            .children
            .insert(session.to_string(), origin_of_url(url));
    }
    let app = app.clone();
    let tab = incoming.tab;
    let session = session.to_string();
    thread::spawn(move || {
        if let Some(webview) = tabs::page_view(&app, tab)
            && let Err(error) = enable_session(&webview, &session)
        {
            eprintln!("console child session: {error}");
        }
    });
}

fn on_detached(incoming: &Incoming) {
    let Ok(body) = serde_json::from_str::<Value>(&incoming.body) else {
        return;
    };
    let Some(session) = body.get("sessionId").and_then(Value::as_str) else {
        return;
    };
    let mut guard = watches();
    let Some(watch) = guard.as_mut().expect("watches").get_mut(&incoming.tab) else {
        return;
    };
    if watch.generation != incoming.generation {
        return;
    }
    watch.children.remove(session);
    watch.contexts.retain(|(owner, _), _| owner != session);
}

fn origin_of_url(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return String::new();
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    if host.is_empty() {
        String::new()
    } else {
        format!("{scheme}://{host}")
    }
}

fn on_console(app: &AppHandle, incoming: &Incoming) {
    let Ok(body) = serde_json::from_str::<Value>(&incoming.body) else {
        return;
    };
    let raw = body.get("type").and_then(Value::as_str).unwrap_or("log");
    if raw == "clear" {
        let _ = app.emit(
            "console-cleared",
            &ConsoleTab {
                tab_id: incoming.tab,
            },
        );
        return;
    }
    let level = level_of(raw).to_string();
    let session = incoming.session.as_str();
    let args = body
        .get("args")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .map(|remote| tag_object(session, slim_remote(remote)))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let table = if level == "table" {
        args.first()
            .map(|arg| arg.object_id.clone())
            .filter(|id| !id.is_empty())
            .and_then(|id| table_of(app, incoming.tab, &id))
    } else {
        None
    };
    let context = body.get("executionContextId").and_then(Value::as_i64);
    let message = ConsoleMessage {
        tab_id: incoming.tab,
        id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        level,
        timestamp: body.get("timestamp").and_then(Value::as_f64).unwrap_or(0.0),
        origin: origin_of(incoming.tab, session, context),
        text: text_of(&args),
        stack: stack_of(&body),
        table,
        args,
    };
    emit_message(app, &message);
}

fn on_exception(app: &AppHandle, incoming: &Incoming) {
    let Ok(body) = serde_json::from_str::<Value>(&incoming.body) else {
        return;
    };
    let details = body.get("exceptionDetails").cloned().unwrap_or(Value::Null);
    let remote = details.get("exception").cloned().unwrap_or(Value::Null);
    let session = incoming.session.as_str();
    let arg = if remote.is_object() {
        tag_object(session, slim_remote(&remote))
    } else {
        ConsoleArg::text(
            "error",
            details
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("Uncaught exception"),
        )
    };
    let text = if arg.description.is_empty() {
        "Uncaught exception".to_string()
    } else {
        arg.description.clone()
    };
    let context = details.get("executionContextId").and_then(Value::as_i64);
    let message = ConsoleMessage {
        tab_id: incoming.tab,
        id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        level: "error".to_string(),
        timestamp: body.get("timestamp").and_then(Value::as_f64).unwrap_or(0.0),
        origin: origin_of(incoming.tab, session, context),
        text,
        stack: stack_of(&details),
        table: None,
        args: vec![arg],
    };
    emit_message(app, &message);
}

fn emit_message(app: &AppHandle, message: &ConsoleMessage) {
    if first_sighting(message) {
        let _ = app.emit("console-message", message);
    }
}

fn first_sighting(message: &ConsoleMessage) -> bool {
    if message.timestamp == 0.0 {
        return true;
    }
    let mut hasher = DefaultHasher::new();
    message.level.hash(&mut hasher);
    message.timestamp.to_bits().hash(&mut hasher);
    message.text.hash(&mut hasher);
    if let Some(frame) = message.stack.first() {
        (&frame.url, frame.line, frame.column).hash(&mut hasher);
    }
    let key = hasher.finish();
    let mut guard = watches();
    let Some(watch) = guard.as_mut().expect("watches").get_mut(&message.tab_id) else {
        return true;
    };
    if !watch.seen.insert(key) {
        return false;
    }
    watch.seen_order.push_back(key);
    if watch.seen_order.len() > SEEN_LIMIT
        && let Some(old) = watch.seen_order.pop_front()
    {
        watch.seen.remove(&old);
    }
    true
}

/// Messages the browser itself logs: failed loads, blocked scripts, CSP, deprecations.
fn on_log(app: &AppHandle, incoming: &Incoming) {
    let Ok(body) = serde_json::from_str::<Value>(&incoming.body) else {
        return;
    };
    let Some(entry) = body.get("entry") else {
        return;
    };
    let level = match entry.get("level").and_then(Value::as_str).unwrap_or("") {
        "error" => "error",
        "warning" => "warn",
        "info" => "info",
        "verbose" => "debug",
        _ => "log",
    };
    let mut text = entry
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let url = entry.get("url").and_then(Value::as_str).unwrap_or("");
    let source = entry.get("source").and_then(Value::as_str).unwrap_or("");
    if source == "network" && !url.is_empty() && !text.contains(url) {
        text = format!("{text} {url}");
    }
    let message = ConsoleMessage {
        tab_id: incoming.tab,
        id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        level: level.to_string(),
        timestamp: entry
            .get("timestamp")
            .and_then(Value::as_f64)
            .unwrap_or(0.0),
        origin: origin_of(incoming.tab, &incoming.session, None),
        text: clip(&text, 2000),
        stack: stack_of(entry),
        table: None,
        args: Vec::new(),
    };
    emit_message(app, &message);
}

fn on_inspect(app: &AppHandle, incoming: &Incoming) {
    let Ok(body) = serde_json::from_str::<Value>(&incoming.body) else {
        return;
    };
    if !take_picking(incoming.tab) {
        return;
    }
    let Some(webview) = tabs::page_view(app, incoming.tab) else {
        return;
    };
    let _ = stop_pick(&webview);
    let Some(backend) = body.get("backendNodeId").and_then(Value::as_u64) else {
        emit_note(app, incoming.tab, "Couldn't read that element.");
        return;
    };
    let remote = match resolve_node(&webview, backend) {
        Ok(remote) => remote,
        Err(_) => {
            emit_note(app, incoming.tab, "Couldn't read that element.");
            return;
        }
    };
    if let Err(error) = bind_name(incoming.tab, &webview, "$0", &remote) {
        eprintln!("console $0: {error}");
    }
    let value = slim_remote(&remote);
    flash_node(app, incoming.tab, "", backend);
    let _ = app.emit(
        "console-picked",
        &ConsolePicked {
            tab_id: incoming.tab,
            value,
        },
    );
}

fn emit_note(app: &AppHandle, tab: u32, text: &str) {
    let arg = ConsoleArg::text("error", text);
    let _ = app.emit(
        "console-message",
        &ConsoleMessage {
            tab_id: tab,
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            level: "error".to_string(),
            timestamp: 0.0,
            origin: String::new(),
            text: text.to_string(),
            stack: Vec::new(),
            table: None,
            args: vec![arg],
        },
    );
}

fn note_context(incoming: &Incoming) {
    let Ok(value) = serde_json::from_str::<Value>(&incoming.body) else {
        return;
    };
    let Some(id) = value.pointer("/context/id").and_then(Value::as_i64) else {
        return;
    };
    let origin = value
        .pointer("/context/origin")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let is_default = value
        .pointer("/context/auxData/isDefault")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut guard = watches();
    let Some(watch) = guard.as_mut().expect("watches").get_mut(&incoming.tab) else {
        return;
    };
    if watch.generation != incoming.generation {
        return;
    }
    if is_default && incoming.session.is_empty() {
        watch.default_origin.clone_from(&origin);
    }
    watch
        .contexts
        .insert((incoming.session.clone(), id), origin);
}

fn clear_contexts(incoming: &Incoming) {
    let mut guard = watches();
    let Some(watch) = guard.as_mut().expect("watches").get_mut(&incoming.tab) else {
        return;
    };
    if watch.generation != incoming.generation {
        return;
    }
    watch
        .contexts
        .retain(|(owner, _), _| *owner != incoming.session);
    if incoming.session.is_empty() {
        watch.default_origin.clear();
        watch.global_id = None;
    }
}

fn reset_after_navigation(app: &AppHandle, tab: u32) {
    if !watch_live(tab) {
        return;
    }
    names().as_mut().expect("names").remove(&tab);
    {
        let mut guard = watches();
        if let Some(watch) = guard.as_mut().expect("watches").get_mut(&tab) {
            watch.contexts.clear();
            watch.default_origin.clear();
            watch.global_id = None;
            watch.picking = false;
        }
    }
    if let Some(webview) = tabs::page_view(app, tab) {
        let _ = stop_pick(&webview);
        let _ = release_group(&webview, GROUP);
        let _ = enable_session(&webview, "");
    }
}

fn attach_tab(app: &AppHandle, tab: u32) -> Result<(), String> {
    let webview = crate::automation::driver::view(app, tab)?;
    let generation = GENERATION.fetch_add(1, Ordering::Relaxed);
    // Registered before enabling: the replayed backlog arrives before `Runtime.enable` answers.
    watches().as_mut().expect("watches").insert(
        tab,
        Watch {
            generation,
            contexts: HashMap::new(),
            default_origin: String::new(),
            global_id: None,
            picking: false,
            children: HashMap::new(),
            seen: HashSet::new(),
            seen_order: VecDeque::new(),
        },
    );
    let attached = install_listeners(app, &webview, tab, generation)
        .and_then(|()| enable_session(&webview, ""));
    if let Err(error) = attached {
        watches().as_mut().expect("watches").remove(&tab);
        uninstall(&webview, tab);
        return Err(error);
    }
    Ok(())
}

fn release_tab(app: &AppHandle, tab: u32) {
    names().as_mut().expect("names").remove(&tab);
    let children = watches()
        .as_mut()
        .expect("watches")
        .remove(&tab)
        .map(|watch| watch.children)
        .unwrap_or_default();
    let Some(webview) = tabs::page_view(app, tab) else {
        drop_handlers(app, tab);
        return;
    };
    let _ = clear_bindings(&webview);
    let _ = stop_pick(&webview);
    let _ = release_group(&webview, GROUP);
    uninstall(&webview, tab);
    let _ = cdp(&webview, "Target.setAutoAttach", auto_attach(false), SHORT);
    for session in children.keys() {
        let _ = cdp(
            &webview,
            "Target.detachFromTarget",
            json!({ "sessionId": session }),
            SHORT,
        );
    }
    let _ = cdp(&webview, "Log.disable", json!({}), SHORT);
}

fn tab_in_use(tab: u32) -> bool {
    sessions()
        .as_ref()
        .expect("sessions")
        .values()
        .any(|current| *current == tab)
}

fn watch_live(tab: u32) -> bool {
    watches().as_ref().expect("watches").contains_key(&tab)
}

fn generation_is(tab: u32, generation: u64) -> bool {
    watches()
        .as_ref()
        .expect("watches")
        .get(&tab)
        .is_some_and(|watch| watch.generation == generation)
}

fn watched_view(app: &AppHandle, tab: u32) -> Result<Webview, String> {
    if !watch_live(tab) {
        return Err("Open the console on this page first.".into());
    }
    tabs::page_view(app, tab).ok_or_else(|| "That tab is closed.".to_string())
}

fn set_picking(tab: u32, picking: bool) {
    if let Some(watch) = watches().as_mut().expect("watches").get_mut(&tab) {
        watch.picking = picking;
    }
}

fn take_picking(tab: u32) -> bool {
    let mut guard = watches();
    let Some(watch) = guard.as_mut().expect("watches").get_mut(&tab) else {
        return false;
    };
    if !watch.picking {
        return false;
    }
    watch.picking = false;
    true
}

fn origin_of(tab: u32, session: &str, context: Option<i64>) -> String {
    let guard = watches();
    let Some(watch) = guard.as_ref().expect("watches").get(&tab) else {
        return String::new();
    };
    context
        .and_then(|id| watch.contexts.get(&(session.to_string(), id)).cloned())
        .filter(|origin| !origin.is_empty())
        .or_else(|| {
            watch
                .children
                .get(session)
                .filter(|origin| !origin.is_empty())
                .cloned()
        })
        .unwrap_or_else(|| watch.default_origin.clone())
}

/// Marks a child session's object id so later calls on it reach that session.
fn tag_object(session: &str, mut arg: ConsoleArg) -> ConsoleArg {
    if !session.is_empty() && !arg.object_id.is_empty() {
        arg.object_id = format!("{SESSION_MARK}{session}/{}", arg.object_id);
    }
    arg
}

/// Splits a console object id into its session (empty for the page) and the DevTools id.
fn split_object(object_id: &str) -> (&str, &str) {
    object_id
        .strip_prefix(SESSION_MARK)
        .and_then(|rest| rest.split_once('/'))
        .unwrap_or(("", object_id))
}

fn install_listeners(
    app: &AppHandle,
    webview: &Webview,
    tab: u32,
    generation: u64,
) -> Result<(), String> {
    let tx = events(app);
    let (done_tx, done_rx) = mpsc::channel();
    webview
        .with_webview(move |platform| {
            let outcome = unsafe {
                match platform.controller().CoreWebView2() {
                    Ok(core) => install(&core, tab, generation, &tx),
                    Err(error) => Err(error.to_string()),
                }
            };
            let _ = done_tx.send(outcome);
        })
        .map_err(|error| error.to_string())?;
    done_rx
        .recv_timeout(Duration::from_secs(3))
        .map_err(|_| "The page did not open its console stream.".to_string())?
}

fn install(
    core: &ICoreWebView2,
    tab: u32,
    generation: u64,
    tx: &SyncSender<Incoming>,
) -> Result<(), String> {
    uninstall_local(core, tab);
    let mut added = 0;
    for event in LISTEN {
        if add_listener(core, tab, event, generation, tx).is_ok() {
            added += 1;
        }
    }
    if added == 0 {
        Err("The page did not open its console stream.".into())
    } else {
        Ok(())
    }
}

fn add_listener(
    core: &ICoreWebView2,
    tab: u32,
    event: &'static str,
    generation: u64,
    tx: &SyncSender<Incoming>,
) -> Result<(), String> {
    unsafe {
        let receiver = core
            .GetDevToolsProtocolEventReceiver(&HSTRING::from(event))
            .map_err(|error| error.to_string())?;
        let sender = tx.clone();
        let handler =
            DevToolsProtocolEventReceivedEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else {
                    return Ok(());
                };
                let body = webview2::read_pwstr(|value| args.ParameterObjectAsJson(value));
                if !body.is_empty() {
                    let session = args
                        .cast::<ICoreWebView2DevToolsProtocolEventReceivedEventArgs2>()
                        .map(|args| webview2::read_pwstr(|value| args.SessionId(value)))
                        .unwrap_or_default();
                    let _ = sender.try_send(Incoming {
                        generation,
                        tab,
                        event,
                        session,
                        body,
                    });
                }
                Ok(())
            }));
        let mut token = 0i64;
        receiver
            .add_DevToolsProtocolEventReceived(&handler, &mut token)
            .map_err(|error| error.to_string())?;
        SUBS.with(|cell| {
            cell.borrow_mut()
                .entry(tab)
                .or_default()
                .push(Subscription {
                    event,
                    token,
                    _handler: handler,
                });
        });
        Ok(())
    }
}

fn uninstall(webview: &Webview, tab: u32) {
    let (done_tx, done_rx) = mpsc::channel();
    let _ = webview.with_webview(move |platform| {
        if let Ok(core) = unsafe { platform.controller().CoreWebView2() } {
            uninstall_local(&core, tab);
        } else {
            SUBS.with(|cell| {
                cell.borrow_mut().remove(&tab);
            });
        }
        let _ = done_tx.send(());
    });
    let _ = done_rx.recv_timeout(Duration::from_secs(2));
}

fn uninstall_local(core: &ICoreWebView2, tab: u32) {
    let subs = SUBS
        .with(|cell| cell.borrow_mut().remove(&tab))
        .unwrap_or_default();
    for sub in subs {
        if let Ok(receiver) =
            unsafe { core.GetDevToolsProtocolEventReceiver(&HSTRING::from(sub.event)) }
        {
            let _ = unsafe { receiver.remove_DevToolsProtocolEventReceived(sub.token) };
        }
    }
}

fn drop_handlers(app: &AppHandle, tab: u32) {
    let _ = app.run_on_main_thread(move || {
        SUBS.with(|cell| {
            cell.borrow_mut().remove(&tab);
        });
    });
}

fn evaluate_raw(webview: &Webview, expression: &str) -> Result<Value, String> {
    let full = json!({
        "expression": expression,
        "objectGroup": GROUP,
        "includeCommandLineAPI": true,
        "silent": true,
        "returnByValue": false,
        "generatePreview": true,
        "userGesture": true,
        "awaitPromise": true,
        "replMode": true,
        "allowUnsafeEvalBlockedByCSP": true,
    });
    match cdp(webview, "Runtime.evaluate", full, EVAL_TIMEOUT) {
        Ok(value) => Ok(value),
        Err(error) if invalid_params(&error) => cdp(
            webview,
            "Runtime.evaluate",
            json!({
                "expression": expression,
                "objectGroup": GROUP,
                "silent": true,
                "returnByValue": false,
                "generatePreview": true,
                "userGesture": true,
                "awaitPromise": true,
            }),
            EVAL_TIMEOUT,
        ),
        Err(error) => Err(error),
    }
}

fn invalid_params(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("invalid parameters")
        || lower.contains("replmode")
        || lower.contains("unrecognized")
}

fn bind_name(tab: u32, webview: &Webview, name: &str, remote: &Value) -> Result<(), String> {
    let kind = remote.get("type").and_then(Value::as_str).unwrap_or("");
    if kind == "undefined" {
        return cdp(
            webview,
            "Runtime.evaluate",
            json!({
                "expression": format!("Object.defineProperty(globalThis, \"{name}\", {{ value: undefined, writable: true, configurable: true }})"),
                "silent": true,
                "objectGroup": GROUP,
            }),
            SHORT,
        )
        .map(|_| ());
    }
    let global = global_id(tab, webview)?;
    let argument = if let Some(id) = remote.get("objectId").and_then(Value::as_str) {
        json!({ "objectId": id })
    } else if let Some(raw) = remote.get("unserializableValue").and_then(Value::as_str) {
        json!({ "unserializableValue": raw })
    } else {
        json!({ "value": remote.get("value").cloned().unwrap_or(Value::Null) })
    };
    cdp(
        webview,
        "Runtime.callFunctionOn",
        json!({
            "objectId": global,
            "functionDeclaration": format!("function(value){{Object.defineProperty(globalThis,\"{name}\",{{value:value,writable:true,configurable:true}});}}"),
            "arguments": [argument],
            "silent": true,
        }),
        SHORT,
    )
    .map(|_| ())
}

fn global_id(tab: u32, webview: &Webview) -> Result<String, String> {
    if let Some(id) = watches()
        .as_ref()
        .expect("watches")
        .get(&tab)
        .and_then(|watch| watch.global_id.clone())
    {
        return Ok(id);
    }
    let response = cdp(
        webview,
        "Runtime.evaluate",
        json!({
            "expression": "globalThis",
            "returnByValue": false,
            "objectGroup": GROUP,
            "silent": true,
        }),
        SHORT,
    )?;
    let id = response
        .pointer("/result/objectId")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if id.is_empty() {
        return Err("The page has no global object.".into());
    }
    if let Some(watch) = watches().as_mut().expect("watches").get_mut(&tab) {
        watch.global_id = Some(id.clone());
    }
    Ok(id)
}

fn call_on(
    webview: &Webview,
    session: &str,
    object_id: &str,
    function: &str,
    by_value: bool,
) -> Result<Value, String> {
    cdp_in(
        webview,
        session,
        "Runtime.callFunctionOn",
        json!({
            "objectId": object_id,
            "functionDeclaration": function,
            "returnByValue": by_value,
            "silent": true,
        }),
        SHORT,
    )
}

fn member_names(webview: &Webview, receiver: &str) -> Result<Vec<String>, String> {
    let response = cdp(
        webview,
        "Runtime.evaluate",
        json!({
            "expression": receiver,
            "returnByValue": false,
            "objectGroup": COMPLETE_GROUP,
            "silent": true,
            "userGesture": false,
        }),
        COMPLETE_TIMEOUT,
    )?;
    let object_id = response
        .pointer("/result/objectId")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let names = if object_id.is_empty() {
        Vec::new()
    } else {
        property_names(webview, &object_id).unwrap_or_default()
    };
    let _ = release_group(webview, COMPLETE_GROUP);
    Ok(names)
}

fn property_names(webview: &Webview, object_id: &str) -> Result<Vec<String>, String> {
    let response = cdp(
        webview,
        "Runtime.getProperties",
        json!({ "objectId": object_id, "ownProperties": false }),
        COMPLETE_TIMEOUT,
    )?;
    let Some(list) = response.get("result").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    Ok(list
        .iter()
        .filter_map(|prop| prop.get("name").and_then(Value::as_str).map(str::to_string))
        .filter(|name| !name.is_empty())
        .take(400)
        .collect())
}

fn global_names(app: &AppHandle, tab: u32) -> Result<Vec<String>, String> {
    if let Some((at, names)) = names().as_ref().expect("names").get(&tab)
        && at.elapsed() < Duration::from_secs(5)
    {
        return Ok(names.clone());
    }
    let webview = watched_view(app, tab)?;
    let response = cdp(
        &webview,
        "Runtime.evaluate",
        json!({
            "expression": "(() => { const names = []; const seen = new Set(); let object = globalThis; let guard = 0; while (object && guard < 8) { for (const key of Object.getOwnPropertyNames(object)) { if (!seen.has(key)) { seen.add(key); names.push(key); } } object = Object.getPrototypeOf(object); guard += 1; } return names.slice(0, 800); })()",
            "returnByValue": true,
            "silent": true,
        }),
        COMPLETE_TIMEOUT,
    )?;
    let parsed = response
        .pointer("/result/value")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    names()
        .as_mut()
        .expect("names")
        .insert(tab, (Instant::now(), parsed.clone()));
    Ok(parsed)
}

fn filter_names(names: &[String], prefix: &str) -> Vec<String> {
    let mut matched: Vec<String> = names
        .iter()
        .filter(|name| name.starts_with(prefix) && !name.is_empty())
        .cloned()
        .collect();
    matched.sort();
    matched.dedup();
    matched.truncate(40);
    matched
}

fn table_of(app: &AppHandle, tab: u32, object_id: &str) -> Option<ConsoleTable> {
    let webview = tabs::page_view(app, tab)?;
    let (session, object_id) = split_object(object_id);
    let response = call_on(&webview, session, object_id, TABLE_JS, true).ok()?;
    let data = response.pointer("/result/value")?;
    let columns = data
        .get("columns")?
        .as_array()?
        .iter()
        .filter_map(|value| value.as_str().map(str::to_string))
        .collect();
    let rows = data
        .get("rows")?
        .as_array()?
        .iter()
        .filter_map(|row| {
            row.as_array().map(|cells| {
                cells
                    .iter()
                    .map(|cell| cell.as_str().unwrap_or("").to_string())
                    .collect()
            })
        })
        .collect();
    Some(ConsoleTable { columns, rows })
}

fn resolve_node(webview: &Webview, backend: u64) -> Result<Value, String> {
    let response = cdp(
        webview,
        "DOM.resolveNode",
        json!({ "backendNodeId": backend }),
        SHORT,
    )?;
    Ok(response.get("object").cloned().unwrap_or(Value::Null))
}

fn backend_of(webview: &Webview, session: &str, object_id: &str) -> Option<u64> {
    let response = cdp_in(
        webview,
        session,
        "DOM.describeNode",
        json!({ "objectId": object_id }),
        SHORT,
    )
    .ok()?;
    response
        .pointer("/node/backendNodeId")
        .and_then(Value::as_u64)
}

fn flash_node(app: &AppHandle, tab: u32, session: &str, backend: u64) {
    let Some(webview) = tabs::page_view(app, tab) else {
        return;
    };
    let _ = cdp_in(
        &webview,
        session,
        "Overlay.highlightNode",
        json!({
            "backendNodeId": backend,
            "highlightConfig": highlight_config(),
        }),
        SHORT,
    );
    let app = app.clone();
    let session = session.to_string();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(800));
        if let Some(webview) = tabs::page_view(&app, tab) {
            let _ = cdp_in(
                &webview,
                &session,
                "Overlay.hideHighlight",
                json!({}),
                SHORT,
            );
        }
    });
}

fn stop_pick(webview: &Webview) -> Result<(), String> {
    cdp(
        webview,
        "Overlay.setInspectMode",
        json!({ "mode": "none" }),
        SHORT,
    )
    .map(|_| ())
}

fn clear_bindings(webview: &Webview) -> Result<(), String> {
    cdp(
        webview,
        "Runtime.evaluate",
        json!({
            "expression": "delete globalThis.$_; delete globalThis.$0;",
            "silent": true,
        }),
        SHORT,
    )
    .map(|_| ())
}

fn release_group(webview: &Webview, group: &str) -> Result<(), String> {
    cdp(
        webview,
        "Runtime.releaseObjectGroup",
        json!({ "objectGroup": group }),
        SHORT,
    )
    .map(|_| ())
}

fn highlight_config() -> Value {
    json!({
        "showInfo": true,
        "contentColor": { "r": 111, "g": 168, "b": 220, "a": 0.35 },
        "borderColor": { "r": 56, "g": 121, "b": 217, "a": 0.9 },
    })
}

fn cdp(webview: &Webview, method: &str, params: Value, timeout: Duration) -> Result<Value, String> {
    webview2::call_cdp_json(webview, method, &params.to_string(), timeout, FAILURE)
}

fn cdp_in(
    webview: &Webview,
    session: &str,
    method: &str,
    params: Value,
    timeout: Duration,
) -> Result<Value, String> {
    webview2::call_cdp_json_in(
        webview,
        session,
        method,
        &params.to_string(),
        timeout,
        FAILURE,
    )
}

fn level_of(kind: &str) -> &'static str {
    match kind {
        "warning" => "warn",
        "startGroup" => "group",
        "startGroupCollapsed" => "groupCollapsed",
        "endGroup" => "groupEnd",
        "timeEnd" => "time",
        "dirxml" => "dir",
        "debug" => "debug",
        "info" => "info",
        "error" => "error",
        "dir" => "dir",
        "table" => "table",
        "trace" => "trace",
        "assert" => "assert",
        "count" => "count",
        "profile" | "profileEnd" | "log" => "log",
        _ => "log",
    }
}

fn text_of(args: &[ConsoleArg]) -> String {
    let text = args
        .iter()
        .map(|arg| {
            if arg.description.is_empty() {
                arg.kind.clone()
            } else {
                arg.description.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    clip(&text, 2000)
}

fn stack_of(value: &Value) -> Vec<ConsoleFrame> {
    let frames = value
        .pointer("/stackTrace/callFrames")
        .or_else(|| value.get("callFrames"))
        .and_then(Value::as_array);
    let mut rows = Vec::new();
    if let Some(frames) = frames {
        for frame in frames.iter().take(20) {
            rows.push(ConsoleFrame {
                function_name: frame
                    .get("functionName")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                url: frame
                    .get("url")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                line: frame.get("lineNumber").and_then(Value::as_u64).unwrap_or(0) as u32 + 1,
                column: frame
                    .get("columnNumber")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as u32
                    + 1,
            });
        }
    }
    if rows.is_empty()
        && let Some(url) = value.get("url").and_then(Value::as_str)
        && !url.is_empty()
    {
        rows.push(ConsoleFrame {
            function_name: String::new(),
            url: url.to_string(),
            line: value.get("lineNumber").and_then(Value::as_u64).unwrap_or(0) as u32 + 1,
            column: value
                .get("columnNumber")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32
                + 1,
        });
    }
    rows
}

fn slim_remote(remote: &Value) -> ConsoleArg {
    let kind = remote
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("undefined");
    let subtype = remote.get("subtype").and_then(Value::as_str).unwrap_or("");
    let class_name = remote
        .get("className")
        .and_then(Value::as_str)
        .unwrap_or("");
    let object_id = remote.get("objectId").and_then(Value::as_str).unwrap_or("");
    let mut value = remote.get("value").cloned().unwrap_or(Value::Null);
    if let Value::String(text) = &value {
        let clipped = clip(text, 4000);
        if clipped != *text {
            value = Value::String(clipped);
        }
    }
    if value.is_null()
        && let Some(raw) = remote.get("unserializableValue").and_then(Value::as_str)
    {
        value = Value::String(raw.to_string());
    }
    let mut description = remote
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if description.is_empty() {
        description = match kind {
            "string" => value
                .as_str()
                .map(|text| format!("\"{}\"", clip(text, 200)))
                .unwrap_or_else(|| "\"\"".to_string()),
            "number" | "boolean" => match &value {
                Value::Null => String::new(),
                other => other.to_string(),
            },
            "undefined" => "undefined".to_string(),
            "object" if subtype == "null" => "null".to_string(),
            _ => class_name.to_string(),
        };
    }
    let description = clip(&description, 400);
    let mut preview = Vec::new();
    if let Some(list) = remote
        .pointer("/preview/properties")
        .and_then(Value::as_array)
    {
        for prop in list.iter().take(12) {
            preview.push(ConsolePreviewEntry {
                name: prop
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                kind: prop
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                value: prop
                    .get("value")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            });
        }
    }
    ConsoleArg {
        kind: kind.to_string(),
        subtype: subtype.to_string(),
        class_name: class_name.to_string(),
        description,
        value,
        object_id: object_id.to_string(),
        overflow: remote
            .pointer("/preview/overflow")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        preview,
    }
}

impl ConsoleArg {
    fn text(kind: &str, description: &str) -> Self {
        Self {
            kind: kind.to_string(),
            subtype: String::new(),
            class_name: String::new(),
            description: description.to_string(),
            value: Value::Null,
            object_id: String::new(),
            overflow: false,
            preview: Vec::new(),
        }
    }
}

fn clip(text: &str, max: usize) -> String {
    let count = text.chars().count();
    if count <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max).collect();
    out.push('…');
    out
}

fn completion_query(source: &str) -> Option<Query> {
    if source.len() > 8000
        || source.ends_with(|ch: char| ch.is_whitespace())
        || in_code_span(source)
    {
        return None;
    }
    let ident = trailing_ident(source);
    let before = &source[..source.len() - ident.len()];
    if let Some(receiver) = before.strip_suffix('.') {
        let receiver = member_receiver(receiver)?;
        if receiver
            .chars()
            .any(|ch| ch == '(' || ch == ')' || ch == '`')
        {
            return None;
        }
        return Some(Query::Member {
            receiver,
            prefix: ident.to_string(),
        });
    }
    if ident.is_empty() || ident.starts_with(|ch: char| ch.is_ascii_digit()) {
        return None;
    }
    Some(Query::Global {
        prefix: ident.to_string(),
    })
}

fn trailing_ident(source: &str) -> &str {
    let bytes = source.as_bytes();
    let mut start = bytes.len();
    while start > 0 {
        let ch = bytes[start - 1] as char;
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '$' {
            start -= 1;
            continue;
        }
        break;
    }
    &source[start..]
}

fn member_receiver(source: &str) -> Option<String> {
    let chars: Vec<char> = source.chars().collect();
    if chars.is_empty() {
        return None;
    }
    let mut index = chars.len();
    let mut depth = 0i32;
    let mut started = false;
    while index > 0 {
        let ch = chars[index - 1];
        if depth > 0 {
            if ch == ']' {
                depth += 1;
            } else if ch == '[' {
                depth -= 1;
            }
            index -= 1;
            started = true;
            continue;
        }
        if ch == ']' {
            depth = 1;
            index -= 1;
            started = true;
            continue;
        }
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '$' || ch == '.' {
            index -= 1;
            started = true;
            continue;
        }
        break;
    }
    if depth != 0 || !started {
        return None;
    }
    let receiver: String = chars[index..].iter().collect();
    let receiver = receiver.trim_matches('.').trim().to_string();
    if receiver.is_empty() || receiver.len() > 300 || receiver.contains("..") {
        return None;
    }
    Some(receiver)
}

fn in_code_span(source: &str) -> bool {
    let chars: Vec<char> = source.chars().collect();
    let mut index = 0;
    let mut quote: Option<char> = None;
    let mut block = false;
    while index < chars.len() {
        let ch = chars[index];
        let next = chars.get(index + 1).copied();
        if block {
            if ch == '*' && next == Some('/') {
                block = false;
                index += 2;
                continue;
            }
            index += 1;
            continue;
        }
        if let Some(mark) = quote {
            if ch == '\\' {
                index += 2;
                continue;
            }
            if ch == mark {
                quote = None;
            }
            index += 1;
            continue;
        }
        if ch == '/' && next == Some('/') {
            index += 2;
            let mut ended = false;
            while index < chars.len() {
                if chars[index] == '\n' {
                    ended = true;
                    index += 1;
                    break;
                }
                index += 1;
            }
            if !ended {
                return true;
            }
            continue;
        }
        if ch == '/' && next == Some('*') {
            block = true;
            index += 2;
            continue;
        }
        if ch == '"' || ch == '\'' || ch == '`' {
            quote = Some(ch);
            index += 1;
            continue;
        }
        index += 1;
    }
    quote.is_some() || block
}

#[cfg(test)]
mod tests {
    use super::{ConsoleArg, Query, completion_query, origin_of_url, split_object, tag_object};

    #[test]
    fn child_object_ids_round_trip() {
        let mut arg = ConsoleArg::text("object", "Window");
        arg.object_id = "-4.1.7".into();
        let tagged = tag_object("8F1A", arg.clone());
        assert_eq!(split_object(&tagged.object_id), ("8F1A", "-4.1.7"));
        assert_eq!(tag_object("", arg).object_id, "-4.1.7");
        assert_eq!(split_object("{\"id\":2}"), ("", "{\"id\":2}"));
    }

    #[test]
    fn origin_of_url_keeps_scheme_and_host() {
        assert_eq!(
            origin_of_url("https://ads.example.com:8443/frame.html?x=1"),
            "https://ads.example.com:8443"
        );
        assert_eq!(origin_of_url("about:blank"), "");
    }

    #[test]
    fn completion_reads_a_member_and_a_global() {
        assert_eq!(
            completion_query("document.queryS"),
            Some(Query::Member {
                receiver: "document".into(),
                prefix: "queryS".into(),
            })
        );
        assert_eq!(
            completion_query("doc"),
            Some(Query::Global {
                prefix: "doc".into()
            })
        );
        assert_eq!(
            completion_query("items[0].na"),
            Some(Query::Member {
                receiver: "items[0]".into(),
                prefix: "na".into(),
            })
        );
    }

    #[test]
    fn completion_skips_strings_calls_and_empty_input() {
        assert_eq!(completion_query("\"hello"), None);
        assert_eq!(completion_query("obj.method().na"), None);
        assert_eq!(completion_query(""), None);
        assert_eq!(completion_query("let value = "), None);
        assert_eq!(completion_query("// document"), None);
    }
}
