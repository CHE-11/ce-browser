use std::sync::Mutex;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::{files, paths, settings, tabs};

const FILE: &str = "session.json";
const SAVE_DELAY: Duration = Duration::from_millis(800);

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SessionFile {
    pub tabs: Vec<SessionTabFile>,
    pub active_index: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SessionTabFile {
    pub url: String,
    pub title: String,
    pub pinned: bool,
    pub favicon: Option<String>,
}

pub struct SessionSaver {
    tx: Mutex<Option<Sender<()>>>,
}

/// Tab changes arrive in bursts while pages load. The saver thread writes once the burst
/// has been quiet for `SAVE_DELAY`.
pub fn init(app: &AppHandle) {
    let (tx, rx) = mpsc::channel::<()>();
    let saver_app = app.clone();
    std::thread::spawn(move || {
        while rx.recv().is_ok() {
            loop {
                match rx.recv_timeout(SAVE_DELAY) {
                    Ok(()) => continue,
                    Err(RecvTimeoutError::Timeout) => break,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
            let _ = save_now(&saver_app);
        }
    });
    app.manage(SessionSaver {
        tx: Mutex::new(Some(tx)),
    });
}

pub fn schedule(app: &AppHandle) {
    let Some(saver) = app.try_state::<SessionSaver>() else {
        return;
    };
    if let Some(tx) = saver.tx.lock().expect("session lock").as_ref() {
        let _ = tx.send(());
    }
}

pub fn save_now(app: &AppHandle) -> Result<(), String> {
    let (tabs, active_index) = tabs::session_tabs(app);
    if tabs.is_empty() {
        return Ok(());
    }
    let file = SessionFile {
        tabs: tabs
            .into_iter()
            .map(|tab| SessionTabFile {
                url: tab.url,
                title: tab.title,
                pinned: tab.pinned,
                favicon: tab.favicon,
            })
            .collect(),
        active_index,
    };
    files::write_json(&paths::data_file(app, FILE), &file)
}

/// Stops the debounced writer so a late save cannot run after the window is gone.
pub fn shutdown(app: &AppHandle) {
    let _ = save_now(app);
    if let Some(saver) = app.try_state::<SessionSaver>() {
        saver.tx.lock().expect("session lock").take();
    }
}

pub fn restore(app: &AppHandle) -> Result<(), String> {
    let file: SessionFile = files::read_json(&paths::data_file(app, FILE));
    let restore_all = settings::get(app).startup == "restore";
    let mut saved: Vec<SessionTabFile> = file
        .tabs
        .into_iter()
        .filter(|tab| !tab.url.trim().is_empty())
        .collect();
    let active_url = file
        .active_index
        .and_then(|index| saved.get(index))
        .map(|tab| tab.url.clone());
    if !restore_all {
        saved.retain(|tab| tab.pinned);
    }
    let pins = settings::get(app).pinned_tabs;
    saved = merge_startup_pins(saved, &pins);
    if saved.is_empty() {
        let url = (!restore_all).then(|| tabs::home_url(app).to_string());
        return tabs::create_tab(app, url, false, None).map(|_| ());
    }
    saved.sort_by_key(|tab| !tab.pinned);
    let active = if restore_all {
        active_url.and_then(|url| saved.iter().position(|tab| tab.url == url))
    } else {
        None
    };
    let restored = saved
        .into_iter()
        .map(|tab| tabs::SessionTab {
            url: tab.url,
            title: tab.title,
            pinned: tab.pinned,
            favicon: tab.favicon,
        })
        .collect();
    tabs::restore(app, restored, active)?;
    if !restore_all {
        tabs::create_tab(app, Some(tabs::home_url(app).to_string()), false, None)?;
    }
    Ok(())
}

/// Settings pins open first. A tab already in the session keeps its title and favicon and is pinned.
fn merge_startup_pins(saved: Vec<SessionTabFile>, pins: &[String]) -> Vec<SessionTabFile> {
    let mut used = vec![false; saved.len()];
    let mut merged = Vec::new();
    let mut seen = Vec::new();
    for pin in pins {
        let key = paths::pin_key(pin);
        if key.is_empty() || seen.iter().any(|existing: &String| existing == &key) {
            continue;
        }
        seen.push(key.clone());
        let mut found = None;
        for (index, tab) in saved.iter().enumerate() {
            if !used[index] && paths::pin_key(&tab.url) == key {
                found = Some(index);
                break;
            }
        }
        if let Some(index) = found {
            let mut tab = saved[index].clone();
            tab.pinned = true;
            used[index] = true;
            merged.push(tab);
        } else {
            merged.push(SessionTabFile {
                url: key,
                title: String::new(),
                pinned: true,
                favicon: None,
            });
        }
    }
    for (index, tab) in saved.into_iter().enumerate() {
        if !used[index] {
            merged.push(tab);
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::{SessionTabFile, merge_startup_pins};

    fn tab(url: &str, pinned: bool) -> SessionTabFile {
        SessionTabFile {
            url: url.into(),
            title: url.into(),
            pinned,
            favicon: None,
        }
    }

    #[test]
    fn startup_pins_lead_and_reuse_open_tabs() {
        let saved = vec![
            tab("https://news.example/", false),
            tab("https://mail.example.com", false),
        ];
        let merged = merge_startup_pins(
            saved,
            &[
                "https://mail.example.com/".into(),
                "https://calendar.example".into(),
            ],
        );
        assert_eq!(merged[0].url, "https://mail.example.com");
        assert!(merged[0].pinned);
        assert_eq!(merged[0].title, "https://mail.example.com");
        assert_eq!(merged[1].url, "https://calendar.example");
        assert!(merged[1].pinned);
        assert!(!merged[2].pinned);
        assert_eq!(merged[2].url, "https://news.example/");
    }
}
