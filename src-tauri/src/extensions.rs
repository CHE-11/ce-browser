use std::collections::HashSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
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
    pub folder: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct ExtensionRecord {
    id: String,
    /// Folder name inside `<data>/extensions`.
    folder: String,
}

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
    let file: ExtensionsFile = files::read_json(&paths::data_file(app, FILE));
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
    files::write_json(
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
    let webview = app
        .get_webview(tabs::UI)
        .ok_or("The browser UI is not ready yet.")?;
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
    Ok(installed
        .into_iter()
        .map(|extension| {
            let folder = records
                .iter()
                .find(|record| record.id == extension.id)
                .map(|record| record.folder.clone());
            let manifest = folder
                .as_ref()
                .map(|folder| Manifest::read(&root.join(folder)));
            let manifest = manifest.unwrap_or_default();
            ExtensionInfo {
                name: manifest
                    .name
                    .clone()
                    .filter(|name| !name.is_empty())
                    .unwrap_or(extension.name),
                id: extension.id,
                version: manifest.version.clone().unwrap_or_default(),
                description: manifest.description.clone().unwrap_or_default(),
                enabled: extension.enabled,
                icon: manifest.icon.clone(),
                popup: manifest.popup.clone(),
                options_page: manifest.options_page.clone(),
                folder,
            }
        })
        .collect())
}

fn emit(app: &AppHandle) {
    if let Ok(list) = list(app) {
        let _ = app.emit("extensions-changed", &list);
    }
}

/// Extensions persist in the WebView2 profile. Folders dropped into `<data>/extensions` by hand,
/// or a profile that was reset, get (re)added here.
pub fn reload(app: &AppHandle) -> Result<(), String> {
    let file: ExtensionsFile = files::read_json(&paths::data_file(app, FILE));
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
                records.retain(|record| record.folder != name && record.id != id);
                records.push(ExtensionRecord { id, folder: name });
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
    install_from(app, target)
}

pub fn install_zip(app: &AppHandle, archive: &Path) -> Result<ExtensionInfo, String> {
    let root = paths::extensions_dir(app);
    let staging = unique_folder(&root, ".staging");
    extract_zip(archive, &staging).inspect_err(|_| {
        let _ = fs::remove_dir_all(&staging);
    })?;
    let Some(source) = find_manifest_root(&staging) else {
        let _ = fs::remove_dir_all(&staging);
        return Err("That zip does not contain a manifest.json.".into());
    };
    let manifest = Manifest::read(&source);
    let target = unique_folder(&root, manifest.name.as_deref().unwrap_or("extension"));
    let moved = fs::rename(&source, &target);
    let _ = fs::remove_dir_all(&staging);
    moved.map_err(|error| format!("Could not unpack the extension: {error}"))?;
    install_from(app, target)
}

fn install_from(app: &AppHandle, target: PathBuf) -> Result<ExtensionInfo, String> {
    let folder = target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (id, _) = add_to_profile(app, &target).inspect_err(|_| {
        let _ = fs::remove_dir_all(&target);
    })?;
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
        records.push(ExtensionRecord {
            id: id.clone(),
            folder,
        });
    })?;
    emit(app);
    list(app)?
        .into_iter()
        .find(|extension| extension.id == id)
        .ok_or_else(|| "The extension was added but WebView2 does not list it.".to_string())
}

pub fn set_enabled(app: &AppHandle, id: &str, enabled: bool) -> Result<(), String> {
    act_on(app, id, Some(enabled))?;
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
    let record = records(app)
        .into_iter()
        .find(|record| record.id == id)
        .ok_or("That extension has no toolbar popup.")?;
    let manifest = Manifest::read(&paths::extensions_dir(app).join(record.folder));
    let popup = manifest
        .popup
        .ok_or("That extension has no toolbar popup.")?;
    extension_url(id, &popup)
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

/// A `.crx` is a zip behind a `Cr24` header. Version 2 stores key and signature lengths, version 3 a header length.
fn zip_offset(bytes: &[u8]) -> Result<usize, String> {
    if bytes.len() < 16 || &bytes[..4] != b"Cr24" {
        return Ok(0);
    }
    let read_u32 = |at: usize| {
        u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as usize
    };
    let offset = match read_u32(4) {
        2 => 16 + read_u32(8) + read_u32(12),
        3 => 12 + read_u32(8),
        version => return Err(format!("Unsupported .crx version {version}.")),
    };
    if offset >= bytes.len() {
        return Err("That .crx file is truncated.".into());
    }
    Ok(offset)
}

fn extract_zip(archive: &Path, target: &Path) -> Result<(), String> {
    let bytes = fs::read(archive)
        .map_err(|error| format!("Could not open {}: {error}", archive.display()))?;
    let offset = zip_offset(&bytes)?;
    extract_zip_bytes(&bytes[offset..], target)
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

const MAX_EXTENSION_ZIP: usize = 200 * 1024 * 1024;

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
        });
    }
    files::write_json(
        &profile_dir.join("extensions.json"),
        &ExtensionsFile { extensions: records },
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
        let id = if current_hash.as_deref() == Some(extension.hash.as_str()) && present {
            known.expect("present record").id
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
        if installed(app)?
            .iter()
            .find(|item| item.id == id)
            .is_some_and(|item| item.enabled != extension.enabled)
        {
            act_on(app, &id, Some(extension.enabled))?;
        }
        records.retain(|record| record.folder != extension.folder && record.id != id);
        records.push(ExtensionRecord {
            id,
            folder: extension.folder.clone(),
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
        Self {
            name: text("short_name")
                .filter(|name| !name.is_empty())
                .or_else(|| text("name")),
            version: text("version"),
            description: text("description"),
            icon,
            popup,
            options_page,
        }
    }
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
