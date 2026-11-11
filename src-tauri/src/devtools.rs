//! Docked DevTools loads the WebView2 runtime's own inspector.
//!
//! Each data folder gets a free localhost port, picked once per run by `tabs::browser_args`.
//! The inspector page is served by that runtime, so it stays in step with Edge updates. Any
//! process on this machine can attach for the life of the session. WebView2 keeps the port on
//! localhost.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde_json::{Map, Value};
use tauri::Webview;

use crate::webview2;

pub const LABEL_PREFIX: &str = "devtools-";

/// Runs before the docked inspector's own scripts. The inspector origin is the debugging port,
/// and that port changes every launch, so Edge treats every open as a first run and selects
/// Welcome. The stored choice is Elements unless this origin already has another panel. Welcome
/// is closed again if Edge selects it after release notes load. A panel the user already picked
/// is left as it is. The page stays hidden until a real panel is showing.
///
/// Current Edge puts `role="tab"` on the title span and `aria-selected` on
/// `.tabbed-pane-header-tab`. The header id is `tab-` plus the tool id (`welcome`,
/// `release-note`, `whats-new`).
pub const PREFER_ELEMENTS_SCRIPT: &str = r#"(function () {
  var root = document.documentElement;
  if (!root) return;
  var WELCOME = { welcome: 1, "release-note": 1, "whats-new": 1 };
  var remembered = "";
  try { seed(); } catch (e) {}
  root.style.visibility = "hidden";
  var started = Date.now();
  var timer = setInterval(scan, 50);
  function scan() {
    try { seed(); } catch (e) {}
    var landing = null;
    var welcome = null;
    var selected = false;
    var prefer = remembered || "elements";
    headers(function (header) {
      if (toolId(header) === prefer) landing = header;
      if (!isSelected(header)) return;
      selected = true;
      if (isWelcome(header)) welcome = header;
    });
    if (welcome && landing && welcome !== landing) {
      landing.click();
      closeTab(welcome);
      return;
    }
    if ((selected && !welcome) || Date.now() - started > 8000) reveal();
  }
  function reveal() {
    clearInterval(timer);
    root.style.visibility = "";
  }
  function seed() {
    var panel = unquote(localStorage.getItem("panel-selected-tab") || "");
    if (panel && !WELCOME[panel]) remembered = panel;
    if (!panel || WELCOME[panel]) {
      localStorage.setItem("panel-selected-tab", JSON.stringify(remembered || "elements"));
    }
    var tabs = objectSetting("closeable-tabs");
    var tabsChanged = false;
    for (var id in WELCOME) {
      if (tabs[id] !== false) {
        tabs[id] = false;
        tabsChanged = true;
      }
    }
    if (tabsChanged) localStorage.setItem("closeable-tabs", JSON.stringify(tabs));
    var notes = objectSetting("cloud-release-notes");
    if (notes.shouldOpenWelcome !== false) {
      notes.shouldOpenWelcome = false;
      localStorage.setItem("cloud-release-notes", JSON.stringify(notes));
    }
    bump("release-note-version-seen");
    bump("releaseNoteVersionSeen");
    if (localStorage.getItem("help.show-release-note") !== "false") {
      localStorage.setItem("help.show-release-note", "false");
    }
  }
  function objectSetting(key) {
    try {
      var value = JSON.parse(localStorage.getItem(key) || "{}");
      if (value && typeof value === "object") return value;
    } catch (e) {}
    return {};
  }
  function bump(key) {
    var seen = parseInt(unquote(localStorage.getItem(key) || ""), 10);
    if (!seen || seen < 1000) localStorage.setItem(key, "1000");
  }
  function unquote(value) {
    return String(value).replace(/^"|"$/g, "");
  }
  function headers(visit) {
    walk(function (node) {
      if (node.classList && node.classList.contains("tabbed-pane-header-tab")) visit(node);
    });
  }
  function toolId(header) {
    var id = header.id || "";
    if (id.indexOf("tab-") === 0) return id.slice(4);
    return labelOf(header).split(" ")[0];
  }
  function labelOf(header) {
    var title = header.querySelector && header.querySelector(".tabbed-pane-header-tab-title");
    var text = (title && title.textContent) || header.getAttribute("aria-label") || header.textContent || "";
    return text.replace(/\s+/g, " ").trim().toLowerCase();
  }
  function isWelcome(header) {
    if (WELCOME[toolId(header)]) return true;
    var label = labelOf(header);
    return label.indexOf("what's new") === 0 || label.indexOf("whats new") === 0;
  }
  function isSelected(header) {
    return header.getAttribute("aria-selected") === "true" || header.classList.contains("selected");
  }
  function closeTab(header) {
    var button = header.querySelector(".tabbed-pane-close-button, .close-button, dt-close-button");
    if (button) button.click();
  }
  function walk(visit) {
    var stack = [document];
    while (stack.length) {
      var node = stack.pop();
      if (!node) continue;
      if (node.nodeType === 1) visit(node);
      if (node.shadowRoot) stack.push(node.shadowRoot);
      var kids = node.childNodes;
      if (!kids) continue;
      for (var i = kids.length - 1; i >= 0; i--) stack.push(kids[i]);
    }
  }
})();
"#;

