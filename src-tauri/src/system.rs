//! Windows integration: site windows, a summon hotkey, toasts, the default browser, the tray, and battery saver.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};

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
    /// Local PDFs to open in the markup editor. Not kept in `LAUNCH`: those wait until
    /// session restore has finished so the new tab stays in front.
    Pdfs(Vec<String>),
}

const PDF_EDITOR: &str = "https://ce-file-tools.vercel.app/pdf/view-and-markup";

struct PendingPdfs {
    ready: bool,
    paths: Vec<String>,
}

#[derive(Clone)]
struct PdfAttach {
    dir: PathBuf,
    /// `https://pdf-<id>.ce-browser.example/document.pdf`. The page fetches this.
    /// `.example` is a reserved name, so it never hits the network.
    fetch: String,
    file_name: String,
}

static LAUNCH: Mutex<Launch> = Mutex::new(Launch::None);
static PDF_LAUNCH: Mutex<PendingPdfs> = Mutex::new(PendingPdfs {
    ready: false,
    paths: Vec::new(),
});
static PDF_ARMED: Mutex<Option<PdfAttach>> = Mutex::new(None);
static PDF_TABS: Mutex<Vec<(u32, PdfAttach)>> = Mutex::new(Vec::new());
static TRAY: OnceLock<tauri::tray::TrayIcon> = OnceLock::new();
static FRAMES_CAPPED: Mutex<bool> = Mutex::new(false);

pub fn read_args() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match launch_from_args(&args) {
        Launch::Pdfs(paths) => {
            PDF_LAUNCH.lock().expect("pdf launch").paths.extend(paths);
        }
        other => *LAUNCH.lock().expect("launch") = other,
    }
}

pub fn launch_from_args(args: &[String]) -> Launch {
    let mut pdfs = Vec::new();
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
        if arg == "--profile" {
            index += 2;
            continue;
        }
        if arg.starts_with("--profile=") {
            index += 1;
            continue;
        }
        if is_website(arg) {
            return Launch::Url(arg.to_string());
        }
        if let Some(path) = pdf_argument(arg) {
            pdfs.push(path);
        }
        index += 1;
    }
    if pdfs.is_empty() {
        Launch::None
    } else {
        Launch::Pdfs(pdfs)
    }
}

/// A command-line PDF from a file association or a `file://` URL. Flags are left alone.
fn pdf_argument(arg: &str) -> Option<String> {
    let arg = arg.trim().trim_matches('"');
    if arg.is_empty() || arg.starts_with('-') {
        return None;
    }
    let path = if arg.len() >= 7 && arg[..7].eq_ignore_ascii_case("file://") {
        url::Url::parse(arg).ok()?.to_file_path().ok()?
    } else {
        PathBuf::from(arg)
    };
    let pdf = path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"));
    if !pdf {
        return None;
    }
    Some(path.to_string_lossy().to_string())
}

pub fn is_pdf_editor(url: &str) -> bool {
    let Ok(page) = url::Url::parse(url) else {
        return false;
    };
    let Ok(editor) = url::Url::parse(PDF_EDITOR) else {
        return false;
    };
    page.host_str() == editor.host_str()
        && page.path().trim_end_matches('/') == editor.path().trim_end_matches('/')
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
    let pdfs = PDF_LAUNCH.lock().expect("pdf launch").paths.clone();
    if matches!(launch, Launch::None) && pdfs.is_empty() {
        return;
    }
    let mut value = launch_file(&launch);
    if !pdfs.is_empty() {
        value["pdfs"] = serde_json::json!(pdfs);
    }
    let path = paths::root_local_dir(app).join(LAUNCH_FILE);
    let _ = files::write_json(&path, &value);
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
        Launch::Pdfs(paths) => serde_json::json!({ "pdfs": paths }),
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
            let pdfs = value
                .get("pdfs")
                .and_then(|item| item.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str())
                        .filter_map(pdf_argument)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            open_launch(app, launch);
            enqueue_pdfs(app, pdfs);
        }
    }
}

