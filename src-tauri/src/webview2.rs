use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use base64::Engine;
use tauri::webview::PlatformWebview;
use tauri::{AppHandle, Manager, Webview};
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use webview2_com::*;
use windows::Win32::Foundation::{E_NOINTERFACE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    ClientToScreen, CreateRoundRectRgn, DeleteObject, HGDIOBJ, ScreenToClient, SetWindowRgn,
};
use windows::Win32::System::Com::IStream;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyState, VK_CONTROL, VK_MENU, VK_SHIFT,
};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, EnumChildWindows, GA_PARENT, GA_ROOT, GA_ROOTOWNER, GWL_EXSTYLE, GetAncestor,
    GetClientRect, GetCursorPos, GetForegroundWindow, GetWindowLongPtrW, GetWindowRect, HC_ACTION,
    HWND_TOP, IsChild, IsWindowVisible, MSLLHOOKSTRUCT, PostMessageW, SWP_NOACTIVATE, SWP_NOZORDER,
    SWP_SHOWWINDOW, SendMessageW, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos,
    SetWindowsHookExW, WH_MOUSE_LL, WM_APP, WM_CREATE, WM_LBUTTONDOWN, WM_MBUTTONDOWN,
    WM_MOUSEWHEEL, WM_NCACTIVATE, WM_NCDESTROY, WM_PARENTNOTIFY, WM_RBUTTONDOWN, WS_EX_NOACTIVATE,
    WindowFromPoint,
};
use windows::core::{BOOL, HSTRING, Interface, PWSTR};

use crate::shortcuts::{self, Target};
use crate::{downloads, popup, tabs};

/// Moves a COM pointer into a closure that `run_on_main_thread` or `with_webview` will run
/// on the thread that created it. Never touch the inner value on another thread.
pub struct MainThread<T>(T);
unsafe impl<T> Send for MainThread<T> {}

impl<T> MainThread<T> {
    pub fn new(value: T) -> Self {
        Self(value)
    }

    /// Taking the value through a method makes closures capture the whole wrapper instead of
    /// its (non-`Send`) field.
    pub fn into_inner(self) -> T {
        self.0
    }
}

pub fn read_pwstr(read: impl FnOnce(&mut PWSTR) -> windows::core::Result<()>) -> String {
    let mut value = PWSTR::null();
    match read(&mut value) {
        Ok(()) if !value.is_null() => take_pwstr(value),
        _ => String::new(),
    }
}

pub fn read_bool(read: impl FnOnce(&mut BOOL) -> windows::core::Result<()>) -> bool {
    let mut value = BOOL::default();
    read(&mut value).is_ok() && value.as_bool()
}

pub fn user_agent(core: &ICoreWebView2) -> String {
    unsafe {
        let Ok(settings) = core.Settings() else {
            return String::new();
        };
        let Ok(settings) = settings.cast::<ICoreWebView2Settings2>() else {
            return String::new();
        };
        read_pwstr(|value| settings.UserAgent(value))
    }
}

/// Asks the webview for the cookies it would send to `url`. The callback runs on the webview thread.
pub fn request_cookies(
    core: &ICoreWebView2,
    url: &str,
    done: impl FnOnce(String) + Send + 'static,
) {
    let slot = Arc::new(Mutex::new(Some(done)));
    let fallback = slot.clone();
    let started = (|| -> windows::core::Result<()> {
        let manager = unsafe { core.cast::<ICoreWebView2_2>()?.CookieManager()? };
        let slot = slot.clone();
        let handler = GetCookiesCompletedHandler::create(Box::new(move |result, list| {
            let header = if result.is_ok() {
                cookie_header(list.as_ref())
            } else {
                String::new()
            };
            fire(&slot, header);
            Ok(())
        }));
        unsafe { manager.GetCookies(&HSTRING::from(url), &handler) }
    })();
    if started.is_err() {
        fire(&fallback, String::new());
    }
}

/// Cookie header and user agent for a URL, from a webview that is not this thread.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
}

pub fn list_cookies(webview: &Webview, url: &str) -> Vec<SiteCookie> {
    let (tx, rx) = mpsc::channel();
    let url = url.to_string();
    let posted = webview.with_webview(move |platform| {
        let Ok(core) = core(&platform) else {
            let _ = tx.send(Vec::new());
            return;
        };
        let sender = tx.clone();
        let started = (|| -> windows::core::Result<()> {
            let manager = unsafe { core.cast::<ICoreWebView2_2>()?.CookieManager()? };
            let handler = GetCookiesCompletedHandler::create(Box::new(move |_result, list| {
                let _ = sender.send(read_cookie_list(list.as_ref()));
                Ok(())
            }));
            unsafe { manager.GetCookies(&HSTRING::from(&url), &handler) }
        })();
        if started.is_err() {
            let _ = tx.send(Vec::new());
        }
    });
    if posted.is_err() {
        return Vec::new();
    }
    rx.recv_timeout(Duration::from_secs(3)).unwrap_or_default()
}

pub fn put_cookie(
    webview: &Webview,
    name: String,
    value: String,
    domain: String,
    path: String,
) -> std::result::Result<(), String> {
    let (tx, rx) = mpsc::channel();
    let posted = webview.with_webview(move |platform| {
        let result = (|| -> windows::core::Result<()> {
            let core = core(&platform)?;
            let manager = unsafe { core.cast::<ICoreWebView2_2>()?.CookieManager()? };
            let cookie = unsafe {
                manager.CreateCookie(
                    &HSTRING::from(&name),
                    &HSTRING::from(&value),
                    &HSTRING::from(&domain),
                    &HSTRING::from(&path),
                )?
            };
            unsafe { manager.AddOrUpdateCookie(&cookie) }
        })();
        let _ = tx.send(result.map_err(|error| error.to_string()));
    });
    posted.map_err(|error| error.to_string())?;
    rx.recv_timeout(Duration::from_secs(3))
        .map_err(|_| "WebView2 did not answer.".to_string())?
}

pub fn delete_cookie(
    webview: &Webview,
    name: String,
    domain: String,
    path: String,
) -> std::result::Result<(), String> {
    let (tx, rx) = mpsc::channel();
    let posted = webview.with_webview(move |platform| {
        let result = (|| -> windows::core::Result<()> {
            let core = core(&platform)?;
            let manager = unsafe { core.cast::<ICoreWebView2_2>()?.CookieManager()? };
            unsafe {
                manager.DeleteCookiesWithDomainAndPath(
                    &HSTRING::from(&name),
                    &HSTRING::from(&domain),
                    &HSTRING::from(&path),
                )
            }
        })();
        let _ = tx.send(result.map_err(|error| error.to_string()));
    });
    posted.map_err(|error| error.to_string())?;
    rx.recv_timeout(Duration::from_secs(3))
        .map_err(|_| "WebView2 did not answer.".to_string())?
}

fn read_cookie_list(list: Option<&ICoreWebView2CookieList>) -> Vec<SiteCookie> {
    let Some(list) = list else {
        return Vec::new();
    };
    unsafe {
        let mut count = 0u32;
        if list.Count(&mut count).is_err() {
            return Vec::new();
        }
        let mut cookies = Vec::new();
        for index in 0..count {
            let Ok(cookie) = list.GetValueAtIndex(index) else {
                continue;
            };
            cookies.push(SiteCookie {
                name: read_pwstr(|value| cookie.Name(value)),
                value: read_pwstr(|value| cookie.Value(value)),
                domain: read_pwstr(|value| cookie.Domain(value)),
                path: read_pwstr(|value| cookie.Path(value)),
            });
        }
        cookies
    }
}

pub fn session_for(webview: &Webview, url: &str) -> (String, String) {
    let (tx, rx) = mpsc::channel();
    let url = url.to_string();
    let posted = webview.with_webview(move |platform| {
        let Ok(core) = core(&platform) else {
            let _ = tx.send((String::new(), String::new()));
            return;
        };
        let agent = user_agent(&core);
        let tx = tx.clone();
        request_cookies(&core, &url, move |cookie| {
            let _ = tx.send((cookie, agent));
        });
    });
    if posted.is_err() {
        return (String::new(), String::new());
    }
    rx.recv_timeout(Duration::from_secs(3)).unwrap_or_default()
}

fn fire(slot: &Mutex<Option<impl FnOnce(String)>>, header: String) {
    if let Some(done) = slot.lock().ok().and_then(|mut guard| guard.take()) {
        done(header);
    }
}

fn cookie_header(list: Option<&ICoreWebView2CookieList>) -> String {
    let Some(list) = list else {
        return String::new();
    };
    unsafe {
        let mut count = 0u32;
        if list.Count(&mut count).is_err() {
            return String::new();
        }
        let mut parts = Vec::new();
        for index in 0..count {
            let Ok(cookie) = list.GetValueAtIndex(index) else {
                continue;
            };
            let name = read_pwstr(|value| cookie.Name(value));
            let value = read_pwstr(|value| cookie.Value(value));
            if name.is_empty() || name.contains([';', '\r', '\n']) {
                continue;
            }
            parts.push(format!("{name}={value}"));
        }
        parts.join("; ")
    }
}

fn key_down(key: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY) -> bool {
    unsafe { GetKeyState(key.0 as i32) < 0 }
}

/// Shift, Ctrl, and Alt as held right now. Read from a new-window request: that is how
/// WebView2 delivers Shift+click on a link.
pub fn pointer_modifiers() -> (bool, bool, bool) {
    (
        async_key(VK_SHIFT),
        async_key(VK_CONTROL),
        async_key(VK_MENU),
    )
}

fn async_key(key: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY) -> bool {
    unsafe { (GetAsyncKeyState(i32::from(key.0)) as u16) & 0x8000 != 0 }
}

fn read_stream(stream: &IStream) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        let mut read = 0u32;
        let status = unsafe {
            stream.Read(
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
                Some(&mut read),
            )
        };
        if status.is_err() || read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read as usize]);
        if bytes.len() > 512 * 1024 {
            break;
        }
    }
    (!bytes.is_empty()).then_some(bytes)
}

fn core(platform: &PlatformWebview) -> windows::core::Result<ICoreWebView2> {
    unsafe { platform.controller().CoreWebView2() }
}

/// `prefers-color-scheme` follows Windows. Sites that do not query it keep the colors they paint.
fn pin_profile_scheme(core: &ICoreWebView2) {
    let pinned = (|| unsafe {
        core.cast::<ICoreWebView2_13>()?
            .Profile()?
            .SetPreferredColorScheme(COREWEBVIEW2_PREFERRED_COLOR_SCHEME_AUTO)
    })();
    if let Err(error) = pinned {
        if error.code() != E_NOINTERFACE {
            eprintln!("WebView2 color scheme: {error}");
        }
    }
}

/// Points every open profile at the Windows scheme. `prefers-color-scheme` follows that,
/// and the backdrop behind a remote page is painted separately.
pub fn apply_site_color_scheme(app: &AppHandle) {
    for webview in tabs::open_surface_views(app) {
        let _ = webview.with_webview(|platform| {
            if let Ok(core) = core(&platform) {
                pin_profile_scheme(&core);
            }
        });
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HostScheme {
    Light,
    Dark,
}

fn host_decisions() -> &'static Mutex<HashMap<String, HostScheme>> {
    static DECISIONS: std::sync::OnceLock<Mutex<HashMap<String, HostScheme>>> =
        std::sync::OnceLock::new();
    DECISIONS.get_or_init(Default::default)
}

/// Host of a remote `http` or `https` page. The app origin serves `ce://` pages, and other
/// schemes have no site decision.
fn page_host(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" || is_app_origin(&parsed) {
        return None;
    }
    let host = parsed
        .host_str()?
        .trim_end_matches('.')
        .to_ascii_lowercase();
    (!host.is_empty()).then_some(host)
}

/// Vite in dev (`localhost:1420`) and `tauri.localhost` in a build. Either scheme is the
/// same origin: HTTPS-only mode used to upgrade the build host.
fn is_app_origin(url: &url::Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    host == "tauri.localhost" || (host == "localhost" && url.port() == Some(1420))
}

fn host_decision(url: &str) -> Option<HostScheme> {
    let host = page_host(url)?;
    host_decisions().lock().ok()?.get(&host).copied()
}

fn remember_host(url: &str, scheme: HostScheme) {
    let Some(host) = page_host(url) else {
        return;
    };
    if let Ok(mut decisions) = host_decisions().lock() {
        decisions.insert(host, scheme);
    }
}

/// A remote page keeps a white backdrop while Windows is dark, until its host declares that
/// it can be rendered dark. Remembered dark hosts, and every page while Windows is light, use
/// the window canvas. App pages are not remote hosts.
fn wants_light_scheme(windows_dark: bool, known: Option<HostScheme>, url: &str) -> bool {
    windows_dark && page_host(url).is_some() && known != Some(HostScheme::Dark)
}

/// White canvas for this URL. The gap around a remote page uses it.
pub(crate) fn page_keeps_light_scheme(app: &AppHandle, url: &str) -> bool {
    wants_light_scheme(
        !crate::settings::windows_uses_light(app),
        host_decision(url),
        url,
    )
}

/// `selectors` come from rules whose `color-scheme` value already contains `dark`.
fn declares_dark_scheme<'a>(meta: &str, selectors: impl IntoIterator<Item = &'a str>) -> bool {
    scheme_has_dark_token(meta)
        || selectors
            .into_iter()
            .any(|selector| canvas_selector(selector))
}

fn scheme_has_dark_token(value: &str) -> bool {
    value
        .split_whitespace()
        .any(|token| token.eq_ignore_ascii_case("dark"))
}

/// `html`, `body`, or `:root`, including one item of a selector list. A class or attribute
/// on that element is the site's own theme toggle, not a statement that the page supports
/// the user's dark scheme.
fn canvas_selector(selector: &str) -> bool {
    selector.split(',').any(|part| {
        let part = part.trim();
        part.eq_ignore_ascii_case("html")
            || part.eq_ignore_ascii_case("body")
            || part.eq_ignore_ascii_case(":root")
    })
}

const SCHEME_PROBE_SCRIPT: &str = r#"(function () {
  var meta = "";
  var tags = document.getElementsByTagName("meta");
  for (var i = 0; i < tags.length; i++) {
    var name = tags[i].getAttribute("name") || "";
    if (name.toLowerCase() === "color-scheme") {
      meta = tags[i].getAttribute("content") || "";
      break;
    }
  }
  var selectors = [];
  function darkValue(value) {
    return /(^|\s)dark(\s|$)/i.test(value || "");
  }
  function inline(el, name) {
    if (!el || !el.style) return;
    if (darkValue(el.style.getPropertyValue("color-scheme"))) selectors.push(name);
  }
  inline(document.documentElement, "html");
  inline(document.body, "body");
  function visit(rules) {
    if (!rules || selectors.length > 64) return;
    for (var i = 0; i < rules.length; i++) {
      var rule = rules[i];
      if (rule.selectorText && rule.style) {
        var value = "";
        try { value = rule.style.getPropertyValue("color-scheme"); } catch (e) {}
        if (darkValue(value)) selectors.push(rule.selectorText);
      }
      if (rule.styleSheet) {
        try { visit(rule.styleSheet.cssRules); } catch (e) {}
      } else if (rule.cssRules) {
        visit(rule.cssRules);
      }
      if (selectors.length > 64) return;
    }
  }
  function visitSheet(sheet) {
    if (!sheet) return;
    try { visit(sheet.cssRules); } catch (e) {}
  }
  var sheets = document.styleSheets;
  for (var s = 0; s < sheets.length; s++) visitSheet(sheets[s]);
  if (document.adoptedStyleSheets) {
    for (var a = 0; a < document.adoptedStyleSheets.length; a++) visitSheet(document.adoptedStyleSheets[a]);
  }
  return { meta: meta, selectors: selectors };
})()"#;

/// Reads whether this document declares a dark canvas. A host is recorded once per session.
/// A dark host repaints the backdrop. The page keeps the Windows color scheme.
fn probe_page_scheme(app: &AppHandle, id: u32, core: &ICoreWebView2) {
    let url = tabs::tab_page_url(app, id).unwrap_or_default();
    if page_host(&url).is_none() || host_decision(&url).is_some() {
        return;
    }
    let app = app.clone();
    let probed = url;
    let handler = ExecuteScriptCompletedHandler::create(Box::new(move |result, value| {
        if result.is_ok() {
            settle_scheme_probe(&app, id, &probed, &value);
        }
        Ok(())
    }));
    let script = HSTRING::from(SCHEME_PROBE_SCRIPT);
    if let Err(error) = unsafe { core.ExecuteScript(&script, &handler) } {
        eprintln!("WebView2 color scheme: could not read the page scheme: {error}");
    }
}

fn settle_scheme_probe(app: &AppHandle, id: u32, probed: &str, raw: &str) {
    let Some((meta, selectors)) = parse_scheme_probe(raw) else {
        return;
    };
    let current = tabs::tab_page_url(app, id).unwrap_or_default();
    if page_host(&current) != page_host(probed) || host_decision(&current).is_some() {
        return;
    }
    let dark = declares_dark_scheme(&meta, selectors.iter().map(String::as_str));
    remember_host(
        &current,
        if dark {
            HostScheme::Dark
        } else {
            HostScheme::Light
        },
    );
    if !dark {
        return;
    }
    let app = app.clone();
    let url = current;
    let queued = app.clone().run_on_main_thread(move || {
        tabs::paint_document_background(&app, id, &url);
    });
    if let Err(error) = queued {
        eprintln!("WebView2 color scheme: could not paint the page backdrop: {error}");
    }
}

fn parse_scheme_probe(raw: &str) -> Option<(String, Vec<String>)> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let value = match value {
        serde_json::Value::String(inner) => serde_json::from_str(&inner).ok()?,
        other => other,
    };
    let meta = value.get("meta")?.as_str()?.to_string();
    let selectors = value
        .get("selectors")?
        .as_array()?
        .iter()
        .filter_map(|item| item.as_str().map(str::to_string))
        .collect();
    Some((meta, selectors))
}

pub fn attach_ui_hooks(app: &AppHandle, webview: &Webview, window: &str) {
    let app = app.clone();
    let window = window.to_string();
    let _ = webview.with_webview(move |platform| {
        attach_zoom_wheel(&platform, app.clone(), ZoomKind::Ui(window));
        if let Err(error) = attach_accelerators(&app, &platform.controller(), Target::Ui) {
            eprintln!("WebView2 shortcuts for the browser UI: {error}");
        }
        if let Ok(core) = core(&platform) {
            pin_profile_scheme(&core);
            let failed_app = app.clone();
            let mut token = 0i64;
            let handler = ProcessFailedEventHandler::create(Box::new(move |sender, args| {
                let Some(args) = args else {
                    return Ok(());
                };
                let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND::default();
                unsafe { args.ProcessFailedKind(&mut kind)? };
                if kind == COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED {
                    let app = failed_app.clone();
                    tauri::async_runtime::spawn(async move {
                        crate::page_errors::browser_process_exited(&app)
                    });
                } else if kind == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED
                    && let Some(core) = sender
                {
                    let _ = unsafe { core.Reload() };
                }
                Ok(())
            }));
            if let Err(error) = unsafe { core.add_ProcessFailed(&handler, &mut token) } {
                eprintln!("WebView2 crash events for the browser UI: {error}");
            }
        }
    });
}

/// Ctrl+wheel is handled here. With zoom control disabled, WebView2 consumes the gesture and
/// never delivers a DOM wheel event, so the page script and the header never see it.
/// The wheel often lands on a WebView2 window this process cannot subclass, or on the chrome
/// webview that sits under a hit-test-transparent page. A low-level hook sees the notch first
/// and chooses the target from the cursor.
const ZOOM_WHEEL_SUBCLASS: usize = 0x00CE_5A11;
const ZOOM_SINK_SUBCLASS: usize = 0x00CE_5A12;
const WM_ZOOM_SUBCLASS: u32 = WM_APP + 0x5A;
const WM_ZOOM_WHEEL: u32 = WM_APP + 0x5B;
const WHEEL_NOTCH: i32 = 120;

