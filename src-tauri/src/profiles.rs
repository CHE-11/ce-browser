use std::ffi::OsString;
use std::fs;
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use windows::Win32::Foundation::{HWND, LPARAM, PROPERTYKEY};
use windows::core::BOOL;
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoTaskMemFree, IPersistFile, STGM_READ, STGM_READWRITE,
};
use windows::Win32::System::Variant::{VT_EMPTY, VT_LPWSTR};
use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, SHGetPropertyStoreForWindow};
use windows::Win32::UI::Shell::{
    FOLDERID_Programs, FOLDERID_UserPinned, IShellLinkW, KNOWN_FOLDER_FLAG, SHCNE_UPDATEITEM,
    SHCNF_PATHW, SHChangeNotify, SHGetKnownFolderPath, SetCurrentProcessExplicitAppUserModelID,
    ShellLink,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE,
};
use windows::core::{GUID, HSTRING, Interface, PWSTR};

use crate::{files, paths};

pub const DEFAULT_ID: &str = "default";
pub const DEFAULT_FOLDER: &str = "Default";
/// Name and sync folder of the default profile before it was called Default.
const LEGACY_DEFAULT_NAME: &str = "ce-browser";

const APP_MODEL_FMT: u128 = 0x9F4C2855_9F79_4B39_A8D0_E1D42DE1D5F3;

static REQUESTED: Mutex<String> = Mutex::new(String::new());
static ACTIVE: OnceLock<String> = OnceLock::new();

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateMutexW(attrs: *mut core::ffi::c_void, initial_owner: i32, name: *const u16) -> *mut core::ffi::c_void;
    fn OpenMutexW(access: u32, inherit: i32, name: *const u16) -> *mut core::ffi::c_void;
    fn CloseHandle(handle: *mut core::ffi::c_void) -> i32;
    fn GetLastError() -> u32;
    fn SetLastError(code: u32);
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct ProfileRecord {
    id: String,
    name: String,
    shortcut: String,
    /// `default`, a preset id, `custom`, or empty for the app icon.
    #[serde(default)]
    icon: String,
    /// File name of the generated `.ico` inside the profile folder.
    #[serde(default)]
    icon_file: String,
    /// Directory name in the sync folder. Empty until the profile name is used.
    #[serde(default)]
    folder: String,
}

