use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter};

use crate::bookmarks::{self, BookmarkTree};
use crate::extensions::{self, IncomingExtension};
use crate::history::{self, HistoryItem};
use crate::settings::{self, BrowserSettings};
use crate::{files, paths};

const FORMAT: u32 = 1;
const BOOKMARKS_FILE: &str = "bookmarks.json";
const EXTENSIONS_FILE: &str = "extensions.json";
const SETTINGS_FILE: &str = "settings.json";
const HISTORY_FILE: &str = "history.json";
const PARTS: [Part; 4] = [
    Part::Bookmarks,
    Part::Extensions,
    Part::Settings,
    Part::History,
];

static DIRTY: AtomicBool = AtomicBool::new(false);
static APPLYING: AtomicBool = AtomicBool::new(false);
static GENERATION: AtomicU64 = AtomicU64::new(0);
static TICK: AtomicU64 = AtomicU64::new(0);
static STARTED: AtomicBool = AtomicBool::new(false);
static CLOSE_STARTED: AtomicBool = AtomicBool::new(false);
static ALLOW_CLOSE: AtomicBool = AtomicBool::new(false);
static OPERATION: Mutex<()> = Mutex::new(());
static ERROR: Mutex<Option<SyncProblem>> = Mutex::new(None);
static LAST_STATUS: Mutex<Option<SyncStatus>> = Mutex::new(None);
static LAST_STAMP: Mutex<Option<FolderStamp>> = Mutex::new(None);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Part {
    Bookmarks,
    Extensions,
    Settings,
    History,
}

