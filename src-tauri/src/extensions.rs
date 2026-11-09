use std::collections::HashSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::time::Duration;

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};
use url::Url;
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use webview2_com::{
    BrowserExtensionEnableCompletedHandler, BrowserExtensionRemoveCompletedHandler,
    ProfileAddBrowserExtensionCompletedHandler, ProfileGetBrowserExtensionsCompletedHandler,
};
use windows::Win32::Foundation::E_POINTER;
use windows::core::{HSTRING, Interface};

use crate::extension_store::{self, VerifiedCrx};
use crate::webview2::{read_bool, read_pwstr};
use crate::{files, paths, tabs};

const FILE: &str = "extensions.json";
const PROFILE_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub enabled: bool,
    /// `data:` URL for the toolbar button.
    pub icon: Option<String>,
    /// Path of the toolbar popup inside the extension, e.g. `popup/index.html`.
    pub popup: Option<String>,
    pub options_page: Option<String>,
    /// `action.default_title`, localized. Empty when the manifest does not set one.
    pub default_title: String,
    /// `side_panel.default_path`, or Edge's `sidebar_action.default_panel` when that is the only one.
    pub side_panel: Option<String>,
    /// `chrome_url_overrides.newtab`, when this package supplies one.
    pub new_tab: Option<String>,
    /// Commands that name a suggested shortcut.
    pub commands: Vec<ExtensionCommand>,
    pub folder: Option<String>,
    /// `chrome` or `edge` when the package has a store key and update URL.
    pub store: Option<String>,
    /// Newer store version waiting for the user to install it.
    pub update_version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionCommand {
    pub name: String,
    pub description: String,
    /// Suggested shortcut. The Windows value wins over `default` when both are set.
    pub shortcut: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct ExtensionRecord {
    id: String,
    /// Folder name inside `<data>/extensions`.
    folder: String,
    /// Newer store version waiting for the user. Empty when there is nothing to install.
    #[serde(default)]
    available_version: String,
    /// Increases when the extension is installed or enabled. The highest value among enabled
    /// extensions wins `chrome_url_overrides.newtab`.
    #[serde(default)]
    enabled_seq: u64,
}

static STORE_BUSY: Mutex<()> = Mutex::new(());

/// Shortcuts and the new-tab override, refreshed whenever the extension list is built.
struct ExtensionRuntime {
    new_tab: Option<String>,
    /// An enabled extension declares `declarativeNetRequest` or `webRequest`.
    network: bool,
    /// An enabled extension declares `downloads`.
    downloads: bool,
}

/// A `commands` shortcut. WebView2 does not deliver `chrome.commands.onCommand`, so the
/// browser keeps the chord. Parsing is tested for when that changes.
#[cfg(test)]
#[derive(Clone, Copy)]
struct Chord {
    key: u32,
    ctrl: bool,
    shift: bool,
    alt: bool,
}

static RUNTIME: Mutex<ExtensionRuntime> = Mutex::new(ExtensionRuntime {
    new_tab: None,
    network: false,
    downloads: false,
});

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct ExtensionsFile {
    extensions: Vec<ExtensionRecord>,
}

pub struct Extensions(Mutex<Vec<ExtensionRecord>>);

struct Installed {
    id: String,
    name: String,
    enabled: bool,
}

pub fn init(app: &AppHandle) {
    crate::sync_conflicts::note(app, &paths::data_file(app, FILE));
    let file: ExtensionsFile = crate::sync_conflicts::read_json(app, &paths::data_file(app, FILE));
    app.manage(Extensions(Mutex::new(file.extensions)));
}

fn records(app: &AppHandle) -> Vec<ExtensionRecord> {
    app.state::<Extensions>()
        .0
        .lock()
        .expect("extensions lock")
        .clone()
}

fn save_records(
    app: &AppHandle,
    change: impl FnOnce(&mut Vec<ExtensionRecord>),
) -> Result<(), String> {
    let snapshot = {
        let state = app.state::<Extensions>();
        let mut records = state.0.lock().expect("extensions lock");
        change(&mut records);
        records.clone()
    };
    crate::sync_conflicts::store(
        app,
        &paths::data_file(app, FILE),
        &ExtensionsFile {
            extensions: snapshot,
        },
    )?;
    crate::sync::touch();
    Ok(())
}

fn error_text(error: windows::core::Error) -> String {
    let message = error.message();
    if message.is_empty() {
        format!("{error:?}")
    } else {
        message
    }
}

/// Runs `work` on the main thread with the shared WebView2 profile and waits for it to report
/// through `tx`. Must be called off the main thread.
fn with_profile<T: Send + 'static>(
    app: &AppHandle,
    work: impl FnOnce(ICoreWebView2Profile7, Sender<Result<T, String>>) + Send + 'static,
) -> Result<T, String> {
    let webview = tabs::any_ui(app).ok_or("The browser UI is not ready yet.")?;
    let (tx, rx) = mpsc::channel();
    let fail = tx.clone();
    webview
        .with_webview(move |platform| {
            let profile = unsafe {
                platform
                    .controller()
                    .CoreWebView2()
                    .and_then(|core| core.cast::<ICoreWebView2_13>())
                    .and_then(|core| core.Profile())
                    .and_then(|profile| profile.cast::<ICoreWebView2Profile7>())
            };
            match profile {
                Ok(profile) => work(profile, tx),
                Err(error) => {
                    let _ = fail.send(Err(format!(
                        "This WebView2 runtime does not support extensions. Update Microsoft Edge WebView2. ({})",
                        error_text(error)
                    )));
                }
            }
        })
        .map_err(|error| error.to_string())?;
    rx.recv_timeout(PROFILE_TIMEOUT)
        .map_err(|_| "WebView2 did not answer.".to_string())?
}

fn installed(app: &AppHandle) -> Result<Vec<Installed>, String> {
    with_profile(app, |profile, tx| {
        let fail = tx.clone();
        let handler =
            ProfileGetBrowserExtensionsCompletedHandler::create(Box::new(move |result, list| {
                let outcome = (|| -> windows::core::Result<Vec<Installed>> {
                    result?;
                    let list = list.ok_or_else(|| windows::core::Error::from(E_POINTER))?;
                    let mut count = 0u32;
                    unsafe { list.Count(&mut count)? };
                    let mut items = Vec::new();
                    for index in 0..count {
                        let extension = unsafe { list.GetValueAtIndex(index)? };
                        items.push(Installed {
                            id: read_pwstr(|value| unsafe { extension.Id(value) }),
                            name: read_pwstr(|value| unsafe { extension.Name(value) }),
                            enabled: read_bool(|value| unsafe { extension.IsEnabled(value) }),
                        });
                    }
                    Ok(items)
                })();
                let _ = tx.send(outcome.map_err(error_text));
                Ok(())
            }));
        if let Err(error) = unsafe { profile.GetBrowserExtensions(&handler) } {
            let _ = fail.send(Err(error_text(error)));
        }
    })
}

