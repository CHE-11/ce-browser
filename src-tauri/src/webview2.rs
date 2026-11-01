use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use tauri::webview::PlatformWebview;
use tauri::{AppHandle, Webview};
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use webview2_com::*;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn, DeleteObject, HGDIOBJ, SetWindowRgn};
use windows::Win32::System::Com::IStream;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_MENU, VK_SHIFT};
use windows::Win32::UI::WindowsAndMessaging::{EnumChildWindows, GetClientRect};
use windows::core::{BOOL, HSTRING, Interface, PWSTR};

use crate::shortcuts::{self, Target};
use crate::{downloads, tabs};

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
pub fn request_cookies(core: &ICoreWebView2, url: &str, done: impl FnOnce(String) + Send + 'static) {
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

pub fn put_cookie(webview: &Webview, name: String, value: String, domain: String, path: String) -> std::result::Result<(), String> {
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

pub fn delete_cookie(webview: &Webview, name: String, domain: String, path: String) -> std::result::Result<(), String> {
    let (tx, rx) = mpsc::channel();
    let posted = webview.with_webview(move |platform| {
        let result = (|| -> windows::core::Result<()> {
            let core = core(&platform)?;
            let manager = unsafe { core.cast::<ICoreWebView2_2>()?.CookieManager()? };
            unsafe { manager.DeleteCookiesWithDomainAndPath(&HSTRING::from(&name), &HSTRING::from(&domain), &HSTRING::from(&path)) }
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

fn read_stream(stream: &IStream) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        let mut read = 0u32;
        let status = unsafe { stream.Read(buffer.as_mut_ptr().cast(), buffer.len() as u32, Some(&mut read)) };
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

/// Clips the popup's own window to a rounded rectangle. The main window is left alone.
pub fn round_popup(webview: &Webview, main: isize, radius: i32) {
    let radius = radius.max(1);
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
        let Some((width, height)) = client_size(hwnd) else {
            return;
        };
        clip_round(hwnd, radius);
        let clip = Clip {
            radius,
            width,
            height,
        };
        let _ = unsafe {
            EnumChildWindows(
                Some(hwnd),
                Some(clip_matching_child),
                LPARAM(std::ptr::addr_of!(clip) as isize),
            )
        };
    });
}

struct Clip {
    radius: i32,
    width: i32,
    height: i32,
}

unsafe extern "system" fn clip_matching_child(child: HWND, param: LPARAM) -> BOOL {
    let clip = unsafe { &*(param.0 as *const Clip) };
    if let Some((width, height)) = client_size(child) {
        if (width - clip.width).abs() <= 8 && (height - clip.height).abs() <= 8 {
            clip_round(child, clip.radius);
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

fn clip_round(hwnd: HWND, radius: i32) {
    let Some((width, height)) = client_size(hwnd) else {
        return;
    };
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

/// Closes the popup when it loses focus. Attached only once the popup is shown, so the
/// hidden-while-loading webview is not dismissed before it appears.
pub fn attach_popup_blur(app: &AppHandle, webview: &Webview, label: &str) {
    let app = app.clone();
    let label = label.to_string();
    let _ = webview.with_webview(move |platform| {
        let blur_app = app.clone();
        let mut token = 0i64;
        let handler = FocusChangedEventHandler::create(Box::new(move |_, _| {
            let app = blur_app.clone();
            let label = label.clone();
            tauri::async_runtime::spawn(async move { tabs::close_popup_label(&app, &label) });
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
                    tauri::async_runtime::spawn(async move { tabs::update_audio(&app, id, audible) });
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
                            tauri::async_runtime::spawn(async move {
                                crate::peek::request(&app, id, &url);
                            });
                        }
                    }
                    Some("ce-theme") => {
                        let color = message.get("color").and_then(|value| value.as_str()).unwrap_or("").to_string();
                        let app = message_app.clone();
                        tauri::async_runtime::spawn(async move { tabs::set_theme_color(&app, id, &color) });
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
                        if let Some(selector) = message.get("selector").and_then(|value| value.as_str()) {
                            let app = message_app.clone();
                            let selector = selector.to_string();
                            tauri::async_runtime::spawn(async move {
                                if let Some(url) = tabs::tab_page_url(&app, id) {
                                    crate::page_rules::remember_zap(&app, &url, &selector);
                                }
                            });
                        }
                    }
                    Some("ce-gesture") => {
                        if !crate::settings::get(&message_app).mouse_gestures {
                            return Ok(());
                        }
                        let direction = message.get("direction").and_then(|value| value.as_str()).unwrap_or("");
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
                    let Some(core) = sender.and_then(|core| core.cast::<ICoreWebView2_15>().ok()) else {
                        return Ok(());
                    };
                    let favicon = read_pwstr(|value| core.FaviconUri(value));
                    let favicon = (!favicon.is_empty()).then_some(favicon);
                    tabs::update_favicon(&favicon_app, id, favicon);
                    let accent_app = favicon_app.clone();
                    let handler = GetFaviconCompletedHandler::create(Box::new(move |result, stream| {
                        if result.is_ok()
                            && let Some(stream) = stream
                            && let Some(bytes) = read_stream(&stream)
                            && let Some(color) = crate::accent::average_png(&bytes)
                        {
                            let app = accent_app.clone();
                            tauri::async_runtime::spawn(async move { tabs::set_favicon_accent(&app, id, color) });
                        }
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
                        let _ = add_link_items(&menu_app, &environment, &args, id);
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
    attach_downloads(app, &core)?;
    Ok(())
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
                                && let Some(stream) = windows::Win32::UI::Shell::SHCreateMemStream(Some(&bytes))
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
                if crate::adblock::should_block(&request_app, id, &uri, is_document) {
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
                let source = read_pwstr(|value| core.Source(value));
                if source.starts_with("http") && crate::adblock::active_for(&source) {
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
                    let status = args
                        .Response()
                        .ok()
                        .and_then(|response| {
                            let mut code = 0i32;
                            response.StatusCode(&mut code).ok().map(|_| code)
                        });
                    crate::privacy::record_request(&response_app, id, &method, &uri, "request", status);
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
            let _ = unsafe { headers.SetHeader(windows::core::w!("Referer"), &HSTRING::from(value)) };
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
                    if let Err(error) = downloads::on_download_starting(&download_app, &download_core, args) {
                        eprintln!("download: {error}");
                    }
                }
                Ok(())
            })),
            &mut token,
        )
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
    call_cdp_json(webview, "Storage.clearDataForOrigin", &params, timeout, FAILURE)?;
    if call_cdp_json(webview, "Page.reload", r#"{"ignoreCache":true}"#, timeout, FAILURE).is_err() {
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
                .and_then(|core| unsafe { core.ShowPrintUI(COREWEBVIEW2_PRINT_DIALOG_KIND_SYSTEM) });
            let _ = tx.send(shown.map_err(|error| format!("Couldn't open the print dialog. {error}")));
        })
        .map_err(|error| error.to_string())?;
    rx.recv_timeout(Duration::from_secs(10))
        .map_err(|_| "Couldn't open the print dialog.".to_string())?
}
