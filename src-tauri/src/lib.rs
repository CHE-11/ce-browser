mod accelerator;
mod accent;
mod adblock;
mod ai;
mod answer;
mod automation;
mod backdrop;
mod boards;
mod bookmark_health;
mod bookmarks;
mod browsers;
mod capture;
mod debug;
mod devtools;
mod downloads;
mod extension_store;
mod extensions;
mod favicons;
mod files;
mod find;
mod hang_guard;
mod history;
mod history_index;
mod keywords;
mod library;
mod media;
mod microphone;
mod ocr;
mod page_dialogs;
mod page_errors;
mod page_menu;
mod page_rules;
mod password;
mod paths;
mod peek;
mod placement;
mod popup;
mod prices;
mod print;
mod privacy;
mod profile_archive;
mod profile_icons;
mod profiles;
mod rewrite;
mod secrets;
mod session;
mod settings;
mod shopping;
mod shortcuts;
mod site_info;
mod status;
mod storage;
mod sync;
mod sync_conflicts;
mod system;
mod tab_sleep;
mod tabs;
mod thumbnails;
mod tools;
mod undo;
mod update;
mod user_styles;
mod webview2;
mod what_runs;

use std::path::PathBuf;

use tauri::{AppHandle, Emitter, Manager, Webview, Window, WindowEvent};

use adblock::AdblockStats;
use bookmarks::{BookmarkNode, BookmarkTree};
use capture::PageCaptureResult;
use downloads::DownloadItem;
use extensions::ExtensionInfo;
use history::HistoryItem;
use popup::{PopupRequest, PopupShow};
use print::{ExtractedPage, PdfOptions};
use profiles::{ProfileIconChoice, ProfileInfo};
use settings::BrowserSettings;
use sync::SyncStatus;
use tabs::TabsSnapshot;
use what_runs::PageTechnologies;

/// The browser window a command came from. A popup's command acts on the window it belongs to.
fn browser_window(window: &Window) -> String {
    popup::owner_of(window.label())
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| error.to_string())?
}

// Tabs and navigation. These are async: creating or moving a page view waits on the main thread.

#[tauri::command]
async fn list_tabs(app: AppHandle) -> TabsSnapshot {
    tabs::snapshot(&app)
}

#[tauri::command]
async fn create_tab(
    app: AppHandle,
    window: Window,
    url: Option<String>,
    background: Option<bool>,
    index: Option<usize>,
) -> Result<u32, String> {
    tabs::create_tab_on(
        &app,
        &browser_window(&window),
        url,
        background.unwrap_or(false),
        index,
    )
}

#[tauri::command]
async fn close_tab(app: AppHandle, id: u32) -> Result<(), String> {
    tabs::close_tab(&app, id)
}

#[tauri::command]
async fn close_selected_tabs(app: AppHandle) -> Result<(), String> {
    tabs::close_selected(&app)
}

#[tauri::command]
async fn toggle_tab_selection(app: AppHandle, id: u32) -> Result<(), String> {
    tabs::toggle_selected(&app, id)
}

#[tauri::command]
async fn select_tab_range(app: AppHandle, id: u32) -> Result<(), String> {
    tabs::select_range(&app, id)
}

#[tauri::command]
async fn close_other_tabs(app: AppHandle, id: u32) -> Result<(), String> {
    tabs::close_other_tabs(&app, id)
}

#[tauri::command]
async fn close_tabs_to_right(app: AppHandle, id: u32) -> Result<(), String> {
    tabs::close_tabs_to_right(&app, id)
}

#[tauri::command]
async fn activate_tab(app: AppHandle, id: u32) -> Result<(), String> {
    tabs::activate(&app, id)
}

#[tauri::command]
async fn navigate(app: AppHandle, id: u32, input: String) -> Result<(), String> {
    tabs::navigate(&app, id, &input)
}

#[tauri::command]
async fn go_back(app: AppHandle, id: u32) {
    tabs::go_back(&app, id);
}

#[tauri::command]
async fn go_forward(app: AppHandle, id: u32) {
    tabs::go_forward(&app, id);
}

#[tauri::command]
async fn reload(app: AppHandle, id: u32) -> Result<(), String> {
    tabs::reload(&app, id)
}

#[tauri::command]
async fn clear_site_cache(app: AppHandle, id: u32) -> Result<(), String> {
    blocking(move || tabs::clear_site_cache(&app, id)).await
}

#[tauri::command]
fn get_adblock_stats(id: u32) -> Result<AdblockStats, String> {
    Ok(adblock::stats(id))
}

#[tauri::command]
async fn stop(app: AppHandle, id: u32) {
    tabs::stop(&app, id);
}

/// Ends the renderer of a page that stopped responding. The tab then shows its crash page.
#[tauri::command]
async fn end_page(app: AppHandle, id: u32) -> Result<(), String> {
    blocking(move || {
        let pids = status::renderer_pids(&app, id);
        if pids.is_empty() {
            return Err("Couldn't find this page's process.".into());
        }
        let ended = pids
            .into_iter()
            .filter(|pid| status::terminate(*pid))
            .count();
        if ended == 0 {
            return Err("Windows would not end this page's process.".into());
        }
        Ok(())
    })
    .await
}

/// Loads the failed page again, passing its certificate error for the rest of this session.
#[tauri::command]
async fn allow_certificate(app: AppHandle, id: u32) -> Result<(), String> {
    let failure = tabs::failure_of(&app, id).ok_or("This page has no certificate error.")?;
    page_errors::allow_certificate(&failure.url)?;
    tabs::navigate(&app, id, &failure.url)
}

#[tauri::command]
async fn move_tab(app: AppHandle, id: u32, index: usize) {
    tabs::move_tab(&app, id, index);
}

#[tauri::command]
async fn pin_tab(app: AppHandle, id: u32) {
    tabs::set_pinned(&app, id, true);
}

#[tauri::command]
async fn unpin_tab(app: AppHandle, id: u32) {
    tabs::set_pinned(&app, id, false);
}

#[tauri::command]
async fn restore_pinned_tabs(app: AppHandle, window: Window) -> Result<(), String> {
    tabs::restore_saved_pins(&app, &browser_window(&window))
}

#[tauri::command]
async fn duplicate_tab(app: AppHandle, id: u32) -> Result<u32, String> {
    tabs::duplicate_tab(&app, id)
}

#[tauri::command]
async fn reopen_closed_tab(app: AppHandle) -> Result<Option<u32>, String> {
    tabs::reopen_closed(&app)
}

#[tauri::command]
fn list_recently_closed(app: AppHandle) -> Vec<tabs::ClosedEntry> {
    tabs::recently_closed(&app)
}

#[tauri::command]
async fn restore_closed(app: AppHandle, id: Option<u32>) -> Result<Option<u32>, String> {
    tabs::restore_closed(&app, id)
}

#[tauri::command]
async fn tab_thumbnail(app: AppHandle, id: u32) -> Result<Option<String>, String> {
    blocking(move || Ok(thumbnails::thumbnail(&app, id))).await
}

/// Bytes one tab's page uses now. Null when it has no page view or the renderer can't be read.
#[tauri::command]
async fn tab_memory(app: AppHandle, id: u32) -> Result<Option<u64>, String> {
    blocking(move || Ok(status::tab_memory(&app, id))).await
}

#[tauri::command]
async fn undo_action(app: AppHandle, id: u32) -> Result<(), String> {
    undo::undo(&app, id)
}

#[tauri::command]
async fn dismiss_toast(app: AppHandle, window: Window, id: u32) {
    undo::dismiss(&app, &browser_window(&window), id);
}

#[tauri::command]
async fn set_ui_insets(
    app: AppHandle,
    window: Window,
    top: f64,
    left: f64,
    right: f64,
    bottom: f64,
) {
    tabs::set_ui_insets(&app, &browser_window(&window), top, left, right, bottom);
}

#[tauri::command]
async fn set_tabs_muted(app: AppHandle, ids: Vec<u32>, muted: bool) -> Result<(), String> {
    tabs::set_muted(&app, &ids, muted)
}

#[tauri::command]
async fn toggle_selected_muted(app: AppHandle) -> Result<(), String> {
    tabs::toggle_selected_muted(&app)
}

#[tauri::command]
async fn sleep_tab(app: AppHandle, id: u32) -> Result<(), String> {
    tabs::sleep_tab(&app, id)
}

#[tauri::command]
async fn sleep_other_tabs(app: AppHandle, id: u32) -> Result<(), String> {
    tabs::sleep_other_tabs(&app, id)
}

#[tauri::command]
async fn never_sleep_site(app: AppHandle, id: u32) -> Result<(), String> {
    tabs::never_sleep_site(&app, id)
}

// Split view

#[tauri::command]
async fn split_with(app: AppHandle, id: u32) -> Result<(), String> {
    tabs::split_with(&app, id)
}

#[tauri::command]
async fn open_in_split(app: AppHandle, window: Window, url: String) -> Result<u32, String> {
    tabs::open_in_split(&app, &browser_window(&window), &url)
}

#[tauri::command]
async fn swap_split(app: AppHandle, window: Window) -> Result<(), String> {
    tabs::swap_split(&app, &browser_window(&window))
}

#[tauri::command]
async fn exit_split(app: AppHandle, window: Window) -> Result<(), String> {
    tabs::exit_split(&app, &browser_window(&window))
}

#[tauri::command]
async fn set_split_ratio(app: AppHandle, window: Window, ratio: f64) {
    tabs::set_split_ratio(&app, &browser_window(&window), ratio);
}

#[tauri::command]
async fn set_devtools_ratio(app: AppHandle, window: Window, ratio: f64) {
    tabs::set_devtools_ratio(&app, &browser_window(&window), ratio);
}

// Tab groups

#[tauri::command]
async fn create_group(app: AppHandle, ids: Vec<u32>) -> Result<u32, String> {
    tabs::create_group(&app, &ids)
}