#[derive(Clone)]
enum ZoomKind {
    Page(u32),
    /// Browser window whose header band changes Browser UI size.
    Ui(String),
}

#[derive(Clone)]
struct ZoomHook {
    app: AppHandle,
    kind: ZoomKind,
    /// WRY container. Its client area matches the webview.
    host: isize,
    /// Top-level browser window that owns this webview.
    root: isize,
}

struct QueuedZoom {
    app: AppHandle,
    root: isize,
    /// `0` changes Browser UI size. Any other value is a tab id.
    id: u32,
    steps: i32,
}

struct ZoomHit {
    app: AppHandle,
    /// Bucket for partial wheel notches.
    bucket: isize,
    root: isize,
    id: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WheelBand {
    Header,
    Page,
    Other,
}

/// `top`/`left`/`right`/`bottom` are the chrome insets in CSS pixels.
fn wheel_band(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    top: f64,
    left: f64,
    right: f64,
    bottom: f64,
) -> WheelBand {
    if x < 0.0 || y < 0.0 || x >= width || y >= height {
        return WheelBand::Other;
    }
    if top > 0.0 && y < top {
        return WheelBand::Header;
    }
    if x >= left && y >= top && x < width - right && y < height - bottom {
        return WheelBand::Page;
    }
    WheelBand::Other
}

fn zoom_hooks() -> &'static Mutex<HashMap<isize, ZoomHook>> {
    static HOOKS: std::sync::OnceLock<Mutex<HashMap<isize, ZoomHook>>> = std::sync::OnceLock::new();
    HOOKS.get_or_init(Default::default)
}

fn zoom_buckets() -> &'static Mutex<HashMap<isize, i32>> {
    static BUCKETS: std::sync::OnceLock<Mutex<HashMap<isize, i32>>> = std::sync::OnceLock::new();
    BUCKETS.get_or_init(Default::default)
}

/// One wheel notch is 120. Smaller trackpad deltas add up, and the leftover stays for the next one.
fn wheel_steps(bucket: i32, delta: i32) -> (i32, i32) {
    let next = bucket.saturating_add(delta);
    let steps = next / WHEEL_NOTCH;
    (next - steps * WHEEL_NOTCH, steps)
}

fn zoom_queue() -> &'static Mutex<Vec<QueuedZoom>> {
    static QUEUE: std::sync::OnceLock<Mutex<Vec<QueuedZoom>>> = std::sync::OnceLock::new();
    QUEUE.get_or_init(Default::default)
}

fn zoom_sinks() -> &'static Mutex<HashSet<isize>> {
    static SINKS: std::sync::OnceLock<Mutex<HashSet<isize>>> = std::sync::OnceLock::new();
    SINKS.get_or_init(Default::default)
}

fn zoom_hook_ready() -> &'static AtomicBool {
    static READY: AtomicBool = AtomicBool::new(false);
    &READY
}

fn hwnd_of(value: isize) -> HWND {
    HWND(value as *mut std::ffi::c_void)
}

fn attach_zoom_wheel(platform: &PlatformWebview, app: AppHandle, kind: ZoomKind) {
    let mut hwnd = HWND(std::ptr::null_mut());
    if unsafe { platform.controller().ParentWindow(&mut hwnd) }.is_err() || hwnd.0.is_null() {
        return;
    }
    let host = hwnd.0 as isize;
    let root_hwnd = unsafe { GetAncestor(hwnd, GA_ROOT) };
    let root = if root_hwnd.0.is_null() {
        host
    } else {
        root_hwnd.0 as isize
    };
    raise_zoom_subclass(
        hwnd,
        host,
        ZoomHook {
            app,
            kind,
            host,
            root,
        },
    );
    let _ = unsafe { EnumChildWindows(Some(hwnd), Some(subclass_zoom_child), LPARAM(host)) };
    let _ = ensure_zoom_sink(root);
    install_zoom_hook();
}

fn raise_zoom_subclass(hwnd: HWND, host: isize, hook: ZoomHook) {
    if hwnd.0.is_null() {
        return;
    }
    // The hook is recorded even when the subclass cannot be installed. Hit testing only
    // needs the container, and WebView2's render window often belongs to another process.
    zoom_hooks()
        .lock()
        .expect("zoom wheel")
        .insert(hwnd.0 as isize, hook);
    // Remove and add so this proc runs before WebView2's own subclass, which swallows Ctrl+wheel.
    unsafe {
        let _ = RemoveWindowSubclass(hwnd, Some(zoom_wheel_proc), ZOOM_WHEEL_SUBCLASS);
        let _ = SetWindowSubclass(
            hwnd,
            Some(zoom_wheel_proc),
            ZOOM_WHEEL_SUBCLASS,
            host as usize,
        );
    }
}

unsafe extern "system" fn subclass_zoom_child(child: HWND, host: LPARAM) -> BOOL {
    let hook = zoom_hooks()
        .lock()
        .expect("zoom wheel")
        .get(&host.0)
        .cloned();
    if let Some(hook) = hook {
        raise_zoom_subclass(child, host.0, hook);
    }
    BOOL(1)
}

fn zoom_hook(host: isize) -> Option<ZoomHook> {
    zoom_hooks().lock().expect("zoom wheel").get(&host).cloned()
}

fn push_wheel(host: isize, delta: i32) -> i32 {
    let mut buckets = zoom_buckets().lock().expect("zoom wheel");
    let bucket = buckets.entry(host).or_insert(0);
    let (rest, steps) = wheel_steps(*bucket, delta);
    *bucket = rest;
    steps
}

fn clear_wheel(host: isize) {
    if let Some(bucket) = zoom_buckets().lock().expect("zoom wheel").get_mut(&host) {
        *bucket = 0;
    }
}

fn ctrl_down(wparam: WPARAM) -> bool {
    const MK_CONTROL: usize = 0x0008;
    (wparam.0 & 0xFFFF & MK_CONTROL) != 0 || key_down(VK_CONTROL)
}

fn wheel_delta(wparam: WPARAM) -> i32 {
    ((wparam.0 >> 16) as u16) as i16 as i32
}

fn wheel_cursor(lparam: LPARAM) -> POINT {
    let packed = lparam.0 as u32;
    POINT {
        x: (packed & 0xFFFF) as i16 as i32,
        y: ((packed >> 16) & 0xFFFF) as i16 as i32,
    }
}

/// Screen point converted into the webview's CSS pixel space, plus the client size.
fn client_css(host: HWND, point: POINT) -> Option<(f64, f64, f64, f64)> {
    let mut point = point;
    if !unsafe { ScreenToClient(host, &mut point) }.as_bool() {
        return None;
    }
    let mut rect = RECT::default();
    if unsafe { GetClientRect(host, &mut rect) }.is_err() {
        return None;
    }
    let scale = hwnd_scale(host);
    if scale <= f64::EPSILON {
        return None;
    }
    Some((
        point.x as f64 / scale,
        point.y as f64 / scale,
        rect.right as f64 / scale,
        rect.bottom as f64 / scale,
    ))
}

fn hit_page(hook: ZoomHook, id: u32) -> ZoomHit {
    ZoomHit {
        app: hook.app,
        bucket: hook.host,
        root: hook.root,
        id,
    }
}

fn hit_ui(hook: ZoomHook) -> ZoomHit {
    ZoomHit {
        app: hook.app,
        bucket: hook.host,
        root: hook.root,
        id: 0,
    }
}

fn ancestor_host(point: POINT) -> Option<ZoomHook> {
    let mut hwnd = unsafe { WindowFromPoint(point) };
    for _ in 0..32 {
        if hwnd.0.is_null() {
            return None;
        }
        if let Some(hook) = zoom_hook(hwnd.0 as isize) {
            return Some(hook);
        }
        let parent = unsafe { GetAncestor(hwnd, GA_PARENT) };
        if parent.0.is_null() || parent == hwnd {
            return None;
        }
        hwnd = parent;
    }
    None
}

fn over_browser(point: POINT) -> bool {
    let hit = unsafe { WindowFromPoint(point) };
    if hit.0.is_null() {
        return false;
    }
    let root = unsafe { GetAncestor(hit, GA_ROOT) };
    if root.0.is_null() {
        return false;
    }
    let root = root.0 as isize;
    zoom_hooks()
        .lock()
        .ok()
        .is_some_and(|hooks| hooks.values().any(|hook| hook.root == root))
}

/// The container under `point`. `page` selects tab webviews; otherwise the browser UI webview.
fn host_containing(point: POINT, page: bool) -> Option<ZoomHook> {
    let candidates = {
        let hooks = zoom_hooks().lock().ok()?;
        hooks
            .iter()
            .filter(|(key, hook)| {
                **key == hook.host
                    && match hook.kind {
                        ZoomKind::Page(_) => page,
                        ZoomKind::Ui(_) => !page,
                    }
            })
            .map(|(key, hook)| (*key, hook.clone()))
            .collect::<Vec<_>>()
    };
    let mut best: Option<(i32, ZoomHook)> = None;
    for (key, hook) in candidates {
        let hwnd = hwnd_of(key);
        if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
            continue;
        }
        let mut rect = RECT::default();
        if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
            continue;
        }
        if point.x < rect.left
            || point.x >= rect.right
            || point.y < rect.top
            || point.y >= rect.bottom
        {
            continue;
        }
        let area = (rect.right - rect.left).saturating_mul(rect.bottom - rect.top);
        if area <= 0 {
            continue;
        }
        if best.as_ref().is_none_or(|(best_area, _)| area < *best_area) {
            best = Some((area, hook));
        }
    }
    best.map(|(_, hook)| hook)
}

fn page_containing(point: POINT) -> Option<ZoomHit> {
    let hook = host_containing(point, true)?;
    let ZoomKind::Page(id) = hook.kind else {
        return None;
    };
    Some(hit_page(hook, id))
}

fn classify_ui(point: POINT, hook: ZoomHook) -> Option<ZoomHit> {
    // The page view covers this part of the chrome. Hit testing can still land on the UI
    // webview when the page window does not take the pointer.
    if let Some(page) = page_containing(point) {
        return Some(page);
    }
    let window = match &hook.kind {
        ZoomKind::Ui(window) => window.clone(),
        ZoomKind::Page(_) => return None,
    };
    let (top, left, right, bottom) = tabs::page_insets(&hook.app, &window)?;
    let (x, y, width, height) = client_css(hwnd_of(hook.host), point)?;
    match wheel_band(x, y, width, height, top, left, right, bottom) {
        WheelBand::Header => Some(hit_ui(hook)),
        WheelBand::Page => {
            let id = tabs::active_in(&hook.app, &window)?;
            Some(hit_page(hook, id))
        }
        WheelBand::Other => None,
    }
}

/// A page whose window contains the pointer, including a render child owned by WebView2.
fn page_at_pointer(point: POINT) -> Option<ZoomHit> {
    let hook = host_containing(point, true)?;
    let ZoomKind::Page(id) = hook.kind else {
        return None;
    };
    let hit = unsafe { WindowFromPoint(point) };
    let host = hwnd_of(hook.host);
    let on_page = hit == host || (!hit.0.is_null() && unsafe { IsChild(host, hit).as_bool() });
    if on_page || over_browser(point) {
        Some(hit_page(hook, id))
    } else {
        None
    }
}

pub enum HoveredView {
    /// Browser chrome: header, tab strip, sidebars, tool strip.
    Chrome(String),
    /// The tab page under the pointer.
    Page(u32),
}

/// What the pointer is over. Built-in pages are `Page`; the caller tells them apart by address.
pub fn hovered_view() -> Option<HoveredView> {
    let mut point = POINT::default();
    if unsafe { GetCursorPos(&mut point) }.is_err() {
        return None;
    }
    if let Some(hook) = ancestor_host(point) {
        return match &hook.kind {
            ZoomKind::Page(id) => Some(HoveredView::Page(*id)),
            ZoomKind::Ui(window) => Some(inspect_ui(point, window)),
        };
    }
    if let Some(page) = page_at_pointer(point)
        && page.id != 0
    {
        return Some(HoveredView::Page(page.id));
    }
    None
}

fn inspect_ui(point: POINT, window: &str) -> HoveredView {
    if let Some(page) = page_containing(point)
        && page.id != 0
    {
        return HoveredView::Page(page.id);
    }
    HoveredView::Chrome(window.to_string())
}

/// Page under the pointer, or the header when the pointer is on the browser chrome.
fn classify(point: POINT) -> Option<ZoomHit> {
    if let Some(hook) = ancestor_host(point) {
        return match hook.kind {
            ZoomKind::Page(id) => Some(hit_page(hook, id)),
            ZoomKind::Ui(_) => classify_ui(point, hook),
        };
    }
    if let Some(page) = page_at_pointer(point) {
        return Some(page);
    }
    if !over_browser(point) {
        return None;
    }
    let hook = host_containing(point, false)?;
    classify_ui(point, hook)
}

/// `true` when this notch belongs to a page or the browser header and WebView2 must not see it.
fn offer_wheel(point: POINT, delta: i32, ctrl: bool) -> bool {
    let Some(hit) = classify(point) else {
        return false;
    };
    if !ctrl {
        clear_wheel(hit.bucket);
        return false;
    }
    // A site can turn page zoom off for the domain or for one address. The notch is still
    // taken, so WebView2 does not zoom the page on its own.
    if hit.id != 0
        && let Some(url) = tabs::try_tab_url(&hit.app, hit.id)
        && crate::page_rules::try_page_zoom_disabled(&hit.app, &url)
    {
        clear_wheel(hit.bucket);
        return true;
    }
    let steps = push_wheel(hit.bucket, delta);
    enqueue_zoom(QueuedZoom {
        app: hit.app,
        root: hit.root,
        id: hit.id,
        steps,
    });
    true
}

fn enqueue_zoom(item: QueuedZoom) {
    if item.steps == 0 {
        return;
    }
    let root = item.root;
    zoom_queue().lock().expect("zoom queue").push(item);
    if !ensure_zoom_sink(root) || !post_zoom(root) {
        drain_zoom();
    }
}

fn post_zoom(root: isize) -> bool {
    unsafe { PostMessageW(Some(hwnd_of(root)), WM_ZOOM_WHEEL, WPARAM(0), LPARAM(0)).is_ok() }
}

fn drain_zoom() {
    let batch = {
        let mut queue = zoom_queue().lock().expect("zoom queue");
        std::mem::take(&mut *queue)
    };
    for item in batch {
        if item.id == 0 {
            step_ui_zoom(&item.app, item.steps);
        } else {
            step_page_zoom(&item.app, item.id, item.steps);
        }
    }
}

fn ensure_zoom_sink(root: isize) -> bool {
    if root == 0 {
        return false;
    }
    let mut sinks = zoom_sinks().lock().expect("zoom sink");
    if sinks.contains(&root) {
        return true;
    }
    let installed = unsafe {
        SetWindowSubclass(
            hwnd_of(root),
            Some(zoom_sink_proc),
            ZOOM_SINK_SUBCLASS,
            root as usize,
        )
        .as_bool()
    };
    if installed {
        sinks.insert(root);
    }
    installed
}

unsafe extern "system" fn zoom_sink_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _root: usize,
) -> LRESULT {
    if message == WM_ZOOM_WHEEL {
        drain_zoom();
        return LRESULT(0);
    }
    if message == WM_NCDESTROY {
        if let Ok(mut sinks) = zoom_sinks().lock() {
            sinks.remove(&(hwnd.0 as isize));
        }
        let _ = unsafe { RemoveWindowSubclass(hwnd, Some(zoom_sink_proc), ZOOM_SINK_SUBCLASS) };
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

fn install_zoom_hook() {
    static STARTED: AtomicBool = AtomicBool::new(false);
    if zoom_hook_ready().load(Ordering::Acquire) || STARTED.swap(true, Ordering::AcqRel) {
        return;
    }
    match unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(zoom_ll_proc), None, 0) } {
        Ok(_hook) => {
            // `HHOOK` is `Copy` and is not unhooked on drop. The hook stays for the process.
            zoom_hook_ready().store(true, Ordering::Release);
        }
        Err(error) => {
            STARTED.store(false, Ordering::Release);
            eprintln!("Ctrl+scroll zoom: {error}");
        }
    }
}

fn browser_app() -> Option<AppHandle> {
    zoom_hooks()
        .lock()
        .ok()?
        .values()
        .next()
        .map(|hook| hook.app.clone())
}

