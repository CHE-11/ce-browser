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
  /** The page is playing sound. */
  audible: boolean;
  /** Muted by the user. Background muting from settings does not set this. */
  muted: boolean;
  /** The page is suspended until the tab is shown again. */
  sleeping: boolean;
  groupId: number | null;
  workspace: string;
  /** Seconds between automatic reloads. Zero is off. */
  refreshSecs: number;
  /** Compare the page text before each reload and mark the tab when it differs. */
  watchChanges: boolean;
  /** The watched page changed since this tab was last selected. */
  changed: boolean;
  /** An InPrivate tab. It is not saved and it is left out of history. */
  ephemeral: boolean;
  /** Page errors since the last navigation. */
  errors: number;
  /** The page stopped answering. Cleared on the next navigation, or once it recovers. */
  unresponsive: boolean;
  /** `#rrggbb` from the site's theme color or favicon. */
  accent: string | null;
  /** Registrable domain of an http(s) page, such as `example.co.uk`. Empty for other pages. */
  site: string;
  /** When the tab was last selected, in milliseconds since 1970. Zero until it is shown. */
  lastActive: number;
  /** Load progress from 0 to 100. */
  progress: number;
  /** Why the page can't be shown. The browser UI draws this in place of the page. */
  failure: TabFailure | null;
  /** Bytes the page used just before it went to sleep. Zero while awake or when unknown. */
  sleepSaved: number;
};

export type TabFailureKind = "crashed" | "offline" | "dns" | "certificate" | "timeout" | "refused" | "other";

export type TabFailure = {
  kind: TabFailureKind;
  /** Address that failed, so Retry can load it again. */
  url: string;
  /** WebView2 error name, for example `HOST_NAME_NOT_RESOLVED`. */
  code: string;
  /** Extra text, such as the certificate issuer and expiry. */
  detail: string;
};

/** One tab as it was when it closed. */
export type ClosedTab = {
  url: string;
  title: string;
  favicon: string | null;
  pinned: boolean;
  index: number;
  window: string;
  workspace: string;
};

export type ClosedGroup = {
  name: string;
  color: TabGroupColor;
  container: string;
};

/** A closed tab, a closed group, or tabs closed together. Restored as one. */
export type ClosedEntry = {
  id: number;
  /** Milliseconds since 1970. */
  closedAt: number;
  kind: "tab" | "group" | "batch";
  group: ClosedGroup | null;
  tabs: ClosedTab[];
};

export type SplitView = {
  left: number;
  right: number;
  /** Share of the page area the left tab takes, 0.2 to 0.8. */
  ratio: number;
};

export type WindowTabs = {
  label: string;
  activeId: number | null;
  fullscreen: boolean;
  workspace: string;
  split: SplitView | null;
  /** Address of the open peek, if any. */
  peekUrl: string | null;
  /** `phone`, `tablet`, `laptop`, or empty for the full page. */
  device: string;
};

export type RewriteRule = {
  id: number;
  enabled: boolean;
  contains: string;
  action: "block" | "redirect" | "file";
  target: string;
};

export type ExternalBrowser = {
  id: string;
  name: string;
};

export type SiteCookie = {
  name: string;
  value: string;
  domain: string;
  path: string;
};

export type ReadingStats = {
  words: number;
  minutes: number;
  ease: number;
};

export type AiProviderId = "openai" | "claude" | "custom" | "local";

export type AiProviderSlot = {
  endpoint: string;
  model: string;
  /** `low`, `medium`, or `high`. Empty keeps the provider default. */
  thinking: string;
};

export type AiProviders = {
  openai: AiProviderSlot;
  claude: AiProviderSlot;
  custom: AiProviderSlot;
  local: AiProviderSlot;
};

/** Whether a provider key is saved. The key itself never crosses the bridge. */
export type AiKeyStatus = {
  hasKey: boolean;
  /** Last four characters, when a saved key could be read. */
  hint: string;
  /** A matching environment variable is set and no saved key is in use. */
  fromEnv: boolean;
  /** Ciphertext is present but this Windows user cannot read it. */
  unreadable: boolean;
};

export type PricePoint = {
  at: number;
  price: number;
  currency: string;
  availability: string;
};

