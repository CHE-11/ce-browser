use std::sync::mpsc;
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

fn key_down(key: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY) -> bool {
    unsafe { GetKeyState(key.0 as i32) < 0 }
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

/// Hides the popup before it can paint WebView2's white default, then tells the tab state
/// the hide has landed so a page that already finished loading can be shown.
pub fn conceal_popup(app: &AppHandle, webview: &Webview, label: &str) {
    let task_app = app.clone();
    let task_label = label.to_string();
    let fallback = task_label.clone();
    let queued = webview.with_webview(move |platform| {
        let _ = unsafe { platform.controller().SetIsVisible(false) };
        tauri::async_runtime::spawn(async move { tabs::popup_concealed(&task_app, &task_label) });
    });
    if queued.is_err() {
        tabs::popup_concealed(app, &fallback);
    }
}

/// The WebView2 environment already created for this profile. A second
/// `CreateCoreWebView2EnvironmentWithOptions` for the same user-data folder
/// waits forever, which is what froze the app when another window opened.
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

        if let Ok(core15) = core.cast::<ICoreWebView2_15>() {
            let favicon_app = app.clone();
            core15.add_FaviconChanged(
                &FaviconChangedEventHandler::create(Box::new(move |sender, _| {
                    let favicon = sender
                        .and_then(|core| core.cast::<ICoreWebView2_15>().ok())
                        .map(|core| read_pwstr(|value| core.FaviconUri(value)))
                        .filter(|uri| !uri.is_empty());
                    tabs::update_favicon(&favicon_app, id, favicon);
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

    attach_downloads(app, &core)?;
    Ok(())
}

fn attach_downloads(app: &AppHandle, core: &ICoreWebView2) -> windows::core::Result<()> {
    let Ok(core4) = core.cast::<ICoreWebView2_4>() else {
        return Ok(());
    };
    let download_app = app.clone();
    let mut token = 0i64;
    unsafe {
        core4.add_DownloadStarting(
            &DownloadStartingEventHandler::create(Box::new(move |_, args| {
                if let Some(args) = args {
                    if let Err(error) = downloads::on_download_starting(&download_app, args) {
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
                        let index = tabs::snapshot(&app)
                            .tabs
                            .iter()
                            .position(|tab| tab.id == id)
                            .map(|index| index + 1);
                        let _ = tabs::open(
                            &app,
                            tabs::OpenTab {
                                background,
                                index,
                                ..tabs::OpenTab::new(url)
                            },
                        );
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
