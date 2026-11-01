//! Windows integration: site windows, a summon hotkey, toasts, the default browser, the tray, and battery saver.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::{files, paths, profiles, settings, tabs};

const LAUNCH_FILE: &str = "launch.json";
const APPS_FILE: &str = "apps.json";
pub const DEFAULT_HOTKEY: &str = "ctrl+alt+space";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launch {
    None,
    Url(String),
    App(String),
    Open(String),
}

static LAUNCH: Mutex<Launch> = Mutex::new(Launch::None);
static TRAY: OnceLock<tauri::tray::TrayIcon> = OnceLock::new();
static FRAMES_CAPPED: Mutex<bool> = Mutex::new(false);

pub fn read_args() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    *LAUNCH.lock().expect("launch") = launch_from_args(&args);
}

pub fn launch_from_args(args: &[String]) -> Launch {
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        if let Some(id) = arg.strip_prefix("--app=") {
            return Launch::App(id.to_string());
        }
        if arg == "--app" {
            if let Some(id) = args.get(index + 1) {
                return Launch::App(id.clone());
            }
        }
        if let Some(path) = arg.strip_prefix("--open=") {
            return Launch::Open(path.to_string());
        }
        if arg == "--open" {
            if let Some(path) = args.get(index + 1) {
                return Launch::Open(path.clone());
            }
        }
        if is_website(arg) {
            return Launch::Url(arg.to_string());
        }
        index += 1;
    }
    Launch::None
}

pub fn is_website(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

pub fn site_id(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return None;
    }
    let host = parsed.host_str()?.trim_start_matches("www.");
    let id: String = host
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let id = id.trim_matches('-').to_string();
    if id.is_empty() { None } else { Some(id) }
}

pub fn hotkey_mods(token: &str) -> Option<(u32, u32)> {
    let mut mods = 0u32;
    let mut key = None;
    for part in token
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => mods |= 0x0002,
            "alt" => mods |= 0x0001,
            "shift" => mods |= 0x0004,
            "space" => key = Some(0x20),
            other if other.len() == 1 => {
                let character = other.chars().next()?.to_ascii_uppercase();
                if character.is_ascii_alphabetic() {
                    key = Some(character as u32);
                }
            }
            _ => {}
        }
    }
    key.map(|key| (mods, key))
}

pub fn power_is_battery(ac_line: u8) -> bool {
    ac_line == 0
}

pub fn write_handoff(app: &AppHandle) {
    let launch = LAUNCH.lock().expect("launch").clone();
    if matches!(launch, Launch::None) {
        return;
    }
    let path = paths::root_local_dir(app).join(LAUNCH_FILE);
    let _ = files::write_json(&path, &launch_file(&launch));
}

pub fn start(app: &AppHandle) {
    let _ = register_browser();
    start_hotkey(app.clone());
    let _ = install_tray(app);
    apply_launch(app);
}

fn launch_file(launch: &Launch) -> serde_json::Value {
    match launch {
        Launch::Url(url) => serde_json::json!({ "url": url }),
        Launch::App(id) => serde_json::json!({ "app": id }),
        Launch::Open(path) => serde_json::json!({ "open": path }),
        Launch::None => serde_json::json!({}),
    }
}

pub fn apply_launch(app: &AppHandle) {
    let pending = {
        let mut guard = LAUNCH.lock().expect("launch");
        std::mem::replace(&mut *guard, Launch::None)
    };
    open_launch(app, pending);
    let path = paths::root_local_dir(app).join(LAUNCH_FILE);
    if let Ok(text) = fs::read_to_string(&path) {
        let _ = fs::remove_file(&path);
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
            let launch = if let Some(url) = value.get("url").and_then(|item| item.as_str()) {
                Launch::Url(url.to_string())
            } else if let Some(id) = value.get("app").and_then(|item| item.as_str()) {
                Launch::App(id.to_string())
            } else if let Some(path) = value.get("open").and_then(|item| item.as_str()) {
                Launch::Open(path.to_string())
            } else {
                Launch::None
            };
            open_launch(app, launch);
        }
    }
}