unsafe extern "system" fn zoom_ll_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && lparam.0 != 0 {
        let message = wparam.0 as u32;
        let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        if message == WM_MOUSEWHEEL {
            let delta = ((info.mouseData >> 16) as u16) as i16 as i32;
            if offer_wheel(info.pt, delta, async_key(VK_CONTROL)) {
                return LRESULT(1);
            }
        } else if message == WM_LBUTTONDOWN
            || message == WM_RBUTTONDOWN
            || message == WM_MBUTTONDOWN
        {
            // A downloads panel opened without focus never blurs when the page is clicked.
            // The click is still delivered; this only decides whether that panel should close.
            if let Some(app) = browser_app() {
                popup::dismiss_downloads_outside(&app, info.pt.x, info.pt.y);
                popup::dismiss_held_outside(&app, info.pt.x, info.pt.y);
            }
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn step_page_zoom(app: &AppHandle, id: u32, steps: i32) {
    if steps == 0 {
        return;
    }
    let change = if steps > 0 { "in" } else { "out" };
    for _ in 0..steps.unsigned_abs() {
        if tabs::zoom(app, id, change).is_err() {
            break;
        }
    }
}

fn step_ui_zoom(app: &AppHandle, steps: i32) {
    if steps == 0 {
        return;
    }
    let current = crate::settings::get(app).ui_zoom as i32;
    let next = (current + steps * 5).clamp(80, 150) as u32;
    if next == current as u32 {
        return;
    }
    let _ = crate::settings::update(app, |settings| settings.ui_zoom = next);
}

unsafe extern "system" fn zoom_wheel_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    host: usize,
) -> LRESULT {
    let host_key = host as isize;
    if message == WM_PARENTNOTIFY && (wparam.0 & 0xFFFF) as u32 == WM_CREATE {
        let child = HWND(lparam.0 as *mut std::ffi::c_void);
        if !child.0.is_null()
            && let Some(hook) = zoom_hook(host_key)
        {
            raise_zoom_subclass(child, host_key, hook);
            let _ = unsafe {
                PostMessageW(
                    Some(hwnd),
                    WM_ZOOM_SUBCLASS,
                    WPARAM(child.0 as usize),
                    LPARAM(0),
                )
            };
        }
    } else if message == WM_ZOOM_SUBCLASS {
        let child = HWND(wparam.0 as *mut std::ffi::c_void);
        if let Some(hook) = zoom_hook(host_key) {
            raise_zoom_subclass(child, host_key, hook);
        }
        return LRESULT(0);
    } else if message == WM_NCDESTROY {
        zoom_hooks()
            .lock()
            .expect("zoom wheel")
            .remove(&(hwnd.0 as isize));
        if hwnd.0 as isize == host_key {
            zoom_buckets().lock().expect("zoom wheel").remove(&host_key);
        }
        let _ = unsafe { RemoveWindowSubclass(hwnd, Some(zoom_wheel_proc), ZOOM_WHEEL_SUBCLASS) };
    } else if message == WM_MOUSEWHEEL && !zoom_hook_ready().load(Ordering::Acquire) {
        // The low-level hook is the path that runs before WebView2. This is only the fallback
        // for a window in this process when that hook could not be installed.
        if offer_wheel(wheel_cursor(lparam), wheel_delta(wparam), ctrl_down(wparam)) {
            return LRESULT(0);
        }
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

/// The WebView2 environment already created for this profile. A second
/// `CreateCoreWebView2EnvironmentWithOptions` for the same user-data folder
/// waits forever, which is what froze the app when another window opened.
pub type WebViewEnvironment = ICoreWebView2Environment;

pub fn environment_of(webview: &Webview) -> Option<ICoreWebView2Environment> {
    let (tx, rx) = mpsc::channel();
    let queued = webview.with_webview(move |platform| {
        let _ = tx.send(MainThread::new(platform.environment()));
    });
    if queued.is_err() {
        return None;
    }
    rx.recv().ok().map(MainThread::into_inner)
}

/// True while a popup is being activated, so its owner keeps drawing itself as active.
static POPUP_ACTIVATING: AtomicBool = AtomicBool::new(false);
const OWNER_SUBCLASS_ID: usize = 0xCE_0901;

/// Keeps a browser window painted active (title, border, Mica) while one of its own popups
/// takes focus. Call on the main thread, once per window.
pub fn keep_owner_active(owner: isize) {
    if owner == 0 {
        return;
    }
    let _ = unsafe {
        SetWindowSubclass(
            HWND(owner as *mut std::ffi::c_void),
            Some(owner_proc),
            OWNER_SUBCLASS_ID,
            0,
        )
    };
}

unsafe extern "system" fn owner_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    if message == WM_NCACTIVATE && wparam.0 == 0 && POPUP_ACTIVATING.load(Ordering::Acquire) {
        return unsafe { DefSubclassProc(hwnd, message, WPARAM(1), lparam) };
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

/// Gives a popup window the keyboard. Main thread only.
pub fn activate_popup(popup: isize) {
    if popup == 0 {
        return;
    }
    POPUP_ACTIVATING.store(true, Ordering::Release);
    let _ = unsafe { SetForegroundWindow(HWND(popup as *mut std::ffi::c_void)) };
    POPUP_ACTIVATING.store(false, Ordering::Release);
}

/// Focus left a popup for another program. Its owner was kept looking active, so it now
/// draws itself inactive. Main thread only.
pub fn release_owner(owner: isize) {
    if owner == 0 || foreground_window() == owner {
        return;
    }
    unsafe {
        SendMessageW(
            HWND(owner as *mut std::ffi::c_void),
            WM_NCACTIVATE,
            Some(WPARAM(0)),
            Some(LPARAM(0)),
        );
    }
}

pub fn foreground_window() -> isize {
    unsafe { GetForegroundWindow() }.0 as isize
}

/// True when `x`, `y` (screen px) lie in the window's client area. The shadow frame is outside it.
pub fn client_contains_screen_point(hwnd: isize, x: i32, y: i32) -> bool {
    if hwnd == 0 {
        return false;
    }
    let hwnd = HWND(hwnd as *mut std::ffi::c_void);
    let mut origin = POINT::default();
    let mut rect = RECT::default();
    let measured = unsafe {
        ClientToScreen(hwnd, &mut origin).as_bool() && GetClientRect(hwnd, &mut rect).is_ok()
    };
    if !measured {
        return false;
    }
    x >= origin.x
        && y >= origin.y
        && x < origin.x.saturating_add(rect.right)
        && y < origin.y.saturating_add(rect.bottom)
}

/// The top-level window that owns the foreground window, so a focused popup gives its
/// browser window. 0 when nothing has focus.
pub fn foreground_root_owner() -> isize {
    let foreground = unsafe { GetForegroundWindow() };
    if foreground.0.is_null() {
        return 0;
    }
    unsafe { GetAncestor(foreground, GA_ROOTOWNER) }.0 as isize
}

/// Window rect for a client area at `x`, `y` sized `width` by `height`. A borderless window
/// with a shadow keeps an invisible frame, so its window rect is larger than its client area.
unsafe fn frame_around_client(
    hwnd: HWND,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> (i32, i32, i32, i32) {
    let mut window = RECT::default();
    let mut client = RECT::default();
    let mut origin = POINT::default();
    let measured = unsafe {
        GetWindowRect(hwnd, &mut window).is_ok()
            && GetClientRect(hwnd, &mut client).is_ok()
            && ClientToScreen(hwnd, &mut origin).as_bool()
    };
    if !measured {
        return (x, y, width, height);
    }
    let left = origin.x - window.left;
    let top = origin.y - window.top;
    let extra_width = (window.right - window.left) - (client.right - client.left);
    let extra_height = (window.bottom - window.top) - (client.bottom - client.top);
    (
        x - left,
        y - top,
        width + extra_width,
        height + extra_height,
    )
}

/// Moves and sizes a popup window's client area in one step, above its siblings. Physical
/// px. A `no_activate` popup never takes focus, even when it is clicked. Main thread only.
pub fn place_popup_window(
    popup: isize,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    no_activate: bool,
) {
    if popup == 0 {
        return;
    }
    let hwnd = HWND(popup as *mut std::ffi::c_void);
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let flag = WS_EX_NOACTIVATE.0 as isize;
        let next = if no_activate {
            style | flag
        } else {
            style & !flag
        };
        if next != style {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next);
        }
        let (x, y, width, height) = frame_around_client(hwnd, x, y, width, height);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOP),
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}

/// Shows a popup window that was built hidden, already moved off screen, so it never
/// appears at its default position. Main thread only.
pub fn show_parked_popup(popup: isize, x: i32, y: i32, width: i32, height: i32) {
    if popup == 0 {
        return;
    }
    let hwnd = HWND(popup as *mut std::ffi::c_void);
    unsafe {
        let (x, y, width, height) = frame_around_client(hwnd, x, y, width, height);
        let _ = SetWindowPos(
            hwnd,
            None,
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE | SWP_NOZORDER | SWP_SHOWWINDOW,
        );
    }
}

/// Moves a popup window off screen. A popup that had the keyboard hands it back to its owner.
/// Main thread only.
pub fn park_popup_window(popup: isize, owner: isize, x: i32, y: i32, width: i32, height: i32) {
    if popup == 0 {
        return;
    }
    let hwnd = HWND(popup as *mut std::ffi::c_void);
    unsafe {
        if foreground_window() == popup && owner != 0 {
            let _ = SetForegroundWindow(HWND(owner as *mut std::ffi::c_void));
        }
        let (x, y, width, height) = frame_around_client(hwnd, x, y, width, height);
        let _ = SetWindowPos(
            hwnd,
            None,
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE | SWP_NOZORDER,
        );
    }
}

/// Clips a child webview to a rounded rectangle. `width` and `height` are the size just
/// requested. The HWND may still be the previous size, because its move is posted and not
/// applied yet, so the region is built from this size rather than from `GetClientRect`. When
/// `logical` is set, the size and radius are CSS pixels and are scaled by the webview's DPI.
/// Otherwise they are already physical pixels. The main window is left alone.
pub fn round_popup(
    webview: &Webview,
    main: isize,
    width: f64,
    height: f64,
    radius: f64,
    logical: bool,
) {
    if width < 8.0 || height < 8.0 {
        return;
    }
    let _ = webview.with_webview(move |platform| {
        let main = HWND(main as *mut std::ffi::c_void);
        let mut hwnd = HWND(std::ptr::null_mut());
        if unsafe {
            platform
                .controller()
                .ParentWindow(std::ptr::addr_of_mut!(hwnd))
        }
        .is_err()
        {
            return;
        }
        if hwnd.0.is_null() || hwnd == main {
            return;
        }
        let scale = if logical { hwnd_scale(hwnd) } else { 1.0 };
        let width = (width * scale).round().max(1.0) as i32;
        let height = (height * scale).round().max(1.0) as i32;
        let radius = (radius.max(1.0) * scale).round().max(1.0) as i32;
        apply_region(hwnd, radius, width, height);
    });
}

/// Reads the document's scroll size. The callback runs on the UI thread once the script finishes.
pub fn measure_document(webview: &Webview, done: impl FnOnce(f64, f64) + Send + 'static) {
    let slot = Arc::new(Mutex::new(Some(done)));
    let _ = webview.with_webview(move |platform| {
        let Ok(core) = core(&platform) else {
            return;
        };
        let slot = slot.clone();
        let handler = ExecuteScriptCompletedHandler::create(Box::new(move |result, value| {
            if result.is_ok()
                && let Some(done) = slot.lock().ok().and_then(|mut guard| guard.take())
                && let Some(size) = parse_document_size(&value)
            {
                done(size.0, size.1);
            }
            Ok(())
        }));
        let script = HSTRING::from(DOCUMENT_SIZE_SCRIPT);
        let _ = unsafe { core.ExecuteScript(&script, &handler) };
    });
}

const DOCUMENT_SIZE_SCRIPT: &str = r#"(function () {
  var root = document.documentElement;
  var body = document.body;
  function box(el) {
    if (!el) return [0, 0];
    return [
      Math.max(el.scrollWidth || 0, el.offsetWidth || 0),
      Math.max(el.scrollHeight || 0, el.offsetHeight || 0)
    ];
  }
  var a = box(root);
  var b = box(body);
  return { w: Math.ceil(Math.max(a[0], b[0])), h: Math.ceil(Math.max(a[1], b[1])) };
})()"#;

fn parse_document_size(raw: &str) -> Option<(f64, f64)> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let value = match value {
        serde_json::Value::String(inner) => serde_json::from_str(&inner).ok()?,
        other => other,
    };
    let width = value.get("w")?.as_f64()?;
    let height = value.get("h")?.as_f64()?;
    (width.is_finite() && height.is_finite() && width >= 1.0 && height >= 1.0)
        .then_some((width, height))
}

/// The popup HWND's DPI, as a scale from 96. This is the factor wry uses when it turns a
/// logical size into physical pixels.
fn hwnd_scale(hwnd: HWND) -> f64 {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    if dpi == 0 { 1.0 } else { dpi as f64 / 96.0 }
}

struct Clip {
    radius: i32,
    /// Client size a child must be close to before it is clipped. The WebView2 child matches
    /// the popup window; smaller children are left alone.
    match_width: i32,
    match_height: i32,
    draw_width: i32,
    draw_height: i32,
}

fn apply_region(hwnd: HWND, radius: i32, width: i32, height: i32) {
    clip_round(hwnd, radius, width, height);
    let Some((match_width, match_height)) = client_size(hwnd) else {
        return;
    };
    let clip = Clip {
        radius,
        match_width,
        match_height,
        draw_width: width,
        draw_height: height,
    };
    let _ = unsafe {
        EnumChildWindows(
            Some(hwnd),
            Some(clip_matching_child),
            LPARAM(std::ptr::addr_of!(clip) as isize),
        )
    };
}

unsafe extern "system" fn clip_matching_child(child: HWND, param: LPARAM) -> BOOL {
    let clip = unsafe { &*(param.0 as *const Clip) };
    if let Some((width, height)) = client_size(child) {
        let near_window =
            (width - clip.match_width).abs() <= 8 && (height - clip.match_height).abs() <= 8;
        let near_target =
            (width - clip.draw_width).abs() <= 8 && (height - clip.draw_height).abs() <= 8;
        if near_window || near_target {
            clip_round(child, clip.radius, clip.draw_width, clip.draw_height);
        }
    }
    BOOL(1)
}

fn client_size(hwnd: HWND) -> Option<(i32, i32)> {
    let mut rect = RECT::default();
    if unsafe { GetClientRect(hwnd, &mut rect) }.is_err() {
        return None;
    }
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if width < 8 || height < 8 {
        None
    } else {
        Some((width, height))
    }
}

fn clip_round(hwnd: HWND, radius: i32, width: i32, height: i32) {
    if width < 8 || height < 8 {
        return;
    }
    let diameter = radius.max(1).saturating_mul(2);
    let region = unsafe { CreateRoundRectRgn(0, 0, width + 1, height + 1, diameter, diameter) };
    if region.0.is_null() {
        return;
    }
    if unsafe { SetWindowRgn(hwnd, Some(region), true) } == 0 {
        let _ = unsafe { DeleteObject(HGDIOBJ(region.0)) };
    }
}

/// The docked agent panel takes the browser's shortcuts the way the browser UI does.
pub fn attach_panel_hooks(app: &AppHandle, webview: &Webview) {
    let app = app.clone();
    let _ = webview.with_webview(move |platform| {
        if let Err(error) = attach_accelerators(&app, &platform.controller(), Target::Ui) {
            eprintln!("WebView2 shortcuts for the agent panel: {error}");
        }
    });
}

pub fn attach_popup_hooks(app: &AppHandle, webview: &Webview, label: &str) {
    let app = app.clone();
    let label = label.to_string();
    let _ = webview.with_webview(move |platform| {
        let controller = platform.controller();
        if let Err(error) = attach_accelerators(&app, &controller, Target::Popup) {
            eprintln!("WebView2 shortcuts for {label}: {error}");
        }
        if let Ok(core) = core(&platform) {
            let _ = attach_downloads(&app, &core, 0);
        }
    });
}

/// Sizes the webview to its window. A popout created for `SetNewWindow` can keep a zero-size
/// view, so the frame stays the background color after the document has loaded.
pub fn fill_client_area(webview: &Webview) {
    let _ = webview.with_webview(|platform| {
        let controller = platform.controller();
        unsafe {
            let mut parent = HWND::default();
            if controller.ParentWindow(&mut parent).is_err() || parent.0.is_null() {
                return;
            }
            let host = GetAncestor(parent, GA_ROOT);
            let window = if host.0.is_null() { parent } else { host };
            let mut rect = RECT::default();
            if GetClientRect(window, &mut rect).is_err() {
                return;
            }
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            if width <= 0 || height <= 0 {
                return;
            }
            if parent.0 != window.0 {
                let _ = SetWindowPos(
                    parent,
                    None,
                    0,
                    0,
                    width,
                    height,
                    SWP_NOZORDER | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
            let _ = controller.SetBounds(RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: height,
            });
            let _ = controller.SetIsVisible(true);
            let _ = controller.NotifyParentWindowPositionChanged();
        }
    });
}

/// `window.close()` and `chrome.windows.remove` ask the webview to close. wry only destroys
/// the child HWND, which leaves the decorated frame. Close the whole window.
pub fn close_window_when_requested(app: &AppHandle, label: &str) {
    let Some(webview) = app.get_webview(label) else {
        return;
    };
    let app = app.clone();
    let label = label.to_string();
    let _ = webview.with_webview(move |platform| {
        let Ok(core) = core(&platform) else {
            return;
        };
        let app = app.clone();
        let label = label.clone();
        let mut token = 0i64;
        let _ = unsafe {
            core.add_WindowCloseRequested(
                &WindowCloseRequestedEventHandler::create(Box::new(move |_, _| {
                    let app = app.clone();
                    let label = label.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Some(window) = app.get_window(&label) {
                            let _ = window.close();
                        }
                    });
                    Ok(())
                })),
                &mut token,
            )
        };
    });
}

/// Closes the popup stack when focus leaves it. Attached once, for the life of the window.
pub fn attach_popup_blur(app: &AppHandle, webview: &Webview, label: &str) {
    let app = app.clone();
    let label = label.to_string();
    let _ = webview.with_webview(move |platform| {
        let blur_app = app.clone();
        let mut token = 0i64;
        let handler = FocusChangedEventHandler::create(Box::new(move |_, _| {
            // A file drag takes focus for its own message loop. Closing the popup then
            // would destroy this webview on the UI thread, under DoDragDrop.
            if downloads::dragging_file() {
                downloads::close_popup_when_drag_ends();
                return Ok(());
            }
            let app = blur_app.clone();
            let label = label.clone();
            tauri::async_runtime::spawn(async move { popup::lost_focus(&app, &label) });
            Ok(())
        }));
        let _ = unsafe { platform.controller().add_LostFocus(&handler, &mut token) };
    });
}

/// The dev debug HUD of browser window `window` lost focus. A popup it kept open closes
/// unless focus went back into that popup.
pub fn attach_debug_hud_blur(app: &AppHandle, webview: &Webview, window: &str) {
    let app = app.clone();
    let window = window.to_string();
    let _ = webview.with_webview(move |platform| {
        let mut token = 0i64;
        let handler = FocusChangedEventHandler::create(Box::new(move |_, _| {
            let app = app.clone();
            let window = window.clone();
            tauri::async_runtime::spawn(async move { popup::debug_hud_lost_focus(&app, &window) });
            Ok(())
        }));
        let _ = unsafe { platform.controller().add_LostFocus(&handler, &mut token) };
    });
}

pub fn attach_page_hooks(app: &AppHandle, webview: &Webview, id: u32) {
    let app = app.clone();
    let _ = webview.with_webview(move |platform| {
        if let Err(error) = page_hooks(&app, &platform, id) {
            eprintln!("WebView2 hooks for tab {id}: {error}");
        }
    });
}

/// The inspector is not a tab. Downloads and permission prompts stay off it, and the only
/// browser shortcut it takes is the one that closes the dock, so the tools keep their own keys.
pub fn attach_devtools_hooks(app: &AppHandle, webview: &Webview) {
    let app = app.clone();
    let _ = webview.with_webview(move |platform| {
        if let Err(error) = devtools_hooks(&app, &platform) {
            eprintln!("WebView2 hooks for developer tools: {error}");
        }
    });
}

fn devtools_hooks(app: &AppHandle, platform: &PlatformWebview) -> windows::core::Result<()> {
    let controller = platform.controller();
    let core = core(platform)?;
    let mut token = 0i64;
    let app = app.clone();
    unsafe {
        controller.add_AcceleratorKeyPressed(
            &AcceleratorKeyPressedEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else {
                    return Ok(());
                };
                let mut kind = COREWEBVIEW2_KEY_EVENT_KIND::default();
                args.KeyEventKind(&mut kind)?;
                if kind != COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN
                    && kind != COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN
                {
                    return Ok(());
                }
                let mut key = 0u32;
                args.VirtualKey(&mut key)?;
                let action = shortcuts::action_for(
                    key,
                    key_down(VK_CONTROL),
                    key_down(VK_SHIFT),
                    key_down(VK_MENU),
                    Target::Page,
                );
                if matches!(action, Some(shortcuts::Action::DevTools)) {
                    args.SetHandled(true)?;
                    shortcuts::dispatch(&app, shortcuts::Action::DevTools);
                }
                Ok(())
            })),
            &mut token,
        )?;
        core.add_PermissionRequested(
            &PermissionRequestedEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else {
                    return Ok(());
                };
                hide_default_permission_ui(&args);
                let mut kind = COREWEBVIEW2_PERMISSION_KIND::default();
                let _ = args.PermissionKind(&mut kind);
                let state = if kind == COREWEBVIEW2_PERMISSION_KIND_CLIPBOARD_READ {
                    COREWEBVIEW2_PERMISSION_STATE_ALLOW
                } else {
                    COREWEBVIEW2_PERMISSION_STATE_DENY
                };
                let _ = args.SetState(state);
                save_permission_choice(&args, false);
                Ok(())
            })),
            &mut token,
        )?;
        if let Ok(core4) = core.cast::<ICoreWebView2_4>() {
            core4.add_DownloadStarting(
                &DownloadStartingEventHandler::create(Box::new(move |_, args| {
                    if let Some(args) = args {
                        let _ = args.SetHandled(true);
                        let _ = args.SetCancel(true);
                    }
                    Ok(())
                })),
                &mut token,
            )?;
        }
    }
    Ok(())
}

