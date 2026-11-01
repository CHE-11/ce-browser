use std::collections::VecDeque;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tauri::AppHandle;

use crate::{files, history, paths, settings, tabs, webview2};

const FILE: &str = "history-text.json";
const MAX_CHARS: usize = 16_000;
const MAX_HITS: usize = 50;

const TEXT_SCRIPT: &str = r#"(() => {
  const text = document.body && document.body.innerText ? document.body.innerText : "";
  return text.slice(0, 16000);
})()"#;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryHit {
    pub id: u32,
    pub url: String,
    pub title: String,
    pub visit_count: u32,
    pub last_visited: u64,
    pub snippet: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TextEntry {
    url: String,
    text: String,
    hash: String,
    indexed_at: u64,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TextIndex {
    entries: Vec<TextEntry>,
}

struct Job {
    app: AppHandle,
    tab_id: u32,
    url: String,
}

static QUEUE: Mutex<VecDeque<Job>> = Mutex::new(VecDeque::new());
static WORKER: Mutex<bool> = Mutex::new(false);

pub fn queue(app: &AppHandle, tab_id: u32, url: &str) {
    if !settings::get(app).index_page_text || !history::tracked(url) {
        return;
    }
    let url = url.to_string();
    {
        let mut queue = QUEUE.lock().unwrap_or_else(|error| error.into_inner());
        queue.retain(|job| job.tab_id != tab_id);
        queue.push_back(Job {
            app: app.clone(),
            tab_id,
            url,
        });
    }
    let mut started = WORKER.lock().unwrap_or_else(|error| error.into_inner());
    if *started {
        return;
    }
    *started = true;
    thread::Builder::new()
        .name("history-index".into())
        .spawn(|| {
            loop {
                let job = {
                    let mut queue = QUEUE.lock().unwrap_or_else(|error| error.into_inner());
                    queue.pop_front()
                };
                let Some(job) = job else {
                    thread::sleep(Duration::from_millis(300));
                    continue;
                };
                index_job(job);
            }
        })
        .ok();
}

fn index_job(job: Job) {
    if !settings::get(&job.app).index_page_text {
        return;
    }
    let still = tabs::snapshot(&job.app).tabs.into_iter().any(|tab| {
        tab.id == job.tab_id
            && !tab.ephemeral
            && paths::pin_key(&tab.url) == paths::pin_key(&job.url)
    });
    if !still {
        return;
    }
    let Some(webview) = tabs::page_view(&job.app, job.tab_id) else {
        return;
    };
    let Ok(value) = webview2::evaluate(
        &webview,
        TEXT_SCRIPT,
        Duration::from_secs(8),
        "Could not read the page text.",
    ) else {
        return;
    };
    let Value::String(text) = value else {
        return;
    };
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.trim().is_empty() {
        return;
    }
    let text: String = text.chars().take(MAX_CHARS).collect();
    remember(&job.app, &job.url, &text);
}

fn remember(app: &AppHandle, url: &str, text: &str) {
    let key = paths::pin_key(url);
    let hash = digest(text);
    let mut index = load(app);
    if let Some(entry) = index.entries.iter_mut().find(|entry| entry.url == key) {
        if entry.hash == hash {
            return;
        }
        entry.text = text.to_string();
        entry.hash = hash;
        entry.indexed_at = now_ms();
    } else {
        index.entries.insert(
            0,
            TextEntry {
                url: key,
                text: text.to_string(),
                hash,
                indexed_at: now_ms(),
            },
        );
    }
    let live: std::collections::HashSet<String> = history::list(app)
        .iter()
        .map(|item| paths::pin_key(&item.url))
        .collect();
    index.entries.retain(|entry| live.contains(&entry.url));
    index.entries.truncate(2000);
    let _ = save(app, &index);
}

pub fn forget(app: &AppHandle, url: &str) {
    let key = paths::pin_key(url);
    let mut index = load(app);
    let before = index.entries.len();
    index.entries.retain(|entry| entry.url != key);
    if index.entries.len() != before {
        let _ = save(app, &index);
    }
}

pub fn clear(app: &AppHandle) {
    let _ = save(app, &TextIndex::default());
}

pub fn search(app: &AppHandle, query: &str) -> Vec<HistoryHit> {
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }
    let needle = query.to_lowercase();
    let index = load(app);
    let mut hits = Vec::new();
    for item in history::list(app) {
        let text = index
            .entries
            .iter()
            .find(|entry| entry.url == paths::pin_key(&item.url))
            .map(|entry| entry.text.as_str())
            .unwrap_or("");
        let title = item.title.to_lowercase();
        let url = item.url.to_lowercase();
        let body = text.to_lowercase();
        let title_match = title.contains(&needle);
        let url_match = url.contains(&needle);
        let text_match = !text.is_empty() && body.contains(&needle);
        if !title_match && !url_match && !text_match {
            continue;
        }
        let rank = if title_match {
            0
        } else if url_match {
            1
        } else {
            2
        };
        hits.push((
            rank,
            HistoryHit {
                id: item.id,
                url: item.url,
                title: item.title,
                visit_count: item.visit_count,
                last_visited: item.last_visited,
                snippet: if text_match {
                    snippet(text, &needle)
                } else {
                    String::new()
                },
            },
        ));
    }
    hits.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then(right.1.last_visited.cmp(&left.1.last_visited))
    });
    hits.truncate(MAX_HITS);
    hits.into_iter().map(|(_, hit)| hit).collect()
}

pub(crate) fn snippet(text: &str, needle: &str) -> String {
    let lower = text.to_lowercase();
    let Some(pos) = lower.find(&needle.to_lowercase()) else {
        return String::new();
    };
    let start = text.floor_char_boundary(pos.saturating_sub(60));
    let end = text.ceil_char_boundary((pos + needle.len() + 80).min(text.len()));
    let mut excerpt = text[start..end].replace(['\n', '\r', '\t'], " ");
    if start > 0 {
        excerpt.insert(0, '…');
    }
    if end < text.len() {
        excerpt.push('…');
    }
    excerpt
}

fn load(app: &AppHandle) -> TextIndex {
    files::read_json(&paths::data_file(app, FILE))
}

fn save(app: &AppHandle, index: &TextIndex) -> Result<(), String> {
    files::write_json(&paths::data_file(app, FILE), index)
}

fn digest(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_snippet_shows_the_words_around_the_match() {
        let text = "The quick brown fox jumps over the lazy dog near the river bank.";
        let excerpt = snippet(text, "lazy");
        assert!(excerpt.contains("lazy"));
        assert!(excerpt.starts_with('…') || excerpt.contains("quick"));
    }

    #[test]
    fn a_missing_word_has_no_snippet() {
        assert!(snippet("nothing to see", "zebra").is_empty());
    }
}