fn open_launch(app: &AppHandle, launch: Launch) {
    match launch {
        Launch::Url(url) if is_website(&url) => {
            let _ = tabs::create_tab(app, Some(url), false, None);
            tabs::show_main(app);
        }
        Launch::App(id) => {
            if let Some(site) = find_app(app, &id) {
                let _ = tabs::open_site_window(app, &site.name, &site.url);
                mark_open_site(app, &site);
            }
        }
        Launch::Open(path) => {
            let _ = crate::files::open_with_default(Path::new(&path));
        }
        _ => {}
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct SiteApp {
    id: String,
    name: String,
    url: String,
}

impl Default for SiteApp {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            url: String::new(),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct AppFile {
    apps: Vec<SiteApp>,
}

pub fn install_site(app: &AppHandle, id: u32) -> Result<(), String> {
    let url = tabs::tab_page_url(app, id).ok_or("That tab is gone.")?;
    let site_id = site_id(&url).ok_or("Only a website can be installed.")?;
    let snapshot = tabs::snapshot(app);
    let name = snapshot
        .tabs
        .iter()
        .find(|tab| tab.id == id)
        .map(|tab| {
            if tab.title.is_empty() {
                site_id.clone()
            } else {
                tab.title.clone()
            }
        })
        .unwrap_or_else(|| site_id.clone());
    let icon = save_site_icon(
        app,
        &site_id,
        snapshot
            .tabs
            .iter()
            .find(|tab| tab.id == id)
            .and_then(|tab| tab.favicon.as_deref()),
    );
    let model = format!("com.ce.browser.site.{site_id}");
    let arguments = format!("--app {site_id}");
    let _ = profiles::install_link(
        &format!("{name}.lnk"),
        &arguments,
        &name,
        icon.as_deref(),
        &model,
    )?;
    let mut file: AppFile = files::read_json(&paths::data_file(app, APPS_FILE));
    file.apps.retain(|site| site.id != site_id);
    file.apps.insert(
        0,
        SiteApp {
            id: site_id,
            name,
            url,
        },
    );
    files::write_json(&paths::data_file(app, APPS_FILE), &file)
}

fn find_app(app: &AppHandle, id: &str) -> Option<SiteApp> {
    let file: AppFile = files::read_json(&paths::data_file(app, APPS_FILE));
    file.apps.into_iter().find(|site| site.id == id)
}

fn save_site_icon(app: &AppHandle, id: &str, favicon: Option<&str>) -> Option<PathBuf> {
    use std::io::Read;
    let fetched = favicon.and_then(|url| {
        if !is_website(url) {
            return None;
        }
        let response = ureq::get(url).timeout(Duration::from_secs(8)).call().ok()?;
        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(1_000_000)
            .read_to_end(&mut bytes)
            .ok()?;
        image::load_from_memory(&bytes).ok()
    });
    let image = fetched
        .or_else(|| image::load_from_memory(crate::profile_icons::default_png()).ok())?
        .to_rgba8();
    let dir = paths::data_dir(app).join("apps");
    fs::create_dir_all(&dir).ok()?;
    let ico = crate::profile_icons::encode_ico(&image).ok()?;
    let path = dir.join(format!("{id}.ico"));
    fs::write(&path, ico).ok()?;
    Some(path)
}

pub fn mark_site_window(app: &AppHandle, window: &tauri::Window, id: &str, name: &str) {
    let model = format!("com.ce.browser.site.{id}");
    let relaunch = paths::launch_exe()
        .map(|exe| format!("\"{}\" --app {id}", exe.display()))
        .unwrap_or_default();
    let icon = paths::data_dir(app).join("apps").join(format!("{id}.ico"));
    let icon = if icon.is_file() {
        icon.display().to_string()
    } else {
        String::new()
    };
    let _ = profiles::mark_model(window, &model, &relaunch, name, &icon);
    if icon_file_ok(&icon) {
        if let Ok(image) = tauri::image::Image::from_path(&icon) {
            let _ = window.set_icon(image);
        }
    }
}

fn icon_file_ok(path: &str) -> bool {
    !path.is_empty() && Path::new(path).is_file()
}

fn mark_open_site(app: &AppHandle, site: &SiteApp) {
    let labels = tabs::browser_labels(app);
    if let Some(label) = labels.last() {
        if let Some(window) = app.get_window(label) {
            let _ = window.set_title(&site.name);
            mark_site_window(app, &window, &site.id, &site.name);
        }
    }
}

pub fn summon(app: &AppHandle) {
    tabs::show_main(app);
}

fn start_hotkey(app: AppHandle) {
    std::thread::spawn(move || {
        let token = settings::get(&app).summon_hotkey;
        let Some((mods, key)) = hotkey_mods(if token.trim().is_empty() {
            DEFAULT_HOTKEY
        } else {
            token.as_str()
        }) else {
            return;
        };
        unsafe { hotkey_loop(&app, mods, key) };
    });
}

unsafe fn hotkey_loop(app: &AppHandle, mods: u32, key: u32) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{HOT_KEY_MODIFIERS, RegisterHotKey};
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, MSG, TranslateMessage,
    };
    unsafe {
        if RegisterHotKey(None, 1, HOT_KEY_MODIFIERS(mods), key).is_err() {
            return;
        }
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            if message.message == 0x0312 {
                summon(app);
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

fn install_tray(app: &AppHandle) -> Result<(), String> {
    use tauri::image::Image;
    use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
    let icon = Image::from_bytes(crate::profile_icons::default_png())
        .map_err(|error| error.to_string())?;
    let menu = tray_menu(app)?;
    let tray = TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            if id == "show" || id == "search" {
                summon(app);
            } else if id == "quit" {
                app.exit(0);
            } else if let Some(tab) = id.strip_prefix("tab-") {
                if let Ok(tab) = tab.parse::<u32>() {
                    let _ = tabs::activate(app, tab);
                    summon(app);
                }
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                ..
            } = event
            {
                summon(tray.app_handle());
            }
        })
        .build(app)
        .map_err(|error| error.to_string())?;
    let _ = TRAY.set(tray);
    Ok(())
}

fn tray_menu(app: &AppHandle) -> Result<tauri::menu::Menu<tauri::Wry>, String> {
    use tauri::menu::{Menu, MenuItem};
    let mut items: Vec<MenuItem<tauri::Wry>> = Vec::new();
    for (id, title) in tabs::recent_tabs(app) {
        let label: String = title.chars().take(80).collect();
        if let Ok(item) = MenuItem::with_id(app, format!("tab-{id}"), label, true, None::<&str>) {
            items.push(item);
        }
    }
    let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>)
        .map_err(|error| error.to_string())?;
    let search = MenuItem::with_id(app, "search", "Quick search", true, None::<&str>)
        .map_err(|error| error.to_string())?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)
        .map_err(|error| error.to_string())?;
    let mut refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> =
        items.iter().map(|item| item as _).collect();
    refs.push(&show);
    refs.push(&search);
    refs.push(&quit);
    Menu::with_items(app, &refs).map_err(|error| error.to_string())
}