fn page_hooks(app: &AppHandle, platform: &PlatformWebview, id: u32) -> windows::core::Result<()> {
    attach_zoom_wheel(platform, app.clone(), ZoomKind::Page(id));
    let controller = platform.controller();
    let core = core(platform)?;
    pin_profile_scheme(&core);
    attach_accelerators(app, &controller, Target::Page)?;
    let mut token = 0i64;

    unsafe {
        let history_app = app.clone();
        core.add_HistoryChanged(
            &HistoryChangedEventHandler::create(Box::new(move |sender, _| {
                if let Some(core) = sender {
                    let back = read_bool(|value| core.CanGoBack(value));
                    let forward = read_bool(|value| core.CanGoForward(value));
                    tabs::update_history(&history_app, id, back, forward);
                }
                Ok(())
            })),
            &mut token,
        )?;

        let source_app = app.clone();
        core.add_SourceChanged(
            &SourceChangedEventHandler::create(Box::new(move |sender, _| {
                if let Some(core) = sender {
                    let source = read_pwstr(|value| core.Source(value));
                    if !source.is_empty() {
                        tabs::update_url(&source_app, id, &source);
                    }
                }
                Ok(())
            })),
            &mut token,
        )?;

        if let Ok(settings) = core.Settings() {
            let _ = settings.SetIsStatusBarEnabled(false);
        }
        if let Ok(core12) = core.cast::<ICoreWebView2_12>() {
            let text_app = app.clone();
            core12.add_StatusBarTextChanged(
                &StatusBarTextChangedEventHandler::create(Box::new(move |sender, _| {
                    let Some(core) = sender.and_then(|core| core.cast::<ICoreWebView2_12>().ok())
                    else {
                        return Ok(());
                    };
                    let text = read_pwstr(|value| core.StatusBarText(value));
                    crate::status::emit_status_text(&text_app, id, text);
                    Ok(())
                })),
                &mut token,
            )?;
        }

        let fullscreen_app = app.clone();
        core.add_ContainsFullScreenElementChanged(
            &ContainsFullScreenElementChangedEventHandler::create(Box::new(move |sender, _| {
                if let Some(core) = sender {
                    let fullscreen = read_bool(|value| core.ContainsFullScreenElement(value));
                    tabs::set_fullscreen(&fullscreen_app, id, fullscreen);
                }
                Ok(())
            })),
            &mut token,
        )?;

        let close_app = app.clone();
        core.add_WindowCloseRequested(
            &WindowCloseRequestedEventHandler::create(Box::new(move |_, _| {
                let app = close_app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = tabs::close_tab(&app, id);
                });
                Ok(())
            })),
            &mut token,
        )?;

        if let Ok(core8) = core.cast::<ICoreWebView2_8>() {
            let audio_app = app.clone();
            core8.add_IsDocumentPlayingAudioChanged(
                &IsDocumentPlayingAudioChangedEventHandler::create(Box::new(move |sender, _| {
                    let audible = sender
                        .and_then(|core| core.cast::<ICoreWebView2_8>().ok())
                        .is_some_and(|core| read_bool(|value| core.IsDocumentPlayingAudio(value)));
                    let app = audio_app.clone();
                    tauri::async_runtime::spawn(
                        async move { tabs::update_audio(&app, id, audible) },
                    );
                    Ok(())
                })),
                &mut token,
            )?;
        }

        let focus_app = app.clone();
        controller.add_GotFocus(
            &FocusChangedEventHandler::create(Box::new(move |_, _| {
                let app = focus_app.clone();
                tauri::async_runtime::spawn(async move { tabs::page_focused(&app, id) });
                Ok(())
            })),
            &mut token,
        )?;

        let message_app = app.clone();
        core.add_WebMessageReceived(
            &WebMessageReceivedEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else {
                    return Ok(());
                };
                let json = read_pwstr(|value| args.WebMessageAsJson(value));
                let Ok(mut message) = serde_json::from_str::<serde_json::Value>(&json) else {
                    return Ok(());
                };
                // Remote pages hide `chrome.webview`, and an object `postMessage` then never
                // arrives. Page scripts stringify and post a string. That arrives as a JSON
                // string, so parse it once more.
                if let Some(text) = message.as_str() {
                    if let Ok(inner) = serde_json::from_str::<serde_json::Value>(text) {
                        message = inner;
                    }
                }
                match message.get("type").and_then(|kind| kind.as_str()) {
                    Some("ce-peek") => {
                        if let Some(url) = message.get("url").and_then(|url| url.as_str()) {
                            let app = message_app.clone();
                            let url = url.to_string();
                            crate::peek::note_click(id, &url);
                            if crate::settings::get(&app).peek_on_shift_click {
                                crate::peek::hold_focus(&app, id);
                            }
                            tauri::async_runtime::spawn(async move {
                                crate::peek::request(&app, id, &url);
                            });
                        }
                    }
                    Some("ce-background-tab") => {
                        if let Some(url) = message.get("url").and_then(|url| url.as_str()) {
                            tabs::open_background_link(&message_app, id, url);
                        }
                    }
                    Some("ce-notify") => {
                        let title = message
                            .get("title")
                            .and_then(|value| value.as_str())
                            .unwrap_or("Notification");
                        let body = message
                            .get("body")
                            .and_then(|value| value.as_str())
                            .unwrap_or("");
                        let page = tabs::tab_page_url(&message_app, id).unwrap_or_default();
                        if crate::privacy::decision(&message_app, &page, "notifications")
                            == Some(true)
                        {
                            crate::system::toast(title, body, "");
                        }
                    }
                    Some("ce-theme") => {
                        let color = message
                            .get("color")
                            .and_then(|value| value.as_str())
                            .unwrap_or("")
                            .to_string();
                        let app = message_app.clone();
                        tauri::async_runtime::spawn(async move {
                            tabs::set_theme_color(&app, id, &color)
                        });
                    }
                    Some("ce-error") => {
                        let app = message_app.clone();
                        tauri::async_runtime::spawn(async move { tabs::note_error(&app, id) });
                    }
                    Some("ce-inspect") => {
                        if let Some(text) = message.get("text").and_then(|value| value.as_str()) {
                            let _ = crate::ocr::copy_text(text);
                        }
                    }
                    Some("ce-zap") => {
                        if let Some(selector) =
                            message.get("selector").and_then(|value| value.as_str())
                        {
                            let app = message_app.clone();
                            let selector = selector.to_string();
                            tauri::async_runtime::spawn(async move {
                                if let Some(url) = tabs::tab_page_url(&app, id) {
                                    crate::page_rules::remember_zap(&app, &url, &selector);
                                }
                            });
                        }
                    }
                    Some("ce-mic") => {
                        let active = message
                            .get("active")
                            .and_then(|value| value.as_bool())
                            .unwrap_or(false);
                        let token = message
                            .get("token")
                            .and_then(|value| value.as_str())
                            .unwrap_or("");
                        crate::microphone::note_capture(&message_app, id, token, active);
                    }
                    Some("ce-zoom") => {
                        let change = message
                            .get("change")
                            .and_then(|value| value.as_str())
                            .unwrap_or("");
                        if change == "in" || change == "out" {
                            let app = message_app.clone();
                            let change = change.to_string();
                            tauri::async_runtime::spawn(async move {
                                if tabs::tab_page_url(&app, id).is_some_and(|url| {
                                    crate::page_rules::page_zoom_disabled(&app, &url)
                                }) {
                                    return;
                                }
                                let _ = tabs::zoom(&app, id, &change);
                            });
                        }
                    }
                    Some("ce-protocol") => {
                        let scheme = message
                            .get("scheme")
                            .and_then(|value| value.as_str())
                            .unwrap_or("")
                            .to_string();
                        let source = read_pwstr(|value| args.Source(value));
                        let app = message_app.clone();
                        let _ = message_app.run_on_main_thread(move || {
                            on_protocol(&app, id, &source, &scheme);
                        });
                    }
                    Some("ce-record") => {
                        crate::automation::recorder::on_message(&message_app, id, &message);
                    }
                    Some("ce-rpc") | Some("ce-content") | Some("ce-chrome") => {
                        let source = if message.get("type").and_then(|kind| kind.as_str())
                            == Some("ce-chrome")
                        {
                            read_pwstr(|value| args.Source(value))
                        } else {
                            tabs::tab_page_url(&message_app, id).unwrap_or_default()
                        };
                        crate::host_extensions::handle_message(
                            &message_app,
                            Some(id),
                            &format!("tab-{id}"),
                            &source,
                            &json,
                        );
                    }
                    Some("ce-speed") => {
                        if let Some(speed) = message.get("speed").and_then(|value| value.as_f64()) {
                            let app = message_app.clone();
                            tauri::async_runtime::spawn(async move {
                                if let Some(url) = tabs::tab_page_url(&app, id) {
                                    crate::page_rules::remember_speed(&app, &url, speed);
                                }
                            });
                        }
                    }
                    _ => {}
                }
                Ok(())
            })),
            &mut token,
        )?;

        if let Ok(core15) = core.cast::<ICoreWebView2_15>() {
            let favicon_app = app.clone();
            core15.add_FaviconChanged(
                &FaviconChangedEventHandler::create(Box::new(move |sender, _| {
                    let Some(core) = sender.and_then(|core| core.cast::<ICoreWebView2_15>().ok())
                    else {
                        return Ok(());
                    };
                    let favicon = read_pwstr(|value| core.FaviconUri(value));
                    let favicon = (!favicon.is_empty()).then_some(favicon);
                    tabs::update_favicon(&favicon_app, id, favicon);
                    let page_url = read_pwstr(|value| core.Source(value));
                    let accent_app = favicon_app.clone();
                    let handler =
                        GetFaviconCompletedHandler::create(Box::new(move |result, stream| {
                            let bytes = result
                                .ok()
                                .and(stream)
                                .and_then(|stream| read_stream(&stream));
                            let Some(bytes) = bytes else {
                                return Ok(());
                            };
                            if let Some(color) = crate::accent::average_png(&bytes) {
                                let app = accent_app.clone();
                                tauri::async_runtime::spawn(async move {
                                    tabs::set_favicon_accent(&app, id, color)
                                });
                            }
                            let app = accent_app.clone();
                            let page_url = page_url.clone();
                            tauri::async_runtime::spawn_blocking(move || {
                                tabs::update_favicon_image(&app, id, &page_url, &bytes);
                            });
                            Ok(())
                        }));
                    let _ = core.GetFavicon(COREWEBVIEW2_FAVICON_IMAGE_FORMAT_PNG, &handler);
                    Ok(())
                })),
                &mut token,
            )?;
        }

        if let (Ok(core11), Ok(environment)) = (
            core.cast::<ICoreWebView2_11>(),
            platform.environment().cast::<ICoreWebView2Environment9>(),
        ) {
            let menu_app = app.clone();
            core11.add_ContextMenuRequested(
                &ContextMenuRequestedEventHandler::create(Box::new(move |_, args| {
                    if let Some(args) = args {
                        let _ = add_page_items(&menu_app, &environment, &args, id);
                        let _ = add_plain_page_items(&menu_app, &environment, &args, id);
                        let _ = redirect_inspect(&menu_app, &environment, &args, id);
                    }
                    Ok(())
                })),
                &mut token,
            )?;
        }
    }

    if let Err(error) = attach_adblock(app, platform, &core, id) {
        eprintln!("WebView2 ad blocker for tab {id}: {error}");
    }
    if let Err(error) = attach_page_state(app, &core, id) {
        eprintln!("WebView2 load and crash events for tab {id}: {error}");
    }
    attach_downloads(app, &core, id)?;
    let dialog_app = app.clone();
    unsafe {
        core.add_ScriptDialogOpening(
            &ScriptDialogOpeningEventHandler::create(Box::new(move |_, args| {
                if let Some(args) = args {
                    crate::automation::dialog::handle(&dialog_app, id, args);
                }
                Ok(())
            })),
            &mut token,
        )?;
    }
    Ok(())
}

fn process_failed_reason(reason: COREWEBVIEW2_PROCESS_FAILED_REASON) -> &'static str {
    match reason {
        COREWEBVIEW2_PROCESS_FAILED_REASON_OUT_OF_MEMORY => "The page ran out of memory.",
        COREWEBVIEW2_PROCESS_FAILED_REASON_TERMINATED => "The page's process was ended.",
        COREWEBVIEW2_PROCESS_FAILED_REASON_UNRESPONSIVE => "The page stopped responding.",
        COREWEBVIEW2_PROCESS_FAILED_REASON_LAUNCH_FAILED => "The page's process could not start.",
        COREWEBVIEW2_PROCESS_FAILED_REASON_CRASHED => "The page's process crashed.",
        _ => "",
    }
}

fn certificate_detail(certificate: &ICoreWebView2Certificate) -> String {
    unsafe {
        let subject = read_pwstr(|value| certificate.Subject(value));
        let issuer = read_pwstr(|value| certificate.Issuer(value));
        let mut valid_to = 0f64;
        let _ = certificate.ValidTo(&mut valid_to);
        let until = crate::page_errors::date_of(valid_to);
        let mut parts = Vec::new();
        if !subject.is_empty() {
            parts.push(format!("Issued to {subject}."));
        }
        if !issuer.is_empty() {
            parts.push(format!("Issued by {issuer}."));
        }
        if !until.is_empty() {
            parts.push(format!("Valid until {until}."));
        }
        parts.join(" ")
    }
}

/// Load progress, finished and failed navigations, certificate errors, and crashed or hung pages.
fn attach_page_state(app: &AppHandle, core: &ICoreWebView2, id: u32) -> windows::core::Result<()> {
    let mut token = 0i64;
    unsafe {
        let failed_app = app.clone();
        core.add_ProcessFailed(
            &ProcessFailedEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else {
                    return Ok(());
                };
                let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND::default();
                args.ProcessFailedKind(&mut kind)?;
                let app = failed_app.clone();
                if kind == COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED {
                    tauri::async_runtime::spawn(async move {
                        crate::page_errors::browser_process_exited(&app)
                    });
                } else if kind == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED {
                    crate::page_errors::clear_unresponsive(id);
                    let detail = if crate::hang_guard::ended_recently() {
                        "It stopped responding, so it was ended to keep the rest of the browser working."
                            .to_string()
                    } else {
                        args.cast::<ICoreWebView2ProcessFailedEventArgs2>()
                            .ok()
                            .map(|args| {
                                let mut reason = COREWEBVIEW2_PROCESS_FAILED_REASON::default();
                                let _ = args.Reason(&mut reason);
                                process_failed_reason(reason).to_string()
                            })
                            .unwrap_or_default()
                    };
                    tauri::async_runtime::spawn(async move {
                        tabs::set_failure(&app, id, "crashed", "RENDER_PROCESS_EXITED", detail)
                    });
                } else if kind == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_UNRESPONSIVE {
                    tauri::async_runtime::spawn_blocking(move || {
                        let pids = crate::status::renderer_pids(&app, id);
                        crate::page_errors::note_unresponsive(id, pids);
                        tabs::set_unresponsive(&app, id, true);
                        if let Err(error) = tabs::show_unresponsive(&app, id) {
                            eprintln!("unresponsive page: {error}");
                        }
                    });
                }
                Ok(())
            })),
            &mut token,
        )?;

        let done_app = app.clone();
        core.add_NavigationCompleted(
            &NavigationCompletedEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else {
                    return Ok(());
                };
                let success = read_bool(|value| args.IsSuccess(value));
                if success {
                    tabs::set_progress(&done_app, id, 100);
                    return Ok(());
                }
                let mut status = COREWEBVIEW2_WEB_ERROR_STATUS::default();
                let _ = args.WebErrorStatus(&mut status);
                let Some((kind, code)) = crate::page_errors::web_error(status) else {
                    tabs::set_progress(&done_app, id, 100);
                    return Ok(());
                };
                let app = done_app.clone();
                tauri::async_runtime::spawn(async move {
                    if tabs::failure_of(&app, id).is_none() {
                        tabs::set_failure(&app, id, kind, code, String::new());
                    }
                });
                Ok(())
            })),
            &mut token,
        )?;

        if let Ok(core2) = core.cast::<ICoreWebView2_2>() {
            let dom_app = app.clone();
            core2.add_DOMContentLoaded(
                &DOMContentLoadedEventHandler::create(Box::new(move |sender, _| {
                    tabs::set_progress(&dom_app, id, 70);
                    // Gmail's document is ready here, but its load event can stay pending
                    // (splash images and a late stylesheet). Show the page instead of
                    // leaving the tab on the spinner.
                    let url = tabs::tab_page_url(&dom_app, id).unwrap_or_default();
                    if !url.is_empty() {
                        let app = dom_app.clone();
                        std::thread::spawn(move || {
                            std::thread::sleep(Duration::from_secs(3));
                            tabs::release_settled_page(&app, id, &url);
                        });
                    }
                    if let Some(core) = sender {
                        probe_page_scheme(&dom_app, id, &core);
                    }
                    Ok(())
                })),
                &mut token,
            )?;
        }

        if let Ok(core14) = core.cast::<ICoreWebView2_14>() {
            let certificate_app = app.clone();
            core14.add_ServerCertificateErrorDetected(
                &ServerCertificateErrorDetectedEventHandler::create(Box::new(move |_, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };
                    let uri = read_pwstr(|value| args.RequestUri(value));
                    if crate::page_errors::certificate_allowed(&uri) {
                        args.SetAction(COREWEBVIEW2_SERVER_CERTIFICATE_ERROR_ACTION_ALWAYS_ALLOW)?;
                        return Ok(());
                    }
                    args.SetAction(COREWEBVIEW2_SERVER_CERTIFICATE_ERROR_ACTION_CANCEL)?;
                    if !crate::page_errors::is_page_request(id, &uri) {
                        return Ok(());
                    }
                    let mut status = COREWEBVIEW2_WEB_ERROR_STATUS::default();
                    let _ = args.ErrorStatus(&mut status);
                    let code = crate::page_errors::web_error(status)
                        .map(|(_, code)| code)
                        .unwrap_or("CERTIFICATE_IS_INVALID");
                    let detail = args
                        .ServerCertificate()
                        .map(|certificate| certificate_detail(&certificate))
                        .unwrap_or_default();
                    let app = certificate_app.clone();
                    tauri::async_runtime::spawn(async move {
                        tabs::set_failure(&app, id, "certificate", code, detail)
                    });
                    Ok(())
                })),
                &mut token,
            )?;
        }
    }
    Ok(())
}

fn header_rows(iterator: &ICoreWebView2HttpHeadersCollectionIterator) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    unsafe {
        let mut has_current = BOOL::default();
        if iterator.HasCurrentHeader(&mut has_current).is_err() {
            return rows;
        }
        while has_current.as_bool() && rows.len() < 80 {
            let mut name = PWSTR::null();
            let mut value = PWSTR::null();
            if iterator.GetCurrentHeader(&mut name, &mut value).is_err() {
                break;
            }
            let name = if name.is_null() {
                String::new()
            } else {
                take_pwstr(name)
            };
            let value = if value.is_null() {
                String::new()
            } else {
                take_pwstr(value)
            };
            if !name.is_empty() {
                rows.push((name, value));
            }
            if iterator.MoveNext(&mut has_current).is_err() {
                break;
            }
        }
    }
    rows
}

fn resource_kind(context: COREWEBVIEW2_WEB_RESOURCE_CONTEXT) -> &'static str {
    match context {
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT => "document",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_STYLESHEET => "stylesheet",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_IMAGE => "image",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_MEDIA => "media",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_FONT => "font",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_SCRIPT => "script",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_XML_HTTP_REQUEST => "xhr",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_FETCH => "fetch",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_TEXT_TRACK => "track",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_EVENT_SOURCE => "event",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_WEBSOCKET => "websocket",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_MANIFEST => "manifest",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_SIGNED_EXCHANGE => "exchange",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_PING => "ping",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_CSP_VIOLATION_REPORT => "report",
        _ => "other",
    }
}

#[derive(Default)]
struct EarlyScriptSlot {
    script_id: Option<String>,
    stamp: Option<String>,
    inflight: bool,
}

fn early_slots() -> &'static Mutex<HashMap<u32, EarlyScriptSlot>> {
    static SLOTS: std::sync::OnceLock<Mutex<HashMap<u32, EarlyScriptSlot>>> =
        std::sync::OnceLock::new();
    SLOTS.get_or_init(Default::default)
}

/// Drops the document-start script id for a tab whose webview is going away.
pub fn forget_early_script(id: u32) {
    early_slots()
        .lock()
        .expect("adblock early script")
        .remove(&id);
    microphone_scripts()
        .lock()
        .expect("microphone script")
        .remove(&id);
    protocol_scripts()
        .lock()
        .expect("protocol script")
        .remove(&id);
}

