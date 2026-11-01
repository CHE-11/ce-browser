use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::webview::Color;
use tauri::{AppHandle, Emitter, Manager};

use crate::{files, paths};

const FILE: &str = "settings.json";

pub const SEARCH_ENGINES: &[(&str, &str)] = &[
    ("google", "https://www.google.com/search?q=%s"),
    ("bing", "https://www.bing.com/search?q=%s"),
    ("duckduckgo", "https://duckduckgo.com/?q=%s"),
    ("brave", "https://search.brave.com/search?q=%s"),
    ("startpage", "https://www.startpage.com/do/search?q=%s"),
    ("kagi", "https://kagi.com/search?q=%s"),
    ("mojeek", "https://www.mojeek.com/search?q=%s"),
];
const STARTUP_MODES: &[&str] = &["restore", "home"];
const COLOR_MODES: &[&str] = &["dark", "light", "system"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct BrowserSettings {
    #[serde(alias = "searchProvider")]
    pub search_engine: String,
    /// Used when `search_engine` is `custom`. `%s` is replaced with the query.
    pub custom_search_url: String,
    pub home_page: String,
    pub startup: String,
    /// URLs pinned every time this profile starts.
    #[serde(default)]
    pub pinned_tabs: Vec<String>,
    /// Empty means the system Downloads folder.
    pub download_dir: String,
    pub ask_where_to_save: bool,
    pub show_bookmarks_bar: bool,
    pub theme: String,
    pub color_mode: String,
    pub ui_zoom: u32,
    /// Default zoom for websites, as a percent. `ce://` pages stay on the UI size.
    #[serde(default = "default_page_zoom")]
    pub page_zoom: u32,
    /// Shared sync folder. Empty when sync is off. Kept on this computer, not copied into the folder.
    /// Older settings stored this as `syncFile`, a path to one combined file.
    #[serde(default, alias = "syncFile")]
    pub sync_folder: String,
    /// Highest revision applied from the sync folder. Older files stored only this.
    #[serde(default)]
    pub sync_revision: u64,
    #[serde(default)]
    pub sync_bookmarks_revision: u64,
    #[serde(default)]
    pub sync_extensions_revision: u64,
    #[serde(default)]
    pub sync_settings_revision: u64,
    #[serde(default)]
    pub sync_history_revision: u64,
    /// Last choices in the Print to PDF editor.
    #[serde(default)]
    pub print_editor: PrintEditorSettings,
    /// Last choices in the Page to PNG popover.
    #[serde(default)]
    pub page_capture: PageCaptureSettings,
    /// Extension ids left off the address bar. An older file omits this, so every extension stays pinned.
    #[serde(default)]
    pub hidden_toolbar_extensions: Vec<String>,
    /// Built-in ad blocker. An older file omits this, so blocking starts on with no paused sites.
    #[serde(default)]
    pub adblock: AdblockSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AdblockSettings {
    pub enabled: bool,
    /// Hosts where blocking is paused, lowercase and without a leading `www.`.
    pub allowed_sites: Vec<String>,
}

impl Default for AdblockSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            allowed_sites: Vec::new(),
        }
    }
}

impl AdblockSettings {
    fn normalize(mut self) -> Self {
        let mut sites: Vec<String> = Vec::new();
        for raw in &self.allowed_sites {
            let host = crate::adblock::site_key(raw);
            if host.is_empty()
                || host.len() > 253
                || host.chars().any(char::is_whitespace)
                || sites.contains(&host)
            {
                continue;
            }
            sites.push(host);
            if sites.len() >= 1000 {
                break;
            }
        }
        self.allowed_sites = sites;
        self
    }
}

