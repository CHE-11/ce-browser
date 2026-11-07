//! Drives one tab's page through the DevTools Protocol. Every call blocks on the main
//! thread, so run these from a worker thread.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Webview};

use crate::{ocr, tabs, webview2};

pub const PAGE_SCRIPT: &str = concat!(include_str!("locators.js"), "\n", include_str!("page.js"));
const WORLD_NAME: &str = "ce-automation";
const MISSING: &str = "__ce_missing__";
const CALL_TIMEOUT: Duration = Duration::from_secs(15);
const FAILURE: &str = "The page did not respond.";
/// Screenshots sent to a model are scaled down to this width.
pub const SHOT_WIDTH: u32 = 1280;

/// Isolated world context per tab. `None` means the page refused one and the main world is used.
fn worlds() -> &'static Mutex<HashMap<u32, Option<i64>>> {
    static WORLDS: OnceLock<Mutex<HashMap<u32, Option<i64>>>> = OnceLock::new();
    WORLDS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PageSnapshot {
    pub url: String,
    pub title: String,
    pub width: f64,
    pub height: f64,
    pub scroll_y: f64,
    pub scroll_height: f64,
    pub focused: Option<u32>,
    pub count: u32,
    pub total: u32,
    pub text: String,
}

impl PageSnapshot {
    /// The snapshot as the agent reads it.
    pub fn render(&self) -> String {
        let mut out = format!("URL: {}\nTitle: {}\n", self.url, self.title);
        let pages = if self.height > 0.0 {
            (self.scroll_height / self.height).ceil().max(1.0)
        } else {
            1.0
        };
        let at = if self.height > 0.0 {
            (self.scroll_y / self.height).floor() + 1.0
        } else {
            1.0
        };
        out.push_str(&format!(
            "Viewport {}x{}, screen {at} of {pages}.\n",
            self.width.round(),
            self.height.round()
        ));
        if let Some(focused) = self.focused {
            out.push_str(&format!("Focused element: [{focused}]\n"));
        }
        if self.total > self.count {
            out.push_str(&format!(
                "Showing {} of {} interactive elements. Scroll to see the rest.\n",
                self.count, self.total
            ));
        }
        out.push('\n');
        out.push_str(&self.text);
        out
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct Point {
    x: f64,
    y: f64,
    ok: bool,
    cover: String,
}

/// Facts about one element, used by the action policy.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ElementInfo {
    pub tag: String,
    pub role: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub autocomplete: String,
    pub href: String,
    pub submits: Vec<String>,
    pub payment: bool,
    pub editable: bool,
    pub file: bool,
    pub r#ref: Option<u32>,
}

pub fn view(app: &AppHandle, id: u32) -> Result<Webview, String> {
    if let Some(webview) = tabs::page_view(app, id) {
        return Ok(webview);
    }
    if tabs::tab_info(app, id).is_none() {
        return Err("That tab is closed.".into());
    }
    tabs::ensure_page_view(app, id)?;
    for _ in 0..40 {
        if let Some(webview) = tabs::page_view(app, id) {
            return Ok(webview);
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err("The tab's page did not open.".into())
}

pub fn cdp(app: &AppHandle, id: u32, method: &str, params: Value) -> Result<Value, String> {
    let webview = view(app, id)?;
    webview2::call_cdp_json(&webview, method, &params.to_string(), CALL_TIMEOUT, FAILURE)
}

fn create_world(webview: &Webview) -> Option<i64> {
    let tree =
        webview2::call_cdp_json(webview, "Page.getFrameTree", "{}", CALL_TIMEOUT, FAILURE).ok()?;
    let frame = tree.pointer("/frameTree/frame/id")?.as_str()?.to_string();
    let params = json!({ "frameId": frame, "worldName": WORLD_NAME }).to_string();
    let created = webview2::call_cdp_json(
        webview,
        "Page.createIsolatedWorld",
        &params,
        CALL_TIMEOUT,
        FAILURE,
    )
    .ok()?;
    created.get("executionContextId")?.as_i64()
}

fn context(webview: &Webview, id: u32) -> Option<i64> {
    if let Some(known) = worlds().lock().expect("worlds lock").get(&id).copied() {
        return known;
    }
    let created = create_world(webview);
    worlds().lock().expect("worlds lock").insert(id, created);
    created
}

pub fn forget(id: u32) {
    worlds().lock().expect("worlds lock").remove(&id);
}

fn run_in(
    webview: &Webview,
    context: Option<i64>,
    expression: &str,
    by_value: bool,
    timeout: Duration,
) -> Result<Value, String> {
    let mut params = json!({
        "expression": expression,
        "returnByValue": by_value,
        "awaitPromise": true,
        "userGesture": true,
    });
    if let Some(context) = context {
        params["contextId"] = json!(context);
    }
    let mut response = webview2::call_cdp_json(
        webview,
        "Runtime.evaluate",
        &params.to_string(),
        timeout,
        FAILURE,
    )?;
    if let Some(details) = response.get("exceptionDetails") {
        let text = details
            .pointer("/exception/description")
            .or_else(|| details.get("text"))
            .and_then(Value::as_str)
            .unwrap_or("The page script failed.");
        let first = text.lines().next().unwrap_or(text);
        return Err(first.trim_start_matches("Error: ").to_string());
    }
    if by_value {
        Ok(response
            .pointer_mut("/result/value")
            .map(Value::take)
            .unwrap_or(Value::Null))
    } else {
        Ok(response
            .get_mut("result")
            .map(Value::take)
            .unwrap_or(Value::Null))
    }
}

fn stale_context(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("cannot find context") || lower.contains("execution context was destroyed")
}

fn guarded(call: &str) -> String {
    format!("globalThis.__ce ? ({call}) : \"{MISSING}\"")
}

/// Calls `__ce.<call>` in the tab's automation world, loading the page script when needed.
fn call_with(
    app: &AppHandle,
    id: u32,
    call: &str,
    by_value: bool,
    timeout: Duration,
) -> Result<Value, String> {
    let webview = view(app, id)?;
    let expression = guarded(call);
    for attempt in 0..3 {
        let world = context(&webview, id);
        let result = run_in(&webview, world, &expression, by_value, timeout);
        match result {
            Ok(value)
                if value.as_str() == Some(MISSING)
                    || value.get("value").and_then(Value::as_str) == Some(MISSING) =>
            {
                run_in(&webview, world, PAGE_SCRIPT, true, CALL_TIMEOUT).or_else(|error| {
                    if stale_context(&error) {
                        Ok(Value::Null)
                    } else {
                        Err(error)
                    }
                })?;
                if attempt == 2 {
                    break;
                }
            }
            Ok(value) => return Ok(value),
            Err(error) if stale_context(&error) && attempt < 2 => {
                forget(id);
            }
            Err(error) => return Err(error),
        }
    }
    Err("The page is still loading. Try again.".into())
}

fn call(app: &AppHandle, id: u32, call: &str) -> Result<Value, String> {
    call_with(app, id, call, true, CALL_TIMEOUT)
}

/// Runs an expression in the automation world, out of the page's reach, and awaits it.
pub fn evaluate_world(
    app: &AppHandle,
    id: u32,
    expression: &str,
    timeout: Duration,
) -> Result<Value, String> {
    call_with(app, id, expression, true, timeout)
}

pub fn snapshot(app: &AppHandle, id: u32) -> Result<PageSnapshot, String> {
    let value = call(app, id, "__ce.snapshot({})")?;
    serde_json::from_value(value).map_err(|error| error.to_string())
}

pub fn element_info(app: &AppHandle, id: u32, element: u32) -> Result<ElementInfo, String> {
    let value = call(app, id, &format!("__ce.info({element})"))?;
    serde_json::from_value(value).map_err(|error| error.to_string())
}

pub fn focused_info(app: &AppHandle, id: u32) -> Result<Option<ElementInfo>, String> {
    let value = call(app, id, "__ce.focused()")?;
    if value.is_null() {
        return Ok(None);
    }
    serde_json::from_value(value)
        .map(Some)
        .map_err(|error| error.to_string())
}

fn mouse(
    app: &AppHandle,
    id: u32,
    kind: &str,
    x: f64,
    y: f64,
    button: &str,
    clicks: u32,
) -> Result<(), String> {
    cdp(
        app,
        id,
        "Input.dispatchMouseEvent",
        json!({
            "type": kind,
            "x": x,
            "y": y,
            "button": button,
            "buttons": if kind == "mousePressed" { button_bits(button) } else { 0 },
            "clickCount": clicks,
        }),
    )
    .map(|_| ())
}

fn button_bits(button: &str) -> u32 {
    match button {
        "right" => 2,
        "middle" => 4,
        _ => 1,
    }
}

/// Clicks a point in the page's viewport, in CSS pixels.
pub fn click_at(
    app: &AppHandle,
    id: u32,
    x: f64,
    y: f64,
    button: &str,
    double: bool,
) -> Result<(), String> {
    let button = match button {
        "right" | "middle" => button,
        _ => "left",
    };
    mouse(app, id, "mouseMoved", x, y, "none", 0)?;
    thread::sleep(Duration::from_millis(30));
    let rounds = if double { 2 } else { 1 };
    for count in 1..=rounds {
        mouse(app, id, "mousePressed", x, y, button, count)?;
        thread::sleep(Duration::from_millis(40));
        mouse(app, id, "mouseReleased", x, y, button, count)?;
    }
    Ok(())
}

fn point(app: &AppHandle, id: u32, element: u32) -> Result<Point, String> {
    let value = call(app, id, &format!("__ce.point({element})"))?;
    serde_json::from_value(value).map_err(|error| error.to_string())
}

/// Scrolls the element into view and clicks its centre with a trusted mouse event.
pub fn click(
    app: &AppHandle,
    id: u32,
    element: u32,
    button: &str,
    double: bool,
) -> Result<(), String> {
    let target = point(app, id, element)?;
    if !target.ok {
        let cover = if target.cover.is_empty() {
            "another element".to_string()
        } else {
            target.cover
        };
        return Err(format!(
            "Element {element} is covered by {cover}. Close or scroll past it first."
        ));
    }
    click_at(app, id, target.x, target.y, button, double)
}

pub fn hover(app: &AppHandle, id: u32, element: u32) -> Result<(), String> {
    let target = point(app, id, element)?;
    mouse(app, id, "mouseMoved", target.x, target.y, "none", 0)
}

/// Types into the element, or into the focused field when `element` is empty.
pub fn type_text(
    app: &AppHandle,
    id: u32,
    element: Option<u32>,
    text: &str,
    clear: bool,
    submit: bool,
) -> Result<(), String> {
    if let Some(element) = element {
        if click(app, id, element, "left", false).is_err() {
            call(app, id, &format!("__ce.focus({element})"))?;
        }
        thread::sleep(Duration::from_millis(60));
        if clear {
            call(app, id, &format!("__ce.selectContents({element})"))?;
        }
    } else if clear {
        press(app, id, "Control+A")?;
    }
    if !text.is_empty() {
        cdp(app, id, "Input.insertText", json!({ "text": text }))?;
    }
    if submit {
        thread::sleep(Duration::from_millis(60));
        press(app, id, "Enter")?;
    }
    Ok(())
}

struct Key {
    key: String,
    code: String,
    vk: u32,
    text: Option<String>,
}

fn named_key(name: &str) -> Option<Key> {
    let (key, code, vk) = match name.to_ascii_lowercase().as_str() {
        "enter" | "return" => ("Enter", "Enter", 13),
        "tab" => ("Tab", "Tab", 9),
        "escape" | "esc" => ("Escape", "Escape", 27),
        "backspace" => ("Backspace", "Backspace", 8),
        "delete" | "del" => ("Delete", "Delete", 46),
        "space" | " " => (" ", "Space", 32),
        "arrowup" | "up" => ("ArrowUp", "ArrowUp", 38),
        "arrowdown" | "down" => ("ArrowDown", "ArrowDown", 40),
        "arrowleft" | "left" => ("ArrowLeft", "ArrowLeft", 37),
        "arrowright" | "right" => ("ArrowRight", "ArrowRight", 39),
        "home" => ("Home", "Home", 36),
        "end" => ("End", "End", 35),
        "pageup" => ("PageUp", "PageUp", 33),
        "pagedown" => ("PageDown", "PageDown", 34),
        "insert" => ("Insert", "Insert", 45),
        "f1" => ("F1", "F1", 112),
        "f2" => ("F2", "F2", 113),
        "f3" => ("F3", "F3", 114),
        "f4" => ("F4", "F4", 115),
        "f5" => ("F5", "F5", 116),
        "f6" => ("F6", "F6", 117),
        "f7" => ("F7", "F7", 118),
        "f8" => ("F8", "F8", 119),
        "f9" => ("F9", "F9", 120),
        "f10" => ("F10", "F10", 121),
        "f11" => ("F11", "F11", 122),
        "f12" => ("F12", "F12", 123),
        _ => return None,
    };
    let text = match key {
        "Enter" => Some("\r".to_string()),
        " " => Some(" ".to_string()),
        _ => None,
    };
    Some(Key {
        key: key.into(),
        code: code.into(),
        vk,
        text,
    })
}

fn char_key(ch: char, shift: bool) -> Key {
    let upper = ch.to_ascii_uppercase();
    let (code, vk) = if upper.is_ascii_alphabetic() {
        (format!("Key{upper}"), upper as u32)
    } else if ch.is_ascii_digit() {
        (format!("Digit{ch}"), ch as u32)
    } else {
        let (code, vk) = match ch {
            '-' | '_' => ("Minus", 189),
            '=' | '+' => ("Equal", 187),
            '[' | '{' => ("BracketLeft", 219),
            ']' | '}' => ("BracketRight", 221),
            ';' | ':' => ("Semicolon", 186),
            '\'' | '"' => ("Quote", 222),
            ',' | '<' => ("Comma", 188),
            '.' | '>' => ("Period", 190),
            '/' | '?' => ("Slash", 191),
            '\\' | '|' => ("Backslash", 220),
            '`' | '~' => ("Backquote", 192),
            _ => ("", 0),
        };
        (code.to_string(), vk)
    };
    let shown = if shift { upper } else { ch };
    Key {
        key: shown.to_string(),
        code,
        vk,
        text: Some(shown.to_string()),
    }
}

/// Presses a key or chord such as `Enter`, `Tab`, `Control+A` or `Shift+ArrowDown`.
pub fn press(app: &AppHandle, id: u32, keys: &str) -> Result<(), String> {
    let (down, up) = key_events(keys)?;
    cdp(app, id, "Input.dispatchKeyEvent", down)?;
    cdp(app, id, "Input.dispatchKeyEvent", up)?;
    Ok(())
}

/// The `Input.dispatchKeyEvent` params that press and release `keys`.
pub fn key_events(keys: &str) -> Result<(Value, Value), String> {
    let parts: Vec<&str> = keys
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    let Some((last, mods)) = parts.split_last() else {
        return Err("Name a key to press.".into());
    };
    let mut modifiers = 0u32;
    for name in mods {
        modifiers |= match name.to_ascii_lowercase().as_str() {
            "alt" => 1,
            "ctrl" | "control" => 2,
            "meta" | "cmd" | "win" => 4,
            "shift" => 8,
            other => return Err(format!("Unknown modifier key \"{other}\".")),
        };
    }
    let key = match named_key(last) {
        Some(key) => key,
        None => {
            let mut chars = last.chars();
            match (chars.next(), chars.next()) {
                (Some(ch), None) => char_key(ch, modifiers & 8 != 0),
                _ => return Err(format!("Unknown key \"{last}\".")),
            }
        }
    };
    // Chords with Ctrl, Alt or Meta are commands, not text.
    let text = if modifiers & 7 != 0 {
        None
    } else {
        key.text.clone()
    };
    let mut down = json!({
        "type": if text.is_some() { "keyDown" } else { "rawKeyDown" },
        "modifiers": modifiers,
        "key": key.key,
        "code": key.code,
        "windowsVirtualKeyCode": key.vk,
        "nativeVirtualKeyCode": key.vk,
    });
    if let Some(text) = &text {
        down["text"] = json!(text);
        down["unmodifiedText"] = json!(text);
    }
    let up = json!({
        "type": "keyUp",
        "modifiers": modifiers,
        "key": key.key,
        "code": key.code,
        "windowsVirtualKeyCode": key.vk,
        "nativeVirtualKeyCode": key.vk,
    });
    Ok((down, up))
}

fn viewport(app: &AppHandle, id: u32) -> Result<(f64, f64), String> {
    let value = call(app, id, "[innerWidth, innerHeight]")?;
    let width = value.get(0).and_then(Value::as_f64).unwrap_or(800.0);
    let height = value.get(1).and_then(Value::as_f64).unwrap_or(600.0);
    Ok((width, height))
}

/// Scrolls with the mouse wheel. `screens` is how many viewport heights to move; negative scrolls up.
pub fn scroll(
    app: &AppHandle,
    id: u32,
    element: Option<u32>,
    screens: f64,
    horizontal: bool,
) -> Result<String, String> {
    let (width, height) = viewport(app, id)?;
    let (x, y) = match element {
        Some(element) => {
            let rect = call(app, id, &format!("__ce.rectOf({element})"))?;
            let x = rect.get("x").and_then(Value::as_f64).unwrap_or(0.0)
                + rect.get("width").and_then(Value::as_f64).unwrap_or(0.0) / 2.0;
            let y = rect.get("y").and_then(Value::as_f64).unwrap_or(0.0)
                + rect.get("height").and_then(Value::as_f64).unwrap_or(0.0) / 2.0;
            (x.clamp(1.0, width - 1.0), y.clamp(1.0, height - 1.0))
        }
        None => (width / 2.0, height / 2.0),
    };
    let distance = screens * if horizontal { width } else { height } * 0.85;
    let before = call(
        app,
        id,
        "(document.scrollingElement||document.documentElement).scrollTop",
    )?
    .as_f64()
    .unwrap_or(0.0);
    cdp(
        app,
        id,
        "Input.dispatchMouseEvent",
        json!({
            "type": "mouseWheel",
            "x": x,
            "y": y,
            "deltaX": if horizontal { distance } else { 0.0 },
            "deltaY": if horizontal { 0.0 } else { distance },
        }),
    )?;
    thread::sleep(Duration::from_millis(350));
    if element.is_none() && !horizontal {
        let after = call(
            app,
            id,
            "(document.scrollingElement||document.documentElement).scrollTop",
        )?
        .as_f64()
        .unwrap_or(0.0);
        if (after - before).abs() < 1.0 {
            let moved = call(app, id, &format!("__ce.scrollBy(0, {distance})"))?;
            if moved.get("moved").and_then(Value::as_bool) != Some(true) {
                return Ok(if distance > 0.0 {
                    "Already at the bottom of the page.".into()
                } else {
                    "Already at the top of the page.".into()
                });
            }
        }
    }
    Ok("Scrolled.".into())
}

pub fn select(
    app: &AppHandle,
    id: u32,
    element: u32,
    values: &[String],
) -> Result<Vec<String>, String> {
    let list = serde_json::to_string(values).map_err(|error| error.to_string())?;
    let value = call(app, id, &format!("__ce.choose({element}, {list})"))?;
    Ok(value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default())
}

/// Puts local files into a file input without opening the file dialog.
pub fn upload(app: &AppHandle, id: u32, element: u32, files: &[String]) -> Result<(), String> {
    for file in files {
        if !std::path::Path::new(file).is_file() {
            return Err(format!("{file} is not a file."));
        }
    }
    let info = element_info(app, id, element)?;
    if !info.file {
        return Err(format!("Element {element} is not a file input."));
    }
    let handle = call_with(
        app,
        id,
        &format!("__ce.element({element})"),
        false,
        CALL_TIMEOUT,
    )?;
    let object = handle
        .get("objectId")
        .and_then(Value::as_str)
        .ok_or("Couldn't reach the file input.")?;
    cdp(
        app,
        id,
        "DOM.setFileInputFiles",
        json!({ "files": files, "objectId": object }),
    )?;
    Ok(())
}

pub fn navigate(app: &AppHandle, id: u32, url: &str) -> Result<(), String> {
    tabs::navigate(app, id, url)?;
    forget(id);
    wait_load(app, id, Duration::from_secs(20));
    Ok(())
}

pub fn back(app: &AppHandle, id: u32) -> Result<(), String> {
    let info = tabs::tab_info(app, id).ok_or("That tab is closed.")?;
    if !info.can_go_back {
        return Err("There is no earlier page in this tab.".into());
    }
    tabs::go_back(app, id);
    forget(id);
    wait_load(app, id, Duration::from_secs(20));
    Ok(())
}

/// Waits for the document to finish and the network to go quiet. Gives up quietly at `limit`.
pub fn wait_load(app: &AppHandle, id: u32, limit: Duration) {
    let start = Instant::now();
    thread::sleep(Duration::from_millis(200));
    let mut last_resources = -1i64;
    let mut quiet = 0;
    while start.elapsed() < limit {
        let loading = tabs::tab_info(app, id).is_none_or(|info| info.loading);
        let state = call(app, id, "__ce.idle()").ok();
        let ready = state
            .as_ref()
            .and_then(|state| state.get("ready"))
            .and_then(Value::as_str)
            == Some("complete");
        let resources = state
            .as_ref()
            .and_then(|state| state.get("resources"))
            .and_then(Value::as_i64)
            .unwrap_or(-1);
        if !loading && ready && resources == last_resources {
            quiet += 1;
            if quiet >= 2 {
                return;
            }
        } else {
            quiet = 0;
        }
        last_resources = resources;
        thread::sleep(Duration::from_millis(250));
    }
}

/// What `wait_for` waits on. Any field left empty is not checked.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WaitQuery {
    pub text: String,
    pub selector: String,
    pub gone: String,
}

pub fn wait_for(
    app: &AppHandle,
    id: u32,
    query: &WaitQuery,
    limit: Duration,
) -> Result<bool, String> {
    let body = serde_json::to_string(query).map_err(|error| error.to_string())?;
    let start = Instant::now();
    loop {
        if call(app, id, &format!("__ce.find({body})"))
            .ok()
            .and_then(|value| value.as_bool())
            == Some(true)
        {
            return Ok(true);
        }
        if start.elapsed() >= limit {
            return Ok(false);
        }
        thread::sleep(Duration::from_millis(300));
    }
}

fn capture(app: &AppHandle, id: u32, format: &str) -> Result<Vec<u8>, String> {
    let mut params = json!({ "format": format, "captureBeyondViewport": false });
    if format == "jpeg" {
        params["quality"] = json!(75);
    }
    let shot = cdp(app, id, "Page.captureScreenshot", params)?;
    let data = shot
        .get("data")
        .and_then(Value::as_str)
        .ok_or("The page returned no screenshot.")?;
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|error| error.to_string())
}

/// A JPEG and its size in pixels.
pub struct Shot {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// A JPEG of the viewport no wider than `max_width`. `marks` draws the last snapshot's refs on it.
pub fn screenshot(app: &AppHandle, id: u32, marks: bool, max_width: u32) -> Result<Shot, String> {
    if marks {
        let _ = call(app, id, "__ce.marks(true)");
        thread::sleep(Duration::from_millis(60));
    }
    let shot = capture(app, id, "jpeg");
    if marks {
        let _ = call(app, id, "__ce.marks(false)");
    }
    shrink_jpeg(&shot?, max_width, 70)
}

pub fn shrink_jpeg(bytes: &[u8], max_width: u32, quality: u8) -> Result<Shot, String> {
    let image = image::load_from_memory(bytes).map_err(|error| error.to_string())?;
    let image = if image.width() > max_width {
        let height =
            (image.height() as f64 * max_width as f64 / image.width() as f64).round() as u32;
        image.resize(
            max_width,
            height.max(1),
            image::imageops::FilterType::Triangle,
        )
    } else {
        image
    };
    let mut out = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
    image
        .to_rgb8()
        .write_with_encoder(encoder)
        .map_err(|error| error.to_string())?;
    Ok(Shot {
        bytes: out,
        width: image.width(),
        height: image.height(),
    })
}

/// Reads the visible page with Windows OCR. Finds text drawn in canvases and images.
pub fn ocr(app: &AppHandle, id: u32) -> Result<String, String> {
    let png = capture(app, id, "png")?;
    ocr::recognize_png(&png)
}

/// The page's visible text. `links` appends each link's address after its text.
pub fn text(app: &AppHandle, id: u32, max: usize, links: bool) -> Result<String, String> {
    let value = call(app, id, &format!("__ce.text({max}, {links})"))?;
    Ok(value.as_str().unwrap_or_default().to_string())
}

/// Finds the element a saved locator describes. Returns its ref and the locator that matched.
pub fn locate(app: &AppHandle, id: u32, target: &Value) -> Result<Option<(u32, String)>, String> {
    let value = call(app, id, &format!("__ce.locate({target})"))?;
    if value.is_null() {
        return Ok(None);
    }
    let element = value.get("ref").and_then(Value::as_u64).unwrap_or(0) as u32;
    let by = value
        .get("by")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    Ok(Some((element, by)))
}

pub fn locators_of(app: &AppHandle, id: u32, element: u32) -> Result<Value, String> {
    call(app, id, &format!("__ce.locatorsOf({element})"))
}

/// Lets the user click an item in a list on the page. Returns the extraction spec and a preview.
pub fn pick(app: &AppHandle, id: u32) -> Result<Option<Value>, String> {
    let limit = Duration::from_secs(180);
    let value = call_with(app, id, "__ce.pick(175000)", true, limit)?;
    Ok(if value.is_null() { None } else { Some(value) })
}

pub fn extract_rows(
    app: &AppHandle,
    id: u32,
    spec: &Value,
) -> Result<Vec<serde_json::Map<String, Value>>, String> {
    let value = call(app, id, &format!("__ce.extract({spec})"))?;
    Ok(value
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|row| row.as_object().cloned())
                .collect()
        })
        .unwrap_or_default())
}

pub fn next_link(app: &AppHandle, id: u32, text: Option<&str>) -> Result<Option<u32>, String> {
    let label = serde_json::to_string(&text.unwrap_or("")).map_err(|error| error.to_string())?;
    let value = call(app, id, &format!("__ce.nextLink({label})"))?;
    Ok(value.as_u64().map(|value| value as u32))
}

/// Runs a script in the page's own world. Only for callers the user allowed to run code.
pub fn evaluate_main(app: &AppHandle, id: u32, expression: &str) -> Result<Value, String> {
    let webview = view(app, id)?;
    webview2::evaluate(&webview, expression, CALL_TIMEOUT, FAILURE)
}
