//! Extension popouts opened with `chrome.windows.create`.
//!
//! WebView2 shows the window, then the extension API resolves with no window. Bitwarden's
//! passkey flow reads `.id` on that result and aborts (`Fido2UserInterfaceSession was closed`).
//! Each `chrome-extension:` popout is registered here. A script injected into the extension
//! service worker asks for it when `windows.create` comes back empty, and uses the same record
//! to focus, close, and report `windows.onRemoved`.
//!
//! The script also skips content-script ids Chromium already kept, so Bitwarden's
//! `fido2-page-script-registration` register call does not reject the whole batch.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine;
use rand::RngCore;
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};
use tiny_http::{Header, Method, Request, Response, Server};

use crate::tabs;

const POPOUT_BASE: i64 = 2_100_000_000;
const POPOUT_SPAN: i64 = 1_000_000;
const FRESH: Duration = Duration::from_secs(5);
const CLAIM_RETRY: Duration = Duration::from_secs(1);
const KEEP_CLOSED: Duration = Duration::from_secs(30);

struct Popout {
    id: i64,
    label: String,
    url: String,
    created: Instant,
    claimed: Option<Instant>,
    closed: Option<Instant>,
}

fn registry() -> &'static Mutex<Vec<Popout>> {
    static POPOUTS: OnceLock<Mutex<Vec<Popout>>> = OnceLock::new();
    POPOUTS.get_or_init(|| Mutex::new(Vec::new()))
}

fn next_id() -> i64 {
    static NEXT: AtomicI64 = AtomicI64::new(POPOUT_BASE + 1);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    if id >= POPOUT_BASE + POPOUT_SPAN {
        POPOUT_BASE + 1
    } else {
        id
    }
}

fn prune(popouts: &mut Vec<Popout>, now: Instant) {
    popouts.retain(|popout| match popout.closed {
        None => true,
        Some(closed) => now.duration_since(closed) < KEEP_CLOSED,
    });
}

/// Remember a `chrome-extension:` window created for `chrome.windows.create`.
pub fn track(label: &str, url: &str) {
    let mut popouts = registry().lock().expect("extension popouts");
    let now = Instant::now();
    prune(&mut popouts, now);
    if popouts
        .iter()
        .any(|popout| popout.label == label && popout.closed.is_none())
    {
        return;
    }
    popouts.push(Popout {
        id: next_id(),
        label: label.to_string(),
        url: url.to_string(),
        created: now,
        claimed: None,
        closed: None,
    });
}

/// The window is gone. `windows.onRemoved` is reported from the closed list.
pub fn closed(label: &str) {
    let mut popouts = registry().lock().expect("extension popouts");
    let now = Instant::now();
    for popout in popouts.iter_mut() {
        if popout.label == label && popout.closed.is_none() {
            popout.closed = Some(now);
        }
    }
    prune(&mut popouts, now);
}

/// The popout `windows.create` should return. A recent claim of the same URL is repeated
/// so a retry uses the same id.
fn claim_id(popouts: &mut [Popout], url: &str, now: Instant) -> Option<i64> {
    if let Some(index) = newest(popouts, |popout| {
        popout.closed.is_none()
            && popout
                .claimed
                .is_some_and(|claimed| now.duration_since(claimed) < CLAIM_RETRY)
            && url_matches(url, &popout.url)
    }) {
        return Some(popouts[index].id);
    }
    if let Some(index) = newest(popouts, |popout| {
        popout.closed.is_none() && popout.claimed.is_none() && url_matches(url, &popout.url)
    }) {
        popouts[index].claimed = Some(now);
        return Some(popouts[index].id);
    }
    if let Some(index) = newest(popouts, |popout| {
        popout.closed.is_none()
            && popout.claimed.is_none()
            && now.duration_since(popout.created) < FRESH
            && (url.is_empty() || popout.url.is_empty() || same_extension(url, &popout.url))
    }) {
        popouts[index].claimed = Some(now);
        return Some(popouts[index].id);
    }
    None
}

fn newest(popouts: &[Popout], pred: impl Fn(&Popout) -> bool) -> Option<usize> {
    popouts
        .iter()
        .enumerate()
        .filter(|(_, popout)| pred(popout))
        .max_by_key(|(_, popout)| popout.created)
        .map(|(index, _)| index)
}