fn open_launch(app: &AppHandle, launch: Launch) {
    match launch {
        Launch::Url(url) if is_website(&url) => {
            let _ = tabs::create_tab(app, Some(url), false, None);
            tabs::show_main(app);
        }
        Launch::Url(_) => {}
        Launch::App(id) => {
            if let Some(site) = find_app(app, &id) {
                let _ = tabs::open_site_window(app, &site.name, &site.url);
                mark_open_site(app, &site);
            }
        }
        Launch::Open(path) => {
            let _ = crate::files::open_with_default(Path::new(&path));
        }
        Launch::Pdfs(paths) => enqueue_pdfs(app, paths),
        Launch::None => {}
    }
}

/// Opens queued PDFs once session restore has finished, so the editor tab stays selected.
pub fn open_pending_pdfs(app: &AppHandle) {
    let paths = {
        let mut pending = PDF_LAUNCH.lock().expect("pdf launch");
        pending.ready = true;
        std::mem::take(&mut pending.paths)
    };
    open_pdfs(app, &paths);
}

fn enqueue_pdfs(app: &AppHandle, paths: Vec<String>) {
    if paths.is_empty() {
        return;
    }
    let mut pending = PDF_LAUNCH.lock().expect("pdf launch");
    if pending.ready {
        drop(pending);
        open_pdfs(app, &paths);
    } else {
        pending.paths.extend(paths);
    }
}

/// Opens one PDF in `window`. A download clicked in a browser window stays in that window.
pub fn open_pdf_in_window(app: &AppHandle, path: &str, window: &str) -> Result<(), String> {
    let path = pdf_argument(path).ok_or_else(|| "That file is not a PDF.".to_string())?;
    sweep_old_pdfs();
    open_one_pdf(app, &path, Some(window))?;
    reveal_window(app, Some(window));
    Ok(())
}

fn open_pdfs(app: &AppHandle, paths: &[String]) {
    if paths.is_empty() {
        return;
    }
    sweep_old_pdfs();
    let mut opened = false;
    for path in paths {
        match open_one_pdf(app, path, None) {
            Ok(()) => opened = true,
            Err(error) => eprintln!("pdf: {error}"),
        }
    }
    if opened {
        reveal_window(app, None);
    }
}

fn open_one_pdf(app: &AppHandle, path: &str, window: Option<&str>) -> Result<(), String> {
    let attach = stage_pdf(path)?;
    let title = attach.file_name.clone();
    arm_pdf(attach);
    let url = tabs::resolve(app, PDF_EDITOR);
    let mut tab = tabs::OpenTab::new(url);
    tab.title = Some(title);
    if let Some(window) = window {
        tab.window = window.to_string();
    }
    if let Err(error) = tabs::open(app, tab) {
        if let Some(attach) = disarm_pdf() {
            let _ = fs::remove_dir_all(attach.dir);
        }
        return Err(error);
    }
    Ok(())
}

fn reveal_window(app: &AppHandle, window: Option<&str>) {
    let Some(label) = window.filter(|label| tabs::is_browser_label(label)) else {
        tabs::show_main(app);
        return;
    };
    let Some(browser) = app.get_window(label) else {
        tabs::show_main(app);
        return;
    };
    let _ = browser.unminimize();
    let _ = browser.show();
    let _ = browser.set_focus();
}