impl Part {
    fn file(self) -> &'static str {
        match self {
            Self::Bookmarks => BOOKMARKS_FILE,
            Self::Extensions => EXTENSIONS_FILE,
            Self::Settings => SETTINGS_FILE,
            Self::History => HISTORY_FILE,
        }
    }

    fn local_revision(self, settings: &BrowserSettings) -> u64 {
        match self {
            Self::Bookmarks => settings.sync_bookmarks_revision,
            Self::Extensions => settings.sync_extensions_revision,
            Self::Settings => settings.sync_settings_revision,
            Self::History => settings.sync_history_revision,
        }
    }

    fn set_revision(self, settings: &mut BrowserSettings, revision: u64) {
        match self {
            Self::Bookmarks => settings.sync_bookmarks_revision = revision,
            Self::Extensions => settings.sync_extensions_revision = revision,
            Self::Settings => settings.sync_settings_revision = revision,
            Self::History => settings.sync_history_revision = revision,
        }
        settings.sync_revision = settings.sync_revision.max(revision);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Stamp {
    modified_ms: u128,
    len: u64,
}

#[derive(Clone, PartialEq, Eq)]
struct FolderStamp {
    bookmarks: Stamp,
    extensions: Stamp,
    settings: Stamp,
    history: Stamp,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub folder: String,
    pub error: Option<String>,
    /// True when `error` is a failure to write one of the files in the sync folder.
    pub save_error: bool,
}

#[derive(Clone)]
struct SyncProblem {
    message: String,
    save: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BookmarksDocument {
    format: u32,
    revision: u64,
    bookmarks: BookmarkTree,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingsDocument {
    format: u32,
    revision: u64,
    settings: BrowserSettings,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryDocument {
    format: u32,
    revision: u64,
    items: Vec<HistoryItem>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExtensionsDocument {
    format: u32,
    revision: u64,
    extensions: Vec<SyncExtension>,
}

/// The previous sync file kept every part together. A folder that still has one is split on read.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyDocument {
    format: u32,
    revision: u64,
    settings: BrowserSettings,
    bookmarks: BookmarkTree,
    extensions: Vec<SyncExtension>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncExtension {
    folder: String,
    enabled: bool,
    hash: String,
    archive: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExtensionSig {
    folder: String,
    enabled: bool,
    hash: String,
}

pub fn touch() {
    if APPLYING.load(Ordering::Relaxed) {
        return;
    }
    GENERATION.fetch_add(1, Ordering::Relaxed);
    DIRTY.store(true, Ordering::Relaxed);
}

pub fn configured(app: &AppHandle) -> bool {
    paths::storage_root().is_some() || !settings::get(app).sync_folder.trim().is_empty()
}

pub fn closing_allowed() -> bool {
    ALLOW_CLOSE.load(Ordering::SeqCst)
}

/// Returns true the first time a close is requested.
pub fn begin_close() -> bool {
    !CLOSE_STARTED.swap(true, Ordering::SeqCst)
}

pub fn allow_close() {
    ALLOW_CLOSE.store(true, Ordering::SeqCst);
}

pub fn start(app: &AppHandle) {
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    crate::sync_conflicts::prepare(app);
    let app = app.clone();
    let _ = thread::Builder::new().name("sync".into()).spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(1));
            {
                let _lock = operation_lock();
                match reconcile(&app) {
                    Ok(()) => set_error(&app, None),
                    Err(error) => set_error(&app, Some(error)),
                }
            }
            crate::sync_conflicts::apply_pending(&app);
        }
    });
}

pub fn flush(app: &AppHandle) {
    {
        let _lock = operation_lock();
        DIRTY.store(true, Ordering::Relaxed);
        if let Err(error) = reconcile(app) {
            eprintln!("sync: {error}");
        }
    }
    crate::sync_conflicts::apply_pending(app);
}

pub fn show_folder(app: &AppHandle) -> Result<(), String> {
    let folder = active_folder(app);
    let folder = folder.trim();
    if folder.is_empty() {
        return Err("Choose a sync folder first.".into());
    }
    files::reveal_path(Path::new(folder))
}

pub fn status(app: &AppHandle) -> SyncStatus {
    let problem = error_slot()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    SyncStatus {
        folder: active_folder(app),
        error: problem.as_ref().map(|problem| problem.message.clone()),
        save_error: problem.as_ref().is_some_and(|problem| problem.save),
    }
}

/// Points profile storage at the sync folder already saved on this computer, and moves any
/// profiles still sitting in app data into that folder.
pub fn attach(app: &AppHandle) {
    let appdata = paths::root_data_dir(app);
    let moved = crate::profiles::migrate_layout(&appdata, true).unwrap_or_else(|error| {
        eprintln!("profiles: {error}");
        false
    });
    let moved = moved | attach_folder(app, &appdata);
    if moved {
        crate::profiles::refresh_icons(app);
    }
}

/// Returns true when profile folders moved into the sync folder.
fn attach_folder(app: &AppHandle, appdata: &Path) -> bool {
    let Some(folder) = saved_folder(app) else {
        paths::set_storage_root(None);
        return false;
    };
    let path = PathBuf::from(folder);
    if !path.is_dir() || path.starts_with(appdata) {
        paths::set_storage_root(None);
        return false;
    }
    if !crate::profiles::sync_has_profile_settings(&path)
        && !crate::profiles::sync_has_profile_settings(appdata)
    {
        if let Err(error) = materialize_legacy(&path) {
            eprintln!("sync: {error}");
        }
    }
    if let Err(error) = write_machine(app, &path) {
        eprintln!("sync: {error}");
        return false;
    }
    let moved = crate::profiles::transfer_appdata_to_sync(appdata, &path).unwrap_or_else(|error| {
        eprintln!("sync: {error}");
        false
    });
    paths::set_storage_root(Some(path));
    moved
}

pub fn create(app: &AppHandle, raw: &str) -> Result<SyncStatus, String> {
    let _lock = operation_lock();
    let folder = target_folder(app, raw, false)?;
    relocate_into(app, &folder)?;
    point_at(app, &folder)?;
    crate::profiles::refresh_icons(app);
    refresh_open_profile(app)?;
    remember_chosen(&folder)?;
    set_error(app, None);
    Ok(status(app))
}

pub fn select(app: &AppHandle, raw: &str) -> Result<SyncStatus, String> {
    let _lock = operation_lock();
    let folder = target_folder(app, raw, true)?;
    if !crate::profiles::sync_has_profile_settings(&folder) {
        materialize_legacy(&folder)?;
    }
    if !folder_has_profiles(&folder) {
        return Err("That folder has no profiles.".into());
    }
    crate::profiles::migrate_layout(&folder, false)?;
    point_at(app, &folder)?;
    let _ = crate::profiles::discard_appdata_profiles(&paths::root_data_dir(app));
    crate::profiles::refresh_icons(app);
    refresh_open_profile(app)?;
    remember_chosen(&folder)?;
    set_error(app, None);
    Ok(status(app))
}

pub fn clear(app: &AppHandle) -> Result<SyncStatus, String> {
    let _lock = operation_lock();
    if let Some(folder) = paths::storage_root() {
        crate::profiles::transfer_sync_to_appdata(&folder, &paths::root_data_dir(app))?;
    }
    paths::set_storage_root(None);
    crate::sync_conflicts::rebase(app);
    crate::profiles::refresh_icons(app);
    clear_machine(app)?;
    let mut settings = settings::get(app);
    settings.sync_folder.clear();
    settings.sync_revision = 0;
    settings.sync_bookmarks_revision = 0;
    settings.sync_extensions_revision = 0;
    settings.sync_settings_revision = 0;
    settings.sync_history_revision = 0;
    settings::store(app, settings, false)?;
    refresh_open_profile(app)?;
    *stamp_slot()
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = None;
    DIRTY.store(false, Ordering::Relaxed);
    set_error(app, None);
    Ok(status(app))
}

fn reconcile(app: &AppHandle) -> Result<(), String> {
    let start_gen = GENERATION.load(Ordering::Relaxed);
    let result = reconcile_inner(app, start_gen);
    if result.is_err() {
        DIRTY.store(true, Ordering::Relaxed);
    }
    result
}

fn reconcile_inner(app: &AppHandle, start_gen: u64) -> Result<(), String> {
    if paths::storage_root().is_some() {
        crate::sync_conflicts::scan(app);
        finish(start_gen);
        return Ok(());
    }
    let Ok(folder) = required_folder(app) else {
        finish(start_gen);
        return Ok(());
    };
    if !folder.is_dir() {
        return Err(format!("The sync folder is missing: {}", folder.display()));
    }
    import_legacy(app, &folder)?;
    require_files(&folder)?;
    let stamp =
        folder_stamp(&folder).ok_or_else(|| format!("Could not read {}", folder.display()))?;
    let tick = TICK.fetch_add(1, Ordering::Relaxed);
    let force = tick % 15 == 0;
    let dirty = DIRTY.load(Ordering::Relaxed);
    let same_stamp = stamp_slot()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .as_ref()
        == Some(&stamp);
    if same_stamp && !force && !dirty {
        return Ok(());
    }
    for part in PARTS {
        sync_part(app, &folder, part, dirty)?;
    }
    remember(&folder)?;
    finish(start_gen);
    Ok(())
}

fn sync_part(app: &AppHandle, folder: &Path, part: Part, dirty: bool) -> Result<(), String> {
    let path = folder.join(part.file());
    let remote_revision = read_revision(&path)?;
    let local_revision = part.local_revision(&settings::get(app));
    let same = same_as_local(app, part, &path)?;
    if remote_revision > local_revision {
        if same {
            set_revision(app, part, remote_revision)?;
        } else {
            apply_part(app, part, &path)?;
        }
        return Ok(());
    }
    if remote_revision == local_revision && !dirty {
        return Ok(());
    }
    if same {
        if remote_revision != local_revision {
            set_revision(app, part, remote_revision)?;
        }
        return Ok(());
    }
    write_part(
        app,
        part,
        &path,
        next_revision(remote_revision.max(local_revision)),
    )
}

fn write_part(app: &AppHandle, part: Part, path: &Path, revision: u64) -> Result<(), String> {
    match part {
        Part::Bookmarks => files::write_json(
            path,
            &BookmarksDocument {
                format: FORMAT,
                revision,
                bookmarks: bookmarks::tree(app),
            },
        )?,
        Part::Settings => files::write_json(
            path,
            &SettingsDocument {
                format: FORMAT,
                revision,
                settings: exported(settings::get(app)),
            },
        )?,
        Part::History => files::write_json(
            path,
            &HistoryDocument {
                format: FORMAT,
                revision,
                items: history::list(app),
            },
        )?,
        Part::Extensions => {
            let states = sigs(&extensions::extension_states(app)?);
            let mut packed = Vec::with_capacity(states.len());
            for state in &states {
                let zip = extensions::zip_extension(app, &state.folder)?;
                packed.push(SyncExtension {
                    folder: state.folder.clone(),
                    enabled: state.enabled,
                    hash: state.hash.clone(),
                    archive: base64::engine::general_purpose::STANDARD.encode(zip),
                });
            }
            files::write_json(
                path,
                &ExtensionsDocument {
                    format: FORMAT,
                    revision,
                    extensions: packed,
                },
            )?;
        }
    }
    set_revision(app, part, revision)
}

fn apply_part(app: &AppHandle, part: Part, path: &Path) -> Result<(), String> {
    let value = read_value(path)?;
    let revision = revision_of(&value, path)?;
    APPLYING.store(true, Ordering::Relaxed);
    let result = (|| -> Result<(), String> {
        match part {
            Part::Bookmarks => {
                let document: BookmarksDocument = serde_json::from_value(value)
                    .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
                bookmarks::replace(app, document.bookmarks)?;
            }
            Part::Extensions => {
                let document: ExtensionsDocument = serde_json::from_value(value)
                    .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
                extensions::apply_extensions(app, &decode_extensions(&document.extensions)?)?;
            }
            Part::Settings => {
                let document: SettingsDocument = serde_json::from_value(value)
                    .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
                apply_settings(app, document.settings)?;
            }
            Part::History => {
                let document: HistoryDocument = serde_json::from_value(value)
                    .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
                history::replace(app, document.items)?;
            }
        }
        Ok(())
    })();
    APPLYING.store(false, Ordering::Relaxed);
    result?;
    set_revision(app, part, revision)?;
    GENERATION.fetch_add(1, Ordering::Relaxed);
    DIRTY.store(false, Ordering::Relaxed);
    Ok(())
}

fn apply_settings(app: &AppHandle, remote: BrowserSettings) -> Result<(), String> {
    let current = settings::get(app);
    let mut settings = remote;
    settings.download_dir = current.download_dir;
    settings.legacy_assistant_key = None;
    settings.sync_folder.clone_from(&current.sync_folder);
    settings.sync_revision = current.sync_revision;
    settings.sync_bookmarks_revision = current.sync_bookmarks_revision;
    settings.sync_extensions_revision = current.sync_extensions_revision;
    settings.sync_settings_revision = current.sync_settings_revision;
    settings.sync_history_revision = current.sync_history_revision;
    settings::store(app, settings, false)?;
    Ok(())
}

fn same_as_local(app: &AppHandle, part: Part, path: &Path) -> Result<bool, String> {
    let value = read_value(path)?;
    let remote = match part {
        Part::Bookmarks => {
            let document: BookmarksDocument = serde_json::from_value(value)
                .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
            serde_json::to_string(&document.bookmarks).map_err(|error| error.to_string())?
        }
        Part::Settings => {
            let document: SettingsDocument = serde_json::from_value(value)
                .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
            serde_json::to_string(&exported(document.settings))
                .map_err(|error| error.to_string())?
        }
        Part::History => {
            let document: HistoryDocument = serde_json::from_value(value)
                .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
            serde_json::to_string(&document.items).map_err(|error| error.to_string())?
        }
        Part::Extensions => {
            let document: ExtensionsDocument = serde_json::from_value(value)
                .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
            let remote = document
                .extensions
                .iter()
                .map(|extension| ExtensionSig {
                    folder: extension.folder.clone(),
                    enabled: extension.enabled,
                    hash: extension.hash.clone(),
                })
                .collect::<Vec<_>>();
            return Ok(
                signature(&sigs(&extensions::extension_states(app)?))? == signature(&remote)?
            );
        }
    };
    let local = match part {
        Part::Bookmarks => {
            serde_json::to_string(&bookmarks::tree(app)).map_err(|error| error.to_string())?
        }
        Part::Settings => serde_json::to_string(&exported(settings::get(app)))
            .map_err(|error| error.to_string())?,
        Part::History => {
            serde_json::to_string(&history::list(app)).map_err(|error| error.to_string())?
        }
        Part::Extensions => unreachable!("extensions return above"),
    };
    Ok(remote == local)
}

fn import_legacy(app: &AppHandle, folder: &Path) -> Result<(), String> {
    if all_present(folder) {
        return Ok(());
    }
    let Some(path) = find_legacy(folder) else {
        return Ok(());
    };
    let document = read_legacy(&path)?;
    let revision = document.revision;
    if !folder.join(BOOKMARKS_FILE).is_file() {
        files::write_json(
            &folder.join(BOOKMARKS_FILE),
            &BookmarksDocument {
                format: FORMAT,
                revision,
                bookmarks: document.bookmarks,
            },
        )?;
    }
    if !folder.join(EXTENSIONS_FILE).is_file() {
        files::write_json(
            &folder.join(EXTENSIONS_FILE),
            &ExtensionsDocument {
                format: FORMAT,
                revision,
                extensions: document.extensions,
            },
        )?;
    }
    if !folder.join(SETTINGS_FILE).is_file() {
        files::write_json(
            &folder.join(SETTINGS_FILE),
            &SettingsDocument {
                format: FORMAT,
                revision,
                settings: exported(document.settings),
            },
        )?;
    }
    if !folder.join(HISTORY_FILE).is_file() {
        files::write_json(
            &folder.join(HISTORY_FILE),
            &HistoryDocument {
                format: FORMAT,
                revision,
                items: history::list(app),
            },
        )?;
    }
    Ok(())
}

fn find_legacy(folder: &Path) -> Option<PathBuf> {
    let preferred = folder.join("ce-browser-sync.json");
    if is_legacy_file(&preferred) {
        return Some(preferred);
    }
    let entries = fs::read_dir(folder).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "json")
            && is_legacy_file(&path)
        {
            return Some(path);
        }
    }
    None
}