fn microphone_scripts() -> &'static Mutex<HashSet<u32>> {
    static IDS: std::sync::OnceLock<Mutex<HashSet<u32>>> = std::sync::OnceLock::new();
    IDS.get_or_init(Default::default)
}

/// The microphone wrapper is static, so each tab webview installs it once.
fn protocol_scripts() -> &'static Mutex<HashSet<u32>> {
    static IDS: std::sync::OnceLock<Mutex<HashSet<u32>>> = std::sync::OnceLock::new();
    IDS.get_or_init(Default::default)
}

/// Asks the page to report `registerProtocolHandler` before the site's own scripts run.
fn install_protocol_script(core: &ICoreWebView2, id: u32) {
    {
        let mut ids = protocol_scripts().lock().expect("protocol script");
        if !ids.insert(id) {
            return;
        }
    }
    let script = HSTRING::from(crate::privacy::PROTOCOL_JS);
    let handler = AddScriptToExecuteOnDocumentCreatedCompletedHandler::create(Box::new(
        move |result, _script_id| {
            if result.is_err() {
                protocol_scripts()
                    .lock()
                    .expect("protocol script")
                    .remove(&id);
            }
            Ok(())
        },
    ));
    if unsafe { core.AddScriptToExecuteOnDocumentCreated(&script, &handler) }.is_err() {
        protocol_scripts()
            .lock()
            .expect("protocol script")
            .remove(&id);
    }
}

fn install_microphone_script(core: &ICoreWebView2, id: u32) {
    {
        let mut ids = microphone_scripts().lock().expect("microphone script");
        if !ids.insert(id) {
            return;
        }
    }
    let script = HSTRING::from(crate::microphone::DOCUMENT_SCRIPT);
    let handler = AddScriptToExecuteOnDocumentCreatedCompletedHandler::create(Box::new(
        move |result, _script_id| {
            if result.is_err() {
                microphone_scripts()
                    .lock()
                    .expect("microphone script")
                    .remove(&id);
            }
            Ok(())
        },
    ));
    if unsafe { core.AddScriptToExecuteOnDocumentCreated(&script, &handler) }.is_err() {
        microphone_scripts()
            .lock()
            .expect("microphone script")
            .remove(&id);
    }
}

/// Registers the analytics opt-out script for documents created after this call.
/// A second call removes the previous script when the paused-site list changed.
fn sync_early_script(core: &ICoreWebView2, id: u32) {
    let stamp = crate::adblock::early_stamp();
    let mut slots = early_slots().lock().expect("adblock early script");
    let slot = slots.entry(id).or_default();
    if slot.inflight {
        return;
    }
    if slot.stamp.as_deref() == Some(stamp.as_str()) && slot.script_id.is_some() {
        return;
    }
    if let Some(script_id) = slot.script_id.take() {
        let _ = unsafe { core.RemoveScriptToExecuteOnDocumentCreated(&HSTRING::from(script_id)) };
    }
    slot.inflight = true;
    slot.stamp = Some(stamp);
    drop(slots);

    let script = HSTRING::from(crate::adblock::early_script());
    let handler = AddScriptToExecuteOnDocumentCreatedCompletedHandler::create(Box::new(
        move |result, script_id| {
            let mut slots = early_slots().lock().expect("adblock early script");
            let slot = slots.entry(id).or_default();
            slot.inflight = false;
            if result.is_ok() {
                slot.script_id = Some(script_id);
            } else {
                slot.stamp = None;
            }
            Ok(())
        },
    ));
    if unsafe { core.AddScriptToExecuteOnDocumentCreated(&script, &handler) }.is_err() {
        let mut slots = early_slots().lock().expect("adblock early script");
        if let Some(slot) = slots.get_mut(&id) {
            slot.inflight = false;
            slot.stamp = None;
        }
    }
}

fn attach_adblock(
    app: &AppHandle,
    platform: &PlatformWebview,
    core: &ICoreWebView2,
    id: u32,
) -> windows::core::Result<()> {
    let mut token = 0i64;
    unsafe {
        // Without source kinds, requests made inside iframes skip the event.
        match core.cast::<ICoreWebView2_22>() {
            Ok(core22) => core22.AddWebResourceRequestedFilterWithRequestSourceKinds(
                windows::core::w!("*"),
                COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
                COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL,
            )?,
            Err(_) => core.AddWebResourceRequestedFilter(
                windows::core::w!("*"),
                COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
            )?,
        }

        sync_early_script(core, id);
        install_microphone_script(core, id);
        install_protocol_script(core, id);
        crate::host_extensions::prepare_core(app, core, &format!("tab-{id}"));

        let environment = platform.environment();
        let request_app = app.clone();
        core.add_WebResourceRequested(
            &WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else {
                    return Ok(());
                };
                let mut context = COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL;
                let _ = args.ResourceContext(&mut context);
                if let Ok(request) = args.Request() {
                    crate::privacy::note_request_started(
                        &request_app,
                        id,
                        request.as_raw() as usize,
                        resource_kind(context),
                    );
                }
                let uri = args
                    .Request()
                    .map(|request| read_pwstr(|value| request.Uri(value)))
                    .unwrap_or_default();
                if let Some(file) = crate::system::file_bytes(&uri)
                    && let Some(stream) =
                        windows::Win32::UI::Shell::SHCreateMemStream(Some(&file.bytes))
                {
                    let headers = HSTRING::from(format!(
                        "Content-Type: {}\r\nAccess-Control-Allow-Origin: *",
                        file.content_type
                    ));
                    let response = environment.CreateWebResourceResponse(
                        &stream,
                        200,
                        windows::core::w!("OK"),
                        &headers,
                    )?;
                    args.SetResponse(&response)?;
                    return Ok(());
                }
                let defer_blocks = crate::extensions::extension_handles_requests();
                if let Some(hit) = crate::rewrite::find(&uri, &crate::rewrite::list(&request_app)) {
                    match hit {
                        crate::rewrite::RewriteHit::Block if defer_blocks => {}
                        crate::rewrite::RewriteHit::Block => {
                            let response = environment.CreateWebResourceResponse(
                                None::<&IStream>,
                                403,
                                windows::core::w!("Blocked"),
                                windows::core::w!(""),
                            )?;
                            args.SetResponse(&response)?;
                            return Ok(());
                        }
                        crate::rewrite::RewriteHit::Redirect(target) => {
                            if let Ok(request) = args.Request() {
                                let _ = request.SetUri(&HSTRING::from(target));
                            }
                        }
                        crate::rewrite::RewriteHit::File(path) => {
                            let method = args
                                .Request()
                                .map(|request| read_pwstr(|value| request.Method(value)))
                                .unwrap_or_default();
                            if method.eq_ignore_ascii_case("GET")
                                && let Some(bytes) = crate::rewrite::file_bytes(&path)
                                && let Some(stream) =
                                    windows::Win32::UI::Shell::SHCreateMemStream(Some(&bytes))
                            {
                                let response = environment.CreateWebResourceResponse(
                                    &stream,
                                    200,
                                    windows::core::w!("OK"),
                                    windows::core::w!("Content-Type: application/octet-stream"),
                                )?;
                                args.SetResponse(&response)?;
                                return Ok(());
                            }
                        }
                    }
                }
                let is_document = context == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT;
                if !defer_blocks
                    && crate::adblock::should_block(
                        &request_app,
                        id,
                        &uri,
                        is_document,
                        resource_kind(context),
                    )
                {
                    let response = environment.CreateWebResourceResponse(
                        None::<&IStream>,
                        403,
                        windows::core::w!("Blocked"),
                        windows::core::w!(""),
                    )?;
                    args.SetResponse(&response)?;
                    return Ok(());
                }
                let page = tabs::tab_page_url(&request_app, id).unwrap_or_default();
                let blank = !is_document
                    && ((context == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_IMAGE
                        && crate::page_rules::blocks_images(&request_app, &page))
                        || (context == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_FONT
                            && crate::page_rules::blocks_fonts(&request_app, &page)));
                if blank {
                    let response = environment.CreateWebResourceResponse(
                        None::<&IStream>,
                        200,
                        windows::core::w!("OK"),
                        windows::core::w!(""),
                    )?;
                    args.SetResponse(&response)?;
                    return Ok(());
                }
                if let Ok(request) = args.Request() {
                    if let Ok(headers) = request.Headers() {
                        strip_webview_client_hints(&headers);
                        apply_referrer(&request_app, &page, &uri, &headers);
                    }
                }
                Ok(())
            })),
            &mut token,
        )?;

        let navigation_app = app.clone();
        core.add_NavigationStarting(
            &NavigationStartingEventHandler::create(Box::new(move |sender, args| {
                if let Some(args) = args {
                    let uri = read_pwstr(|value| args.Uri(value));
                    crate::page_errors::note_navigation(id, &uri);
                    tabs::navigation_started(&navigation_app, id, &uri);
                    tabs::clear_errors(&navigation_app, id);
                    tabs::clear_accent(&navigation_app, id);
                    if let Some(next) = crate::privacy::rewrite_navigation(&navigation_app, &uri) {
                        let _ = args.SetCancel(true);
                        let app = navigation_app.clone();
                        tauri::async_runtime::spawn(async move {
                            let _ = tabs::navigate(&app, id, &next);
                        });
                        return Ok(());
                    }
                    crate::adblock::page_started(&navigation_app, id, &uri);
                    tabs::paint_document_background(&navigation_app, id, &uri);
                    if let Some(core) = sender {
                        apply_page_settings(&navigation_app, &core, &uri);
                        sync_early_script(&core, id);
                    }
                }
                Ok(())
            })),
            &mut token,
        )?;

        let content_app = app.clone();
        core.add_ContentLoading(
            &ContentLoadingEventHandler::create(Box::new(move |sender, _| {
                let Some(core) = sender else {
                    return Ok(());
                };
                tabs::set_progress(&content_app, id, 35);
                let source = read_pwstr(|value| core.Source(value));
                if source.starts_with("http") && crate::adblock::active_for(&source) {
                    let script = HSTRING::from(crate::adblock::early_script());
                    let _ = core.ExecuteScript(
                        &script,
                        None::<&ICoreWebView2ExecuteScriptCompletedHandler>,
                    );
                    let script = HSTRING::from(crate::adblock::cosmetic_script());
                    let _ = core.ExecuteScript(
                        &script,
                        None::<&ICoreWebView2ExecuteScriptCompletedHandler>,
                    );
                }
                let injection = crate::page_rules::injection(&content_app, &source);
                if !injection.is_empty() {
                    let script = HSTRING::from(injection);
                    let _ = core.ExecuteScript(
                        &script,
                        None::<&ICoreWebView2ExecuteScriptCompletedHandler>,
                    );
                }
                Ok(())
            })),
            &mut token,
        )?;

        if let Ok(core2) = core.cast::<ICoreWebView2_2>() {
            let response_app = app.clone();
            core2.add_WebResourceResponseReceived(
                &WebResourceResponseReceivedEventHandler::create(Box::new(move |_, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };
                    let Ok(request) = args.Request() else {
                        return Ok(());
                    };
                    let uri = read_pwstr(|value| request.Uri(value));
                    let method = read_pwstr(|value| request.Method(value));
                    let response = args.Response().ok();
                    let status = response.as_ref().and_then(|response| {
                        let mut code = 0i32;
                        response.StatusCode(&mut code).ok().map(|_| code)
                    });
                    let started = crate::privacy::take_request_started(
                        &response_app,
                        id,
                        request.as_raw() as usize,
                    );
                    let tracked = !crate::privacy::ignored_url(&uri);
                    let request_headers = if tracked {
                        request
                            .Headers()
                            .ok()
                            .and_then(|headers| headers.GetIterator().ok())
                            .map(|iterator| header_rows(&iterator))
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };
                    let response_headers = if tracked {
                        response
                            .as_ref()
                            .and_then(|response| response.Headers().ok())
                            .and_then(|headers| headers.GetIterator().ok())
                            .map(|iterator| header_rows(&iterator))
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };
                    let kind = started
                        .as_ref()
                        .map(|(_, kind)| kind.clone())
                        .unwrap_or_else(|| "other".to_string());
                    let started_at = started.map(|(at, _)| at);
                    crate::privacy::record_request(
                        &response_app,
                        id,
                        &method,
                        &uri,
                        &kind,
                        status,
                        started_at,
                        request_headers,
                        response_headers,
                    );
                    Ok(())
                })),
                &mut token,
            )?;
        }

        let permission_app = app.clone();
        core.add_PermissionRequested(
            &PermissionRequestedEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else {
                    return Ok(());
                };
                let uri = read_pwstr(|value| args.Uri(value));
                let mut kind = COREWEBVIEW2_PERMISSION_KIND::default();
                args.PermissionKind(&mut kind)?;
                on_permission(&permission_app, id, &uri, kind, &args)
            })),
            &mut token,
        )?;
    }
    Ok(())
}

fn permission_name(kind: COREWEBVIEW2_PERMISSION_KIND) -> &'static str {
    if kind == COREWEBVIEW2_PERMISSION_KIND_CAMERA {
        "camera"
    } else if kind == COREWEBVIEW2_PERMISSION_KIND_MICROPHONE {
        "microphone"
    } else if kind == COREWEBVIEW2_PERMISSION_KIND_GEOLOCATION {
        "location"
    } else if kind == COREWEBVIEW2_PERMISSION_KIND_NOTIFICATIONS {
        "notifications"
    } else if kind == COREWEBVIEW2_PERMISSION_KIND_CLIPBOARD_READ {
        "clipboard"
    } else if kind == COREWEBVIEW2_PERMISSION_KIND_OTHER_SENSORS {
        "sensors"
    } else if kind == COREWEBVIEW2_PERMISSION_KIND_MULTIPLE_AUTOMATIC_DOWNLOADS {
        "downloads"
    } else if kind == COREWEBVIEW2_PERMISSION_KIND_FILE_READ_WRITE {
        "file-system"
    } else if kind == COREWEBVIEW2_PERMISSION_KIND_AUTOPLAY {
        "autoplay"
    } else if kind == COREWEBVIEW2_PERMISSION_KIND_LOCAL_FONTS {
        "local-fonts"
    } else if kind == COREWEBVIEW2_PERMISSION_KIND_MIDI_SYSTEM_EXCLUSIVE_MESSAGES {
        "midi"
    } else if kind == COREWEBVIEW2_PERMISSION_KIND_WINDOW_MANAGEMENT {
        "window-management"
    } else if kind.0 == 13 {
        "storage"
    } else {
        "other"
    }
}

/// A permission the user has not answered. WebView2 requests keep their deferral on the main
/// thread. Protocol requests call back into the page when the user answers.
struct PendingPrompt {
    id: u64,
    tab: u32,
    origin: String,
    kind: String,
    host: String,
    label: String,
    shown: bool,
    /// Further identical protocol requests waiting on this same answer.
    extras: u32,
    scheme: Option<String>,
    web: Option<
        MainThread<(
            ICoreWebView2PermissionRequestedEventArgs,
            ICoreWebView2Deferral,
        )>,
    >,
}

struct AutoAllow {
    tab: u32,
    origin: String,
    at: Instant,
}

struct PermissionPrompts {
    next: u64,
    pending: Vec<PendingPrompt>,
    auto: Vec<AutoAllow>,
}

fn prompts() -> &'static Mutex<PermissionPrompts> {
    static PROMPTS: std::sync::OnceLock<Mutex<PermissionPrompts>> = std::sync::OnceLock::new();
    PROMPTS.get_or_init(|| {
        Mutex::new(PermissionPrompts {
            next: 1,
            pending: Vec::new(),
            auto: Vec::new(),
        })
    })
}

fn on_permission(
    app: &AppHandle,
    tab: u32,
    uri: &str,
    kind: COREWEBVIEW2_PERMISSION_KIND,
    args: &ICoreWebView2PermissionRequestedEventArgs,
) -> windows::core::Result<()> {
    let name = permission_name(kind);
    let origin = origin_of(app, tab, uri);
    let allowed = !origin.is_empty() && crate::privacy::decision(app, uri, name) == Some(true);
    let auto = name == "other" && take_auto(tab, &origin);
    if allowed || auto {
        hide_default_permission_ui(args);
        let _ = unsafe { args.SetState(COREWEBVIEW2_PERMISSION_STATE_ALLOW) };
        save_permission_choice(args, true);
        return Ok(());
    }
    if !origin.is_empty() && crate::privacy::decision(app, uri, name) == Some(false) {
        hide_default_permission_ui(args);
        let _ = unsafe { args.SetState(COREWEBVIEW2_PERMISSION_STATE_DENY) };
        save_permission_choice(args, true);
        return Ok(());
    }
    let deferral = unsafe { args.GetDeferral()? };
    hide_default_permission_ui(args);
    let held = MainThread::new((args.clone(), deferral));
    let host = site_host(if origin.is_empty() { uri } else { &origin });
    let label = crate::privacy::request_label(name);
    if name != "other" {
        crate::privacy::note_request(app, uri, name);
    }
    let mut guard = prompts().lock().expect("permission prompts");
    let id = guard.next;
    guard.next += 1;
    guard.pending.push(PendingPrompt {
        id,
        tab,
        origin,
        kind: name.to_string(),
        host,
        label,
        shown: false,
        extras: 0,
        scheme: None,
        web: Some(held),
    });
    drop(guard);
    let later = app.clone();
    let _ = app.run_on_main_thread(move || present_next(&later));
    Ok(())
}

fn on_protocol(app: &AppHandle, tab: u32, source: &str, scheme: &str) {
    let Some(kind) = crate::privacy::protocol_kind(scheme) else {
        finish_protocol(app, tab, false, scheme, 1);
        return;
    };
    let origin = origin_of(app, tab, source);
    if origin.is_empty() {
        finish_protocol(app, tab, false, scheme, 1);
        return;
    }
    if let Some(allow) = crate::privacy::decision(app, &origin, &kind) {
        if allow {
            arm_auto(tab, &origin);
        }
        finish_protocol(app, tab, allow, scheme, 1);
        return;
    }
    {
        let mut guard = prompts().lock().expect("permission prompts");
        if let Some(existing) = guard
            .pending
            .iter_mut()
            .find(|item| item.tab == tab && item.kind == kind)
        {
            existing.extras += 1;
            return;
        }
        let id = guard.next;
        guard.next += 1;
        guard.pending.push(PendingPrompt {
            id,
            tab,
            origin: origin.clone(),
            kind: kind.clone(),
            host: site_host(&origin),
            label: crate::privacy::request_label(&kind),
            shown: false,
            extras: 0,
            scheme: Some(scheme.to_string()),
            web: None,
        });
    }
    crate::privacy::note_request(app, &origin, &kind);
    let later = app.clone();
    let _ = app.run_on_main_thread(move || present_next(&later));
}

/// Records the user's choice and, for a protocol handler, tells the page to finish registering.
pub fn answer_permission(app: &AppHandle, id: u64, choice: &str) {
    let Some(prompt) = take_prompt(id) else {
        return;
    };
    let choice = match choice {
        "allow" | "block" => choice.to_string(),
        _ => "dismiss".into(),
    };
    let later = app.clone();
    let _ = app.run_on_main_thread(move || settle(&later, prompt, &choice));
}

/// The prompt window went away. An answer that already arrived is left alone.
pub fn permission_popup_closed(app: &AppHandle, id: u64) {
    let prompt = take_prompt(id);
    let later = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(prompt) = prompt {
            settle(&later, prompt, "dismiss");
        }
        present_next(&later);
    });
}

fn take_prompt(id: u64) -> Option<PendingPrompt> {
    let mut guard = prompts().lock().expect("permission prompts");
    let index = guard.pending.iter().position(|item| item.id == id)?;
    Some(guard.pending.remove(index))
}