export type ProductPrice = {
  id: string;
  url: string;
  title: string;
  price: number;
  currency: string;
  availability: string;
  imageUrl: string;
  store: string;
  trackedAt: number;
  lastCheckedAt: number;
  lastError: string;
  /** `fetch` or `tab` when the last check succeeded. */
  lastMethod: string;
  targetPrice: number | null;
  alertOnDrop: boolean;
  alertBelowTarget: boolean;
  alertBackInStock: boolean;
  alertDropArmed: boolean;
  alertTargetArmed: boolean;
  alertStockArmed: boolean;
  checkIntervalHours: number;
  paused: boolean;
  note: string;
  tags: string[];
  listPrice: number | null;
  history: PricePoint[];
  previousPrice: number | null;
  previousAvailability: string;
  dropped: boolean;
  stockChanged: boolean;
};

export type PriceCheckProgress = {
  checked: number;
  total: number;
  done: boolean;
};

export type TrackedPricePatch = {
  id: string;
  targetPrice?: number | null;
  clearTarget?: boolean;
  alertOnDrop?: boolean;
  alertBelowTarget?: boolean;
  alertBackInStock?: boolean;
  checkIntervalHours?: number;
  paused?: boolean;
  note?: string;
  tags?: string[];
};

export type SearchResult = {
  title: string;
  url: string;
  snippet: string;
};

export type Coupon = {
  code: string;
  description: string;
  /** Search result that shows the code. */
  source: string;
  expiry: string;
  triedAt: number;
  worked: boolean;
  failed: boolean;
};

export type Offer = {
  store: string;
  price: number | null;
  currency: string;
  url: string;
  note: string;
  verified: boolean;
  variantMismatch: boolean;
  shipping: string;
  /** Product image from the other page. Empty until that page is read. */
  imageUrl: string;
  /** The confirmed price is under the tracked price. */
  lower: boolean;
};

export type ShoppingDeals = {
  /** Price read from the page, when it has structured product data. */
  product: ProductPrice | null;
  /** Product name that was searched for. */
  query: string;
  /** Host of the current store, without `www.`. */
  store: string;
  summary: string;
  coupons: Coupon[];
  /** Confirmed prices elsewhere, cheapest first. */
  offers: Offer[];
  sources: SearchResult[];
  aiAvailable: boolean;
  checkedAt: number;
  cached: boolean;
  /** Canonical product key shared with price tracking. */
  key: string;
  /** Which identifier was searched, and which engines were skipped. */
  searchNote: string;
};

export type StoragePair = {
  key: string;
  value: string;
};

export type TabGroupColor = "grey" | "blue" | "red" | "yellow" | "green" | "pink" | "purple" | "cyan" | "orange";

export type TabGroup = {
  id: number;
  name: string;
  color: TabGroupColor;
  collapsed: boolean;
  window: string;
  /** Empty uses the default profile. */
  container: string;
};

export type Workspace = {
  id: string;
  name: string;
  color: TabGroupColor;
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
  groups: TabGroup[];
  workspaces: Workspace[];
};

export type DarkMode = {
  enabled: boolean;
  /** Percent, 50 to 150. */
  brightness: number;
  contrast: number;
};

/** Controls stored for one site. A rule for a host also covers its subdomains. */
export type SiteControl = {
  forceInteraction: boolean;
  cleanPage: boolean;
  dark: DarkMode;
  /** Empty keeps the browser's normal user agent. */
  userAgent: string;
  /** 1 leaves the page's own speed. */
  videoSpeed: number;
  blockScript: boolean;
  blockImages: boolean;
  blockFonts: boolean;
  /** CSS selectors hidden by the element zapper. */
  hidden: string[];
  /** `default`, `none`, `origin` or `same-origin`. */
  referrer: ReferrerMode;
  /** Page zoom in percent remembered for this site. Zero follows the default page zoom. */
  zoom: number;
};

export type ReferrerMode = "default" | "none" | "origin" | "same-origin";

export type UserScript = {
  id: string;
  name: string;
  enabled: boolean;
  /** Empty matches every site. */
  hosts: string[];
  css: string;
  js: string;
};

export type PageRules = {
  sites: Record<string, SiteControl>;
  scripts: UserScript[];
};

/** Absent fields stay as they are. */
export type SitePatch = {
  forceInteraction?: boolean;
  cleanPage?: boolean;
  darkEnabled?: boolean;
  brightness?: number;
  contrast?: number;
  userAgent?: string;
  videoSpeed?: number;
  blockScript?: boolean;
  blockImages?: boolean;
  blockFonts?: boolean;
  hide?: string;
  clearHidden?: boolean;
  referrer?: ReferrerMode;
  /** Percent. Zero forgets the site's zoom. */
  zoom?: number;
};