impl Default for ProfileRecord {
    fn default() -> Self {
        Self {
            id: DEFAULT_ID.into(),
            name: DEFAULT_FOLDER.into(),
            shortcut: String::new(),
            icon: String::new(),
            icon_file: String::new(),
            folder: String::new(),
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct ProfilesFile {
    profiles: Vec<ProfileRecord>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileInfo {
    pub id: String,
    pub name: String,
    pub current: bool,
    pub start_menu: bool,
    pub taskbar: bool,
    /// `default`, a preset id, or `custom`.
    pub icon_id: String,
    /// PNG data URL of the icon used for this profile.
    pub icon: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileIconChoice {
    pub id: String,
    pub label: String,
    pub icon: String,
}

pub fn read_args() {
    let mut args = std::env::args().skip(1);
    let mut requested = String::new();
    while let Some(arg) = args.next() {
        if arg == "--profile" {
            if let Some(id) = args.next() {
                requested = id;
            }
        } else if let Some(id) = arg.strip_prefix("--profile=") {
            requested = id.to_string();
        }
    }
    *REQUESTED.lock().expect("profile args") = requested;
}

pub fn active_id() -> String {
    ACTIVE
        .get()
        .cloned()
        .unwrap_or_else(|| DEFAULT_ID.to_string())
}

pub fn init(app: &AppHandle) {
    let mut file = load(app);
    if ensure_default(&mut file) {
        let _ = save(app, &file);
    }
    let requested = REQUESTED.lock().expect("profile args").clone();
    let id = if !requested.is_empty() && file.profiles.iter().any(|profile| profile.id == requested)
    {
        requested
    } else {
        DEFAULT_ID.to_string()
    };
    let _ = ACTIVE.set(id.clone());
    if !hold_profile_mutex(&id) {
        focus_profile_window(&id);
        std::process::exit(0);
    }
}

pub fn window_title(app: &AppHandle) -> String {
    let id = active_id();
    if id == DEFAULT_ID {
        return "ce-browser".into();
    }
    load(app)
        .profiles
        .into_iter()
        .find(|profile| profile.id == id)
        .map(|profile| format!("{} — ce-browser", profile.name))
        .unwrap_or_else(|| "ce-browser".into())
}

pub fn set_process_identity() -> Result<(), String> {
    com_init();
    let model = HSTRING::from(app_model_id(&active_id()));
    unsafe { SetCurrentProcessExplicitAppUserModelID(&model) }.map_err(win_err)
}

pub fn mark_window(window: &tauri::Window) -> Result<(), String> {
    let id = active_id();
    let hwnd = window.hwnd().map_err(|error| error.to_string())?;
    com_init();
    unsafe {
        let store: IPropertyStore = SHGetPropertyStoreForWindow(hwnd).map_err(win_err)?;
        set_text(&store, app_model_key(5), &app_model_id(&id))?;
        set_text(&store, app_model_key(2), &relaunch_command(&id))?;
        let icon = relaunch_icon(window.app_handle(), &id);
        if !icon.is_empty() {
            set_text(&store, app_model_key(3), &icon)?;
        }
        store.Commit().map_err(win_err)?;
    }
    Ok(())
}

pub fn list(app: &AppHandle) -> Vec<ProfileInfo> {
    let current = active_id();
    let pinned = taskbar_ids();
    load(app)
        .profiles
        .into_iter()
        .map(|profile| {
            let (icon_id, icon) = profile_icon(app, &profile);
            ProfileInfo {
                taskbar: pinned.iter().any(|pinned_id| pinned_id == &profile.id),
                current: profile.id == current,
                start_menu: Path::new(&profile.shortcut).is_file(),
                icon_id,
                icon,
                id: profile.id,
                name: profile.name,
            }
        })
        .collect()
}

pub fn icon_choices() -> Vec<ProfileIconChoice> {
    crate::profile_icons::choices()
        .into_iter()
        .map(|choice| ProfileIconChoice {
            id: choice.id.to_string(),
            label: choice.label.to_string(),
            icon: choice.preview,
        })
        .collect()
}

pub fn set_icon(
    app: &AppHandle,
    id: &str,
    icon_id: &str,
    path: Option<&str>,
) -> Result<Vec<ProfileInfo>, String> {
    let mut file = load(app);
    let Some(index) = file.profiles.iter().position(|profile| profile.id == id) else {
        return Err("That profile no longer exists.".into());
    };
    let dir = paths::profile_data_dir(app, id);
    if icon_id == "default" {
        clear_icon_files(&dir, &file.profiles[index].icon_file);
        file.profiles[index].icon.clear();
        file.profiles[index].icon_file.clear();
    } else {
        let image = if icon_id == "custom" {
            let source = path
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or("Choose an image file.")?;
            crate::profile_icons::from_file(Path::new(source))?
        } else {
            crate::profile_icons::render(icon_id)?
        };
        fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        let png = crate::profile_icons::encode_png(&image)?;
        let ico = crate::profile_icons::encode_ico(&image)?;
        let ico_name = format!("window-icon-{}.ico", icon_stamp());
        let png_path = dir.join("window-icon.png");
        let ico_path = dir.join(&ico_name);
        fs::write(&ico_path, ico).map_err(|error| error.to_string())?;
        if let Err(error) = fs::write(&png_path, &png) {
            let _ = fs::remove_file(&ico_path);
            return Err(error.to_string());
        }
        let previous = file.profiles[index].icon_file.clone();
        if is_owned_icon_name(&previous) && previous != ico_name {
            let _ = fs::remove_file(dir.join(previous));
        }
        file.profiles[index].icon = icon_id.to_string();
        file.profiles[index].icon_file = ico_name;
    }
    save(app, &file)?;
    refresh_shell_icons(app, &file.profiles[index])?;
    Ok(list(app))
}

pub fn pin_taskbar(app: &AppHandle, id: &str) -> Result<Vec<ProfileInfo>, String> {
    let mut file = load(app);
    let Some(index) = file.profiles.iter().position(|profile| profile.id == id) else {
        return Err("That profile no longer exists.".into());
    };
    if !Path::new(&file.profiles[index].shortcut).is_file() {
        let icon = owned_icon(
            app,
            &file.profiles[index].id,
            &file.profiles[index].icon_file,
        );
        let path = install_shortcut(
            &file.profiles[index].id,
            &file.profiles[index].name,
            icon.as_deref(),
        )?;
        file.profiles[index].shortcut = path.to_string_lossy().into_owned();
        save(app, &file)?;
    } else {
        refresh_shell_icons(app, &file.profiles[index])?;
    }
    let shortcut =
        shortcut_path(&file.profiles[index]).ok_or("The Start menu item could not be created.")?;
    let dir = known_folder(&FOLDERID_UserPinned)
        .map(|path| path.join("TaskBar"))
        .map_err(|_| "The taskbar folder is unavailable.".to_string())?;
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    for path in matching_shortcuts(&dir, id) {
        let _ = fs::remove_file(path);
    }
    let name = shortcut
        .file_name()
        .ok_or("The Start menu item could not be created.")?;
    let dest = dir.join(name);
    fs::copy(&shortcut, &dest).map_err(|error| format!("Could not pin that profile ({error})."))?;
    notify_item(&dest);
    if taskbar_ids().iter().all(|pinned| pinned != id) {
        return Err("Windows did not add the taskbar button. Right-click this window's taskbar button and choose Pin to taskbar.".into());
    }
    Ok(list(app))
}

pub fn unpin_taskbar(app: &AppHandle, id: &str) -> Result<Vec<ProfileInfo>, String> {
    if !load(app).profiles.iter().any(|profile| profile.id == id) {
        return Err("That profile no longer exists.".into());
    }
    let Some(dir) = taskbar_dir() else {
        return Ok(list(app));
    };
    for path in matching_shortcuts(&dir, id) {
        fs::remove_file(&path)
            .map_err(|error| format!("The taskbar button is still pinned ({error})."))?;
        notify_item(&path);
    }
    Ok(list(app))
}

pub fn apply_window_icon(app: &AppHandle, window: &tauri::Window) {
    let bytes = window_icon_bytes(app);
    let icon = tauri::image::Image::from_bytes(&bytes)
        .or_else(|_| tauri::image::Image::from_bytes(crate::profile_icons::default_png()));
    match icon {
        Ok(icon) => {
            if let Err(error) = window.set_icon(icon) {
                eprintln!("window icon: {error}");
            }
        }
        Err(error) => eprintln!("window icon: {error}"),
    }
    if let Err(error) = mark_window(window) {
        eprintln!("profile identity: {error}");
    }
}

pub fn create(app: &AppHandle, name: &str) -> Result<Vec<ProfileInfo>, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("A profile needs a name.".into());
    }
    if name.chars().count() > 64 {
        return Err("That profile name is too long.".into());
    }
    let mut file = load(app);
    let id = unique_id(&file, name);
    let folder = unique_directory_name(name, &id, &taken_folders(&file));
    let data_dir = paths::data_root(app).join(&folder);
    fs::create_dir_all(&data_dir).map_err(|error| error.to_string())?;
    fs::create_dir_all(paths::profile_webview_dir(app, &id)).map_err(|error| error.to_string())?;
    let shortcut = match install_shortcut(&id, name, None) {
        Ok(path) => path,
        Err(error) => {
            let _ = fs::remove_dir_all(&data_dir);
            let _ = fs::remove_dir_all(paths::profile_webview_dir(app, &id));
            return Err(error);
        }
    };
    file.profiles.push(ProfileRecord {
        id: id.clone(),
        name: name.to_string(),
        shortcut: shortcut.to_string_lossy().into_owned(),
        folder,
        ..ProfileRecord::default()
    });
    save(app, &file)?;
    let _ = open(&id);
    Ok(list(app))
}

pub fn install_shortcut_for(app: &AppHandle, id: &str) -> Result<Vec<ProfileInfo>, String> {
    let mut file = load(app);
    let profile = file
        .profiles
        .iter_mut()
        .find(|profile| profile.id == id)
        .ok_or("That profile no longer exists.")?;
    if Path::new(&profile.shortcut).is_file() {
        return Ok(list(app));
    }
    let icon = owned_icon(app, &profile.id, &profile.icon_file);
    let path = install_shortcut(&profile.id, &profile.name, icon.as_deref())?;
    profile.shortcut = path.to_string_lossy().into_owned();
    save(app, &file)?;
    Ok(list(app))
}

pub fn open(id: &str) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    Command::new(exe)
        .arg("--profile")
        .arg(id)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("Could not open that profile: {error}"))?;
    Ok(())
}

pub fn delete(app: &AppHandle, id: &str) -> Result<Vec<ProfileInfo>, String> {
    if id == DEFAULT_ID {
        return Err("The first profile stays on this computer.".into());
    }
    if !safe_profile_id(id) {
        return Err("That profile no longer exists.".into());
    }
    if id == active_id() || profile_is_open(id) {
        return Err("Close that profile before removing it.".into());
    }
    let mut file = load(app);
    let Some(index) = file.profiles.iter().position(|profile| profile.id == id) else {
        return Err("That profile no longer exists.".into());
    };
    let profile = file.profiles[index].clone();
    // The profile stays in the list until its own files are gone, so a locked folder can be retried.
    purge_profile_storage(
        &paths::data_root(app),
        &paths::profile_data_dir(app, id),
        &paths::profile_webview_dir(app, id),
        id,
    )?;
    if let Some(path) = shortcut_path(&profile) {
        let _ = fs::remove_file(path);
    }
    if let Some(dir) = taskbar_dir() {
        for path in matching_shortcuts(&dir, id) {
            let _ = fs::remove_file(path);
        }
    }
    file.profiles.remove(index);
    save(app, &file)?;
    Ok(list(app))
}

/// Settings, history, bookmarks, extensions, downloads, the session, and styles live in `data_dir`.
/// Cookies and the installed extension state live in `webview_dir`.
fn purge_profile_storage(
    data_root: &Path,
    data_dir: &Path,
    webview_dir: &Path,
    id: &str,
) -> Result<(), String> {
    remove_profile_dir(data_dir, id, data_root)?;
    remove_profile_dir(webview_dir, id, data_root)?;
    Ok(())
}

fn remove_profile_dir(path: &Path, id: &str, data_root: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if !profile_child_dir(&canonical, id, data_root) || canonical.join("profiles.json").is_file() {
        return Err("Refusing to delete a folder outside that profile.".into());
    }
    let mut last = String::new();
    for attempt in 0..8 {
        if !canonical.exists() {
            return Ok(());
        }
        match fs::remove_dir_all(&canonical) {
            Ok(()) => return Ok(()),
            Err(error) => last = error.to_string(),
        }
        if attempt + 1 < 8 {
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
    }
    Err(format!(
        "Could not delete {} ({last}). Close that profile and try again.",
        canonical.display()
    ))
}

/// WebView2 keeps `profiles/<id>`. The data root keeps `<profile name>` directly inside it.
fn profile_child_dir(path: &Path, id: &str, data_root: &Path) -> bool {
    let Some(name) = path.file_name() else {
        return false;
    };
    let Some(parent) = path.parent() else {
        return false;
    };
    if parent.file_name().is_some_and(|parent_name| parent_name == "profiles") && name == id {
        return true;
    }
    let root = data_root.canonicalize().unwrap_or_else(|_| data_root.to_path_buf());
    parent == root
}

/// Folder of a profile inside the data root.
pub fn folder_name(app: &AppHandle, id: &str) -> String {
    folder_of(&load(app), id)
}

/// Folder of the default profile inside `root`.
pub fn default_dir(root: &Path) -> PathBuf {
    root.join(folder_of(&read_profiles_file(&root.join("profiles.json")), DEFAULT_ID))
}

pub fn sync_has_profile_settings(folder: &Path) -> bool {
    if folder.join("profiles").join(DEFAULT_ID).join("settings.json").is_file() {
        return true;
    }
    let Ok(entries) = fs::read_dir(folder) else {
        return false;
    };
    entries.flatten().any(|entry| {
        entry.file_type().is_ok_and(|kind| kind.is_dir()) && entry.path().join("settings.json").is_file()
    })
}

fn safe_profile_id(id: &str) -> bool {
    !id.is_empty()
        && id != DEFAULT_ID
        && id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
}

const ERROR_ALREADY_EXISTS: u32 = 183;

/// Returns false when this profile is already open. The caller should focus that window and exit
/// before WebView2 tries to open the same profile folder.
fn hold_profile_mutex(id: &str) -> bool {
    let name = wide(format!("Local\\ce-browser-profile-{id}"));
    unsafe { SetLastError(0) };
    let handle = unsafe { CreateMutexW(std::ptr::null_mut(), 0, name.as_ptr()) };
    let error = unsafe { GetLastError() };
    if handle.is_null() {
        eprintln!("profile lock: {}", std::io::Error::last_os_error());
        return true;
    }
    if error == ERROR_ALREADY_EXISTS {
        unsafe {
            let _ = CloseHandle(handle);
        }
        return false;
    }
    let _ = PROFILE_MUTEX.set(handle as usize);
    true
}

struct WindowSeek {
    model: String,
    found: HWND,
}

unsafe extern "system" fn seek_profile_window(hwnd: HWND, param: LPARAM) -> BOOL {
    let seek = unsafe { &mut *(param.0 as *mut WindowSeek) };
    if window_model_id(hwnd).as_deref() == Some(seek.model.as_str()) {
        seek.found = hwnd;
        return BOOL(0);
    }
    BOOL(1)
}

fn window_model_id(hwnd: HWND) -> Option<String> {
    com_init();
    unsafe {
        let store: IPropertyStore = SHGetPropertyStoreForWindow(hwnd).ok()?;
        let value = store.GetValue(&app_model_key(5)).ok()?;
        prop_text(&value)
    }
}

fn prop_text(value: &PROPVARIANT) -> Option<String> {
    unsafe {
        if (*value.Anonymous.Anonymous).vt != VT_LPWSTR {
            return None;
        }
        let raw = (*value.Anonymous.Anonymous).Anonymous.pwszVal;
        if raw.is_null() {
            return None;
        }
        let mut length = 0;
        while *raw.0.add(length) != 0 {
            length += 1;
        }
        let slice = std::slice::from_raw_parts(raw.0, length);
        Some(String::from_utf16_lossy(slice))
    }
}

fn profile_id_from_model(model: &str) -> Option<String> {
    if model == "com.ce.browser" {
        return Some(DEFAULT_ID.to_string());
    }
    model
        .strip_prefix("com.ce.browser.")
        .filter(|id| !id.is_empty())
        .map(str::to_string)
}

fn focus_profile_window(id: &str) {
    let mut seek = WindowSeek {
        model: app_model_id(id),
        found: HWND(std::ptr::null_mut()),
    };
    unsafe {
        let _ = EnumWindows(
            Some(seek_profile_window),
            LPARAM(&mut seek as *mut WindowSeek as isize),
        );
        if seek.found.0.is_null() {
            return;
        }
        if IsIconic(seek.found).as_bool() {
            let _ = ShowWindow(seek.found, SW_RESTORE);
        }
        let _ = SetForegroundWindow(seek.found);
    }
}

fn profile_is_open(id: &str) -> bool {
    let name = wide(format!("Local\\ce-browser-profile-{id}"));
    let handle = unsafe { OpenMutexW(0x0010_0000, 0, name.as_ptr()) };
    if handle.is_null() {
        return false;
    }
    unsafe {
        let _ = CloseHandle(handle);
    }
    true
}

static PROFILE_MUTEX: OnceLock<usize> = OnceLock::new();

fn load(app: &AppHandle) -> ProfilesFile {
    let mut file: ProfilesFile = files::read_json(&registry_path(app));
    ensure_default(&mut file);
    migrate_default(&mut file);
    assign_folders(&mut file);
    file
}

/// The default profile used to be named `ce-browser` and kept that folder in a sync folder.
fn migrate_default(file: &mut ProfilesFile) {
    let other_owns_default = file.profiles.iter().any(|profile| {
        profile.id != DEFAULT_ID && profile.folder.eq_ignore_ascii_case(DEFAULT_FOLDER)
    });
    let Some(record) = file.profiles.iter_mut().find(|profile| profile.id == DEFAULT_ID) else {
        return;
    };
    if record.name == LEGACY_DEFAULT_NAME || record.name.trim().is_empty() {
        record.name = DEFAULT_FOLDER.into();
    }
    if record.folder == LEGACY_DEFAULT_NAME && !other_owns_default {
        record.folder = DEFAULT_FOLDER.into();
    }
}

fn save(app: &AppHandle, file: &ProfilesFile) -> Result<(), String> {
    files::write_json(&registry_path(app), file)
}

fn registry_path(app: &AppHandle) -> PathBuf {
    paths::profiles_registry(app)
}

fn ensure_default(file: &mut ProfilesFile) -> bool {
    if file.profiles.iter().any(|profile| profile.id == DEFAULT_ID) {
        return false;
    }
    file.profiles.insert(0, ProfileRecord::default());
    true
}

fn unique_id(file: &ProfilesFile, name: &str) -> String {
    let mut base = String::new();
    for character in name.chars().flat_map(|character| character.to_lowercase()) {
        if character.is_ascii_alphanumeric() {
            base.push(character);
        } else if (character.is_whitespace() || character == '-')
            && !base.ends_with('-')
            && !base.is_empty()
        {
            base.push('-');
        }
        if base.len() >= 24 {
            break;
        }
    }
    let base = base.trim_matches('-').to_string();
    let base = if base.is_empty() || base == DEFAULT_ID {
        "profile".to_string()
    } else {
        base
    };
    let mut id = base.clone();
    let mut index = 2;
    while file.profiles.iter().any(|profile| profile.id == id) {
        id = format!("{base}-{index}");
        index += 1;
    }
    id
}

fn install_shortcut(id: &str, name: &str, icon: Option<&Path>) -> Result<PathBuf, String> {
    com_init();
    let programs = programs_dir()?;
    let label = if id == DEFAULT_ID {
        "ce-browser.lnk".to_string()
    } else {
        format!("ce-browser {name}.lnk")
    };
    let path = files::unique_path(&programs, &label);
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let exe_text = exe.display().to_string();
    unsafe {
        let link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(win_err)?;
        link.SetPath(&HSTRING::from(&exe_text)).map_err(win_err)?;
        link.SetArguments(&HSTRING::from(format!("--profile {id}")))
            .map_err(win_err)?;
        link.SetDescription(&HSTRING::from(name)).map_err(win_err)?;
        let icon_path = icon
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| exe_text.clone());
        link.SetIconLocation(&HSTRING::from(&icon_path), 0)
            .map_err(win_err)?;
        if let Some(directory) = exe.parent() {
            link.SetWorkingDirectory(&HSTRING::from(directory.as_os_str()))
                .map_err(win_err)?;
        }
        let store: IPropertyStore = link.cast().map_err(win_err)?;
        set_text(&store, app_model_key(5), &app_model_id(id))?;
        store.Commit().map_err(win_err)?;
        let file: IPersistFile = link.cast().map_err(win_err)?;
        file.Save(&HSTRING::from(path.as_os_str()), true)
            .map_err(win_err)?;
    }
    Ok(path)
}

fn programs_dir() -> Result<PathBuf, String> {
    let path = known_folder(&FOLDERID_Programs)?;
    if path.as_os_str().is_empty() {
        return Err("The Start menu folder is unavailable.".into());
    }
    Ok(path)
}

fn known_folder(id: &GUID) -> Result<PathBuf, String> {
    com_init();
    unsafe {
        let raw = SHGetKnownFolderPath(id, KNOWN_FOLDER_FLAG(0), None).map_err(win_err)?;
        let path = pwstr_path(raw);
        CoTaskMemFree(Some(raw.0 as *const core::ffi::c_void));
        Ok(path)
    }
}

fn window_icon_bytes(app: &AppHandle) -> Vec<u8> {
    let id = active_id();
    let custom = load(app)
        .profiles
        .into_iter()
        .find(|profile| profile.id == id)
        .is_some_and(|profile| {
            !profile.icon.is_empty()
                && profile.icon != "default"
                && paths::profile_data_dir(app, &id)
                    .join("window-icon.png")
                    .is_file()
        });
    if custom {
        if let Ok(bytes) = fs::read(paths::profile_data_dir(app, &id).join("window-icon.png")) {
            if !bytes.is_empty() {
                return bytes;
            }
        }
    }
    crate::profile_icons::default_png().to_vec()
}

fn relaunch_icon(app: &AppHandle, id: &str) -> String {
    let record = load(app)
        .profiles
        .into_iter()
        .find(|profile| profile.id == id);
    if let Some(path) = record
        .as_ref()
        .and_then(|profile| owned_icon(app, &profile.id, &profile.icon_file))
    {
        return format!("{},0", path.display());
    }
    std::env::current_exe()
        .map(|exe| format!("{},0", exe.display()))
        .unwrap_or_default()
}

fn profile_icon(app: &AppHandle, profile: &ProfileRecord) -> (String, String) {
    let fallback = crate::profile_icons::preview("default").unwrap_or_default();
    if profile.icon == "custom" {
        let png = paths::profile_data_dir(app, &profile.id).join("window-icon.png");
        if let Some(preview) = crate::profile_icons::preview_file(&png) {
            return ("custom".into(), preview);
        }
        return ("default".into(), fallback);
    }
    if profile.icon.is_empty() || profile.icon == "default" {
        return ("default".into(), fallback);
    }
    match crate::profile_icons::preview(&profile.icon) {
        Some(preview) => (profile.icon.clone(), preview),
        None => ("default".into(), fallback),
    }
}

fn refresh_shell_icons(app: &AppHandle, profile: &ProfileRecord) -> Result<(), String> {
    let icon = owned_icon(app, &profile.id, &profile.icon_file);
    if let Some(path) = shortcut_path(profile) {
        if path.is_file() {
            update_link_icon(&path, icon.as_deref())?;
        }
    }
    if let Some(dir) = taskbar_dir() {
        for path in matching_shortcuts(&dir, &profile.id) {
            update_link_icon(&path, icon.as_deref())?;
        }
    }
    Ok(())
}

fn update_link_icon(link_path: &Path, icon: Option<&Path>) -> Result<(), String> {
    let icon_path = match icon {
        Some(path) => path.display().to_string(),
        None => std::env::current_exe()
            .map_err(|error| error.to_string())?
            .display()
            .to_string(),
    };
    com_init();
    unsafe {
        let link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(win_err)?;
        let file: IPersistFile = link.cast().map_err(win_err)?;
        file.Load(&HSTRING::from(link_path.as_os_str()), STGM_READWRITE)
            .map_err(win_err)?;
        link.SetIconLocation(&HSTRING::from(&icon_path), 0)
            .map_err(win_err)?;
        file.Save(&HSTRING::from(link_path.as_os_str()), true)
            .map_err(win_err)?;
    }
    notify_item(link_path);
    Ok(())
}

fn owned_icon(app: &AppHandle, id: &str, icon_file: &str) -> Option<PathBuf> {
    if !is_owned_icon_name(icon_file) {
        return None;
    }
    let path = paths::profile_data_dir(app, id).join(icon_file);
    path.is_file().then_some(path)
}

fn clear_icon_files(dir: &Path, icon_file: &str) {
    let _ = fs::remove_file(dir.join("window-icon.png"));
    if is_owned_icon_name(icon_file) {
        let _ = fs::remove_file(dir.join(icon_file));
    }
}

fn is_owned_icon_name(name: &str) -> bool {
    !name.is_empty()
        && Path::new(name).file_name().is_some_and(|file| file == name)
        && name.starts_with("window-icon-")
        && name.ends_with(".ico")
        && !name.contains(['/', '\\'])
}

fn icon_stamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn taskbar_ids() -> Vec<String> {
    let Some(dir) = taskbar_dir() else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| shortcut_profile_id(&entry.path()))
        .collect()
}