fn stage_pdf(source: &str) -> Result<PdfAttach, String> {
    let path = PathBuf::from(source);
    if !path.is_file() {
        return Err(format!("Couldn't find {source}."));
    }
    let id = rand::random::<u64>();
    let dir = std::env::temp_dir()
        .join("ce-browser-pdf")
        .join(format!("{id:x}"));
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let dest = dir.join("document.pdf");
    // A hard link keeps one copy of a large file. Another drive has to be copied.
    if fs::hard_link(&path, &dest).is_err() {
        fs::copy(&path, &dest).map_err(|error| format!("Couldn't read {source}. {error}"))?;
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("document.pdf")
        .to_string();
    Ok(PdfAttach {
        dir,
        fetch: format!("https://pdf-{id:x}.ce-browser.example/document.pdf"),
        file_name,
    })
}

fn sweep_old_pdfs() {
    let root = std::env::temp_dir().join("ce-browser-pdf");
    let Ok(entries) = fs::read_dir(&root) else {
        return;
    };
    let Some(cutoff) = SystemTime::now().checked_sub(Duration::from_secs(24 * 60 * 60)) else {
        return;
    };
    for entry in entries.flatten() {
        let stale = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .is_ok_and(|modified| modified < cutoff);
        if stale {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

fn arm_pdf(attach: PdfAttach) {
    *PDF_ARMED.lock().expect("pdf") = Some(attach);
}

fn disarm_pdf() -> Option<PdfAttach> {
    PDF_ARMED.lock().expect("pdf").take()
}

/// The tab just created takes the PDF that was armed for it, before its page starts loading.
pub fn claim_pdf(id: u32) {
    if let Some(attach) = disarm_pdf() {
        PDF_TABS.lock().expect("pdf").push((id, attach));
    }
}

/// After the markup editor finishes loading, give it the PDF from this open.
pub fn attach_open_pdf(app: &AppHandle, id: u32, url: &str) {
    if !is_pdf_editor(url) {
        return;
    }
    let Some(attach) = PDF_TABS
        .lock()
        .expect("pdf")
        .iter()
        .find(|(tab, _)| *tab == id)
        .map(|(_, attach)| attach.clone())
    else {
        return;
    };
    let Some(webview) = tabs::page_view(app, id) else {
        return;
    };
    crate::webview2::attach_local_pdf(&webview, &attach.fetch, &attach.file_name);
}

/// Bytes for a PDF fetch from the markup editor. The address only exists inside this app.
pub fn pdf_bytes(url: &str) -> Option<Vec<u8>> {
    let path = PDF_TABS
        .lock()
        .expect("pdf")
        .iter()
        .find_map(|(_, attach)| {
            same_pdf_url(url, &attach.fetch).then(|| attach.dir.join("document.pdf"))
        })?;
    fs::read(path).ok()
}

fn same_pdf_url(request: &str, fetch: &str) -> bool {
    request.trim_end_matches('/') == fetch.trim_end_matches('/')
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
    set_key(r"Software\Classes\ce-browserPDF", "", "PDF Document")?;
    set_key(
        r"Software\Classes\ce-browserPDF",
        "Content Type",
        "application/pdf",
    )?;
    set_key(
        r"Software\Classes\ce-browserPDF\DefaultIcon",
        "",
        &format!("\"{}\",0", exe.display()),
    )?;
    set_key(
        r"Software\Classes\ce-browserPDF\shell\open\command",
        "",
        &command,
    )?;
    set_key(
        r"Software\Classes\.pdf\OpenWithProgids",
        "ce-browserPDF",
        "",
    )?;
    set_key(
        r"Software\Clients\StartMenuInternet\ce-browser\Capabilities\FileAssociations",
        ".pdf",
        "ce-browserPDF",
    )?;
    if let Some(name) = exe.file_name().and_then(|name| name.to_str()) {
        let apps = format!(r"Software\Classes\Applications\{name}");
        set_key(&apps, "FriendlyAppName", "ce-browser")?;
        set_key(&format!(r"{apps}\shell\open\command"), "", &command)?;
        set_key(&format!(r"{apps}\SupportedTypes"), ".pdf", "")?;
    }
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
    use super::{
        Launch, hotkey_mods, is_pdf_editor, is_website, launch_from_args, power_is_battery, site_id,
    };

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
    fn a_command_line_pdf_is_opened_in_the_editor() {
        assert_eq!(
            launch_from_args(&[
                "--profile".into(),
                "default".into(),
                r"C:\docs\note.PDF".into()
            ]),
            Launch::Pdfs(vec![r"C:\docs\note.PDF".into()])
        );
        assert_eq!(
            launch_from_args(&["file:///C:/docs/note.pdf".into()]),
            Launch::Pdfs(vec![r"C:\docs\note.pdf".into()])
        );
        assert_eq!(
            launch_from_args(&[r"--open=C:\docs\note.pdf".into()]),
            Launch::Open(r"C:\docs\note.pdf".into())
        );
        assert!(is_pdf_editor(
            "https://ce-file-tools.vercel.app/pdf/view-and-markup"
        ));
        assert!(is_pdf_editor(
            "https://ce-file-tools.vercel.app/pdf/view-and-markup?x=1"
        ));
        assert!(!is_pdf_editor("https://ce-file-tools.vercel.app/convert"));
        assert!(!is_pdf_editor("https://example.com/pdf/view-and-markup"));
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