fn add_to_profile(app: &AppHandle, folder: &Path) -> Result<(String, String), String> {
    let path = HSTRING::from(folder.to_string_lossy().as_ref());
    with_profile(app, move |profile, tx| {
        let fail = tx.clone();
        let handler = ProfileAddBrowserExtensionCompletedHandler::create(Box::new(
            move |result, extension| {
                let outcome = (|| -> windows::core::Result<(String, String)> {
                    result?;
                    let extension =
                        extension.ok_or_else(|| windows::core::Error::from(E_POINTER))?;
                    Ok((
                        read_pwstr(|value| unsafe { extension.Id(value) }),
                        read_pwstr(|value| unsafe { extension.Name(value) }),
                    ))
                })();
                let _ = tx.send(outcome.map_err(error_text));
                Ok(())
            },
        ));
        if let Err(error) = unsafe { profile.AddBrowserExtension(&path, &handler) } {
            let _ = fail.send(Err(error_text(error)));
        }
    })
}

/// Finds the extension by id, then enables, disables or removes it.
fn act_on(app: &AppHandle, id: &str, enable: Option<bool>) -> Result<(), String> {
    let id = id.to_string();
    with_profile(app, move |profile, tx| {
        let fail = tx.clone();
        let handler =
            ProfileGetBrowserExtensionsCompletedHandler::create(Box::new(move |result, list| {
                let found = (|| -> windows::core::Result<Option<ICoreWebView2BrowserExtension>> {
                    result?;
                    let list = list.ok_or_else(|| windows::core::Error::from(E_POINTER))?;
                    let mut count = 0u32;
                    unsafe { list.Count(&mut count)? };
                    for index in 0..count {
                        let extension = unsafe { list.GetValueAtIndex(index)? };
                        if read_pwstr(|value| unsafe { extension.Id(value) }) == id {
                            return Ok(Some(extension));
                        }
                    }
                    Ok(None)
                })();
                let extension = match found {
                    Ok(Some(extension)) => extension,
                    Ok(None) => {
                        let _ = tx.send(Err("That extension is not installed.".into()));
                        return Ok(());
                    }
                    Err(error) => {
                        let _ = tx.send(Err(error_text(error)));
                        return Ok(());
                    }
                };
                let done = tx.clone();
                let started = match enable {
                    Some(enabled) => unsafe {
                        extension.Enable(
                            enabled,
                            &BrowserExtensionEnableCompletedHandler::create(Box::new(
                                move |result| {
                                    let _ = done.send(result.map_err(error_text));
                                    Ok(())
                                },
                            )),
                        )
                    },
                    None => unsafe {
                        extension.Remove(&BrowserExtensionRemoveCompletedHandler::create(Box::new(
                            move |result| {
                                let _ = done.send(result.map_err(error_text));
                                Ok(())
                            },
                        )))
                    },
                };
                if let Err(error) = started {
                    let _ = tx.send(Err(error_text(error)));
                }
                Ok(())
            }));
        if let Err(error) = unsafe { profile.GetBrowserExtensions(&handler) } {
            let _ = fail.send(Err(error_text(error)));
        }
    })
}

pub fn list(app: &AppHandle) -> Result<Vec<ExtensionInfo>, String> {
    let installed = installed(app)?;
    let records = records(app);
    let root = paths::extensions_dir(app);
    let mut network = false;
    let mut downloads = false;
    let infos = installed
        .into_iter()
        .map(|extension| {
            let record = records.iter().find(|record| record.id == extension.id);
            let folder = record.map(|record| record.folder.clone());
            let manifest = folder
                .as_ref()
                .map(|folder| Manifest::read(&root.join(folder)))
                .unwrap_or_default();
            if extension.enabled && manifest.network {
                network = true;
            }
            if extension.enabled && manifest.downloads {
                downloads = true;
            }
            let version = manifest.version.clone().unwrap_or_default();
            let update_version = record.and_then(|record| {
                extension_store::version_is_newer(&record.available_version, &version)
                    .then(|| record.available_version.clone())
            });
            ExtensionInfo {
                name: manifest
                    .name
                    .clone()
                    .filter(|name| !name.is_empty())
                    .unwrap_or(extension.name),
                id: extension.id,
                version,
                description: manifest.description.clone().unwrap_or_default(),
                enabled: extension.enabled,
                icon: manifest.icon.clone(),
                popup: manifest.popup.clone(),
                options_page: manifest.options_page.clone(),
                default_title: manifest.default_title.clone().unwrap_or_default(),
                side_panel: manifest.side_panel.clone(),
                new_tab: manifest.new_tab.clone(),
                commands: manifest.commands.clone(),
                folder,
                store: manifest.store.map(|store| store.as_str().to_string()),
                update_version,
            }
        })
        .collect::<Vec<_>>();
    remember_runtime(&records, &infos, network, downloads);
    Ok(infos)
}

fn emit(app: &AppHandle) {
    if let Ok(list) = list(app) {
        let _ = app.emit("extensions-changed", &list);
    }
}

/// Loads `extensions.json` into memory without installing or rewriting it.
pub fn adopt_saved(app: &AppHandle) {
    let file: ExtensionsFile = crate::sync_conflicts::read_json(app, &paths::data_file(app, FILE));
    *app.state::<Extensions>().0.lock().expect("extensions lock") = file.extensions;
    emit(app);
}

/// Extensions persist in the WebView2 profile. Folders dropped into `<data>/extensions` by hand,
/// or a profile that was reset, get (re)added here.
pub fn reload(app: &AppHandle) -> Result<(), String> {
    let file: ExtensionsFile = crate::sync_conflicts::read_json(app, &paths::data_file(app, FILE));
    *app.state::<Extensions>().0.lock().expect("extensions lock") = file.extensions;
    let _ = fs::remove_file(extensions_root_marker(app));
    sync_on_startup(app)
}

pub fn sync_on_startup(app: &AppHandle) -> Result<(), String> {
    if extensions_root_changed(app) {
        if let Ok(installed) = installed(app) {
            for extension in installed {
                let _ = act_on(app, &extension.id, None);
            }
        }
    }
    let root = paths::extensions_dir(app);
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let installed = installed(app)?;
    let mut records = records(app);
    let mut changed = false;

    for record in records.clone() {
        if !root.join(&record.folder).join("manifest.json").is_file() {
            if installed.iter().any(|extension| extension.id == record.id) {
                let _ = act_on(app, &record.id, None);
            }
            records.retain(|kept| kept.folder != record.folder);
            changed = true;
        }
    }

    let entries = fs::read_dir(&root).map_err(|error| error.to_string())?;
    for entry in entries.flatten() {
        let folder = entry.path();
        if !folder.join("manifest.json").is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let known = records.iter().any(|record| {
            record.folder == name && installed.iter().any(|extension| extension.id == record.id)
        });
        if known {
            continue;
        }
        match add_to_profile(app, &folder) {
            Ok((id, _)) => {
                let available_version = records
                    .iter()
                    .find(|record| record.folder == name)
                    .map(|record| record.available_version.clone())
                    .unwrap_or_default();
                let available_version = kept_available(&folder, &available_version);
                records.retain(|record| record.folder != name && record.id != id);
                let enabled_seq = next_seq(&records);
                records.push(ExtensionRecord {
                    id,
                    folder: name,
                    available_version,
                    enabled_seq,
                });
                changed = true;
            }
            Err(error) => eprintln!("extension {}: {error}", folder.display()),
        }
    }

    if changed {
        save_records(app, |saved| *saved = records)?;
    }
    remember_extensions_root(app);
    emit(app);
    Ok(())
}

