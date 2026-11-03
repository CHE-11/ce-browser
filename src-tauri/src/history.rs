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
    /// Visits that began with the address typed into the address bar. Older files omit this.
    pub typed_count: u32,
}

impl Default for HistoryItem {
    fn default() -> Self {
        Self {
            id: 0,
            url: String::new(),
            title: String::new(),
            visit_count: 1,
            last_visited: 0,
            typed_count: 0,
        }
    }
}

/// Addresses typed into the address bar whose visit has not been recorded yet.
static TYPED: Mutex<Vec<(String, u64)>> = Mutex::new(Vec::new());
const TYPED_WINDOW_MS: u64 = 30_000;

/// The next visit to `url` counts as typed.
pub fn note_typed(url: &str) {
    if !tracked(url) {
        return;
    }
    let now = now_ms();
    let mut typed = TYPED.lock().expect("typed");
    typed.retain(|(_, at)| now.saturating_sub(*at) < TYPED_WINDOW_MS);
    typed.push((paths::pin_key(url.trim()), now));
}

fn take_typed(url: &str) -> bool {
    let key = paths::pin_key(url);
    let now = now_ms();
    let mut typed = TYPED.lock().expect("typed");
    typed.retain(|(_, at)| now.saturating_sub(*at) < TYPED_WINDOW_MS);
    match typed.iter().position(|(item, _)| *item == key) {
        Some(index) => {
            typed.remove(index);
            true
        }
        None => false,
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
                typed_count: 0,
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
    let typed = take_typed(url.trim());
    with_state(app, |state| {
        apply_visit(
            &mut state.items,
            &mut state.next_id,
            url.trim(),
            title,
            now_ms(),
        );
        if typed && let Some(item) = state.items.first_mut() {
            item.typed_count = item.typed_count.saturating_add(1);
        }
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

/// Removes one entry from the list. Its saved page text stays until `forget_text` runs,
/// so an undo can bring the entry back whole.
pub fn take(app: &AppHandle, id: u32) -> Option<HistoryItem> {
    let removed = with_state(app, |state| {
        let index = state.items.iter().position(|item| item.id == id)?;
        Some(state.items.remove(index))
    });
    if removed.is_some() {
        changed(app);
    }
    removed
}

/// True when `last_visited` falls inside `[since, until]`. A missing bound is open.
fn visited_between(last_visited: u64, since: Option<u64>, until: Option<u64>) -> bool {
    since.is_none_or(|start| last_visited >= start) && until.is_none_or(|end| last_visited <= end)
}

/// Removes entries whose last visit falls inside `[since, until]`.
/// Both bounds missing removes every entry.
/// The bool is true when the list is empty afterwards.
/// The page-text index waits for `forget_text`.
pub fn take_between(
    app: &AppHandle,
    since: Option<u64>,
    until: Option<u64>,
) -> (Vec<HistoryItem>, bool) {
    let (removed, emptied) = with_state(app, |state| {
        if since.is_none() && until.is_none() {
            let removed = std::mem::take(&mut state.items);
            let emptied = !removed.is_empty();
            return (removed, emptied);
        }
        let mut kept = Vec::new();
        let mut removed = Vec::new();
        for item in std::mem::take(&mut state.items) {
            if visited_between(item.last_visited, since, until) {
                removed.push(item);
            } else {
                kept.push(item);
            }
        }
        state.items = kept;
        let emptied = !removed.is_empty() && state.items.is_empty();
        (removed, emptied)
    });
    if !removed.is_empty() {
        changed(app);
    }
    (removed, emptied)
}

/// Puts removed entries back. A page visited again since keeps its newer entry.
pub fn restore(app: &AppHandle, items: Vec<HistoryItem>) {
    if items.is_empty() {
        return;
    }
    with_state(app, |state| {
        for item in items {
            let key = paths::pin_key(&item.url);
            if state
                .items
                .iter()
                .any(|existing| paths::pin_key(&existing.url) == key)
            {
                continue;
            }
            state.next_id = state.next_id.max(item.id.saturating_add(1));
            state.items.push(item);
        }
        state
            .items
            .sort_by(|left, right| right.last_visited.cmp(&left.last_visited));
        state.items.truncate(MAX_ITEMS);
    });
    changed(app);
}

/// Drops saved page text for entries that were removed for good.
pub fn forget_text(app: &AppHandle, items: &[HistoryItem], cleared: bool) {
    if cleared && with_state(app, |state| state.items.is_empty()) {
        crate::history_index::clear(app);
        return;
    }
    for item in items {
        let key = paths::pin_key(&item.url);
        let kept = with_state(app, |state| {
            state
                .items
                .iter()
                .any(|existing| paths::pin_key(&existing.url) == key)
        });
        if !kept {
            crate::history_index::forget(app, &item.url);
        }
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

    #[test]
    fn a_window_keeps_visits_outside_it() {
        assert!(visited_between(1_000, None, None));
        assert!(visited_between(1_000, Some(1_000), Some(2_000)));
        assert!(visited_between(2_000, Some(1_000), Some(2_000)));
        assert!(!visited_between(999, Some(1_000), Some(2_000)));
        assert!(!visited_between(2_001, Some(1_000), Some(2_000)));
        assert!(visited_between(5_000, Some(1_000), None));
        assert!(visited_between(500, None, Some(1_000)));
        assert!(!visited_between(1_001, None, Some(1_000)));
    }
}
