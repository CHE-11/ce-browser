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
    pub groups: Vec<SessionGroupFile>,
    pub workspaces: Vec<tabs::Workspace>,
    pub active_workspace: String,
    pub split: Option<SessionSplitFile>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SessionTabFile {
    pub url: String,
    pub title: String,
    pub pinned: bool,
    pub favicon: Option<String>,
    pub muted: bool,
    pub group_id: Option<u32>,
    /// Empty in files written before workspaces. Those tabs belong to `default`.
    pub workspace: String,
    pub refresh_secs: u32,
    pub watch_changes: bool,
    /// Position in the file, so the split can find its tabs after pins are merged and sorted.
    #[serde(skip)]
    pub origin: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SessionGroupFile {
    pub id: u32,
    pub name: String,
    pub color: String,
    pub collapsed: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SessionSplitFile {
    /// Indexes into `tabs`.
    pub left: usize,
    pub right: usize,
    pub ratio: f64,
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
    let saved = tabs::session_tabs(app);
    if saved.tabs.is_empty() {
        return Ok(());
    }
    let file = SessionFile {
        tabs: saved
            .tabs
            .into_iter()
            .map(|tab| SessionTabFile {
                url: tab.url,
                title: tab.title,
                pinned: tab.pinned,
                favicon: tab.favicon,
                muted: tab.muted,
                group_id: tab.group_id,
                workspace: tab.workspace,
                refresh_secs: tab.refresh_secs,
                watch_changes: tab.watch_changes,
                origin: None,
            })
            .collect(),
        active_index: saved.active,
        groups: saved
            .groups
            .into_iter()
            .map(|group| SessionGroupFile {
                id: group.id,
                name: group.name,
                color: group.color,
                collapsed: group.collapsed,
            })
            .collect(),
        workspaces: saved.workspaces,
        active_workspace: saved.active_workspace,
        split: saved.split.map(|(left, right, ratio)| SessionSplitFile { left, right, ratio }),
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
    let workspaces = tabs::normalize_workspaces(file.workspaces);
    let known = |id: &str| workspaces.iter().any(|workspace| workspace.id == id);
    let active_workspace = if known(&file.active_workspace) {
        file.active_workspace.clone()
    } else {
        tabs::DEFAULT_WORKSPACE.to_string()
    };
    let mut saved: Vec<SessionTabFile> = file
        .tabs
        .into_iter()
        .enumerate()
        .map(|(index, mut tab)| {
            tab.origin = Some(index);
            if !known(&tab.workspace) {
                tab.workspace = tabs::DEFAULT_WORKSPACE.to_string();
            }
            if tab.group_id.is_some_and(|id| !file.groups.iter().any(|group| group.id == id)) {
                tab.group_id = None;
            }
            tab
        })
        .filter(|tab| !tab.url.trim().is_empty())
        .collect();
    let active_origin = file
        .active_index
        .filter(|index| saved.iter().any(|tab| tab.origin == Some(*index)));
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
    let position_of = |origin: usize| saved.iter().position(|tab| tab.origin == Some(origin));
    let active = if restore_all {
        active_origin.and_then(position_of)
    } else {
        None
    };
    let split = if restore_all {
        file.split.as_ref().and_then(|split| {
            Some((position_of(split.left)?, position_of(split.right)?, split.ratio))
        })
    } else {
        None
    };
    let groups = if restore_all {
        file.groups
            .into_iter()
            .map(|group| tabs::TabGroup {
                id: group.id,
                name: group.name,
                color: group.color,
                collapsed: group.collapsed,
                window: tabs::WINDOW.to_string(),
            })
            .collect()
    } else {
        Vec::new()
    };
    let restored = saved
        .into_iter()
        .map(|tab| tabs::SessionTab {
            url: tab.url,
            title: tab.title,
            pinned: tab.pinned,
            favicon: tab.favicon,
            muted: tab.muted,
            group_id: if restore_all { tab.group_id } else { None },
            workspace: tab.workspace,
            refresh_secs: if restore_all { tab.refresh_secs } else { 0 },
            watch_changes: restore_all && tab.watch_changes,
        })
        .collect();
    tabs::restore(
        app,
        tabs::RestoreState {
            tabs: restored,
            active,
            groups,
            workspaces,
            active_workspace,
            split,
        },
    )?;
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
                pinned: true,
                workspace: crate::tabs::DEFAULT_WORKSPACE.to_string(),
                ..SessionTabFile::default()
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
    use super::{SessionFile, SessionTabFile, merge_startup_pins};

    fn tab(url: &str, pinned: bool) -> SessionTabFile {
        SessionTabFile {
            url: url.into(),
            title: url.into(),
            pinned,
            ..SessionTabFile::default()
        }
    }

    #[test]
    fn an_older_session_file_still_loads() {
        let file: SessionFile = serde_json::from_str(
            r#"{"tabs":[{"url":"https://a.example","title":"A","pinned":false,"favicon":null}],"activeIndex":0}"#,
        )
        .expect("session parse");
        assert_eq!(file.tabs.len(), 1);
        assert_eq!(file.tabs[0].workspace, "");
        assert!(!file.tabs[0].muted);
        assert!(file.tabs[0].group_id.is_none());
        assert!(file.groups.is_empty());
        assert!(file.split.is_none());
        assert!(crate::tabs::normalize_workspaces(file.workspaces)
            .iter()
            .any(|workspace| workspace.id == crate::tabs::DEFAULT_WORKSPACE));
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
