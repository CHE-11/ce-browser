//! `agent-memory.json`: notes the agent keeps across runs, for a site or for everything.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::automation::{clip, now_ms};
use crate::settings::sleep_site_key;
use crate::{files, paths};

pub const FILE: &str = "agent-memory.json";
const MAX_ITEMS: usize = 500;
const MAX_TEXT: usize = 1000;
/// Characters of memory added to the agent's instructions each turn.
const PROMPT_BUDGET: usize = 1500;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct MemoryItem {
    pub id: String,
    pub text: String,
    /// Host the note is about, such as `github.com`. Blank applies everywhere.
    pub site: String,
    /// Always shown to the agent.
    pub pinned: bool,
    pub created: u64,
    /// Last time the agent read it.
    pub used: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Store {
    items: Vec<MemoryItem>,
}

struct Loaded {
    path: PathBuf,
    store: Store,
}

fn cache() -> &'static Mutex<Option<Loaded>> {
    static CACHE: OnceLock<Mutex<Option<Loaded>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

fn load(app: &AppHandle) -> MutexGuard<'static, Option<Loaded>> {
    let path = paths::data_file(app, FILE);
    let mut guard = cache().lock().expect("memory lock");
    if guard.as_ref().is_none_or(|loaded| loaded.path != path) {
        let store: Store = files::read_json(&path);
        *guard = Some(Loaded { path, store });
    }
    guard
}

fn change<T>(app: &AppHandle, edit: impl FnOnce(&mut Vec<MemoryItem>) -> T) -> T {
    let (out, items) = {
        let mut guard = load(app);
        let loaded = guard.as_mut().expect("loaded memory");
        let out = edit(&mut loaded.store.items);
        if let Err(error) = files::write_json(&loaded.path, &loaded.store) {
            eprintln!("agent memory: {error}");
        }
        (out, loaded.store.items.clone())
    };
    let _ = app.emit("agent-memory-changed", items);
    out
}

/// Every note, newest first.
pub fn list(app: &AppHandle) -> Vec<MemoryItem> {
    let guard = load(app);
    let mut items = guard
        .as_ref()
        .map(|loaded| loaded.store.items.clone())
        .unwrap_or_default();
    items.sort_by(|a, b| b.created.cmp(&a.created));
    items
}

fn new_id() -> String {
    format!("m{:x}{:04x}", now_ms(), rand::random::<u16>())
}

fn clean_site(site: &str) -> String {
    let site = site.trim();
    if site.is_empty() {
        return String::new();
    }
    let key = sleep_site_key(site);
    if key.is_empty() {
        clip(&site.to_ascii_lowercase(), 120)
    } else {
        key
    }
}

/// Adds a note, or changes the one with the same id.
pub fn save(app: &AppHandle, item: MemoryItem) -> Result<MemoryItem, String> {
    let text = clip(item.text.trim(), MAX_TEXT);
    if text.is_empty() {
        return Err("Write something to remember.".into());
    }
    let site = clean_site(&item.site);
    Ok(change(app, |items| {
        if let Some(existing) = items
            .iter_mut()
            .find(|existing| !item.id.is_empty() && existing.id == item.id)
        {
            existing.text = text;
            existing.site = site;
            existing.pinned = item.pinned;
            return existing.clone();
        }
        if let Some(same) = items
            .iter_mut()
            .find(|existing| existing.text == text && existing.site == site)
        {
            same.pinned |= item.pinned;
            return same.clone();
        }
        let saved = MemoryItem {
            id: new_id(),
            text,
            site,
            pinned: item.pinned,
            created: now_ms(),
            used: 0,
        };
        items.push(saved.clone());
        while items.len() > MAX_ITEMS {
            let Some(index) = items
                .iter()
                .enumerate()
                .filter(|(_, item)| !item.pinned)
                .min_by_key(|(_, item)| item.used.max(item.created))
                .map(|(index, _)| index)
            else {
                break;
            };
            items.remove(index);
        }
        saved
    }))
}

pub fn delete(app: &AppHandle, id: &str) -> bool {
    change(app, |items| {
        let before = items.len();
        items.retain(|item| item.id != id);
        items.len() != before
    })
}

fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|word| word.chars().count() > 2)
        .map(str::to_string)
        .collect()
}

fn score(item: &MemoryItem, query: &[String], site: &str) -> usize {
    let text = item.text.to_lowercase();
    let hits = query
        .iter()
        .filter(|word| text.contains(word.as_str()))
        .count();
    let here = !site.is_empty() && item.site == site;
    hits * 2 + usize::from(here) + usize::from(item.pinned)
}

/// Notes that match `query`, best first. A blank query lists the newest.
pub fn recall(app: &AppHandle, query: &str, url: &str, limit: usize) -> Vec<MemoryItem> {
    let site = sleep_site_key(url);
    let query = words(query);
    let mut items = list(app);
    if !query.is_empty() {
        items.retain(|item| score(item, &query, &site) > usize::from(item.pinned));
        items.sort_by_key(|item| std::cmp::Reverse(score(item, &query, &site)));
    }
    items.truncate(limit);
    touch(app, &items);
    items
}

fn touch(app: &AppHandle, items: &[MemoryItem]) {
    if items.is_empty() {
        return;
    }
    let now = now_ms();
    let mut guard = load(app);
    if let Some(loaded) = guard.as_mut() {
        for stored in &mut loaded.store.items {
            if items.iter().any(|item| item.id == stored.id) {
                stored.used = now;
            }
        }
        let _ = files::write_json(&loaded.path, &loaded.store);
    }
}

/// Pinned notes and notes for the page's site, for the agent's instructions.
pub fn for_prompt(app: &AppHandle, url: &str) -> String {
    let site = sleep_site_key(url);
    let mut items: Vec<MemoryItem> = list(app)
        .into_iter()
        .filter(|item| item.pinned || (!site.is_empty() && item.site == site))
        .collect();
    items.sort_by_key(|item| std::cmp::Reverse((item.pinned, item.created)));
    let mut out = String::new();
    for item in items {
        let line = if item.site.is_empty() {
            format!("- {}\n", item.text)
        } else {
            format!("- ({}) {}\n", item.site, item.text)
        };
        if out.len() + line.len() > PROMPT_BUDGET {
            break;
        }
        out.push_str(&line);
    }
    out
}

pub fn describe(item: &MemoryItem) -> String {
    let mut out = format!("[{}] {}", item.id, item.text);
    if !item.site.is_empty() {
        out.push_str(&format!(" (site: {})", item.site));
    }
    if item.pinned {
        out.push_str(" (pinned)");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_prefer_matching_words() {
        let item = MemoryItem {
            text: "The user's shoe size is 44".into(),
            ..MemoryItem::default()
        };
        assert!(score(&item, &words("shoe size"), "") > score(&item, &words("hotel"), ""));
        let empty: Store = serde_json::from_str("{}").expect("parse");
        assert!(empty.items.is_empty());
    }
}
