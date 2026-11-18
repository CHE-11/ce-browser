//! Toolbar icon, badge, and tooltip for an extension button.
//!
//! WebView2 accepts `chrome.action.setIcon` and `setBadgeText` and then paints nothing.
//! The service-worker patch in `extension_popouts` posts the call here. Host extensions
//! set the same state through `ce.action`. The address bar and the tools toolbar draw it.

use std::collections::VecDeque;
use std::sync::Mutex;

use serde::Serialize;
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter};

const MAX_TABS: usize = 40;
const MAX_ICON: usize = 400_000;
const MAX_BADGE: usize = 4;

#[derive(Clone, Default)]
struct Face {
    icon: Option<String>,
    badge: Option<String>,
    badge_color: Option<String>,
    badge_text_color: Option<String>,
    title: Option<String>,
}

struct TabFace {
    url: String,
    face: Face,
}

struct Record {
    id: String,
    global: Face,
    tabs: VecDeque<TabFace>,
}

/// A page-specific override. Null fields keep the extension-wide value.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionActionTab {
    pub url: String,
    pub icon: Option<String>,
    pub badge: Option<String>,
    pub badge_color: Option<String>,
    pub badge_text_color: Option<String>,
    pub title: Option<String>,
}

/// What the toolbar should draw for one extension. `id` is the store id, or `host:` plus a host id.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionActionState {
    pub id: String,
    pub icon: Option<String>,
    pub badge: String,
    pub badge_color: String,
    pub badge_text_color: String,
    pub title: String,
    pub tabs: Vec<ExtensionActionTab>,
}

fn records() -> &'static Mutex<Vec<Record>> {
    static RECORDS: std::sync::OnceLock<Mutex<Vec<Record>>> = std::sync::OnceLock::new();
    RECORDS.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn list() -> Vec<ExtensionActionState> {
    let guard = records().lock().expect("extension actions");
    publish(&guard)
}

/// A Chrome or Edge extension reported a new icon, badge, or title.
pub fn update_chrome(app: &AppHandle, body: &str) {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return;
    };
    let Some(id) = value.get("id").and_then(Value::as_str) else {
        return;
    };
    if !chrome_id(id) {
        return;
    }
    commit(app, &value);
}

/// A host extension set its own toolbar icon, badge, or title. `id` is the package id.
pub fn update_host(app: &AppHandle, id: &str, mut patch: Value) -> Result<(), String> {
    if !crate::host_extensions::valid_id(id) {
        return Err("That extension cannot set a badge.".into());
    }
    if let Some(icon) = patch.get("icon").and_then(Value::as_str)
        && !icon.is_empty()
        && clean_icon(icon).is_none()
    {
        return Err("That icon could not be used.".into());
    }
    patch["id"] = json!(format!("host:{id}"));
    commit(app, &patch);
    Ok(())
}

pub fn forget(app: &AppHandle, id: &str) {
    let next = {
        let mut guard = records().lock().expect("extension actions");
        let before = guard.len();
        guard.retain(|record| record.id != id);
        if guard.len() == before {
            None
        } else {
            Some(publish(&guard))
        }
    };
    if let Some(next) = next {
        let _ = app.emit("extension-actions-changed", next);
    }
}

fn commit(app: &AppHandle, value: &Value) {
    let next = {
        let mut guard = records().lock().expect("extension actions");
        let before = publish(&guard);
        apply_value(&mut guard, value);
        let after = publish(&guard);
        (before != after).then_some(after)
    };
    if let Some(next) = next {
        let _ = app.emit("extension-actions-changed", next);
    }
}