#[tauri::command]
async fn add_to_group(app: AppHandle, ids: Vec<u32>, group_id: u32) -> Result<(), String> {
    tabs::add_to_group(&app, &ids, group_id)
}

#[tauri::command]
async fn remove_from_group(app: AppHandle, ids: Vec<u32>) -> Result<(), String> {
    tabs::remove_from_group(&app, &ids)
}

#[tauri::command]
async fn update_group(
    app: AppHandle,
    id: u32,
    name: Option<String>,
    color: Option<String>,
    collapsed: Option<bool>,
) -> Result<(), String> {
    tabs::update_group(&app, id, name, color, collapsed)
}

#[tauri::command]
async fn ungroup(app: AppHandle, id: u32) -> Result<(), String> {
    tabs::ungroup(&app, id)
}

#[tauri::command]
async fn close_group(app: AppHandle, id: u32) -> Result<(), String> {
    tabs::close_group(&app, id)
}

#[tauri::command]
async fn new_tab_in_group(app: AppHandle, id: u32) -> Result<u32, String> {
    tabs::new_tab_in_group(&app, id)
}

#[tauri::command]
async fn save_group_to_bookmarks(app: AppHandle, id: u32) -> Result<BookmarkNode, String> {
    let (name, links) = tabs::group_links(&app, id)?;
    bookmarks::add_folder_of_links(&app, &name, &links)
}

#[tauri::command]
async fn open_folder_as_group(app: AppHandle, window: Window, id: String) -> Result<u32, String> {
    let (title, links) = bookmarks::folder_links(&app, &id)?;
    tabs::open_as_group(&app, &browser_window(&window), &title, &links)
}

#[tauri::command]
async fn group_by_site(app: AppHandle, window: Window) -> Result<usize, String> {
    tabs::group_by_site(&app, &browser_window(&window))
}

/// Ctrl is down right now. The tab switcher closes when it is released.
#[tauri::command]
fn ctrl_held() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL};
    unsafe { GetAsyncKeyState(i32::from(VK_CONTROL.0)) < 0 }
}

// Workspaces

fn workspaces_on(app: &AppHandle) -> Result<(), String> {
    if settings::get(app).workspaces_enabled {
        Ok(())
    } else {
        Err("Workspaces are turned off in Settings.".into())
    }
}

#[tauri::command]
async fn create_workspace(app: AppHandle, name: String, color: String) -> Result<String, String> {
    workspaces_on(&app)?;
    tabs::create_workspace(&app, &name, &color)
}

#[tauri::command]
async fn rename_workspace(
    app: AppHandle,
    id: String,
    name: String,
    color: String,
) -> Result<(), String> {
    workspaces_on(&app)?;
    tabs::rename_workspace(&app, &id, &name, &color)
}

#[tauri::command]
async fn delete_workspace(app: AppHandle, id: String) -> Result<(), String> {
    workspaces_on(&app)?;
    tabs::delete_workspace(&app, &id)
}

#[tauri::command]
async fn switch_workspace(app: AppHandle, window: Window, id: String) -> Result<(), String> {
    workspaces_on(&app)?;
    tabs::switch_workspace(&app, &browser_window(&window), &id)
}

#[tauri::command]
async fn move_tabs_to_workspace(
    app: AppHandle,
    ids: Vec<u32>,
    workspace: String,
) -> Result<(), String> {
    workspaces_on(&app)?;
    tabs::move_tabs_to_workspace(&app, &ids, &workspace)
}

// Peek

#[tauri::command]
fn peek_state(app: AppHandle, window: Window) -> Option<peek::PeekInfo> {
    peek::info(&app, &browser_window(&window))
}

#[tauri::command]
async fn close_peek(app: AppHandle, window: Window) {
    peek::close(&app, &browser_window(&window));
}

#[tauri::command]
async fn peek_to_tab(app: AppHandle, window: Window) -> Result<u32, String> {
    peek::to_tab(&app, &browser_window(&window))
}

#[tauri::command]
async fn peek_to_split(app: AppHandle, window: Window) -> Result<u32, String> {
    peek::to_split(&app, &browser_window(&window))
}

