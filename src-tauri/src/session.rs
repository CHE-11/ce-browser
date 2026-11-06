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
    /// Recently closed tabs and groups, oldest first. Older files omit this.
    pub closed: Vec<tabs::ClosedEntry>,
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
    pub container: String,
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
                container: group.container,
            })
            .collect(),
        workspaces: saved.workspaces,
        active_workspace: saved.active_workspace,
        split: saved
            .split
            .map(|(left, right, ratio)| SessionSplitFile { left, right, ratio }),
        closed: tabs::closed_for_session(app),
    };
    files::write_json(&paths::data_file(app, FILE), &file)
}

/// Stops the debounced writer so a late save cannot run after the window is gone.
pub fn shutdown(app: &AppHandle) {
    crate::privacy::close_windows(app);
    crate::page_errors::note_exiting();
    crate::undo::finish_all(app);
    let _ = save_now(app);
    crate::placement::flush(app);
    crate::status::flush(app);
    if let Some(saver) = app.try_state::<SessionSaver>() {
        saver.tx.lock().expect("session lock").take();
    }
}

pub fn restore(app: &AppHandle) -> Result<(), String> {
    let mut file: SessionFile = files::read_json(&paths::data_file(app, FILE));
    tabs::restore_closed_list(app, std::mem::take(&mut file.closed));
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
            if tab
                .group_id
                .is_some_and(|id| !file.groups.iter().any(|group| group.id == id))
            {
                tab.group_id = None;
            }
            tab
        })
        .filter(|tab| restorable(&tab.url))
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
        tabs::create_tab(app, url, false, None)?;
        return tabs::ensure_bookmarks_host(app, tabs::WINDOW);
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
            Some((
                position_of(split.left)?,
                position_of(split.right)?,
                split.ratio,
            ))
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
                container: group.container,
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
    tabs::ensure_bookmarks_host(app, tabs::WINDOW)
}

/// A tab with no address, or one left on `about:blank`, has nothing to reopen.
fn restorable(url: &str) -> bool {
    let url = url.trim();
    !url.is_empty() && !url.to_ascii_lowercase().starts_with("about:blank")
}

/// Settings pins open first. A tab already in the session keeps its title and favicon and is pinned.
/// It matches the pin the same way the reopen-pins button does, so a redirected pin is not opened twice.
/// Extra pinned copies of the same address on a pin's site are dropped.
fn merge_startup_pins(saved: Vec<SessionTabFile>, pins: &[String]) -> Vec<SessionTabFile> {
    let open: Vec<(&str, bool)> = saved
        .iter()
        .map(|tab| (tab.url.as_str(), tab.pinned))
        .collect();
    let matches = paths::match_pins(pins, &open);
    let mut used = vec![false; saved.len()];
    let mut merged = Vec::new();
    for (pin, found) in pins.iter().zip(matches) {
        match found {
            paths::PinMatch::Open(index) => {
                let mut tab = saved[index].clone();
                tab.pinned = true;
                used[index] = true;
                merged.push(tab);
            }
            paths::PinMatch::Missing => merged.push(SessionTabFile {
                url: paths::pin_key(pin),
                pinned: true,
                workspace: crate::tabs::DEFAULT_WORKSPACE.to_string(),
                ..SessionTabFile::default()
            }),
            paths::PinMatch::Duplicate => {}
        }
    }
    let mut kept: Vec<String> = merged.iter().map(|tab| paths::pin_key(&tab.url)).collect();
    for (index, tab) in saved.into_iter().enumerate() {
        if used[index] {
            continue;
        }
        if tab.pinned && pins.iter().any(|pin| paths::same_pin_site(pin, &tab.url)) {
            let key = paths::pin_key(&tab.url);
            if kept.contains(&key) {
                continue;
            }
            kept.push(key);
        }
        merged.push(tab);
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::{SessionFile, SessionTabFile, merge_startup_pins, restorable};

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
        assert!(file.closed.is_empty());
        assert!(
            crate::tabs::normalize_workspaces(file.workspaces)
                .iter()
                .any(|workspace| workspace.id == crate::tabs::DEFAULT_WORKSPACE)
        );
    }

    #[test]
    fn blank_pinned_tabs_are_not_restored() {
        let saved: Vec<SessionTabFile> = vec![
            tab("about:blank", true),
            tab("https://mail.example.com", true),
            tab(" about:blank#blocked", true),
            tab("", false),
            tab("https://news.example/", false),
        ]
        .into_iter()
        .filter(|tab| restorable(&tab.url))
        .collect();
        let merged = merge_startup_pins(saved, &["https://mail.example.com/".into()]);
        let urls: Vec<&str> = merged.iter().map(|tab| tab.url.as_str()).collect();
        assert_eq!(urls, vec!["https://mail.example.com", "https://news.example/"]);
        assert!(merged[0].pinned);
        assert!(!merged[1].pinned);
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

    #[test]
    fn redirected_pins_are_reused_and_their_copies_dropped() {
        let sign_in = "https://cms.example/sign-in?redirect_url=%2F";
        let login = "https://app.example/auth/login?success_page=%2Fapp%2Ftoday";
        let saved = vec![
            tab("https://chat.example/", true),
            tab(sign_in, true),
            tab("https://app.example/app/today", true),
            tab(sign_in, true),
            tab(login, true),
            tab(sign_in, true),
            tab(login, true),
            tab("https://chat.example/c/123", true),
            tab("https://news.example/", false),
        ];
        let merged = merge_startup_pins(
            saved,
            &[
                "https://chat.example".into(),
                "https://cms.example".into(),
                "https://app.example/app/today".into(),
            ],
        );
        let urls: Vec<&str> = merged.iter().map(|tab| tab.url.as_str()).collect();
        assert_eq!(
            urls,
            vec![
                "https://chat.example/",
                sign_in,
                "https://app.example/app/today",
                login,
                "https://chat.example/c/123",
                "https://news.example/",
            ]
        );
    }
}