fn without_hash(value: &str) -> &str {
    value.split('#').next().unwrap_or(value)
}

fn url_matches(requested: &str, stored: &str) -> bool {
    if requested.is_empty() || stored.is_empty() {
        return false;
    }
    requested == stored || without_hash(requested) == without_hash(stored)
}

fn extension_id(value: &str) -> Option<&str> {
    value
        .strip_prefix("chrome-extension://")
        .and_then(|rest| rest.split(['/', '?', '#']).next())
        .filter(|id| !id.is_empty())
}

fn same_extension(requested: &str, stored: &str) -> bool {
    match (extension_id(requested), extension_id(stored)) {
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

pub fn start(app: &AppHandle) {
    static STARTED: OnceLock<()> = OnceLock::new();
    let server = match Server::http(("127.0.0.1", 0)) {
        Ok(server) => server,
        Err(error) => {
            eprintln!("extension popouts: {error}");
            return;
        }
    };
    let port = server
        .server_addr()
        .to_ip()
        .map(|addr| addr.port())
        .unwrap_or(0);
    if port == 0 {
        eprintln!("extension popouts: no local port");
        return;
    }
    if STARTED.set(()).is_err() {
        return;
    }
    let _ = PORT.set(port);
    let app = app.clone();
    let _ = thread::Builder::new()
        .name("extension-popouts".into())
        .spawn(move || {
            for request in server.incoming_requests() {
                let app = app.clone();
                let _ = thread::Builder::new()
                    .name("extension-popout-request".into())
                    .spawn(move || handle(&app, request));
            }
        });
    let _ = thread::Builder::new()
        .name("extension-popout-patch".into())
        .spawn(move || patch_loop(port));
}

static PORT: OnceLock<u16> = OnceLock::new();

fn handle(app: &AppHandle, mut request: Request) {
    let origin = header(&request, "Origin");
    if !origin_ok(origin.as_deref()) {
        let _ = request.respond(Response::from_string(String::new()).with_status_code(403));
        return;
    }
    if request.method() == &Method::Options {
        let _ = request.respond(empty(204));
        return;
    }
    let url = request.url().to_string();
    let path = url.split('?').next().unwrap_or("");
    let body = read_body(&mut request);
    let response = match (request.method(), path) {
        (&Method::Get, "/claim") => {
            let wanted = query_param(&url, "url").unwrap_or_default();
            json_body(claim(app, &wanted))
        }
        (&Method::Get, "/windows") => json_body(json!(open_windows(app))),
        (&Method::Get, "/closed") => json_body(json!(closed_ids())),
        (&Method::Post, "/remove") => {
            if let Some(id) = json_id(&body) {
                remove(app, id);
            }
            json_body(json!({}))
        }
        (&Method::Post, "/update") => {
            if let Ok(value) = serde_json::from_str::<Value>(&body)
                && let Some(id) = value.get("id").and_then(Value::as_i64)
            {
                update(app, id, &value);
            }
            json_body(json!({}))
        }
        _ => Response::from_string(String::new()).with_status_code(404),
    };
    let _ = request.respond(response);
}

fn claim(app: &AppHandle, url: &str) -> Value {
    let id = {
        let mut popouts = registry().lock().expect("extension popouts");
        let now = Instant::now();
        prune(&mut popouts, now);
        claim_id(&mut popouts, url, now)
    };
    let Some(id) = id else {
        return Value::Null;
    };
    window_value(app, id).unwrap_or(Value::Null)
}

fn open_windows(app: &AppHandle) -> Vec<Value> {
    let ids: Vec<i64> = {
        let popouts = registry().lock().expect("extension popouts");
        popouts
            .iter()
            .filter(|popout| popout.closed.is_none())
            .map(|popout| popout.id)
            .collect()
    };
    ids.into_iter()
        .filter_map(|id| window_value(app, id))
        .collect()
}

fn closed_ids() -> Vec<i64> {
    let popouts = registry().lock().expect("extension popouts");
    let now = Instant::now();
    popouts
        .iter()
        .filter(|popout| {
            popout
                .closed
                .is_some_and(|closed| now.duration_since(closed) < KEEP_CLOSED)
        })
        .map(|popout| popout.id)
        .collect()
}

fn window_value(app: &AppHandle, id: i64) -> Option<Value> {
    let popouts = registry().lock().expect("extension popouts");
    let popout = popouts.iter().find(|popout| popout.id == id)?;
    if popout.closed.is_some() {
        return None;
    }
    let label = popout.label.clone();
    let url = popout.url.clone();
    drop(popouts);
    let (left, top, width, height, focused) = bounds(app, &label);
    Some(json!({
        "id": id,
        "focused": focused,
        "incognito": false,
        "type": "popup",
        "state": "normal",
        "alwaysOnTop": false,
        "left": left,
        "top": top,
        "width": width,
        "height": height,
        "tabs": [{
            "id": id,
            "windowId": id,
            "index": 0,
            "active": true,
            "highlighted": true,
            "pinned": false,
            "status": "complete",
            "incognito": false,
            "url": url,
            "title": "Extension"
        }]
    }))
}

fn bounds(app: &AppHandle, label: &str) -> (i64, i64, i64, i64, bool) {
    let Some(window) = app.get_window(label) else {
        return (0, 0, 480, 640, true);
    };
    let scale = window.scale_factor().unwrap_or(1.0);
    let (left, top) = window
        .outer_position()
        .map(|origin| (dip(origin.x, scale), dip(origin.y, scale)))
        .unwrap_or((0, 0));
    let (width, height) = window
        .outer_size()
        .map(|size| {
            (
                dip(size.width as i32, scale).max(1),
                dip(size.height as i32, scale).max(1),
            )
        })
        .unwrap_or((480, 640));
    let focused = window.is_focused().unwrap_or(false);
    (left, top, width, height, focused)
}

fn dip(physical: i32, scale: f64) -> i64 {
    if scale <= f64::EPSILON {
        return i64::from(physical);
    }
    (f64::from(physical) / scale).round() as i64
}

fn label_for(id: i64) -> Option<String> {
    let popouts = registry().lock().expect("extension popouts");
    popouts.iter().find_map(|popout| {
        (popout.id == id && popout.closed.is_none()).then(|| popout.label.clone())
    })
}

fn remove(app: &AppHandle, id: i64) {
    let Some(label) = label_for(id) else {
        return;
    };
    let host = app.clone();
    let name = label.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(window) = host.get_window(&name) {
            let _ = window.close();
        }
    });
    closed(&label);
}