fn matching_shortcuts(dir: &Path, id: &str) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| shortcut_profile_id(path).as_deref() == Some(id))
        .collect()
}

fn shortcut_profile_id(path: &Path) -> Option<String> {
    let extension = path.extension()?.to_str()?;
    if !extension.eq_ignore_ascii_case("lnk") {
        return None;
    }
    com_init();
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let file: IPersistFile = link.cast().ok()?;
        file.Load(&HSTRING::from(path.as_os_str()), STGM_READ)
            .ok()?;
        let mut buffer = [0u16; 1024];
        link.GetArguments(&mut buffer).ok()?;
        if let Some(id) = profile_id_from_args(&wide_string(&buffer)) {
            return Some(id);
        }
        let store: IPropertyStore = link.cast().ok()?;
        let value = store.GetValue(&app_model_key(5)).ok()?;
        prop_text(&value).and_then(|model| profile_id_from_model(&model))
    }
}

fn profile_id_from_args(args: &str) -> Option<String> {
    let mut parts = args.split_whitespace();
    while let Some(part) = parts.next() {
        if let Some(id) = part.strip_prefix("--profile=") {
            return Some(id.to_string());
        }
        if part == "--profile" {
            return parts.next().map(str::to_string);
        }
    }
    None
}

fn taskbar_dir() -> Option<PathBuf> {
    let pinned = known_folder(&FOLDERID_UserPinned).ok()?;
    let dir = pinned.join("TaskBar");
    dir.is_dir().then_some(dir)
}