const CLOSEABLE_TABS: &str = "closeable-tabs";
const RELEASE_NOTES: &str = "cloud-release-notes";
const VERSION_SEEN: &str = "release-note-version-seen";
const SELECTED_TAB: &str = "panel-selected-tab";

/// Closes Edge's Welcome tool before WebView2 reads `profile`.
///
/// The native inspector reads this file. Welcome stays closed when `closeable-tabs` records it
/// shut, the release-notes blob has `shouldOpenWelcome` off, and the open panel is Elements
/// when nothing else was chosen. The file is patched once per folder, before the first
/// environment for that folder exists; a later rewrite would race the running browser.
///
/// The docked inspector is a normal page on the debugging port, so it does not read this file.
/// Its port changes every launch, and the fresh page would open Welcome. `PREFER_ELEMENTS_SCRIPT`
/// runs in that page instead.
pub fn suppress_welcome(profile: &Path) {
    static CLAIMED: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    let claimed = CLAIMED.get_or_init(|| Mutex::new(HashSet::new()));
    let mut claimed = claimed.lock().expect("devtools profiles");
    if !claimed.insert(profile.to_path_buf()) {
        return;
    }
    if suppress_welcome_file(profile).is_err() {
        claimed.remove(profile);
    }
}

pub fn label(window: &str) -> String {
    format!("{LABEL_PREFIX}{window}")
}

/// Inspector URL for `webview`. Call this off the main thread: the protocol call waits there.
pub fn inspector_for(webview: &Webview, profile: &Path) -> Result<String, String> {
    let port = crate::tabs::debugging_port(profile)
        .or_else(|| wait_for_port(profile))
        .ok_or_else(|| "DevTools did not open a local debugging port.".to_string())?;
    let info = webview2::call_cdp_json(
        webview,
        "Target.getTargetInfo",
        "{}",
        Duration::from_secs(3),
        "Couldn't identify this page.",
    )
    .ok();
    let target_id = info.as_ref().and_then(target_id_of);
    let page_url = webview
        .url()
        .ok()
        .map(|url| url.to_string())
        .unwrap_or_default();
    // Edge's devtoolsFrontendUrl is an aka.ms docs page, not the inspector. The runtime still
    // serves the real frontend on its own debugging port.
    let id = match target_id {
        Some(id) => id,
        None => fetch_target_list(port)
            .ok()
            .and_then(|body| serde_json::from_str::<Value>(&body).ok())
            .and_then(|list| pick_target_id(&list, &page_url))
            .ok_or_else(|| "Couldn't find this page in DevTools.".to_string())?,
    };
    Ok(local_inspector(port, &id))
}

