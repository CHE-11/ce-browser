use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::webview::Color;
use tauri::{AppHandle, Emitter, Manager};

use crate::paths;

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
const UI_FONTS: &[&str] = &[
    "system",
    "inter",
    "source-sans",
    "roboto",
    "nunito",
    "atkinson",
    "plex",
    "source-serif",
    "space-grotesk",
    "bricolage",
    "fraunces",
    "literata",
    "newsreader",
    "jetbrains-mono",
    "space-mono",
    "martian-mono",
    "fira-code",
    "source-code",
    "share-tech-mono",
    "oxanium",
    "exo-2",
    "jura",
    "shantell",
    "comic-neue",
];
const BOOKMARK_TEXT_REM: (f32, f32) = (0.5, 1.5);
const BOOKMARK_PADDING_REM: (f32, f32) = (0.0, 1.5);
const TAB_WIDTH_PX: (u32, u32) = (88, 480);
/// Icons on tabs, menus, and the toolbar, in rem.
const ICON_REM: (f32, f32) = (0.5, 3.0);
/// Row and bar heights that always take space, in rem.
const HEIGHT_REM: (f32, f32) = (1.0, 4.0);
/// A menu row height that may be 0, meaning the text and padding decide.
const OPTIONAL_HEIGHT_REM: (f32, f32) = (0.0, 4.0);
const MENU_WIDTH_PX: (u32, u32) = (160, 480);
const PANEL_WIDTH_PX: (u32, u32) = (240, 720);
/// Docked agent panel, in CSS pixels. Narrower than this clips the composer.
pub const AGENT_PANEL_WIDTH_PX: (u32, u32) = (280, 720);
pub const AGENT_PANEL_DEFAULT_WIDTH: u32 = 380;
/// Height of the bookmark editor's place list, in rem.
const LIST_HEIGHT_REM: (f32, f32) = (8.0, 36.0);
const PALETTE_WIDTH_PX: (u32, u32) = (360, 880);
/// Fine-tune sizes the Styles page copies from another profile. Theme, font, and zoom stay out.
pub const STYLE_METRIC_KEYS: &[&str] = &[
    "tabWidth",
    "verticalTabsWidth",
    "tabItemTextSize",
    "tabItemPadding",
    "tabItemHeight",
    "tabIconSize",
    "tabMenuTextSize",
    "tabMenuPadding",
    "tabMenuIconSize",
    "tabMenuHeight",
    "tabMenuWidth",
    "bookmarksBarTextSize",
    "bookmarksBarPadding",
    "bookmarksBarIconSize",
    "bookmarksBarHeight",
    "bookmarkFolderTextSize",
    "bookmarkFolderPadding",
    "bookmarkFolderIconSize",
    "bookmarksBarMenuTextSize",
    "bookmarksBarMenuPadding",
    "bookmarksBarMenuIconSize",
    "bookmarksBarMenuWidth",
    "bookmarkEditorTextSize",
    "bookmarkEditorPadding",
    "bookmarkEditorRowPadding",
    "bookmarkEditorIconSize",
    "bookmarkEditorListHeight",
    "bookmarkEditorWidth",
    "menuTextSize",
    "menuPadding",
    "menuHeight",
    "menuIconSize",
    "toolbarHeight",
    "toolbarButtonSize",
    "toolbarIconSize",
    "addressHeight",
    "addressTextSize",
    "omniboxRowHeight",
    "omniboxTextSize",
    "omniboxIconSize",
    "omniboxUrlSize",
    "downloadsPanelWidth",
    "downloadsTextSize",
    "downloadsPadding",
    "downloadsRowPadding",
    "downloadsIconSize",
    "historyPanelWidth",
    "historyTextSize",
    "historyMetaSize",
    "historyPadding",
    "historyRowPadding",
    "historyIconSize",
    "commandPaletteWidth",
    "commandPaletteInputHeight",
    "commandPaletteRowHeight",
    "commandPaletteTextSize",
    "commandPaletteHintSize",
];
pub const AI_PROVIDER_IDS: &[&str] = &["openai", "claude", "custom", "local"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiProviderSlot {
    pub endpoint: String,
    pub model: String,
    /// `low`, `medium`, or `high` when the model should think. Empty keeps the provider default.
    #[serde(default)]
    pub thinking: String,
}

impl Default for AiProviderSlot {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            model: String::new(),
            thinking: String::new(),
        }
    }
}

/// One saved endpoint and model per provider. Keys live in DPAPI, not here.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiProviders {
    pub openai: AiProviderSlot,
    pub claude: AiProviderSlot,
    pub custom: AiProviderSlot,
    pub local: AiProviderSlot,
}

impl Default for AiProviders {
    fn default() -> Self {
        Self {
            openai: AiProviderSlot {
                endpoint: crate::ai::OPENAI_ENDPOINT.into(),
                model: "gpt-4o-mini".into(),
                thinking: String::new(),
            },
            claude: AiProviderSlot {
                endpoint: crate::ai::CLAUDE_ENDPOINT.into(),
                model: "claude-3-5-haiku-latest".into(),
                thinking: String::new(),
            },
            custom: AiProviderSlot::default(),
            local: AiProviderSlot {
                endpoint: crate::ai::LOCAL_ENDPOINT.into(),
                model: "llama3.2".into(),
                thinking: String::new(),
            },
        }
    }
}

/// Page that opens one file type inside the browser.
pub const DEFAULT_PDF_EDITOR: &str = "https://ce-file-tools.vercel.app/pdf/view-and-markup";
pub const DEFAULT_PDF_INPUT: &str = "pdf-dropzone-input";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct FileHandler {
    /// Lowercase extension without a dot, such as `pdf`.
    pub extension: String,
    /// `http` or `https` page. The file is placed on that page's file input.
    pub url: String,
    /// Element id of the file input. Empty uses the first file input on the page.
    pub input_id: String,
    /// Profile that opens this type. Empty uses an open profile, or the default profile.
    pub profile: String,
}

impl Default for FileHandler {
    fn default() -> Self {
        Self {
            extension: String::new(),
            url: String::new(),
            input_id: String::new(),
            profile: String::new(),
        }
    }
}

/// Used when `settings.json` has no `fileHandlers` key. A saved empty list stays empty.
pub fn default_file_handlers() -> Vec<FileHandler> {
    vec![FileHandler {
        extension: "pdf".into(),
        url: DEFAULT_PDF_EDITOR.into(),
        input_id: DEFAULT_PDF_INPUT.into(),
        profile: String::new(),
    }]
}

fn clean_extension(raw: &str) -> Option<String> {
    let text = raw.trim().trim_start_matches('.').to_ascii_lowercase();
    if (1..=16).contains(&text.len())
        && text
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
    {
        Some(text)
    } else {
        None
    }
}

fn clean_handler_url(raw: &str) -> Option<String> {
    let text = raw.trim();
    if text.chars().any(|character| character.is_whitespace()) {
        return None;
    }
    let lower = text.to_ascii_lowercase();
    if !lower.starts_with("https://") && !lower.starts_with("http://") {
        return None;
    }
    let rest = text.split_once("://")?.1;
    if rest.is_empty() || rest.starts_with('/') {
        return None;
    }
    Some(text.chars().take(500).collect())
}

fn clean_profile(raw: &str) -> String {
    let text = raw.trim();
    if text.is_empty()
        || text.len() > 64
        || !text
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return String::new();
    }
    text.to_string()
}

fn clean_input_id(raw: &str) -> String {
    raw.trim()
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || *character == '-' || *character == '_'
        })
        .take(80)
        .collect()
}

