use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};
use url::form_urlencoded;

use crate::{paths, settings, tabs};

const FIRST_CHECK: Duration = Duration::from_secs(20);
const CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);
const EXE_FILE: &str = "last-exe.txt";
/// Releases live on GitHub next to the `latest.json` the updater reads.
const RELEASES_API: &str = "https://api.github.com/repos/CHE-11/ce-browser/releases/tags/v";
const RELEASES_PAGE: &str = "https://github.com/CHE-11/ce-browser/releases/tag/v";

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub current: String,
    /// A newer version that is ready to install.
    pub available: Option<String>,
    /// Release notes for `available`.
    pub notes: String,
    pub checking: bool,
    pub installing: bool,
    pub error: Option<String>,
    /// Milliseconds since 1970 of the last finished check. Zero before the first one.
    pub checked_at: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseNotes {
    pub version: String,
    pub name: String,
    /// Markdown from the release page.
    pub body: String,
    pub published_at: String,
    pub url: String,
}

struct UpdateState {
    status: Mutex<UpdateStatus>,
    pending: Mutex<Option<Update>>,
    /// Version a toast was already shown for, so a daily check does not repeat it.
    announced: Mutex<Option<String>>,
}

pub fn init(app: &AppHandle) {
    app.manage(UpdateState {
        status: Mutex::new(UpdateStatus {
            current: env!("CARGO_PKG_VERSION").to_string(),
            ..UpdateStatus::default()
        }),
        pending: Mutex::new(None),
        announced: Mutex::new(None),
    });
}

fn state(app: &AppHandle) -> tauri::State<'_, UpdateState> {
    app.state::<UpdateState>()
}

pub fn status(app: &AppHandle) -> UpdateStatus {
    state(app).status.lock().expect("update status").clone()
}

fn change(app: &AppHandle, edit: impl FnOnce(&mut UpdateStatus)) -> UpdateStatus {
    let next = {
        let state = state(app);
        let mut status = state.status.lock().expect("update status");
        edit(&mut status);
        status.clone()
    };
    let _ = app.emit("update-status", &next);
    next
}

/// Asks the release feed for a newer version. A found update waits for `install`.
pub async fn check(app: &AppHandle) -> Result<UpdateStatus, String> {
    if status(app).checking {
        return Ok(status(app));
    }
    change(app, |status| {
        status.checking = true;
        status.error = None;
    });
    let found = match app.updater() {
        Ok(updater) => updater.check().await.map_err(|error| error.to_string()),
        Err(error) => Err(error.to_string()),
    };
    let checked_at = tabs::now_ms();
    match found {
        Ok(Some(update)) => {
            let version = update.version.clone();
            let notes = update.body.clone().unwrap_or_default();
            *state(app).pending.lock().expect("pending update") = Some(update);
            Ok(change(app, |status| {
                status.checking = false;
                status.available = Some(version);
                status.notes = notes;
                status.checked_at = checked_at;
            }))
        }
        Ok(None) => {
            *state(app).pending.lock().expect("pending update") = None;
            Ok(change(app, |status| {
                status.checking = false;
                status.available = None;
                status.notes.clear();
                status.checked_at = checked_at;
            }))
        }
        Err(error) => {
            let message = format!("Couldn't check for updates. {error}");
            change(app, |status| {
                status.checking = false;
                status.error = Some(message.clone());
                status.checked_at = checked_at;
            });
            Err(message)
        }
    }
}

/// Downloads the waiting update, saves the session and runs the installer. The installer
/// closes the browser and starts the new version, which restores every tab.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    let pending = state(app).pending.lock().expect("pending update").take();
    let update = match pending {
        Some(update) => update,
        None => {
            check(app).await?;
            state(app)
                .pending
                .lock()
                .expect("pending update")
                .take()
                .ok_or("ce-browser is up to date.")?
        }
    };
    change(app, |status| {
        status.installing = true;
        status.error = None;
    });
    let bytes = match update.download(|_, _| {}, || {}).await {
        Ok(bytes) => bytes,
        Err(error) => {
            let message = format!("Couldn't download the update. {error}");
            *state(app).pending.lock().expect("pending update") = Some(update);
            change(app, |status| {
                status.installing = false;
                status.error = Some(message.clone());
            });
            return Err(message);
        }
    };
    crate::session::shutdown(app);
    if let Err(error) = update.install(bytes) {
        let message = format!("Couldn't start the installer. {error}");
        change(app, |status| {
            status.installing = false;
            status.error = Some(message.clone());
        });
        return Err(message);
    }
    app.restart();
}

