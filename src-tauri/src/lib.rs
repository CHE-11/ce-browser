mod accelerator;
mod accent;
mod adblock;
mod answer;
mod backdrop;
mod boards;
mod bookmark_health;
mod bookmarks;
mod browsers;
mod capture;
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
mod ocr;
mod page_errors;
mod page_menu;
mod page_rules;
mod paths;
mod peek;
mod print;
mod privacy;
mod profile_archive;
mod profile_icons;
mod profiles;
mod rewrite;
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

use tauri::{AppHandle, Manager, Window, WindowEvent};

use adblock::AdblockStats;
use bookmarks::{BookmarkNode, BookmarkTree};
use capture::PageCaptureResult;
use downloads::DownloadItem;
use extensions::ExtensionInfo;
use history::HistoryItem;
use print::{ExtractedPage, PdfOptions};
use profiles::{ProfileIconChoice, ProfileInfo};
use settings::BrowserSettings;
use sync::SyncStatus;
use tabs::{PopupRequest, TabsSnapshot};
use what_runs::PageTechnologies;

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
        window.label(),
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
    tabs::restore_saved_pins(&app, window.label())
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
    undo::dismiss(&app, window.label(), id);
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
    tabs::set_ui_insets(&app, window.label(), top, left, right, bottom);
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
    tabs::open_in_split(&app, window.label(), &url)
}

#[tauri::command]
async fn swap_split(app: AppHandle, window: Window) -> Result<(), String> {
    tabs::swap_split(&app, window.label())
}

#[tauri::command]
async fn exit_split(app: AppHandle, window: Window) -> Result<(), String> {
    tabs::exit_split(&app, window.label())
}

#[tauri::command]
async fn set_split_ratio(app: AppHandle, window: Window, ratio: f64) {
    tabs::set_split_ratio(&app, window.label(), ratio);
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
    tabs::open_as_group(&app, window.label(), &title, &links)
}

#[tauri::command]
async fn group_by_site(app: AppHandle, window: Window) -> Result<usize, String> {
    tabs::group_by_site(&app, window.label())
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
    tabs::switch_workspace(&app, window.label(), &id)
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
    peek::info(&app, window.label())
}

#[tauri::command]
async fn close_peek(app: AppHandle, window: Window) {
    peek::close(&app, window.label());
}

#[tauri::command]
async fn peek_to_tab(app: AppHandle, window: Window) -> Result<u32, String> {
    peek::to_tab(&app, window.label())
}

#[tauri::command]
async fn peek_to_split(app: AppHandle, window: Window) -> Result<u32, String> {
    peek::to_split(&app, window.label())
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
    let label = window.label().to_string();
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
async fn open_popup(app: AppHandle, window: Window, request: PopupRequest) -> Result<bool, String> {
    tabs::open_popup(&app, window.label(), request)
}

#[tauri::command]
async fn close_popup(app: AppHandle, window: Window) {
    tabs::close_window_popup(&app, window.label());
}

#[tauri::command]
async fn fit_popup(app: AppHandle, window: Window, height: f64) -> Result<(), String> {
    tabs::fit_popup(&app, window.label(), None, height, None)
}

#[tauri::command]
async fn set_popup_shape(
    app: AppHandle,
    window: Window,
    shape: Vec<tabs::PopupRect>,
) -> Result<(), String> {
    tabs::set_popup_shape(&app, window.label(), shape)
}

#[tauri::command]
async fn popup_host_ready(app: AppHandle, window: Window) {
    tabs::popup_host_ready(&app, window.label());
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
            window.label(),
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
async fn open_download(app: AppHandle, id: u32) -> Result<(), String> {
    blocking(move || downloads::open(&app, id)).await
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
            window.label(),
            "Removed from history",
            undo::UndoAction::History {
                items: vec![item],
                cleared: false,
            },
        );
    }
}