fn normalize_file_handlers(handlers: Vec<FileHandler>) -> Vec<FileHandler> {
    let mut kept = Vec::new();
    for handler in handlers {
        let Some(extension) = clean_extension(&handler.extension) else {
            continue;
        };
        let Some(url) = clean_handler_url(&handler.url) else {
            continue;
        };
        if kept
            .iter()
            .any(|item: &FileHandler| item.extension == extension)
        {
            continue;
        }
        kept.push(FileHandler {
            extension,
            url,
            input_id: clean_input_id(&handler.input_id),
            profile: clean_profile(&handler.profile),
        });
        if kept.len() >= 40 {
            break;
        }
    }
    kept
}

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
    /// Large files use four connections when the server accepts ranges.
    #[serde(default = "default_true")]
    pub accelerate_downloads: bool,
    /// Empty keeps the name the site suggested. Tokens: `{site}`, `{date}`, `{name}`, `{ext}`.
    #[serde(default)]
    pub download_name_template: String,
    pub show_bookmarks_bar: bool,
    /// Folder icons on the bookmarks bar. Off leaves folder titles only. Site favicons always show.
    #[serde(default = "default_true")]
    pub bookmarks_bar_folder_icons: bool,
    /// Text size of bookmarks on the bar, in rem.
    #[serde(default = "default_bar_text_size")]
    pub bookmarks_bar_text_size: f32,
    /// Side padding of bookmarks on the bar, in rem. Top and bottom get half.
    #[serde(default = "default_bar_padding")]
    pub bookmarks_bar_padding: f32,
    /// Text size of rows in a bookmark folder menu, in rem.
    #[serde(default = "default_folder_text_size")]
    pub bookmark_folder_text_size: f32,
    /// Side padding of rows in a bookmark folder menu, in rem. Top and bottom get half.
    #[serde(default = "default_folder_padding")]
    pub bookmark_folder_padding: f32,
    /// Text size of the bookmarks bar context menu, in rem.
    #[serde(default = "default_menu_text_size")]
    pub bookmarks_bar_menu_text_size: f32,
    /// Side padding of rows in the bookmarks bar context menu, in rem. Top and bottom get half.
    #[serde(default = "default_menu_padding")]
    pub bookmarks_bar_menu_padding: f32,
    /// Favicon and folder icon on the bookmarks bar, in rem.
    #[serde(default = "default_bookmarks_bar_icon_size")]
    pub bookmarks_bar_icon_size: f32,
    /// Minimum height of a bookmarks bar item, in rem.
    #[serde(default = "default_bookmarks_bar_height")]
    pub bookmarks_bar_height: f32,
    /// Icon in a bookmark folder menu, in rem.
    #[serde(default = "default_bookmark_folder_icon_size")]
    pub bookmark_folder_icon_size: f32,
    /// Icon in the bookmarks bar context menu, in rem.
    #[serde(default = "default_bookmarks_bar_menu_icon_size")]
    pub bookmarks_bar_menu_icon_size: f32,
    /// Width of the bookmarks bar context menu, in CSS pixels at 100% UI size.
    #[serde(default = "default_bookmarks_bar_menu_width")]
    pub bookmarks_bar_menu_width: u32,
    /// Text size of the bookmark editor, in rem.
    #[serde(default = "default_bookmark_editor_text_size")]
    pub bookmark_editor_text_size: f32,
    /// Padding around the bookmark editor, in rem.
    #[serde(default = "default_bookmark_editor_padding")]
    pub bookmark_editor_padding: f32,
    /// Side padding of a row in the bookmark editor, in rem. Top and bottom get half.
    #[serde(default = "default_bookmark_editor_row_padding")]
    pub bookmark_editor_row_padding: f32,
    /// Favicon in the bookmark editor's place list, in rem.
    #[serde(default = "default_bookmark_editor_icon_size")]
    pub bookmark_editor_icon_size: f32,
    /// Height of the place list in the bookmark editor, in rem.
    #[serde(default = "default_bookmark_editor_list_height")]
    pub bookmark_editor_list_height: f32,
    /// Width of the bookmark editor, in CSS pixels at 100% UI size.
    #[serde(default = "default_bookmark_editor_width")]
    pub bookmark_editor_width: u32,
    /// Retired preset (`small`, `default`, `large`, `xlarge`). Read once and mapped onto the
    /// four values above.
    #[serde(rename = "bookmarkSize", skip_serializing)]
    pub legacy_bookmark_size: Option<String>,
    /// The status bar under every page. An older file omits this, so the bar stays on.
    #[serde(default = "default_true")]
    pub show_status_bar: bool,
    /// Memory, time and the other status readings. Off leaves the version and zoom.
    #[serde(default = "default_true")]
    pub status_stats: bool,
    pub theme: String,
    pub color_mode: String,
    /// Browser UI font. An older file omits this and keeps Segoe UI.
    #[serde(default = "default_ui_font")]
    pub ui_font: String,
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
    /// Last choices in the Webpage to Markdown popover.
    #[serde(default)]
    pub page_markdown: PageMarkdownSettings,
    /// Extension ids left off the toolbars. An older file omits this, so every extension stays pinned.
    #[serde(default)]
    pub hidden_toolbar_extensions: Vec<String>,
    /// Order of pinned toolbar icons. Empty keeps the built-in order, then extensions.
    #[serde(default)]
    pub toolbar_order: Vec<String>,
    /// Built-in tools sit on the right strip. Off puts them on the address bar.
    #[serde(default = "default_true")]
    pub vertical_toolbar: bool,
    /// Pinned extensions share the vertical tool strip on the right.
    #[serde(default)]
    pub vertical_toolbar_extensions: bool,
    /// Short names under icons on the vertical tool strip.
    #[serde(default)]
    pub vertical_toolbar_labels: bool,
    /// Tint the active tab and address bar from the site's theme color or favicon.
    #[serde(default = "default_true")]
    pub site_accent: bool,
    /// Site theme color or favicon along the top of pinned tabs. An older file omits this and gets the border.
    #[serde(default = "default_true")]
    pub pin_tab_border: bool,
    /// Target language for the translate tool. `%t` in the provider URL.
    #[serde(default = "default_translate_language")]
    pub translate_language: String,
    /// Provider URL. `%s` is the text and `%t` is the language code.
    #[serde(default = "default_translate_url")]
    pub translate_url: String,
    /// Active Shopping summary provider: `openai`, `claude`, `custom`, or `local`.
    /// Empty means an older file has not been migrated yet.
    #[serde(default)]
    pub ai_provider: String,
    #[serde(default)]
    pub ai_providers: AiProviders,
    /// Older installs stored the endpoint here. Read once, then dropped.
    #[serde(default, alias = "assistantEndpoint", skip_serializing)]
    pub legacy_assistant_endpoint: Option<String>,
    #[serde(default, alias = "assistantModel", skip_serializing)]
    pub legacy_assistant_model: Option<String>,
    /// Older installs stored a plaintext key here. Moved to DPAPI, then dropped.
    #[serde(default, alias = "assistantKey", skip_serializing)]
    pub legacy_assistant_key: Option<String>,
    /// DuckDuckGo region (`kl`) for shopping searches, for example `us-en`.
    #[serde(default = "default_shopping_region")]
    pub shopping_region: String,
    /// Model override for Shopping summaries. Blank uses the active provider's model.
    #[serde(default)]
    pub shopping_model: String,
    /// Store hosts used for one extra deal search, for example `walmart.com`. Empty skips that query.
    #[serde(default)]
    pub shopping_competitors: Vec<String>,
    /// Restores the window and focuses the address bar. Default is Ctrl+Alt+Space.
    #[serde(default = "default_hotkey")]
    pub summon_hotkey: String,
    /// On battery, background tabs sleep sooner and the visible page stays near 30 frames a second.
    #[serde(default = "default_true")]
    pub battery_saver: bool,
    /// Built-in ad blocker. An older file omits this, so blocking starts on with no paused sites.
    #[serde(default)]
    pub adblock: AdblockSettings,
    /// The workspace button on the tab strip. An older file omits this, so workspaces stay on.
    #[serde(default = "default_true")]
    pub workspaces_enabled: bool,
    /// `horizontal` puts tabs above the address bar, `vertical` in a sidebar on the left.
    #[serde(default)]
    pub tab_layout: String,
    /// The vertical sidebar shows icons only.
    #[serde(default)]
    pub vertical_tabs_collapsed: bool,
    /// Width of the expanded vertical sidebar in CSS pixels.
    #[serde(default = "default_vertical_tabs_width")]
    pub vertical_tabs_width: u32,
    /// Tabs that are not on screen are muted.
    #[serde(default)]
    pub mute_background_tabs: bool,
    /// Shift+click on a link opens it in a peek instead of a new tab.
    #[serde(default = "default_true")]
    pub peek_on_shift_click: bool,
    /// Background tabs are suspended after a while.
    #[serde(default)]
    pub tab_sleep: TabSleepSettings,
    /// Drop `utm_*` and other click identifiers from URLs.
    #[serde(default = "default_true")]
    pub strip_tracking: bool,
    /// Add stable noise to canvas, WebGL and audio reads.
    #[serde(default)]
    pub fingerprint_shield: bool,
    /// Load `http` pages over `https` unless the site is excepted.
    #[serde(default)]
    pub https_only: bool,
    /// Hosts that may stay on `http`, lowercase and without a leading `www.`.
    #[serde(default)]
    pub https_exceptions: Vec<String>,
    /// Save page text on this computer so history search can match words from the page.
    #[serde(default = "default_true")]
    pub index_page_text: bool,
    /// Ctrl+Tab order: `strip` follows the tab strip, `recent` goes to the last used tab first.
    #[serde(default = "default_tab_cycle_order")]
    pub tab_cycle_order: String,
    /// In recent order, Ctrl+Tab shows a switcher with thumbnails until Ctrl is released.
    #[serde(default = "default_true")]
    pub tab_switcher: bool,
    /// Resting on a tab shows a small preview of its page.
    #[serde(default = "default_true")]
    pub tab_hover_preview: bool,
    /// Preferred width of a horizontal tab, in CSS pixels. Tabs shrink when the strip is full.
    #[serde(default = "default_tab_width")]
    pub tab_width: u32,
    /// Text size of a tab title, in rem.
    #[serde(default = "default_tab_item_text_size")]
    pub tab_item_text_size: f32,
    /// Side padding of a tab, in rem. Top and bottom get half.
    #[serde(default = "default_tab_item_padding")]
    pub tab_item_padding: f32,
    /// Text size of the tab context menu, in rem.
    #[serde(default = "default_tab_menu_text_size")]
    pub tab_menu_text_size: f32,
    /// Side padding of rows in the tab context menu, in rem. Top and bottom get half.
    #[serde(default = "default_tab_menu_padding")]
    pub tab_menu_padding: f32,
    /// Minimum height of a horizontal tab, in rem.
    #[serde(default = "default_tab_item_height")]
    pub tab_item_height: f32,
    /// Favicon on a tab, in rem. The close icon is 0.875 times this.
    #[serde(default = "default_tab_icon_size")]
    pub tab_icon_size: f32,
    /// Icon in the tab context menu, in rem.
    #[serde(default = "default_tab_menu_icon_size")]
    pub tab_menu_icon_size: f32,
    /// Minimum height of a tab context menu row, in rem. Zero keeps the height of the text and padding.
    #[serde(default)]
    pub tab_menu_height: f32,
    /// Width of the tab context menu, in CSS pixels at 100% UI size.
    #[serde(default = "default_tab_menu_width")]
    pub tab_menu_width: u32,
    /// Text size of shared menu rows, in rem. Tab and bookmark menus keep their own sizes.
    #[serde(default = "default_menu_text_size")]
    pub menu_text_size: f32,
    /// Side padding of shared menu rows, in rem.
    #[serde(default = "default_menu_padding")]
    pub menu_padding: f32,
    /// Height of a shared menu row, in rem.
    #[serde(default = "default_menu_height")]
    pub menu_height: f32,
    /// Icon in a shared menu row, in rem.
    #[serde(default = "default_menu_icon_size")]
    pub menu_icon_size: f32,
    /// Height of the address bar, in rem.
    #[serde(default = "default_toolbar_height")]
    pub toolbar_height: f32,
    /// Address-bar and side-strip buttons, in rem.
    #[serde(default = "default_toolbar_button_size")]
    pub toolbar_button_size: f32,
    /// Icons in those buttons, in rem.
    #[serde(default = "default_toolbar_icon_size")]
    pub toolbar_icon_size: f32,
    /// Height of the address field, in rem.
    #[serde(default = "default_address_height")]
    pub address_height: f32,
    /// Text in the address field, in rem.
    #[serde(default = "default_address_text_size")]
    pub address_text_size: f32,
    /// Height of an omnibox suggestion, in rem.
    #[serde(default = "default_omnibox_row_height")]
    pub omnibox_row_height: f32,
    /// Title in an omnibox suggestion, in rem.
    #[serde(default = "default_menu_text_size")]
    pub omnibox_text_size: f32,
    /// Icon in an omnibox suggestion, in rem.
    #[serde(default = "default_menu_icon_size")]
    pub omnibox_icon_size: f32,
    /// Address beside an omnibox title, in rem.
    #[serde(default = "default_omnibox_url_size")]
    pub omnibox_url_size: f32,
    /// Width of the downloads panel, in CSS pixels at 100% UI size.
    #[serde(default = "default_downloads_panel_width")]
    pub downloads_panel_width: u32,
    /// File name in the downloads panel, in rem.
    #[serde(default = "default_menu_text_size")]
    pub downloads_text_size: f32,
    /// Side padding of a downloads panel row, in rem.
    #[serde(default = "default_panel_padding")]
    pub downloads_padding: f32,
    /// Top and bottom padding of a downloads panel row, in rem.
    #[serde(default = "default_panel_row_padding")]
    pub downloads_row_padding: f32,
    /// File icon in the downloads panel, in rem.
    #[serde(default = "default_downloads_icon_size")]
    pub downloads_icon_size: f32,
    /// Width of the history popover, in CSS pixels at 100% UI size.
    #[serde(default = "default_history_panel_width")]
    pub history_panel_width: u32,
    /// Title in the history popover, in rem.
    #[serde(default = "default_menu_text_size")]
    pub history_text_size: f32,
    /// Host and time in the history popover, in rem.
    #[serde(default = "default_omnibox_url_size")]
    pub history_meta_size: f32,
    /// Side padding of a history popover row, in rem.
    #[serde(default = "default_panel_padding")]
    pub history_padding: f32,
    /// Top and bottom padding of a history popover row, in rem.
    #[serde(default = "default_panel_row_padding")]
    pub history_row_padding: f32,
    /// Favicon in the history popover, in rem.
    #[serde(default = "default_menu_icon_size")]
    pub history_icon_size: f32,
    /// Width of the command palette, in CSS pixels at 100% UI size.
    #[serde(default = "default_command_palette_width")]
    pub command_palette_width: u32,
    /// Height of the command palette search field, in rem.
    #[serde(default = "default_command_palette_input_height")]
    pub command_palette_input_height: f32,
    /// Height of a command palette row, in rem.
    #[serde(default = "default_command_palette_row_height")]
    pub command_palette_row_height: f32,
    /// Title in a command palette row, in rem.
    #[serde(default = "default_menu_text_size")]
    pub command_palette_text_size: f32,
    /// Hint in a command palette row, in rem.
    #[serde(default = "default_omnibox_url_size")]
    pub command_palette_hint_size: f32,
    /// `none`, `mica` or `acrylic`. Windows draws it behind the tab strip and toolbars.
    #[serde(default = "default_backdrop")]
    pub window_backdrop: String,
    /// Check for a new version at startup and once a day.
    #[serde(default = "default_true")]
    pub auto_update: bool,
    /// Version that last ran.
    #[serde(default)]
    pub last_seen_version: String,
    /// Corner or edge of the page area where toasts appear, such as `bottom-right`.
    #[serde(default = "default_toast_position")]
    pub toast_position: String,
    /// Pages that open files of each type inside the browser. Missing means the PDF editor.
    #[serde(default = "default_file_handlers")]
    pub file_handlers: Vec<FileHandler>,
    /// Width of the docked agent panel, in CSS pixels. An older file omits this and opens at 380.
    #[serde(default = "default_agent_panel_width")]
    pub agent_panel_width: u32,
    /// Agent, recorded flows, schedules and the MCP server. An older file omits this.
    #[serde(default)]
    pub automation: AutomationSettings,
}