/// Shows a toast once per new version, with a button that installs it.
fn announce(app: &AppHandle, version: &str) {
    {
        let state = state(app);
        let mut announced = state.announced.lock().expect("announced update");
        if announced.as_deref() == Some(version) {
            return;
        }
        *announced = Some(version.to_string());
    }
    let params = form_urlencoded::Serializer::new(String::new())
        .append_pair("id", "0")
        .append_pair("message", &format!("ce-browser {version} is ready"))
        .append_pair("undo", "0")
        .append_pair("action", "update")
        .append_pair("actionLabel", "Restart to update")
        .finish();
    let window = tabs::focused_window(app);
    if let Err(error) = tabs::show_toast(app, &window, &params) {
        eprintln!("update toast: {error}");
    }
}

/// Checks soon after startup and then once a day while automatic updates are on.
/// Development builds never check: they are not installed.
pub fn start(app: &AppHandle) {
    if tauri::is_dev() {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(FIRST_CHECK);
        loop {
            if settings::get(&app).auto_update
                && let Ok(found) = tauri::async_runtime::block_on(check(&app))
                && let Some(version) = found.available
            {
                announce(&app, &version);
            }
            std::thread::sleep(CHECK_EVERY);
        }
    });
}

/// Release notes for `version` from GitHub, or the notes that came with a waiting update.
pub fn release_notes(app: &AppHandle, version: &str) -> Result<ReleaseNotes, String> {
    let version = version.trim().trim_start_matches('v').to_string();
    let current = status(app);
    if current.available.as_deref() == Some(version.as_str()) && !current.notes.is_empty() {
        return Ok(ReleaseNotes {
            name: format!("ce-browser {version}"),
            body: current.notes,
            url: format!("{RELEASES_PAGE}{version}"),
            version,
            published_at: String::new(),
        });
    }
    let text = ureq::get(&format!("{RELEASES_API}{version}"))
        .timeout(Duration::from_secs(15))
        .set("User-Agent", "ce-browser")
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|error| format!("Couldn't load the release notes. {error}"))?
        .into_string()
        .map_err(|error| error.to_string())?;
    let body: serde_json::Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let text = |key: &str| body.get(key).and_then(|value| value.as_str()).unwrap_or_default().to_string();
    Ok(ReleaseNotes {
        name: text("name"),
        body: text("body"),
        published_at: text("published_at"),
        url: Some(text("html_url"))
            .filter(|url| !url.is_empty())
            .unwrap_or_else(|| format!("{RELEASES_PAGE}{version}")),
        version,
    })
}

/// After an update the new version opens ce://whats-new once. A first run only remembers the version.
pub fn note_version(app: &AppHandle) {
    let current = env!("CARGO_PKG_VERSION");
    let seen = settings::get(app).last_seen_version;
    if seen == current {
        return;
    }
    if settings::update(app, |settings| settings.last_seen_version = current.to_string()).is_err() {
        return;
    }
    if !seen.is_empty() && !tauri::is_dev() {
        let _ = tabs::create_tab(app, Some(format!("ce://whats-new?from={seen}")), false, None);
    }
}

/// Remembers which exe is running. Returns the previous exe when this one is somewhere else,
/// as after moving from a copied exe to the installed one.
pub fn record_exe(app: &AppHandle) -> Option<PathBuf> {
    let exe = paths::launch_exe().ok()?;
    let file = paths::root_local_dir(app).join(EXE_FILE);
    let previous = std::fs::read_to_string(&file)
        .ok()
        .map(|text| PathBuf::from(text.trim()))
        .filter(|path| !path.as_os_str().is_empty());
    if previous.as_deref() != Some(exe.as_path()) {
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&file, exe.to_string_lossy().as_bytes());
    }
    previous.filter(|path| path != &exe)
}
