//! What a click or drag caused besides itself: a download, a new tab, or a script window.

use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use tauri::AppHandle;

use crate::automation::driver;
use crate::automation::now_ms;
use crate::automation::policy::{self, Verdict};
use crate::settings::{self, AutomationSettings};
use crate::tabs;

struct DownloadNote {
    tab: u32,
    id: u32,
    name: String,
    path: String,
    at: u64,
    done: bool,
}

struct Spawn {
    opener: u32,
    tab: Option<u32>,
    window: Option<String>,
    at: u64,
}

fn downloads() -> &'static Mutex<Vec<DownloadNote>> {
    static NOTES: OnceLock<Mutex<Vec<DownloadNote>>> = OnceLock::new();
    NOTES.get_or_init(|| Mutex::new(Vec::new()))
}

fn spawns() -> &'static Mutex<Vec<Spawn>> {
    static SPAWNS: OnceLock<Mutex<Vec<Spawn>>> = OnceLock::new();
    SPAWNS.get_or_init(|| Mutex::new(Vec::new()))
}

fn trim<T>(items: &mut Vec<T>, at: impl Fn(&T) -> u64) {
    let cutoff = now_ms().saturating_sub(60_000);
    items.retain(|item| at(item) >= cutoff);
    if items.len() > 40 {
        items.drain(0..items.len() - 40);
    }
}

/// A page view started saving a file. `tab` is 0 when the view is not a tab.
pub fn note_download(tab: u32, id: u32, name: String, path: String) {
    if tab == 0 {
        return;
    }
    let mut notes = downloads().lock().expect("download notes");
    notes.push(DownloadNote {
        tab,
        id,
        name,
        path,
        at: now_ms(),
        done: false,
    });
    trim(&mut notes, |note| note.at);
}

/// The download finished, or failed.
pub fn finish_download(id: u32, done: bool) {
    if let Some(note) = downloads()
        .lock()
        .expect("download notes")
        .iter_mut()
        .rev()
        .find(|note| note.id == id)
    {
        note.done = done;
    }
}

/// `target="_blank"` opened a tab from `opener`.
pub fn remember_tab(opener: u32, child: u32) {
    let mut items = spawns().lock().expect("spawns");
    items.push(Spawn {
        opener,
        tab: Some(child),
        window: None,
        at: now_ms(),
    });
    trim(&mut items, |item| item.at);
}

/// A sized `window.open` opened a top-level window from `opener`.
pub fn remember_window(opener: u32, label: String) {
    let mut items = spawns().lock().expect("spawns");
    items.push(Spawn {
        opener,
        tab: None,
        window: Some(label),
        at: now_ms(),
    });
    trim(&mut items, |item| item.at);
}

/// Refuses a coordinate click that lands in a cross-origin frame on a blocked site.
pub fn guard_point(
    app: &AppHandle,
    auto: &AutomationSettings,
    tab: u32,
    x: f64,
    y: f64,
) -> Result<(), String> {
    for frame in driver::frames(app, tab) {
        if frame.host.is_empty()
            || x < frame.x
            || y < frame.y
            || x > frame.x + frame.width
            || y > frame.y + frame.height
        {
            continue;
        }
        let url = format!("https://{}/", frame.host);
        if let Verdict::Block(reason) = policy::page(auto, &url) {
            return Err(reason);
        }
        if settings::get(app).ai_blocked(&url) {
            return Err(settings::AI_SITE_BLOCKED.into());
        }
    }
    Ok(())
}

/// Taken just before a page action, so later effects can be told apart from older ones.
pub struct Armed {
    pub at: u64,
    pub opener: u32,
}

pub fn arm(tab: u32) -> Armed {
    Armed {
        at: now_ms(),
        opener: tab,
    }
}

pub struct Effect {
    pub file_name: String,
    pub file_path: String,
    pub tab: Option<u32>,
    pub window: String,
}

impl Effect {
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if !self.file_name.is_empty() {
            parts.push(format!(
                "Download: {} at {}",
                self.file_name, self.file_path
            ));
        }
        if let Some(tab) = self.tab {
            parts.push(format!("Opened tab {tab}. switch_tab can move there."));
        }
        if !self.window.is_empty() {
            parts.push(format!("Opened a window ({}).", self.window));
        }
        parts.join(" ")
    }
}

/// Waits briefly for a download, tab or window started by the action.
pub fn collect(app: &AppHandle, armed: &Armed, wait: Duration) -> Effect {
    let deadline = std::time::Instant::now() + wait;
    let quiet = std::time::Instant::now() + Duration::from_millis(700);
    let mut effect = Effect {
        file_name: String::new(),
        file_path: String::new(),
        tab: None,
        window: String::new(),
    };
    loop {
        if let Some(note) = downloads()
            .lock()
            .expect("download notes")
            .iter()
            .rev()
            .find(|note| note.tab == armed.opener && note.at >= armed.at)
        {
            effect.file_name = note.name.clone();
            effect.file_path = note.path.clone();
        }
        if effect.tab.is_none() || effect.window.is_empty() {
            for spawn in spawns().lock().expect("spawns").iter().rev() {
                if spawn.opener != armed.opener || spawn.at < armed.at {
                    continue;
                }
                if effect.tab.is_none() {
                    effect.tab = spawn.tab;
                }
                if effect.window.is_empty() {
                    effect.window = spawn.window.clone().unwrap_or_default();
                }
            }
        }
        let download_done = effect.file_name.is_empty()
            || downloads()
                .lock()
                .expect("download notes")
                .iter()
                .rev()
                .any(|note| note.tab == armed.opener && note.at >= armed.at && note.done);
        let started =
            effect.tab.is_some() || !effect.window.is_empty() || !effect.file_name.is_empty();
        if started && download_done {
            break;
        }
        if !started && std::time::Instant::now() >= quiet {
            break;
        }
        if std::time::Instant::now() >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(80));
    }
    if let Some(tab) = effect.tab {
        if tabs::tab_info(app, tab).is_none() {
            effect.tab = None;
        }
    }
    effect
}
