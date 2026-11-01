//! Cookies and localStorage for the site open in a tab.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::{tabs, webview2};

const FAILURE: &str = "Couldn't read storage for this page.";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoragePair {
    pub key: String,
    pub value: String,
}

const LIST_SCRIPT: &str = r#"(function () {
  var items = [];
  for (var i = 0; i < localStorage.length; i++) {
    var key = localStorage.key(i);
    items.push({ key: key, value: String(localStorage.getItem(key) || "") });
  }
  return items;
})()"#;

pub fn cookies(app: &AppHandle, id: u32) -> Result<Vec<webview2::SiteCookie>, String> {
    let (webview, url) = page(app, id)?;
    Ok(webview2::list_cookies(&webview, &url))
}

pub fn set_cookie(app: &AppHandle, id: u32, name: &str, value: &str) -> Result<(), String> {
    let (webview, url) = page(app, id)?;
    let host = host_of(&url)?;
    if name.trim().is_empty() {
        return Err("A cookie needs a name.".into());
    }
    webview2::put_cookie(
        &webview,
        name.trim().to_string(),
        value.to_string(),
        host,
        "/".into(),
    )
}

pub fn delete_cookie(
    app: &AppHandle,
    id: u32,
    name: &str,
    domain: &str,
    path: &str,
) -> Result<(), String> {
    let (webview, _) = page(app, id)?;
    webview2::delete_cookie(
        &webview,
        name.to_string(),
        domain.to_string(),
        path.to_string(),
    )
}

pub fn local_items(app: &AppHandle, id: u32) -> Result<Vec<StoragePair>, String> {
    let (webview, _) = page(app, id)?;
    let value = webview2::evaluate(&webview, LIST_SCRIPT, Duration::from_secs(4), FAILURE)?;
    serde_json::from_value(value).map_err(|error| format!("{FAILURE} {error}"))
}

pub fn set_local(app: &AppHandle, id: u32, key: &str, value: &str) -> Result<(), String> {
    let (webview, _) = page(app, id)?;
    if key.is_empty() {
        return Err("A storage item needs a key.".into());
    }
    let script = format!(
        "localStorage.setItem({key}, {value}); true",
        key = serde_json::to_string(key).unwrap_or_else(|_| "\"\"".into()),
        value = serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into()),
    );
    webview2::evaluate(&webview, &script, Duration::from_secs(4), FAILURE).map(|_| ())
}

pub fn delete_local(app: &AppHandle, id: u32, key: &str) -> Result<(), String> {
    let (webview, _) = page(app, id)?;
    let script = format!(
        "localStorage.removeItem({key}); true",
        key = serde_json::to_string(key).unwrap_or_else(|_| "\"\"".into()),
    );
    webview2::evaluate(&webview, &script, Duration::from_secs(4), FAILURE).map(|_| ())
}

fn page(app: &AppHandle, id: u32) -> Result<(tauri::Webview, String), String> {
    let url = tabs::tab_page_url(app, id).ok_or("That tab is gone.")?;
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("Open the site in this tab to read its storage.".into());
    }
    let webview = tabs::page_view(app, id).ok_or("This page is not loaded yet.")?;
    Ok((webview, url))
}

fn host_of(url: &str) -> Result<String, String> {
    url::Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(|host| host.to_string()))
        .filter(|host| !host.is_empty())
        .ok_or_else(|| "That address has no site.".to_string())
}