pub fn refresh_tray(app: &AppHandle) {
    let Some(tray) = TRAY.get() else {
        return;
    };
    if let Ok(menu) = tray_menu(app) {
        let _ = tray.set_menu(Some(menu));
    }
}

pub fn toast(title: &str, body: &str, launch: &str) {
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
    let xml = format!(
        r#"<toast launch="{}"><visual><binding template="ToastGeneric"><text>{}</text><text>{}</text></binding></visual></toast>"#,
        xml_escape(launch),
        xml_escape(title),
        xml_escape(body)
    );
    let document = match XmlDocument::new() {
        Ok(document) => document,
        Err(_) => return,
    };
    if document
        .LoadXml(&windows::core::HSTRING::from(&xml))
        .is_err()
    {
        return;
    }
    let toast = match ToastNotification::CreateToastNotification(&document) {
        Ok(toast) => toast,
        Err(_) => return,
    };
    if let Ok(notifier) = ToastNotificationManager::CreateToastNotifierWithId(
        &windows::core::HSTRING::from(profiles::current_model_id()),
    ) {
        let _ = notifier.Show(&toast);
    }
}

pub fn toast_download(name: &str, path: &str) {
    toast("Download finished", name, &format!("--open={path}"));
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn register_browser() -> Result<(), String> {
    let exe = paths::launch_exe().map_err(|error| error.to_string())?;
    let command = format!("\"{}\" \"%1\"", exe.display());
    set_key(r"Software\Classes\ce-browser", "", "ce-browser")?;
    set_key(
        r"Software\Classes\ce-browser\shell\open\command",
        "",
        &command,
    )?;
    set_key(r"Software\Classes\ce-browserHTML", "", "HTML document")?;
    set_key(
        r"Software\Classes\ce-browserHTML\shell\open\command",
        "",
        &command,
    )?;
    set_key(
        r"Software\Clients\StartMenuInternet\ce-browser",
        "",
        "ce-browser",
    )?;
    set_key(
        r"Software\Clients\StartMenuInternet\ce-browser\Capabilities",
        "ApplicationName",
        "ce-browser",
    )?;
    set_key(
        r"Software\Clients\StartMenuInternet\ce-browser\Capabilities\URLAssociations",
        "http",
        "ce-browser",
    )?;
    set_key(
        r"Software\Clients\StartMenuInternet\ce-browser\Capabilities\URLAssociations",
        "https",
        "ce-browser",
    )?;
    set_key(
        r"Software\Clients\StartMenuInternet\ce-browser\Capabilities\FileAssociations",
        ".html",
        "ce-browserHTML",
    )?;
    set_key(
        r"Software\RegisteredApplications",
        "ce-browser",
        r"Software\Clients\StartMenuInternet\ce-browser\Capabilities",
    )?;
    Ok(())
}

pub fn open_default_settings() -> Result<(), String> {
    std::process::Command::new("cmd")
        .args(["/C", "start", "", "ms-settings:defaultapps"])
        .spawn()
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn set_key(path: &str, name: &str, value: &str) -> Result<(), String> {
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegCreateKeyExW(
            key: isize,
            sub: *const u16,
            reserved: u32,
            class: *const u16,
            options: u32,
            access: u32,
            security: *mut core::ffi::c_void,
            result: *mut isize,
            disposition: *mut u32,
        ) -> i32;
        fn RegSetValueExW(
            key: isize,
            name: *const u16,
            reserved: u32,
            kind: u32,
            data: *const u8,
            bytes: u32,
        ) -> i32;
        fn RegCloseKey(key: isize) -> i32;
    }
    let sub = wide(path);
    let name = wide(name);
    let mut data = wide(value);
    data.pop();
    let bytes = (data.len() * 2) as u32;
    unsafe {
        let mut key = 0isize;
        let mut disposition = 0u32;
        let status = RegCreateKeyExW(
            0x8000_0001u32 as isize,
            sub.as_ptr(),
            0,
            std::ptr::null(),
            0,
            0x20006,
            std::ptr::null_mut(),
            &mut key,
            &mut disposition,
        );
        if status != 0 {
            return Err(format!("Couldn't register the browser ({status})."));
        }
        let status = RegSetValueExW(key, name.as_ptr(), 0, 1, data.as_ptr().cast(), bytes + 2);
        let _ = RegCloseKey(key);
        if status != 0 {
            return Err(format!("Couldn't register the browser ({status})."));
        }
    }
    Ok(())
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn saver_active(app: &AppHandle) -> bool {
    settings::get(app).battery_saver && power_is_battery(ac_line())
}

fn ac_line() -> u8 {
    #[repr(C)]
    struct Power {
        ac: u8,
        flag: u8,
        percent: u8,
        status: u8,
        life: u32,
        full: u32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetSystemPowerStatus(status: *mut Power) -> i32;
    }
    let mut status = Power {
        ac: 255,
        flag: 0,
        percent: 0,
        status: 0,
        life: 0,
        full: 0,
    };
    if unsafe { GetSystemPowerStatus(&mut status) } == 0 {
        return 255;
    }
    status.ac
}

pub fn sync_frames(app: &AppHandle) {
    let on = saver_active(app);
    let changed = {
        let mut guard = FRAMES_CAPPED.lock().expect("frames");
        if *guard == on {
            false
        } else {
            *guard = on;
            true
        }
    };
    if !changed {
        return;
    }
    let script = format!(
        r#"(function(on) {{
          if (on) {{
            if (window.__ceFps) return;
            window.__ceFpsNative = window.requestAnimationFrame.bind(window);
            var last = 0;
            window.requestAnimationFrame = function(callback) {{
              return window.__ceFpsNative(function(time) {{
                if (time - last < 33) {{ window.requestAnimationFrame(callback); return; }}
                last = time;
                callback(time);
              }});
            }};
            window.__ceFps = true;
          }} else if (window.__ceFpsNative) {{
            window.requestAnimationFrame = window.__ceFpsNative;
            window.__ceFps = false;
          }}
        }})({on})"#
    );
    for (id, _) in tabs::recent_tabs(app) {
        if let Some(webview) = tabs::page_view(app, id) {
            let _ = webview.eval(&script);
        }
    }
}

