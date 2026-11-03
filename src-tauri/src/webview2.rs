use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use tauri::webview::PlatformWebview;
use tauri::{AppHandle, Webview};
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use webview2_com::*;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    ClientToScreen, CreateRoundRectRgn, DeleteObject, HGDIOBJ, SetWindowRgn,
};
use windows::Win32::System::Com::IStream;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyState, VK_CONTROL, VK_MENU, VK_SHIFT,
};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, GA_ROOTOWNER, GWL_EXSTYLE, GetAncestor, GetClientRect, GetForegroundWindow,
    GetWindowLongPtrW, GetWindowRect, HWND_TOP, SWP_NOACTIVATE, SWP_NOZORDER, SWP_SHOWWINDOW,
    SendMessageW, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, WM_NCACTIVATE,
    WS_EX_NOACTIVATE,
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

pub fn attach_ui_hooks(app: &AppHandle, webview: &Webview) {
    let app = app.clone();
    let _ = webview.with_webview(move |platform| {
        if let Err(error) = attach_accelerators(&app, &platform.controller(), Target::Ui) {
            eprintln!("WebView2 shortcuts for the browser UI: {error}");
        }
        if let Ok(core) = core(&platform) {
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

pub fn attach_popup_hooks(app: &AppHandle, webview: &Webview, label: &str) {
    let app = app.clone();
    let label = label.to_string();
    let _ = webview.with_webview(move |platform| {
        let controller = platform.controller();
        if let Err(error) = attach_accelerators(&app, &controller, Target::Popup) {
            eprintln!("WebView2 shortcuts for {label}: {error}");
        }
        if let Ok(core) = core(&platform) {
            let _ = attach_downloads(&app, &core);
        }
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

pub fn attach_page_hooks(app: &AppHandle, webview: &Webview, id: u32) {
    let app = app.clone();
    let _ = webview.with_webview(move |platform| {
        if let Err(error) = page_hooks(&app, &platform, id) {
            eprintln!("WebView2 hooks for tab {id}: {error}");
        }
    });
}

fn page_hooks(app: &AppHandle, platform: &PlatformWebview, id: u32) -> windows::core::Result<()> {
    let controller = platform.controller();
    let core = core(platform)?;
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
                let Ok(message) = serde_json::from_str::<serde_json::Value>(&json) else {
                    return Ok(());
                };
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
                    Some("ce-zoom") => {
                        let change = message
                            .get("change")
                            .and_then(|value| value.as_str())
                            .unwrap_or("");
                        if change == "in" || change == "out" {
                            let app = message_app.clone();
                            let change = change.to_string();
                            tauri::async_runtime::spawn(async move {
                                let _ = tabs::zoom(&app, id, &change);
                            });
                        }
                    }
                    Some("ce-gesture") => {
                        if !crate::settings::get(&message_app).mouse_gestures {
                            return Ok(());
                        }
                        let direction = message
                            .get("direction")
                            .and_then(|value| value.as_str())
                            .unwrap_or("");
                        let app = message_app.clone();
                        let direction = direction.to_string();
                        tauri::async_runtime::spawn(async move {
                            let action = match direction.as_str() {
                                "left" => Some(crate::shortcuts::Action::Back),
                                "right" => Some(crate::shortcuts::Action::Forward),
                                "up" => Some(crate::shortcuts::Action::Reload),
                                "down" => Some(crate::shortcuts::Action::CloseTab),
                                _ => None,
                            };
                            if let Some(action) = action {
                                let _ = crate::shortcuts::run(&app, action);
                            }
                        });
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
                                if !tabs::is_ephemeral(&app, id) {
                                    crate::favicons::remember(&app, &page_url, &bytes);
                                }
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
    attach_downloads(app, &core)?;
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
                &DOMContentLoadedEventHandler::create(Box::new(move |_, _| {
                    tabs::set_progress(&dom_app, id, 70);
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
    static SLOTS: std::sync::OnceLock<Mutex<HashMap<u32, EarlyScriptSlot>>> = std::sync::OnceLock::new();
    SLOTS.get_or_init(Default::default)
}

/// Drops the document-start script id for a tab whose webview is going away.
pub fn forget_early_script(id: u32) {
    early_slots()
        .lock()
        .expect("adblock early script")
        .remove(&id);
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

        let environment = platform.environment();
        let request_app = app.clone();
        core.add_WebResourceRequested(
            &WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else {
                    return Ok(());
                };
                let uri = args
                    .Request()
                    .map(|request| read_pwstr(|value| request.Uri(value)))
                    .unwrap_or_default();
                if let Some(hit) = crate::rewrite::find(&uri, &crate::rewrite::list(&request_app)) {
                    match hit {
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
                let mut context = COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL;
                let _ = args.ResourceContext(&mut context);
                let is_document = context == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT;
                if crate::adblock::should_block(
                    &request_app,
                    id,
                    &uri,
                    is_document,
                    resource_kind(context),
                ) {
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
                    tabs::navigation_started(&navigation_app, id);
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
                    let status = args.Response().ok().and_then(|response| {
                        let mut code = 0i32;
                        response.StatusCode(&mut code).ok().map(|_| code)
                    });
                    crate::privacy::record_request(
                        &response_app,
                        id,
                        &method,
                        &uri,
                        "request",
                        status,
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
                let Some(name) = permission_name(kind) else {
                    return Ok(());
                };
                if let Some(allow) = crate::privacy::decision(&permission_app, &uri, name) {
                    args.SetState(if allow {
                        COREWEBVIEW2_PERMISSION_STATE_ALLOW
                    } else {
                        COREWEBVIEW2_PERMISSION_STATE_DENY
                    })?;
                } else {
                    crate::privacy::note_request(&permission_app, &uri, name);
                }
                Ok(())
            })),
            &mut token,
        )?;
    }
    Ok(())
}

fn permission_name(kind: COREWEBVIEW2_PERMISSION_KIND) -> Option<&'static str> {
    if kind == COREWEBVIEW2_PERMISSION_KIND_CAMERA {
        Some("camera")
    } else if kind == COREWEBVIEW2_PERMISSION_KIND_MICROPHONE {
        Some("microphone")
    } else if kind == COREWEBVIEW2_PERMISSION_KIND_GEOLOCATION {
        Some("location")
    } else if kind == COREWEBVIEW2_PERMISSION_KIND_NOTIFICATIONS {
        Some("notifications")
    } else {
        None
    }
}

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

fn attach_downloads(app: &AppHandle, core: &ICoreWebView2) -> windows::core::Result<()> {
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
                        downloads::on_download_starting(&download_app, &download_core, args)
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
                if key == 0x1B
                    && matches!(target, shortcuts::Target::Ui | shortcuts::Target::Page)
                    && !key_down(VK_CONTROL)
                    && !key_down(VK_MENU)
                {
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
        let _ = attach_downloads(&app, &core);
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
