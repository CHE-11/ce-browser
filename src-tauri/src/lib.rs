mod adblock;
mod bookmarks;
mod capture;
mod downloads;
mod history;
mod extensions;
mod files;
mod paths;
mod print;
mod profile_icons;
mod profiles;
mod session;
mod settings;
mod shortcuts;
mod sync;
mod tabs;
mod user_styles;
mod webview2;
mod what_runs;

use std::path::PathBuf;

use tauri::{AppHandle, Manager, Window, WindowEvent};

use adblock::AdblockStats;
use bookmarks::{BookmarkNode, BookmarkTree};
use capture::PageCaptureResult;
use print::{ExtractedPage, PdfOptions};
use downloads::DownloadItem;
use history::HistoryItem;
use extensions::ExtensionInfo;
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
async fn duplicate_tab(app: AppHandle, id: u32) -> Result<u32, String> {
    tabs::duplicate_tab(&app, id)
}

#[tauri::command]
async fn reopen_closed_tab(app: AppHandle) -> Result<Option<u32>, String> {
    tabs::reopen_closed(&app)
}

#[tauri::command]
async fn set_ui_height(app: AppHandle, window: Window, height: f64) {
    tabs::set_ui_height(&app, window.label(), height);
}

#[tauri::command]
async fn zoom_tab(app: AppHandle, id: u32, change: String) -> Result<f64, String> {
    tabs::zoom(&app, id, &change)
}

#[tauri::command]
async fn find_in_page(
    app: AppHandle,
    id: u32,
    query: String,
    backwards: bool,
) -> Result<bool, String> {
    blocking(move || tabs::find_in_page(&app, id, &query, backwards)).await
}

#[tauri::command]
async fn clear_find(app: AppHandle, id: u32) {
    tabs::clear_find(&app, id);
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
async fn resize_popup(
    app: AppHandle,
    window: Window,
    width: Option<f64>,
    height: f64,
) -> Result<(), String> {
    tabs::resize_popup(&app, window.label(), width, height)
}

#[tauri::command]
async fn detach_tab(app: AppHandle, id: u32) -> Result<(), String> {
    tabs::detach_tab(&app, id)
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
fn delete_bookmark(app: AppHandle, id: String) -> Result<(), String> {
    bookmarks::delete(&app, &id)
}

#[tauri::command]
fn find_bookmark(app: AppHandle, url: String) -> Option<BookmarkNode> {
    bookmarks::find_by_url(&app, &url)
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
fn remove_history(app: AppHandle, id: u32) {
    history::remove(&app, id);
}

#[tauri::command]
fn clear_history(app: AppHandle) {
    history::clear(&app);
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
fn get_settings(app: AppHandle) -> BrowserSettings {
    settings::get(&app)
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: BrowserSettings) -> Result<BrowserSettings, String> {
    settings::save(&app, settings)
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
    profiles::read_args();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            sync::attach(&handle);
            profiles::init(&handle);
            if let Err(error) = profiles::set_process_identity() {
                eprintln!("profile identity: {error}");
            }
            settings::init(&handle);
            if let Err(error) = user_styles::ensure(&handle) {
                eprintln!("user styles: {error}");
            }
            bookmarks::init(&handle);
            downloads::init(&handle);
            history::init(&handle);
            extensions::init(&handle);
            tabs::init(&handle);
            session::init(&handle);
            build_window(&handle)?;

            tauri::async_runtime::spawn(async move {
                if let Err(error) = session::restore(&handle) {
                    eprintln!("session restore: {error}");
                }
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
                    let closing = label.clone();
                    tauri::async_runtime::spawn(async move {
                        let flush_app = app.clone();
                        let _ =
                            tauri::async_runtime::spawn_blocking(move || sync::flush(&flush_app))
                                .await;
                        sync::allow_close();
                        if let Some(main) = app.get_webview_window(&closing) {
                            let _ = main.close();
                        }
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
            move_tab,
            detach_tab,
            pin_tab,
            unpin_tab,
            duplicate_tab,
            reopen_closed_tab,
            set_ui_height,
            zoom_tab,
            find_in_page,
            clear_find,
            open_devtools,
            open_popup,
            close_popup,
            resize_popup,
            capture_page_png,
            copy_page_png,
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
            find_bookmark,
            import_bookmarks_html,
            export_bookmarks_html,
            list_downloads,
            pause_download,
            resume_download,
            cancel_download,
            open_download,
            show_download_in_folder,
            remove_download,
            clear_downloads,
            list_history,
            remove_history,
            clear_history,
            list_extensions,
            install_extension_folder,
            install_extension_zip,
            set_extension_enabled,
            remove_extension,
            open_extension_options,
            get_settings,
            save_settings,
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
