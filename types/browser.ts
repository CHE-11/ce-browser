export type TabInfo = {
  id: number;
  /** Display URL. Internal pages use `ce://<page>`. */
  url: string;
  title: string;
  favicon: string | null;
  pinned: boolean;
  loading: boolean;
  canGoBack: boolean;
  canGoForward: boolean;
  zoom: number;
  /** False for restored tabs whose page has not been loaded yet. */
  loaded: boolean;
  /** Browser window that shows this tab. The first window is `main`. */
  window: string;
};

export type WindowTabs = {
  label: string;
  activeId: number | null;
  fullscreen: boolean;
};

export type TabsSnapshot = {
  tabs: TabInfo[];
  activeId: number | null;
  /** Selected tabs in strip order. Includes the active tab whenever one is open. */
  selectedIds: number[];
  canReopen: boolean;
  fullscreen: boolean;
  /** One entry per browser window, with that window's active tab. */
  windows: WindowTabs[];
};

export type BrowserCommand = "focus-address" | "find" | "bookmark" | "downloads";

export type PopupKind =
  | "downloads"
  | "bookmark-edit"
  | "bookmark-folder"
  | "bookmarks-bar-menu"
  | "bookmark-add"
  | "tab-menu"
  | "app-menu"
  | "omnibox"
  | "extension"
  | "print-pdf"
  | "page-capture";

export type PopupRequest = {
  kind: PopupKind;
  /** Logical px relative to the browser UI view. */
  x: number;
  y: number;
  width: number;
  height: number;
  /** URL query string read by the popup page. For `extension` it is the extension id. */
  params?: string;
  /** Popups that take focus close when they lose it. */
  focus?: boolean;
};

export type PopupChange = {
  kind: PopupKind | null;
  params: string | null;
};

export type BookmarkNode = {
  id: string;
  title: string;
  /** Null for folders and dividers. */
  url: string | null;
  /** Null for links and dividers. */
  children: BookmarkNode[] | null;
  /** A divider has no address and no children. */
  separator: boolean;
  addedAt: number;
};

export type BookmarkTree = {
  bar: BookmarkNode;
  other: BookmarkNode;
};

export type DownloadState = "inProgress" | "paused" | "completed" | "cancelled" | "interrupted";

export type DownloadItem = {
  id: number;
  url: string;
  fileName: string;
  path: string;
  mimeType: string;
  totalBytes: number | null;
  receivedBytes: number;
  state: DownloadState;
  startedAt: number;
  finishedAt: number | null;
  canResume: boolean;
};

export type HistoryItem = {
  id: number;
  url: string;
  title: string;
  visitCount: number;
  lastVisited: number;
};

export type ExtensionInfo = {
  id: string;
  name: string;
  version: string;
  description: string;
  enabled: boolean;
  icon: string | null;
  popup: string | null;
  optionsPage: string | null;
  folder: string | null;
};

export type SearchEngine =
  | "google"
  | "bing"
  | "duckduckgo"
  | "brave"
  | "startpage"
  | "kagi"
  | "mojeek"
  | "custom";

export type BrowserSettings = {
  searchEngine: SearchEngine;
  customSearchUrl: string;
  homePage: string;
  startup: "restore" | "home";
  /** URLs pinned every time this profile starts. */
  pinnedTabs: string[];
  /** Empty means the system Downloads folder. */
  downloadDir: string;
  askWhereToSave: boolean;
  showBookmarksBar: boolean;
  theme: string;
  colorMode: "dark" | "light" | "system";
  uiZoom: number;
  /** Default website zoom, as a percent. */
  pageZoom: number;
  /** Shared sync folder. Empty when sync is off. This path stays on this computer. */
  syncFolder: string;
  /** Last choices in the Print to PDF editor. */
  printEditor: PrintEditorSettings;
  /** Last choices in the Page to PNG popover. */
  pageCapture: PageCaptureSettings;
};

export type PrintMode = "article" | "full";
export type PrintImageSize = "original" | "large" | "medium" | "small" | "none";
export type PrintFont = "original" | "serif" | "sans";
export type PaperSize = "letter" | "a4" | "legal" | "tabloid";
export type PrintMargins = "none" | "narrow" | "normal" | "wide";

export type PrintEditorSettings = {
  /** `article` keeps the main content only. `full` keeps the whole page. */
  mode: PrintMode;
  removeAds: boolean;
  removeNavigation: boolean;
  removePopups: boolean;
  removeSocial: boolean;
  removeMedia: boolean;
  /** Percent of the page's text size, 70 to 150. */
  textSize: number;
  imageSize: PrintImageSize;
  removeBackgrounds: boolean;
  font: PrintFont;
  /** Title, source URL and date above the content. */
  showHeader: boolean;
  paper: PaperSize;
  landscape: boolean;
  margins: PrintMargins;
  /** WebView2's own header and footer on every page. */
  headerFooter: boolean;
  printBackgrounds: boolean;
  openAfterSave: boolean;
};

export type PageCaptureSettings = {
  /** The whole document, not only what is on screen. */
  fullPage: boolean;
  /** Scroll through the page first so lazy images load. */
  loadLazy: boolean;
};

export type PageCaptureResult = {
  /** Set when the image was saved to a file. */
  path: string | null;
  width: number;
  height: number;
  /** The page was taller than the capture limit, so the image stops early. */
  truncated: boolean;
};

/**
 * Sent by a built-in extension popover to its toolbar button. The button runs the job in the
 * browser UI, because a save dialog takes focus and closes the popover.
 */
export type PageToolRequest =
  | {
      tool: "page-capture";
      action: "save" | "copy";
      /** Browser window whose toolbar should run the job. */
      window: string;
      tabId: number;
      title: string;
      url: string;
      fullPage: boolean;
      loadLazy: boolean;
    }
  | {
      tool: "print-pdf";
      action: "quick" | "as-is";
      window: string;
      tabId: number;
      title: string;
      url: string;
    };

/** A tab's document copied for the print editor: no scripts, absolute URLs. */
export type ExtractedPage = {
  title: string;
  url: string;
  html: string;
  lang: string;
  byline: string;
  siteName: string;
  published: string;
};

/** Paper and layout for a PDF. Sizes are inches and describe the page in portrait. */
export type PdfOptions = {
  pageWidth: number;
  pageHeight: number;
  landscape: boolean;
  margin: number;
  backgrounds: boolean;
  headerFooter: boolean;
  scale: number;
  title: string;
  url: string;
};

export type UserStyles = {
  css: string;
  /** Path of this profile's styles.css. */
  path: string;
};

export type SyncStatus = {
  folder: string;
  error: string | null;
  /** True when `error` is a failure to write one of the files in the sync folder. */
  saveError: boolean;
};

export type BrowserProfile = {
  id: string;
  name: string;
  current: boolean;
  /** A Start menu shortcut exists for this profile. */
  startMenu: boolean;
  /** A taskbar shortcut exists for this profile. */
  taskbar: boolean;
  /** `default`, a preset id, or `custom`. */
  iconId: string;
  /** PNG data URL of the icon used for the window, Start menu, and taskbar. */
  icon: string;
};

export type ProfileIconChoice = {
  id: string;
  label: string;
  /** PNG data URL. */
  icon: string;
};

export type OmniboxSuggestion = {
  kind: "search" | "url" | "bookmark" | "tab" | "history";
  title: string;
  url: string;
  /** Set for `tab` suggestions: switching selects the open tab. */
  tabId?: number;
};

export type OmniboxState = {
  suggestions: OmniboxSuggestion[];
  selected: number;
};