pub const DEFAULT_MCP_PORT: u16 = 17345;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AutomationSettings {
    pub enabled: bool,
    /// Model for the agent. Blank uses the AI provider's model.
    pub agent_model: String,
    /// Actions one agent run may take before it stops.
    pub max_steps: u32,
    /// Ask before purchases, sends, deletes, uploads and similar actions.
    pub confirm_sensitive: bool,
    /// When not empty, automation may only act on these hosts without asking.
    pub allowed_sites: Vec<String>,
    /// Hosts automation never touches.
    pub blocked_sites: Vec<String>,
    /// Send a screenshot with each step. Off sends the text snapshot only.
    pub screenshots_to_model: bool,
    pub mcp_enabled: bool,
    pub mcp_port: u16,
    /// MCP clients may read pages but not click, type or navigate.
    pub mcp_read_only: bool,
    /// MCP clients may run scripts in pages.
    pub mcp_allow_evaluate: bool,
    /// Scheduled flows and monitors run.
    pub scheduler_enabled: bool,
    /// Scheduled runs wait while the PC is on battery.
    pub pause_on_battery: bool,
}

impl Default for AutomationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            agent_model: String::new(),
            max_steps: 30,
            confirm_sensitive: true,
            allowed_sites: Vec::new(),
            blocked_sites: Vec::new(),
            screenshots_to_model: true,
            mcp_enabled: false,
            mcp_port: DEFAULT_MCP_PORT,
            mcp_read_only: false,
            mcp_allow_evaluate: false,
            scheduler_enabled: true,
            pause_on_battery: true,
        }
    }
}

impl AutomationSettings {
    fn normalize(mut self) -> Self {
        self.agent_model = self.agent_model.trim().chars().take(120).collect();
        self.max_steps = self.max_steps.clamp(5, 200);
        if self.mcp_port < 1024 {
            self.mcp_port = DEFAULT_MCP_PORT;
        }
        self.allowed_sites = automation_hosts(&self.allowed_sites);
        self.blocked_sites = automation_hosts(&self.blocked_sites);
        self
    }
}

fn automation_hosts(list: &[String]) -> Vec<String> {
    let mut sites: Vec<String> = Vec::new();
    for raw in list {
        let host = sleep_site_key(raw);
        let host = if host.is_empty() {
            crate::adblock::site_key(raw.trim())
        } else {
            host
        };
        if host.is_empty()
            || host.len() > 253
            || host.chars().any(char::is_whitespace)
            || sites.contains(&host)
        {
            continue;
        }
        sites.push(host);
        if sites.len() >= 500 {
            break;
        }
    }
    sites
}

const TAB_LAYOUTS: &[&str] = &["horizontal", "vertical"];
pub const TOAST_POSITIONS: &[&str] = &[
    "top-left",
    "top-center",
    "top-right",
    "bottom-left",
    "bottom-center",
    "bottom-right",
];

fn default_toast_position() -> String {
    "bottom-right".into()
}
const TAB_CYCLE_ORDERS: &[&str] = &["strip", "recent"];

fn default_tab_cycle_order() -> String {
    "strip".into()
}

const BACKDROPS: &[&str] = &["none", "mica", "acrylic"];

fn default_backdrop() -> String {
    "none".into()
}

fn default_ui_font() -> String {
    "system".into()
}

fn default_true() -> bool {
    true
}

fn default_translate_language() -> String {
    "en".into()
}

fn default_translate_url() -> String {
    crate::tools::DEFAULT_TRANSLATE_URL.into()
}

fn default_shopping_region() -> String {
    "us-en".into()
}

fn default_hotkey() -> String {
    crate::system::DEFAULT_HOTKEY.into()
}

fn default_vertical_tabs_width() -> u32 {
    240
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct TabSleepSettings {
    pub enabled: bool,
    /// Minutes a tab stays off screen before it sleeps.
    pub minutes: u32,
    /// Older files may set this. Pinned tabs never sleep, so load maps it to false.
    pub sleep_pinned: bool,
    /// Hosts that never sleep, lowercase and without a leading `www.`.
    pub never_sleep_sites: Vec<String>,
}

impl Default for TabSleepSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            minutes: 30,
            sleep_pinned: false,
            never_sleep_sites: Vec::new(),
        }
    }
}

impl TabSleepSettings {
    fn normalize(mut self) -> Self {
        self.minutes = self.minutes.clamp(5, 720);
        self.sleep_pinned = false;
        let mut sites: Vec<String> = Vec::new();
        for raw in &self.never_sleep_sites {
            let host = sleep_site_key(raw);
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
        self.never_sleep_sites = sites;
        self
    }

    pub fn never_sleeps(&self, url: &str) -> bool {
        let host = sleep_site_key(url);
        !host.is_empty()
            && self
                .never_sleep_sites
                .iter()
                .any(|site| host == *site || host.ends_with(&format!(".{site}")))
    }
}

/// Host of a URL or a bare host, as the never-sleep list stores it. Empty for pages without a host.
pub fn sleep_site_key(raw: &str) -> String {
    let raw = raw.trim();
    let host = if raw.contains("://") {
        match url::Url::parse(raw) {
            Ok(url) if matches!(url.scheme(), "http" | "https") => {
                url.host_str().unwrap_or_default().to_string()
            }
            _ => String::new(),
        }
    } else {
        raw.split('/').next().unwrap_or_default().to_string()
    };
    crate::adblock::site_key(&host)
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
const PAPER_SIZES: &[&str] = &[
    "letter",
    "legal",
    "tabloid",
    "statement",
    "executive",
    "folio",
    "a3",
    "a4",
    "a5",
    "a6",
    "b5",
    "photo4x6",
    "photo5x7",
    "photo8x10",
    "iphone16",
    "iphone16Plus",
    "iphone16Pro",
    "iphone16ProMax",
    "iphoneSe",
    "pixel9",
    "pixel9ProXl",
    "galaxyS24",
    "galaxyS24Ultra",
    "ipadMini",
    "ipad",
    "ipadAir13",
    "ipadPro11",
    "ipadPro13",
    "galaxyTabS9",
    "custom",
];
const DEFAULT_PAPER_CHOICES: &[&str] = &["letter", "tabloid", "legal"];
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
    /// Sizes listed in the print editor. Empty means Letter, Tabloid, and Legal.
    pub paper_choices: Vec<String>,
    /// Custom page width in inches. `0` means it is not set.
    pub custom_width: f64,
    /// Custom page height in inches. `0` means it is not set.
    pub custom_height: f64,
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
            paper_choices: DEFAULT_PAPER_CHOICES
                .iter()
                .map(|id| (*id).to_string())
                .collect(),
            custom_width: 0.0,
            custom_height: 0.0,
            landscape: false,
            margins: "normal".into(),
            header_footer: false,
            print_backgrounds: true,
            open_after_save: false,
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
        self.paper_choices = paper_choices(self.paper_choices);
        self.custom_width = paper_inches(self.custom_width);
        self.custom_height = paper_inches(self.custom_height);
        self
    }
}

fn paper_choices(choices: Vec<String>) -> Vec<String> {
    let mut kept = Vec::new();
    for id in choices {
        if PAPER_SIZES.contains(&id.as_str()) && !kept.iter().any(|item: &String| item == &id) {
            kept.push(id);
        }
    }
    if kept.is_empty() {
        DEFAULT_PAPER_CHOICES
            .iter()
            .map(|id| (*id).to_string())
            .collect()
    } else {
        kept
    }
}