fn notify_item(path: &Path) {
    let wide = wide(path.as_os_str());
    unsafe {
        SHChangeNotify(
            SHCNE_UPDATEITEM,
            SHCNF_PATHW,
            Some(wide.as_ptr() as *const _),
            None,
        );
    }
}

fn wide_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|unit| *unit == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

fn relaunch_command(id: &str) -> String {
    let Ok(exe) = std::env::current_exe() else {
        return String::new();
    };
    if id == DEFAULT_ID {
        format!("\"{}\"", exe.display())
    } else {
        format!("\"{}\" --profile {id}", exe.display())
    }
}

fn app_model_id(id: &str) -> String {
    if id == DEFAULT_ID {
        "com.ce.browser".into()
    } else {
        format!("com.ce.browser.{id}")
    }
}

fn app_model_key(pid: u32) -> PROPERTYKEY {
    PROPERTYKEY {
        fmtid: GUID::from_u128(APP_MODEL_FMT),
        pid,
    }
}

fn set_text(store: &IPropertyStore, key: PROPERTYKEY, text: &str) -> Result<(), String> {
    let wide = HSTRING::from(text);
    unsafe {
        let mut value = PROPVARIANT::default();
        (*value.Anonymous.Anonymous).vt = VT_LPWSTR;
        (*value.Anonymous.Anonymous).Anonymous.pwszVal = PWSTR(wide.as_ptr() as *mut u16);
        let result = store.SetValue(&key, &value);
        // SetValue copies the text. Drop would free this pointer with CoTaskMemFree,
        // but it belongs to `wide`, and that frees the wrong heap.
        (*value.Anonymous.Anonymous).vt = VT_EMPTY;
        (*value.Anonymous.Anonymous).Anonymous.pwszVal = PWSTR::null();
        result.map_err(win_err)?;
    }
    Ok(())
}

