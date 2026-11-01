use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::paths;

const FILE: &str = "history.json";
const MAX_ITEMS: usize = 2000;
/// A load and the source change that follows it are one visit.
const REPEAT_WINDOW_MS: u64 = 1500;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct HistoryItem {
    pub id: u32,
    pub url: String,
    pub title: String,
    pub visit_count: u32,
    pub last_visited: u64,
}

impl Default for HistoryItem {
    fn default() -> Self {
        Self {
            id: 0,
            url: String::new(),
            title: String::new(),
            visit_count: 1,
            last_visited: 0,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct HistoryFile {
    items: Vec<HistoryItem>,
}

struct HistoryState {
    items: Vec<HistoryItem>,
    next_id: u32,
}

pub struct History(Mutex<HistoryState>);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

/// Web pages and local files. Internal `ce://` pages stay out of history.
pub fn tracked(url: &str) -> bool {
    let url = url.trim();
    if url.is_empty() || url.len() > 2048 {
        return false;
    }
    let lower = url.to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://") || lower.starts_with("file:")
}

fn useful_title(url: &str, title: &str) -> Option<String> {
    let title = title.trim();
    if title.is_empty() || title == url {
        None
    } else {
        Some(title.to_string())
    }
}

fn apply_visit(items: &mut Vec<HistoryItem>, next_id: &mut u32, url: &str, title: &str, now: u64) {
    let key = paths::pin_key(url);
    if let Some(index) = items
        .iter()
        .position(|item| paths::pin_key(&item.url) == key)
    {
        let mut item = items.remove(index);
        let repeat = now.saturating_sub(item.last_visited) < REPEAT_WINDOW_MS;
        item.url = url.to_string();
        if let Some(title) = useful_title(url, title) {
            item.title = title;
        }
        if !repeat {
            item.visit_count = item.visit_count.saturating_add(1).max(1);
            item.last_visited = now;
        }
        items.insert(0, item);
    } else {
        let id = *next_id;
        *next_id = next_id.saturating_add(1);
        items.insert(
            0,
            HistoryItem {
                id,
                url: url.to_string(),
                title: useful_title(url, title).unwrap_or_else(|| url.to_string()),
                visit_count: 1,
                last_visited: now,
            },
        );
    }
    if items.len() > MAX_ITEMS {
        items.truncate(MAX_ITEMS);
    }
}

fn apply_title(items: &mut Vec<HistoryItem>, url: &str, title: &str) -> bool {
    let Some(title) = useful_title(url, title) else {
        return false;
    };
    let key = paths::pin_key(url);
    let Some(item) = items
        .iter_mut()
        .find(|item| paths::pin_key(&item.url) == key)
    else {
        return false;
    };
    if item.title == title {
        return false;
    }
    item.title = title;
    true
}

pub fn init(app: &AppHandle) {
    crate::sync_conflicts::note(app, &paths::data_file(app, FILE));
    let file: HistoryFile = crate::sync_conflicts::read_json(app, &paths::data_file(app, FILE));
    let mut items = file.items;
    items.retain(|item| tracked(&item.url));
    for item in &mut items {
        if item.visit_count == 0 {
            item.visit_count = 1;
        }
        if item.title.trim().is_empty() {
            item.title = item.url.clone();
        }
    }
    items.sort_by(|left, right| right.last_visited.cmp(&left.last_visited));
    items.truncate(MAX_ITEMS);
    let next_id = items
        .iter()
        .map(|item| item.id)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    app.manage(History(Mutex::new(HistoryState { items, next_id })));
}

fn with_state<T>(app: &AppHandle, change: impl FnOnce(&mut HistoryState) -> T) -> T {
    let state = app.state::<History>();
    let mut guard = state.inner().0.lock().expect("history lock");
    change(&mut guard)
}

pub fn reload(app: &AppHandle) {
    let file: HistoryFile = crate::sync_conflicts::read_json(app, &paths::data_file(app, FILE));
    let mut items = file.items;
    items.retain(|item| tracked(&item.url));
    for item in &mut items {
        if item.visit_count == 0 {
            item.visit_count = 1;
        }
        if item.title.trim().is_empty() {
            item.title = item.url.clone();
        }
    }
    items.sort_by(|left, right| right.last_visited.cmp(&left.last_visited));
    items.truncate(MAX_ITEMS);
    let next_id = items
        .iter()
        .map(|item| item.id)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    with_state(app, |state| {
        state.items = items.clone();
        state.next_id = next_id;
    });
    let _ = app.emit("history-changed", &items);
}

pub fn list(app: &AppHandle) -> Vec<HistoryItem> {
    with_state(app, |state| state.items.clone())
}

fn changed(app: &AppHandle) {
    let items = list(app);
    let _ = crate::sync_conflicts::store(
        app,
        &paths::data_file(app, FILE),
        &HistoryFile {
            items: items.clone(),
        },
    );
    let _ = app.emit("history-changed", &items);
    crate::sync::touch();
}

/// Replaces this profile's history with the synced list.
pub fn replace(app: &AppHandle, mut items: Vec<HistoryItem>) -> Result<(), String> {
    items.retain(|item| tracked(&item.url));
    for item in &mut items {
        if item.visit_count == 0 {
            item.visit_count = 1;
        }
        if item.title.trim().is_empty() {
            item.title = item.url.clone();
        }
    }
    items.sort_by(|left, right| right.last_visited.cmp(&left.last_visited));
    items.truncate(MAX_ITEMS);
    let next_id = items
        .iter()
        .map(|item| item.id)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    with_state(app, |state| {
        state.items = items;
        state.next_id = next_id;
    });
    changed(app);
    Ok(())
}

/// Records one visit. The same address inside a short window updates the title only.
pub fn visit(app: &AppHandle, url: &str, title: &str) {
    if !tracked(url) {
        return;
    }
    with_state(app, |state| {
        apply_visit(
            &mut state.items,
            &mut state.next_id,
            url.trim(),
            title,
            now_ms(),
        );
    });
    changed(app);
}

/// Fills in the page title after the visit was recorded.
pub fn set_title(app: &AppHandle, url: &str, title: &str) {
    if !tracked(url) {
        return;
    }
    let updated = with_state(app, |state| {
        apply_title(&mut state.items, url.trim(), title)
    });
    if updated {
        changed(app);
    }
}

pub fn remove(app: &AppHandle, id: u32) {
    let removed = with_state(app, |state| {
        let url = state
            .items
            .iter()
            .find(|item| item.id == id)
            .map(|item| item.url.clone());
        let before = state.items.len();
        state.items.retain(|item| item.id != id);
        url.filter(|_| state.items.len() != before)
    });
    if let Some(url) = removed {
        changed(app);
        crate::history_index::forget(app, &url);
    }
}

pub fn clear(app: &AppHandle) {
    let had = with_state(app, |state| {
        let had = !state.items.is_empty();
        state.items.clear();
        had
    });
    if had {
        changed(app);
        crate::history_index::clear(app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracked_skips_internal_pages() {
        assert!(tracked("https://example.com/a"));
        assert!(tracked("http://localhost:1420/"));
        assert!(tracked("file:///C:/notes.txt"));
        assert!(!tracked("ce://history"));
        assert!(!tracked("ce://newtab"));
        assert!(!tracked("about:blank"));
        assert!(!tracked(""));
    }

    #[test]
    fn a_repeat_in_the_same_moment_is_one_visit() {
        let mut items = Vec::new();
        let mut next = 1;
        apply_visit(&mut items, &mut next, "https://example.com/a", "", 1_000);
        apply_visit(
            &mut items,
            &mut next,
            "https://example.com/a",
            "Example",
            1_200,
        );
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].visit_count, 1);
        assert_eq!(items[0].title, "Example");
        assert_eq!(items[0].last_visited, 1_000);
    }

    #[test]
    fn a_later_visit_counts_and_a_trailing_slash_matches() {
        let mut items = Vec::new();
        let mut next = 1;
        apply_visit(
            &mut items,
            &mut next,
            "https://example.com/a",
            "First",
            1_000,
        );
        apply_visit(&mut items, &mut next, "https://other.test/", "Other", 5_000);
        apply_visit(
            &mut items,
            &mut next,
            "https://example.com/a/",
            "Again",
            8_000,
        );
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].url, "https://example.com/a/");
        assert_eq!(items[0].title, "Again");
        assert_eq!(items[0].visit_count, 2);
        assert_eq!(items[1].url, "https://other.test/");
        assert!(apply_title(&mut items, "https://other.test", "Renamed"));
        assert_eq!(items[1].title, "Renamed");
        assert!(!apply_title(&mut items, "https://missing.test", "Nope"));
    }
}