fn present_next(app: &AppHandle) {
    let next = {
        let mut guard = prompts().lock().expect("permission prompts");
        let Some(item) = guard.pending.iter_mut().find(|item| !item.shown) else {
            return;
        };
        item.shown = true;
        let params = format!(
            "id={}&host={}&label={}&kind={}",
            item.id,
            form_escape(&item.host),
            form_escape(&item.label),
            form_escape(&item.kind),
        );
        Some((item.id, item.tab, params))
    };
    let Some((id, tab, params)) = next else {
        return;
    };
    if let Err(error) = tabs::show_permission_prompt(app, tab, &params) {
        if error == "open" {
            if let Some(item) = prompts()
                .lock()
                .expect("permission prompts")
                .pending
                .iter_mut()
                .find(|item| item.id == id)
            {
                item.shown = false;
            }
            return;
        }
        if let Some(prompt) = take_prompt(id) {
            settle(app, prompt, "dismiss");
        }
        present_next(app);
    }
}

fn settle(app: &AppHandle, prompt: PendingPrompt, choice: &str) {
    let allow = choice == "allow";
    let persist = choice != "dismiss";
    if persist && !prompt.origin.is_empty() && prompt.kind != "other" {
        let _ =
            crate::privacy::set_permission(app, &prompt.origin, &prompt.kind, allow, Some(true));
    }
    if let Some(held) = prompt.web {
        let (args, deferral) = held.into_inner();
        unsafe {
            if persist {
                save_permission_choice(&args, true);
            }
            let _ = args.SetState(if allow {
                COREWEBVIEW2_PERMISSION_STATE_ALLOW
            } else {
                COREWEBVIEW2_PERMISSION_STATE_DENY
            });
            let _ = deferral.Complete();
        }
    }
    if let Some(scheme) = &prompt.scheme {
        if allow {
            arm_auto(prompt.tab, &prompt.origin);
        }
        finish_protocol(app, prompt.tab, allow, scheme, prompt.extras + 1);
    }
}

fn finish_protocol(app: &AppHandle, tab: u32, allow: bool, scheme: &str, count: u32) {
    let scheme = serde_json::to_string(scheme).unwrap_or_else(|_| "\"\"".into());
    let script = format!(
        "if (window.__ceProtocolFinish) window.__ceProtocolFinish({allow}, {scheme}, {count});"
    );
    if let Some(webview) = tabs::page_view(app, tab) {
        let _ = webview.eval(&script);
    }
}

fn arm_auto(tab: u32, origin: &str) {
    if origin.is_empty() {
        return;
    }
    prompts()
        .lock()
        .expect("permission prompts")
        .auto
        .push(AutoAllow {
            tab,
            origin: origin.to_string(),
            at: Instant::now(),
        });
}

fn take_auto(tab: u32, origin: &str) -> bool {
    if origin.is_empty() {
        return false;
    }
    let mut guard = prompts().lock().expect("permission prompts");
    let now = Instant::now();
    guard
        .auto
        .retain(|item| now.saturating_duration_since(item.at) < Duration::from_secs(2));
    let Some(index) = guard
        .auto
        .iter()
        .position(|item| item.tab == tab && item.origin == origin)
    else {
        return false;
    };
    guard.auto.remove(index);
    true
}

fn hide_default_permission_ui(args: &ICoreWebView2PermissionRequestedEventArgs) {
    if let Ok(args2) = args.cast::<ICoreWebView2PermissionRequestedEventArgs2>() {
        let _ = unsafe { args2.SetHandled(true) };
    }
}

fn save_permission_choice(args: &ICoreWebView2PermissionRequestedEventArgs, save: bool) {
    if let Ok(args3) = args.cast::<ICoreWebView2PermissionRequestedEventArgs3>() {
        let _ = unsafe { args3.SetSavesInProfile(save) };
    }
}

fn origin_of(app: &AppHandle, tab: u32, source: &str) -> String {
    crate::paths::http_origin(source)
        .or_else(|| tabs::tab_page_url(app, tab).and_then(|url| crate::paths::http_origin(&url)))
        .unwrap_or_default()
}

fn site_host(raw: &str) -> String {
    url::Url::parse(raw)
        .ok()
        .and_then(|url| url.host_str().map(ToString::to_string))
        .filter(|host| !host.is_empty())
        .unwrap_or_else(|| "This site".into())
}

fn form_escape(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Drops the WebView2 client-hint brand before the request leaves.
///
/// Prime Video's player CDN answers 403 when `Sec-CH-UA` names "Microsoft Edge WebView2",
/// and the page then never receives `WebLoader.js`. Edge itself is left in the hint.
fn strip_webview_client_hints(headers: &ICoreWebView2HttpRequestHeaders) {
    rewrite_client_hint(headers, windows::core::w!("Sec-CH-UA"));
    rewrite_client_hint(headers, windows::core::w!("Sec-CH-UA-Full-Version-List"));
    let agent = header_value(headers, windows::core::w!("User-Agent"));
    if !agent.to_ascii_lowercase().contains("webview") {
        return;
    }
    let cleaned = strip_webview_user_agent(&agent);
    if cleaned.is_empty() || cleaned == agent {
        return;
    }
    let _ = unsafe { headers.SetHeader(windows::core::w!("User-Agent"), &HSTRING::from(cleaned)) };
}

fn header_value(headers: &ICoreWebView2HttpRequestHeaders, name: windows::core::PCWSTR) -> String {
    read_pwstr(|value| unsafe { headers.GetHeader(name, value) })
}

fn rewrite_client_hint(headers: &ICoreWebView2HttpRequestHeaders, name: windows::core::PCWSTR) {
    let current = header_value(headers, name);
    if !current.to_ascii_lowercase().contains("webview") {
        return;
    }
    let cleaned = strip_webview_brand(&current);
    if cleaned == current {
        return;
    }
    let _ = unsafe { headers.RemoveHeader(name) };
    if !cleaned.is_empty() {
        let _ = unsafe { headers.SetHeader(name, &HSTRING::from(cleaned)) };
    }
}

/// Removes client-hint brands whose name contains "webview".
fn strip_webview_brand(value: &str) -> String {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty() && !part.to_ascii_lowercase().contains("webview"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Removes a `WebView` or `WebView2` product token from a user agent.
fn strip_webview_user_agent(agent: &str) -> String {
    let lower = agent.to_ascii_lowercase();
    let mut result = String::with_capacity(agent.len());
    let mut rest = agent;
    let mut lower_rest = lower.as_str();
    while let Some(index) = webview_at(lower_rest) {
        result.push_str(&rest[..index]);
        let len = webview_token_len(&lower_rest[index..]);
        rest = &rest[index + len..];
        lower_rest = &lower_rest[index + len..];
        if result.ends_with(' ') && (rest.is_empty() || rest.starts_with(' ')) {
            result.pop();
            if let Some(stripped) = rest.strip_prefix(' ') {
                rest = stripped;
                lower_rest = &lower_rest[1..];
            }
        }
    }
    result.push_str(rest);
    result
}

fn webview_at(lower: &str) -> Option<usize> {
    let bytes = lower.as_bytes();
    let mut start = 0;
    while start + 7 <= bytes.len() {
        let Some(rel) = lower[start..].find("webview") else {
            break;
        };
        let index = start + rel;
        let before_ok = index == 0 || !bytes[index - 1].is_ascii_alphanumeric();
        if before_ok {
            return Some(index);
        }
        start = index + 7;
    }
    None
}

fn webview_token_len(lower: &str) -> usize {
    let bytes = lower.as_bytes();
    let mut len = 7;
    if bytes.get(len) == Some(&b'2') {
        len += 1;
    }
    if bytes.get(len) == Some(&b'/') {
        len += 1;
        while len < bytes.len() && (bytes[len].is_ascii_digit() || bytes[len] == b'.') {
            len += 1;
        }
    }
    len
}

/// Runs before every frame's own scripts. Prime Video treats `chrome.webview` as its native
/// shell and then crashes on the missing host object (`Element not found` / `0x80070490`).
/// The real bridge stays on `__ceHost` for this browser's own page scripts.
pub const WEBVIEW_CLOAK_SCRIPT: &str = r#"(function () {
  if (window.__ceCloak) return;
  window.__ceCloak = true;
  var chromeObj = window.chrome;
  var bridge = null;
  try { bridge = chromeObj && chromeObj.webview; } catch (error) {}
  if (bridge && !window.__ceHost) {
    var send = function (message) {
      try {
        bridge.postMessage(typeof message === "string" ? message : JSON.stringify(message));
      } catch (error) {}
    };
    var host = { postMessage: send };
    try {
      Object.defineProperty(window, "__ceHost", {
        configurable: true,
        enumerable: false,
        writable: false,
        value: host
      });
    } catch (error) {
      window.__ceHost = host;
    }
  }
  if (!remotePage()) return;
  hideWebView(chromeObj);
  patchIpc();
  spoofClientHints();

  function appHost(host) {
    host = String(host || "").toLowerCase();
    return host === "tauri.localhost" || host === "localhost" || host === "127.0.0.1" || host === "::1" || host.endsWith(".ce-ext.localhost");
  }
  function remotePage() {
    if (appHost(location.hostname)) return false;
    var protocol = location.protocol;
    if (protocol === "http:" || protocol === "https:") return true;
    if (protocol !== "about:" && protocol !== "blob:") return false;
    try {
      if (!window.top || window.top === window) return false;
      if (appHost(window.top.location.hostname)) return false;
      return window.top.location.protocol === "http:" || window.top.location.protocol === "https:";
    } catch (error) {
      return true;
    }
  }
  function hideWebView(chromeObj) {
    if (!chromeObj) return;
    try { delete chromeObj.webview; } catch (error) {}
    var still = false;
    try { still = !!chromeObj.webview; } catch (error) { still = true; }
    if (!still) return;
    try {
      Object.defineProperty(chromeObj, "webview", {
        configurable: true,
        enumerable: false,
        get: function () { return undefined; },
        set: function () {}
      });
    } catch (error) {
      try { chromeObj.webview = undefined; } catch (ignored) {}
    }
  }
  function patchIpc() {
    var host = window.__ceHost;
    if (!host) return;
    try {
      Object.defineProperty(window, "ipc", {
        configurable: true,
        enumerable: true,
        writable: true,
        value: Object.freeze({
          postMessage: function (message) { host.postMessage(message); }
        })
      });
    } catch (error) {}
  }
  function keep(list) {
    return (list || []).filter(function (item) {
      return !item || !/webview/i.test(String(item.brand || ""));
    });
  }
  function spoofClientHints() {
    var data = navigator.userAgentData;
    if (data && data.brands && data.brands.some(function (item) { return item && /webview/i.test(String(item.brand || "")); })) {
      var brands = keep(data.brands);
      var fake = {
        brands: brands,
        mobile: !!data.mobile,
        platform: data.platform || "Windows",
        toJSON: function () {
          return { brands: brands, mobile: !!data.mobile, platform: data.platform || "Windows" };
        },
        getHighEntropyValues: function (hints) {
          if (!data.getHighEntropyValues) return Promise.resolve(fake.toJSON());
          return data.getHighEntropyValues.call(data, hints).then(function (values) {
            var copy = {};
            for (var key in values) copy[key] = values[key];
            if (copy.brands) copy.brands = keep(copy.brands);
            if (copy.fullVersionList) copy.fullVersionList = keep(copy.fullVersionList);
            return copy;
          });
        }
      };
      try {
        Object.defineProperty(Navigator.prototype, "userAgentData", {
          configurable: true,
          get: function () { return fake; }
        });
      } catch (error) {}
    }
    var agent = navigator.userAgent || "";
    if (!/webview/i.test(agent)) return;
    var cleaned = agent.replace(/\s*WebView2?\/[\d.]+/ig, "").replace(/\s*WebView2?(?=[\s);]|$)/ig, "");
    if (!cleaned || cleaned === agent) return;
    try {
      Object.defineProperty(Navigator.prototype, "userAgent", {
        configurable: true,
        get: function () { return cleaned; }
      });
    } catch (error) {}
  }
})();"#;

fn apply_referrer(
    app: &AppHandle,
    page: &str,
    request: &str,
    headers: &ICoreWebView2HttpRequestHeaders,
) {
    let rule = crate::page_rules::control_for(app, page).referrer;
    match crate::privacy::referrer_value(&rule, page, request) {
        crate::privacy::ReferrerAction::Keep => {}
        crate::privacy::ReferrerAction::Remove => {
            let _ = unsafe { headers.RemoveHeader(windows::core::w!("Referer")) };
        }
        crate::privacy::ReferrerAction::Set(value) => {
            let _ =
                unsafe { headers.SetHeader(windows::core::w!("Referer"), &HSTRING::from(value)) };
        }
    }
}

/// Sets the user agent and whether JavaScript runs for the page that is about to load.
fn apply_page_settings(app: &AppHandle, core: &ICoreWebView2, url: &str) {
    let agent = crate::page_rules::user_agent_for(app, url);
    let enabled = crate::page_rules::scripts_enabled(app, url);
    unsafe {
        let Ok(settings) = core.Settings() else {
            return;
        };
        let _ = settings.SetIsScriptEnabled(enabled);
        if let Some(agent) = agent {
            if let Ok(settings) = settings.cast::<ICoreWebView2Settings2>() {
                let _ = settings.SetUserAgent(&HSTRING::from(agent));
            }
        }
    }
}

fn attach_downloads(app: &AppHandle, core: &ICoreWebView2, tab: u32) -> windows::core::Result<()> {
    let Ok(core4) = core.cast::<ICoreWebView2_4>() else {
        return Ok(());
    };
    let download_app = app.clone();
    let download_core = core.clone();
    let mut token = 0i64;
    unsafe {
        core4.add_DownloadStarting(
            &DownloadStartingEventHandler::create(Box::new(move |_, args| {
                if let Some(args) = args {
                    if let Err(error) =
                        downloads::on_download_starting(&download_app, &download_core, args, tab)
                    {
                        eprintln!("download: {error}");
                    }
                }
                Ok(())
            })),
            &mut token,
        )
    }
}

/// The page menu's Inspect row opens the docked inspector, the same one Ctrl+Shift+I uses.
fn redirect_inspect(
    app: &AppHandle,
    environment: &ICoreWebView2Environment9,
    args: &ICoreWebView2ContextMenuRequestedEventArgs,
    id: u32,
) -> windows::core::Result<()> {
    unsafe {
        let items = args.MenuItems()?;
        let mut count = 0u32;
        items.Count(&mut count)?;
        for index in 0..count {
            let item = items.GetValueAtIndex(index)?;
            if !is_inspect_item(&item) {
                continue;
            }
            let label = read_pwstr(|value| item.Label(value));
            let label = if label.is_empty() {
                "Inspect".to_string()
            } else {
                label
            };
            drop(item);
            items.RemoveValueAtIndex(index)?;
            let menu_app = app.clone();
            let replacement = menu_command(environment, &label, move || {
                tabs::inspect_page(&menu_app, id);
            })?;
            items.InsertValueAtIndex(index, &replacement)?;
            return Ok(());
        }
        Ok(())
    }
}

/// A menu entry that runs `action` on a worker thread when chosen.
fn menu_command(
    environment: &ICoreWebView2Environment9,
    label: &str,
    action: impl Fn() + Send + Sync + 'static,
) -> windows::core::Result<ICoreWebView2ContextMenuItem> {
    let action = Arc::new(action);
    unsafe {
        let item = environment.CreateContextMenuItem(
            &HSTRING::from(label),
            None::<&IStream>,
            COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_COMMAND,
        )?;
        let mut token = 0i64;
        item.add_CustomItemSelected(
            &CustomItemSelectedEventHandler::create(Box::new(move |_, _| {
                let action = action.clone();
                std::thread::spawn(move || action());
                Ok(())
            })),
            &mut token,
        )?;
        Ok(item)
    }
}

/// Copy and bookmark on a plain page click. They sit above Inspect.
fn add_plain_page_items(
    app: &AppHandle,
    environment: &ICoreWebView2Environment9,
    args: &ICoreWebView2ContextMenuRequestedEventArgs,
    id: u32,
) -> windows::core::Result<()> {
    let Some(page) = plain_web_page(args)? else {
        return Ok(());
    };
    let copy_app = app.clone();
    let page_url = page.clone();
    let copy = menu_command(environment, "Copy page address", move || {
        crate::page_menu::copy_page_address(&copy_app, id, &page_url);
    })?;
    let bookmark_app = app.clone();
    let bookmark = menu_command(environment, "Bookmark this page", move || {
        crate::page_menu::bookmark_page(&bookmark_app, id);
    })?;
    unsafe {
        let items = args.MenuItems()?;
        let mut count = 0u32;
        items.Count(&mut count)?;
        let inspect = (0..count).find(|&index| {
            items
                .GetValueAtIndex(index)
                .is_ok_and(|item| is_inspect_item(&item))
        });
        let (insert_at, above_separator) = match inspect {
            Some(index) if index > 0 && item_is_separator(&items, index - 1) => (index - 1, true),
            Some(index) => (index, false),
            None => (count, false),
        };
        if above_separator {
            items.InsertValueAtIndex(insert_at, &copy)?;
            items.InsertValueAtIndex(insert_at + 1, &bookmark)?;
        } else {
            let separator = environment.CreateContextMenuItem(
                &HSTRING::new(),
                None::<&IStream>,
                COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_SEPARATOR,
            )?;
            items.InsertValueAtIndex(insert_at, &separator)?;
            items.InsertValueAtIndex(insert_at + 1, &copy)?;
            items.InsertValueAtIndex(insert_at + 2, &bookmark)?;
        }
        Ok(())
    }
}

/// WebView2 names this row `inspectElement`. Older runtimes used `inspect`.
fn is_inspect_item(item: &ICoreWebView2ContextMenuItem) -> bool {
    let name = read_pwstr(|value| unsafe { item.Name(value) });
    name.eq_ignore_ascii_case("inspect") || name.eq_ignore_ascii_case("inspectElement")
}

fn item_is_separator(items: &ICoreWebView2ContextMenuItemCollection, index: u32) -> bool {
    unsafe {
        let Ok(item) = items.GetValueAtIndex(index) else {
            return false;
        };
        let mut kind = COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND::default();
        item.Kind(&mut kind).is_ok() && kind == COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_SEPARATOR
    }
}

/// An `http` or `https` page click that is not a link, a selection, an image, or a text field.
fn plain_web_page(
    args: &ICoreWebView2ContextMenuRequestedEventArgs,
) -> windows::core::Result<Option<String>> {
    unsafe {
        let target = args.ContextMenuTarget()?;
        if read_bool(|value| target.IsEditable(value)) {
            return Ok(None);
        }
        if read_bool(|value| target.HasLinkUri(value))
            && !read_pwstr(|value| target.LinkUri(value)).is_empty()
        {
            return Ok(None);
        }
        if read_bool(|value| target.HasSelection(value))
            && !read_pwstr(|value| target.SelectionText(value))
                .trim()
                .is_empty()
        {
            return Ok(None);
        }
        let mut kind = COREWEBVIEW2_CONTEXT_MENU_TARGET_KIND::default();
        let _ = target.Kind(&mut kind);
        if kind == COREWEBVIEW2_CONTEXT_MENU_TARGET_KIND_IMAGE
            && read_bool(|value| target.HasSourceUri(value))
            && !read_pwstr(|value| target.SourceUri(value)).is_empty()
        {
            return Ok(None);
        }
        let page = read_pwstr(|value| target.PageUri(value));
        if page.starts_with("http://") || page.starts_with("https://") {
            Ok(Some(page))
        } else {
            Ok(None)
        }
    }
}