fn apply_value(records: &mut Vec<Record>, value: &Value) {
    let Some(id) = value.get("id").and_then(Value::as_str) else {
        return;
    };
    if !allowed(id) {
        return;
    }
    let mut patch = Face::default();
    let mut touched = false;
    if let Some(icon) = value.get("icon").and_then(Value::as_str)
        && let Some(icon) = clean_icon(icon)
    {
        patch.icon = Some(icon);
        touched = true;
    }
    if value.get("badge").is_some() {
        patch.badge = Some(clean_badge(
            value.get("badge").and_then(Value::as_str).unwrap_or(""),
        ));
        touched = true;
    }
    if let Some(color) = value.get("badgeColor").and_then(Value::as_str)
        && let Some(color) = clean_color(color)
    {
        patch.badge_color = Some(color);
        touched = true;
    }
    if let Some(color) = value.get("badgeTextColor").and_then(Value::as_str)
        && let Some(color) = clean_color(color)
    {
        patch.badge_text_color = Some(color);
        touched = true;
    }
    if value.get("title").is_some() {
        patch.title = Some(clean_title(
            value.get("title").and_then(Value::as_str).unwrap_or(""),
        ));
        touched = true;
    }
    if !touched {
        return;
    }
    let url = value
        .get("url")
        .and_then(Value::as_str)
        .map(page_key)
        .unwrap_or_default();
    let index = if let Some(index) = records.iter().position(|record| record.id == id) {
        index
    } else {
        records.push(Record {
            id: id.to_string(),
            global: Face::default(),
            tabs: VecDeque::new(),
        });
        records.len() - 1
    };
    {
        let record = &mut records[index];
        if url.is_empty() {
            merge(&mut record.global, patch);
        } else if let Some(tab_index) = record.tabs.iter().position(|tab| tab.url == url) {
            merge(&mut record.tabs[tab_index].face, patch);
            if let Some(tab) = record.tabs.remove(tab_index) {
                if !face_blank(&tab.face) {
                    record.tabs.push_front(tab);
                }
            }
        } else {
            let mut face = Face::default();
            merge(&mut face, patch);
            if !face_blank(&face) {
                record.tabs.push_front(TabFace { url, face });
                while record.tabs.len() > MAX_TABS {
                    record.tabs.pop_back();
                }
            }
        }
    }
    if record_blank(&records[index]) {
        records.remove(index);
    }
}

fn merge(target: &mut Face, patch: Face) {
    if let Some(icon) = patch.icon {
        target.icon = Some(icon);
    }
    if let Some(badge) = patch.badge {
        target.badge = Some(badge);
    }
    if let Some(color) = patch.badge_color {
        target.badge_color = Some(color);
    }
    if let Some(color) = patch.badge_text_color {
        target.badge_text_color = Some(color);
    }
    if let Some(title) = patch.title {
        target.title = Some(title);
    }
}

fn publish(records: &[Record]) -> Vec<ExtensionActionState> {
    records
        .iter()
        .map(|record| ExtensionActionState {
            id: record.id.clone(),
            icon: show(&record.global.icon),
            badge: text(&record.global.badge),
            badge_color: text(&record.global.badge_color),
            badge_text_color: text(&record.global.badge_text_color),
            title: text(&record.global.title),
            tabs: record
                .tabs
                .iter()
                .map(|tab| ExtensionActionTab {
                    url: tab.url.clone(),
                    icon: tab.face.icon.clone(),
                    badge: tab.face.badge.clone(),
                    badge_color: tab.face.badge_color.clone(),
                    badge_text_color: tab.face.badge_text_color.clone(),
                    title: tab.face.title.clone(),
                })
                .collect(),
        })
        .collect()
}

fn show(value: &Option<String>) -> Option<String> {
    value.clone().filter(|text| !text.is_empty())
}

fn text(value: &Option<String>) -> String {
    value.clone().unwrap_or_default()
}

fn filled(value: &Option<String>) -> bool {
    value.as_ref().is_some_and(|text| !text.is_empty())
}

fn face_blank(face: &Face) -> bool {
    !filled(&face.icon)
        && !filled(&face.badge)
        && !filled(&face.title)
        && !filled(&face.badge_color)
        && !filled(&face.badge_text_color)
}

fn record_blank(record: &Record) -> bool {
    face_blank(&record.global) && record.tabs.iter().all(|tab| face_blank(&tab.face))
}

fn chrome_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|byte| matches!(byte, b'a'..=b'p'))
}

fn allowed(id: &str) -> bool {
    if chrome_id(id) {
        return true;
    }
    id.strip_prefix("host:")
        .is_some_and(crate::host_extensions::valid_id)
}

fn page_key(url: &str) -> String {
    let url = url.split('#').next().unwrap_or(url).trim();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return String::new();
    }
    url.chars().take(2000).collect()
}

fn clean_icon(value: &str) -> Option<String> {
    if value.is_empty() {
        return Some(String::new());
    }
    if value.len() > MAX_ICON {
        return None;
    }
    let image = value.starts_with("data:image/png")
        || value.starts_with("data:image/jpeg")
        || value.starts_with("data:image/gif")
        || value.starts_with("data:image/webp")
        || value.starts_with("data:image/svg+xml");
    if image && value.contains(";base64,") {
        Some(value.to_string())
    } else {
        None
    }
}

fn clean_badge(value: &str) -> String {
    value.trim().chars().take(MAX_BADGE).collect()
}

