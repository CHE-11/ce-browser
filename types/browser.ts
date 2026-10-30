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
};

export type TabsSnapshot = {
  tabs: TabInfo[];
  activeId: number | null;
  /** Selected tabs in strip order. Includes the active tab whenever one is open. */
  selectedIds: number[];
  canReopen: boolean;
  fullscreen: boolean;
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
  | "extension";

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

export type SearchEngine = "google" | "bing" | "duckduckgo" | "brave" | "startpage" | "custom";

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