fn extensions_root_marker(app: &AppHandle) -> PathBuf {
    paths::profile_webview_dir(app, &crate::profiles::active_id()).join("extensions-root.txt")
}

fn extensions_root_changed(app: &AppHandle) -> bool {
    let current = paths::extensions_dir(app).to_string_lossy().into_owned();
    fs::read_to_string(extensions_root_marker(app))
        .ok()
        .map(|saved| saved.trim() != current)
        .unwrap_or(true)
}

fn remember_extensions_root(app: &AppHandle) {
    let path = extensions_root_marker(app);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, paths::extensions_dir(app).to_string_lossy().as_ref());
}

pub fn install_folder(app: &AppHandle, source: &Path) -> Result<ExtensionInfo, String> {
    let source =
        find_manifest_root(source).ok_or("That folder does not contain a manifest.json.")?;
    let manifest = Manifest::read(&source);
    let root = paths::extensions_dir(app);
    let target = unique_folder(&root, manifest.name.as_deref().unwrap_or("extension"));
    files::copy_dir(&source, &target)?;
    install_from(app, target, None)
}

pub fn install_zip(app: &AppHandle, archive: &Path) -> Result<ExtensionInfo, String> {
    let bytes = fs::read(archive)
        .map_err(|error| format!("Could not open {}: {error}", archive.display()))?;
    if bytes.len() > MAX_EXTENSION_ZIP {
        return Err("That extension is too large to install.".into());
    }
    let decoded = extension_store::decode_archive(&bytes)?;
    let root = paths::extensions_dir(app);
    let unpacked = unpack_zip(&decoded.zip)?;
    let manifest = Manifest::read(&unpacked.manifest);
    let target = unique_folder(&root, manifest.name.as_deref().unwrap_or("extension"));
    unpacked.into_dir(&target)?;
    install_from(app, target, decoded.public_key.as_deref())
}

pub fn install_from_store(app: &AppHandle, url: &str) -> Result<ExtensionInfo, String> {
    let _busy = store_busy();
    let listing = extension_store::parse_listing(url)?;
    let verified = verified_download(listing.store, &listing.id)?;
    if records(app).iter().any(|record| record.id == listing.id) {
        return match update_verified(app, &listing.id, &verified) {
            Err(error) if error == "That extension is already up to date." => {
                Err("That extension is already installed.".into())
            }
            result => result,
        };
    }
    let unpacked = unpack_zip(&verified.zip)?;
    let manifest = Manifest::read(&unpacked.manifest);
    let target = unique_folder(
        &paths::extensions_dir(app),
        manifest.name.as_deref().unwrap_or("extension"),
    );
    unpacked.into_dir(&target)?;
    install_from(app, target, Some(verified.public_key.as_slice()))
}

pub fn update_extension(app: &AppHandle, id: &str) -> Result<ExtensionInfo, String> {
    let _busy = store_busy();
    let record = records(app)
        .into_iter()
        .find(|record| record.id == id)
        .ok_or_else(|| "That extension is not installed.".to_string())?;
    let manifest = Manifest::read(&paths::extensions_dir(app).join(&record.folder));
    let store = manifest.store.ok_or_else(|| {
        "That extension did not come from the Chrome Web Store or Edge Add-ons.".to_string()
    })?;
    let verified = verified_download(store, id)?;
    update_verified(app, id, &verified)
}

/// Checks store extensions for a newer version. Nothing is installed.
pub fn check_store_updates(app: &AppHandle, announce: bool) -> Result<Vec<ExtensionInfo>, String> {
    let watches = store_watches(app);
    if watches.is_empty() {
        return list(app);
    }
    let mut noted = Vec::new();
    let mut errors = Vec::new();
    for watch in &watches {
        match extension_store::latest_version(watch.store, &watch.id, &watch.version) {
            Ok(version) => noted.push((watch.id.clone(), version.unwrap_or_default())),
            Err(error) => {
                eprintln!("extension update {}: {error}", watch.id);
                errors.push(error);
            }
        }
    }
    if noted.is_empty() && !errors.is_empty() {
        return Err(errors.remove(0));
    }
    {
        let _busy = store_busy();
        note_available_versions(app, &noted)?;
    }
    if announce {
        announce_updates(app);
    }
    list(app)
}

fn install_from(
    app: &AppHandle,
    target: PathBuf,
    public_key: Option<&[u8]>,
) -> Result<ExtensionInfo, String> {
    let folder = target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    prepare_extension_dir(&target, public_key).inspect_err(|_| {
        let _ = fs::remove_dir_all(&target);
    })?;
    let expected = public_key.map(extension_store::extension_id);
    if expected
        .as_ref()
        .is_some_and(|expected| records(app).iter().any(|record| record.id == *expected))
    {
        let _ = fs::remove_dir_all(&target);
        return Err("That extension is already installed.".into());
    }
    let (id, _) = add_to_profile(app, &target).inspect_err(|_| {
        let _ = fs::remove_dir_all(&target);
    })?;
    if expected.as_ref().is_some_and(|expected| expected != &id) {
        let _ = act_on(app, &id, None);
        let _ = fs::remove_dir_all(&target);
        return Err("WebView2 gave this extension a different id, so it was not installed.".into());
    }
    let replaced: Vec<String> = records(app)
        .into_iter()
        .filter(|record| record.id == id && record.folder != folder)
        .map(|record| record.folder)
        .collect();
    for old in replaced {
        let _ = fs::remove_dir_all(paths::extensions_dir(app).join(old));
    }
    save_records(app, |records| {
        records.retain(|record| record.id != id);
        let enabled_seq = next_seq(records);
        records.push(ExtensionRecord {
            id: id.clone(),
            folder,
            available_version: String::new(),
            enabled_seq,
        });
    })?;
    emit(app);
    list(app)?
        .into_iter()
        .find(|extension| extension.id == id)
        .ok_or_else(|| "The extension was added but WebView2 does not list it.".to_string())
}

fn verified_download(store: extension_store::Store, id: &str) -> Result<VerifiedCrx, String> {
    let bytes = extension_store::download_crx(store, id)?;
    let verified = extension_store::open_crx(&bytes)?;
    if extension_store::extension_id(&verified.public_key) != id {
        return Err("The downloaded package is not the extension that was requested.".into());
    }
    Ok(verified)
}