fn clean_title(value: &str) -> String {
    value
        .replace(['\n', '\r'], " ")
        .chars()
        .take(200)
        .collect::<String>()
        .trim()
        .to_string()
}

fn clean_color(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return Some(String::new());
    }
    if color_ok(value) {
        Some(value.to_string())
    } else {
        None
    }
}

fn color_ok(value: &str) -> bool {
    if value.len() > 48 {
        return false;
    }
    if let Some(hex) = value.strip_prefix('#') {
        return matches!(hex.len(), 3 | 4 | 6 | 8)
            && hex.chars().all(|char| char.is_ascii_hexdigit());
    }
    if value.chars().all(|char| char.is_ascii_alphabetic()) && (3..=20).contains(&value.len()) {
        return true;
    }
    let Some(inner) = value
        .strip_prefix("rgba(")
        .or_else(|| value.strip_prefix("rgb("))
    else {
        return false;
    };
    let Some(inner) = inner.strip_suffix(')') else {
        return false;
    };
    !inner.is_empty()
        && inner
            .chars()
            .all(|char| char.is_ascii_digit() || matches!(char, ',' | '.' | ' ' | '/' | '%'))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn preview(values: &[&str]) -> Vec<ExtensionActionState> {
        let mut records = Vec::new();
        for value in values {
            let parsed: Value = serde_json::from_str(value).unwrap();
            apply_value(&mut records, &parsed);
        }
        publish(&records)
    }

    #[test]
    fn a_badge_and_a_later_icon_stay_together() {
        let states = preview(&[
            r#"{"id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","badge":"  12  ","badgeColor":"rgb(41, 78, 95)"}"#,
            r#"{"id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","icon":"data:image/png;base64,aaaa","title":"Bitwarden is locked"}"#,
        ]);
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].id, ID);
        assert_eq!(states[0].badge, "12");
        assert_eq!(states[0].badge_color, "rgb(41, 78, 95)");
        assert_eq!(
            states[0].icon.as_deref(),
            Some("data:image/png;base64,aaaa")
        );
        assert_eq!(states[0].title, "Bitwarden is locked");
    }

    #[test]
    fn badge_text_keeps_four_characters() {
        let states = preview(&[r#"{"id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","badge":"12345"}"#]);
        assert_eq!(states[0].badge, "1234");
    }

    #[test]
    fn a_page_badge_does_not_replace_the_locked_icon() {
        let states = preview(&[
            r#"{"id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","icon":"data:image/png;base64,bbbb"}"#,
            r#"{"id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","badge":"3","url":"https://example.com/login#form"}"#,
        ]);
        assert_eq!(
            states[0].icon.as_deref(),
            Some("data:image/png;base64,bbbb")
        );
        assert_eq!(states[0].badge, "");
        assert_eq!(states[0].tabs.len(), 1);
        assert_eq!(states[0].tabs[0].url, "https://example.com/login");
        assert_eq!(states[0].tabs[0].badge.as_deref(), Some("3"));
        assert!(states[0].tabs[0].icon.is_none());
    }

    #[test]
    fn an_extension_page_updates_the_global_icon() {
        let states = preview(&[
            r#"{"id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","icon":"data:image/png;base64,cccc","url":"chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/popup.html"}"#,
        ]);
        assert_eq!(
            states[0].icon.as_deref(),
            Some("data:image/png;base64,cccc")
        );
        assert!(states[0].tabs.is_empty());
    }

    #[test]
    fn a_cleared_badge_drops_an_empty_record() {
        let states = preview(&[
            r#"{"id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","badge":"1"}"#,
            r#"{"id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","badge":""}"#,
        ]);
        assert!(states.is_empty());
    }

    #[test]
    fn a_bad_icon_and_a_foreign_id_are_ignored() {
        let states = preview(&[
            r#"{"id":"not-an-extension","badge":"1"}"#,
            r#"{"id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","icon":"https://example.com/lock.png","badge":"9"}"#,
        ]);
        assert_eq!(states.len(), 1);
        assert!(states[0].icon.is_none());
        assert_eq!(states[0].badge, "9");
    }

    #[test]
    fn a_host_extension_keeps_its_own_badge() {
        let states = preview(&[
            r#"{"id":"host:notes.example","badge":"!","badgeColor":"red","title":"Locked"}"#,
        ]);
        assert_eq!(states[0].id, "host:notes.example");
        assert_eq!(states[0].badge, "!");
        assert_eq!(states[0].badge_color, "red");
        assert_eq!(states[0].title, "Locked");
    }
}