fn update(app: &AppHandle, id: i64, value: &Value) {
    let Some(label) = label_for(id) else {
        return;
    };
    let focused = value.get("focused").and_then(Value::as_bool);
    let left = value.get("left").and_then(Value::as_f64);
    let top = value.get("top").and_then(Value::as_f64);
    let width = value.get("width").and_then(Value::as_f64);
    let height = value.get("height").and_then(Value::as_f64);
    let host = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Some(window) = host.get_window(&label) else {
            return;
        };
        if let (Some(left), Some(top)) = (left, top) {
            let _ = window.set_position(tauri::LogicalPosition::new(left, top));
        }
        if let (Some(width), Some(height)) = (width, height)
            && width >= 50.0
            && height >= 50.0
        {
            let _ = window.set_size(tauri::LogicalSize::new(width, height));
        }
        if focused == Some(true) {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
    });
}

fn json_id(body: &str) -> Option<i64> {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| value.get("id").and_then(Value::as_i64))
}

fn json_body(body: Value) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(body.to_string())
        .with_status_code(200)
        .with_header(header_line("Content-Type", "application/json"))
        .with_header(header_line("Access-Control-Allow-Origin", "*"))
        .with_header(header_line("Cache-Control", "no-store"))
}

fn empty(status: u16) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(String::new())
        .with_status_code(status)
        .with_header(header_line("Access-Control-Allow-Origin", "*"))
        .with_header(header_line(
            "Access-Control-Allow-Methods",
            "GET, POST, OPTIONS",
        ))
        .with_header(header_line("Access-Control-Allow-Headers", "Content-Type"))
        .with_header(header_line("Access-Control-Max-Age", "600"))
}

fn header_line(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("static header")
}

fn header(request: &Request, name: &str) -> Option<String> {
    request
        .headers()
        .iter()
        .find(|header| header.field.as_str().as_str().eq_ignore_ascii_case(name))
        .map(|header| header.value.as_str().to_string())
}