fn is_legacy_file(path: &Path) -> bool {
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    serde_json::from_str::<Value>(&text)
        .ok()
        .is_some_and(|value| is_legacy_document(&value))
}

fn is_legacy_document(value: &Value) -> bool {
    let Some(format) = value.get("format").and_then(Value::as_u64) else {
        return false;
    };
    format > 0
        && format <= u64::from(FORMAT)
        && value.get("settings").is_some()
        && value.get("bookmarks").is_some()
        && value.get("extensions").is_some()
}

fn read_legacy(path: &Path) -> Result<LegacyDocument, String> {
    let value = read_value(path)?;
    if !is_legacy_document(&value) {
        return Err(format!("{} is not a ce-browser sync file.", path.display()));
    }
    serde_json::from_value(value)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))
}

fn decode_extensions(extensions: &[SyncExtension]) -> Result<Vec<IncomingExtension>, String> {
    extensions
        .iter()
        .map(|extension| {
            let zip = base64::engine::general_purpose::STANDARD
                .decode(extension.archive.trim())
                .map_err(|_| {
                    format!(
                        "The {} extension in the sync folder is damaged.",
                        extension.folder
                    )
                })?;
            Ok(IncomingExtension {
                folder: extension.folder.clone(),
                enabled: extension.enabled,
                hash: extension.hash.clone(),
                zip,
            })
        })
        .collect()
}