fn shortcut_path(profile: &ProfileRecord) -> Option<PathBuf> {
    let path = PathBuf::from(&profile.shortcut);
    if path.as_os_str().is_empty() {
        None
    } else {
        Some(path)
    }
}

fn pwstr_path(raw: PWSTR) -> PathBuf {
    if raw.0.is_null() {
        return PathBuf::new();
    }
    let mut length = 0;
    unsafe {
        while *raw.0.add(length) != 0 {
            length += 1;
        }
        let slice = std::slice::from_raw_parts(raw.0, length);
        PathBuf::from(OsString::from_wide(slice))
    }
}

fn com_init() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
}

fn win_err(error: windows::core::Error) -> String {
    let message = error.message();
    if message.is_empty() {
        format!("{error}")
    } else {
        message.to_string()
    }
}

fn wide(value: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    value
        .as_ref()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

const PROFILE_FILES: &[&str] = &[
    "settings.json",
    "history.json",
    "bookmarks.json",
    "extensions.json",
    "downloads.json",
    "session.json",
    "styles.css",
    "site-controls.json",
    "permissions.json",
    "containers.json",
    "keywords.json",
    "shortcuts.json",
    "rewrite-rules.json",
    "window-icon.png",
];

/// Moves profile stores between app data and a sync folder. Both keep `profiles.json` and one
/// folder per profile at the root.
/// `overwrite` replaces the destination. When it is false and the destination already has a file,
/// the source copy is removed so the profile is not kept in both places.
pub fn transfer_appdata_to_sync(appdata: &Path, sync_folder: &Path) -> Result<bool, String> {
    transfer_profiles(appdata, true, sync_folder, false, false)
}

pub fn transfer_sync_to_sync(from: &Path, to: &Path) -> Result<bool, String> {
    if from == to {
        return Ok(false);
    }
    transfer_profiles(from, false, to, false, false)
}

pub fn transfer_sync_to_appdata(sync_folder: &Path, appdata: &Path) -> Result<bool, String> {
    transfer_profiles(sync_folder, false, appdata, true, true)
}

/// Returns true when any profile folder moved, so shortcut icons need to point at the new place.
fn transfer_profiles(
    from: &Path,
    from_appdata: bool,
    to: &Path,
    to_appdata: bool,
    overwrite: bool,
) -> Result<bool, String> {
    let mut moved = migrate_layout(from, from_appdata)?;
    moved |= migrate_layout(to, to_appdata)?;
    let source = read_profiles_file(&from.join("profiles.json"));
    move_entry(&from.join("profiles.json"), &to.join("profiles.json"), overwrite)?;
    let file = read_profiles_file(&to.join("profiles.json"));
    for profile in &file.profiles {
        let from_dir = from.join(folder_of(&source, &profile.id));
        if from_dir.exists() {
            move_profile_dir(&from_dir, &to.join(&profile.folder), overwrite)?;
            moved = true;
        }
    }
    Ok(moved)
}

/// Puts every profile in `<root>\<folder>`. Older builds kept the default profile's files loose
/// in app data (`loose_default`), other profiles in `profiles\<id>`, and the default profile of a
/// sync folder in `ce-browser`. A folder already in this layout is left unchanged.
/// Returns true when a profile folder or file moved.
pub fn migrate_layout(root: &Path, loose_default: bool) -> Result<bool, String> {
    let registry = root.join("profiles.json");
    let stored_value = fs::read_to_string(&registry)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());
    let mut file: ProfilesFile = if registry.is_file() {
        files::read_json(&registry)
    } else {
        ProfilesFile::default()
    };
    ensure_default(&mut file);
    let stored_folders: Vec<(String, String)> = file
        .profiles
        .iter()
        .map(|profile| (profile.id.clone(), profile.folder.clone()))
        .collect();
    migrate_default(&mut file);
    assign_folders(&mut file);

    let mut moved = false;
    if loose_default && has_loose_files(root) {
        move_loose_profile(root, &root.join(folder_of(&file, DEFAULT_ID)), false)?;
        moved = true;
    }
    for profile in &file.profiles {
        let dest = root.join(&profile.folder);
        let old = stored_folders
            .iter()
            .find(|(id, _)| id == &profile.id)
            .map(|(_, folder)| folder.as_str())
            .unwrap_or_default();
        if valid_folder_name(old) && !old.eq_ignore_ascii_case(&profile.folder) {
            let source = root.join(old);
            if source.is_dir() {
                move_profile_dir(&source, &dest, false)?;
                moved = true;
            }
        }
        let nested = root.join("profiles").join(&profile.id);
        if nested.is_dir() {
            move_profile_dir(&nested, &dest, false)?;
            moved = true;
        }
    }
    if let Ok(entries) = fs::read_dir(root.join("profiles")) {
        let leftover = entries.flatten().any(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()));
        if !leftover {
            let _ = fs::remove_dir_all(root.join("profiles"));
        }
    }

    let current = serde_json::to_value(&file).ok();
    let changed = stored_value.is_some() && stored_value != current;
    if moved || changed {
        files::write_json(&registry, &file)?;
    }
    Ok(moved)
}