fn update_verified(
    app: &AppHandle,
    id: &str,
    verified: &VerifiedCrx,
) -> Result<ExtensionInfo, String> {
    let record = records(app)
        .into_iter()
        .find(|record| record.id == id)
        .ok_or_else(|| "That extension is not installed.".to_string())?;
    let dest = paths::extensions_dir(app).join(&record.folder);
    let manifest = Manifest::read(&dest);
    let saved = manifest.key.ok_or_else(|| {
        "That extension has no saved key, so it cannot be updated from the store.".to_string()
    })?;
    if saved != verified.public_key {
        return Err("The update is signed by a different key, so it was not installed.".into());
    }
    let unpacked = unpack_zip(&verified.zip)?;
    prepare_extension_dir(&unpacked.manifest, Some(verified.public_key.as_slice()))?;
    let new_version = Manifest::read(&unpacked.manifest)
        .version
        .unwrap_or_default();
    let current = manifest.version.unwrap_or_default();
    if !extension_store::version_is_newer(&new_version, &current) {
        return Err("That extension is already up to date.".into());
    }
    let enabled = installed(app)?
        .iter()
        .find(|item| item.id == id)
        .map(|item| item.enabled)
        .unwrap_or(true);
    act_on(app, id, None)?;
    let backup = match swap_extension_dir(&dest, unpacked) {
        Ok(backup) => backup,
        Err(error) => {
            restore_installed(app, &dest, enabled);
            return Err(error);
        }
    };
    match add_to_profile(app, &dest) {
        Ok((new_id, _)) if new_id == id => {
            let _ = fs::remove_dir_all(&backup);
            let _ = act_on(app, id, Some(enabled));
        }
        Ok((new_id, _)) => {
            let _ = act_on(app, &new_id, None);
            restore_tree(&dest, &backup);
            restore_installed(app, &dest, enabled);
            return Err(
                "WebView2 gave this extension a different id, so the update was not installed."
                    .into(),
            );
        }
        Err(error) => {
            restore_tree(&dest, &backup);
            restore_installed(app, &dest, enabled);
            return Err(error);
        }
    }
    save_records(app, |records| {
        if let Some(record) = records.iter_mut().find(|record| record.id == id) {
            record.available_version.clear();
        }
    })?;
    emit(app);
    list(app)?
        .into_iter()
        .find(|extension| extension.id == id)
        .ok_or_else(|| "The extension was updated but WebView2 does not list it.".to_string())
}

pub fn set_enabled(app: &AppHandle, id: &str, enabled: bool) -> Result<(), String> {
    act_on(app, id, Some(enabled))?;
    if enabled {
        save_records(app, |records| {
            let seq = next_seq(records);
            if let Some(record) = records.iter_mut().find(|record| record.id == id) {
                record.enabled_seq = seq;
            }
        })?;
    }
    emit(app);
    crate::sync::touch();
    Ok(())
}

pub fn remove(app: &AppHandle, id: &str) -> Result<(), String> {
    act_on(app, id, None)?;
    let folders: Vec<String> = records(app)
        .into_iter()
        .filter(|record| record.id == id)
        .map(|record| record.folder)
        .collect();
    for folder in folders {
        let _ = fs::remove_dir_all(paths::extensions_dir(app).join(folder));
    }
    save_records(app, |records| records.retain(|record| record.id != id))?;
    emit(app);
    Ok(())
}

fn extension_url(id: &str, page: &str) -> Result<Url, String> {
    Url::parse(&format!(
        "chrome-extension://{id}/{}",
        page.trim_start_matches('/')
    ))
    .map_err(|error| error.to_string())
}

/// `params` is the extension id. Opens its toolbar popup page.
pub fn popup_url(app: &AppHandle, params: &str) -> Result<Url, String> {
    let id = params.trim_start_matches("id=");
    let manifest = manifest_for(app, id)?;
    let popup = manifest
        .popup
        .ok_or("That extension has no toolbar popup.")?;
    extension_url(id, &popup)
}

/// Page shown when the toolbar opens this extension as a side panel.
/// A popup stays the panel document. A side-panel-only extension uses its panel page.
pub fn store_panel_url(app: &AppHandle, id: &str) -> Result<Url, String> {
    let manifest = manifest_for(app, id)?;
    if let Some(popup) = manifest.popup {
        return extension_url(id, &popup);
    }
    if let Some(panel) = manifest.side_panel {
        return extension_url(id, &panel);
    }
    Err("That extension has no panel page.".into())
}

/// Page `chrome.sidePanel.open` asks for. The side panel document wins over the popup.
pub fn side_panel_url(app: &AppHandle, id: &str) -> Result<Url, String> {
    let manifest = manifest_for(app, id)?;
    if let Some(panel) = manifest.side_panel {
        return extension_url(id, &panel);
    }
    if let Some(popup) = manifest.popup {
        return extension_url(id, &popup);
    }
    Err("That extension has no panel page.".into())
}

/// New tab page from the most recently enabled extension that supplies one.
pub fn new_tab_url() -> Option<Url> {
    let runtime = RUNTIME.lock().unwrap_or_else(|poison| poison.into_inner());
    let text = runtime.new_tab.clone()?;
    Url::parse(&text).ok()
}

/// An enabled extension wants to see network requests through `declarativeNetRequest` or `webRequest`.
pub fn extension_handles_requests() -> bool {
    RUNTIME
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .network
}

/// An enabled extension reads downloads through `chrome.downloads`.
pub fn extension_uses_downloads() -> bool {
    RUNTIME
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .downloads
}

fn manifest_for(app: &AppHandle, id: &str) -> Result<Manifest, String> {
    let record = records(app)
        .into_iter()
        .find(|record| record.id == id)
        .ok_or("That extension is not installed.")?;
    Ok(Manifest::read(
        &paths::extensions_dir(app).join(record.folder),
    ))
}

fn next_seq(records: &[ExtensionRecord]) -> u64 {
    records
        .iter()
        .map(|record| record.enabled_seq)
        .max()
        .unwrap_or(0)
        .saturating_add(1)
}

fn newer_override(current: Option<(u64, usize)>, seq: u64, index: usize) -> bool {
    match current {
        None => true,
        Some((best_seq, best_index)) => (seq, index) > (best_seq, best_index),
    }
}

fn remember_runtime(
    records: &[ExtensionRecord],
    infos: &[ExtensionInfo],
    network: bool,
    downloads: bool,
) {
    let mut best: Option<(u64, usize, String)> = None;
    for info in infos {
        if !info.enabled {
            continue;
        }
        let Some(page) = info.new_tab.as_deref() else {
            continue;
        };
        let index = records
            .iter()
            .position(|record| record.id == info.id)
            .unwrap_or(0);
        let seq = records
            .iter()
            .find(|record| record.id == info.id)
            .map(|record| record.enabled_seq)
            .unwrap_or(0);
        if !newer_override(
            best.as_ref().map(|(seq, index, _)| (*seq, *index)),
            seq,
            index,
        ) {
            continue;
        }
        if let Ok(url) = extension_url(&info.id, page) {
            best = Some((seq, index, url.to_string()));
        }
    }
    let mut runtime = RUNTIME.lock().unwrap_or_else(|poison| poison.into_inner());
    runtime.new_tab = best.map(|(_, _, url)| url);
    runtime.network = network;
    runtime.downloads = downloads;
}