fn sigs(states: &[extensions::ExtensionState]) -> Vec<ExtensionSig> {
    states
        .iter()
        .map(|state| ExtensionSig {
            folder: state.folder.clone(),
            enabled: state.enabled,
            hash: state.hash.clone(),
        })
        .collect()
}

fn signature(extensions: &[ExtensionSig]) -> Result<String, String> {
    let mut extensions = extensions.to_vec();
    extensions.sort_by(|left, right| left.folder.cmp(&right.folder));
    serde_json::to_string(&extensions).map_err(|error| error.to_string())
}

/// The sync folder carries bookmarks, extensions, settings, and history. The download folder
/// and the sync folder path stay on each computer.
fn exported(mut settings: BrowserSettings) -> BrowserSettings {
    settings.sync_folder.clear();
    settings.sync_revision = 0;
    settings.sync_bookmarks_revision = 0;
    settings.sync_extensions_revision = 0;
    settings.sync_settings_revision = 0;
    settings.sync_history_revision = 0;
    settings.download_dir.clear();
    settings.legacy_assistant_key = None;
    settings
}

fn set_revision(app: &AppHandle, part: Part, revision: u64) -> Result<(), String> {
    let mut settings = settings::get(app);
    part.set_revision(&mut settings, revision);
    settings::store(app, settings, false)?;
    Ok(())
}