fn has_loose_files(dir: &Path) -> bool {
    PROFILE_FILES.iter().any(|name| dir.join(name).is_file()) || dir.join("extensions").is_dir()
}

/// Points Start menu and taskbar shortcuts at each profile's icon after its folder moved.
pub fn refresh_icons(app: &AppHandle) {
    for profile in load(app).profiles {
        if let Err(error) = refresh_shell_icons(app, &profile) {
            eprintln!("profile icon: {error}");
        }
    }
}

pub fn discard_appdata_profiles(appdata: &Path) -> Result<(), String> {
    let file = read_profiles_file(&appdata.join("profiles.json"));
    for profile in &file.profiles {
        if valid_folder_name(&profile.folder) {
            remove_path(&appdata.join(&profile.folder))?;
        }
    }
    for name in PROFILE_FILES {
        remove_path(&appdata.join(name))?;
    }
    remove_path(&appdata.join("extensions"))?;
    remove_path(&appdata.join("profiles.json"))?;
    remove_path(&appdata.join("profiles"))?;
    let Ok(entries) = fs::read_dir(appdata) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let text = name.to_string_lossy();
        if text.starts_with("window-icon") && (text.ends_with(".ico") || text.ends_with(".png")) {
            remove_path(&entry.path())?;
        }
    }
    Ok(())
}