fn wait_for_port(profile: &Path) -> Option<u16> {
    for _ in 0..20 {
        if let Some(port) = debugging_port(profile) {
            return Some(port);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    None
}

/// Chromium writes the chosen port into the profile. WebView2 keeps that profile under `EBWebView`.
fn debugging_port(profile: &Path) -> Option<u16> {
    let paths = [
        profile.join("EBWebView").join("DevToolsActivePort"),
        profile.join("DevToolsActivePort"),
    ];
    for path in paths {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        if let Some(port) = port_from_active_file(&text) {
            return Some(port);
        }
    }
    None
}

fn port_from_active_file(text: &str) -> Option<u16> {
    let port = text.lines().next()?.trim().parse().ok()?;
    (port > 0).then_some(port)
}

fn fetch_target_list(port: u16) -> Result<String, String> {
    ureq::get(&format!("http://127.0.0.1:{port}/json/list"))
        .timeout(Duration::from_secs(3))
        .call()
        .map_err(|error| format!("Couldn't reach DevTools. {error}"))?
        .into_string()
        .map_err(|error| error.to_string())
}

fn target_id_of(value: &Value) -> Option<String> {
    value
        .pointer("/targetInfo/targetId")
        .or_else(|| value.get("targetId"))
        .and_then(|id| id.as_str())
        .filter(|id| !id.is_empty())
        .map(str::to_string)
}

/// The only page whose address matches. Two pages at the same address are left unmatched so the
/// dock does not attach to the wrong one.
fn pick_target_id(list: &Value, page_url: &str) -> Option<String> {
    let items = list.as_array()?;
    let mut matches = items.iter().filter(|item| {
        item.get("type").and_then(|value| value.as_str()) == Some("page")
            && item.get("url").and_then(|value| value.as_str()) == Some(page_url)
            && !is_inspector(item)
    });
    let first = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    first
        .get("id")
        .and_then(|value| value.as_str())
        .filter(|id| !id.is_empty())
        .map(str::to_string)
}

fn is_inspector(item: &Value) -> bool {
    item.get("url")
        .and_then(|value| value.as_str())
        .is_some_and(|url| url.contains("/devtools/inspector.html"))
}

fn suppress_welcome_file(profile: &Path) -> std::io::Result<()> {
    let path = profile
        .join("EBWebView")
        .join("Default")
        .join("Preferences");
    let mut root = match std::fs::read(&path) {
        Ok(bytes) if bytes.iter().all(|byte| byte.is_ascii_whitespace()) => {
            Value::Object(Map::new())
        }
        Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
            Ok(value) if value.is_object() => value,
            _ => return Ok(()),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Value::Object(Map::new()),
        Err(error) => return Err(error),
    };
    if !hide_welcome(&mut root) {
        return Ok(());
    }
    let text = serde_json::to_string(&root).map_err(std::io::Error::other)?;
    write_preferences(&path, &text)
}

/// `true` when Welcome's stored state changed. Other preferences are left in place.
fn hide_welcome(root: &mut Value) -> bool {
    let Some(prefs) = preference_map(root) else {
        return false;
    };
    let mut changed = false;
    changed |= close_welcome_tab(prefs);
    changed |= disable_welcome_reopen(prefs);
    changed |= leave_welcome_panel(prefs);
    changed
}

fn preference_map(root: &mut Value) -> Option<&mut Map<String, Value>> {
    let root = root.as_object_mut()?;
    let devtools = root
        .entry("devtools")
        .or_insert_with(|| Value::Object(Map::new()));
    let devtools = devtools.as_object_mut()?;
    let prefs = devtools
        .entry("preferences")
        .or_insert_with(|| Value::Object(Map::new()));
    prefs.as_object_mut()
}

fn close_welcome_tab(prefs: &mut Map<String, Value>) -> bool {
    let Some(mut tabs) = setting_object(prefs, CLOSEABLE_TABS) else {
        return false;
    };
    if tabs.get("welcome").and_then(Value::as_bool) == Some(false) {
        return false;
    }
    tabs.insert("welcome".into(), Value::Bool(false));
    set_setting(prefs, CLOSEABLE_TABS, &tabs);
    true
}

fn disable_welcome_reopen(prefs: &mut Map<String, Value>) -> bool {
    let Some(mut notes) = setting_object(prefs, RELEASE_NOTES) else {
        return false;
    };
    let mut changed = false;
    if notes.get("shouldOpenWelcome").and_then(Value::as_bool) != Some(false) {
        notes.insert("shouldOpenWelcome".into(), Value::Bool(false));
        set_setting(prefs, RELEASE_NOTES, &notes);
        changed = true;
    }
    if let Some(version) = notes.get("edgeVersion").and_then(Value::as_i64) {
        let behind = seen_version(prefs).is_none_or(|seen| seen < version);
        if behind {
            prefs.insert(VERSION_SEEN.into(), Value::String(version.to_string()));
            changed = true;
        }
    }
    changed
}

/// Elements is the panel Welcome was standing in for. A real choice, such as Console, stays.
fn leave_welcome_panel(prefs: &mut Map<String, Value>) -> bool {
    let welcome_is_open = prefs
        .get(SELECTED_TAB)
        .and_then(Value::as_str)
        .is_none_or(panel_is_welcome);
    if !welcome_is_open {
        return false;
    }
    prefs.insert(SELECTED_TAB.into(), Value::String("\"elements\"".into()));
    true
}

fn panel_is_welcome(panel: &str) -> bool {
    matches!(panel, "\"welcome\"" | "welcome" | "")
}

/// A DevTools setting is a JSON string. `None` means the stored text is not an object.
fn setting_object(prefs: &Map<String, Value>, key: &str) -> Option<Map<String, Value>> {
    match prefs.get(key) {
        None => Some(Map::new()),
        Some(Value::String(text)) => {
            serde_json::from_str::<Value>(text)
                .ok()
                .and_then(|value| match value {
                    Value::Object(map) => Some(map),
                    _ => None,
                })
        }
        Some(_) => None,
    }
}

fn set_setting(prefs: &mut Map<String, Value>, key: &str, value: &Map<String, Value>) {
    let text = serde_json::to_string(&Value::Object(value.clone())).unwrap_or_else(|_| "{}".into());
    prefs.insert(key.into(), Value::String(text));
}

fn seen_version(prefs: &Map<String, Value>) -> Option<i64> {
    let text = prefs.get(VERSION_SEEN)?.as_str()?;
    text.trim().trim_matches('"').parse().ok()
}

fn write_preferences(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_file_name("Preferences.welcome.tmp");
    std::fs::write(&temporary, text)?;
    match std::fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        Err(_) if path.exists() => replace_preferences(path, &temporary),
        Err(error) => {
            let _ = std::fs::remove_file(&temporary);
            Err(error)
        }
    }
}

/// Windows rename will not replace an existing file. Move the current file aside first, and put
/// it back if the new file cannot take its place.
fn replace_preferences(path: &Path, temporary: &Path) -> std::io::Result<()> {
    let backup = path.with_file_name("Preferences.welcome.bak");
    if let Err(error) = std::fs::rename(path, &backup) {
        let _ = std::fs::remove_file(temporary);
        return Err(error);
    }
    if let Err(error) = std::fs::rename(temporary, path) {
        let _ = std::fs::rename(&backup, path);
        return Err(error);
    }
    let _ = std::fs::remove_file(backup);
    Ok(())
}

fn local_inspector(port: u16, target_id: &str) -> String {
    // `inspector.html` registers the screencast app, which draws a second copy of the page
    // inside the tools. `devtools_app.html` is the tools only; the page stays the real webview.
    // `panel=elements` is the frontend's own initial-panel parameter. Welcome can still
    // replace it after release notes load; `PREFER_ELEMENTS_SCRIPT` puts Elements back.
    format!(
        "http://127.0.0.1:{port}/devtools/devtools_app.html?panel=elements&ws=127.0.0.1:{port}/devtools/page/{target_id}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_file_uses_the_first_line() {
        assert_eq!(
            port_from_active_file("9222\n/devtools/browser/abc\n"),
            Some(9222)
        );
        assert_eq!(port_from_active_file("  9333\r\n"), Some(9333));
        assert_eq!(port_from_active_file("0\n"), None);
        assert_eq!(port_from_active_file("\n9222\n"), None);
    }

    #[test]
    fn inspector_url_stays_on_the_debugging_host() {
        let url = local_inspector(9, "abc");
        assert_eq!(
            url,
            "http://127.0.0.1:9/devtools/devtools_app.html?panel=elements&ws=127.0.0.1:9/devtools/page/abc"
        );
        assert!(!url.contains("aka.ms"));
    }

    #[test]
    fn a_single_url_match_is_used_when_the_id_is_missing() {
        let list = serde_json::json!([
            {
                "id": "a",
                "type": "page",
                "url": "https://example.com/",
                "devtoolsFrontendUrl": "https://aka.ms/docs-landing-page/inspector.html?ws=1"
            }
        ]);
        assert_eq!(
            pick_target_id(&list, "https://example.com/").as_deref(),
            Some("a")
        );
    }

    #[test]
    fn duplicate_pages_are_not_guessed() {
        let list = serde_json::json!([
            {"id": "a", "type": "page", "url": "https://example.com/"},
            {"id": "b", "type": "page", "url": "https://example.com/"}
        ]);
        assert_eq!(pick_target_id(&list, "https://example.com/"), None);
    }

    #[test]
    fn target_info_id_is_read_from_either_shape() {
        let nested = serde_json::json!({"targetInfo": {"targetId": "abc"}});
        let flat = serde_json::json!({"targetId": "def"});
        assert_eq!(target_id_of(&nested).as_deref(), Some("abc"));
        assert_eq!(target_id_of(&flat).as_deref(), Some("def"));
        assert_eq!(target_id_of(&serde_json::json!({})), None);
    }

    #[test]
    fn welcome_closes_without_dropping_other_devtools_state() {
        let mut root = serde_json::json!({
            "profile": {"name": "kept"},
            "devtools": {
                "preferences": {
                    "closeable-tabs": "{\"network\":true,\"welcome\":true}",
                    "cloud-release-notes": "{\"edgeVersion\":156,\"shouldOpenWelcome\":true,\"header\":{\"title\":\"What's New\"}}",
                    "release-note-version-seen": "100",
                    "panel-selected-tab": "\"welcome\"",
                    "inspectorVersion": "47"
                }
            }
        });
        assert!(hide_welcome(&mut root));
        assert_eq!(root["profile"]["name"], "kept");
        assert_eq!(root["devtools"]["preferences"]["inspectorVersion"], "47");
        assert_eq!(setting(&root, CLOSEABLE_TABS)["welcome"], false);
        assert_eq!(setting(&root, CLOSEABLE_TABS)["network"], true);
        assert_eq!(setting(&root, RELEASE_NOTES)["shouldOpenWelcome"], false);
        assert_eq!(
            setting(&root, RELEASE_NOTES)["header"]["title"],
            "What's New"
        );
        assert_eq!(root["devtools"]["preferences"][VERSION_SEEN], "156");
        assert_eq!(
            root["devtools"]["preferences"][SELECTED_TAB],
            "\"elements\""
        );
    }

    #[test]
    fn a_closed_welcome_tool_is_left_alone() {
        let mut root = serde_json::json!({
            "devtools": {
                "preferences": {
                    "closeable-tabs": "{\"welcome\":false,\"network\":true}",
                    "cloud-release-notes": "{\"edgeVersion\":156,\"shouldOpenWelcome\":false}",
                    "release-note-version-seen": "\"200\"",
                    "panel-selected-tab": "\"console\""
                }
            }
        });
        let before = root.clone();
        assert!(!hide_welcome(&mut root));
        assert_eq!(root, before);
    }

    #[test]
    fn an_older_seen_version_catches_up_when_welcome_is_already_closed() {
        let mut root = serde_json::json!({
            "devtools": {
                "preferences": {
                    "closeable-tabs": "{\"welcome\":false}",
                    "cloud-release-notes": "{\"edgeVersion\":156,\"shouldOpenWelcome\":false}",
                    "release-note-version-seen": "\"100\""
                }
            }
        });
        assert!(hide_welcome(&mut root));
        assert_eq!(root["devtools"]["preferences"][VERSION_SEEN], "156");
    }

    #[test]
    fn a_new_profile_starts_with_welcome_closed() {
        let dir = temp_profile("fresh");
        suppress_welcome_file(&dir).unwrap();
        let root = read_preferences(&dir);
        assert_eq!(setting(&root, CLOSEABLE_TABS)["welcome"], false);
        assert_eq!(setting(&root, RELEASE_NOTES)["shouldOpenWelcome"], false);
        assert_eq!(
            root["devtools"]["preferences"][SELECTED_TAB],
            "\"elements\""
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_preferences_file_that_does_not_parse_is_left_alone() {
        let dir = temp_profile("bad");
        let path = dir.join("EBWebView").join("Default").join("Preferences");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{").unwrap();
        suppress_welcome(&dir);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn welcome_is_patched_once_per_folder() {
        let dir = temp_profile("once");
        suppress_welcome(&dir);
        let path = dir.join("EBWebView").join("Default").join("Preferences");
        std::fs::write(&path, "{}").unwrap();
        suppress_welcome(&dir);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{}");
        let _ = std::fs::remove_dir_all(dir);
    }

    fn setting(root: &Value, key: &str) -> Value {
        let text = root["devtools"]["preferences"][key].as_str().unwrap();
        serde_json::from_str(text).unwrap()
    }

    fn read_preferences(dir: &Path) -> Value {
        let path = dir.join("EBWebView").join("Default").join("Preferences");
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    fn temp_profile(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ce-browser-welcome-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }
}