fn required_folder(app: &AppHandle) -> Result<PathBuf, String> {
    let folder = settings::get(app).sync_folder;
    let folder = folder.trim();
    if folder.is_empty() {
        return Err("Choose a sync folder first.".into());
    }
    Ok(PathBuf::from(folder))
}

fn target_folder(app: &AppHandle, raw: &str, must_exist: bool) -> Result<PathBuf, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("Choose a folder.".into());
    }
    let path = std::path::absolute(raw).map_err(|error| error.to_string())?;
    if path.is_file() {
        return Err("Choose a folder, not a file.".into());
    }
    if must_exist && !path.is_dir() {
        return Err(format!("Folder does not exist: {}", path.display()));
    }
    if !must_exist {
        fs::create_dir_all(&path)
            .map_err(|error| format!("Could not create {}: {error}", path.display()))?;
    }
    if path.starts_with(paths::root_data_dir(app)) || path.starts_with(paths::root_local_dir(app)) {
        return Err(
            "That folder is inside the browser data folder. Choose a folder on the shared drive."
                .into(),
        );
    }
    Ok(path)
}

fn all_present(folder: &Path) -> bool {
    PARTS.iter().all(|part| folder.join(part.file()).is_file())
}

fn require_files(folder: &Path) -> Result<(), String> {
    for part in PARTS {
        if !folder.join(part.file()).is_file() {
            return Err(format!("That folder is missing {}.", part.file()));
        }
    }
    Ok(())
}

