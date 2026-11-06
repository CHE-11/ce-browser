//! Docked DevTools loads the WebView2 runtime's own inspector.
//!
//! Each data folder gets a free localhost port, picked once per run by `tabs::browser_args`.
//! The inspector page is served by that runtime, so it stays in step with Edge updates. Any
//! process on this machine can attach for the life of the session. WebView2 keeps the port on
//! localhost.

use std::path::Path;
use std::time::Duration;

use serde_json::Value;
use tauri::Webview;

use crate::webview2;

pub const LABEL_PREFIX: &str = "devtools-";

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

fn local_inspector(port: u16, target_id: &str) -> String {
    // `inspector.html` registers the screencast app, which draws a second copy of the page
    // inside the tools. `devtools_app.html` is the tools only; the page stays the real webview.
    format!(
        "http://127.0.0.1:{port}/devtools/devtools_app.html?ws=127.0.0.1:{port}/devtools/page/{target_id}"
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
            "http://127.0.0.1:9/devtools/devtools_app.html?ws=127.0.0.1:9/devtools/page/abc"
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
}