pub const NOTIFY_JS: &str = r#"(function () {
  if (window.__ceNotify || !window.Notification) return;
  window.__ceNotify = true;
  var Native = window.Notification;
  function Wrapped(title, options) {
    try {
      if (window.chrome && window.chrome.webview) {
        window.chrome.webview.postMessage({ type: "ce-notify", title: String(title || ""), body: options && options.body ? String(options.body) : "" });
      }
    } catch (error) {}
    return new Native(title, options);
  }
  Wrapped.permission = "granted";
  Wrapped.requestPermission = function () { return Promise.resolve("granted"); };
  window.Notification = Wrapped;
})();"#;

#[cfg(test)]
mod tests {
    use super::{Launch, hotkey_mods, is_website, launch_from_args, power_is_battery, site_id};

    #[test]
    fn a_site_id_comes_from_the_host() {
        assert_eq!(
            site_id("https://www.example.com/a").as_deref(),
            Some("example-com")
        );
        assert_eq!(site_id("ce://settings"), None);
    }

    #[test]
    fn a_command_line_website_is_kept() {
        assert_eq!(
            launch_from_args(&["https://example.com".into()]),
            Launch::Url("https://example.com".into())
        );
        assert_eq!(launch_from_args(&["ce://settings".into()]), Launch::None);
        assert!(!is_website("ce://settings"));
    }

    #[test]
    fn the_summon_hotkey_is_ctrl_alt_space() {
        assert_eq!(hotkey_mods("ctrl+alt+space"), Some((0x0002 | 0x0001, 0x20)));
        assert_eq!(hotkey_mods("nope"), None);
    }

    #[test]
    fn battery_power_is_unplugged_only() {
        assert!(power_is_battery(0));
        assert!(!power_is_battery(1));
        assert!(!power_is_battery(255));
    }
}