fn read_value(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|_| format!("{} is not a ce-browser sync file.", path.display()))?;
    require_format(&value)?;
    Ok(value)
}

fn read_revision(path: &Path) -> Result<u64, String> {
    revision_of(&read_value(path)?, path)
}

fn revision_of(value: &Value, path: &Path) -> Result<u64, String> {
    value
        .get("revision")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{} is not a ce-browser sync file.", path.display()))
}

fn require_format(value: &Value) -> Result<(), String> {
    let Some(format) = value.get("format").and_then(Value::as_u64) else {
        return Err("That file is not a ce-browser sync file.".into());
    };
    if format == 0 {
        return Err("That file is not a ce-browser sync file.".into());
    }
    if format > u64::from(FORMAT) {
        return Err("That sync folder was written by a newer version of ce-browser.".into());
    }
    Ok(())
}

fn remember(folder: &Path) -> Result<(), String> {
    let stamp =
        folder_stamp(folder).ok_or_else(|| format!("Could not read {}", folder.display()))?;
    *stamp_slot()
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = Some(stamp);
    Ok(())
}

/// Profile folders keep bookmarks and settings inside each profile, not as the four files
/// at the sync folder root. Choosing one of those still counts as selected.
fn remember_chosen(folder: &Path) -> Result<(), String> {
    if all_present(folder) {
        return remember(folder);
    }
    *stamp_slot()
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = None;
    Ok(())
}

fn folder_stamp(folder: &Path) -> Option<FolderStamp> {
    Some(FolderStamp {
        bookmarks: file_stamp(&folder.join(BOOKMARKS_FILE))?,
        extensions: file_stamp(&folder.join(EXTENSIONS_FILE))?,
        settings: file_stamp(&folder.join(SETTINGS_FILE))?,
        history: file_stamp(&folder.join(HISTORY_FILE))?,
    })
}

fn file_stamp(path: &Path) -> Option<Stamp> {
    let meta = fs::metadata(path).ok()?;
    let modified_ms = meta
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_millis();
    Some(Stamp {
        modified_ms,
        len: meta.len(),
    })
}

fn finish(start_gen: u64) {
    if GENERATION.load(Ordering::Relaxed) == start_gen {
        DIRTY.store(false, Ordering::Relaxed);
    }
}