fn origin_ok(origin: Option<&str>) -> bool {
    match origin.map(str::trim).filter(|origin| !origin.is_empty()) {
        None | Some("null") => true,
        Some(origin) => origin.starts_with("chrome-extension://"),
    }
}

fn read_body(request: &mut Request) -> String {
    let mut body = String::new();
    let _ = request.as_reader().take(65_536).read_to_string(&mut body);
    body
}

fn query_param(url: &str, key: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    url::form_urlencoded::parse(query.as_bytes())
        .find_map(|(name, value)| (name == key).then(|| value.into_owned()))
}

fn patch_loop(port: u16) {
    let mut patched = std::collections::HashSet::new();
    loop {
        let ports = tabs::active_debugging_ports();
        let mut live = std::collections::HashSet::new();
        for debug_port in ports {
            let Ok(body) = fetch_targets(debug_port) else {
                continue;
            };
            let Ok(list) = serde_json::from_str::<Value>(&body) else {
                continue;
            };
            let Some(items) = list.as_array() else {
                continue;
            };
            for item in items {
                let Some(id) = item.get("id").and_then(Value::as_str) else {
                    continue;
                };
                let kind = item.get("type").and_then(Value::as_str).unwrap_or("");
                let url = item.get("url").and_then(Value::as_str).unwrap_or("");
                if !should_patch(kind, url) {
                    continue;
                }
                let key = format!("{debug_port}:{id}");
                live.insert(key.clone());
                if patched.contains(&key) {
                    continue;
                }
                if inject(debug_port, item, port) {
                    patched.insert(key);
                }
            }
        }
        patched.retain(|key| live.contains(key));
        thread::sleep(Duration::from_millis(500));
    }
}

fn should_patch(kind: &str, url: &str) -> bool {
    if !url.starts_with("chrome-extension:") {
        return false;
    }
    let kind = kind.to_ascii_lowercase();
    kind.contains("worker") || kind == "background_page" || kind == "page"
}

fn fetch_targets(port: u16) -> Result<String, ()> {
    ureq::get(&format!("http://127.0.0.1:{port}/json/list"))
        .timeout(Duration::from_secs(2))
        .call()
        .map_err(|_| ())?
        .into_string()
        .map_err(|_| ())
}

fn inject(debug_port: u16, target: &Value, api_port: u16) -> bool {
    let id = target.get("id").and_then(Value::as_str).unwrap_or("");
    let path = target
        .get("webSocketDebuggerUrl")
        .and_then(Value::as_str)
        .and_then(|url| {
            url.split_once("/devtools/")
                .map(|(_, rest)| format!("/devtools/{rest}"))
        })
        .unwrap_or_else(|| format!("/devtools/page/{id}"));
    let expression = patch_script(api_port);
    let message = json!({
        "id": 1,
        "method": "Runtime.evaluate",
        "params": { "expression": expression, "returnByValue": true }
    })
    .to_string();
    match evaluate(debug_port, &path, &message) {
        Ok(value) => value
            .pointer("/result/result/value")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        Err(_) => false,
    }
}

fn evaluate(port: u16, path: &str, message: &str) -> Result<Value, ()> {
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream =
        TcpStream::connect_timeout(&address, Duration::from_secs(2)).map_err(|_| ())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|_| ())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|_| ())?;
    let mut key = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut key);
    let key = base64::engine::general_purpose::STANDARD.encode(key);
    let handshake = format!(
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n"
    );
    stream.write_all(handshake.as_bytes()).map_err(|_| ())?;
    if !accepted(&mut stream) {
        return Err(());
    }
    stream
        .write_all(&ws_frame(0x1, message.as_bytes()))
        .map_err(|_| ())?;
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        let text = read_text(&mut stream)?;
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        if value.get("id").and_then(Value::as_i64) == Some(1) {
            return Ok(value);
        }
    }
    Err(())
}

fn accepted(stream: &mut TcpStream) -> bool {
    let mut bytes = Vec::new();
    let mut byte = [0u8; 1];
    while bytes.windows(4).all(|window| window != b"\r\n\r\n") {
        if bytes.len() > 8_192 || stream.read_exact(&mut byte).is_err() {
            return false;
        }
        bytes.push(byte[0]);
    }
    let text = String::from_utf8_lossy(&bytes);
    text.lines().next().is_some_and(|line| line.contains("101"))
}