pub fn options_url(app: &AppHandle, id: &str) -> Result<Url, String> {
    let record = records(app)
        .into_iter()
        .find(|record| record.id == id)
        .ok_or("That extension has no options page.")?;
    let manifest = Manifest::read(&paths::extensions_dir(app).join(record.folder));
    let page = manifest
        .options_page
        .ok_or("That extension has no options page.")?;
    extension_url(id, &page)
}

fn unique_folder(root: &Path, name: &str) -> PathBuf {
    let base: String = files::sanitize_file_name(name)
        .chars()
        .map(|character| {
            if character.is_whitespace() {
                '-'
            } else {
                character
            }
        })
        .collect();
    let mut candidate = root.join(&base);
    let mut index = 2;
    while candidate.exists() {
        candidate = root.join(format!("{base}-{index}"));
        index += 1;
    }
    candidate
}

fn find_manifest_root(path: &Path) -> Option<PathBuf> {
    if path.join("manifest.json").is_file() {
        return Some(path.to_path_buf());
    }
    let children: Vec<PathBuf> = fs::read_dir(path)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|child| child.is_dir())
        .collect();
    match children.as_slice() {
        [only] if only.join("manifest.json").is_file() => Some(only.clone()),
        _ => None,
    }
}

fn store_busy() -> std::sync::MutexGuard<'static, ()> {
    STORE_BUSY
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

fn kept_available(folder: &Path, available: &str) -> String {
    let version = Manifest::read(folder).version.unwrap_or_default();
    if extension_store::version_is_newer(available, &version) {
        available.to_string()
    } else {
        String::new()
    }
}

struct StoreWatch {
    id: String,
    version: String,
    store: extension_store::Store,
}

fn store_watches(app: &AppHandle) -> Vec<StoreWatch> {
    let root = paths::extensions_dir(app);
    records(app)
        .into_iter()
        .filter_map(|record| {
            if record.id.is_empty() {
                return None;
            }
            let manifest = Manifest::read(&root.join(&record.folder));
            Some(StoreWatch {
                id: record.id,
                version: manifest.version.unwrap_or_default(),
                store: manifest.store?,
            })
        })
        .collect()
}

fn note_available_versions(app: &AppHandle, noted: &[(String, String)]) -> Result<(), String> {
    if noted.is_empty() {
        return Ok(());
    }
    let root = paths::extensions_dir(app);
    let mut next = records(app);
    let mut changed = false;
    for record in &mut next {
        let Some((_, available)) = noted.iter().find(|(id, _)| id == &record.id) else {
            continue;
        };
        let current = Manifest::read(&root.join(&record.folder))
            .version
            .unwrap_or_default();
        let value = if extension_store::version_is_newer(available, &current) {
            available.clone()
        } else {
            String::new()
        };
        if record.available_version != value {
            record.available_version = value;
            changed = true;
        }
    }
    if changed {
        save_records(app, |saved| *saved = next)?;
        emit(app);
    }
    Ok(())
}

struct PendingUpdate {
    id: String,
    name: String,
    version: String,
}

fn announce_updates(app: &AppHandle) {
    let root = paths::extensions_dir(app);
    let mut pending = Vec::new();
    for record in records(app) {
        if record.available_version.is_empty() {
            continue;
        }
        let manifest = Manifest::read(&root.join(&record.folder));
        let current = manifest.version.clone().unwrap_or_default();
        if !extension_store::version_is_newer(&record.available_version, &current) {
            continue;
        }
        let name = manifest
            .name
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| record.id.clone());
        pending.push(PendingUpdate {
            id: record.id,
            name,
            version: record.available_version,
        });
    }
    if pending.is_empty() {
        return;
    }
    let mut parts: Vec<String> = pending
        .iter()
        .map(|item| format!("{}@{}", item.id, item.version))
        .collect();
    parts.sort();
    if !extension_store::claim_announcement(&parts.join("\n")) {
        return;
    }
    let (message, action, label, extension_id) = if pending.len() == 1 {
        let item = &pending[0];
        (
            format!("{} {} is available", item.name, item.version),
            "extension-update",
            "Update",
            item.id.clone(),
        )
    } else {
        (
            format!("{} extension updates are available", pending.len()),
            "extension-updates",
            "View",
            String::new(),
        )
    };
    let params = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("id", "0")
        .append_pair("message", &message)
        .append_pair("undo", "0")
        .append_pair("action", action)
        .append_pair("actionLabel", label)
        .append_pair("extensionId", &extension_id)
        .finish();
    let window = tabs::focused_window(app);
    if let Err(error) = tabs::show_toast(app, &window, &params) {
        eprintln!("extension update toast: {error}");
    }
}

fn prepare_extension_dir(folder: &Path, public_key: Option<&[u8]>) -> Result<(), String> {
    let metadata = folder.join("_metadata");
    if metadata.exists() {
        fs::remove_dir_all(&metadata).map_err(|error| error.to_string())?;
    }
    let Some(public_key) = public_key else {
        return Ok(());
    };
    let path = folder.join("manifest.json");
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("Could not read the manifest: {error}"))?;
    let mut json: Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|error| format!("Could not read the manifest: {error}"))?;
    json["key"] = Value::String(extension_store::encode_extension_key(public_key));
    let bytes = serde_json::to_vec_pretty(&json).map_err(|error| error.to_string())?;
    fs::write(&path, bytes).map_err(|error| format!("Could not write the manifest: {error}"))
}

struct Unpacked {
    root: PathBuf,
    manifest: PathBuf,
    cleanup: bool,
}

impl Drop for Unpacked {
    fn drop(&mut self) {
        if self.cleanup {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

impl Unpacked {
    fn into_dir(mut self, dest: &Path) -> Result<(), String> {
        self.cleanup = false;
        match fs::rename(&self.manifest, dest) {
            Ok(()) => {
                if self.manifest != self.root {
                    let _ = fs::remove_dir_all(&self.root);
                }
                Ok(())
            }
            Err(error) => {
                let _ = fs::remove_dir_all(&self.root);
                Err(format!("Could not unpack the extension: {error}"))
            }
        }
    }
}

fn unpack_zip(bytes: &[u8]) -> Result<Unpacked, String> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "ce-extension-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    if let Err(error) = fs::create_dir_all(&root) {
        return Err(error.to_string());
    }
    if let Err(error) = extract_zip_bytes(bytes, &root) {
        let _ = fs::remove_dir_all(&root);
        return Err(error);
    }
    let Some(manifest) = find_manifest_root(&root) else {
        let _ = fs::remove_dir_all(&root);
        return Err("That extension does not contain a manifest.json.".into());
    };
    Ok(Unpacked {
        root,
        manifest,
        cleanup: true,
    })
}

fn swap_extension_dir(dest: &Path, unpacked: Unpacked) -> Result<PathBuf, String> {
    let name = dest
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "extension".to_string());
    let backup = dest.with_file_name(format!("{name}.previous"));
    if backup.exists() {
        fs::remove_dir_all(&backup)
            .map_err(|error| format!("Could not replace the extension: {error}"))?;
    }
    fs::rename(dest, &backup)
        .map_err(|error| format!("Could not replace the extension: {error}"))?;
    if let Err(error) = unpacked.into_dir(dest) {
        let _ = fs::rename(&backup, dest);
        return Err(error);
    }
    Ok(backup)
}