fn move_loose_profile(from_dir: &Path, to_dir: &Path, overwrite: bool) -> Result<(), String> {
    if !from_dir.is_dir() {
        return Ok(());
    }
    let _ = fs::create_dir_all(to_dir);
    for name in PROFILE_FILES {
        move_entry(&from_dir.join(name), &to_dir.join(name), overwrite)?;
    }
    move_entry(&from_dir.join("extensions"), &to_dir.join("extensions"), overwrite)?;
    let Ok(entries) = fs::read_dir(from_dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let text = name.to_string_lossy();
        if text.starts_with("window-icon") && text.ends_with(".ico") {
            move_entry(&entry.path(), &to_dir.join(name), overwrite)?;
        }
    }
    Ok(())
}

fn move_profile_dir(from: &Path, to: &Path, overwrite: bool) -> Result<(), String> {
    if !from.exists() {
        return Ok(());
    }
    if to.join("settings.json").is_file() && !overwrite {
        remove_path(from)?;
        return Ok(());
    }
    if to.exists() && overwrite {
        remove_path(to)?;
    }
    if to.exists() {
        return move_loose_profile(from, to, overwrite);
    }
    move_entry(from, to, false)
}

fn move_entry(from: &Path, to: &Path, overwrite: bool) -> Result<(), String> {
    if !from.exists() {
        return Ok(());
    }
    if to.exists() {
        if overwrite {
            remove_path(to)?;
        } else {
            remove_path(from)?;
            return Ok(());
        }
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    if fs::rename(from, to).is_ok() {
        return Ok(());
    }
    if from.is_dir() {
        files::copy_dir(from, to)?;
        fs::remove_dir_all(from).map_err(|error| error.to_string())?;
    } else {
        fs::copy(from, to).map_err(|error| format!("Could not copy {}: {error}", from.display()))?;
        fs::remove_file(from).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn remove_path(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    if path.is_dir() {
        fs::remove_dir_all(path).map_err(|error| error.to_string())
    } else {
        fs::remove_file(path).map_err(|error| error.to_string())
    }
}

fn read_profiles_file(path: &Path) -> ProfilesFile {
    let mut file: ProfilesFile = if path.is_file() {
        files::read_json(path)
    } else {
        ProfilesFile::default()
    };
    ensure_default(&mut file);
    migrate_default(&mut file);
    assign_folders(&mut file);
    file
}

fn folder_of(file: &ProfilesFile, id: &str) -> String {
    file.profiles
        .iter()
        .find(|profile| profile.id == id)
        .map(|profile| profile.folder.clone())
        .filter(|folder| valid_folder_name(folder))
        .unwrap_or_else(|| fallback_folder(id))
}

fn taken_folders(file: &ProfilesFile) -> Vec<String> {
    file.profiles
        .iter()
        .map(|profile| profile.folder.clone())
        .filter(|folder| valid_folder_name(folder))
        .collect()
}

/// Gives every profile without a stored folder one that no other profile uses.
fn assign_folders(file: &mut ProfilesFile) {
    let mut used: Vec<String> = Vec::new();
    for profile in &mut file.profiles {
        if valid_folder_name(&profile.folder) && !folder_taken(&profile.folder, &used) {
            used.push(profile.folder.clone());
        } else {
            profile.folder.clear();
        }
    }
    for profile in &mut file.profiles {
        if profile.folder.is_empty() {
            profile.folder = unique_directory_name(&profile.name, &profile.id, &used);
            used.push(profile.folder.clone());
        }
    }
}

fn fallback_folder(id: &str) -> String {
    if id == DEFAULT_ID {
        DEFAULT_FOLDER.into()
    } else {
        let name = sanitize_directory_name(id);
        if valid_folder_name(&name) {
            name
        } else {
            "profile".into()
        }
    }
}

fn unique_directory_name(name: &str, id: &str, used: &[String]) -> String {
    let base = sanitize_directory_name(name);
    let base = if valid_folder_name(&base) {
        base
    } else {
        fallback_folder(id)
    };
    if !folder_taken(&base, used) {
        return base;
    }
    let with_id = format!("{base} ({id})");
    if !folder_taken(&with_id, used) {
        return with_id;
    }
    let mut index = 2;
    loop {
        let candidate = format!("{base} ({index})");
        if !folder_taken(&candidate, used) {
            return candidate;
        }
        index += 1;
    }
}

fn sanitize_directory_name(name: &str) -> String {
    let mut folder = String::new();
    for character in name.chars() {
        if matches!(character, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || character.is_control() {
            folder.push('-');
        } else {
            folder.push(character);
        }
        if folder.chars().count() >= 80 {
            break;
        }
    }
    folder.trim().trim_end_matches(['.', ' ']).trim().to_string()
}

fn valid_folder_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.eq_ignore_ascii_case("profiles")
        && !name.ends_with('.')
        && !name.ends_with(' ')
        && !name.chars().any(|character| {
            matches!(character, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || character.is_control()
        })
}

fn folder_taken(name: &str, used: &[String]) -> bool {
    used.iter().any(|folder| folder.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{
        ProfilesFile, migrate_layout, profile_child_dir, purge_profile_storage, read_profiles_file,
        remove_profile_dir, safe_profile_id,
    };

    fn registry(root: &std::path::Path) -> ProfilesFile {
        read_profiles_file(&root.join("profiles.json"))
    }

    #[test]
    fn deleting_a_profile_removes_its_stores() {
        let root = std::env::temp_dir().join(format!("ce-browser-profile-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let id = "work";
        let roaming = root.join("roaming");
        let data = roaming.join("Work");
        let webview = root.join("local").join("profiles").join(id);
        fs::create_dir_all(data.join("extensions")).unwrap();
        fs::create_dir_all(&webview).unwrap();
        for name in ["settings.json", "history.json", "bookmarks.json", "extensions.json"] {
            fs::write(data.join(name), "{}").unwrap();
        }
        fs::write(data.join("extensions").join("manifest.json"), "{}").unwrap();
        fs::write(webview.join("Cookies"), "x").unwrap();

        purge_profile_storage(&roaming, &data, &webview, id).unwrap();

        assert!(!data.exists());
        assert!(!webview.exists());
        assert!(roaming.exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn old_app_data_moves_into_one_folder_per_profile() {
        let root = std::env::temp_dir().join(format!("ce-browser-appdata-layout-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("profiles").join("work").join("extensions")).unwrap();
        fs::create_dir_all(root.join("extensions").join("ublock")).unwrap();
        fs::write(
            root.join("profiles.json"),
            r#"{"profiles":[{"id":"default","name":"ce-browser"},{"id":"work","name":"Work"}]}"#,
        )
        .unwrap();
        fs::write(root.join("sync-folder.json"), "{}").unwrap();
        for name in ["settings.json", "downloads.json", "session.json", "styles.css"] {
            fs::write(root.join(name), "{}").unwrap();
        }
        fs::write(root.join("extensions").join("ublock").join("manifest.json"), "{}").unwrap();
        fs::write(root.join("profiles").join("work").join("settings.json"), "{}").unwrap();

        assert!(migrate_layout(&root, true).unwrap());

        for name in ["settings.json", "downloads.json", "session.json", "styles.css"] {
            assert!(root.join("Default").join(name).is_file(), "{name}");
            assert!(!root.join(name).exists(), "{name}");
        }
        assert!(root.join("Default").join("extensions").join("ublock").join("manifest.json").is_file());
        assert!(root.join("Work").join("settings.json").is_file());
        assert!(!root.join("profiles").exists());
        assert!(root.join("sync-folder.json").is_file());
        let file = registry(&root);
        assert_eq!(file.profiles[0].name, "Default");
        assert_eq!(file.profiles[0].folder, "Default");
        assert_eq!(file.profiles[1].folder, "Work");

        let written = fs::read_to_string(root.join("profiles.json")).unwrap();
        assert!(!migrate_layout(&root, true).unwrap());
        assert_eq!(fs::read_to_string(root.join("profiles.json")).unwrap(), written);
        assert!(root.join("Default").join("settings.json").is_file());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn an_old_sync_folder_renames_ce_browser_to_default() {
        let root = std::env::temp_dir().join(format!("ce-browser-sync-layout-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("ce-browser")).unwrap();
        fs::create_dir_all(root.join("Work")).unwrap();
        fs::write(
            root.join("profiles.json"),
            r#"{"profiles":[{"id":"default","name":"ce-browser","folder":"ce-browser"},{"id":"work","name":"Work","folder":"Work"}]}"#,
        )
        .unwrap();
        fs::write(root.join("ce-browser").join("settings.json"), "{}").unwrap();
        fs::write(root.join("Work").join("settings.json"), "{}").unwrap();

        assert!(migrate_layout(&root, false).unwrap());

        assert!(root.join("Default").join("settings.json").is_file());
        assert!(!root.join("ce-browser").exists());
        assert!(root.join("Work").join("settings.json").is_file());
        let file = registry(&root);
        assert_eq!(file.profiles[0].name, "Default");
        assert_eq!(file.profiles[0].folder, "Default");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_profile_already_named_default_keeps_its_folder() {
        let root = std::env::temp_dir().join(format!("ce-browser-default-clash-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("ce-browser")).unwrap();
        fs::create_dir_all(root.join("Default")).unwrap();
        fs::write(
            root.join("profiles.json"),
            r#"{"profiles":[{"id":"default","name":"ce-browser","folder":"ce-browser"},{"id":"default-2","name":"Default","folder":"Default"}]}"#,
        )
        .unwrap();
        fs::write(root.join("ce-browser").join("settings.json"), "{\"a\":1}").unwrap();
        fs::write(root.join("Default").join("settings.json"), "{\"b\":2}").unwrap();

        migrate_layout(&root, false).unwrap();

        assert_eq!(fs::read_to_string(root.join("ce-browser").join("settings.json")).unwrap(), "{\"a\":1}");
        assert_eq!(fs::read_to_string(root.join("Default").join("settings.json")).unwrap(), "{\"b\":2}");
        let file = registry(&root);
        assert_eq!(file.profiles[0].folder, "ce-browser");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_profile_delete_cannot_remove_the_registry() {
        let root = std::env::temp_dir().join(format!(
            "ce-browser-profile-guard-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("profiles")).unwrap();
        fs::write(root.join("profiles.json"), "{}").unwrap();

        let error = remove_profile_dir(&root.join("profiles").join(".."), "nope", &root).unwrap_err();
        assert!(error.contains("outside"));
        assert!(root.join("profiles.json").is_file());
        assert!(!profile_child_dir(&root, "default", &root));
        assert!(!safe_profile_id("default"));
        assert!(!safe_profile_id("../default"));
        assert!(safe_profile_id("work"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn profiles_move_into_the_sync_folder_and_leave_app_data() {
        let root = std::env::temp_dir().join(format!("ce-browser-sync-move-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let appdata = root.join("appdata");
        let sync = root.join("sync");
        fs::create_dir_all(appdata.join("profiles").join("work").join("extensions")).unwrap();
        fs::create_dir_all(appdata.join("extensions")).unwrap();
        fs::write(
            appdata.join("profiles.json"),
            r#"{"profiles":[{"id":"default","name":"ce-browser"},{"id":"work","name":"Work"}]}"#,
        )
        .unwrap();
        fs::write(appdata.join("sync-folder.json"), "{}").unwrap();
        for name in ["settings.json", "history.json", "bookmarks.json", "extensions.json"] {
            fs::write(appdata.join(name), "{}").unwrap();
        }
        fs::write(appdata.join("extensions").join("manifest.json"), "{}").unwrap();
        fs::write(appdata.join("profiles").join("work").join("settings.json"), "{}").unwrap();

        super::transfer_appdata_to_sync(&appdata, &sync).unwrap();

        assert!(sync.join("profiles.json").is_file());
        assert!(sync.join("Default").join("settings.json").is_file());
        assert!(sync.join("Default").join("history.json").is_file());
        assert!(sync.join("Default").join("bookmarks.json").is_file());
        assert!(sync.join("Default").join("extensions").join("manifest.json").is_file());
        assert!(sync.join("Work").join("settings.json").is_file());
        assert!(!sync.join("profiles").exists());
        assert!(!appdata.join("settings.json").exists());
        assert!(!appdata.join("Default").exists());
        assert!(!appdata.join("Work").exists());
        assert!(!appdata.join("profiles.json").exists());
        assert!(!appdata.join("profiles").exists());
        assert!(appdata.join("sync-folder.json").is_file());

        super::transfer_sync_to_appdata(&sync, &appdata).unwrap();

        assert!(appdata.join("Default").join("settings.json").is_file());
        assert!(appdata.join("Default").join("extensions").join("manifest.json").is_file());
        assert!(appdata.join("Work").join("settings.json").is_file());
        assert!(appdata.join("profiles.json").is_file());
        assert!(!sync.join("Default").exists());
        assert!(!sync.join("profiles.json").exists());
        let _ = fs::remove_dir_all(&root);
    }
}