fn read_text(stream: &mut TcpStream) -> Result<String, ()> {
    let mut message = Vec::new();
    loop {
        let (opcode, fin, data) = read_frame(stream)?;
        if opcode == 0x8 {
            return Err(());
        }
        if opcode == 0x9 {
            let _ = stream.write_all(&ws_frame(0xA, &data));
            continue;
        }
        if opcode == 0xA {
            continue;
        }
        if opcode == 0x1 || opcode == 0x2 || opcode == 0x0 {
            if message.len() + data.len() > 2_000_000 {
                return Err(());
            }
            message.extend_from_slice(&data);
            if fin {
                return String::from_utf8(message).map_err(|_| ());
            }
        }
    }
}

fn read_frame(stream: &mut TcpStream) -> Result<(u8, bool, Vec<u8>), ()> {
    let mut header = [0u8; 2];
    stream.read_exact(&mut header).map_err(|_| ())?;
    let opcode = header[0] & 0x0f;
    let fin = header[0] & 0x80 != 0;
    let masked = header[1] & 0x80 != 0;
    let mut len = u64::from(header[1] & 0x7f);
    if len == 126 {
        let mut extra = [0u8; 2];
        stream.read_exact(&mut extra).map_err(|_| ())?;
        len = u64::from(u16::from_be_bytes(extra));
    } else if len == 127 {
        let mut extra = [0u8; 8];
        stream.read_exact(&mut extra).map_err(|_| ())?;
        len = u64::from_be_bytes(extra);
    }
    if len > 2_000_000 {
        return Err(());
    }
    let mut mask = [0u8; 4];
    if masked {
        stream.read_exact(&mut mask).map_err(|_| ())?;
    }
    let mut data = vec![0u8; len as usize];
    stream.read_exact(&mut data).map_err(|_| ())?;
    if masked {
        for (index, byte) in data.iter_mut().enumerate() {
            *byte ^= mask[index % 4];
        }
    }
    Ok((opcode, fin, data))
}

fn ws_frame(opcode: u8, payload: &[u8]) -> Vec<u8> {
    let mut mask = [0u8; 4];
    rand::thread_rng().fill_bytes(&mut mask);
    let mut frame = Vec::with_capacity(payload.len() + 14);
    frame.push(0x80 | opcode);
    let len = payload.len();
    if len < 126 {
        frame.push(0x80 | len as u8);
    } else if len < 65_536 {
        frame.push(0x80 | 126);
        frame.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        frame.push(0x80 | 127);
        frame.extend_from_slice(&(len as u64).to_be_bytes());
    }
    frame.extend_from_slice(&mask);
    for (index, byte) in payload.iter().enumerate() {
        frame.push(byte ^ mask[index % 4]);
    }
    frame
}

fn patch_script(port: u16) -> String {
    PATCH.replace("__CE_PORT__", &port.to_string())
}