/// Page menu entries for a link, selected text, or an image.
fn add_page_items(
    app: &AppHandle,
    environment: &ICoreWebView2Environment9,
    args: &ICoreWebView2ContextMenuRequestedEventArgs,
    id: u32,
) -> windows::core::Result<()> {
    add_link_items(app, environment, args, id)?;
    unsafe {
        let target = args.ContextMenuTarget()?;
        let selection = if read_bool(|value| target.HasSelection(value)) {
            read_pwstr(|value| target.SelectionText(value))
                .trim()
                .to_string()
        } else {
            String::new()
        };
        let mut kind = COREWEBVIEW2_CONTEXT_MENU_TARGET_KIND::default();
        let _ = target.Kind(&mut kind);
        let image = (kind == COREWEBVIEW2_CONTEXT_MENU_TARGET_KIND_IMAGE
            && read_bool(|value| target.HasSourceUri(value)))
        .then(|| read_pwstr(|value| target.SourceUri(value)))
        .filter(|source| !source.is_empty());
        if selection.is_empty() && image.is_none() {
            return Ok(());
        }
        let page = read_pwstr(|value| target.PageUri(value));
        let mut location = windows::Win32::Foundation::POINT::default();
        let _ = args.Location(&mut location);
        let mut entries = Vec::new();
        if !selection.is_empty() {
            let engine = crate::settings::get(app).search_engine_name();
            let short = crate::page_menu::label_text(&selection, 28);
            let (search_app, text) = (app.clone(), selection.clone());
            entries.push(menu_command(
                environment,
                &format!("Search {engine} for \u{201c}{short}\u{201d}"),
                move || crate::page_menu::search_selection(&search_app, id, &text),
            )?);
            let (translate_app, text) = (app.clone(), selection.clone());
            let (x, y) = (location.x, location.y);
            entries.push(menu_command(
                environment,
                "Translate selection",
                move || {
                    let params = url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("tab", &id.to_string())
                        .append_pair("text", &text)
                        .finish();
                    crate::page_menu::open_popup_at(
                        &translate_app,
                        id,
                        x,
                        y,
                        "selection-translate",
                        params,
                    );
                },
            )?);
            let (link_app, text, page_url) = (app.clone(), selection.clone(), page.clone());
            if page_url.starts_with("http") {
                entries.push(menu_command(
                    environment,
                    "Copy link to highlight",
                    move || crate::page_menu::copy_link_to_text(&link_app, id, &page_url, &text),
                )?);
            }
        }
        if let Some(source) = image {
            let (ocr_app, src) = (app.clone(), source.clone());
            entries.push(menu_command(
                environment,
                "Copy text from image",
                move || crate::page_menu::copy_image_text(&ocr_app, id, &src),
            )?);
            let (lens_app, src) = (app.clone(), source.clone());
            entries.push(menu_command(
                environment,
                "Search image with Google Lens",
                move || crate::page_menu::search_image(&lens_app, id, &src),
            )?);
            let (save_app, src) = (app.clone(), source.clone());
            entries.push(menu_command(
                environment,
                "Save image to Downloads",
                move || crate::page_menu::save_image(&save_app, id, &src),
            )?);
            let (board_app, src) = (app.clone(), source);
            entries.push(menu_command(
                environment,
                "Save image to board",
                move || crate::boards::save_menu_image(&board_app, id, &src),
            )?);
        }
        let items = args.MenuItems()?;
        let mut count = 0u32;
        items.Count(&mut count)?;
        let separator = environment.CreateContextMenuItem(
            &HSTRING::new(),
            None::<&IStream>,
            COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_SEPARATOR,
        )?;
        items.InsertValueAtIndex(count, &separator)?;
        for (offset, entry) in entries.iter().enumerate() {
            items.InsertValueAtIndex(count + 1 + offset as u32, entry)?;
        }
        Ok(())
    }
}

fn add_link_items(
    app: &AppHandle,
    environment: &ICoreWebView2Environment9,
    args: &ICoreWebView2ContextMenuRequestedEventArgs,
    id: u32,
) -> windows::core::Result<()> {
    unsafe {
        let target = args.ContextMenuTarget()?;
        if !read_bool(|value| target.HasLinkUri(value)) {
            return Ok(());
        }
        let link = read_pwstr(|value| target.LinkUri(value));
        if link.is_empty() {
            return Ok(());
        }
        let items = args.MenuItems()?;
        if link.starts_with("http://") || link.starts_with("https://") {
            let item = environment.CreateContextMenuItem(
                &HSTRING::from("Open link in peek"),
                None::<&IStream>,
                COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_COMMAND,
            )?;
            let peek_app = app.clone();
            let peek_link = link.clone();
            let mut token = 0i64;
            item.add_CustomItemSelected(
                &CustomItemSelectedEventHandler::create(Box::new(move |_, _| {
                    let app = peek_app.clone();
                    let link = peek_link.clone();
                    tauri::async_runtime::spawn(async move {
                        crate::peek::open_for_tab(&app, id, &link);
                    });
                    Ok(())
                })),
                &mut token,
            )?;
            items.InsertValueAtIndex(0, &item)?;
        }
        let entries = [
            ("Open link in new tab", false),
            ("Open link in background tab", true),
        ];
        for (index, (label, background)) in entries.into_iter().enumerate() {
            let item = environment.CreateContextMenuItem(
                &HSTRING::from(label),
                None::<&IStream>,
                COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_COMMAND,
            )?;
            let item_app = app.clone();
            let link = link.clone();
            let mut token = 0i64;
            item.add_CustomItemSelected(
                &CustomItemSelectedEventHandler::create(Box::new(move |_, _| {
                    let app = item_app.clone();
                    let link = link.clone();
                    tauri::async_runtime::spawn(async move {
                        let Ok(url) = url::Url::parse(&link) else {
                            return;
                        };
                        let _ = tabs::open_beside(&app, id, url, background);
                    });
                    Ok(())
                })),
                &mut token,
            )?;
            items.InsertValueAtIndex(index as u32, &item)?;
        }
        Ok(())
    }
}

fn attach_accelerators(
    app: &AppHandle,
    controller: &ICoreWebView2Controller,
    target: Target,
) -> windows::core::Result<()> {
    let app = app.clone();
    let mut token = 0i64;
    unsafe {
        controller.add_AcceleratorKeyPressed(
            &AcceleratorKeyPressedEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else {
                    return Ok(());
                };
                let mut kind = COREWEBVIEW2_KEY_EVENT_KIND::default();
                args.KeyEventKind(&mut kind)?;
                if kind != COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN
                    && kind != COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN
                {
                    return Ok(());
                }
                let mut key = 0u32;
                args.VirtualKey(&mut key)?;
                if key == 0x1B && !key_down(VK_CONTROL) && !key_down(VK_MENU) {
                    // Stop before the page sees Escape, so a reply already chosen cannot
                    // click or type over the key the user just pressed.
                    let stopped_agent = crate::automation::agent::stop_focused(&app);
                    if matches!(target, shortcuts::Target::Ui | shortcuts::Target::Page) {
                        let windows = popup::passive_open(&app);
                        if !windows.is_empty() {
                            args.SetHandled(true)?;
                            let app = app.clone();
                            tauri::async_runtime::spawn(async move {
                                for window in windows {
                                    popup::close_all(&app, &window, false);
                                }
                            });
                            return Ok(());
                        }
                        if stopped_agent {
                            args.SetHandled(true)?;
                            return Ok(());
                        }
                    }
                }
                let action = shortcuts::action_for(
                    key,
                    key_down(VK_CONTROL),
                    key_down(VK_SHIFT),
                    key_down(VK_MENU),
                    target,
                );
                if let Some(action) = action {
                    args.SetHandled(true)?;
                    shortcuts::dispatch(&app, action);
                }
                Ok(())
            })),
            &mut token,
        )
    }
}

pub fn run_script(webview: &Webview, script: &str) {
    let script = script.to_string();
    let _ = webview.with_webview(move |platform| {
        let Ok(core) = core(&platform) else {
            return;
        };
        let _ = unsafe {
            core.ExecuteScript(
                &HSTRING::from(script),
                None::<&ICoreWebView2ExecuteScriptCompletedHandler>,
            )
        };
    });
}

fn with_core(
    webview: &Webview,
    work: impl FnOnce(&ICoreWebView2) -> windows::core::Result<()> + Send + 'static,
) {
    let _ = webview.with_webview(move |platform| {
        if let Ok(core) = core(&platform) {
            let _ = work(&core);
        }
    });
}

/// Drops a staged file onto the handler page's file input once that page has loaded.
/// The page only accepts a picked or dropped file, and a remote page cannot read a local path.
pub fn attach_local_file(
    webview: &Webview,
    fetch: &str,
    file_name: &str,
    input_id: &str,
    content_type: &str,
) {
    let script = file_attach_script(fetch, file_name, input_id, content_type);
    let _ = webview.with_webview(move |platform| {
        let Ok(core) = core(&platform) else {
            return;
        };
        let script = HSTRING::from(script);
        let _ = unsafe {
            core.ExecuteScript(&script, None::<&ICoreWebView2ExecuteScriptCompletedHandler>)
        };
    });
}

fn file_attach_script(fetch: &str, file_name: &str, input_id: &str, content_type: &str) -> String {
    let url = serde_json::to_string(fetch).unwrap_or_else(|_| "\"\"".to_string());
    let name = serde_json::to_string(file_name).unwrap_or_else(|_| "\"document\"".to_string());
    let mime = serde_json::to_string(content_type)
        .unwrap_or_else(|_| "\"application/octet-stream\"".to_string());
    let input = serde_json::to_string(input_id).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        r#"(function() {{
  if (window.__ceFileBusy) return;
  window.__ceFileBusy = true;
  var name = {name};
  var url = {url};
  var mime = {mime};
  var inputId = {input};
  var started = Date.now();
  function giveUp() {{ window.__ceFileBusy = false; }}
  function findInput() {{
    if (inputId) return document.getElementById(inputId);
    return document.querySelector('input[type="file"]');
  }}
  function tick() {{
    var input = findInput();
    if (!input) {{
      if (Date.now() - started < 20000) setTimeout(tick, 200);
      else giveUp();
      return;
    }}
    fetch(url).then(function(response) {{
      if (!response.ok) throw new Error("missing");
      return response.blob();
    }}).then(function(blob) {{
      var file = new File([blob], name, {{ type: mime }});
      var transfer = new DataTransfer();
      transfer.items.add(file);
      input.files = transfer.files;
      input.dispatchEvent(new Event("change", {{ bubbles: true }}));
    }}).catch(function() {{
      if (Date.now() - started < 20000) setTimeout(tick, 500);
      else giveUp();
    }});
  }}
  tick();
}})();"#
    )
}

pub fn set_muted(webview: &Webview, muted: bool) {
    with_core(webview, move |core| unsafe {
        core.cast::<ICoreWebView2_8>()?.SetIsMuted(muted)
    });
}

/// Hides the page and asks WebView2 to suspend it. `tabs::finish_sleep` gets the answer.
pub fn suspend(app: &AppHandle, webview: &Webview, id: u32) {
    let app = app.clone();
    let _ = webview.with_webview(move |platform| {
        let controller = platform.controller();
        let started = (|| -> windows::core::Result<()> {
            let core3 = core(&platform)?.cast::<ICoreWebView2_3>()?;
            unsafe { controller.SetIsVisible(false)? };
            let done_app = app.clone();
            let handler = TrySuspendCompletedHandler::create(Box::new(move |result, suspended| {
                let app = done_app.clone();
                let suspended = result.is_ok() && suspended;
                tauri::async_runtime::spawn(async move { tabs::finish_sleep(&app, id, suspended) });
                Ok(())
            }));
            unsafe { core3.TrySuspend(&handler) }
        })();
        if started.is_err() {
            let app = app.clone();
            tauri::async_runtime::spawn(async move { tabs::finish_sleep(&app, id, false) });
        }
    });
}

/// Makes a hidden page visible again. WebView2 resumes a suspended page when it becomes visible.
pub fn wake(webview: &Webview) {
    let _ = webview.with_webview(move |platform| {
        let _ = unsafe { platform.controller().SetIsVisible(true) };
    });
}

/// Shortcuts for the peek's two views. Esc closes the peek.
pub fn attach_peek_hooks(app: &AppHandle, webview: &Webview, window: &str, page: bool) {
    let app = app.clone();
    let window = window.to_string();
    let _ = webview.with_webview(move |platform| {
        if let Err(error) = attach_accelerators(&app, &platform.controller(), Target::Peek) {
            eprintln!("WebView2 shortcuts for the peek: {error}");
        }
        if !page {
            return;
        }
        let Ok(core) = core(&platform) else {
            return;
        };
        let _ = attach_downloads(&app, &core, 0);
        let mut token = 0i64;
        let source_app = app.clone();
        let source_window = window.clone();
        let _ = unsafe {
            core.add_SourceChanged(
                &SourceChangedEventHandler::create(Box::new(move |sender, _| {
                    if let Some(core) = sender {
                        let source = read_pwstr(|value| core.Source(value));
                        if !source.is_empty() {
                            let app = source_app.clone();
                            let window = source_window.clone();
                            tauri::async_runtime::spawn(async move {
                                crate::peek::update_url(&app, &window, &source);
                            });
                        }
                    }
                    Ok(())
                })),
                &mut token,
            )
        };
        if let Ok(core15) = core.cast::<ICoreWebView2_15>() {
            let favicon_app = app.clone();
            let favicon_window = window.clone();
            let _ = unsafe {
                core15.add_FaviconChanged(
                    &FaviconChangedEventHandler::create(Box::new(move |sender, _| {
                        let favicon = sender
                            .and_then(|core| core.cast::<ICoreWebView2_15>().ok())
                            .map(|core| read_pwstr(|value| core.FaviconUri(value)))
                            .filter(|uri| !uri.is_empty());
                        let app = favicon_app.clone();
                        let window = favicon_window.clone();
                        tauri::async_runtime::spawn(async move {
                            crate::peek::update_favicon(&app, &window, favicon);
                        });
                        Ok(())
                    })),
                    &mut token,
                )
            };
        }
    });
}

pub fn go_back(webview: &Webview) {
    with_core(webview, |core| unsafe { core.GoBack() });
}

pub fn go_forward(webview: &Webview) {
    with_core(webview, |core| unsafe { core.GoForward() });
}

pub fn stop(webview: &Webview) {
    with_core(webview, |core| unsafe { core.Stop() });
}

/// Drops this origin's cache storage and service worker, then reloads the page without the
/// HTTP cache. WebView2 has no way to delete HTTP cache entries for one origin, so the reload
/// fetches this page again and replaces the entries it uses. Cookies and site storage stay.
pub fn clear_site_cache(webview: &Webview, origin: &str) -> std::result::Result<(), String> {
    let params = serde_json::json!({
        "origin": origin,
        "storageTypes": "appcache,cache_storage,shader_cache,service_workers",
    })
    .to_string();
    const FAILURE: &str = "Couldn't clear the cache for this site.";
    let timeout = Duration::from_secs(10);
    call_cdp_json(
        webview,
        "Storage.clearDataForOrigin",
        &params,
        timeout,
        FAILURE,
    )?;
    if call_cdp_json(
        webview,
        "Page.reload",
        r#"{"ignoreCache":true}"#,
        timeout,
        FAILURE,
    )
    .is_err()
    {
        webview.reload().map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Runs a DevTools Protocol method and returns its JSON result. Waits on the main thread,
/// so call it from a worker thread. `failure` starts every error message.
pub fn call_cdp_json(
    webview: &Webview,
    method: &str,
    params: &str,
    timeout: Duration,
    failure: &str,
) -> std::result::Result<serde_json::Value, String> {
    let (tx, rx) = mpsc::channel::<std::result::Result<String, String>>();
    let method = method.to_string();
    let params = params.to_string();
    let message = failure.to_string();
    webview
        .with_webview(move |platform| {
            let core = match core(&platform) {
                Ok(core) => core,
                Err(error) => {
                    let _ = tx.send(Err(format!("{message} {error}")));
                    return;
                }
            };
            let callback_tx = tx.clone();
            let callback_message = message.clone();
            let handler = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(
                move |result, body| {
                    let outcome = match result {
                        Ok(()) => Ok(body),
                        Err(error) => Err(cdp_error(&callback_message, &error, &body)),
                    };
                    let _ = callback_tx.send(outcome);
                    Ok(())
                },
            ));
            let method = HSTRING::from(method);
            let params = HSTRING::from(params);
            if let Err(error) =
                unsafe { core.CallDevToolsProtocolMethod(&method, &params, &handler) }
            {
                let _ = tx.send(Err(format!("{message} {error}")));
            }
        })
        .map_err(|error| error.to_string())?;
    let body = rx
        .recv_timeout(timeout)
        .map_err(|_| format!("{failure} The page did not answer in time."))??;
    if body.trim().is_empty() {
        return Ok(serde_json::Value::Null);
    }
    serde_json::from_str(&body).map_err(|error| format!("{failure} {error}"))
}

fn cdp_error(failure: &str, error: &windows::core::Error, body: &str) -> String {
    let detail = error.to_string();
    let body = body.trim();
    if body.is_empty() || body == "{}" {
        format!("{failure} {detail}")
    } else {
        format!("{failure} {detail} {body}")
    }
}

/// Evaluates `expression` in the page's main world and returns its value. Promises are awaited.
pub fn evaluate(
    webview: &Webview,
    expression: &str,
    timeout: Duration,
    failure: &str,
) -> std::result::Result<serde_json::Value, String> {
    let params = serde_json::json!({
        "expression": expression,
        "returnByValue": true,
        "awaitPromise": true,
        "userGesture": false,
    })
    .to_string();
    let mut response = call_cdp_json(webview, "Runtime.evaluate", &params, timeout, failure)?;
    if let Some(details) = response.get("exceptionDetails") {
        let text = details
            .pointer("/exception/description")
            .or_else(|| details.get("text"))
            .and_then(|value| value.as_str())
            .unwrap_or("The page script failed.");
        return Err(format!("{failure} {text}"));
    }
    Ok(response
        .pointer_mut("/result/value")
        .map(serde_json::Value::take)
        .unwrap_or(serde_json::Value::Null))
}

/// Page layout for `print_to_pdf`. Sizes are inches.
pub struct PdfLayout {
    pub page_width: f64,
    pub page_height: f64,
    pub landscape: bool,
    pub margin: f64,
    pub backgrounds: bool,
    pub header_footer: bool,
    /// Print `current/total` in `footer_template` instead of WebView2's fixed footer.
    pub page_numbers: bool,
    pub header_template: String,
    pub footer_template: String,
    pub title: String,
    pub url: String,
    pub scale: f64,
}

fn print_settings(
    platform: &PlatformWebview,
    layout: &PdfLayout,
) -> windows::core::Result<ICoreWebView2PrintSettings> {
    let environment = platform.environment().cast::<ICoreWebView2Environment6>()?;
    unsafe {
        let settings = environment.CreatePrintSettings()?;
        settings.SetOrientation(if layout.landscape {
            COREWEBVIEW2_PRINT_ORIENTATION_LANDSCAPE
        } else {
            COREWEBVIEW2_PRINT_ORIENTATION_PORTRAIT
        })?;
        settings.SetPageWidth(layout.page_width)?;
        settings.SetPageHeight(layout.page_height)?;
        settings.SetMarginTop(layout.margin)?;
        settings.SetMarginBottom(layout.margin)?;
        settings.SetMarginLeft(layout.margin)?;
        settings.SetMarginRight(layout.margin)?;
        settings.SetScaleFactor(layout.scale.clamp(0.1, 2.0))?;
        settings.SetShouldPrintBackgrounds(layout.backgrounds)?;
        settings.SetShouldPrintHeaderAndFooter(layout.header_footer)?;
        if layout.header_footer {
            settings.SetHeaderTitle(&HSTRING::from(layout.title.as_str()))?;
            settings.SetFooterUri(&HSTRING::from(layout.url.as_str()))?;
        }
        Ok(settings)
    }
}

/// Saves what the webview shows as a PDF at `path`. Waits on the main thread.
pub fn print_to_pdf(
    webview: &Webview,
    path: &std::path::Path,
    layout: PdfLayout,
) -> std::result::Result<(), String> {
    if layout.page_numbers {
        return print_to_pdf_numbered(webview, path, &layout);
    }
    const FAILURE: &str = "Couldn't save the PDF.";
    let (tx, rx) = mpsc::channel::<std::result::Result<(), String>>();
    let path = HSTRING::from(path.as_os_str());
    webview
        .with_webview(move |platform| {
            let started = (|| -> windows::core::Result<()> {
                let core = core(&platform)?.cast::<ICoreWebView2_7>()?;
                let settings = print_settings(&platform, &layout)?;
                let callback_tx = tx.clone();
                let handler = PrintToPdfCompletedHandler::create(Box::new(move |result, saved| {
                    let outcome = match result {
                        Ok(()) if saved => Ok(()),
                        Ok(()) => Err(FAILURE.to_string()),
                        Err(error) => Err(format!("{FAILURE} {error}")),
                    };
                    let _ = callback_tx.send(outcome);
                    Ok(())
                }));
                unsafe { core.PrintToPdf(&path, &settings, &handler) }
            })();
            if let Err(error) = started {
                let _ = tx.send(Err(format!("{FAILURE} {error}")));
            }
        })
        .map_err(|error| error.to_string())?;
    rx.recv_timeout(Duration::from_secs(120))
        .map_err(|_| format!("{FAILURE} Printing took too long."))?
}

/// Page numbers need Chromium's header template, which can fill in the current page and the
/// total. WebView2's own footer always pins that number to the right, beside the URL.
/// The footer is drawn in the bottom margin, so a tight margin is opened up enough to show it.
fn print_to_pdf_numbered(
    webview: &Webview,
    path: &Path,
    layout: &PdfLayout,
) -> std::result::Result<(), String> {
    const FAILURE: &str = "Couldn't save the PDF.";
    let margin_top = if layout.header_footer {
        layout.margin.max(0.4)
    } else {
        layout.margin
    };
    let params = serde_json::json!({
        "paperWidth": layout.page_width,
        "paperHeight": layout.page_height,
        "landscape": layout.landscape,
        "marginTop": margin_top,
        "marginBottom": layout.margin.max(0.4),
        "marginLeft": layout.margin,
        "marginRight": layout.margin,
        "printBackground": layout.backgrounds,
        "scale": layout.scale.clamp(0.1, 2.0),
        "displayHeaderFooter": true,
        "headerTemplate": layout.header_template,
        "footerTemplate": layout.footer_template,
        "preferCSSPageSize": false,
    });
    let response = call_cdp_json(
        webview,
        "Page.printToPDF",
        &params.to_string(),
        Duration::from_secs(120),
        FAILURE,
    )?;
    let data = response
        .get("data")
        .and_then(|value| value.as_str())
        .filter(|data| !data.is_empty())
        .ok_or_else(|| format!("{FAILURE} The page did not return a PDF."))?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|error| format!("{FAILURE} {error}"))?;
    if bytes.is_empty() {
        return Err(format!("{FAILURE} The page did not return a PDF."));
    }
    std::fs::write(path, bytes).map_err(|error| format!("{FAILURE} {error}"))
}