const PRINT_MODES: &[&str] = &["article", "full"];
const IMAGE_SIZES: &[&str] = &["original", "large", "medium", "small", "none"];
const PRINT_FONTS: &[&str] = &["original", "serif", "sans"];
const PAPER_SIZES: &[&str] = &["letter", "a4", "legal", "tabloid"];
const MARGINS: &[&str] = &["none", "narrow", "normal", "wide"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct PrintEditorSettings {
    /// `article` keeps the main content only. `full` keeps the whole page.
    pub mode: String,
    pub remove_ads: bool,
    pub remove_navigation: bool,
    pub remove_popups: bool,
    pub remove_social: bool,
    pub remove_media: bool,
    /// Percent of the page's text size.
    pub text_size: u32,
    pub image_size: String,
    pub remove_backgrounds: bool,
    pub font: String,
    /// Title, source URL and date above the content.
    pub show_header: bool,
    pub paper: String,
    pub landscape: bool,
    pub margins: String,
    pub header_footer: bool,
    pub print_backgrounds: bool,
    pub open_after_save: bool,
}

impl Default for PrintEditorSettings {
    fn default() -> Self {
        Self {
            mode: "article".into(),
            remove_ads: true,
            remove_navigation: true,
            remove_popups: true,
            remove_social: true,
            remove_media: false,
            text_size: 100,
            image_size: "original".into(),
            remove_backgrounds: false,
            font: "original".into(),
            show_header: true,
            paper: "letter".into(),
            landscape: false,
            margins: "normal".into(),
            header_footer: false,
            print_backgrounds: true,
            open_after_save: true,
        }
    }
}

impl PrintEditorSettings {
    fn normalize(mut self) -> Self {
        let defaults = Self::default();
        let keep = |value: &mut String, allowed: &[&str], fallback: String| {
            if !allowed.contains(&value.as_str()) {
                *value = fallback;
            }
        };
        keep(&mut self.mode, PRINT_MODES, defaults.mode);
        keep(&mut self.image_size, IMAGE_SIZES, defaults.image_size);
        keep(&mut self.font, PRINT_FONTS, defaults.font);
        keep(&mut self.paper, PAPER_SIZES, defaults.paper);
        keep(&mut self.margins, MARGINS, defaults.margins);
        self.text_size = ((self.text_size.clamp(70, 150) + 2) / 5) * 5;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct PageCaptureSettings {
    /// The whole document, not only what is on screen.
    pub full_page: bool,
    /// Scroll through the page first so lazy images load.
    pub load_lazy: bool,
}

impl Default for PageCaptureSettings {
    fn default() -> Self {
        Self {
            full_page: true,
            load_lazy: true,
        }
    }
}

impl Default for BrowserSettings {
    fn default() -> Self {
        Self {
            search_engine: "google".into(),
            custom_search_url: String::new(),
            home_page: paths::BOOKMARKS.into(),
            startup: "restore".into(),
            pinned_tabs: Vec::new(),
            download_dir: String::new(),
            ask_where_to_save: false,
            show_bookmarks_bar: true,
            theme: "zinc".into(),
            color_mode: "dark".into(),
            ui_zoom: 100,
            page_zoom: default_page_zoom(),
            sync_folder: String::new(),
            sync_revision: 0,
            sync_bookmarks_revision: 0,
            sync_extensions_revision: 0,
            sync_settings_revision: 0,
            sync_history_revision: 0,
            print_editor: PrintEditorSettings::default(),
            page_capture: PageCaptureSettings::default(),
            hidden_toolbar_extensions: Vec::new(),
            adblock: AdblockSettings::default(),
        }
    }
}

impl BrowserSettings {
    pub fn normalize(mut self) -> Self {
        let defaults = Self::default();
        let custom_ok =
            self.search_engine == "custom" && self.custom_search_url.starts_with("http");
        if !custom_ok
            && !SEARCH_ENGINES
                .iter()
                .any(|(id, _)| *id == self.search_engine)
        {
            self.search_engine = defaults.search_engine;
        }
        if !STARTUP_MODES.contains(&self.startup.as_str()) {
            self.startup = defaults.startup;
        }
        if !COLOR_MODES.contains(&self.color_mode.as_str()) {
            self.color_mode = defaults.color_mode;
        }
        if self.theme.trim().is_empty() {
            self.theme = defaults.theme;
        }
        if self.home_page.trim().is_empty() || self.home_page == paths::NEW_TAB {
            self.home_page = defaults.home_page;
        }
        self.ui_zoom = ((self.ui_zoom.clamp(80, 150) + 2) / 5) * 5;
        self.page_zoom = snap_page_zoom(self.page_zoom);
        self.print_editor = self.print_editor.normalize();
        self.hidden_toolbar_extensions = toolbar_hidden(&self.hidden_toolbar_extensions);
        self.adblock = self.adblock.normalize();
        self.sync_folder = folder_from_stored(&self.sync_folder);
        if self.sync_bookmarks_revision == 0
            && self.sync_extensions_revision == 0
            && self.sync_settings_revision == 0
            && self.sync_history_revision == 0
            && self.sync_revision > 0
        {
            self.sync_bookmarks_revision = self.sync_revision;
            self.sync_extensions_revision = self.sync_revision;
            self.sync_settings_revision = self.sync_revision;
            self.sync_history_revision = self.sync_revision;
        }
        self
    }

    pub fn page_zoom_factor(&self) -> f64 {
        f64::from(self.page_zoom) / 100.0
    }

    pub fn search_url(&self) -> String {
        if self.search_engine == "custom" && !self.custom_search_url.is_empty() {
            return self.custom_search_url.clone();
        }
        SEARCH_ENGINES
            .iter()
            .find(|(id, _)| *id == self.search_engine)
            .map(|(_, url)| (*url).to_string())
            .unwrap_or_else(|| SEARCH_ENGINES[0].1.to_string())
    }

    pub fn download_dir(&self, app: &AppHandle) -> PathBuf {
        let configured = self.download_dir.trim();
        if !configured.is_empty() {
            return PathBuf::from(configured);
        }
        app.path()
            .download_dir()
            .unwrap_or_else(|_| paths::data_dir(app).join("Downloads"))
    }
}

pub struct SettingsState(Mutex<BrowserSettings>);

pub fn init(app: &AppHandle) {
    let mut loaded: BrowserSettings = files::read_json(&paths::data_file(app, FILE));
    loaded = loaded.normalize();
    loaded.pinned_tabs = canonicalize_pins(app, &loaded);
    crate::adblock::configure(&loaded.adblock);
    app.manage(SettingsState(Mutex::new(loaded)));
}

fn prefers_light(app: &AppHandle, color_mode: &str) -> bool {
    match color_mode {
        "light" => true,
        "dark" => false,
        _ => app
            .get_window(crate::tabs::WINDOW)
            .and_then(|window| window.theme().ok())
            .is_some_and(|theme| theme == tauri::Theme::Light),
    }
}

/// Popup surface (`--app-zinc-900`) for the saved theme, so a new webview matches the menu before CSS loads.
pub fn popup_surface(app: &AppHandle) -> (u8, u8, u8) {
    let settings = get(app);
    surface_rgb(&settings.theme, prefers_light(app, &settings.color_mode))
}

/// Page background (`--app-zinc-950`) for the saved theme.
pub fn page_surface(app: &AppHandle) -> (u8, u8, u8) {
    let settings = get(app);
    page_rgb(&settings.theme, prefers_light(app, &settings.color_mode))
}

pub fn page_color(app: &AppHandle) -> Color {
    let (r, g, b) = page_surface(app);
    Color(r, g, b, 255)
}

fn surface_rgb(theme: &str, light: bool) -> (u8, u8, u8) {
    let table: &[(&str, u8, u8, u8)] = if light {
        &[
            ("nord", 0xe5, 0xe9, 0xf0),
            ("catppuccin", 0xe6, 0xe9, 0xef),
            ("solarized", 0xee, 0xe8, 0xd5),
            ("dracula", 0xf0, 0xf0, 0xe8),
            ("gruvbox", 0xf2, 0xe5, 0xbc),
            ("tokyo", 0xd8, 0xdb, 0xe6),
            ("rosepine", 0xff, 0xfa, 0xf3),
            ("github", 0xf6, 0xf8, 0xfa),
            ("onedark", 0xf4, 0xf4, 0xf4),
        ]
    } else {
        &[
            ("nord", 0x3b, 0x42, 0x52),
            ("catppuccin", 0x1e, 0x1e, 0x2e),
            ("solarized", 0x07, 0x36, 0x42),
            ("dracula", 0x34, 0x37, 0x46),
            ("gruvbox", 0x32, 0x30, 0x2f),
            ("tokyo", 0x1f, 0x23, 0x35),
            ("rosepine", 0x1f, 0x1d, 0x2e),
            ("github", 0x16, 0x1b, 0x22),
            ("onedark", 0x2c, 0x31, 0x3a),
        ]
    };
    table
        .iter()
        .find(|(id, _, _, _)| *id == theme)
        .map(|(_, r, g, b)| (*r, *g, *b))
        .unwrap_or(if light {
            (0xf4, 0xf4, 0xf5)
        } else {
            (0x18, 0x18, 0x1b)
        })
}

fn page_rgb(theme: &str, light: bool) -> (u8, u8, u8) {
    let table: &[(&str, u8, u8, u8)] = if light {
        &[
            ("nord", 0xec, 0xef, 0xf4),
            ("catppuccin", 0xef, 0xf1, 0xf5),
            ("solarized", 0xfd, 0xf6, 0xe3),
            ("dracula", 0xf8, 0xf8, 0xf2),
            ("gruvbox", 0xfb, 0xf1, 0xc7),
            ("tokyo", 0xe1, 0xe2, 0xe7),
            ("rosepine", 0xfa, 0xf4, 0xed),
            ("github", 0xff, 0xff, 0xff),
            ("onedark", 0xfa, 0xfa, 0xfa),
        ]
    } else {
        &[
            ("nord", 0x2e, 0x34, 0x40),
            ("catppuccin", 0x11, 0x11, 0x1b),
            ("solarized", 0x00, 0x2b, 0x36),
            ("dracula", 0x28, 0x2a, 0x36),
            ("gruvbox", 0x28, 0x28, 0x28),
            ("tokyo", 0x1a, 0x1b, 0x26),
            ("rosepine", 0x19, 0x17, 0x24),
            ("github", 0x0d, 0x11, 0x17),
            ("onedark", 0x28, 0x2c, 0x34),
        ]
    };
    table
        .iter()
        .find(|(id, _, _, _)| *id == theme)
        .map(|(_, r, g, b)| (*r, *g, *b))
        .unwrap_or(if light {
            (0xfa, 0xfa, 0xfa)
        } else {
            (0x09, 0x09, 0x0b)
        })
}

pub fn reload(app: &AppHandle) {
    let mut loaded: BrowserSettings = files::read_json(&paths::data_file(app, FILE));
    loaded = loaded.normalize();
    loaded.pinned_tabs = canonicalize_pins(app, &loaded);
    if let Some(root) = paths::storage_root() {
        loaded.sync_folder = root.to_string_lossy().into_owned();
    }
    crate::adblock::configure(&loaded.adblock);
    let previous_page_zoom = {
        let state = app.state::<SettingsState>();
        let mut guard = state.0.lock().expect("settings lock");
        let previous = guard.page_zoom;
        *guard = loaded.clone();
        previous
    };
    if loaded.page_zoom != previous_page_zoom {
        crate::tabs::apply_default_page_zoom(app, previous_page_zoom, loaded.page_zoom);
    }
    let _ = app.emit("settings-changed", &loaded);
    crate::tabs::apply_page_background(app);
}

pub fn get(app: &AppHandle) -> BrowserSettings {
    app.state::<SettingsState>()
        .0
        .lock()
        .expect("settings lock")
        .clone()
}

pub fn save(app: &AppHandle, settings: BrowserSettings) -> Result<BrowserSettings, String> {
    let saved = store(app, settings, true)?;
    crate::sync::touch();
    Ok(saved)
}

/// Writes settings. When `keep_sync` is set, the sync path and revision already stored on this
/// computer stay as they are, so a settings change cannot clear them.
pub fn store(
    app: &AppHandle,
    settings: BrowserSettings,
    keep_sync: bool,
) -> Result<BrowserSettings, String> {
    let mut settings = settings.normalize();
    settings.pinned_tabs = canonicalize_pins(app, &settings);
    let (saved, previous_page_zoom) = {
        let state = app.state::<SettingsState>();
        let mut guard = state.0.lock().expect("settings lock");
        if keep_sync {
            settings.sync_folder.clone_from(&guard.sync_folder);
            settings.sync_revision = guard.sync_revision;
            settings.sync_bookmarks_revision = guard.sync_bookmarks_revision;
            settings.sync_extensions_revision = guard.sync_extensions_revision;
            settings.sync_settings_revision = guard.sync_settings_revision;
            settings.sync_history_revision = guard.sync_history_revision;
        }
        let previous_page_zoom = guard.page_zoom;
        files::write_json(&paths::data_file(app, FILE), &settings)?;
        crate::adblock::configure(&settings.adblock);
        *guard = settings.clone();
        (settings, previous_page_zoom)
    };
    if saved.page_zoom != previous_page_zoom {
        crate::tabs::apply_default_page_zoom(app, previous_page_zoom, saved.page_zoom);
    }
    let _ = app.emit("settings-changed", &saved);
    crate::tabs::apply_page_background(app);
    Ok(saved)
}

/// A stored `syncFile` points at one json file. The folder beside that file is the sync folder.
fn folder_from_stored(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let path = std::path::Path::new(trimmed);
    if path.is_dir() {
        return trimmed.to_string();
    }
    let looks_like_file = path.is_file() || path.extension().is_some();
    if looks_like_file {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                return parent.to_string_lossy().into_owned();
            }
        }
    }
    trimmed.to_string()
}

fn default_page_zoom() -> u32 {
    100
}

const PAGE_ZOOM_LEVELS: &[u32] = &[
    25, 33, 50, 67, 75, 80, 90, 100, 110, 125, 150, 175, 200, 250, 300,
];

fn snap_page_zoom(value: u32) -> u32 {
    PAGE_ZOOM_LEVELS
        .iter()
        .copied()
        .min_by_key(|level| level.abs_diff(value))
        .unwrap_or(100)
}

/// Drops blanks, duplicates and ids that cannot be an extension id.
fn toolbar_hidden(ids: &[String]) -> Vec<String> {
    let mut hidden = Vec::new();
    for id in ids {
        let id = id.trim();
        if id.is_empty()
            || id.len() > 128
            || id.chars().any(char::is_whitespace)
            || hidden.iter().any(|kept: &String| kept == id)
        {
            continue;
        }
        hidden.push(id.to_string());
        if hidden.len() >= 200 {
            break;
        }
    }
    hidden
}

fn canonicalize_pins(app: &AppHandle, settings: &BrowserSettings) -> Vec<String> {
    let search = settings.search_url();
    let mut pins = Vec::new();
    for raw in &settings.pinned_tabs {
        let Some(url) = paths::pin_url(app, raw, &search) else {
            continue;
        };
        if pins
            .iter()
            .any(|existing: &String| paths::pin_key(existing) == paths::pin_key(&url))
        {
            continue;
        }
        pins.push(url);
        if pins.len() >= 50 {
            break;
        }
    }
    pins
}

pub fn update(
    app: &AppHandle,
    change: impl FnOnce(&mut BrowserSettings),
) -> Result<BrowserSettings, String> {
    let mut settings = get(app);
    change(&mut settings);
    save(app, settings)
}

#[cfg(test)]
mod tests {
    use super::{folder_from_stored, snap_page_zoom};

    #[test]
    fn the_old_new_tab_home_page_opens_bookmarks() {
        let mut settings = super::BrowserSettings::default();
        settings.home_page = crate::paths::NEW_TAB.into();
        assert_eq!(settings.normalize().home_page, crate::paths::BOOKMARKS);
        let mut custom = super::BrowserSettings::default();
        custom.home_page = "https://example.com".into();
        assert_eq!(custom.normalize().home_page, "https://example.com");
    }

    #[test]
    fn older_settings_get_page_tool_defaults() {
        let loaded: super::BrowserSettings =
            serde_json::from_str(r#"{"searchEngine":"bing","printEditor":{"paper":"a9","textSize":400}}"#)
                .expect("settings parse");
        let loaded = loaded.normalize();
        assert_eq!(loaded.search_engine, "bing");
        assert_eq!(loaded.print_editor.paper, "letter");
        assert_eq!(loaded.print_editor.text_size, 150);
        assert!(loaded.print_editor.remove_ads);
        assert!(loaded.page_capture.full_page);
        assert!(loaded.hidden_toolbar_extensions.is_empty());
    }

    #[test]
    fn older_settings_get_ad_blocker_defaults() {
        let loaded: super::BrowserSettings =
            serde_json::from_str(r#"{"searchEngine":"bing"}"#).expect("settings parse");
        let loaded = loaded.normalize();
        assert!(loaded.adblock.enabled);
        assert!(loaded.adblock.allowed_sites.is_empty());
    }

    #[test]
    fn paused_sites_are_cleaned_up() {
        let loaded: super::BrowserSettings = serde_json::from_str(
            r#"{"adblock":{"enabled":false,"allowedSites":[" WWW.Example.com ","example.com","","a b"]}}"#,
        )
        .expect("settings parse");
        let loaded = loaded.normalize();
        assert!(!loaded.adblock.enabled);
        assert_eq!(loaded.adblock.allowed_sites, vec!["example.com".to_string()]);
    }

    #[test]
    fn toolbar_pins_ignore_blank_and_duplicate_ids() {
        let mut settings = super::BrowserSettings::default();
        settings.hidden_toolbar_extensions = vec![
            " print-pdf ".into(),
            "print-pdf".into(),
            String::new(),
            "not an id".into(),
        ];
        assert_eq!(
            settings.normalize().hidden_toolbar_extensions,
            vec!["print-pdf".to_string()]
        );
    }

    #[test]
    fn page_zoom_snaps_to_a_page_level() {
        assert_eq!(snap_page_zoom(100), 100);
        assert_eq!(snap_page_zoom(0), 25);
        assert_eq!(snap_page_zoom(130), 125);
        assert_eq!(snap_page_zoom(900), 300);
    }

    #[test]
    fn a_sync_file_path_becomes_its_folder() {
        assert_eq!(folder_from_stored(r"D:\share\ce-browser-sync.json"), r"D:\share");
        assert_eq!(folder_from_stored(""), "");
    }

    #[test]
    fn an_existing_folder_stays_a_folder() {
        let dir = std::env::temp_dir();
        let stored = folder_from_stored(dir.to_string_lossy().as_ref());
        assert_eq!(stored, dir.to_string_lossy());
    }
}