export type PeekInfo = {
  window: string;
  url: string;
  title: string;
  favicon: string | null;
};

export type BrowserCommand =
  | "focus-address"
  | "find"
  | "bookmark"
  | "downloads"
  | "palette"
  | "tab-search"
  | "shortcut-sheet"
  | "tab-switcher"
  | "tab-switcher-back";

export type PopupKind =
  | "downloads"
  | "bookmark-edit"
  | "bookmark-folder"
  | "bookmarks-bar-menu"
  | "bookmarks-bar-overflow"
  | "bookmark-add"
  | "bookmark-rename"
  | "tab-menu"
  | "app-menu"
  | "status"
  | "omnibox"
  | "command-palette"
  | "extension"
  | "print-pdf"
  | "page-capture"
  | "media-hub"
  | "media-grid"
  | "what-runs"
  | "ad-blocker"
  | "extension-pin"
  | "tab-group"
  | "tab-group-menu"
  | "workspaces"
  | "page-controls"
  | "network"
  | "translate"
  | "password"
  | "reading"
  | "shopping"
  | "toast"
  | "tab-search"
  | "tab-switcher"
  | "tab-overview"
  | "tab-hover"
  | "shortcut-sheet"
  | "unresponsive"
  | "site-info"
  | "find-all"
  | "selection-translate"
  | "boards"
  | "board-images"
  | "theme-picker";

/** Windows app theme. Websites do not follow this. */
export type WindowsColorScheme = "light" | "dark";

/** A theme shown over the saved one. `commit` stores it before the settings file catches up. */
export type ThemePreview = {
  theme: string | null;
  commit: boolean;
};

/**
 * Where a popup sits against its anchor. `bottom-*` hangs below (or above, near the bottom of
 * the screen), `right-start`/`left-start` sit beside it with matching tops, `point` puts the
 * top-left corner on a click, and `center` puts the popup in the middle of the window.
 */
export type PopupPlacement =
  | "bottom-start"
  | "bottom-end"
  | "bottom-center"
  | "right-start"
  | "left-start"
  | "point"
  | "center";

/** Logical px in the webview that asks for the popup. */
export type PopupRect = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export type PopupRequest = {
  kind: PopupKind;
  /** URL query string read by the popup page. For `extension` it is the extension id. */
  params?: string;
  /** Button or point the popup hangs from. A point has no width or height. */
  anchor: PopupRect;
  placement?: PopupPlacement;
  /** Popups that take focus close when they lose it. */
  focus?: boolean;
  /** Opens beside the popup that asks instead of replacing it. */
  child?: boolean;
  /** Logical px. The popup is at least this wide. */
  minWidth?: number;
};