const PATCH: &str = r#"(function () {
  try {
    if (globalThis.__cePasskeyWindows) return true;
    if (!globalThis.chrome || !chrome.windows || !chrome.runtime) return false;
    var port = __CE_PORT__;
    var base = 2100000000;
    var removed = [];
    var seenClosed = {};
    function own(id) {
      return typeof id === "number" && id >= base && id < base + 1000000;
    }
    function host(path, options) {
      return fetch("http://127.0.0.1:" + port + path, options || { cache: "no-store" }).then(function (response) {
        if (!response.ok) return null;
        return response.json();
      }).catch(function () { return null; });
    }
    function claim(url) {
      var started = Date.now();
      function attempt() {
        return host("/claim?url=" + encodeURIComponent(url || "")).then(function (win) {
          if (win && typeof win.id === "number") return win;
          if (Date.now() - started > 2500) return null;
          return new Promise(function (resolve) { setTimeout(function () { resolve(attempt()); }, 40); });
        });
      }
      return attempt();
    }
    function patternMatch(pattern, url) {
      if (!pattern || !url) return false;
      if (pattern === "<all_urls>") return /^(https?|file|ftp):/i.test(url);
      var source = "^" + String(pattern).replace(/[.+?^${}()|[\]\\]/g, "\\$&").replace(/\\\*/g, ".*") + "$";
      try { return new RegExp(source).test(url); } catch (error) { return false; }
    }
    function tabMatches(tab, info) {
      if (!info) return true;
      if (info.active === true || info.currentWindow === true || info.lastFocusedWindow === true) return false;
      if (typeof info.windowId === "number" && info.windowId !== tab.windowId) return false;
      if (info.url) {
        var patterns = Array.isArray(info.url) ? info.url : [info.url];
        var hit = false;
        for (var i = 0; i < patterns.length; i++) if (patternMatch(patterns[i], tab.url || "")) hit = true;
        if (!hit) return false;
      }
      return true;
    }
    function extraTabs(info) {
      return host("/windows").then(function (windows) {
        var tabs = [];
        (windows || []).forEach(function (item) {
          (item.tabs || []).forEach(function (tab) {
            if (tabMatches(tab, info)) tabs.push(tab);
          });
        });
        return tabs;
      });
    }
    if (chrome.scripting && typeof chrome.scripting.registerContentScripts === "function" && !chrome.scripting.registerContentScripts.__cePasskey) {
      var nativeRegister = chrome.scripting.registerContentScripts.bind(chrome.scripting);
      function register(scripts) {
        var list = scripts || [];
        function one(index) {
          if (index >= list.length) return;
          return Promise.resolve(nativeRegister([list[index]])).then(function () {
            return one(index + 1);
          }, function (error) {
            var text = String(error && error.message || error || "");
            if (text.indexOf("Duplicate script ID") === -1) throw error;
            return one(index + 1);
          });
        }
        return one(0);
      }
      register.__cePasskey = true;
      try { chrome.scripting.registerContentScripts = register; } catch (error) {}
    }
    if (typeof chrome.windows.create === "function" && !chrome.windows.create.__cePasskey) {
      var nativeCreate = chrome.windows.create.bind(chrome.windows);
      function round(data) {
        ["left", "top", "width", "height"].forEach(function (key) {
          if (data[key] == null) return;
          var value = Number(data[key]);
          if (!isFinite(value)) delete data[key];
          else data[key] = Math.round(value);
        });
        return data;
      }
      function finish(win, url) {
        if (win && typeof win.id === "number") return Promise.resolve(win);
        return claim(url);
      }
      function createWindow(createData, callback) {
        if (typeof createData === "function") {
          callback = createData;
          createData = {};
        }
        var data = round(Object.assign({}, createData || {}));
        if (typeof callback === "function") {
          var settled = false;
          function done(win) {
            if (settled) return;
            settled = true;
            clearTimeout(timer);
            finish(win, data.url).then(function (value) { callback(value || undefined); }, function () { callback(undefined); });
          }
          var timer = setTimeout(function () { done(null); }, 800);
          try {
            var result = nativeCreate(data, function (win) { done(win); });
            if (result && typeof result.then === "function") {
              result.then(function (win) { done(win); }, function () { done(null); });
            }
          } catch (error) {
            done(null);
          }
          return undefined;
        }
        try {
          var promised = nativeCreate(data);
          if (promised && typeof promised.then === "function") {
            return promised.then(function (win) { return finish(win, data.url); }, function () { return claim(data.url); });
          }
          return finish(promised, data.url);
        } catch (error) {
          return claim(data.url);
        }
      }
      createWindow.__cePasskey = true;
      try { chrome.windows.create = createWindow; } catch (error) {}
    }
    if (typeof chrome.windows.get === "function" && !chrome.windows.get.__cePasskey) {
      var nativeGet = chrome.windows.get.bind(chrome.windows);
      function getWindow(windowId, getInfo, callback) {
        if (typeof getInfo === "function") {
          callback = getInfo;
          getInfo = {};
        }
        function deliver(win) {
          if (typeof callback === "function") {
            callback(win);
            return undefined;
          }
          return Promise.resolve(win);
        }
        if (own(windowId)) {
          var hosted = host("/windows").then(function (windows) {
            var found = null;
            (windows || []).forEach(function (item) { if (item && item.id === windowId) found = item; });
            return found || undefined;
          });
          if (typeof callback === "function") {
            hosted.then(function (win) { callback(win); });
            return undefined;
          }
          return hosted;
        }
        if (typeof callback === "function") {
          try {
            nativeGet(windowId, getInfo || {}, function (win) {
              if (chrome.runtime.lastError) win = undefined;
              callback(win);
            });
          } catch (error) {
            callback(undefined);
          }
          return undefined;
        }
        return new Promise(function (resolve) {
          try {
            var result = nativeGet(windowId, getInfo || {}, function (win) {
              if (chrome.runtime.lastError) win = undefined;
              resolve(win);
            });
            if (result && typeof result.then === "function") {
              result.then(function (win) { resolve(win); }, function () { resolve(undefined); });
            }
          } catch (error) {
            resolve(undefined);
          }
        });
      }
      getWindow.__cePasskey = true;
      try { chrome.windows.get = getWindow; } catch (error) {}
    }
    if (typeof chrome.windows.remove === "function" && !chrome.windows.remove.__cePasskey) {
      var nativeRemove = chrome.windows.remove.bind(chrome.windows);
      function removeWindow(windowId, callback) {
        function done() {
          if (chrome.runtime.lastError) {}
          if (typeof callback === "function") callback();
        }
        if (own(windowId)) {
          host("/remove", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ id: windowId }) }).then(done, done);
          return;
        }
        try { return nativeRemove(windowId, done); } catch (error) { done(); }
      }
      removeWindow.__cePasskey = true;
      try { chrome.windows.remove = removeWindow; } catch (error) {}
    }
    if (typeof chrome.windows.update === "function" && !chrome.windows.update.__cePasskey) {
      var nativeUpdate = chrome.windows.update.bind(chrome.windows);
      function updateWindow(windowId, info, callback) {
        function done(win) {
          if (chrome.runtime.lastError) {}
          if (typeof callback === "function") callback(win);
        }
        if (own(windowId)) {
          var body = Object.assign({ id: windowId }, info || {});
          host("/update", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) }).then(function () { done(); }, done);
          return;
        }
        try { return nativeUpdate(windowId, info || {}, done); } catch (error) { done(); }
      }
      updateWindow.__cePasskey = true;
      try { chrome.windows.update = updateWindow; } catch (error) {}
    }
    if (chrome.windows.onRemoved && typeof chrome.windows.onRemoved.addListener === "function" && !chrome.windows.onRemoved.addListener.__cePasskey) {
      var nativeAdd = chrome.windows.onRemoved.addListener.bind(chrome.windows.onRemoved);
      function addRemoved(listener) {
        if (typeof listener === "function") removed.push(listener);
        try { return nativeAdd(listener); } catch (error) {}
      }
      addRemoved.__cePasskey = true;
      try { chrome.windows.onRemoved.addListener = addRemoved; } catch (error) {}
      setInterval(function () {
        host("/closed").then(function (ids) {
          (ids || []).forEach(function (id) {
            if (seenClosed[id]) return;
            seenClosed[id] = true;
            removed.slice().forEach(function (listener) {
              try { listener(id); } catch (error) {}
            });
          });
        });
      }, 400);
    }
    if (chrome.tabs && typeof chrome.tabs.query === "function" && !chrome.tabs.query.__cePasskey) {
      var nativeQuery = chrome.tabs.query.bind(chrome.tabs);
      function queryTabs(info, callback) {
        if (typeof info === "function") {
          callback = info;
          info = {};
        }
        function merge(tabs) {
          return extraTabs(info || {}).then(function (extra) {
            var all = (tabs || []).slice();
            extra.forEach(function (tab) {
              var seen = false;
              for (var i = 0; i < all.length; i++) if (all[i] && all[i].id === tab.id) seen = true;
              if (!seen) all.push(tab);
            });
            return all;
          });
        }
        if (typeof callback === "function") {
          try {
            nativeQuery(info || {}, function (tabs) {
              if (chrome.runtime.lastError) tabs = [];
              merge(tabs).then(function (all) { callback(all); }, function () { callback(tabs || []); });
            });
          } catch (error) {
            merge([]).then(function (all) { callback(all); }, function () { callback([]); });
          }
          return undefined;
        }
        try {
          var result = nativeQuery(info || {});
          if (result && typeof result.then === "function") return result.then(merge, function () { return merge([]); });
          return merge(result || []);
        } catch (error) {
          return merge([]);
        }
      }
      queryTabs.__cePasskey = true;
      try { chrome.tabs.query = queryTabs; } catch (error) {}
    }
    if (chrome.tabs && typeof chrome.tabs.get === "function" && !chrome.tabs.get.__cePasskey) {
      var nativeTabGet = chrome.tabs.get.bind(chrome.tabs);
      function getTab(tabId, callback) {
        function deliver(tab) {
          if (typeof callback === "function") {
            callback(tab);
            return undefined;
          }
          return Promise.resolve(tab);
        }
        if (own(tabId)) {
          var found = host("/windows").then(function (windows) {
            var tab = undefined;
            (windows || []).forEach(function (item) {
              (item.tabs || []).forEach(function (itemTab) { if (itemTab && itemTab.id === tabId) tab = itemTab; });
            });
            return tab;
          });
          if (typeof callback === "function") {
            found.then(function (tab) { callback(tab); });
            return undefined;
          }
          return found;
        }
        if (typeof callback === "function") {
          try {
            return nativeTabGet(tabId, function (tab) {
              if (chrome.runtime.lastError) tab = undefined;
              callback(tab);
            });
          } catch (error) {
            callback(undefined);
          }
          return undefined;
        }
        return new Promise(function (resolve) {
          try {
            var result = nativeTabGet(tabId, function (tab) {
              if (chrome.runtime.lastError) tab = undefined;
              resolve(tab);
            });
            if (result && typeof result.then === "function") result.then(resolve, function () { resolve(undefined); });
          } catch (error) {
            resolve(undefined);
          }
        });
      }
      getTab.__cePasskey = true;
      try { chrome.tabs.get = getTab; } catch (error) {}
    }
    globalThis.__cePasskeyWindows = true;
    return true;
  } catch (error) {
    return false;
  }
})()"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: i64, url: &str, age: Duration, claimed: bool) -> Popout {
        Popout {
            id,
            label: format!("script-{id}"),
            url: url.to_string(),
            created: Instant::now() - age,
            claimed: claimed.then(Instant::now),
            closed: None,
        }
    }

    #[test]
    fn claim_uses_the_matching_unclaimed_popout() {
        let mut popouts = vec![
            sample(
                1,
                "chrome-extension://bw/popup/index.html?uilocation=popout#/fido2",
                Duration::from_millis(200),
                false,
            ),
            sample(
                2,
                "chrome-extension://other/app.html",
                Duration::from_millis(50),
                false,
            ),
        ];
        let id = claim_id(
            &mut popouts,
            "chrome-extension://bw/popup/index.html?uilocation=popout#/fido2?sessionId=1",
            Instant::now(),
        );
        assert_eq!(id, Some(1));
        assert!(popouts[0].claimed.is_some());
    }

    #[test]
    fn claim_ignores_the_hash_when_the_query_matches() {
        let mut popouts = vec![sample(
            4,
            "chrome-extension://bw/popup/index.html?singleActionPopout=vault_Fido2Popout_abc",
            Duration::from_millis(10),
            false,
        )];
        let id = claim_id(
            &mut popouts,
            "chrome-extension://bw/popup/index.html?singleActionPopout=vault_Fido2Popout_abc#/fido2",
            Instant::now(),
        );
        assert_eq!(id, Some(4));
    }

    #[test]
    fn a_second_claim_of_the_same_url_keeps_the_id() {
        let mut popouts = vec![sample(
            7,
            "chrome-extension://bw/popup/index.html",
            Duration::from_millis(20),
            false,
        )];
        let now = Instant::now();
        assert_eq!(
            claim_id(&mut popouts, "chrome-extension://bw/popup/index.html", now),
            Some(7)
        );
        assert_eq!(
            claim_id(&mut popouts, "chrome-extension://bw/popup/index.html", now),
            Some(7)
        );
    }

    #[test]
    fn a_fresh_popout_is_used_when_the_opener_url_dropped_the_query() {
        let mut popouts = vec![sample(
            9,
            "chrome-extension://bw/popup/index.html?x=1",
            Duration::from_millis(30),
            false,
        )];
        let id = claim_id(
            &mut popouts,
            "chrome-extension://bw/other.html",
            Instant::now(),
        );
        assert_eq!(id, Some(9));
    }

    #[test]
    fn patch_mentions_the_local_port_and_the_duplicate_script() {
        let script = patch_script(1427);
        assert!(script.contains("var port = 1427;"));
        assert!(script.contains("Duplicate script ID"));
        assert!(!script.contains("__CE_PORT__"));
    }
}