fn set_error(app: &AppHandle, error: Option<String>) {
    let problem = error.map(|message| SyncProblem {
        save: is_save_error(&message),
        message,
    });
    *error_slot()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner()) = problem;
    publish(app);
}

/// A save error is a failure to create or write a file in the sync folder.
fn is_save_error(message: &str) -> bool {
    message.contains("Could not write") || message.contains("Could not create")
}

fn publish(app: &AppHandle) {
    let next = status(app);
    let mut last = LAST_STATUS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if last.as_ref() == Some(&next) {
        return;
    }
    *last = Some(next.clone());
    let _ = app.emit("sync-status", &next);
}

fn operation_lock() -> std::sync::MutexGuard<'static, ()> {
    OPERATION
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

fn error_slot() -> &'static Mutex<Option<SyncProblem>> {
    &ERROR
}

fn stamp_slot() -> &'static Mutex<Option<FolderStamp>> {
    &LAST_STAMP
}

fn active_folder(app: &AppHandle) -> String {
    if let Some(root) = paths::storage_root() {
        return root.to_string_lossy().into_owned();
    }
    settings::get(app).sync_folder
}

fn saved_folder(app: &AppHandle) -> Option<String> {
    let machine: MachineFolder = files::read_json(&paths::machine_sync_file(app));
    let folder = machine.folder.trim();
    if !folder.is_empty() {
        return Some(folder.to_string());
    }
    legacy_sync_folder(app)
}