/** What a popup window shows. Its size report carries `token`. */
export type PopupShow = {
  kind: PopupKind;
  params: string;
  token: number;
  /** 0 for a popup opened from the browser UI, one more for each submenu. */
  level: number;
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

/** A cached site icon. `host` is the lowercase URL hostname. `dataUrl` is a PNG data URL. */
export type SiteFavicon = {
  host: string;
  dataUrl: string;
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
  /** Saved with range requests instead of the single WebView2 download. */
  accelerated: boolean;
  ranges: { start: number; end: number; received: number }[];
};

export type LocalEntry = {
  name: string;
  path: string;
  directory: boolean;
  size: number;
  kind: string;
};

export type BrowseResult =
  | { type: "directory"; path: string; parent: string | null; entries: LocalEntry[] }
  | {
      type: "file";
      path: string;
      parent: string | null;
      name: string;
      kind: string;
      text: string;
      tooBig: boolean;
      size: number;
    };

export type HistoryItem = {
  id: number;
  url: string;
  title: string;
  visitCount: number;
  lastVisited: number;
  /** Visits that began with the address typed into the address bar. */
  typedCount: number;
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
  /** `chrome` or `edge` when the package can be updated from that store. */
  store: "chrome" | "edge" | null;
  /** Newer store version waiting for the user to install it. */
  updateVersion: string | null;
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
  /** Files of 8 MB or more use four connections when the server accepts ranges. */
  accelerateDownloads: boolean;
  /** Empty keeps the name the site suggested. */
  downloadNameTemplate: string;
  showBookmarksBar: boolean;
  /** Folder icons on the bookmarks bar. Off leaves folder titles only. Site favicons always show. */
  bookmarksBarFolderIcons: boolean;
  /** Text size of bookmarks on the bar, in rem. */
  bookmarksBarTextSize: number;
  /** Side padding of bookmarks on the bar, in rem. Top and bottom get half. */
  bookmarksBarPadding: number;
  /** Text size of rows in a bookmark folder menu, in rem. */
  bookmarkFolderTextSize: number;
  /** Side padding of rows in a bookmark folder menu, in rem. Top and bottom get half. */
  bookmarkFolderPadding: number;
  /** Text size of the bookmarks bar context menu, in rem. */
  bookmarksBarMenuTextSize: number;
  /** Side padding of rows in the bookmarks bar context menu, in rem. Top and bottom get half. */
  bookmarksBarMenuPadding: number;
  /** The status bar under every page. */
  showStatusBar: boolean;
  /** Memory, time and the other status readings. Off leaves the version and zoom. */
  statusStats: boolean;
  theme: string;
  colorMode: "dark" | "light" | "system";
  /** Browser UI font. `system` keeps Segoe UI. */
  uiFont: string;
  uiZoom: number;
  /** Default website zoom, as a percent. */
  pageZoom: number;
  /** Shared sync folder. Empty when sync is off. This path stays on this computer. */
  syncFolder: string;
  /** Last choices in the Print to PDF editor. */
  printEditor: PrintEditorSettings;
  /** Last choices in the Page to PNG popover. */
  pageCapture: PageCaptureSettings;
  /** Extension ids left off the toolbars. Empty means every enabled extension stays pinned. */
  hiddenToolbarExtensions: string[];
  /** Order of pinned toolbar icons. Empty keeps the built-in order, then extensions. */
  toolbarOrder: string[];
  /** Built-in tools sit on the right strip. Off puts them on the address bar. */
  verticalToolbar: boolean;
  /** Pinned extensions share the vertical tool strip on the right. */
  verticalToolbarExtensions: boolean;
  /** Short names under icons on the vertical tool strip. */
  verticalToolbarLabels: boolean;
  /** Tint the active tab and address bar from the site's color. */
  siteAccent: boolean;
  /** Site theme color or favicon along the top of pinned tabs. */
  pinTabBorder: boolean;
  /** Target language code for the translate tool. */
  translateLanguage: string;
  /** Provider URL. `%s` is the text and `%t` is the language. */
  translateUrl: string;
  /** Active Shopping summary provider. */
  aiProvider: AiProviderId;
  /** Endpoint and model remembered for each provider. Keys are not included. */
  aiProviders: AiProviders;
  /** DuckDuckGo region for shopping searches, for example `us-en`. */
  shoppingRegion: string;
  /** Model override for Shopping summaries. Blank uses the active provider's model. */
  shoppingModel: string;
  /** Store hosts added to one extra deal search. Empty skips that query. */
  shoppingCompetitors: string[];
  /** Restores the window and focuses the address bar. */
  summonHotkey: string;
  /** On battery, background tabs sleep sooner and the visible page stays near 30 frames a second. */
  batterySaver: boolean;
  /** Built-in ad blocker. */
  adblock: AdblockSettings;
  /** The workspace button on the tab strip. Off hides switching. */
  workspacesEnabled: boolean;
  /** `horizontal` puts tabs above the address bar, `vertical` in a sidebar on the left. */
  tabLayout: "horizontal" | "vertical";
  /** The vertical sidebar shows icons only. */
  verticalTabsCollapsed: boolean;
  /** Width of the expanded vertical sidebar in CSS pixels, 160 to 400. */
  verticalTabsWidth: number;
  /** Tabs that are not on screen are muted. */
  muteBackgroundTabs: boolean;
  /** Shift+click on a link opens it in a peek instead of a new tab. */
  peekOnShiftClick: boolean;
  tabSleep: TabSleepSettings;
  /** Drop `utm_*` and other click identifiers from URLs. */
  stripTracking: boolean;
  /** Add stable noise to canvas, WebGL and audio reads. */
  fingerprintShield: boolean;
  /** Load `http` pages over `https` unless the site is excepted. */
  httpsOnly: boolean;
  /** Hosts that may stay on `http`. */
  httpsExceptions: string[];
  /** Save page text on this computer so history search can match words from the page. */
  indexPageText: boolean;
  /** Ctrl+Tab order: `strip` follows the tab strip, `recent` goes to the last used tab first. */
  tabCycleOrder: "strip" | "recent";
  /** In recent order, Ctrl+Tab shows a switcher with thumbnails until Ctrl is released. */
  tabSwitcher: boolean;
  /** Windows draws this behind the tab strip and toolbars. */
  windowBackdrop: "none" | "mica" | "acrylic";
  /** Check for a new version at startup and once a day. */
  autoUpdate: boolean;
  /** Version that last ran. */
  lastSeenVersion: string;
  /** Resting on a tab shows a small preview of its page. */
  tabHoverPreview: boolean;
  /** Preferred width of a horizontal tab, in CSS pixels. Tabs shrink when the strip is full. */
  tabWidth: number;
  /** Text size of the tab context menu, in rem. */
  tabMenuTextSize: number;
  /** Side padding of rows in the tab context menu, in rem. Top and bottom get half. */
  tabMenuPadding: number;
  /** Corner or edge of the page area where toasts appear. */
  toastPosition: ToastPosition;
};

export type ToastPosition = "top-left" | "top-center" | "top-right" | "bottom-left" | "bottom-center" | "bottom-right";

/** One on-screen page in a status snapshot. */
export type StatusPage = {
  id: number;
  /** Renderer working set, in bytes. Null when it could not be matched. */
  tabUsed: number | null;
  /** Another open tab can share this renderer. */
  tabShared: boolean;
  /** JavaScript heap, in bytes, when the renderer process was not matched. */
  tabHeap: number | null;
  siteHost: string;
  siteAllTimeMs: number;
  tabSessionMs: number;
  /** The page is on screen, so the clocks keep counting from this snapshot. */
  shown: boolean;
};

/** Memory and time for the status bar. The flat fields are the focused window's active tab. */
export type StatusSnapshot = {
  version: string;
  startedAt: number;
  systemUsed: number;
  systemTotal: number;
  appUsed: number;
  tabUsed: number | null;
  tabShared: boolean;
  tabHeap: number | null;
  siteHost: string;
  siteAllTimeMs: number;
  tabSessionMs: number;
  batterySaver: boolean;
  pages: StatusPage[];
};

export type StatusText = {
  tabId: number;
  text: string;
};

/** Choices kept for the password generator, in the profile's `password.json`. */
export type PasswordSettings = {
  length: number;
  upper: boolean;
  lower: boolean;
  digits: boolean;
  symbols: boolean;
  minSymbols: number;
  avoidAmbiguous: boolean;
  /** Characters to leave out, separated by commas. */
  exclude: string;
};

export type Keyword = {
  keyword: string;
  title: string;
  /** `%s` is replaced with the query. */
  url: string;
};

export type ShortcutBinding = {
  action: string;
  label: string;
  /** One token, or two for a chord. Empty unbinds the action. */
  keys: string[];
  /** Cheat sheet heading. Filled by the browser and never saved. */
  category?: string;
};

export type TabSleepSettings = {
  enabled: boolean;
  /** Minutes a tab stays off screen before it sleeps, 5 to 720. */
  minutes: number;
  /** Older settings files may still contain this. Pinned tabs never sleep, and load clears it. */
  sleepPinned: boolean;
  /** Hosts that never sleep, lowercase and without a leading `www.`. */
  neverSleepSites: string[];
};

export type PermissionGrant = {
  origin: string;
  kind: "camera" | "microphone" | "location" | "notifications" | string;
  /** `allow` (every visit), `session` (until the browser exits), `deny`, or `asked`. */
  state: "allow" | "deny" | "session" | "asked" | string;
  /** WASAPI endpoint id. Empty uses the Windows default capture device. */
  deviceId: string;
  /** Friendly name matched against the page's device list. */
  deviceLabel: string;
};

export type MicrophoneDevice = {
  id: string;
  label: string;
  isDefault: boolean;
};

export type MicrophoneLevel = {
  deviceId: string;
  /** Peak from 0 to 1. */
  level: number;
};

export type MicrophoneCapture = {
  tabId: number;
  active: boolean;
};

export type Container = {
  id: string;
  name: string;
  color: TabGroupColor;
};

export type NetworkEntry = {
  method: string;
  url: string;
  kind: string;
  status: number | null;
  /** Unix milliseconds when the response arrived. */
  time: number;
  /** Milliseconds from the request start until the response, when the start was seen. */
  durationMs: number | null;
};

export type AdblockSettings = {
  enabled: boolean;
  /** Hosts where blocking is paused, lowercase and without a leading `www.`. */
  allowedSites: string[];
};

/** One kind of request blocked from a host, such as scripts or images. */
export type AdblockKind = {
  kind: string;
  count: number;
};

/** A host with requests blocked on the current page. */
export type AdblockHost = {
  host: string;
  count: number;
  /** Blocklist domain that matched. Same as `host` when the host itself is listed. */
  rule: string;
  kinds: AdblockKind[];
};

/** Requests blocked in a tab since its current page started loading. */
export type AdblockStats = {
  tabId: number;
  blocked: number;
  hosts: AdblockHost[];
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

export type RecordingStatus = {
  running: boolean;
  secondsLeft: number;
  path: string | null;
  error: string | null;
};

export type MediaElement = {
  tabId: number;
  index: number;
  kind: string;
  /** Media address. Often a blob URL. */
  label: string;
  /** Title or accessible name on the element itself. */
  title: string;
  /** Poster image address, when the page set one. */
  poster: string;
  current: number;
  duration: number;
  paused: boolean;
  muted: boolean;
  /** 0 to 1. */
  volume: number;
  playbackRate: number;
  width: number;
  height: number;
  ended: boolean;
  tabTitle: string;
  tabUrl: string;
  favicon: string | null;
  site: string;
};

export type PageMedia = {
  url: string;
  kind: string;
  width: number;
  height: number;
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
      action:
        | "save"
        | "copy"
        | "region"
        | "edit"
        | "record-tab"
        | "record-area"
        | "record-window"
        | "stop-record"
        | "media"
        | "save-html"
        | "save-markdown"
        | "ocr"
        | "ocr-region";
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
    }
  | {
      tool: "boards";
      action: "page" | "full-page" | "region" | "element" | "link" | "images";
      window: string;
      tabId: number;
      title: string;
      url: string;
      boardId: string;
    };

export type PinKind = "page" | "region" | "element" | "link" | "image" | "file";

/** One file, image or link sent to `import_board_items`. */
export type BoardImportItem = {
  name: string;
  mime: string;
  url: string;
  /** Base64 file bytes. Empty when `path` or `url` is set. */
  data: string;
  /** Local path from the file picker. Empty for a drop, paste or link. */
  path: string;
};

/** A folder on `ce://boards`. Folders nest; `parentId` is null at the top level. */
export type BoardFolder = {
  id: string;
  name: string;
  parentId: string | null;
  createdAt: number;
};

export type Board = {
  id: string;
  name: string;
  /** Null when the board sits at the top level. */
  folderId: string | null;
  /** Pin whose image shows for the board. Null uses the newest pin. */
  coverPinId: string | null;
  createdAt: number;
  updatedAt: number;
};

/** One saved item on a board. Times are milliseconds since the epoch. */
export type Pin = {
  id: string;
  boardId: string;
  kind: PinKind;
  title: string;
  /** Page the pin was saved from. */
  pageUrl: string;
  /** Link target for link pins, image address for image pins. */
  linkUrl: string;
  /** Section text or link description. */
  text: string;
  note: string;
  hasImage: boolean;
  width: number;
  height: number;
  createdAt: number;
  /** Original name for a file pin, and for an image added from a file. */
  fileName: string;
  mime: string;
};

export type BoardsSnapshot = {
  folders: BoardFolder[];
  boards: Board[];
  pins: Pin[];
  /** Board the toolbar and the page menu save to by default. */
  lastBoardId: string | null;
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

export type TechnologyCategory =
  | "framework"
  | "language"
  | "library"
  | "ui"
  | "cms"
  | "analytics"
  | "server"
  | "hosting"
  | "build"
  | "other";

/** One technology the What runs scan found on a page. */
export type DetectedTechnology = {
  name: string;
  category: TechnologyCategory;
  /** Null when the page does not expose a version. */
  version: string | null;
  /** Project home page, shown as a link. */
  url: string | null;
};

export type PageTechnologies = {
  technologies: DetectedTechnology[];
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

export type SyncConflict = {
  id: string;
  path: string;
  label: string;
  time: number;
  summary: string;
  canMerge: boolean;
};

export type ImportedProfile = {
  id: string;
  name: string;
};

export type HistoryHit = HistoryItem & {
  /** Words from the page around the match. Empty when the title or address matched. */
  snippet: string;
};

export type BookmarkHealthBookmark = {
  id: string;
  title: string;
  url: string;
};

export type BookmarkDuplicateGroup = {
  url: string;
  bookmarks: BookmarkHealthBookmark[];
};

export type BookmarkHealthLink = {
  id: string;
  title: string;
  url: string;
  /** `dead`, `blocked`, or `unreachable`. */
  status: string;
  detail: string;
};

export type BookmarkHealthReport = {
  duplicates: BookmarkDuplicateGroup[];
  failed: BookmarkHealthLink[];
  checked: number;
};

export type BookmarkHealthProgress = {
  done: number;
  total: number;
};

/**
 * One part of a profile the copy dialog can include.
 * Keep this list in step with `COPY_PARTS` in `src-tauri/src/profiles.rs`.
 */
export type ProfileCopyPart =
  | "search"
  | "history"
  | "appearance"
  | "downloads"
  | "bookmarks"
  | "extensions"
  | "cookies"
  | "logins"
  | "siteData"
  | "tabs"
  | "startup"
  | "openTabs"
  | "privacy"
  | "adblock"
  | "ai"
  | "shopping"
  | "translate"
  | "system"
  | "shortcuts"
  | "rewrites"
  | "boards"
  | "passwords"
  | "print"
  | "window"
  | "icon";

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
  /** The first 10 letters of the profile name are drawn on the icon. */
  iconText: boolean;
};

export type ProfileIconChoice = {
  id: string;
  label: string;
  /** PNG data URL. */
  icon: string;
};

export type OmniboxSuggestion = {
  kind: "search" | "url" | "bookmark" | "tab" | "history" | "answer";
  title: string;
  url: string;
  /** Set for `tab` suggestions: switching selects the open tab. */
  tabId?: number;
  /** Name of the other workspace an open tab is in. */
  workspace?: string;
  /** Set for `history` suggestions, so Shift+Delete can remove the entry. */
  historyId?: number;
};

export type UpdateStatus = {
  current: string;
  /** A newer version that is ready to install. */
  available: string | null;
  /** Release notes for `available`. */
  notes: string;
  checking: boolean;
  installing: boolean;
  error: string | null;
  /** Milliseconds since 1970 of the last finished check. Zero before the first one. */
  checkedAt: number;
};

export type ReleaseNotes = {
  version: string;
  name: string;
  /** Markdown from the release page. */
  body: string;
  publishedAt: string;
  url: string;
};

export type BackdropSupport = {
  mica: boolean;
  acrylic: boolean;
  /** The setting asks for a backdrop the open window was not created for. A restart shows it. */
  restartNeeded: boolean;
};

/** Text matches on one page. `index` is the highlighted match, or -1. */
export type FindResult = {
  count: number;
  index: number;
};

/** Matches in one tab for "Find in all tabs". */
export type FindTabResult = {
  id: number;
  title: string;
  url: string;
  /** Null when the tab was not searched. */
  count: number | null;
  /** Why the tab was not searched, such as `Not loaded`. */
  skipped: string;
};

export type CertificateInfo = {
  subject: string;
  issuer: string;
  /** Seconds since 1970. */
  validFrom: number;
  validTo: number;
  /** For example `TLS 1.3`. */
  protocol: string;
  cipher: string;
  keyExchange: string;
};

/** Connection, permissions and stored data for the site in one tab. */
export type SiteInfo = {
  url: string;
  host: string;
  /** `https://example.com`. Empty for pages that are not http or https. */
  origin: string;
  security: "secure" | "neutral" | "insecure" | "insecure-broken" | "unknown" | string;
  certificate: CertificateInfo | null;
  /** The certificate error for this host was passed this session. */
  certificateException: boolean;
  permissions: PermissionGrant[];
  /** Null when the page is not loaded. */
  cookies: number | null;
  /** Bytes in local storage, IndexedDB and caches. Null when unknown. */
  storageBytes: number | null;
  adsBlocked: number;
};

export type OmniboxState = {
  suggestions: OmniboxSuggestion[];
  selected: number;
};