fn restore_tree(dest: &Path, backup: &Path) {
    if dest.exists() {
        let _ = fs::remove_dir_all(dest);
    }
    let _ = fs::rename(backup, dest);
}

fn restore_installed(app: &AppHandle, folder: &Path, enabled: bool) {
    if let Ok((id, _)) = add_to_profile(app, folder)
        && !enabled
    {
        let _ = act_on(app, &id, Some(false));
    }
}

fn extract_zip_bytes(bytes: &[u8], target: &Path) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(io::Cursor::new(bytes))
        .map_err(|error| format!("That is not a zip file: {error}"))?;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(|error| error.to_string())?;
        let Some(relative) = entry.enclosed_name() else {
            continue;
        };
        let out = target.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&out).map_err(|error| error.to_string())?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let mut writer = fs::File::create(&out).map_err(|error| error.to_string())?;
        io::copy(&mut entry, &mut writer).map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(crate) const MAX_EXTENSION_ZIP: usize = 200 * 1024 * 1024;

#[derive(Clone)]
pub struct ExtensionState {
    pub folder: String,
    pub enabled: bool,
    pub hash: String,
}

#[derive(Clone)]
pub struct IncomingExtension {
    pub folder: String,
    pub enabled: bool,
    pub hash: String,
    pub zip: Vec<u8>,
}

/// Writes unpacked extension folders and `extensions.json` before WebView2 is running.
pub fn stage_incoming(profile_dir: &Path, incoming: &[IncomingExtension]) -> Result<(), String> {
    let root = profile_dir.join("extensions");
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let mut records = Vec::new();
    for extension in incoming {
        if !safe_folder_name(&extension.folder) {
            return Err("The sync folder has an invalid extension folder name.".into());
        }
        let dest = root.join(&extension.folder);
        if !dest.join("manifest.json").is_file() {
            extract_zip_bytes(&extension.zip, &dest)?;
        }
        records.push(ExtensionRecord {
            id: String::new(),
            folder: extension.folder.clone(),
            available_version: String::new(),
            enabled_seq: 0,
        });
    }
    files::write_json(
        &profile_dir.join("extensions.json"),
        &ExtensionsFile {
            extensions: records,
        },
    )
}

pub fn extension_states(app: &AppHandle) -> Result<Vec<ExtensionState>, String> {
    let installed = installed(app)?;
    let root = paths::extensions_dir(app);
    let mut states = Vec::new();
    for record in records(app) {
        let folder = root.join(&record.folder);
        if !folder.join("manifest.json").is_file() || !safe_folder_name(&record.folder) {
            continue;
        }
        let enabled = installed
            .iter()
            .find(|item| item.id == record.id)
            .map(|item| item.enabled)
            .unwrap_or(true);
        states.push(ExtensionState {
            folder: record.folder,
            enabled,
            hash: hash_folder(&folder)?,
        });
    }
    states.sort_by(|left, right| left.folder.cmp(&right.folder));
    Ok(states)
}

pub fn zip_extension(app: &AppHandle, folder: &str) -> Result<Vec<u8>, String> {
    if !safe_folder_name(folder) {
        return Err("That extension folder name is not valid.".into());
    }
    let path = paths::extensions_dir(app).join(folder);
    if !path.join("manifest.json").is_file() {
        return Err(format!("The {folder} extension is missing its files."));
    }
    zip_folder(&path)
}

/// Makes the installed extensions match `incoming`. Folders are the identity, because an unpacked
/// extension's id changes when its path changes.
pub fn apply_extensions(app: &AppHandle, incoming: &[IncomingExtension]) -> Result<(), String> {
    for extension in incoming {
        if !safe_folder_name(&extension.folder) {
            return Err("The sync folder has an invalid extension folder name.".into());
        }
        if extension.zip.len() > MAX_EXTENSION_ZIP {
            return Err(format!(
                "The {} extension is too large to load.",
                extension.folder
            ));
        }
    }
    let root = paths::extensions_dir(app);
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let mut records = records(app);
    for extension in incoming {
        let dest = root.join(&extension.folder);
        let current_hash = if dest.join("manifest.json").is_file() {
            Some(hash_folder(&dest)?)
        } else {
            None
        };
        let known = records
            .iter()
            .find(|record| record.folder == extension.folder)
            .cloned();
        let installed_now = installed(app)?;
        let present = known
            .as_ref()
            .is_some_and(|record| installed_now.iter().any(|item| item.id == record.id));
        let previous = known
            .as_ref()
            .map(|record| record.available_version.clone())
            .unwrap_or_default();
        let same = current_hash.as_deref() == Some(extension.hash.as_str()) && present;
        let id = if same {
            known.as_ref().expect("present record").id.clone()
        } else {
            if let Some(record) = &known {
                if installed_now.iter().any(|item| item.id == record.id) {
                    act_on(app, &record.id, None)?;
                }
            }
            if dest.exists() {
                fs::remove_dir_all(&dest)
                    .map_err(|error| format!("Could not replace {}: {error}", dest.display()))?;
            }
            extract_zip_bytes(&extension.zip, &dest)?;
            let extracted = hash_folder(&dest)?;
            if extracted != extension.hash {
                let _ = fs::remove_dir_all(&dest);
                return Err(format!(
                    "The {} extension in the sync folder is damaged.",
                    extension.folder
                ));
            }
            let (id, _) = add_to_profile(app, &dest)?;
            id
        };
        let available_version = if same {
            kept_available(&dest, &previous)
        } else {
            String::new()
        };
        if installed(app)?
            .iter()
            .find(|item| item.id == id)
            .is_some_and(|item| item.enabled != extension.enabled)
        {
            act_on(app, &id, Some(extension.enabled))?;
        }
        let enabled_seq = if same {
            known.as_ref().map(|record| record.enabled_seq).unwrap_or(0)
        } else {
            next_seq(&records)
        };
        records.retain(|record| record.folder != extension.folder && record.id != id);
        records.push(ExtensionRecord {
            id,
            folder: extension.folder.clone(),
            available_version,
            enabled_seq,
        });
        save_records(app, |saved| *saved = records.clone())?;
    }

    let keep: HashSet<&str> = incoming
        .iter()
        .map(|extension| extension.folder.as_str())
        .collect();
    let installed_now = installed(app)?;
    for record in records.clone() {
        if keep.contains(record.folder.as_str()) {
            continue;
        }
        if installed_now.iter().any(|item| item.id == record.id) {
            act_on(app, &record.id, None)?;
        }
        let folder = root.join(&record.folder);
        if safe_folder_name(&record.folder) && folder.join("manifest.json").is_file() {
            let _ = fs::remove_dir_all(&folder);
        }
        records.retain(|kept| kept.folder != record.folder);
    }
    let keep_ids: HashSet<String> = records.iter().map(|record| record.id.clone()).collect();
    for item in installed(app)? {
        if !keep_ids.contains(&item.id) {
            act_on(app, &item.id, None)?;
        }
    }
    save_records(app, |saved| *saved = records)?;
    emit(app);
    Ok(())
}