/// `0` is unset. Anything outside 1 to 50 inches is unset too.
fn paper_inches(value: f64) -> f64 {
    if value.is_finite() && (1.0..=50.0).contains(&value) {
        value
    } else {
        0.0
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct PageMarkdownSettings {
    /// `article` keeps the main content. `page` keeps the whole document.
    pub scope: String,
    pub images: bool,
    pub links: bool,
    /// Title and source URL above the content.
    pub header: bool,
}

impl Default for PageMarkdownSettings {
    fn default() -> Self {
        Self {
            scope: "article".into(),
            images: true,
            links: true,
            header: true,
        }
    }
}

impl PageMarkdownSettings {
    fn normalize(mut self) -> Self {
        if self.scope != "article" && self.scope != "page" {
            self.scope = "article".into();
        }
        self
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
            accelerate_downloads: true,
            download_name_template: String::new(),
            show_bookmarks_bar: true,
            bookmarks_bar_folder_icons: true,
            bookmarks_bar_text_size: default_bar_text_size(),
            bookmarks_bar_padding: default_bar_padding(),
            bookmark_folder_text_size: default_folder_text_size(),
            bookmark_folder_padding: default_folder_padding(),
            bookmarks_bar_menu_text_size: default_menu_text_size(),
            bookmarks_bar_menu_padding: default_menu_padding(),
            bookmarks_bar_icon_size: default_bookmarks_bar_icon_size(),
            bookmarks_bar_height: default_bookmarks_bar_height(),
            bookmark_folder_icon_size: default_bookmark_folder_icon_size(),
            bookmarks_bar_menu_icon_size: default_bookmarks_bar_menu_icon_size(),
            bookmarks_bar_menu_width: default_bookmarks_bar_menu_width(),
            bookmark_editor_text_size: default_bookmark_editor_text_size(),
            bookmark_editor_padding: default_bookmark_editor_padding(),
            bookmark_editor_row_padding: default_bookmark_editor_row_padding(),
            bookmark_editor_icon_size: default_bookmark_editor_icon_size(),
            bookmark_editor_list_height: default_bookmark_editor_list_height(),
            bookmark_editor_width: default_bookmark_editor_width(),
            legacy_bookmark_size: None,
            show_status_bar: true,
            status_stats: true,
            theme: "zinc".into(),
            color_mode: "dark".into(),
            ui_font: default_ui_font(),
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
            page_markdown: PageMarkdownSettings::default(),
            hidden_toolbar_extensions: Vec::new(),
            toolbar_order: Vec::new(),
            vertical_toolbar: true,
            vertical_toolbar_extensions: false,
            vertical_toolbar_labels: false,
            site_accent: true,
            pin_tab_border: true,
            translate_language: default_translate_language(),
            translate_url: default_translate_url(),
            ai_provider: "local".into(),
            ai_providers: AiProviders::default(),
            legacy_assistant_endpoint: None,
            legacy_assistant_model: None,
            legacy_assistant_key: None,
            shopping_region: default_shopping_region(),
            shopping_model: String::new(),
            shopping_competitors: Vec::new(),
            summon_hotkey: default_hotkey(),
            battery_saver: true,
            adblock: AdblockSettings::default(),
            workspaces_enabled: true,
            tab_layout: "horizontal".into(),
            vertical_tabs_collapsed: false,
            vertical_tabs_width: default_vertical_tabs_width(),
            mute_background_tabs: false,
            peek_on_shift_click: true,
            tab_sleep: TabSleepSettings::default(),
            strip_tracking: true,
            fingerprint_shield: false,
            https_only: false,
            https_exceptions: Vec::new(),
            index_page_text: true,
            tab_cycle_order: default_tab_cycle_order(),
            tab_switcher: true,
            tab_hover_preview: true,
            tab_width: default_tab_width(),
            tab_item_text_size: default_tab_item_text_size(),
            tab_item_padding: default_tab_item_padding(),
            tab_menu_text_size: default_tab_menu_text_size(),
            tab_menu_padding: default_tab_menu_padding(),
            tab_item_height: default_tab_item_height(),
            tab_icon_size: default_tab_icon_size(),
            tab_menu_icon_size: default_tab_menu_icon_size(),
            tab_menu_height: 0.0,
            tab_menu_width: default_tab_menu_width(),
            menu_text_size: default_menu_text_size(),
            menu_padding: default_menu_padding(),
            menu_height: default_menu_height(),
            menu_icon_size: default_menu_icon_size(),
            toolbar_height: default_toolbar_height(),
            toolbar_button_size: default_toolbar_button_size(),
            toolbar_icon_size: default_toolbar_icon_size(),
            address_height: default_address_height(),
            address_text_size: default_address_text_size(),
            omnibox_row_height: default_omnibox_row_height(),
            omnibox_text_size: default_menu_text_size(),
            omnibox_icon_size: default_menu_icon_size(),
            omnibox_url_size: default_omnibox_url_size(),
            downloads_panel_width: default_downloads_panel_width(),
            downloads_text_size: default_menu_text_size(),
            downloads_padding: default_panel_padding(),
            downloads_row_padding: default_panel_row_padding(),
            downloads_icon_size: default_downloads_icon_size(),
            history_panel_width: default_history_panel_width(),
            history_text_size: default_menu_text_size(),
            history_meta_size: default_omnibox_url_size(),
            history_padding: default_panel_padding(),
            history_row_padding: default_panel_row_padding(),
            history_icon_size: default_menu_icon_size(),
            command_palette_width: default_command_palette_width(),
            command_palette_input_height: default_command_palette_input_height(),
            command_palette_row_height: default_command_palette_row_height(),
            command_palette_text_size: default_menu_text_size(),
            command_palette_hint_size: default_omnibox_url_size(),
            window_backdrop: default_backdrop(),
            auto_update: true,
            last_seen_version: String::new(),
            toast_position: default_toast_position(),
            file_handlers: default_file_handlers(),
            agent_panel_width: default_agent_panel_width(),
            automation: AutomationSettings::default(),
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
        if !UI_FONTS.contains(&self.ui_font.as_str()) {
            self.ui_font = defaults.ui_font;
        }
        if let Some(preset) = self.legacy_bookmark_size.take() {
            let sizes = match preset.as_str() {
                "small" => Some((0.6875, 0.375, 0.8125, 0.625)),
                "large" => Some((0.875, 0.625, 0.9375, 0.75)),
                "xlarge" => Some((1.0, 0.75, 1.0, 0.875)),
                _ => None,
            };
            let untouched = self.bookmarks_bar_text_size == defaults.bookmarks_bar_text_size
                && self.bookmarks_bar_padding == defaults.bookmarks_bar_padding
                && self.bookmark_folder_text_size == defaults.bookmark_folder_text_size
                && self.bookmark_folder_padding == defaults.bookmark_folder_padding;
            if let (Some((bar_text, bar_padding, folder_text, folder_padding)), true) =
                (sizes, untouched)
            {
                self.bookmarks_bar_text_size = bar_text;
                self.bookmarks_bar_padding = bar_padding;
                self.bookmark_folder_text_size = folder_text;
                self.bookmark_folder_padding = folder_padding;
            }
        }
        self.bookmarks_bar_text_size = rem_in(
            self.bookmarks_bar_text_size,
            BOOKMARK_TEXT_REM,
            defaults.bookmarks_bar_text_size,
        );
        self.bookmarks_bar_padding = rem_in(
            self.bookmarks_bar_padding,
            BOOKMARK_PADDING_REM,
            defaults.bookmarks_bar_padding,
        );
        self.bookmark_folder_text_size = rem_in(
            self.bookmark_folder_text_size,
            BOOKMARK_TEXT_REM,
            defaults.bookmark_folder_text_size,
        );
        self.bookmark_folder_padding = rem_in(
            self.bookmark_folder_padding,
            BOOKMARK_PADDING_REM,
            defaults.bookmark_folder_padding,
        );
        self.bookmarks_bar_menu_text_size = rem_in(
            self.bookmarks_bar_menu_text_size,
            BOOKMARK_TEXT_REM,
            defaults.bookmarks_bar_menu_text_size,
        );
        self.bookmarks_bar_menu_padding = rem_in(
            self.bookmarks_bar_menu_padding,
            BOOKMARK_PADDING_REM,
            defaults.bookmarks_bar_menu_padding,
        );
        self.tab_width = self.tab_width.clamp(TAB_WIDTH_PX.0, TAB_WIDTH_PX.1);
        self.tab_item_text_size = rem_in(
            self.tab_item_text_size,
            BOOKMARK_TEXT_REM,
            defaults.tab_item_text_size,
        );
        self.tab_item_padding = rem_in(
            self.tab_item_padding,
            BOOKMARK_PADDING_REM,
            defaults.tab_item_padding,
        );
        self.tab_menu_text_size = rem_in(
            self.tab_menu_text_size,
            BOOKMARK_TEXT_REM,
            defaults.tab_menu_text_size,
        );
        self.tab_menu_padding = rem_in(
            self.tab_menu_padding,
            BOOKMARK_PADDING_REM,
            defaults.tab_menu_padding,
        );
        self.tab_item_height = rem_in(self.tab_item_height, HEIGHT_REM, defaults.tab_item_height);
        self.tab_icon_size = rem_in(self.tab_icon_size, ICON_REM, defaults.tab_icon_size);
        self.tab_menu_icon_size = rem_in(
            self.tab_menu_icon_size,
            ICON_REM,
            defaults.tab_menu_icon_size,
        );
        self.tab_menu_height = rem_in(self.tab_menu_height, OPTIONAL_HEIGHT_REM, 0.0);
        self.tab_menu_width = self.tab_menu_width.clamp(MENU_WIDTH_PX.0, MENU_WIDTH_PX.1);
        self.bookmarks_bar_icon_size = rem_in(
            self.bookmarks_bar_icon_size,
            ICON_REM,
            defaults.bookmarks_bar_icon_size,
        );
        self.bookmarks_bar_height = rem_in(
            self.bookmarks_bar_height,
            HEIGHT_REM,
            defaults.bookmarks_bar_height,
        );
        self.bookmark_folder_icon_size = rem_in(
            self.bookmark_folder_icon_size,
            ICON_REM,
            defaults.bookmark_folder_icon_size,
        );
        self.bookmarks_bar_menu_icon_size = rem_in(
            self.bookmarks_bar_menu_icon_size,
            ICON_REM,
            defaults.bookmarks_bar_menu_icon_size,
        );
        self.bookmarks_bar_menu_width = self
            .bookmarks_bar_menu_width
            .clamp(MENU_WIDTH_PX.0, MENU_WIDTH_PX.1);
        self.bookmark_editor_text_size = rem_in(
            self.bookmark_editor_text_size,
            BOOKMARK_TEXT_REM,
            defaults.bookmark_editor_text_size,
        );
        self.bookmark_editor_padding = rem_in(
            self.bookmark_editor_padding,
            BOOKMARK_PADDING_REM,
            defaults.bookmark_editor_padding,
        );
        self.bookmark_editor_row_padding = rem_in(
            self.bookmark_editor_row_padding,
            BOOKMARK_PADDING_REM,
            defaults.bookmark_editor_row_padding,
        );
        self.bookmark_editor_icon_size = rem_in(
            self.bookmark_editor_icon_size,
            ICON_REM,
            defaults.bookmark_editor_icon_size,
        );
        self.bookmark_editor_list_height = rem_in(
            self.bookmark_editor_list_height,
            LIST_HEIGHT_REM,
            defaults.bookmark_editor_list_height,
        );
        self.bookmark_editor_width = self
            .bookmark_editor_width
            .clamp(PANEL_WIDTH_PX.0, PANEL_WIDTH_PX.1);
        self.menu_text_size = rem_in(
            self.menu_text_size,
            BOOKMARK_TEXT_REM,
            defaults.menu_text_size,
        );
        self.menu_padding = rem_in(
            self.menu_padding,
            BOOKMARK_PADDING_REM,
            defaults.menu_padding,
        );
        self.menu_height = rem_in(self.menu_height, HEIGHT_REM, defaults.menu_height);
        self.menu_icon_size = rem_in(self.menu_icon_size, ICON_REM, defaults.menu_icon_size);
        self.toolbar_height = rem_in(self.toolbar_height, HEIGHT_REM, defaults.toolbar_height);
        self.toolbar_button_size = rem_in(
            self.toolbar_button_size,
            HEIGHT_REM,
            defaults.toolbar_button_size,
        );
        self.toolbar_icon_size =
            rem_in(self.toolbar_icon_size, ICON_REM, defaults.toolbar_icon_size);
        self.address_height = rem_in(self.address_height, HEIGHT_REM, defaults.address_height);
        self.address_text_size = rem_in(
            self.address_text_size,
            BOOKMARK_TEXT_REM,
            defaults.address_text_size,
        );
        self.omnibox_row_height = rem_in(
            self.omnibox_row_height,
            HEIGHT_REM,
            defaults.omnibox_row_height,
        );
        self.omnibox_text_size = rem_in(
            self.omnibox_text_size,
            BOOKMARK_TEXT_REM,
            defaults.omnibox_text_size,
        );
        self.omnibox_icon_size =
            rem_in(self.omnibox_icon_size, ICON_REM, defaults.omnibox_icon_size);
        self.omnibox_url_size = rem_in(
            self.omnibox_url_size,
            BOOKMARK_TEXT_REM,
            defaults.omnibox_url_size,
        );
        self.downloads_panel_width = self
            .downloads_panel_width
            .clamp(PANEL_WIDTH_PX.0, PANEL_WIDTH_PX.1);
        self.downloads_text_size = rem_in(
            self.downloads_text_size,
            BOOKMARK_TEXT_REM,
            defaults.downloads_text_size,
        );
        self.downloads_padding = rem_in(
            self.downloads_padding,
            BOOKMARK_PADDING_REM,
            defaults.downloads_padding,
        );
        self.downloads_row_padding = rem_in(
            self.downloads_row_padding,
            BOOKMARK_PADDING_REM,
            defaults.downloads_row_padding,
        );
        self.downloads_icon_size = rem_in(
            self.downloads_icon_size,
            ICON_REM,
            defaults.downloads_icon_size,
        );
        self.history_panel_width = self
            .history_panel_width
            .clamp(PANEL_WIDTH_PX.0, PANEL_WIDTH_PX.1);
        self.history_text_size = rem_in(
            self.history_text_size,
            BOOKMARK_TEXT_REM,
            defaults.history_text_size,
        );
        self.history_meta_size = rem_in(
            self.history_meta_size,
            BOOKMARK_TEXT_REM,
            defaults.history_meta_size,
        );
        self.history_padding = rem_in(
            self.history_padding,
            BOOKMARK_PADDING_REM,
            defaults.history_padding,
        );
        self.history_row_padding = rem_in(
            self.history_row_padding,
            BOOKMARK_PADDING_REM,
            defaults.history_row_padding,
        );
        self.history_icon_size =
            rem_in(self.history_icon_size, ICON_REM, defaults.history_icon_size);
        self.command_palette_width = self
            .command_palette_width
            .clamp(PALETTE_WIDTH_PX.0, PALETTE_WIDTH_PX.1);
        self.command_palette_input_height = rem_in(
            self.command_palette_input_height,
            HEIGHT_REM,
            defaults.command_palette_input_height,
        );
        self.command_palette_row_height = rem_in(
            self.command_palette_row_height,
            HEIGHT_REM,
            defaults.command_palette_row_height,
        );
        self.command_palette_text_size = rem_in(
            self.command_palette_text_size,
            BOOKMARK_TEXT_REM,
            defaults.command_palette_text_size,
        );
        self.command_palette_hint_size = rem_in(
            self.command_palette_hint_size,
            BOOKMARK_TEXT_REM,
            defaults.command_palette_hint_size,
        );
        if self.theme.trim().is_empty() {
            self.theme = defaults.theme;
        }
        if self.home_page.trim().is_empty() || self.home_page == paths::NEW_TAB {
            self.home_page = defaults.home_page;
        }
        self.ui_zoom = ((self.ui_zoom.clamp(80, 150) + 2) / 5) * 5;
        self.page_zoom = snap_page_zoom(self.page_zoom);
        self.print_editor = self.print_editor.normalize();
        self.page_markdown = self.page_markdown.normalize();
        self.hidden_toolbar_extensions =
            toolbar_hidden(&map_retired_tools(&self.hidden_toolbar_extensions));
        self.toolbar_order = toolbar_hidden(&map_retired_tools(&self.toolbar_order));
        self.translate_language = clean_language(&self.translate_language);
        if self.translate_url.trim().is_empty() {
            self.translate_url = default_translate_url();
        }
        self.migrate_ai_provider();
        self.shopping_region = self
            .shopping_region
            .trim()
            .chars()
            .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
            .take(12)
            .collect::<String>()
            .to_ascii_lowercase();
        if self.shopping_region.is_empty() {
            self.shopping_region = default_shopping_region();
        }
        self.shopping_model = self.shopping_model.trim().chars().take(120).collect();
        self.shopping_competitors = clean_hosts(&self.shopping_competitors);
        if self.shopping_competitors.len() > 12 {
            self.shopping_competitors.truncate(12);
        }
        if hotkey_blank(&self.summon_hotkey) {
            self.summon_hotkey = default_hotkey();
        }
        self.adblock = self.adblock.normalize();
        self.agent_panel_width = self
            .agent_panel_width
            .clamp(AGENT_PANEL_WIDTH_PX.0, AGENT_PANEL_WIDTH_PX.1);
        self.automation = self.automation.normalize();
        if !TAB_LAYOUTS.contains(&self.tab_layout.as_str()) {
            self.tab_layout = defaults.tab_layout.clone();
        }
        if !TAB_CYCLE_ORDERS.contains(&self.tab_cycle_order.as_str()) {
            self.tab_cycle_order = defaults.tab_cycle_order.clone();
        }
        if !BACKDROPS.contains(&self.window_backdrop.as_str()) {
            self.window_backdrop = defaults.window_backdrop.clone();
        }
        if !TOAST_POSITIONS.contains(&self.toast_position.as_str()) {
            self.toast_position = defaults.toast_position.clone();
        }
        self.last_seen_version = self.last_seen_version.trim().chars().take(40).collect();
        self.vertical_tabs_width = self.vertical_tabs_width.clamp(160, 400);
        self.tab_sleep = self.tab_sleep.normalize();
        self.https_exceptions = clean_hosts(&self.https_exceptions);
        self.download_name_template = self
            .download_name_template
            .trim()
            .chars()
            .take(180)
            .collect();
        self.file_handlers = normalize_file_handlers(std::mem::take(&mut self.file_handlers));
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

    /// An older file stored one endpoint. A localhost address becomes Local; anything else becomes Custom.
    fn migrate_ai_provider(&mut self) {
        if self.ai_provider.trim().is_empty() {
            let endpoint = self.legacy_assistant_endpoint.clone().unwrap_or_default();
            let model = self.legacy_assistant_model.clone().unwrap_or_default();
            let endpoint = endpoint.trim().to_string();
            let model = model.trim().to_string();
            if endpoint.is_empty() && model.is_empty() {
                self.ai_provider = "local".into();
            } else if endpoint.is_empty() || crate::ai::localhost_endpoint(&endpoint) {
                self.ai_provider = "local".into();
                if !endpoint.is_empty() {
                    self.ai_providers.local.endpoint = endpoint;
                }
                if !model.is_empty() {
                    self.ai_providers.local.model = model;
                }
            } else {
                self.ai_provider = "custom".into();
                self.ai_providers.custom.endpoint = endpoint;
                if !model.is_empty() {
                    self.ai_providers.custom.model = model;
                }
            }
        }
        if !AI_PROVIDER_IDS.contains(&self.ai_provider.as_str()) {
            self.ai_provider = "local".into();
        }
        if self.ai_providers.openai.endpoint.trim().is_empty() {
            self.ai_providers.openai.endpoint = crate::ai::OPENAI_ENDPOINT.into();
        }
        if self.ai_providers.openai.model.trim().is_empty() {
            self.ai_providers.openai.model = "gpt-4o-mini".into();
        }
        if self.ai_providers.claude.endpoint.trim().is_empty() {
            self.ai_providers.claude.endpoint = crate::ai::CLAUDE_ENDPOINT.into();
        }
        if self.ai_providers.claude.model.trim().is_empty() {
            self.ai_providers.claude.model = "claude-3-5-haiku-latest".into();
        }
        if self.ai_providers.local.endpoint.trim().is_empty() {
            self.ai_providers.local.endpoint = crate::ai::LOCAL_ENDPOINT.into();
        }
        if self.ai_providers.local.model.trim().is_empty() {
            self.ai_providers.local.model = "llama3.2".into();
        }
        self.ai_providers.openai.endpoint = self.ai_providers.openai.endpoint.trim().to_string();
        self.ai_providers.openai.model = self.ai_providers.openai.model.trim().to_string();
        self.ai_providers.openai.thinking = clean_thinking(&self.ai_providers.openai.thinking);
        self.ai_providers.claude.endpoint = self.ai_providers.claude.endpoint.trim().to_string();
        self.ai_providers.claude.model = self.ai_providers.claude.model.trim().to_string();
        self.ai_providers.claude.thinking = clean_thinking(&self.ai_providers.claude.thinking);
        self.ai_providers.custom.endpoint = self.ai_providers.custom.endpoint.trim().to_string();
        self.ai_providers.custom.model = self.ai_providers.custom.model.trim().to_string();
        self.ai_providers.custom.thinking = clean_thinking(&self.ai_providers.custom.thinking);
        self.ai_providers.local.endpoint = self.ai_providers.local.endpoint.trim().to_string();
        self.ai_providers.local.model = self.ai_providers.local.model.trim().to_string();
        self.ai_providers.local.thinking = clean_thinking(&self.ai_providers.local.thinking);
        self.legacy_assistant_endpoint = None;
        self.legacy_assistant_model = None;
    }

    pub fn page_zoom_factor(&self) -> f64 {
        f64::from(self.page_zoom) / 100.0
    }

    /// Browser UI size as a WebView2 zoom factor. Extension popups and pages use this.
    pub fn ui_zoom_factor(&self) -> f64 {
        f64::from(self.ui_zoom) / 100.0
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

    /// Name of the search engine for labels, such as `Google`. A custom URL is `the web`.
    pub fn search_engine_name(&self) -> &'static str {
        match self.search_engine.as_str() {
            "google" => "Google",
            "bing" => "Bing",
            "duckduckgo" => "DuckDuckGo",
            "brave" => "Brave",
            "startpage" => "Startpage",
            "kagi" => "Kagi",
            "mojeek" => "Mojeek",
            _ => "the web",
        }
    }

    /// The handler for `extension`, compared without a leading dot.
    pub fn handler_for(&self, extension: &str) -> Option<&FileHandler> {
        let extension = clean_extension(extension)?;
        self.file_handlers
            .iter()
            .find(|handler| handler.extension == extension)
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
    let path = paths::data_file(app, FILE);
    let mut loaded: BrowserSettings = crate::sync_conflicts::read_json(app, &path);
    let legacy_key = loaded
        .legacy_assistant_key
        .take()
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty());
    loaded = loaded.normalize();
    if let Some(key) = legacy_key {
        let provider = loaded.ai_provider.clone();
        if crate::secrets::store_key(app, &provider, &key).is_ok() {
            let _ = crate::files::write_json(&path, &loaded);
        }
    }
    loaded.pinned_tabs = canonicalize_pins(app, &loaded);
    apply_shared_sync_folder(&mut loaded);
    crate::adblock::configure(&loaded.adblock);
    app.manage(SettingsState(Mutex::new(loaded)));
    crate::sync_conflicts::note(app, &paths::data_file(app, FILE));
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

/// The Windows app theme. Browser light and dark mode do not change this.
pub fn windows_uses_light(app: &AppHandle) -> bool {
    let theme_of = |label: &str| app.get_window(label).and_then(|window| window.theme().ok());
    if let Some(theme) = theme_of(crate::tabs::WINDOW) {
        return theme == tauri::Theme::Light;
    }
    if app.try_state::<crate::tabs::Tabs>().is_none() {
        return false;
    }
    crate::tabs::browser_labels(app)
        .iter()
        .any(|label| theme_of(label) == Some(tauri::Theme::Light))
}

/// Fill behind a remote page. Matches Windows so a dark site is not framed in white.
pub fn site_canvas(app: &AppHandle) -> Color {
    if windows_uses_light(app) {
        Color(0xff, 0xff, 0xff, 255)
    } else {
        Color(0x20, 0x21, 0x24, 255)
    }
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
            ("slate", 0xff, 0xff, 0xff),
            ("stone", 0xff, 0xff, 0xff),
            ("paper", 0xf4, 0xf4, 0xf0),
            ("graphite", 0xff, 0xff, 0xff),
            ("ink", 0xf4, 0xf4, 0xf4),
            ("fog", 0xf5, 0xf6, 0xf8),
            ("plain", 0xff, 0xff, 0xff),
            ("line", 0xff, 0xff, 0xff),
            ("soft", 0xf7, 0xf4, 0xef),
            ("lift", 0xf6, 0xf6, 0xf6),
            ("sharp", 0xff, 0xff, 0xff),
            ("contrast", 0xff, 0xff, 0xff),
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
            ("slate", 0x1e, 0x29, 0x3b),
            ("stone", 0x29, 0x25, 0x24),
            ("paper", 0x2a, 0x2a, 0x27),
            ("graphite", 0x38, 0x38, 0x38),
            ("ink", 0x17, 0x17, 0x17),
            ("fog", 0x36, 0x3c, 0x45),
            ("plain", 0x1f, 0x1f, 0x1f),
            ("line", 0x1a, 0x23, 0x30),
            ("soft", 0x24, 0x22, 0x20),
            ("lift", 0x23, 0x23, 0x23),
            ("sharp", 0x18, 0x18, 0x18),
            ("contrast", 0x0a, 0x0a, 0x0a),
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
            ("slate", 0xf8, 0xfa, 0xfc),
            ("stone", 0xfa, 0xfa, 0xf9),
            ("paper", 0xff, 0xff, 0xff),
            ("graphite", 0xf3, 0xf3, 0xf3),
            ("ink", 0xff, 0xff, 0xff),
            ("fog", 0xe8, 0xea, 0xed),
            ("plain", 0xf7, 0xf7, 0xf7),
            ("line", 0xf8, 0xfa, 0xfc),
            ("soft", 0xfa, 0xf8, 0xf5),
            ("lift", 0xf3, 0xf3, 0xf3),
            ("sharp", 0xf7, 0xf7, 0xf7),
            ("contrast", 0xff, 0xff, 0xff),
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
            ("slate", 0x0f, 0x17, 0x2a),
            ("stone", 0x1c, 0x19, 0x17),
            ("paper", 0x1e, 0x1e, 0x1c),
            ("graphite", 0x2b, 0x2b, 0x2b),
            ("ink", 0x0c, 0x0c, 0x0c),
            ("fog", 0x2c, 0x31, 0x38),
            ("plain", 0x14, 0x14, 0x14),
            ("line", 0x10, 0x14, 0x1a),
            ("soft", 0x1a, 0x19, 0x17),
            ("lift", 0x16, 0x16, 0x16),
            ("sharp", 0x10, 0x10, 0x10),
            ("contrast", 0x00, 0x00, 0x00),
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

/// Copies present style keys from `source` onto `current`, then clamps them.
/// A key the source omits stays as it is. Theme and every other key are ignored.
pub fn overlay_style_metrics(
    current: serde_json::Value,
    source: &serde_json::Map<String, serde_json::Value>,
) -> Result<BrowserSettings, String> {
    let mut current = current;
    let Some(object) = current.as_object_mut() else {
        return Err("Settings could not be read.".into());
    };
    for key in STYLE_METRIC_KEYS {
        if let Some(value) = source.get(*key) {
            object.insert((*key).to_string(), value.clone());
        }
    }
    serde_json::from_value(current)
        .map(BrowserSettings::normalize)
        .map_err(|_| "Settings could not be read.".to_string())
}

/// Copies fine-tune sizes from another profile into this one.
pub fn import_style_metrics(app: &AppHandle, profile_id: &str) -> Result<BrowserSettings, String> {
    let id = profile_id.trim();
    if id.is_empty() {
        return Err("Choose a profile.".into());
    }
    if id == crate::profiles::active_id() {
        return Err("Choose a different profile.".into());
    }
    if !crate::profiles::list(app)
        .iter()
        .any(|profile| profile.id == id)
    {
        return Err("That profile no longer exists.".into());
    }
    let path = paths::profile_data_dir(app, id).join(FILE);
    let source = read_style_source(&path)?;
    let current = serde_json::to_value(get(app)).map_err(|error| error.to_string())?;
    let merged = overlay_style_metrics(current, &source)?;
    save(app, merged)
}

fn read_style_source(
    path: &std::path::Path,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    if !path.is_file() {
        return Ok(serde_json::Map::new());
    }
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|_| "That profile's settings could not be read.".to_string())?;
    value
        .as_object()
        .cloned()
        .ok_or_else(|| "That profile's settings could not be read.".into())
}

pub fn reload(app: &AppHandle) {
    let path = paths::data_file(app, FILE);
    let mut loaded: BrowserSettings = crate::sync_conflicts::read_json(app, &path);
    let legacy_key = loaded
        .legacy_assistant_key
        .take()
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty());
    loaded = loaded.normalize();
    if let Some(key) = legacy_key {
        let provider = loaded.ai_provider.clone();
        if crate::secrets::store_key(app, &provider, &key).is_ok() {
            let _ = crate::files::write_json(&path, &loaded);
        }
    }
    loaded.pinned_tabs = canonicalize_pins(app, &loaded);
    apply_shared_sync_folder(&mut loaded);
    crate::adblock::configure(&loaded.adblock);
    let (previous_page_zoom, previous_ui_zoom) = {
        let state = app.state::<SettingsState>();
        let mut guard = state.0.lock().expect("settings lock");
        let previous_page_zoom = guard.page_zoom;
        let previous_ui_zoom = guard.ui_zoom;
        *guard = loaded.clone();
        (previous_page_zoom, previous_ui_zoom)
    };
    if loaded.page_zoom != previous_page_zoom {
        crate::tabs::apply_default_page_zoom(app, previous_page_zoom, loaded.page_zoom);
    }
    if loaded.ui_zoom != previous_ui_zoom {
        crate::tabs::apply_ui_zoom(app, previous_ui_zoom, loaded.ui_zoom);
    }
    let _ = app.emit("settings-changed", &loaded);
    crate::tabs::apply_page_background(app);
    crate::tabs::apply_audio_all(app);
    crate::tabs::place_all_toasts(app);
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
    let mut settings = settings;
    let legacy_key = settings
        .legacy_assistant_key
        .take()
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty());
    let mut settings = settings.normalize();
    if let Some(key) = legacy_key {
        let _ = crate::secrets::store_key(app, &settings.ai_provider, &key);
    }
    settings.pinned_tabs = canonicalize_pins(app, &settings);
    if crate::profiles::active_id() != crate::profiles::DEFAULT_ID {
        apply_shared_sync_folder(&mut settings);
    }
    let (saved, previous_page_zoom, previous_ui_zoom) = {
        let state = app.state::<SettingsState>();
        let mut guard = state.0.lock().expect("settings lock");
        if keep_sync {
            if crate::profiles::active_id() == crate::profiles::DEFAULT_ID {
                settings.sync_folder.clone_from(&guard.sync_folder);
            }
            settings.sync_revision = guard.sync_revision;
            settings.sync_bookmarks_revision = guard.sync_bookmarks_revision;
            settings.sync_extensions_revision = guard.sync_extensions_revision;
            settings.sync_settings_revision = guard.sync_settings_revision;
            settings.sync_history_revision = guard.sync_history_revision;
        }
        let previous_page_zoom = guard.page_zoom;
        let previous_ui_zoom = guard.ui_zoom;
        crate::sync_conflicts::store(app, &paths::data_file(app, FILE), &settings)?;
        crate::adblock::configure(&settings.adblock);
        *guard = settings.clone();
        (settings, previous_page_zoom, previous_ui_zoom)
    };
    if saved.page_zoom != previous_page_zoom {
        crate::tabs::apply_default_page_zoom(app, previous_page_zoom, saved.page_zoom);
    }
    if saved.ui_zoom != previous_ui_zoom {
        crate::tabs::apply_ui_zoom(app, previous_ui_zoom, saved.ui_zoom);
    }
    let _ = app.emit("settings-changed", &saved);
    crate::tabs::apply_page_background(app);
    crate::tabs::apply_audio_all(app);
    crate::tabs::place_all_toasts(app);
    Ok(saved)
}

/// The sync folder belongs to the Default profile. A storage root already chosen on this
/// computer replaces whatever this file stored. Another profile with no shared folder
/// drops a path of its own so it cannot point somewhere else.
fn apply_shared_sync_folder(loaded: &mut BrowserSettings) {
    if let Some(root) = paths::storage_root() {
        loaded.sync_folder = root.to_string_lossy().into_owned();
        return;
    }
    if crate::profiles::active_id() != crate::profiles::DEFAULT_ID {
        loaded.sync_folder.clear();
    }
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

fn default_bar_text_size() -> f32 {
    0.75
}

fn default_bar_padding() -> f32 {
    0.5
}

fn default_folder_text_size() -> f32 {
    0.875
}

fn default_folder_padding() -> f32 {
    0.75
}

fn default_menu_text_size() -> f32 {
    0.875
}

fn default_menu_padding() -> f32 {
    0.625
}

fn default_tab_width() -> u32 {
    220
}

fn default_tab_item_text_size() -> f32 {
    0.6875
}

fn default_tab_item_padding() -> f32 {
    0.625
}

fn default_tab_menu_text_size() -> f32 {
    0.875
}

fn default_tab_menu_padding() -> f32 {
    0.625
}

fn default_tab_item_height() -> f32 {
    1.875
}

fn default_tab_icon_size() -> f32 {
    1.0
}

fn default_tab_menu_icon_size() -> f32 {
    1.0
}

fn default_tab_menu_width() -> u32 {
    260
}

fn default_bookmarks_bar_icon_size() -> f32 {
    0.9
}

fn default_bookmarks_bar_height() -> f32 {
    1.5
}

fn default_bookmark_folder_icon_size() -> f32 {
    1.0
}

fn default_bookmarks_bar_menu_icon_size() -> f32 {
    1.0
}

fn default_bookmarks_bar_menu_width() -> u32 {
    280
}

fn default_bookmark_editor_text_size() -> f32 {
    0.75
}

fn default_bookmark_editor_padding() -> f32 {
    0.5
}

fn default_bookmark_editor_row_padding() -> f32 {
    0.5
}

fn default_bookmark_editor_icon_size() -> f32 {
    1.0
}

fn default_bookmark_editor_list_height() -> f32 {
    24.0
}

fn default_bookmark_editor_width() -> u32 {
    400
}

fn default_menu_height() -> f32 {
    2.25
}

fn default_menu_icon_size() -> f32 {
    1.0
}

fn default_toolbar_height() -> f32 {
    2.5
}

fn default_toolbar_button_size() -> f32 {
    2.0
}

fn default_toolbar_icon_size() -> f32 {
    1.0
}

fn default_address_height() -> f32 {
    2.0
}

fn default_address_text_size() -> f32 {
    0.875
}

fn default_omnibox_row_height() -> f32 {
    2.25
}

fn default_omnibox_url_size() -> f32 {
    0.75
}

fn default_downloads_panel_width() -> u32 {
    380
}

fn default_panel_padding() -> f32 {
    0.75
}

fn default_panel_row_padding() -> f32 {
    0.5
}

fn default_downloads_icon_size() -> f32 {
    2.0
}

fn default_history_panel_width() -> u32 {
    380
}

fn default_agent_panel_width() -> u32 {
    AGENT_PANEL_DEFAULT_WIDTH
}

fn default_command_palette_width() -> u32 {
    560
}

fn default_command_palette_input_height() -> f32 {
    3.0
}

fn default_command_palette_row_height() -> f32 {
    2.5
}

/// Clamps a rem value into `range` and rounds it to 0.0001. Not a number falls back.
fn rem_in(value: f32, range: (f32, f32), fallback: f32) -> f32 {
    if !value.is_finite() {
        return fallback;
    }
    (value.clamp(range.0, range.1) * 10000.0).round() / 10000.0
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
fn clean_hosts(hosts: &[String]) -> Vec<String> {
    let mut kept = Vec::new();
    for raw in hosts {
        let host = sleep_site_key(raw);
        if host.is_empty()
            || host.len() > 253
            || host.chars().any(char::is_whitespace)
            || kept.contains(&host)
        {
            continue;
        }
        kept.push(host);
        if kept.len() >= 1000 {
            break;
        }
    }
    kept
}

/// Saved ids that are still available, in that order, then any available id the list does not mention.
pub fn hotkey_blank(token: &str) -> bool {
    token.trim().is_empty() || crate::system::hotkey_mods(token).is_none()
}

fn clean_thinking(value: &str) -> String {
    match value.trim() {
        "low" | "medium" | "high" => value.trim().to_string(),
        _ => String::new(),
    }
}

fn clean_language(language: &str) -> String {
    let cleaned: String = language
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
        .take(12)
        .collect();
    if cleaned.is_empty() {
        "en".into()
    } else {
        cleaned
    }
}

pub fn toolbar_order(saved: &[String], available: &[String]) -> Vec<String> {
    let mut ordered = Vec::new();
    for id in saved {
        if available.iter().any(|item| item == id) && !ordered.iter().any(|item| item == id) {
            ordered.push(id.clone());
        }
    }
    for id in available {
        if !ordered.iter().any(|item| item == id) {
            ordered.push(id.clone());
        }
    }
    ordered
}

/// The Prices tool is Shopping now. A saved pin maps across, and a second copy is dropped.
fn map_retired_tools(ids: &[String]) -> Vec<String> {
    ids.iter()
        .map(|id| {
            if id == "price" {
                "shopping".to_string()
            } else {
                id.clone()
            }
        })
        .collect()
}

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
        let loaded: super::BrowserSettings = serde_json::from_str(
            r#"{"searchEngine":"bing","printEditor":{"paper":"a9","textSize":400}}"#,
        )
        .expect("settings parse");
        let loaded = loaded.normalize();
        assert_eq!(loaded.search_engine, "bing");
        assert_eq!(loaded.print_editor.paper, "letter");
        assert_eq!(loaded.print_editor.text_size, 150);
        assert!(loaded.print_editor.remove_ads);
        assert!(!loaded.print_editor.open_after_save);
        assert_eq!(
            loaded.print_editor.paper_choices,
            vec![
                "letter".to_string(),
                "tabloid".to_string(),
                "legal".to_string()
            ]
        );
        assert_eq!(loaded.print_editor.custom_width, 0.0);
        assert_eq!(loaded.print_editor.custom_height, 0.0);
        assert!(loaded.page_capture.full_page);
        assert_eq!(loaded.page_markdown.scope, "article");
        assert!(loaded.page_markdown.images);
        assert!(loaded.page_markdown.links);
        assert!(loaded.page_markdown.header);
        assert!(loaded.hidden_toolbar_extensions.is_empty());
    }

    #[test]
    fn a_device_paper_size_is_kept() {
        let loaded: super::BrowserSettings =
            serde_json::from_str(r#"{"printEditor":{"paper":"iphone16"}}"#)
                .expect("settings parse");
        assert_eq!(loaded.normalize().print_editor.paper, "iphone16");
    }

    #[test]
    fn paper_choices_drop_unknown_ids_and_bad_custom_sizes() {
        let loaded: super::BrowserSettings = serde_json::from_str(
            r#"{"printEditor":{"paperChoices":["nope","a4","a4"],"customWidth":80,"customHeight":-3}}"#,
        )
        .expect("settings parse");
        let loaded = loaded.normalize();
        assert_eq!(loaded.print_editor.paper_choices, vec!["a4".to_string()]);
        assert_eq!(loaded.print_editor.custom_width, 0.0);
        assert_eq!(loaded.print_editor.custom_height, 0.0);
    }

    #[test]
    fn an_explicit_open_after_save_is_kept() {
        let loaded: super::BrowserSettings =
            serde_json::from_str(r#"{"printEditor":{"openAfterSave":true}}"#)
                .expect("settings parse");
        assert!(loaded.normalize().print_editor.open_after_save);
    }

    #[test]
    fn a_bad_markdown_scope_falls_back_to_article() {
        let loaded: super::BrowserSettings = serde_json::from_str(
            r#"{"searchEngine":"bing","pageMarkdown":{"scope":"nope","images":false}}"#,
        )
        .expect("settings parse");
        let loaded = loaded.normalize();
        assert_eq!(loaded.page_markdown.scope, "article");
        assert!(!loaded.page_markdown.images);
        assert!(loaded.page_markdown.links);
        assert!(loaded.page_markdown.header);
    }

    #[test]
    fn older_settings_get_ad_blocker_defaults() {
        let loaded: super::BrowserSettings =
            serde_json::from_str(r#"{"searchEngine":"bing"}"#).expect("settings parse");
        let loaded = loaded.normalize();
        assert!(loaded.adblock.enabled);
        assert!(loaded.adblock.allowed_sites.is_empty());
        assert!(loaded.accelerate_downloads);
        assert!(loaded.download_name_template.is_empty());
        assert!(loaded.show_status_bar);
        assert!(loaded.status_stats);
    }

    #[test]
    fn older_settings_get_privacy_defaults() {
        let loaded: super::BrowserSettings =
            serde_json::from_str(r#"{"searchEngine":"bing"}"#).expect("settings parse");
        let loaded = loaded.normalize();
        assert!(loaded.strip_tracking);
        assert!(!loaded.fingerprint_shield);
        assert!(!loaded.https_only);
        assert!(loaded.https_exceptions.is_empty());
        assert!(loaded.index_page_text);
    }

    #[test]
    fn older_settings_get_tab_feature_defaults() {
        let loaded: super::BrowserSettings =
            serde_json::from_str(r#"{"searchEngine":"bing"}"#).expect("settings parse");
        let loaded = loaded.normalize();
        assert_eq!(loaded.tab_layout, "horizontal");
        assert!(loaded.workspaces_enabled);
        assert!(!loaded.vertical_tabs_collapsed);
        assert_eq!(loaded.vertical_tabs_width, 240);
        assert!(!loaded.mute_background_tabs);
        assert!(loaded.peek_on_shift_click);
        assert!(loaded.tab_hover_preview);
        assert_eq!(loaded.tab_width, 220);
        assert_eq!(loaded.tab_item_text_size, 0.6875);
        assert_eq!(loaded.tab_item_padding, 0.625);
        assert_eq!(loaded.tab_menu_text_size, 0.875);
        assert_eq!(loaded.tab_menu_padding, 0.625);
        assert!(loaded.tab_sleep.enabled);
        assert_eq!(loaded.tab_sleep.minutes, 30);
        assert!(!loaded.tab_sleep.sleep_pinned);
    }

    #[test]
    fn an_older_file_that_let_pinned_tabs_sleep_keeps_them_awake() {
        let loaded: super::BrowserSettings = serde_json::from_str(
            r#"{"tabSleep":{"enabled":true,"minutes":30,"sleepPinned":true,"neverSleepSites":[]}}"#,
        )
        .expect("settings parse");
        let loaded = loaded.normalize();
        assert!(!loaded.tab_sleep.sleep_pinned);
        assert!(loaded.tab_sleep.enabled);
    }

    #[test]
    fn older_settings_keep_extensions_on_the_address_bar() {
        let loaded: super::BrowserSettings =
            serde_json::from_str(r#"{"searchEngine":"bing"}"#).expect("settings parse");
        let loaded = loaded.normalize();
        assert!(loaded.vertical_toolbar);
        assert!(!loaded.vertical_toolbar_extensions);
        assert!(!loaded.vertical_toolbar_labels);
    }

    #[test]
    fn bookmark_sizes_default_clamp_and_migrate_the_old_preset() {
        let older: super::BrowserSettings =
            serde_json::from_str(r#"{"searchEngine":"bing"}"#).expect("settings parse");
        let older = older.normalize();
        assert_eq!(older.bookmarks_bar_text_size, 0.75);
        assert_eq!(older.bookmark_folder_padding, 0.75);
        assert_eq!(older.bookmarks_bar_menu_text_size, 0.875);
        assert_eq!(older.bookmarks_bar_menu_padding, 0.625);
        assert!(older.bookmarks_bar_folder_icons);

        let no_icons: super::BrowserSettings =
            serde_json::from_str(r#"{"bookmarksBarFolderIcons":false}"#).expect("settings parse");
        assert!(!no_icons.normalize().bookmarks_bar_folder_icons);

        let preset: super::BrowserSettings =
            serde_json::from_str(r#"{"bookmarkSize":"small"}"#).expect("settings parse");
        let preset = preset.normalize();
        assert_eq!(preset.bookmarks_bar_text_size, 0.6875);
        assert_eq!(preset.bookmark_folder_text_size, 0.8125);
        let saved = serde_json::to_string(&preset).expect("settings save");
        assert!(!saved.contains("bookmarkSize"));
        assert!(saved.contains("\"bookmarksBarTextSize\":0.6875"));

        let custom: super::BrowserSettings = serde_json::from_str(
            r#"{"bookmarkSize":"large","bookmarksBarTextSize":0.6,"bookmarksBarPadding":9,"bookmarkFolderTextSize":0.1}"#,
        )
        .expect("settings parse");
        let custom = custom.normalize();
        assert_eq!(custom.bookmarks_bar_text_size, 0.6);
        assert_eq!(custom.bookmarks_bar_padding, 1.5);
        assert_eq!(custom.bookmark_folder_text_size, 0.5);
    }

    #[test]
    fn tab_settings_out_of_range_fall_back() {
        let loaded: super::BrowserSettings = serde_json::from_str(
            r#"{"tabLayout":"diagonal","verticalTabsWidth":9000,"tabWidth":9000,"tabItemTextSize":9,"tabItemPadding":-1,"tabMenuTextSize":9,"tabMenuPadding":-1,"tabSleep":{"minutes":1,"neverSleepSites":["https://www.Mail.example.com/inbox","mail.example.com",""]}}"#,
        )
        .expect("settings parse");
        let loaded = loaded.normalize();
        assert_eq!(loaded.tab_layout, "horizontal");
        assert_eq!(loaded.vertical_tabs_width, 400);
        assert_eq!(loaded.tab_width, 480);
        assert_eq!(loaded.tab_item_text_size, 1.5);
        assert_eq!(loaded.tab_item_padding, 0.0);
        assert_eq!(loaded.tab_menu_text_size, 1.5);
        assert_eq!(loaded.tab_menu_padding, 0.0);
        assert_eq!(loaded.tab_item_height, 1.875);
        assert_eq!(loaded.tab_menu_height, 0.0);
        assert_eq!(loaded.tab_sleep.minutes, 5);
        assert_eq!(
            loaded.tab_sleep.never_sleep_sites,
            vec!["mail.example.com".to_string()]
        );
        assert!(
            loaded
                .tab_sleep
                .never_sleeps("https://inbox.mail.example.com/x")
        );
        assert!(!loaded.tab_sleep.never_sleeps("https://example.com/"));
    }

    #[test]
    fn older_settings_keep_the_current_chrome_sizes() {
        let loaded: super::BrowserSettings =
            serde_json::from_str(r#"{"searchEngine":"bing"}"#).expect("settings parse");
        let loaded = loaded.normalize();
        assert_eq!(loaded.tab_item_height, 1.875);
        assert_eq!(loaded.tab_icon_size, 1.0);
        assert_eq!(loaded.tab_menu_icon_size, 1.0);
        assert_eq!(loaded.tab_menu_height, 0.0);
        assert_eq!(loaded.tab_menu_width, 260);
        assert_eq!(loaded.bookmarks_bar_icon_size, 0.9);
        assert_eq!(loaded.bookmarks_bar_height, 1.5);
        assert_eq!(loaded.bookmark_folder_icon_size, 1.0);
        assert_eq!(loaded.bookmarks_bar_menu_width, 280);
        assert_eq!(loaded.bookmark_editor_text_size, 0.75);
        assert_eq!(loaded.bookmark_editor_padding, 0.5);
        assert_eq!(loaded.bookmark_editor_row_padding, 0.5);
        assert_eq!(loaded.bookmark_editor_icon_size, 1.0);
        assert_eq!(loaded.bookmark_editor_list_height, 24.0);
        assert_eq!(loaded.bookmark_editor_width, 400);
        assert_eq!(loaded.menu_text_size, 0.875);
        assert_eq!(loaded.menu_padding, 0.625);
        assert_eq!(loaded.menu_height, 2.25);
        assert_eq!(loaded.menu_icon_size, 1.0);
        assert_eq!(loaded.toolbar_height, 2.5);
        assert_eq!(loaded.toolbar_button_size, 2.0);
        assert_eq!(loaded.address_height, 2.0);
        assert_eq!(loaded.omnibox_row_height, 2.25);
        assert_eq!(loaded.downloads_panel_width, 380);
        assert_eq!(loaded.downloads_icon_size, 2.0);
        assert_eq!(loaded.history_panel_width, 380);
        assert_eq!(loaded.agent_panel_width, 380);
        assert_eq!(loaded.command_palette_width, 560);
        assert_eq!(loaded.command_palette_input_height, 3.0);
        assert_eq!(loaded.command_palette_row_height, 2.5);
    }

    #[test]
    fn chrome_sizes_out_of_range_clamp() {
        let loaded: super::BrowserSettings = serde_json::from_str(
            r#"{"tabItemHeight":9,"tabIconSize":0,"tabMenuHeight":-1,"tabMenuWidth":10,"menuHeight":0,"downloadsPanelWidth":9000,"commandPaletteWidth":10,"omniboxUrlSize":4}"#,
        )
        .expect("settings parse");
        let loaded = loaded.normalize();
        assert_eq!(loaded.tab_item_height, 4.0);
        assert_eq!(loaded.tab_icon_size, 0.5);
        assert_eq!(loaded.tab_menu_height, 0.0);
        assert_eq!(loaded.tab_menu_width, 160);
        assert_eq!(loaded.menu_height, 1.0);
        assert_eq!(loaded.downloads_panel_width, 720);
        assert_eq!(loaded.command_palette_width, 360);
        assert_eq!(loaded.omnibox_url_size, 1.5);
    }

    #[test]
    fn agent_panel_width_defaults_and_clamps() {
        let missing: super::BrowserSettings = serde_json::from_str("{}").expect("settings parse");
        assert_eq!(missing.normalize().agent_panel_width, 380);
        let narrow: super::BrowserSettings =
            serde_json::from_str(r#"{"agentPanelWidth":9}"#).expect("settings parse");
        assert_eq!(narrow.normalize().agent_panel_width, 280);
        let wide: super::BrowserSettings =
            serde_json::from_str(r#"{"agentPanelWidth":9000}"#).expect("settings parse");
        assert_eq!(wide.normalize().agent_panel_width, 720);
    }

    #[test]
    fn style_import_copies_a_present_size_and_leaves_the_rest() {
        let mut current = super::BrowserSettings::default();
        current.theme = "nord".into();
        current.tab_icon_size = 1.5;
        let current = serde_json::to_value(current).expect("settings value");
        let source = serde_json::json!({
            "tabItemHeight": 2.5,
            "theme": "dracula",
            "tabItemTextSize": 9
        });
        let source = source.as_object().expect("source object");
        let merged = super::overlay_style_metrics(current, source).expect("overlay");
        assert_eq!(merged.theme, "nord");
        assert_eq!(merged.tab_item_height, 2.5);
        assert_eq!(merged.tab_icon_size, 1.5);
        assert_eq!(merged.tab_item_text_size, 1.5);
        assert_eq!(merged.tab_menu_width, 260);
    }

    #[test]
    fn paused_sites_are_cleaned_up() {
        let loaded: super::BrowserSettings = serde_json::from_str(
            r#"{"adblock":{"enabled":false,"allowedSites":[" WWW.Example.com ","example.com","","a b"]}}"#,
        )
        .expect("settings parse");
        let loaded = loaded.normalize();
        assert!(!loaded.adblock.enabled);
        assert_eq!(
            loaded.adblock.allowed_sites,
            vec!["example.com".to_string()]
        );
    }

    #[test]
    fn an_empty_toolbar_order_keeps_the_default_sequence() {
        let available = ["print-pdf", "page-capture", "ext-a"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        assert_eq!(super::toolbar_order(&[], &available), available);
    }

    #[test]
    fn a_saved_toolbar_order_drops_an_unpinned_id() {
        let available = ["print-pdf".to_string(), "ad-blocker".to_string()];
        let saved = [
            "missing".to_string(),
            "ad-blocker".to_string(),
            "print-pdf".to_string(),
        ];
        assert_eq!(
            super::toolbar_order(&saved, &available),
            vec!["ad-blocker".to_string(), "print-pdf".to_string()]
        );
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
        assert_eq!(
            folder_from_stored(r"D:\share\ce-browser-sync.json"),
            r"D:\share"
        );
        assert_eq!(folder_from_stored(""), "");
    }

    #[test]
    fn an_existing_folder_stays_a_folder() {
        let dir = std::env::temp_dir();
        let stored = folder_from_stored(dir.to_string_lossy().as_ref());
        assert_eq!(stored, dir.to_string_lossy());
    }

    #[test]
    fn a_missing_file_handler_list_keeps_the_pdf_editor() {
        let loaded: super::BrowserSettings =
            serde_json::from_str(r#"{"searchEngine":"bing"}"#).expect("settings parse");
        let loaded = loaded.normalize();
        assert_eq!(loaded.file_handlers.len(), 1);
        assert_eq!(loaded.file_handlers[0].extension, "pdf");
        assert_eq!(loaded.file_handlers[0].url, super::DEFAULT_PDF_EDITOR);
        assert_eq!(loaded.file_handlers[0].input_id, super::DEFAULT_PDF_INPUT);
    }

    #[test]
    fn an_empty_file_handler_list_stays_empty() {
        let loaded: super::BrowserSettings =
            serde_json::from_str(r#"{"fileHandlers":[]}"#).expect("settings parse");
        assert!(loaded.normalize().file_handlers.is_empty());
    }

    #[test]
    fn a_bad_file_handler_is_dropped_and_the_first_extension_wins() {
        let loaded: super::BrowserSettings = serde_json::from_str(
            r#"{"fileHandlers":[
                {"extension":".PNG","url":"https://example.com/png","inputId":"drop zone"},
                {"extension":"png","url":"https://example.com/second","inputId":"other"},
                {"extension":"","url":"https://example.com/x","inputId":""},
                {"extension":"pdf","url":"not a url","inputId":"pdf-dropzone-input"}
            ]}"#,
        )
        .expect("settings parse");
        let loaded = loaded.normalize();
        assert_eq!(loaded.file_handlers.len(), 1);
        assert_eq!(loaded.file_handlers[0].extension, "png");
        assert_eq!(loaded.file_handlers[0].url, "https://example.com/png");
        assert_eq!(loaded.file_handlers[0].input_id, "dropzone");
        assert_eq!(loaded.file_handlers[0].profile, "");
    }

    #[test]
    fn a_file_handler_keeps_a_profile_id_and_drops_a_bad_one() {
        let loaded: super::BrowserSettings = serde_json::from_str(
            r#"{"fileHandlers":[
                {"extension":"pdf","url":"https://example.com/pdf","inputId":"pdf-dropzone-input","profile":"work-1"},
                {"extension":"png","url":"https://example.com/png","inputId":"","profile":"../nope"}
            ]}"#,
        )
        .expect("settings parse");
        let loaded = loaded.normalize();
        assert_eq!(loaded.file_handlers[0].profile, "work-1");
        assert_eq!(loaded.file_handlers[1].profile, "");
    }

    #[test]
    fn a_file_from_before_the_new_tab_and_window_options_keeps_its_values() {
        let loaded: super::BrowserSettings =
            serde_json::from_str(r#"{"searchEngine":"bing","theme":"nord"}"#)
                .expect("settings parse");
        let loaded = loaded.normalize();
        assert_eq!(loaded.search_engine, "bing");
        assert_eq!(loaded.theme, "nord");
        assert_eq!(loaded.ui_font, "system");
        assert_eq!(loaded.tab_cycle_order, "strip");
        assert!(loaded.tab_switcher);
        assert_eq!(loaded.window_backdrop, "none");
        assert!(loaded.auto_update);
        assert_eq!(loaded.last_seen_version, "");
        assert_eq!(loaded.toast_position, "bottom-right");
    }

    #[test]
    fn a_missing_ui_font_stays_on_system_and_an_unknown_one_falls_back() {
        let missing: super::BrowserSettings =
            serde_json::from_str(r#"{"theme":"nord"}"#).expect("settings parse");
        let missing = missing.normalize();
        assert_eq!(missing.theme, "nord");
        assert_eq!(missing.ui_font, "system");
        let unknown: super::BrowserSettings =
            serde_json::from_str(r#"{"uiFont":"comic-sans"}"#).expect("settings parse");
        assert_eq!(unknown.normalize().ui_font, "system");
        let saved: super::BrowserSettings =
            serde_json::from_str(r#"{"uiFont":"inter"}"#).expect("settings parse");
        assert_eq!(saved.normalize().ui_font, "inter");
        let distinctive: super::BrowserSettings =
            serde_json::from_str(r#"{"uiFont":"fraunces"}"#).expect("settings parse");
        assert_eq!(distinctive.normalize().ui_font, "fraunces");
        let coding: super::BrowserSettings =
            serde_json::from_str(r#"{"uiFont":"martian-mono"}"#).expect("settings parse");
        assert_eq!(coding.normalize().ui_font, "martian-mono");
    }

    #[test]
    fn toast_position_keeps_a_saved_corner_and_drops_an_unknown_one() {
        let saved: super::BrowserSettings =
            serde_json::from_str(r#"{"toastPosition":"top-left"}"#).expect("settings parse");
        assert_eq!(saved.normalize().toast_position, "top-left");
        let unknown: super::BrowserSettings =
            serde_json::from_str(r#"{"toastPosition":"middle"}"#).expect("settings parse");
        assert_eq!(unknown.normalize().toast_position, "bottom-right");
    }

    #[test]
    fn unknown_tab_order_and_backdrop_values_fall_back() {
        let loaded: super::BrowserSettings =
            serde_json::from_str(r#"{"tabCycleOrder":"random","windowBackdrop":"glass"}"#)
                .expect("settings parse");
        let loaded = loaded.normalize();
        assert_eq!(loaded.tab_cycle_order, "strip");
        assert_eq!(loaded.window_backdrop, "none");
        let current = super::BrowserSettings::default().normalize();
        assert_eq!(current.clone().normalize(), current);
    }

    #[test]
    fn a_plaintext_assistant_key_migrates_and_is_not_written_back() {
        let loaded: super::BrowserSettings = serde_json::from_str(
            r#"{"assistantEndpoint":"https://openrouter.ai/api/v1","assistantModel":"openai/gpt-4o-mini","assistantKey":"sk-secret-value","theme":"zinc"}"#,
        )
        .expect("settings parse");
        assert_eq!(
            loaded.legacy_assistant_key.as_deref(),
            Some("sk-secret-value")
        );
        let mut loaded = loaded.normalize();
        assert_eq!(loaded.ai_provider, "custom");
        assert_eq!(
            loaded.ai_providers.custom.endpoint,
            "https://openrouter.ai/api/v1"
        );
        assert_eq!(loaded.ai_providers.custom.model, "openai/gpt-4o-mini");
        assert_eq!(
            loaded.ai_providers.local.endpoint,
            "http://127.0.0.1:11434/v1"
        );
        loaded.legacy_assistant_key = None;
        let saved = serde_json::to_string(&loaded).expect("settings json");
        assert!(!saved.contains("sk-secret-value"));
        assert!(!saved.contains("assistantKey"));
        assert!(!saved.contains("assistantEndpoint"));
        assert!(!saved.contains("assistantModel"));
        let again: super::BrowserSettings = serde_json::from_str(&saved).expect("round trip");
        let again = again.normalize();
        assert_eq!(again.ai_provider, "custom");
        assert_eq!(
            again.ai_providers.custom.endpoint,
            loaded.ai_providers.custom.endpoint
        );
        assert!(again.legacy_assistant_key.is_none());
    }

    #[test]
    fn a_localhost_assistant_endpoint_becomes_the_local_provider() {
        let loaded: super::BrowserSettings = serde_json::from_str(
            r#"{"assistantEndpoint":"http://127.0.0.1:11434/v1","assistantModel":"llama3.2"}"#,
        )
        .expect("settings parse");
        let loaded = loaded.normalize();
        assert_eq!(loaded.ai_provider, "local");
        assert_eq!(loaded.ai_providers.local.model, "llama3.2");
        assert_eq!(loaded.ai_providers.custom.endpoint, "");
    }

    #[test]
    fn an_older_ai_slot_without_thinking_still_loads() {
        let loaded: super::BrowserSettings = serde_json::from_str(
            r#"{"aiProvider":"openai","aiProviders":{"openai":{"endpoint":"https://api.openai.com/v1","model":"gpt-4o-mini","thinking":"max"}}}"#,
        )
        .expect("settings parse");
        let loaded = loaded.normalize();
        assert_eq!(loaded.ai_providers.openai.model, "gpt-4o-mini");
        assert_eq!(loaded.ai_providers.openai.thinking, "");
        assert_eq!(loaded.ai_providers.claude.thinking, "");
        let saved = serde_json::to_string(&loaded).expect("settings save");
        assert!(saved.contains("\"thinking\":\"\""));
    }

    #[test]
    fn an_unknown_provider_falls_back_to_local() {
        let loaded: super::BrowserSettings =
            serde_json::from_str(r#"{"aiProvider":"retired"}"#).expect("settings parse");
        assert_eq!(loaded.normalize().ai_provider, "local");
    }

    #[test]
    fn a_saved_price_pin_becomes_one_shopping_pin() {
        let mut settings = super::BrowserSettings::default();
        settings.toolbar_order = vec!["price".into(), "reading".into()];
        settings.hidden_toolbar_extensions = vec!["price".into(), "shopping".into()];
        let settings = settings.normalize();
        assert_eq!(
            settings.toolbar_order,
            vec!["shopping".to_string(), "reading".to_string()]
        );
        assert_eq!(
            settings.hidden_toolbar_extensions,
            vec!["shopping".to_string()]
        );
        let mut both = super::BrowserSettings::default();
        both.toolbar_order = vec!["price".into(), "shopping".into(), "reading".into()];
        assert_eq!(
            both.normalize().toolbar_order,
            vec!["shopping".to_string(), "reading".to_string()]
        );
    }
}