/// Opens the Windows print dialog for what the webview shows.
pub fn show_print_dialog(webview: &Webview) -> std::result::Result<(), String> {
    let (tx, rx) = mpsc::channel::<std::result::Result<(), String>>();
    webview
        .with_webview(move |platform| {
            let shown = core(&platform)
                .and_then(|core| core.cast::<ICoreWebView2_16>())
                .and_then(|core| unsafe {
                    core.ShowPrintUI(COREWEBVIEW2_PRINT_DIALOG_KIND_SYSTEM)
                });
            let _ =
                tx.send(shown.map_err(|error| format!("Couldn't open the print dialog. {error}")));
        })
        .map_err(|error| error.to_string())?;
    rx.recv_timeout(Duration::from_secs(10))
        .map_err(|_| "Couldn't open the print dialog.".to_string())?
}

/// Renderer processes in this webview's environment, with the frame URL each one is hosting.
/// Empty when the runtime does not offer process info.
pub fn renderer_frames(webview: &Webview) -> Vec<(u32, Vec<String>)> {
    let (tx, rx) = mpsc::channel();
    let posted = webview.with_webview(move |platform| {
        let Ok(environment) = platform.environment().cast::<ICoreWebView2Environment13>() else {
            let _ = tx.send(Vec::new());
            return;
        };
        let sender = tx.clone();
        let handler =
            GetProcessExtendedInfosCompletedHandler::create(Box::new(move |result, collection| {
                let rows = if result.is_ok() {
                    read_renderer_frames(collection.as_ref())
                } else {
                    Vec::new()
                };
                let _ = sender.send(rows);
                Ok(())
            }));
        if unsafe { environment.GetProcessExtendedInfos(&handler) }.is_err() {
            let _ = tx.send(Vec::new());
        }
    });
    if posted.is_err() {
        return Vec::new();
    }
    rx.recv_timeout(Duration::from_millis(800))
        .unwrap_or_default()
}

fn read_renderer_frames(
    collection: Option<&ICoreWebView2ProcessExtendedInfoCollection>,
) -> Vec<(u32, Vec<String>)> {
    let Some(collection) = collection else {
        return Vec::new();
    };
    unsafe {
        let mut count = 0u32;
        if collection.Count(&mut count).is_err() {
            return Vec::new();
        }
        let mut rows = Vec::new();
        for index in 0..count {
            let Ok(extended) = collection.GetValueAtIndex(index) else {
                continue;
            };
            let Ok(info) = extended.ProcessInfo() else {
                continue;
            };
            let mut kind = COREWEBVIEW2_PROCESS_KIND::default();
            if info.Kind(&mut kind).is_err() || kind != COREWEBVIEW2_PROCESS_KIND_RENDERER {
                continue;
            }
            let mut pid = 0i32;
            if info.ProcessId(&mut pid).is_err() || pid <= 0 {
                continue;
            }
            let sources = frame_sources(&extended);
            if sources.is_empty() {
                continue;
            }
            rows.push((pid as u32, sources));
        }
        rows
    }
}

fn frame_sources(extended: &ICoreWebView2ProcessExtendedInfo) -> Vec<String> {
    unsafe {
        let Ok(frames) = extended.AssociatedFrameInfos() else {
            return Vec::new();
        };
        let Ok(iterator) = frames.GetIterator() else {
            return Vec::new();
        };
        let mut sources = Vec::new();
        for _ in 0..64 {
            if !read_bool(|value| iterator.HasCurrent(value)) {
                break;
            }
            if let Ok(frame) = iterator.GetCurrent() {
                let source = read_pwstr(|value| frame.Source(value));
                if !source.is_empty() {
                    sources.push(source);
                }
            }
            if !read_bool(|value| iterator.MoveNext(value)) {
                break;
            }
        }
        sources
    }
}

/// The page's security state from DevTools: overall state, protocol, cipher, and the
/// certificate's names and dates. Waits on the main thread, so call it from a worker thread.
pub fn security_state(webview: &Webview) -> Option<serde_json::Value> {
    const EVENT: &str = "Security.visibleSecurityStateChanged";
    let (tx, rx) = mpsc::channel::<String>();
    let (token_tx, token_rx) = mpsc::channel::<i64>();
    webview
        .with_webview(move |platform| {
            let Ok(core) = core(&platform) else {
                return;
            };
            let Ok(receiver) =
                (unsafe { core.GetDevToolsProtocolEventReceiver(&HSTRING::from(EVENT)) })
            else {
                return;
            };
            let sender = tx.clone();
            let handler =
                DevToolsProtocolEventReceivedEventHandler::create(Box::new(move |_, args| {
                    if let Some(args) = args {
                        let _ = sender.send(read_pwstr(|value| unsafe {
                            args.ParameterObjectAsJson(value)
                        }));
                    }
                    Ok(())
                }));
            let mut token = 0i64;
            if unsafe { receiver.add_DevToolsProtocolEventReceived(&handler, &mut token) }.is_ok() {
                let _ = token_tx.send(token);
            }
        })
        .ok()?;
    let token = token_rx.recv_timeout(Duration::from_secs(2)).ok()?;
    let timeout = Duration::from_secs(2);
    let _ = call_cdp_json(webview, "Security.enable", "{}", timeout, "security");
    let state = rx.recv_timeout(Duration::from_millis(1500)).ok();
    let _ = call_cdp_json(webview, "Security.disable", "{}", timeout, "security");
    let _ = webview.with_webview(move |platform| {
        if let Ok(core) = core(&platform)
            && let Ok(receiver) =
                unsafe { core.GetDevToolsProtocolEventReceiver(&HSTRING::from(EVENT)) }
        {
            let _ = unsafe { receiver.remove_DevToolsProtocolEventReceived(token) };
        }
    });
    serde_json::from_str(&state?).ok()
}

/// Stops a DevTools screencast and any device-metrics override so the page view is the real page.
pub fn release_page_emulation(webview: &Webview) {
    let timeout = Duration::from_secs(2);
    let _ = call_cdp_json(webview, "Page.stopScreencast", "{}", timeout, "screencast");
    let _ = call_cdp_json(
        webview,
        "Emulation.clearDeviceMetricsOverride",
        "{}",
        timeout,
        "viewport",
    );
}

/// Hears DevTools start a screencast and turns that into a resize of this page instead of a
/// second rendered view. One subscription per page view.
pub fn watch_screencast(app: AppHandle, id: u32, webview: &Webview) {
    if !note_screencast_watch(id, webview.label()) {
        return;
    }
    let _ = webview.with_webview(move |platform| {
        let Ok(core) = core(&platform) else {
            return;
        };
        let Ok(receiver) = (unsafe {
            core.GetDevToolsProtocolEventReceiver(&HSTRING::from("Page.screencastFrame"))
        }) else {
            return;
        };
        let handler =
            DevToolsProtocolEventReceivedEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else {
                    return Ok(());
                };
                let raw = read_pwstr(|value| unsafe { args.ParameterObjectAsJson(value) });
                let app = app.clone();
                std::thread::spawn(move || absorb_screencast(&app, id, &raw));
                Ok(())
            }));
        let mut token = 0i64;
        let _ = unsafe { receiver.add_DevToolsProtocolEventReceived(&handler, &mut token) };
    });
}

fn note_screencast_watch(id: u32, label: &str) -> bool {
    static WATCHED: std::sync::OnceLock<Mutex<std::collections::HashMap<u32, String>>> =
        std::sync::OnceLock::new();
    let watched = WATCHED.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    let mut watched = watched.lock().expect("screencast");
    if watched.get(&id).map(String::as_str) == Some(label) {
        return false;
    }
    watched.insert(id, label.to_string());
    true
}

fn absorb_screencast(app: &AppHandle, id: u32, frame: &str) {
    static BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if BUSY.swap(true, std::sync::atomic::Ordering::AcqRel) {
        return;
    }
    let Some(webview) = crate::tabs::page_view(app, id) else {
        BUSY.store(false, std::sync::atomic::Ordering::Release);
        return;
    };
    let timeout = Duration::from_secs(2);
    let _ = call_cdp_json(&webview, "Page.stopScreencast", "{}", timeout, "screencast");
    let metrics = call_cdp_json(&webview, "Page.getLayoutMetrics", "{}", timeout, "viewport").ok();
    let frame_value = serde_json::from_str::<serde_json::Value>(frame).ok();
    let (width, height) = screencast_size(metrics.as_ref(), frame_value.as_ref());
    let _ = call_cdp_json(
        &webview,
        "Emulation.clearDeviceMetricsOverride",
        "{}",
        timeout,
        "viewport",
    );
    crate::tabs::adopt_screencast(app, id, width, height);
    BUSY.store(false, std::sync::atomic::Ordering::Release);
}

fn screencast_size(
    metrics: Option<&serde_json::Value>,
    frame: Option<&serde_json::Value>,
) -> (u32, u32) {
    css_layout_size(metrics.unwrap_or(&serde_json::Value::Null))
        .or_else(|| {
            frame.and_then(|frame| {
                let width = frame.pointer("/metadata/deviceWidth").and_then(json_u32);
                let height = frame.pointer("/metadata/deviceHeight").and_then(json_u32);
                match (width, height) {
                    (Some(width), Some(height)) if usable_viewport(width, height) => {
                        Some((width, height))
                    }
                    _ => None,
                }
            })
        })
        .unwrap_or((390, 844))
}

fn css_layout_size(value: &serde_json::Value) -> Option<(u32, u32)> {
    let width = value
        .pointer("/cssLayoutViewport/clientWidth")
        .and_then(json_u32)?;
    let height = value
        .pointer("/cssLayoutViewport/clientHeight")
        .and_then(json_u32)?;
    usable_viewport(width, height).then_some((width, height))
}

fn json_u32(value: &serde_json::Value) -> Option<u32> {
    let number = value
        .as_f64()
        .or_else(|| value.as_u64().map(|n| n as f64))?;
    (number.is_finite() && number > 0.0).then_some(number.round() as u32)
}

fn usable_viewport(width: u32, height: u32) -> bool {
    (240..=1600).contains(&width) && (240..=1600).contains(&height)
}

/// JavaScript heap of one page, used when its renderer process cannot be matched.
pub fn js_heap_used(webview: &Webview) -> Option<u64> {
    let timeout = Duration::from_millis(400);
    let _ = call_cdp_json(webview, "Performance.enable", "{}", timeout, "metrics");
    let value = call_cdp_json(webview, "Performance.getMetrics", "{}", timeout, "metrics").ok()?;
    let metrics = value.get("metrics")?.as_array()?;
    for metric in metrics {
        if metric.get("name").and_then(|name| name.as_str()) != Some("JSHeapUsedSize") {
            continue;
        }
        let amount = metric.get("value")?;
        if let Some(bytes) = amount.as_u64() {
            return Some(bytes);
        }
        if let Some(bytes) = amount.as_f64() {
            return Some(bytes.max(0.0) as u64);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{
        HostScheme, WheelBand, declares_dark_scheme, wants_light_scheme, wheel_band, wheel_steps,
    };

    #[test]
    fn a_device_sized_page_is_kept() {
        let metrics = serde_json::json!({
            "cssLayoutViewport": { "clientWidth": 390.0, "clientHeight": 844.0 }
        });
        assert_eq!(super::screencast_size(Some(&metrics), None), (390, 844));
    }

    #[test]
    fn a_full_desktop_mirror_starts_as_a_phone() {
        let metrics = serde_json::json!({
            "cssLayoutViewport": { "clientWidth": 1920.0, "clientHeight": 1080.0 }
        });
        let frame = serde_json::json!({
            "metadata": { "deviceWidth": 1920, "deviceHeight": 1080 }
        });
        assert_eq!(
            super::screencast_size(Some(&metrics), Some(&frame)),
            (390, 844)
        );
    }

    #[test]
    fn a_new_site_stays_light_only_while_windows_is_dark() {
        assert!(wants_light_scheme(true, None, "https://www.kohls.com/"));
        assert!(wants_light_scheme(
            true,
            Some(HostScheme::Light),
            "https://example.com/page"
        ));
        assert!(!wants_light_scheme(false, None, "https://www.kohls.com/"));
        assert!(!wants_light_scheme(
            true,
            Some(HostScheme::Dark),
            "https://github.com/"
        ));
        assert!(!wants_light_scheme(true, None, "ce://settings"));
        assert!(!wants_light_scheme(
            true,
            None,
            "http://localhost:1420/#/settings"
        ));
        assert!(!wants_light_scheme(
            true,
            None,
            "http://tauri.localhost/#/settings"
        ));
        assert!(!wants_light_scheme(
            true,
            None,
            "https://tauri.localhost/#/settings"
        ));
        assert!(wants_light_scheme(true, None, "http://localhost:3000/"));
        assert!(!wants_light_scheme(true, None, "file:///C:/page.html"));
        assert!(!wants_light_scheme(true, None, "not a url"));
    }

    #[test]
    fn a_page_opts_into_dark_from_the_canvas_scheme() {
        assert!(declares_dark_scheme("light dark", None::<&str>));
        assert!(declares_dark_scheme("dark light", None::<&str>));
        assert!(declares_dark_scheme("Dark", None::<&str>));
        assert!(!declares_dark_scheme("light", None::<&str>));
        assert!(!declares_dark_scheme("only", None::<&str>));
        assert!(!declares_dark_scheme("", None::<&str>));
        assert!(declares_dark_scheme("", [":root"]));
        assert!(declares_dark_scheme("", ["html"]));
        assert!(declares_dark_scheme("", ["body"]));
        assert!(declares_dark_scheme("", ["HTML, .card"]));
        assert!(!declares_dark_scheme("", ["html.dark"]));
        assert!(!declares_dark_scheme("", ["body:hover"]));
        assert!(!declares_dark_scheme("", [".theme-dark"]));
    }

    #[test]
    fn the_webview_brand_is_removed_from_client_hints() {
        let raw =
            r#""Chromium";v="126", "Microsoft Edge";v="126", "Microsoft Edge WebView2";v="126""#;
        assert_eq!(
            super::strip_webview_brand(raw),
            r#""Chromium";v="126", "Microsoft Edge";v="126""#
        );
    }

    #[test]
    fn an_edge_client_hint_is_unchanged() {
        let raw = r#""Chromium";v="126", "Microsoft Edge";v="126", "Not_A Brand";v="24""#;
        assert_eq!(super::strip_webview_brand(raw), raw);
    }

    #[test]
    fn a_webview_token_is_removed_from_the_user_agent() {
        let agent = "Mozilla/5.0 Chrome/126.0.0.0 Safari/537.36 Edg/126.0.0.0 WebView2/126.0.0.0";
        assert_eq!(
            super::strip_webview_user_agent(agent),
            "Mozilla/5.0 Chrome/126.0.0.0 Safari/537.36 Edg/126.0.0.0"
        );
    }

    #[test]
    fn an_edge_user_agent_is_left_alone() {
        let agent = "Mozilla/5.0 Chrome/126.0.0.0 Safari/537.36 Edg/126.0.0.0";
        assert_eq!(super::strip_webview_user_agent(agent), agent);
    }

    #[test]
    fn one_notch_is_one_zoom_step() {
        assert_eq!(wheel_steps(0, 120), (0, 1));
        assert_eq!(wheel_steps(0, -120), (0, -1));
    }

    #[test]
    fn a_short_delta_waits_for_the_next_notch() {
        assert_eq!(wheel_steps(0, 100), (100, 0));
        assert_eq!(wheel_steps(100, 20), (0, 1));
        assert_eq!(wheel_steps(0, -100), (-100, 0));
        assert_eq!(wheel_steps(-100, -20), (0, -1));
        assert_eq!(wheel_steps(50, -30), (20, 0));
    }

    #[test]
    fn a_large_delta_takes_several_steps_and_keeps_the_remainder() {
        assert_eq!(wheel_steps(0, 250), (10, 2));
        assert_eq!(wheel_steps(0, -250), (-10, -2));
    }

    #[test]
    fn ctrl_wheel_over_the_page_is_page_zoom() {
        assert_eq!(
            wheel_band(10.0, 200.0, 800.0, 600.0, 88.0, 0.0, 0.0, 0.0),
            WheelBand::Page
        );
        assert_eq!(
            wheel_band(10.0, 20.0, 800.0, 600.0, 88.0, 0.0, 0.0, 0.0),
            WheelBand::Header
        );
        assert_eq!(
            wheel_band(10.0, 200.0, 800.0, 600.0, 88.0, 240.0, 48.0, 22.0),
            WheelBand::Other
        );
        assert_eq!(
            wheel_band(400.0, 200.0, 800.0, 600.0, 88.0, 240.0, 48.0, 22.0),
            WheelBand::Page
        );
        assert_eq!(
            wheel_band(400.0, 590.0, 800.0, 600.0, 88.0, 240.0, 48.0, 22.0),
            WheelBand::Other
        );
        assert_eq!(
            wheel_band(10.0, 10.0, 800.0, 600.0, 0.0, 0.0, 0.0, 0.0),
            WheelBand::Page
        );
    }
}