fn legacy_sync_folder(app: &AppHandle) -> Option<String> {
    let appdata = paths::root_data_dir(app);
    let Ok(text) = fs::read_to_string(crate::profiles::default_dir(&appdata).join("settings.json"))
        .or_else(|_| fs::read_to_string(appdata.join("settings.json")))
    else {
        return None;
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return None;
    };
    let raw = value
        .get("syncFolder")
        .or_else(|| value.get("syncFile"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if raw.is_empty() {
        return None;
    }
    let path = PathBuf::from(raw);
    let folder = if path.is_file() {
        path.parent().map(Path::to_path_buf).unwrap_or(path)
    } else {
        path
    };
    if folder.as_os_str().is_empty() {
        None
    } else {
        Some(folder.to_string_lossy().into_owned())
    }
}

fn write_machine(app: &AppHandle, folder: &Path) -> Result<(), String> {
    files::write_json(
        &paths::machine_sync_file(app),
        &MachineFolder {
            folder: path_text(folder),
        },
    )
}

fn clear_machine(app: &AppHandle) -> Result<(), String> {
    files::write_json(
        &paths::machine_sync_file(app),
        &MachineFolder {
            folder: String::new(),
        },
    )
}

fn point_at(app: &AppHandle, folder: &Path) -> Result<(), String> {
    write_machine(app, folder)?;
    paths::set_storage_root(Some(folder.to_path_buf()));
    crate::sync_conflicts::rebase(app);
    Ok(())
}

fn relocate_into(app: &AppHandle, folder: &Path) -> Result<(), String> {
    if let Some(current) = paths::storage_root() {
        if current == folder {
            return Ok(());
        }
        crate::profiles::transfer_sync_to_sync(&current, folder)?;
    } else {
        crate::profiles::transfer_appdata_to_sync(&paths::root_data_dir(app), folder)?;
    }
    Ok(())
}

fn refresh_open_profile(app: &AppHandle) -> Result<(), String> {
    settings::reload(app);
    if let Some(root) = paths::storage_root() {
        let mut settings = settings::get(app);
        settings.sync_folder = path_text(&root);
        settings::store(app, settings, false)?;
    }
    bookmarks::reload(app);
    history::reload(app);
    crate::downloads::reload(app);
    let _ = crate::user_styles::reload(app);
    crate::extensions::reload(app)
}

fn folder_has_profiles(folder: &Path) -> bool {
    folder.join("profiles.json").is_file() || crate::profiles::sync_has_profile_settings(folder)
}

fn materialize_legacy(folder: &Path) -> Result<(), String> {
    let dest = folder.join(crate::profiles::DEFAULT_FOLDER);
    if crate::profiles::sync_has_profile_settings(folder) {
        return Ok(());
    }
    let settings = wrapped_settings(&folder.join(SETTINGS_FILE));
    let bookmarks = wrapped_bookmarks(&folder.join(BOOKMARKS_FILE));
    let history = wrapped_history(&folder.join(HISTORY_FILE));
    let extensions = wrapped_extensions(&folder.join(EXTENSIONS_FILE));
    let legacy =
        if settings.is_none() && bookmarks.is_none() && history.is_none() && extensions.is_none() {
            find_legacy(folder).and_then(|path| read_legacy(&path).ok())
        } else {
            None
        };
    if settings.is_none()
        && bookmarks.is_none()
        && history.is_none()
        && extensions.is_none()
        && legacy.is_none()
    {
        return Ok(());
    }
    fs::create_dir_all(&dest).map_err(|error| error.to_string())?;
    let mut settings = settings
        .or_else(|| legacy.as_ref().map(|document| document.settings.clone()))
        .unwrap_or_default();
    settings.sync_folder = path_text(folder);
    files::write_json(&dest.join("settings.json"), &settings)?;
    let bookmarks = bookmarks
        .or_else(|| legacy.as_ref().map(|document| document.bookmarks.clone()))
        .unwrap_or_default();
    files::write_json(&dest.join("bookmarks.json"), &bookmarks)?;
    let history = history.unwrap_or_default();
    files::write_json(&dest.join("history.json"), &RawHistory { items: history })?;
    let extensions = extensions
        .or_else(|| legacy.map(|document| document.extensions))
        .unwrap_or_default();
    let decoded = decode_extensions(&extensions)?;
    if !decoded.is_empty() {
        crate::extensions::stage_incoming(&dest, &decoded)?;
    }
    Ok(())
}

fn wrapped_settings(path: &Path) -> Option<BrowserSettings> {
    let value = read_value(path).ok()?;
    serde_json::from_value::<SettingsDocument>(value)
        .ok()
        .map(|document| document.settings)
}

fn wrapped_bookmarks(path: &Path) -> Option<BookmarkTree> {
    let value = read_value(path).ok()?;
    serde_json::from_value::<BookmarksDocument>(value)
        .ok()
        .map(|document| document.bookmarks)
}

fn wrapped_history(path: &Path) -> Option<Vec<HistoryItem>> {
    let value = read_value(path).ok()?;
    serde_json::from_value::<HistoryDocument>(value)
        .ok()
        .map(|document| document.items)
}

fn wrapped_extensions(path: &Path) -> Option<Vec<SyncExtension>> {
    let value = read_value(path).ok()?;
    serde_json::from_value::<ExtensionsDocument>(value)
        .ok()
        .map(|document| document.extensions)
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct MachineFolder {
    folder: String,
}

#[derive(Serialize)]
struct RawHistory {
    items: Vec<HistoryItem>,
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn next_revision(current: u64) -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(1)
        .max(1);
    if now > current {
        now
    } else {
        current.saturating_add(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_combined_sync_file_is_legacy_and_a_part_file_is_not() {
        let legacy = serde_json::json!({
            "format": 1,
            "revision": 4,
            "settings": {},
            "bookmarks": {},
            "extensions": []
        });
        let bookmarks = serde_json::json!({
            "format": 1,
            "revision": 4,
            "bookmarks": {}
        });
        assert!(is_legacy_document(&legacy));
        assert!(!is_legacy_document(&bookmarks));
        assert!(!is_legacy_document(
            &serde_json::json!({"format": 1, "items": []})
        ));
    }

    #[test]
    fn a_write_failure_is_a_save_error_and_a_read_failure_is_not() {
        assert!(is_save_error(
            r"Could not write D:\share\settings.json: Access is denied. (os error 5)"
        ));
        assert!(is_save_error(
            r"Could not create D:\share: Access is denied. (os error 5)"
        ));
        assert!(!is_save_error(
            r"Could not read D:\share\bookmarks.json: The system cannot find the file specified. (os error 2)"
        ));
        assert!(!is_save_error(r"The sync folder is missing: D:\share"));
    }
}