#[tauri::command]
async fn zoom_tab(app: AppHandle, id: u32, change: String) -> Result<f64, String> {
    tabs::zoom(&app, id, &change)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn find_in_page(
    app: AppHandle,
    id: u32,
    query: String,
    backwards: bool,
    case_sensitive: Option<bool>,
    whole_word: Option<bool>,
    step: Option<bool>,
) -> Result<find::FindResult, String> {
    blocking(move || {
        find::find(
            &app,
            id,
            &query,
            backwards,
            case_sensitive.unwrap_or(false),
            whole_word.unwrap_or(false),
            step.unwrap_or(true),
        )
    })
    .await
}

#[tauri::command]
async fn clear_find(app: AppHandle, id: u32) -> Result<(), String> {
    blocking(move || {
        find::clear(&app, id);
        Ok(())
    })
    .await
}

#[tauri::command]
async fn find_in_tabs(
    app: AppHandle,
    window: Window,
    query: String,
    case_sensitive: bool,
    whole_word: bool,
) -> Result<Vec<find::FindTabResult>, String> {
    let label = browser_window(&window);
    blocking(move || {
        Ok(find::find_in_tabs(
            &app,
            &label,
            &query,
            case_sensitive,
            whole_word,
        ))
    })
    .await
}

#[tauri::command]
async fn open_devtools(app: AppHandle, id: u32) {
    tabs::open_devtools(&app, id);
}

#[tauri::command]
async fn open_ui_devtools(app: AppHandle, window: Window) {
    let label = browser_window(&window);
    tabs::open_ui_devtools(&app, &label);
}

#[tauri::command]
async fn open_popup(
    app: AppHandle,
    webview: Webview,
    request: PopupRequest,
) -> Result<bool, String> {
    popup::open(&app, &webview, request)
}

/// `scope` is `this` for the popup that asked and those opened from it, `above` for only
/// those opened from it, and anything else for every popup of the window.
#[tauri::command]
async fn close_popup(app: AppHandle, webview: Webview, scope: Option<String>) {
    let label = webview.label();
    match scope.as_deref() {
        Some("this") if popup::is_popup_label(label) => popup::close_level(&app, label),
        Some("above") if popup::is_popup_label(label) => popup::close_above(&app, label),
        Some("above") => {}
        // Popups are kept per browser window. The browser UI and page views are webviews
        // inside it whose labels (`ui`, `tab-3`) differ from the window's.
        _ if popup::is_popup_label(label) => popup::close_all(&app, label, true),
        _ => popup::close_all(&app, webview.window().label(), true),
    }
}

#[tauri::command]
async fn popup_measured(
    app: AppHandle,
    webview: Webview,
    token: u64,
    width: f64,
    height: f64,
    ready: bool,
) {
    popup::measured(&app, webview.label(), token, width, height, ready);
}

#[tauri::command]
async fn popup_current(app: AppHandle, webview: Webview) -> Option<PopupShow> {
    popup::current(&app, webview.label())
}

#[tauri::command]
async fn detach_tab(app: AppHandle, id: u32) -> Result<(), String> {
    tabs::detach_tab(&app, id)
}

#[tauri::command]
async fn open_in_window(app: AppHandle, url: String) -> Result<(), String> {
    tabs::open_url_window(&app, &url)
}

// Built-in page tools. They wait on WebView2 on the main thread, so they run on the blocking pool.

#[tauri::command]
async fn capture_page_png(
    app: AppHandle,
    id: u32,
    full_page: bool,
    load_lazy: bool,
    path: String,
) -> Result<PageCaptureResult, String> {
    blocking(move || capture::save_png(&app, id, full_page, load_lazy, &PathBuf::from(path))).await
}

#[tauri::command]
async fn copy_page_png(
    app: AppHandle,
    id: u32,
    full_page: bool,
    load_lazy: bool,
) -> Result<PageCaptureResult, String> {
    blocking(move || capture::copy_png(&app, id, full_page, load_lazy)).await
}

#[tauri::command]
async fn capture_region(app: AppHandle, id: u32) -> Result<PageCaptureResult, String> {
    blocking(move || capture::capture_region(&app, id)).await
}

#[tauri::command]
async fn store_visible_png(app: AppHandle, id: u32) -> Result<PageCaptureResult, String> {
    blocking(move || capture::store_visible(&app, id)).await
}

#[tauri::command]
async fn last_capture_png() -> Result<String, String> {
    blocking(capture::last_png_base64).await
}

#[tauri::command]
async fn save_edited_png(data: String, path: String) -> Result<(), String> {
    blocking(move || {
        let bytes = capture::decode_png(&data)?;
        capture::save_edited_png(&bytes, &PathBuf::from(path))
    })
    .await
}

#[tauri::command]
async fn copy_edited_png(data: String) -> Result<(), String> {
    blocking(move || {
        let bytes = capture::decode_png(&data)?;
        capture::copy_edited_png(&bytes)
    })
    .await
}

#[tauri::command]
async fn start_recording(
    app: AppHandle,
    id: u32,
    kind: String,
    path: String,
) -> Result<(), String> {
    blocking(move || capture::start_recording(&app, id, &kind, PathBuf::from(path))).await
}

#[tauri::command]
async fn stop_recording() -> Result<(), String> {
    capture::stop_recording();
    Ok(())
}

#[tauri::command]
async fn list_media(app: AppHandle) -> Result<Vec<media::MediaElement>, String> {
    blocking(move || Ok(media::list_playing(&app))).await
}

#[tauri::command]
async fn set_all_media_muted(app: AppHandle, muted: bool) -> Result<(), String> {
    blocking(move || media::set_all_muted(&app, muted)).await
}

#[tauri::command]
async fn control_media(
    app: AppHandle,
    tab: u32,
    index: u32,
    action: String,
    time: Option<f64>,
) -> Result<(), String> {
    blocking(move || media::control(&app, tab, index, &action, time)).await
}

#[tauri::command]
async fn page_media(app: AppHandle, id: u32) -> Result<Vec<media::PageMedia>, String> {
    blocking(move || media::page_media(&app, id)).await
}

#[tauri::command]
async fn download_page_media(app: AppHandle, id: u32, urls: Vec<String>) -> Result<u32, String> {
    blocking(move || media::download_media(&app, id, &urls)).await
}

#[tauri::command]
async fn save_page_html(app: AppHandle, id: u32, path: String) -> Result<(), String> {
    blocking(move || media::save_html(&app, id, &PathBuf::from(path))).await
}

#[tauri::command]
async fn save_page_markdown(app: AppHandle, id: u32, path: String) -> Result<(), String> {
    blocking(move || media::save_markdown(&app, id, &PathBuf::from(path))).await
}

#[tauri::command]
fn write_text_file(path: String, text: String) -> Result<(), String> {
    files::write_text_file(&PathBuf::from(path), &text)
}

#[tauri::command]
async fn ocr_visible(app: AppHandle, id: u32) -> Result<String, String> {
    blocking(move || ocr::recognize_visible(&app, id)).await
}

#[tauri::command]
async fn ocr_region(app: AppHandle, id: u32) -> Result<String, String> {
    blocking(move || ocr::recognize_region(&app, id)).await
}

#[tauri::command]
async fn extract_page(app: AppHandle, id: u32) -> Result<ExtractedPage, String> {
    blocking(move || print::extract_page(&app, id)).await
}

#[tauri::command]
async fn detect_technologies(app: AppHandle, id: u32) -> Result<PageTechnologies, String> {
    blocking(move || what_runs::detect_technologies(&app, id)).await
}

#[tauri::command]
async fn print_tab_to_pdf(
    app: AppHandle,
    id: u32,
    options: PdfOptions,
    path: String,
) -> Result<(), String> {
    blocking(move || print::print_tab_to_pdf(&app, id, options, &PathBuf::from(path))).await
}

#[tauri::command]
async fn print_html_to_pdf(
    app: AppHandle,
    html: String,
    options: PdfOptions,
    path: String,
) -> Result<(), String> {
    blocking(move || print::print_html_to_pdf(&app, &html, options, &PathBuf::from(path))).await
}

#[tauri::command]
async fn print_html_dialog(app: AppHandle, html: String) -> Result<(), String> {
    blocking(move || print::print_html_dialog(&app, &html)).await
}

#[tauri::command]
async fn open_saved_file(path: String) -> Result<(), String> {
    blocking(move || print::open_file(&path)).await
}

#[tauri::command]
async fn show_saved_file(path: String) -> Result<(), String> {
    blocking(move || print::show_file(&path)).await
}

// Bookmarks

#[tauri::command]
fn list_bookmarks(app: AppHandle) -> BookmarkTree {
    bookmarks::tree(&app)
}

#[tauri::command]
fn add_bookmark(
    app: AppHandle,
    parent_id: String,
    title: String,
    url: Option<String>,
    index: Option<usize>,
    separator: Option<bool>,
) -> Result<BookmarkNode, String> {
    bookmarks::add(
        &app,
        &parent_id,
        &title,
        url,
        index,
        separator.unwrap_or(false),
    )
}

#[tauri::command]
fn update_bookmark(
    app: AppHandle,
    id: String,
    title: String,
    url: Option<String>,
) -> Result<(), String> {
    bookmarks::update(&app, &id, &title, url)
}

#[tauri::command]
fn move_bookmark(
    app: AppHandle,
    id: String,
    parent_id: String,
    index: Option<usize>,
) -> Result<(), String> {
    bookmarks::move_to(&app, &id, &parent_id, index)
}

#[tauri::command]
async fn delete_bookmark(app: AppHandle, window: Window, id: String) -> Result<(), String> {
    delete_bookmarks(app, window, vec![id]).await
}

/// Deletes bookmarks and offers one Undo for all of them.
#[tauri::command]
async fn delete_bookmarks(app: AppHandle, window: Window, ids: Vec<String>) -> Result<(), String> {
    let mut removed = Vec::new();
    let mut failure = None;
    for id in &ids {
        match bookmarks::delete_returning(&app, id) {
            Ok(entry) => removed.push(entry),
            Err(error) => {
                failure = Some(error);
                break;
            }
        }
    }
    let message = match removed.as_slice() {
        [] => None,
        [(node, _, _)] if node.separator => Some("Deleted divider".to_string()),
        [(node, _, _)] if node.url.is_none() => {
            Some(format!("Deleted folder \u{201c}{}\u{201d}", node.title))
        }
        [(node, _, _)] => Some(format!("Deleted bookmark \u{201c}{}\u{201d}", node.title)),
        many => Some(format!("Deleted {} bookmarks", many.len())),
    };
    if let Some(message) = message {
        undo::offer(
            &app,
            &browser_window(&window),
            &message,
            undo::UndoAction::Bookmarks(removed),
        );
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[tauri::command]
fn find_bookmark(app: AppHandle, url: String) -> Option<BookmarkNode> {
    bookmarks::find_by_url(&app, &url)
}

#[tauri::command]
async fn site_favicons(
    app: AppHandle,
    urls: Vec<String>,
) -> Result<std::collections::HashMap<String, String>, String> {
    blocking(move || Ok(favicons::lookup(&app, &urls))).await
}

#[tauri::command]
async fn import_bookmarks_html(app: AppHandle, path: String) -> Result<usize, String> {
    blocking(move || bookmarks::import_html(&app, &PathBuf::from(path))).await
}

#[tauri::command]
async fn export_bookmarks_html(app: AppHandle, path: String) -> Result<(), String> {
    blocking(move || bookmarks::export_html(&app, &PathBuf::from(path))).await
}

// Downloads

#[tauri::command]
fn list_downloads(app: AppHandle) -> Vec<DownloadItem> {
    downloads::list(&app)
}

#[tauri::command]
fn computer_name() -> String {
    downloads::computer_name()
}

#[tauri::command]
fn browse_local(app: AppHandle, path: Option<String>) -> Result<library::BrowseResult, String> {
    let path = path
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| settings::get(&app).download_dir(&app));
    library::browse(&path)
}

#[tauri::command]
fn open_local_file(path: String) -> Result<(), String> {
    files::open_with_default(std::path::Path::new(&path))
}

#[tauri::command]
async fn pause_download(app: AppHandle, id: u32) -> Result<(), String> {
    blocking(move || downloads::pause(&app, id)).await
}

#[tauri::command]
async fn resume_download(app: AppHandle, id: u32) -> Result<(), String> {
    blocking(move || downloads::resume(&app, id)).await
}

#[tauri::command]
async fn cancel_download(app: AppHandle, id: u32) -> Result<(), String> {
    blocking(move || downloads::cancel(&app, id)).await
}

#[tauri::command]
async fn open_download(app: AppHandle, window: Window, id: u32) -> Result<(), String> {
    let label = window.label().to_string();
    blocking(move || downloads::open(&app, id, &label)).await
}

#[tauri::command]
async fn show_download_in_folder(app: AppHandle, id: u32) -> Result<(), String> {
    blocking(move || downloads::show_in_folder(&app, id)).await
}

/// Synchronous so `DoDragDrop` runs on the UI thread, which already owns a message loop.
#[tauri::command]
fn drag_download(app: AppHandle, window: Window, id: u32) -> Result<(), String> {
    downloads::drag_file(&app, &window, id)
}

#[tauri::command]
async fn remove_download(app: AppHandle, id: u32) -> Result<(), String> {
    blocking(move || downloads::remove(&app, id)).await
}

#[tauri::command]
fn clear_downloads(app: AppHandle) {
    downloads::clear(&app);
}

// History. Saved as history.json beside this profile's other data files.

#[tauri::command]
fn list_history(app: AppHandle) -> Vec<HistoryItem> {
    history::list(&app)
}

#[tauri::command]
async fn remove_history(app: AppHandle, window: Window, id: u32) {
    if let Some(item) = history::take(&app, id) {
        undo::offer(
            &app,
            &browser_window(&window),
            "Removed from history",
            undo::UndoAction::History {
                items: vec![item],
                cleared: false,
            },
        );
    }
}

/// `since` and `until` are millisecond timestamps. A missing bound is open.
/// Both missing clears every entry.
#[tauri::command]
async fn clear_history(app: AppHandle, window: Window, since: Option<u64>, until: Option<u64>) {
    let (items, cleared) = history::take_between(&app, since, until);
    if items.is_empty() {
        return;
    }
    let message = format!(
        "Cleared {} history {}",
        items.len(),
        if items.len() == 1 { "entry" } else { "entries" }
    );
    undo::offer(
        &app,
        &browser_window(&window),
        &message,
        undo::UndoAction::History { items, cleared },
    );
}

/// `text` is what was typed into the address bar. It is resolved the way navigation resolves it.
#[tauri::command]
fn note_typed(app: AppHandle, text: String) {
    history::note_typed(tabs::resolve(&app, &text).as_str());
}

#[tauri::command]
async fn site_info(app: AppHandle, id: u32) -> Result<site_info::SiteInfo, String> {
    blocking(move || site_info::site_info(&app, id)).await
}

// Extensions. Every call waits for WebView2 on the main thread, so they run on the blocking pool.

#[tauri::command]
async fn list_extensions(app: AppHandle) -> Result<Vec<ExtensionInfo>, String> {
    blocking(move || extensions::list(&app)).await
}

#[tauri::command]
async fn install_extension_folder(app: AppHandle, path: String) -> Result<ExtensionInfo, String> {
    blocking(move || extensions::install_folder(&app, &PathBuf::from(path))).await
}

#[tauri::command]
async fn install_extension_zip(app: AppHandle, path: String) -> Result<ExtensionInfo, String> {
    blocking(move || extensions::install_zip(&app, &PathBuf::from(path))).await
}

#[tauri::command]
async fn install_extension_from_store(
    app: AppHandle,
    url: String,
) -> Result<ExtensionInfo, String> {
    blocking(move || extensions::install_from_store(&app, &url)).await
}

#[tauri::command]
async fn check_extension_updates(app: AppHandle) -> Result<Vec<ExtensionInfo>, String> {
    blocking(move || extensions::check_store_updates(&app, false)).await
}

#[tauri::command]
async fn update_extension(app: AppHandle, id: String) -> Result<ExtensionInfo, String> {
    blocking(move || extensions::update_extension(&app, &id)).await
}

#[tauri::command]
async fn set_extension_enabled(app: AppHandle, id: String, enabled: bool) -> Result<(), String> {
    blocking(move || extensions::set_enabled(&app, &id, enabled)).await
}

#[tauri::command]
async fn remove_extension(app: AppHandle, id: String) -> Result<(), String> {
    blocking(move || extensions::remove(&app, &id)).await
}

#[tauri::command]
async fn open_extension_options(app: AppHandle, id: String) -> Result<u32, String> {
    let url = extensions::options_url(&app, &id)?;
    tabs::open(&app, tabs::OpenTab::new(url))
}

// Settings

#[tauri::command]
fn list_permissions(app: AppHandle) -> Vec<privacy::PermissionGrant> {
    privacy::grants(&app)
}

#[tauri::command]
fn set_permission(
    app: AppHandle,
    origin: String,
    kind: String,
    allow: bool,
    persist: Option<bool>,
) -> Result<(), String> {
    privacy::set_permission(&app, &origin, &kind, allow, persist)
}

#[tauri::command]
fn answer_permission(app: AppHandle, id: u64, choice: String) {
    webview2::answer_permission(&app, id, &choice);
}

#[tauri::command]
async fn list_microphones() -> Result<Vec<microphone::MicrophoneDevice>, String> {
    blocking(microphone::list).await
}

#[tauri::command]
fn watch_microphone(app: AppHandle, device_id: String, epoch: u64) {
    microphone::watch(app, device_id, epoch);
}

#[tauri::command]
fn unwatch_microphone(epoch: u64) {
    microphone::unwatch(epoch);
}

#[tauri::command]
fn set_microphone_device(
    app: AppHandle,
    origin: String,
    device_id: String,
    device_label: String,
) -> Result<(), String> {
    privacy::set_microphone_device(&app, &origin, &device_id, &device_label)
}

#[tauri::command]
fn list_containers(app: AppHandle) -> Vec<privacy::Container> {
    privacy::containers(&app)
}

#[tauri::command]
fn create_container(
    app: AppHandle,
    name: String,
    color: String,
) -> Result<privacy::Container, String> {
    privacy::create_container(&app, &name, &color)
}

#[tauri::command]
fn delete_container(app: AppHandle, id: String) -> Result<(), String> {
    privacy::delete_container(&app, &id)
}

#[tauri::command]
async fn set_group_container(
    app: AppHandle,
    group_id: u32,
    container: String,
) -> Result<(), String> {
    tabs::set_group_container(&app, group_id, &container)
}

#[tauri::command]
async fn create_ephemeral_tab(app: AppHandle, window: Window) -> Result<u32, String> {
    tabs::create_ephemeral_tab(&app, &browser_window(&window))
}

#[tauri::command]
fn network_log(app: AppHandle, id: u32) -> Vec<privacy::NetworkEntry> {
    privacy::network_log(&app, id)
}

#[tauri::command]
fn clear_network_log(app: AppHandle, id: u32) {
    privacy::clear_network_log(&app, id);
}

#[tauri::command]
async fn open_network_window(app: AppHandle, window: Window, id: u32) -> Result<(), String> {
    let owner = browser_window(&window);
    privacy::open_window(&app, &owner, id)
}

#[tauri::command]
fn list_keywords(app: AppHandle) -> Vec<keywords::Keyword> {
    keywords::list(&app)
}

#[tauri::command]
fn save_keywords(
    app: AppHandle,
    keywords: Vec<keywords::Keyword>,
) -> Result<Vec<keywords::Keyword>, String> {
    keywords::save(&app, keywords)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OmniboxExtra {
    title: String,
    url: String,
    answer: bool,
}

#[tauri::command]
fn omnibox_extra(app: AppHandle, text: String) -> Option<OmniboxExtra> {
    if let Some(hit) = keywords::expand(&keywords::list(&app), &text) {
        return Some(OmniboxExtra {
            title: hit.title,
            url: hit.url,
            answer: false,
        });
    }
    answer::answer(&text).map(|answer| OmniboxExtra {
        title: answer.title,
        url: answer.url,
        answer: answer.answer,
    })
}

#[tauri::command]
fn list_shortcuts(app: AppHandle) -> Vec<shortcuts::ShortcutBinding> {
    shortcuts::list(&app)
}

#[tauri::command]
fn save_shortcuts(
    app: AppHandle,
    bindings: Vec<shortcuts::ShortcutBinding>,
) -> Result<Vec<shortcuts::ShortcutBinding>, String> {
    shortcuts::save(&app, bindings)
}

#[tauri::command]
fn reset_shortcut(
    app: AppHandle,
    action: String,
) -> Result<Vec<shortcuts::ShortcutBinding>, String> {
    shortcuts::reset_action(&app, &action)
}

#[tauri::command]
fn reset_shortcuts(app: AppHandle) -> Result<Vec<shortcuts::ShortcutBinding>, String> {
    shortcuts::reset_all(&app)
}

#[tauri::command]
fn get_page_rules(app: AppHandle) -> page_rules::PageRules {
    page_rules::get(&app)
}

#[tauri::command]
fn update_site_control(
    app: AppHandle,
    host: String,
    patch: page_rules::SitePatch,
) -> Result<page_rules::PageRules, String> {
    page_rules::update_site(&app, &host, patch)
}

#[tauri::command]
fn save_user_script(
    app: AppHandle,
    script: page_rules::UserScript,
) -> Result<page_rules::PageRules, String> {
    page_rules::save_script(&app, script)
}

#[tauri::command]
fn delete_user_script(app: AppHandle, id: String) -> Result<page_rules::PageRules, String> {
    page_rules::delete_script(&app, &id)
}

#[tauri::command]
async fn start_zapper(app: AppHandle, id: u32) -> Result<(), String> {
    tabs::run_zapper(&app, id)
}

#[tauri::command]
async fn start_inspector(app: AppHandle, id: u32) -> Result<(), String> {
    tabs::run_inspector(&app, id)
}

#[tauri::command]
fn translation_active(id: u32) -> bool {
    tools::translated_now(id)
}

#[tauri::command]
async fn translate_page(app: AppHandle, id: u32, language: String) -> Result<(), String> {
    blocking(move || tools::translate_page(&app, id, &language)).await
}

#[tauri::command]
async fn restore_translation(app: AppHandle, id: u32) -> Result<(), String> {
    blocking(move || tools::restore_page(&app, id)).await
}

#[tauri::command]
async fn translate_selection(app: AppHandle, text: String) -> Result<String, String> {
    blocking(move || tools::translate_selection(&app, &text)).await
}

#[tauri::command]
fn generate_password(
    length: usize,
    upper: bool,
    lower: bool,
    digits: bool,
    symbols: bool,
    min_symbols: usize,
    avoid_ambiguous: bool,
    exclude: String,
) -> Result<String, String> {
    tools::random_password(
        length,
        upper,
        lower,
        digits,
        symbols,
        min_symbols,
        avoid_ambiguous,
        &exclude,
    )
}

#[tauri::command]
fn password_settings(app: AppHandle) -> password::PasswordSettings {
    password::get(&app)
}

#[tauri::command]
fn save_password_settings(
    app: AppHandle,
    settings: password::PasswordSettings,
) -> Result<password::PasswordSettings, String> {
    password::save(&app, settings)
}

#[tauri::command]
fn copy_text(text: String) -> Result<(), String> {
    ocr::copy_text(&text)
}

#[tauri::command]
async fn show_notice(app: AppHandle, window: Window, message: String) {
    undo::notice(&app, &browser_window(&window), &message);
}

#[tauri::command]
async fn reading_stats(app: AppHandle, id: u32) -> Result<tools::ReadingStats, String> {
    blocking(move || tools::page_reading(&app, id)).await
}

#[tauri::command]
fn list_prices(app: AppHandle) -> Vec<tools::ProductPrice> {
    tools::list_prices(&app)
}

#[tauri::command]
async fn track_price(app: AppHandle, id: u32) -> Result<tools::ProductPrice, String> {
    blocking(move || tools::track_price(&app, id)).await
}

#[tauri::command]
async fn check_prices(
    app: AppHandle,
    ids: Option<Vec<String>>,
) -> Result<Vec<tools::ProductPrice>, String> {
    blocking(move || tools::check_prices(&app, ids.as_deref())).await
}

#[tauri::command]
fn untrack_price(app: AppHandle, id: String) -> Result<Vec<tools::ProductPrice>, String> {
    prices::untrack_price(&app, &id)
}

#[tauri::command]
async fn check_price(app: AppHandle, id: String) -> Result<Vec<tools::ProductPrice>, String> {
    blocking(move || prices::check_price(&app, &id)).await
}

#[tauri::command]
async fn preview_price_url(url: String) -> Result<tools::ProductPrice, String> {
    blocking(move || prices::preview_url(&url)).await
}

#[tauri::command]
async fn add_price_url(app: AppHandle, url: String) -> Result<Vec<tools::ProductPrice>, String> {
    blocking(move || prices::add_url(&app, &url)).await
}

#[tauri::command]
fn update_tracked_price(
    app: AppHandle,
    patch: prices::TrackedPricePatch,
) -> Result<Vec<tools::ProductPrice>, String> {
    prices::update_tracked(&app, patch)
}

// Boards: pins saved from pages, on boards inside folders.

#[tauri::command]
fn list_boards(app: AppHandle) -> boards::BoardsSnapshot {
    boards::snapshot(&app)
}

#[tauri::command]
fn create_board_folder(
    app: AppHandle,
    name: String,
    parent_id: Option<String>,
) -> Result<boards::BoardFolder, String> {
    boards::create_folder(&app, &name, parent_id.as_deref())
}

#[tauri::command]
fn rename_board_folder(app: AppHandle, id: String, name: String) -> Result<(), String> {
    boards::rename_folder(&app, &id, &name)
}

#[tauri::command]
fn move_board_folder(app: AppHandle, id: String, parent_id: Option<String>) -> Result<(), String> {
    boards::move_folder(&app, &id, parent_id.as_deref())
}

#[tauri::command]
async fn delete_board_folder(app: AppHandle, id: String) -> Result<(), String> {
    blocking(move || boards::delete_folder(&app, &id)).await
}

#[tauri::command]
fn create_board(
    app: AppHandle,
    name: String,
    folder_id: Option<String>,
) -> Result<boards::Board, String> {
    boards::create_board(&app, &name, folder_id.as_deref())
}

#[tauri::command]
fn rename_board(app: AppHandle, id: String, name: String) -> Result<(), String> {
    boards::rename_board(&app, &id, &name)
}

#[tauri::command]
fn move_board(app: AppHandle, id: String, folder_id: Option<String>) -> Result<(), String> {
    boards::move_board(&app, &id, folder_id.as_deref())
}

#[tauri::command]
fn set_board_cover(app: AppHandle, id: String, pin_id: Option<String>) -> Result<(), String> {
    boards::set_cover(&app, &id, pin_id.as_deref())
}

#[tauri::command]
async fn delete_board(app: AppHandle, id: String) -> Result<(), String> {
    blocking(move || boards::delete_board(&app, &id)).await
}

#[tauri::command]
fn update_pin(app: AppHandle, id: String, title: String, note: String) -> Result<(), String> {
    boards::update_pin(&app, &id, &title, &note)
}

#[tauri::command]
fn move_pins(app: AppHandle, ids: Vec<String>, board_id: String) -> Result<(), String> {
    boards::move_pins(&app, &ids, &board_id)
}

#[tauri::command]
async fn delete_pins(app: AppHandle, ids: Vec<String>) -> Result<(), String> {
    blocking(move || boards::delete_pins(&app, &ids)).await
}

#[tauri::command]
async fn pin_page(
    app: AppHandle,
    id: u32,
    board_id: String,
    full_page: bool,
) -> Result<boards::Pin, String> {
    blocking(move || boards::pin_page(&app, id, &board_id, full_page)).await
}

#[tauri::command]
async fn pin_region(app: AppHandle, id: u32, board_id: String) -> Result<boards::Pin, String> {
    blocking(move || boards::pin_region(&app, id, &board_id)).await
}

#[tauri::command]
async fn pin_element(app: AppHandle, id: u32, board_id: String) -> Result<boards::Pin, String> {
    blocking(move || boards::pin_element(&app, id, &board_id)).await
}

#[tauri::command]
async fn pin_link(app: AppHandle, id: u32, board_id: String) -> Result<boards::Pin, String> {
    blocking(move || boards::pin_link(&app, id, &board_id)).await
}

#[tauri::command]
async fn pin_images(
    app: AppHandle,
    id: u32,
    board_id: String,
    urls: Vec<String>,
) -> Result<Vec<boards::Pin>, String> {
    blocking(move || boards::pin_images(&app, id, &board_id, &urls)).await
}

#[tauri::command]
async fn board_thumbnails(
    app: AppHandle,
    ids: Vec<String>,
) -> Result<std::collections::HashMap<String, String>, String> {
    blocking(move || Ok(boards::thumbnails(&app, &ids))).await
}

#[tauri::command]
async fn board_image(app: AppHandle, id: String) -> Result<String, String> {
    blocking(move || boards::image_data_url(&app, &id)).await
}

#[tauri::command]
async fn copy_pin_image(app: AppHandle, id: String) -> Result<(), String> {
    blocking(move || boards::copy_image(&app, &id)).await
}

#[tauri::command]
async fn save_pin_image(app: AppHandle, id: String, path: String) -> Result<(), String> {
    blocking(move || boards::save_image_as(&app, &id, &PathBuf::from(path))).await
}

#[tauri::command]
async fn import_board_items(
    app: AppHandle,
    board_id: String,
    items: Vec<boards::BoardImportItem>,
) -> Result<Vec<boards::Pin>, String> {
    blocking(move || boards::import_items(&app, &board_id, &items)).await
}

#[tauri::command]
async fn open_pin_file(app: AppHandle, id: String) -> Result<(), String> {
    blocking(move || boards::open_pin_file(&app, &id)).await
}

#[tauri::command]
async fn save_pin_file(app: AppHandle, id: String, path: String) -> Result<(), String> {
    blocking(move || boards::save_file_as(&app, &id, &PathBuf::from(path))).await
}

#[tauri::command]
async fn find_deals(
    app: AppHandle,
    id: u32,
    force: Option<bool>,
) -> Result<shopping::ShoppingDeals, String> {
    blocking(move || shopping::find_deals(&app, id, force.unwrap_or(false))).await
}

#[tauri::command]
async fn find_deals_for_product(
    app: AppHandle,
    url: String,
    title: String,
    force: Option<bool>,
) -> Result<shopping::ShoppingDeals, String> {
    blocking(move || shopping::find_deals_for_product(&app, &url, &title, force.unwrap_or(false)))
        .await
}

#[tauri::command]
fn set_coupon_feedback(
    app: AppHandle,
    key: String,
    code: String,
    action: String,
) -> Result<shopping::ShoppingDeals, String> {
    shopping::set_coupon_feedback(&app, &key, &code, &action)
}

#[tauri::command]
fn ai_key_status(app: AppHandle, provider: String) -> secrets::AiKeyStatus {
    secrets::status(&app, &provider)
}

#[tauri::command]
fn set_ai_key(
    app: AppHandle,
    provider: String,
    key: String,
) -> Result<secrets::AiKeyStatus, String> {
    secrets::store_key(&app, &provider, &key)?;
    Ok(secrets::status(&app, &provider))
}

#[tauri::command]
fn clear_ai_key(app: AppHandle, provider: String) -> Result<secrets::AiKeyStatus, String> {
    secrets::clear_key(&app, &provider)?;
    Ok(secrets::status(&app, &provider))
}

#[tauri::command]
async fn test_ai_connection(app: AppHandle) -> Result<(), String> {
    blocking(move || ai::test_connection(&app)).await
}

#[tauri::command]
async fn list_ai_models(app: AppHandle) -> Result<Vec<String>, String> {
    blocking(move || ai::list_models(&app)).await
}

#[tauri::command]
fn install_site(app: AppHandle, id: u32) -> Result<(), String> {
    system::install_site(&app, id)
}

#[tauri::command]
fn open_default_apps() -> Result<(), String> {
    system::open_default_settings()
}

#[tauri::command]
async fn set_viewport(
    app: AppHandle,
    window: Window,
    width: u32,
    height: u32,
    cutout: bool,
    home_bar: bool,
) {
    tabs::set_viewport(
        &app,
        &browser_window(&window),
        width,
        height,
        cutout,
        home_bar,
    );
}

#[tauri::command]
async fn clear_viewport(app: AppHandle, window: Window) {
    tabs::clear_viewport(&app, &browser_window(&window));
}

#[tauri::command]
fn list_rewrite_rules(app: AppHandle) -> Vec<rewrite::RewriteRule> {
    rewrite::list(&app)
}

#[tauri::command]
fn save_rewrite_rules(
    app: AppHandle,
    rules: Vec<rewrite::RewriteRule>,
) -> Result<Vec<rewrite::RewriteRule>, String> {
    rewrite::save(&app, rules)
}

#[tauri::command]
fn installed_browsers() -> Vec<browsers::ExternalBrowser> {
    browsers::installed()
}

#[tauri::command]
fn open_in_browser(kind: String, url: String) -> Result<(), String> {
    browsers::open(&kind, &url)
}

#[tauri::command]
async fn list_cookies(app: AppHandle, id: u32) -> Result<Vec<webview2::SiteCookie>, String> {
    blocking(move || storage::cookies(&app, id)).await
}

#[tauri::command]
async fn set_cookie(app: AppHandle, id: u32, name: String, value: String) -> Result<(), String> {
    blocking(move || storage::set_cookie(&app, id, &name, &value)).await
}

#[tauri::command]
async fn delete_cookie(
    app: AppHandle,
    id: u32,
    name: String,
    domain: String,
    path: String,
) -> Result<(), String> {
    blocking(move || storage::delete_cookie(&app, id, &name, &domain, &path)).await
}

#[tauri::command]
async fn list_local_storage(app: AppHandle, id: u32) -> Result<Vec<storage::StoragePair>, String> {
    blocking(move || storage::local_items(&app, id)).await
}

#[tauri::command]
async fn set_local_storage(
    app: AppHandle,
    id: u32,
    key: String,
    value: String,
) -> Result<(), String> {
    blocking(move || storage::set_local(&app, id, &key, &value)).await
}

#[tauri::command]
async fn delete_local_storage(app: AppHandle, id: u32, key: String) -> Result<(), String> {
    blocking(move || storage::delete_local(&app, id, &key)).await
}

#[tauri::command]
async fn set_tab_refresh(app: AppHandle, id: u32, secs: u32, watch: bool) -> Result<(), String> {
    tabs::set_refresh(&app, id, secs, watch)
}

#[tauri::command]
async fn open_reader(app: AppHandle, id: u32) -> Result<(), String> {
    let page = blocking({
        let app = app.clone();
        move || print::extract_page(&app, id)
    })
    .await?;
    page_rules::store_reader(&app, page);
    tabs::navigate(&app, id, "ce://reader")
}

#[tauri::command]
fn reader_page(app: AppHandle) -> Option<print::ExtractedPage> {
    page_rules::reader(&app)
}

#[tauri::command]
async fn show_reader_original(app: AppHandle, webview: tauri::Webview) -> Result<(), String> {
    let id = webview
        .label()
        .strip_prefix("tab-")
        .and_then(|id| id.parse().ok())
        .ok_or("This page is not a tab.")?;
    let url = page_rules::reader(&app)
        .map(|page| page.url)
        .filter(|url| !url.is_empty())
        .ok_or("The original page is no longer available.")?;
    tabs::navigate(&app, id, &url)
}

#[tauri::command]
fn get_settings(app: AppHandle) -> BrowserSettings {
    settings::get(&app)
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: BrowserSettings) -> Result<BrowserSettings, String> {
    let saved = settings::save(&app, settings)?;
    if let Err(error) = system::register_browser(&app) {
        eprintln!("file handlers: {error}");
    }
    automation::mcp::sync(&app);
    Ok(saved)
}

/// Print choices for the active profile. Other settings stay as they are.
#[tauri::command]
fn save_print_settings(
    app: AppHandle,
    print_editor: settings::PrintEditorSettings,
) -> Result<BrowserSettings, String> {
    settings::update(&app, |settings| {
        settings.print_editor = print_editor;
    })
}

/// Settings as a new profile gets them, so the settings page can mark changed values.
#[tauri::command]
fn default_settings() -> BrowserSettings {
    BrowserSettings::default().normalize()
}

#[tauri::command]
fn backdrop_support(app: AppHandle) -> backdrop::BackdropSupport {
    backdrop::support(&app)
}

#[tauri::command]
fn update_status(app: AppHandle) -> update::UpdateStatus {
    update::status(&app)
}

#[tauri::command]
async fn check_for_update(app: AppHandle) -> Result<update::UpdateStatus, String> {
    update::check(&app).await
}

/// Downloads and runs the installer. The browser closes and the new version opens.
#[tauri::command]
async fn install_update(app: AppHandle) -> Result<(), String> {
    update::install(&app).await
}

#[tauri::command]
async fn release_notes(app: AppHandle, version: String) -> Result<update::ReleaseNotes, String> {
    blocking(move || update::release_notes(&app, &version)).await
}

#[tauri::command]
fn default_download_dir(app: AppHandle) -> String {
    BrowserSettings::default()
        .download_dir(&app)
        .to_string_lossy()
        .into_owned()
}

#[tauri::command]
fn windows_color_scheme(app: AppHandle) -> String {
    if settings::windows_uses_light(&app) {
        "light".into()
    } else {
        "dark".into()
    }
}

#[tauri::command]
fn get_user_styles(app: AppHandle) -> Result<user_styles::UserStyles, String> {
    user_styles::get(&app)
}

#[tauri::command]
fn save_user_styles(app: AppHandle, css: String) -> Result<user_styles::UserStyles, String> {
    user_styles::save(&app, &css)
}

#[tauri::command]
fn reload_user_styles(app: AppHandle) -> Result<user_styles::UserStyles, String> {
    user_styles::reload(&app)
}

#[tauri::command]
fn show_user_styles(app: AppHandle) -> Result<(), String> {
    user_styles::show(&app)
}

#[tauri::command]
fn show_sync_folder(app: AppHandle) -> Result<(), String> {
    sync::show_folder(&app)
}

#[tauri::command]
fn default_user_styles() -> String {
    user_styles::template().to_string()
}

#[tauri::command]
async fn create_sync_folder(app: AppHandle, path: String) -> Result<SyncStatus, String> {
    blocking(move || sync::create(&app, &path)).await
}

#[tauri::command]
async fn select_sync_folder(app: AppHandle, path: String) -> Result<SyncStatus, String> {
    blocking(move || sync::select(&app, &path)).await
}

#[tauri::command]
async fn clear_sync_folder(app: AppHandle) -> Result<SyncStatus, String> {
    blocking(move || sync::clear(&app)).await
}

#[tauri::command]
fn sync_status(app: AppHandle) -> SyncStatus {
    sync::status(&app)
}

#[tauri::command]
fn list_sync_conflicts(app: AppHandle) -> Vec<sync_conflicts::SyncConflict> {
    sync_conflicts::list(&app)
}

#[tauri::command]
async fn resolve_sync_conflict(app: AppHandle, id: String, choice: String) -> Result<(), String> {
    blocking(move || sync_conflicts::resolve(&app, &id, &choice)).await
}

#[tauri::command]
async fn export_profile_archive(
    app: AppHandle,
    path: String,
    password: String,
) -> Result<(), String> {
    blocking(move || profile_archive::export_archive(&app, std::path::Path::new(&path), &password))
        .await
}

#[tauri::command]
async fn import_profile_archive(
    app: AppHandle,
    path: String,
    password: String,
) -> Result<profile_archive::ImportedProfile, String> {
    let notify = app.clone();
    let imported = blocking(move || {
        profile_archive::import_archive(&app, std::path::Path::new(&path), &password)
    })
    .await?;
    let _ = notify.emit("profiles-changed", profiles::list(&notify));
    Ok(imported)
}

#[tauri::command]
async fn search_history(
    app: AppHandle,
    query: String,
) -> Result<Vec<history_index::HistoryHit>, String> {
    blocking(move || Ok(history_index::search(&app, &query))).await
}

#[tauri::command]
async fn check_bookmark_health(
    app: AppHandle,
) -> Result<bookmark_health::BookmarkHealthReport, String> {
    blocking(move || bookmark_health::check(&app)).await
}

fn publish_profiles(app: &AppHandle, profiles: Vec<ProfileInfo>) -> Vec<ProfileInfo> {
    let _ = app.emit("profiles-changed", &profiles);
    profiles
}

#[tauri::command]
fn list_profiles(app: AppHandle) -> Vec<ProfileInfo> {
    profiles::list(&app)
}

#[tauri::command]
fn create_profile(app: AppHandle, name: String) -> Result<Vec<ProfileInfo>, String> {
    profiles::create(&app, &name).map(|profiles| publish_profiles(&app, profiles))
}

#[tauri::command]
fn import_style_metrics(app: AppHandle, profile_id: String) -> Result<BrowserSettings, String> {
    settings::import_style_metrics(&app, &profile_id)
}

#[tauri::command]
async fn copy_profile(
    app: AppHandle,
    id: String,
    name: String,
    parts: Vec<String>,
) -> Result<Vec<ProfileInfo>, String> {
    let notify = app.clone();
    blocking(move || profiles::copy(&app, &id, &name, &parts))
        .await
        .map(|profiles| publish_profiles(&notify, profiles))
}

#[tauri::command]
fn open_profile(id: String) -> Result<(), String> {
    profiles::open(&id)
}

#[tauri::command]
fn install_profile_shortcut(app: AppHandle, id: String) -> Result<Vec<ProfileInfo>, String> {
    profiles::install_shortcut_for(&app, &id).map(|profiles| publish_profiles(&app, profiles))
}

#[tauri::command]
fn save_profile_shortcut(app: AppHandle, id: String, path: String) -> Result<(), String> {
    profiles::save_shortcut(&app, &id, std::path::Path::new(&path))
}

#[tauri::command]
async fn delete_profile(app: AppHandle, id: String) -> Result<Vec<ProfileInfo>, String> {
    let notify = app.clone();
    blocking(move || profiles::delete(&app, &id))
        .await
        .map(|profiles| publish_profiles(&notify, profiles))
}

#[tauri::command]
fn list_profile_icons() -> Vec<ProfileIconChoice> {
    profiles::icon_choices()
}

#[tauri::command]
fn list_profile_icon_colors() -> Vec<ProfileIconChoice> {
    profiles::icon_color_choices()
}

#[tauri::command]
fn set_profile_icon(
    app: AppHandle,
    id: String,
    icon_id: String,
    path: Option<String>,
) -> Result<Vec<ProfileInfo>, String> {
    let profiles = profiles::set_icon(&app, &id, &icon_id, path.as_deref());
    if id == profiles::active_id() {
        if let Some(window) = app.get_window(tabs::WINDOW) {
            profiles::apply_window_icon(&app, &window);
        }
    }
    profiles.map(|profiles| publish_profiles(&app, profiles))
}

#[tauri::command]
fn set_profile_icon_color(
    app: AppHandle,
    id: String,
    color_id: String,
) -> Result<Vec<ProfileInfo>, String> {
    let profiles = profiles::set_icon_color(&app, &id, &color_id);
    if id == profiles::active_id() {
        if let Some(window) = app.get_window(tabs::WINDOW) {
            profiles::apply_window_icon(&app, &window);
        }
    }
    profiles.map(|profiles| publish_profiles(&app, profiles))
}

#[tauri::command]
fn pin_profile_taskbar(app: AppHandle, id: String) -> Result<Vec<ProfileInfo>, String> {
    profiles::pin_taskbar(&app, &id).map(|profiles| publish_profiles(&app, profiles))
}

#[tauri::command]
fn unpin_profile_taskbar(app: AppHandle, id: String) -> Result<Vec<ProfileInfo>, String> {
    profiles::unpin_taskbar(&app, &id).map(|profiles| publish_profiles(&app, profiles))
}

// Agent. Runs work on their own threads; these commands start, steer and read them.

#[tauri::command]
fn agent_start(app: AppHandle, tab_id: u32, goal: String) -> Result<u32, String> {
    automation::agent::start(&app, tab_id, &goal)
}

#[tauri::command]
fn agent_stop(app: AppHandle, run_id: u32) {
    automation::agent::stop(&app, run_id);
}

#[tauri::command]
fn agent_answer(app: AppHandle, run_id: u32, approval_id: u32, allow: bool) {
    automation::agent::answer(&app, run_id, approval_id, allow);
}

#[tauri::command]
fn agent_reply(app: AppHandle, run_id: u32, text: String) -> Result<(), String> {
    automation::agent::reply(&app, run_id, &text)
}

#[tauri::command]
fn agent_take_over(app: AppHandle, run_id: u32) {
    automation::agent::take_over(&app, run_id);
}

#[tauri::command]
fn agent_resume(app: AppHandle, run_id: u32) {
    automation::agent::resume(&app, run_id);
}

#[tauri::command]
fn agent_runs() -> Vec<automation::agent::AgentRun> {
    automation::agent::runs()
}

#[tauri::command]
fn agent_clear(app: AppHandle) {
    automation::agent::clear(&app);
}

#[tauri::command]
async fn agent_show_tab(app: AppHandle, tab_id: u32) -> Result<(), String> {
    tabs::unshelve(&app, tab_id)
}

#[tauri::command]
async fn toggle_agent_panel(
    app: AppHandle,
    window: Window,
    open: Option<bool>,
) -> Result<bool, String> {
    automation::panel::toggle(&app, &browser_window(&window), open)
}

#[tauri::command]
async fn resize_agent_panel(app: AppHandle, window: Window) -> Result<(), String> {
    let window = browser_window(&window);
    blocking(move || automation::panel::track_resize(&app, &window)).await
}

#[tauri::command]
async fn set_agent_panel_width(app: AppHandle, window: Window, width: f64) -> Result<(), String> {
    automation::panel::set_width(&app, &browser_window(&window), width)
}

#[tauri::command]
fn automation_secret_names(app: AppHandle) -> Vec<String> {
    secrets::vault_names(&app)
}

#[tauri::command]
fn set_automation_secret(
    app: AppHandle,
    name: String,
    value: String,
) -> Result<Vec<String>, String> {
    secrets::vault_store(&app, &name, &value)?;
    Ok(secrets::vault_names(&app))
}

#[tauri::command]
fn clear_automation_secret(app: AppHandle, name: String) -> Result<Vec<String>, String> {
    secrets::vault_clear(&app, &name)?;
    Ok(secrets::vault_names(&app))
}

// MCP server.

#[tauri::command]
fn mcp_status(app: AppHandle) -> automation::mcp::McpStatus {
    automation::mcp::status(&app)
}

#[tauri::command]
fn regenerate_mcp_token(app: AppHandle) -> Result<automation::mcp::McpStatus, String> {
    automation::mcp::regenerate_token(&app)
}

// Flows and the recorder. Runs work on their own threads.

#[tauri::command]
fn automations_snapshot(app: AppHandle) -> automation::flows::AutomationsSnapshot {
    automation::flows::snapshot(&app)
}

#[tauri::command]
fn save_flow(
    app: AppHandle,
    flow: automation::flows::Flow,
) -> Result<automation::flows::Flow, String> {
    automation::flows::save(&app, flow)
}

#[tauri::command]
fn delete_flow(app: AppHandle, id: String) -> Result<(), String> {
    automation::flows::delete(&app, &id)
}

#[tauri::command]
fn run_flow(app: AppHandle, id: String, tab_id: Option<u32>) -> Result<u32, String> {
    automation::flows::run(&app, &id, tab_id, "manual")
}

#[tauri::command]
fn stop_flow_run(app: AppHandle, run_id: u32) {
    automation::flows::stop_run(&app, run_id);
}

#[tauri::command]
fn clear_flow_runs(app: AppHandle) {
    automation::flows::clear_runs(&app);
}

#[tauri::command]
async fn start_flow_recording(app: AppHandle, tab_id: u32) -> Result<(), String> {
    blocking(move || automation::recorder::start(&app, tab_id)).await
}

#[tauri::command]
async fn stop_flow_recording(app: AppHandle, save: bool) -> Result<Option<String>, String> {
    blocking(move || automation::recorder::stop(&app, save)).await
}

#[tauri::command]
fn flow_playwright(app: AppHandle, id: String) -> Result<String, String> {
    automation::flows::playwright(&app, &id)
}

#[tauri::command]
async fn open_automations(
    app: AppHandle,
    window: Window,
    section: Option<String>,
) -> Result<(), String> {
    let label = browser_window(&window);
    automation::flows::open_page(&app, &label, section.as_deref())
}

// Extraction and datasets. Reading pages waits on the main thread, so these block off it.

#[tauri::command]
async fn extract_pick(
    app: AppHandle,
    tab_id: u32,
) -> Result<Option<automation::extract::ExtractPick>, String> {
    blocking(move || automation::extract::pick(&app, tab_id)).await
}

#[tauri::command]
async fn extract_pick_save(
    app: AppHandle,
    tab_id: u32,
    options: automation::extract::ExtractOptions,
) -> Result<Option<automation::extract::DatasetInfo>, String> {
    blocking(move || automation::extract::pick_and_save(&app, tab_id, &options)).await
}

#[tauri::command]
async fn extract_run(
    app: AppHandle,
    tab_id: u32,
    spec: automation::extract::ExtractSpec,
    options: automation::extract::ExtractOptions,
) -> Result<automation::extract::DatasetInfo, String> {
    blocking(move || automation::extract::run(&app, tab_id, &spec, &options)).await
}

#[tauri::command]
async fn extract_ai(
    app: AppHandle,
    tab_id: u32,
    instruction: String,
    options: automation::extract::ExtractOptions,
) -> Result<automation::extract::DatasetInfo, String> {
    blocking(move || automation::extract::ai(&app, tab_id, &instruction, &options)).await
}

#[tauri::command]
async fn extract_crawl(
    app: AppHandle,
    urls: Vec<String>,
    spec: Option<automation::extract::ExtractSpec>,
    instruction: String,
    name: String,
) -> Result<automation::extract::DatasetInfo, String> {
    blocking(move || automation::extract::crawl(&app, &urls, spec.as_ref(), &instruction, &name))
        .await
}

#[tauri::command]
async fn extract_sniff(
    app: AppHandle,
    tab_id: u32,
) -> Result<Vec<automation::extract::SniffedEndpoint>, String> {
    blocking(move || automation::extract::sniff(&app, tab_id)).await
}

#[tauri::command]
async fn extract_endpoint(
    app: AppHandle,
    tab_id: u32,
    url: String,
    name: String,
) -> Result<automation::extract::DatasetInfo, String> {
    blocking(move || automation::extract::endpoint(&app, tab_id, &url, &name)).await
}

#[tauri::command]
fn get_dataset(app: AppHandle, id: String) -> Result<automation::extract::Dataset, String> {
    automation::extract::get(&app, &id)
}

#[tauri::command]
fn delete_dataset(app: AppHandle, id: String) -> Result<(), String> {
    automation::extract::delete(&app, &id)
}

#[tauri::command]
fn rename_dataset(app: AppHandle, id: String, name: String) -> Result<(), String> {
    automation::extract::rename(&app, &id, &name)
}

#[tauri::command]
fn copy_dataset(app: AppHandle, id: String, format: String) -> Result<(), String> {
    automation::extract::copy(&app, &id, &format)
}

#[tauri::command]
async fn export_dataset(
    app: AppHandle,
    id: String,
    format: String,
    path: String,
) -> Result<(), String> {
    blocking(move || automation::extract::export(&app, &id, &format, &PathBuf::from(path))).await
}

#[tauri::command]
async fn dataset_to_board(app: AppHandle, id: String) -> Result<usize, String> {
    blocking(move || automation::extract::to_board(&app, &id)).await
}

fn build_window(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    tabs::create_browser_window(app, tabs::WINDOW)
        .map_err(|error| -> Box<dyn std::error::Error> { Box::new(std::io::Error::other(error)) })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    debug::stderr::capture();
    system::read_args();
    profiles::read_args();
    tauri::Builder::default()
        // Before the dialog plugin, so its alert/confirm replacement is ignored.
        .plugin(page_dialogs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let handle = app.handle().clone();
            debug::init(&handle);
            hang_guard::init(&handle);
            sync::attach(&handle);
            profiles::init(&handle);
            if !tauri::is_dev() {
                let moved_from = update::record_exe(&handle);
                let data_root = paths::data_root(&handle);
                std::thread::spawn(move || profiles::repair_shortcuts(moved_from, &data_root));
            }
            if let Err(error) = profiles::set_process_identity() {
                eprintln!("profile identity: {error}");
            }
            settings::init(&handle);
            page_rules::init(&handle);
            rewrite::init(&handle);
            privacy::init(&handle);
            keywords::init(&handle);
            shortcuts::init(&handle);
            if let Err(error) = user_styles::ensure(&handle) {
                eprintln!("user styles: {error}");
            }
            bookmarks::init(&handle);
            downloads::init(&handle);
            history::init(&handle);
            extensions::init(&handle);
            tabs::init(&handle);
            peek::init(&handle);
            popup::init(&handle);
            session::init(&handle);
            placement::init(&handle);
            status::init(&handle);
            tab_sleep::init(&handle);
            update::init(&handle);
            build_window(&handle)?;
            system::start(&handle);
            update::start(&handle);
            prices::start(&handle);
            extension_store::start(&handle);
            automation::mcp::init(&handle);
            automation::scheduler::init(&handle);

            tauri::async_runtime::spawn(async move {
                if let Err(error) = session::restore(&handle) {
                    eprintln!("session restore: {error}");
                    if let Err(error) = tabs::ensure_bookmarks_host(&handle, tabs::WINDOW) {
                        eprintln!("bookmarks: {error}");
                    }
                }
                system::apply_launch(&handle);
                system::open_pending_files(&handle);
                update::note_version(&handle);
                let extensions_app = handle.clone();
                let _ = tauri::async_runtime::spawn_blocking(move || {
                    if let Err(error) = extensions::sync_on_startup(&extensions_app) {
                        eprintln!("extensions: {error}");
                    }
                    sync::start(&extensions_app);
                })
                .await;
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            let label = window.label().to_string();
            if debug::is_debug_label(&label) {
                debug::window_event(window, event);
                return;
            }
            if popup::is_popup_label(&label) {
                if matches!(event, WindowEvent::ScaleFactorChanged { .. }) {
                    popup::rescaled(window.app_handle(), &label);
                }
                return;
            }
            if !tabs::is_browser_label(&label) {
                return;
            }
            match event {
                WindowEvent::Moved(_) => {
                    popup::close_all(window.app_handle(), &label, false);
                    placement::note(window);
                    debug::follow(window.app_handle(), &label);
                }
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                    placement::note(window);
                    debug::follow(window.app_handle(), &label);
                    let app = window.app_handle().clone();
                    let label = label.clone();
                    tauri::async_runtime::spawn(async move { tabs::layout(&app, &label) });
                }
                WindowEvent::ThemeChanged(theme) => {
                    let app = window.app_handle().clone();
                    webview2::apply_site_color_scheme(&app);
                    tabs::apply_page_background(&app);
                    let scheme = if *theme == tauri::Theme::Light {
                        "light"
                    } else {
                        "dark"
                    };
                    let _ = app.emit("windows-color-scheme", scheme);
                }
                WindowEvent::CloseRequested { api, .. } => {
                    let app = window.app_handle().clone();
                    let others = tabs::browser_labels(&app).iter().any(|item| item != &label);
                    if others {
                        tabs::discard_window(&app, &label);
                        return;
                    }
                    placement::note(window);
                    if !sync::configured(&app) || sync::closing_allowed() {
                        session::shutdown(&app);
                        return;
                    }
                    api.prevent_close();
                    if !sync::begin_close() {
                        return;
                    }
                    session::shutdown(&app);
                    // Browser windows are plain windows with child webviews, so a webview-window
                    // lookup never finds them and the window stays open until the next click.
                    let window = window.clone();
                    tauri::async_runtime::spawn(async move {
                        let flush_app = app.clone();
                        let _ =
                            tauri::async_runtime::spawn_blocking(move || sync::flush(&flush_app))
                                .await;
                        sync::allow_close();
                        let _ = window.close();
                    });
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            list_tabs,
            create_tab,
            close_tab,
            close_selected_tabs,
            toggle_tab_selection,
            select_tab_range,
            close_other_tabs,
            close_tabs_to_right,
            activate_tab,
            navigate,
            go_back,
            go_forward,
            reload,
            clear_site_cache,
            get_adblock_stats,
            stop,
            end_page,
            allow_certificate,
            move_tab,
            detach_tab,
            open_in_window,
            pin_tab,
            unpin_tab,
            restore_pinned_tabs,
            duplicate_tab,
            reopen_closed_tab,
            list_recently_closed,
            restore_closed,
            tab_thumbnail,
            tab_memory,
            undo_action,
            dismiss_toast,
            note_typed,
            site_info,
            set_ui_insets,
            set_tabs_muted,
            toggle_selected_muted,
            sleep_tab,
            sleep_other_tabs,
            never_sleep_site,
            split_with,
            open_in_split,
            swap_split,
            exit_split,
            set_split_ratio,
            set_devtools_ratio,
            create_group,
            add_to_group,
            remove_from_group,
            update_group,
            ungroup,
            close_group,
            new_tab_in_group,
            save_group_to_bookmarks,
            open_folder_as_group,
            group_by_site,
            ctrl_held,
            create_workspace,
            rename_workspace,
            delete_workspace,
            switch_workspace,
            move_tabs_to_workspace,
            peek_state,
            close_peek,
            peek_to_tab,
            peek_to_split,
            zoom_tab,
            find_in_page,
            clear_find,
            find_in_tabs,
            open_devtools,
            open_ui_devtools,
            open_popup,
            close_popup,
            popup_measured,
            popup_current,
            capture_page_png,
            copy_page_png,
            capture_region,
            store_visible_png,
            last_capture_png,
            save_edited_png,
            copy_edited_png,
            start_recording,
            stop_recording,
            list_media,
            set_all_media_muted,
            control_media,
            page_media,
            download_page_media,
            save_page_html,
            save_page_markdown,
            write_text_file,
            ocr_visible,
            ocr_region,
            extract_page,
            detect_technologies,
            print_tab_to_pdf,
            print_html_to_pdf,
            print_html_dialog,
            open_saved_file,
            show_saved_file,
            list_bookmarks,
            add_bookmark,
            update_bookmark,
            move_bookmark,
            delete_bookmark,
            delete_bookmarks,
            find_bookmark,
            site_favicons,
            import_bookmarks_html,
            export_bookmarks_html,
            list_downloads,
            computer_name,
            browse_local,
            open_local_file,
            pause_download,
            resume_download,
            cancel_download,
            open_download,
            show_download_in_folder,
            drag_download,
            remove_download,
            clear_downloads,
            list_history,
            search_history,
            remove_history,
            clear_history,
            check_bookmark_health,
            list_extensions,
            install_extension_folder,
            install_extension_zip,
            install_extension_from_store,
            check_extension_updates,
            update_extension,
            set_extension_enabled,
            remove_extension,
            open_extension_options,
            list_permissions,
            set_permission,
            answer_permission,
            list_microphones,
            watch_microphone,
            unwatch_microphone,
            set_microphone_device,
            list_containers,
            create_container,
            delete_container,
            set_group_container,
            create_ephemeral_tab,
            network_log,
            clear_network_log,
            open_network_window,
            list_keywords,
            save_keywords,
            omnibox_extra,
            list_shortcuts,
            save_shortcuts,
            reset_shortcut,
            reset_shortcuts,
            get_page_rules,
            update_site_control,
            save_user_script,
            delete_user_script,
            start_zapper,
            start_inspector,
            translation_active,
            translate_page,
            restore_translation,
            translate_selection,
            generate_password,
            password_settings,
            save_password_settings,
            copy_text,
            show_notice,
            reading_stats,
            list_prices,
            track_price,
            check_prices,
            untrack_price,
            check_price,
            preview_price_url,
            add_price_url,
            update_tracked_price,
            list_boards,
            create_board_folder,
            rename_board_folder,
            move_board_folder,
            delete_board_folder,
            create_board,
            rename_board,
            move_board,
            set_board_cover,
            delete_board,
            update_pin,
            move_pins,
            delete_pins,
            pin_page,
            pin_region,
            pin_element,
            pin_link,
            pin_images,
            board_thumbnails,
            board_image,
            copy_pin_image,
            save_pin_image,
            import_board_items,
            open_pin_file,
            save_pin_file,
            find_deals,
            find_deals_for_product,
            set_coupon_feedback,
            ai_key_status,
            set_ai_key,
            clear_ai_key,
            test_ai_connection,
            list_ai_models,
            install_site,
            open_default_apps,
            set_viewport,
            clear_viewport,
            list_rewrite_rules,
            save_rewrite_rules,
            installed_browsers,
            open_in_browser,
            list_cookies,
            set_cookie,
            delete_cookie,
            list_local_storage,
            set_local_storage,
            delete_local_storage,
            set_tab_refresh,
            open_reader,
            reader_page,
            show_reader_original,
            get_settings,
            save_settings,
            save_print_settings,
            default_settings,
            backdrop_support,
            update_status,
            check_for_update,
            install_update,
            release_notes,
            default_download_dir,
            windows_color_scheme,
            get_user_styles,
            save_user_styles,
            reload_user_styles,
            show_user_styles,
            show_sync_folder,
            default_user_styles,
            create_sync_folder,
            select_sync_folder,
            clear_sync_folder,
            sync_status,
            list_sync_conflicts,
            resolve_sync_conflict,
            export_profile_archive,
            import_profile_archive,
            list_profiles,
            create_profile,
            import_style_metrics,
            copy_profile,
            open_profile,
            install_profile_shortcut,
            save_profile_shortcut,
            delete_profile,
            list_profile_icons,
            list_profile_icon_colors,
            set_profile_icon,
            set_profile_icon_color,
            pin_profile_taskbar,
            unpin_profile_taskbar,
            agent_start,
            agent_stop,
            agent_answer,
            agent_reply,
            agent_take_over,
            agent_resume,
            agent_runs,
            agent_clear,
            agent_show_tab,
            toggle_agent_panel,
            resize_agent_panel,
            set_agent_panel_width,
            automation_secret_names,
            set_automation_secret,
            clear_automation_secret,
            mcp_status,
            regenerate_mcp_token,
            automations_snapshot,
            save_flow,
            delete_flow,
            run_flow,
            stop_flow_run,
            clear_flow_runs,
            start_flow_recording,
            stop_flow_recording,
            flow_playwright,
            open_automations,
            extract_pick,
            extract_pick_save,
            extract_run,
            extract_ai,
            extract_crawl,
            extract_sniff,
            extract_endpoint,
            get_dataset,
            delete_dataset,
            rename_dataset,
            copy_dataset,
            export_dataset,
            dataset_to_board,
            debug::debug_toggle_hud,
            debug::debug_snapshot,
            debug::debug_record_ipc,
            debug::debug_record_console,
            debug::debug_clear,
            debug::debug_webviews,
            debug::debug_outline,
            debug::debug_clear_outline,
            debug::debug_open_devtools,
            debug::debug_reload_webview,
            debug::debug_paths,
            debug::debug_reveal_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