fn safe_folder_name(name: &str) -> bool {
    let mut components = Path::new(name).components();
    match components.next() {
        Some(Component::Normal(text)) if components.next().is_none() => {
            let text = text.to_string_lossy();
            !text.starts_with('.') && !text.contains([':', '*', '?', '"', '<', '>', '|'])
        }
        _ => false,
    }
}

fn extension_files(folder: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let mut files = Vec::new();
    collect_extension_files(folder, folder, &mut files)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(files)
}

fn collect_extension_files(
    root: &Path,
    dir: &Path,
    files: &mut Vec<(String, PathBuf)>,
) -> Result<(), String> {
    let entries =
        fs::read_dir(dir).map_err(|error| format!("Could not read {}: {error}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.eq_ignore_ascii_case("Thumbs.db") || name.eq_ignore_ascii_case("desktop.ini") {
            continue;
        }
        if path.is_dir() {
            collect_extension_files(root, &path, files)?;
            continue;
        }
        if !path.is_file() {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|error| error.to_string())?
            .components()
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        files.push((relative, path));
    }
    Ok(())
}

fn hash_folder(folder: &Path) -> Result<String, String> {
    let mut hasher = Sha256::new();
    for (name, path) in extension_files(folder)? {
        let bytes = fs::read(&path)
            .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
        hasher.update(name.as_bytes());
        hasher.update([0]);
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
    Ok(hex_bytes(&hasher.finalize()))
}

fn zip_folder(folder: &Path) -> Result<Vec<u8>, String> {
    let files = extension_files(folder)?;
    let mut cursor = io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut cursor);
        for (name, path) in files {
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            zip.start_file(name, options)
                .map_err(|error| error.to_string())?;
            let bytes = fs::read(&path)
                .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
            zip.write_all(&bytes).map_err(|error| error.to_string())?;
        }
        zip.finish().map_err(|error| error.to_string())?;
    }
    let bytes = cursor.into_inner();
    if bytes.len() > MAX_EXTENSION_ZIP {
        return Err(format!(
            "The {} extension is too large to save.",
            folder.display()
        ));
    }
    Ok(bytes)
}

fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

#[derive(Debug, Clone, Default)]
struct Manifest {
    name: Option<String>,
    version: Option<String>,
    description: Option<String>,
    icon: Option<String>,
    popup: Option<String>,
    options_page: Option<String>,
    default_title: Option<String>,
    side_panel: Option<String>,
    new_tab: Option<String>,
    commands: Vec<ExtensionCommand>,
    /// `declarativeNetRequest` or `webRequest` is in `permissions`.
    network: bool,
    /// `downloads` is in `permissions`.
    downloads: bool,
    /// Set when the manifest has a publisher key and a Chrome or Edge update URL.
    store: Option<extension_store::Store>,
    key: Option<Vec<u8>>,
}

impl Manifest {
    fn read(folder: &Path) -> Self {
        let Some(json) = fs::read_to_string(folder.join("manifest.json"))
            .ok()
            .and_then(|text| {
                serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')).ok()
            })
        else {
            return Self::default();
        };
        let messages = locale_messages(folder, json.get("default_locale").and_then(Value::as_str));
        let text = |key: &str| {
            json.get(key)
                .and_then(Value::as_str)
                .map(|value| localize(value, &messages))
        };
        let action = json.get("action").or_else(|| json.get("browser_action"));
        let popup = action
            .and_then(|action| action.get("default_popup"))
            .and_then(Value::as_str)
            .filter(|popup| !popup.is_empty())
            .map(str::to_string);
        let default_title = action
            .and_then(|action| action.get("default_title"))
            .and_then(Value::as_str)
            .map(|value| localize(value, &messages))
            .filter(|title| !title.is_empty());
        let side_panel = page_field(json.get("side_panel"), "default_path")
            .or_else(|| page_field(json.get("sidebar_action"), "default_panel"));
        let new_tab = json
            .get("chrome_url_overrides")
            .and_then(|overrides| overrides.get("newtab"))
            .and_then(Value::as_str)
            .filter(|page| !page.is_empty())
            .map(str::to_string);
        let icon = action
            .and_then(|action| action.get("default_icon"))
            .and_then(|icon| pick_icon(icon))
            .or_else(|| json.get("icons").and_then(pick_icon))
            .and_then(|relative| icon_data_url(&folder.join(relative.trim_start_matches('/'))));
        let options_page = json
            .get("options_ui")
            .and_then(|options| options.get("page"))
            .or_else(|| json.get("options_page"))
            .and_then(Value::as_str)
            .map(str::to_string);
        let key = json
            .get("key")
            .and_then(Value::as_str)
            .and_then(extension_store::decode_extension_key);
        let store = key.as_ref().and_then(|_| {
            json.get("update_url")
                .and_then(Value::as_str)
                .and_then(extension_store::store_of_update_url)
        });
        Self {
            name: text("short_name")
                .filter(|name| !name.is_empty())
                .or_else(|| text("name")),
            version: text("version"),
            description: text("description"),
            icon,
            popup,
            options_page,
            default_title,
            side_panel,
            new_tab,
            commands: command_shortcuts(json.get("commands"), &messages),
            network: permission_list(
                &json,
                "permissions",
                &[
                    "declarativeNetRequest",
                    "declarativeNetRequestWithHostAccess",
                    "webRequest",
                    "webRequestBlocking",
                ],
            ),
            downloads: permission_list(&json, "permissions", &["downloads"]),
            store,
            key,
        }
    }
}

fn page_field(value: Option<&Value>, key: &str) -> Option<String> {
    value
        .and_then(|value| value.get(key))
        .and_then(Value::as_str)
        .filter(|page| !page.is_empty())
        .map(str::to_string)
}

fn permission_list(json: &Value, key: &str, names: &[&str]) -> bool {
    json.get(key)
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                item.as_str()
                    .is_some_and(|name| names.iter().any(|wanted| *wanted == name))
            })
        })
}

fn command_shortcuts(value: Option<&Value>, messages: &Value) -> Vec<ExtensionCommand> {
    let Some(map) = value.and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut commands = Vec::new();
    for (name, spec) in map {
        let Some(shortcut) = spec.get("suggested_key").and_then(|suggested| {
            suggested
                .get("windows")
                .and_then(Value::as_str)
                .filter(|shortcut| !shortcut.is_empty())
                .or_else(|| suggested.get("default").and_then(Value::as_str))
        }) else {
            continue;
        };
        if shortcut.is_empty() {
            continue;
        }
        let description = spec
            .get("description")
            .and_then(Value::as_str)
            .map(|value| localize(value, messages))
            .unwrap_or_default();
        commands.push(ExtensionCommand {
            name: name.clone(),
            description,
            shortcut: shortcut.to_string(),
        });
    }
    commands
}