#[tauri::command]
async fn clear_history(app: AppHandle, window: Window) {
    let items = history::take_all(&app);
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
        window.label(),
        &message,
        undo::UndoAction::History {
            items,
            cleared: true,
        },
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
fn set_permission(app: AppHandle, origin: String, kind: String, allow: bool) -> Result<(), String> {
    privacy::set_permission(&app, &origin, &kind, allow)
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
    tabs::create_ephemeral_tab(&app, window.label())
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
) -> Result<String, String> {
    tools::random_password(
        length,
        upper,
        lower,
        digits,
        symbols,
        min_symbols,
        avoid_ambiguous,
    )
}

#[tauri::command]
fn copy_text(text: String) -> Result<(), String> {
    ocr::copy_text(&text)
}

#[tauri::command]
async fn show_notice(app: AppHandle, window: Window, message: String) {
    undo::notice(&app, window.label(), &message);
}

#[tauri::command]
fn wayback_url(save: bool, url: String) -> Result<String, String> {
    tools::wayback_url(save, &url)
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
async fn check_prices(app: AppHandle) -> Result<Vec<tools::ProductPrice>, String> {
    blocking(move || tools::check_prices(&app)).await
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
async fn ask_page(app: AppHandle, id: u32, question: String) -> Result<String, String> {
    blocking(move || tools::ask_page(&app, id, &question)).await
}

#[tauri::command]
async fn find_deals(app: AppHandle, id: u32) -> Result<shopping::ShoppingDeals, String> {
    blocking(move || shopping::find_deals(&app, id)).await
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
fn set_device(app: AppHandle, window: String, device: String) -> Result<(), String> {
    tabs::set_device(&app, &window, &device)
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
    settings::save(&app, settings)
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
    blocking(move || profile_archive::import_archive(&app, std::path::Path::new(&path), &password))
        .await
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

#[tauri::command]
fn list_profiles(app: AppHandle) -> Vec<ProfileInfo> {
    profiles::list(&app)
}

#[tauri::command]
fn create_profile(app: AppHandle, name: String) -> Result<Vec<ProfileInfo>, String> {
    profiles::create(&app, &name)
}

#[tauri::command]
fn open_profile(id: String) -> Result<(), String> {
    profiles::open(&id)
}

#[tauri::command]
fn install_profile_shortcut(app: AppHandle, id: String) -> Result<Vec<ProfileInfo>, String> {
    profiles::install_shortcut_for(&app, &id)
}

#[tauri::command]
async fn delete_profile(app: AppHandle, id: String) -> Result<Vec<ProfileInfo>, String> {
    blocking(move || profiles::delete(&app, &id)).await
}

#[tauri::command]
fn list_profile_icons() -> Vec<ProfileIconChoice> {
    profiles::icon_choices()
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
    profiles
}

#[tauri::command]
fn pin_profile_taskbar(app: AppHandle, id: String) -> Result<Vec<ProfileInfo>, String> {
    profiles::pin_taskbar(&app, &id)
}

#[tauri::command]
fn unpin_profile_taskbar(app: AppHandle, id: String) -> Result<Vec<ProfileInfo>, String> {
    profiles::unpin_taskbar(&app, &id)
}

fn build_window(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    tabs::create_browser_window(app, tabs::WINDOW)
        .map_err(|error| -> Box<dyn std::error::Error> { Box::new(std::io::Error::other(error)) })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    system::read_args();
    profiles::read_args();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let handle = app.handle().clone();
            hang_guard::init(&handle);
            sync::attach(&handle);
            profiles::init(&handle);
            if !tauri::is_dev() {
                let moved_from = update::record_exe(&handle);
                std::thread::spawn(move || profiles::repair_shortcuts(moved_from));
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
            session::init(&handle);
            status::init(&handle);
            tab_sleep::init(&handle);
            update::init(&handle);
            build_window(&handle)?;
            system::start(&handle);
            update::start(&handle);
            extension_store::start(&handle);

            tauri::async_runtime::spawn(async move {
                if let Err(error) = session::restore(&handle) {
                    eprintln!("session restore: {error}");
                    if let Err(error) = tabs::ensure_bookmarks_host(&handle, tabs::WINDOW) {
                        eprintln!("bookmarks: {error}");
                    }
                }
                system::apply_launch(&handle);
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
            if !tabs::is_browser_label(&label) {
                return;
            }
            match event {
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                    let app = window.app_handle().clone();
                    let label = label.clone();
                    tauri::async_runtime::spawn(async move { tabs::layout(&app, &label) });
                }
                WindowEvent::CloseRequested { api, .. } => {
                    let app = window.app_handle().clone();
                    let others = tabs::browser_labels(&app).iter().any(|item| item != &label);
                    if others {
                        tabs::discard_window(&app, &label);
                        return;
                    }
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
            open_popup,
            close_popup,
            fit_popup,
            set_popup_shape,
            popup_host_ready,
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
            control_media,
            page_media,
            download_page_media,
            save_page_html,
            save_page_markdown,
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
            list_containers,
            create_container,
            delete_container,
            set_group_container,
            create_ephemeral_tab,
            network_log,
            clear_network_log,
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
            copy_text,
            show_notice,
            wayback_url,
            reading_stats,
            list_prices,
            track_price,
            check_prices,
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
            ask_page,
            find_deals,
            install_site,
            open_default_apps,
            set_device,
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
            default_settings,
            backdrop_support,
            update_status,
            check_for_update,
            install_update,
            release_notes,
            default_download_dir,
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
            open_profile,
            install_profile_shortcut,
            delete_profile,
            list_profile_icons,
            set_profile_icon,
            pin_profile_taskbar,
            unpin_profile_taskbar
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