/// `Ctrl+Shift+Y` and the other Chrome `suggested_key` forms. A Mac-only modifier does not match.
#[cfg(test)]
fn parse_shortcut(raw: &str) -> Option<Chord> {
    let parts: Vec<&str> = raw
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    let key_name = parts.last()?;
    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;
    for modifier in &parts[..parts.len() - 1] {
        match modifier.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => ctrl = true,
            "shift" => shift = true,
            "alt" => alt = true,
            "command" | "cmd" | "macctrl" => return None,
            _ => return None,
        }
    }
    Some(Chord {
        key: virtual_key(key_name)?,
        ctrl,
        shift,
        alt,
    })
}

#[cfg(test)]
fn virtual_key(name: &str) -> Option<u32> {
    let name = name.trim();
    let chars: Vec<char> = name.chars().collect();
    if chars.len() == 1 {
        let character = chars[0];
        if character.is_ascii_alphabetic() {
            return Some(u32::from(character.to_ascii_uppercase()));
        }
        if character.is_ascii_digit() {
            return Some(u32::from(character));
        }
    }
    if let Some(rest) = name.strip_prefix('F').or_else(|| name.strip_prefix('f'))
        && !rest.is_empty()
        && rest.chars().all(|character| character.is_ascii_digit())
        && let Ok(number) = rest.parse::<u32>()
        && (1..=24).contains(&number)
    {
        return Some(0x70 + number - 1);
    }
    Some(match name.to_ascii_lowercase().as_str() {
        "comma" => 0xBC,
        "period" => 0xBE,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" => 0x21,
        "pagedown" => 0x22,
        "space" => 0x20,
        "insert" => 0x2D,
        "delete" => 0x2E,
        "up" => 0x26,
        "down" => 0x28,
        "left" => 0x25,
        "right" => 0x27,
        "tab" => 0x09,
        "medianexttrack" => 0xB0,
        "mediaprevtrack" => 0xB1,
        "mediastop" => 0xB2,
        "mediaplaypause" => 0xB3,
        _ => return None,
    })
}

fn locale_messages(folder: &Path, default_locale: Option<&str>) -> Value {
    let locale = default_locale.unwrap_or("en");
    for candidate in [
        locale.to_string(),
        locale.replace('-', "_"),
        "en".to_string(),
    ] {
        let path = folder
            .join("_locales")
            .join(&candidate)
            .join("messages.json");
        if let Some(value) = fs::read_to_string(path).ok().and_then(|text| {
            serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')).ok()
        }) {
            return value;
        }
    }
    Value::Null
}

/// `__MSG_extName__` -> the `extName` message from the default locale.
fn localize(value: &str, messages: &Value) -> String {
    let Some(key) = value
        .strip_prefix("__MSG_")
        .and_then(|rest| rest.strip_suffix("__"))
    else {
        return value.to_string();
    };
    messages
        .as_object()
        .and_then(|map| map.iter().find(|(name, _)| name.eq_ignore_ascii_case(key)))
        .and_then(|(_, entry)| entry.get("message"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| key.to_string())
}

/// Picks the icon closest to 32px from either a path string or a `{ "16": "...", ... }` map.
fn pick_icon(value: &Value) -> Option<String> {
    if let Some(path) = value.as_str() {
        return Some(path.to_string());
    }
    let map = value.as_object()?;
    let mut sizes: Vec<(u32, &str)> = map
        .iter()
        .filter_map(|(size, path)| Some((size.parse().ok()?, path.as_str()?)))
        .collect();
    sizes.sort_by_key(|(size, _)| {
        if *size >= 32 {
            *size - 32
        } else {
            1000 + 32 - *size
        }
    });
    sizes.first().map(|(_, path)| path.to_string())
}

fn icon_data_url(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let mime = match path
        .extension()?
        .to_string_lossy()
        .to_ascii_lowercase()
        .as_str()
    {
        "svg" => "image/svg+xml",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "image/png",
    };
    Some(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_chords_follow_the_windows_key() {
        let chord = parse_shortcut("Ctrl+Shift+9").expect("digit chord");
        assert!(chord.ctrl && chord.shift && !chord.alt);
        assert_eq!(chord.key, u32::from(b'9'));
        let letter = parse_shortcut("Alt+K").expect("letter chord");
        assert!(letter.alt && !letter.ctrl && !letter.shift);
        assert_eq!(letter.key, u32::from(b'K'));
        assert!(parse_shortcut("Command+Shift+Y").is_none());
        assert_eq!(parse_shortcut("F5").expect("function key").key, 0x74);
    }

    #[test]
    fn the_newer_override_wins() {
        assert!(newer_override(None, 0, 0));
        assert!(!newer_override(Some((2, 0)), 1, 5));
        assert!(newer_override(Some((2, 0)), 2, 1));
        assert!(newer_override(Some((2, 1)), 3, 0));
        assert!(!newer_override(Some((2, 1)), 2, 0));
    }

    #[test]
    fn manifest_reads_the_surfaces_the_browser_hosts() {
        let dir = std::env::temp_dir().join(format!("ce-ext-manifest-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("manifest.json"),
            r#"{
              "manifest_version": 3,
              "name": "Sample",
              "version": "1.0.0",
              "action": { "default_title": "Open sample", "default_popup": "popup.html" },
              "side_panel": { "default_path": "side.html" },
              "sidebar_action": { "default_panel": "edge.html" },
              "chrome_url_overrides": { "newtab": "newtab.html" },
              "permissions": ["declarativeNetRequest", "downloads"],
              "commands": {
                "go": {
                  "suggested_key": { "default": "Ctrl+Shift+Y", "windows": "Ctrl+Shift+U" },
                  "description": "Go"
                }
              }
            }"#,
        )
        .unwrap();
        let manifest = Manifest::read(&dir);
        assert_eq!(manifest.default_title.as_deref(), Some("Open sample"));
        assert_eq!(manifest.popup.as_deref(), Some("popup.html"));
        assert_eq!(manifest.side_panel.as_deref(), Some("side.html"));
        assert_eq!(manifest.new_tab.as_deref(), Some("newtab.html"));
        assert_eq!(manifest.commands.len(), 1);
        assert_eq!(manifest.commands[0].shortcut, "Ctrl+Shift+U");
        assert!(manifest.network);
        assert!(manifest.downloads);
        fs::write(
            dir.join("manifest.json"),
            r#"{"manifest_version":3,"name":"Edge","version":"1","sidebar_action":{"default_panel":"edge.html"}}"#,
        )
        .unwrap();
        let edge = Manifest::read(&dir);
        assert_eq!(edge.side_panel.as_deref(), Some("edge.html"));
        assert!(!edge.network);
        let _ = fs::remove_dir_all(&dir);
    }
}
