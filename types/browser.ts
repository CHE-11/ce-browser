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
  /** An agent, flow, or MCP client is driving this page. */
  automated: boolean;
};

/** One tab's place in a strip, in screen CSS pixels along the strip axis. */
export type TabStripSlot = {
  /** Strip index, the way `moveTab` counts it. */
  index: number;
  start: number;
  end: number;
};

/** Where a window's tab strip is on screen, so a dragged tab can land on it. */
export type TabStripLayout = {
  /** `x` for the top strip, `y` for the sidebar. */
  axis: "x" | "y";
  x: number;
  y: number;
  width: number;
  height: number;
  slots: TabStripSlot[];
};

/** Pointer position while a tab is dragged, in screen CSS pixels. `id` is null when the drag ends. */
export type TabDragHover = {
  id: number | null;
  x: number;
  y: number;
  /** Later events replace earlier ones. */
  token: number;
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

export type DevtoolsDock = {
  /** Share of the page area the pages keep. Either panel can shrink to 10% of the window width. */
  ratio: number;
  /** The column is on screen. Fullscreen keeps the dock open with this false. */
  visible: boolean;
};

export type PageViewport = {
  /** CSS pixels. The page view is this size. */
  width: number;
  height: number;
  /** Black cutout over the top of the page. */
  cutout: boolean;
  /** Home indicator along the bottom of the page. */
  homeBar: boolean;
  /** Installed standalone web app. The page extends under the status bar and home indicator. */
  installed: boolean;
  /** An iOS keyboard slides up over the page when a text field is focused, and its keys type into that field. */
  keyboard: boolean;
};

export type WindowTabs = {
  label: string;
  activeId: number | null;
  fullscreen: boolean;
  workspace: string;
  split: SplitView | null;
  /** Docked page inspector. Null until the user opens it. */
  devtools: DevtoolsDock | null;
  /** Address of the open peek, if any. */
  peekUrl: string | null;
  /** Fixed CSS size of the tab that has DevTools open. Null on every other tab. */
  viewport: PageViewport | null;
  /** The agent panel is docked beside the pages. */
  agentPanel: boolean;
  /** The automations panel is docked beside the pages. */
  automationPanel: boolean;
  /** Host extension id docked beside the pages. Empty when another panel is open or the column is closed. */
  hostPanel: string;
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

/** Live adapter throughput. Rates are bytes per second for the whole adapter. */
export type NetworkSpeed = {
  online: boolean;
  name: string;
  kind: "wifi" | "ethernet" | "other" | "offline";
  downBps: number;
  upBps: number;
  /** Negotiated link speed, bits per second. Zero when unknown. */
  linkBps: number;
  /** Wi-Fi signal 0–100. Null on a wired link. */
  signal: number | null;
  /** Round trip to the default gateway, milliseconds. Null until a probe lands. */
  rttMs: number | null;
  /** Round trip to 1.1.1.1, milliseconds. Null until a probe lands. */
  pingMs: number | null;
  /** Mean gap between consecutive public pings, milliseconds. Null until two land. */
  jitterMs: number | null;
  /** IPv4 of the connected adapter, or IPv6 when it has no IPv4. */
  ipAddress: string | null;
  /** Address a public server sees for this connection. */
  externalIp: string | null;
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
  /** Ctrl+scroll over the page does not zoom this site. Older files omit this. */
  pageZoomDomain: boolean;
  /** Full addresses on this site where Ctrl+scroll over the page does not zoom. */
  pageZoomUrls: string[];
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
  /** Ctrl+scroll over the page does not zoom this site. */
  pageZoomDomain?: boolean;
  /**
   * When set, `pageUrl` is added to or removed from the full-address list.
   * `true` turns page zoom off for that address.
   */
  pageZoomUrl?: boolean;
  /** Full page address used with `pageZoomUrl`. */
  pageUrl?: string;
};

export type PeekInfo = {
  window: string;
  url: string;
  title: string;
  favicon: string | null;
};

export type BrowserCommand =
  | "focus-address"
  | "new-tab"
  | "find"
  | "find-all"
  | "bookmark"
  | "downloads"
  | "palette"
  | "tab-search"
  | "shortcut-sheet"
  | "tab-switcher"
  | "tab-switcher-back"
  | "tab-overview";

export type PopupKind =
  | "downloads"
  | "history"
  | "bookmark-edit"
  | "bookmark-folder"
  | "bookmarks-bar-menu"
  | "bookmarks-bar-overflow"
  | "bookmark-add"
  | "bookmark-rename"
  | "tab-menu"
  | "address-bar-ellipsis-menu"
  | "extensions-menu"
  | "status"
  | "omnibox"
  | "command-palette"
  | "extension"
  | "host"
  | "print-pdf"
  | "page-capture"
  | "recording"
  | "webpage-markdown"
  | "media-hub"
  | "media-grid"
  | "what-runs"
  | "extract"
  | "ad-blocker"
  | "extension-pin"
  | "pin-menu"
  | "tab-group"
  | "tab-group-menu"
  | "workspaces"
  | "page-controls"
  | "network"
  | "network-speed"
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
  | "board-drop"
  | "theme-picker"
  | "style-picker"
  | "font-picker"
  | "profile-switcher"
  | "permission";

/** Windows app theme. Sites that support `prefers-color-scheme` follow this. */
export type WindowsColorScheme = "light" | "dark";

/** A theme shown over the saved one. `commit` stores it before the settings file catches up. */
export type ThemePreview = {
  theme: string | null;
  commit: boolean;
};

/** A UI font shown over the saved one. `commit` stores it before the settings file catches up. */
export type FontPreview = {
  font: string | null;
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
  /** Windows name of the computer that saved this file. Empty when an older list could not tell. */
  computerName: string;
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

export type HostExtensionPage = {
  id: string;
  title: string;
};

export type HostPanelSide = "left" | "right";

/**
 * Side and size of a host extension's docked panel. Widths are CSS pixels from the
 * manifest. `side` is the user's choice, or the manifest side when they have not chosen.
 */
export type HostPanelLayout = {
  side: HostPanelSide;
  resizable: boolean;
  minWidth: number;
  maxWidth: number;
  defaultWidth: number;
};

export type HostExtensionSurfaces = {
  toolbar: boolean;
  popup: boolean;
  panel: boolean;
  pages: HostExtensionPage[];
  bookmarksBar: boolean;
  downloads: boolean;
  contentScripts: boolean;
  background: boolean;
  popout: boolean;
  /** A strip the extension draws around the page. */
  pageToolbar: boolean;
};

/** Edge and CSS-pixel thickness of a host extension's page toolbar. */
export type HostPageToolbar = {
  side: ToolbarSide;
  thickness: number;
  /** False after the toolbar's close control hides it. The extension stays enabled. */
  open: boolean;
};

/** A folder of HTML, CSS, and JavaScript loaded by the browser, not a Chrome extension. */
export type HostExtensionInfo = {
  id: string;
  name: string;
  version: string;
  description: string;
  enabled: boolean;
  icon: string | null;
  permissions: string[];
  surfaces: HostExtensionSurfaces;
  /** Present when the extension has a side panel. */
  panel: HostPanelLayout | null;
  /** Present when the extension draws a toolbar around the page. */
  pageToolbar: HostPageToolbar | null;
};

/** How a store extension with a popup opens from its toolbar button. */
export type ExtensionOpenAs = "popover" | "panel";

/** A manifest command that names a suggested shortcut. */
export type ExtensionCommand = {
  name: string;
  description: string;
  /** Suggested shortcut, such as `Ctrl+Shift+Y`. */
  shortcut: string;
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
  /** Toolbar tooltip from `action.default_title`. Empty uses the extension name. */
  defaultTitle: string;
  /** Side panel document. Edge's legacy sidebar page is used when `side_panel` is absent. */
  sidePanel: string | null;
  /** New tab page this package supplies. The most recently enabled one is opened. */
  newTab: string | null;
  /** Commands that name a suggested shortcut. WebView2 does not deliver them. */
  commands: ExtensionCommand[];
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

/** How one file type opens inside the browser. */
export type FileHandler = {
  /** Lowercase extension without a dot, such as `pdf`. */
  extension: string;
  /** Page opened in a tab. The file is placed on that page's file input. Empty when `native` is set. */
  url: string;
  /** Element id of the file input. Empty uses the first file input on the page. */
  inputId: string;
  /** Profile id that opens this type. Empty uses an open profile, or the default profile. */
  profile: string;
  /** Opens the file in Edge's PDF viewer. Only pdf uses this. */
  native: boolean;
};

/** Edge of the window where the tools toolbar sits. */
export type ToolbarSide = "top" | "right" | "bottom" | "left";

/** Where folders and bookmarks sit on the bookmarks bar. */
export type BookmarksBarAlign = "left" | "center" | "right";

/** Where a tool or extension is shown. Both can be set. Both clear means unpinned. */
export type ToolbarPlacement = {
  /** Shown next to the address bar. */
  primary: boolean;
  /** Shown on the tools toolbar. */
  secondary: boolean;
};

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
  /** Where folders and bookmarks sit on the bookmarks bar. */
  bookmarksBarAlign: BookmarksBarAlign;
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
  /** Favicon and folder icon on the bookmarks bar, in rem. */
  bookmarksBarIconSize: number;
  /** Minimum height of a bookmarks bar item, in rem. */
  bookmarksBarHeight: number;
  /** Icon in a bookmark folder menu, in rem. */
  bookmarkFolderIconSize: number;
  /** Icon in the bookmarks bar context menu, in rem. */
  bookmarksBarMenuIconSize: number;
  /** Width of the bookmarks bar context menu, in CSS pixels at 100% UI size. */
  bookmarksBarMenuWidth: number;
  /** Text size of the bookmark editor, in rem. */
  bookmarkEditorTextSize: number;
  /** Padding around the bookmark editor, in rem. */
  bookmarkEditorPadding: number;
  /** Side padding of a row in the bookmark editor, in rem. Top and bottom get half. */
  bookmarkEditorRowPadding: number;
  /** Favicon in the bookmark editor's place list, in rem. */
  bookmarkEditorIconSize: number;
  /** Height of the place list in the bookmark editor, in rem. */
  bookmarkEditorListHeight: number;
  /** Width of the bookmark editor, in CSS pixels at 100% UI size. */
  bookmarkEditorWidth: number;
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
  /** Last choices in the Recording popover. */
  recording: RecordingSettings;
  /** Last choices in the Webpage to Markdown popover. */
  pageMarkdown: PageMarkdownSettings;
  /** Extension ids left off the toolbars. Empty means every enabled extension stays pinned. */
  hiddenToolbarExtensions: string[];
  /** Explicit pins. A missing id keeps the old default: built-in tools on the tools toolbar, extensions on the address bar. */
  toolbarPlacements: Record<string, ToolbarPlacement>;
  /**
   * How a store extension opens. A missing id opens as a popover.
   * An older file omits this map.
   */
  extensionOpenAs: Record<string, ExtensionOpenAs>;
  /**
   * Which edge a store extension's panel uses. A missing id docks on the right.
   * An older file omits this map.
   */
  extensionPanelSide: Record<string, HostPanelSide>;
  /**
   * CSS pixels a store extension's panel opens at. A missing id uses 380.
   * An older file omits this map.
   */
  extensionPanelWidth: Record<string, number>;
  /**
   * `false` turns off dragging a store extension's panel. A missing id stays resizable.
   * An older file omits this map.
   */
  extensionPanelResizable: Record<string, boolean>;
  /** Order of pinned toolbar icons. Empty keeps the built-in order, then extensions. */
  toolbarOrder: string[];
  /** Built-in tool ids the user turned off. Empty means every built-in tool stays on. */
  disabledBuiltinTools: string[];
  /**
   * True after dev-tool defaults have been applied.
   * An older file omits this. A dev build leaves Debug HUD on. A release build turns it off until it is enabled.
   */
  devToolsSettled: boolean;
  /** Host extension id that replaces the bookmarks bar. Empty keeps the built-in bar. */
  hostBookmarksBar: string;
  /** Host extension id that replaces downloads. Empty keeps the built-in downloads UI. */
  hostDownloads: string;
  /** Built-in tools sit on the right. Off lays that toolbar out above the bookmarks bar. */
  verticalToolbar: boolean;
  /** Edge where the tools toolbar sits. Empty until an older file is read, which uses `verticalToolbar`. */
  toolbarSide: ToolbarSide | "";
  /** Retired. Pinned extensions stay next to the address bar. */
  verticalToolbarExtensions: boolean;
  /** Short names under icons on the tools toolbar. */
  verticalToolbarLabels: boolean;
  /** The tools toolbar is visible. Ctrl+Shift+T toggles it. */
  showToolsToolbar: boolean;
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
  /** Text size of a tab title, in rem. */
  tabItemTextSize: number;
  /** Side padding of a tab, in rem. Top and bottom get half. */
  tabItemPadding: number;
  /** Text size of the tab context menu, in rem. */
  tabMenuTextSize: number;
  /** Side padding of rows in the tab context menu, in rem. Top and bottom get half. */
  tabMenuPadding: number;
  /** Minimum height of a horizontal tab, in rem. */
  tabItemHeight: number;
  /** Favicon on a tab, in rem. The close icon is 0.875 times this. */
  tabIconSize: number;
  /** Icon in the tab context menu, in rem. */
  tabMenuIconSize: number;
  /** Minimum height of a tab context menu row, in rem. Zero keeps the height of the text and padding. */
  tabMenuHeight: number;
  /** Width of the tab context menu, in CSS pixels at 100% UI size. */
  tabMenuWidth: number;
  /** Text size of shared menu rows, in rem. */
  menuTextSize: number;
  /** Side padding of shared menu rows, in rem. */
  menuPadding: number;
  /** Height of a shared menu row, in rem. */
  menuHeight: number;
  /** Icon in a shared menu row, in rem. */
  menuIconSize: number;
  /** Height of the address bar, in rem. */
  toolbarHeight: number;
  /** Address-bar and side-strip buttons, in rem. */
  toolbarButtonSize: number;
  /** Icons in those buttons, in rem. */
  toolbarIconSize: number;
  /** Height of the address field, in rem. */
  addressHeight: number;
  /** Text in the address field, in rem. */
  addressTextSize: number;
  /** Height of an omnibox suggestion, in rem. */
  omniboxRowHeight: number;
  /** Title in an omnibox suggestion, in rem. */
  omniboxTextSize: number;
  /** Icon in an omnibox suggestion, in rem. */
  omniboxIconSize: number;
  /** Address beside an omnibox title, in rem. */
  omniboxUrlSize: number;
  /** Width of the downloads panel, in CSS pixels at 100% UI size. */
  downloadsPanelWidth: number;
  /** File name in the downloads panel, in rem. */
  downloadsTextSize: number;
  /** Side padding of a downloads panel row, in rem. */
  downloadsPadding: number;
  /** Top and bottom padding of a downloads panel row, in rem. */
  downloadsRowPadding: number;
  /** File icon in the downloads panel, in rem. */
  downloadsIconSize: number;
  /** Width of the history popover, in CSS pixels at 100% UI size. */
  historyPanelWidth: number;
  /** Title in the history popover, in rem. */
  historyTextSize: number;
  /** Host and time in the history popover, in rem. */
  historyMetaSize: number;
  /** Side padding of a history popover row, in rem. */
  historyPadding: number;
  /** Top and bottom padding of a history popover row, in rem. */
  historyRowPadding: number;
  /** Favicon in the history popover, in rem. */
  historyIconSize: number;
  /** Width of the command palette, in CSS pixels at 100% UI size. */
  commandPaletteWidth: number;
  /** Height of the command palette search field, in rem. */
  commandPaletteInputHeight: number;
  /** Height of a command palette row, in rem. */
  commandPaletteRowHeight: number;
  /** Title in a command palette row, in rem. */
  commandPaletteTextSize: number;
  /** Hint in a command palette row, in rem. */
  commandPaletteHintSize: number;
  /** Corner or edge of the page area where toasts appear. */
  toastPosition: ToastPosition;
  /** Opens these file types in a browser tab. */
  fileHandlers: FileHandler[];
  /** Width of the docked side column, in CSS pixels. The agent panel opens no wider than 720. The automations panel may be saved wider, up to half the page. */
  agentPanelWidth: number;
  /** Prompts offered in the agent panel. */
  agentSuggestions: string[];
  /** Hosts the agent will not read, screenshot, or answer about. Also blocked for automation. */
  aiBlockedSites: string[];
  /** Agent, recorded flows, schedules, and the MCP server. */
  automation: AutomationSettings;
};

export type AutomationSettings = {
  enabled: boolean;
  /** Model for the agent. Blank uses the AI provider's model. */
  agentModel: string;
  /** Actions one agent run may take before it stops. */
  maxSteps: number;
  /** Ask before purchases, sends, deletes, uploads, and similar actions. */
  confirmSensitive: boolean;
  /** When not empty, automation may only open these hosts without asking. */
  allowedSites: string[];
  /** Hosts automation never touches. */
  blockedSites: string[];
  /** Send a screenshot with each step. Off sends the text snapshot only. */
  screenshotsToModel: boolean;
  mcpEnabled: boolean;
  mcpPort: number;
  /** MCP clients may read pages but not click, type, or navigate. */
  mcpReadOnly: boolean;
  /** MCP clients may run scripts in pages. */
  mcpAllowEvaluate: boolean;
  /** Scheduled flows and monitors run. */
  schedulerEnabled: boolean;
  /** Scheduled runs wait while the PC is on battery. */
  pauseOnBattery: boolean;
  /** Autonomy new agent runs start with. */
  defaultAutonomy: AgentAutonomy;
  /** Helper agents one run may have working at once, 0 to 6. Zero turns helpers off. */
  maxParallelAgents: number;
  /** The agent keeps notes across runs and reads them back. */
  agentMemory: boolean;
  /** Windows notification when a background run finishes or needs you. */
  agentNotify: boolean;
  /** The agent may run JavaScript in pages. */
  agentAllowScript: boolean;
  /** Show the model's reply as it is written. */
  streamReplies: boolean;
  /** The agent may use the mouse and keyboard outside the browser. */
  desktopControl: boolean;
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

export type NetworkHeader = {
  name: string;
  value: string;
};

export type NetworkEntry = {
  id: number;
  method: string;
  url: string;
  kind: string;
  status: number | null;
  /** Unix milliseconds when the response arrived. */
  time: number;
  /** Milliseconds from the request start until the response, when the start was seen. */
  durationMs: number | null;
  requestHeaders: NetworkHeader[];
  responseHeaders: NetworkHeader[];
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
export type PrintFont =
  | "original"
  | "serif"
  | "georgia"
  | "cambria"
  | "constantia"
  | "palatino"
  | "times"
  | "sans"
  | "segoe"
  | "calibri"
  | "aptos"
  | "arial"
  | "verdana"
  | "candara"
  | "trebuchet"
  | "consolas"
  | "cascadia"
  | "courier";
export type PaperSize =
  | "letter"
  | "legal"
  | "tabloid"
  | "statement"
  | "executive"
  | "folio"
  | "a3"
  | "a4"
  | "a5"
  | "a6"
  | "b5"
  | "photo4x6"
  | "photo5x7"
  | "photo8x10"
  | "iphone16"
  | "iphone16Plus"
  | "iphone16Pro"
  | "iphone16ProMax"
  | "iphoneSe"
  | "pixel9"
  | "pixel9ProXl"
  | "galaxyS24"
  | "galaxyS24Ultra"
  | "ipadMini"
  | "ipad"
  | "ipadAir13"
  | "ipadPro11"
  | "ipadPro13"
  | "galaxyTabS9";
/** A catalog size, or the one custom size stored beside it. */
export type PrintPaper = PaperSize | "custom";
export type PrintMargins = "none" | "narrow" | "normal" | "wide";
/** Where `2/12` sits along the bottom of each page. */
export type PrintPageNumberLocation = "left" | "center" | "right";

/** A named recipe for Quick clean PDF. */
export type PrintPreset = {
  id: string;
  name: string;
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
  paper: PrintPaper;
  /** Custom page width in inches when `paper` is `custom`. `0` means it is not set. */
  customWidth: number;
  /** Custom page height in inches when `paper` is `custom`. `0` means it is not set. */
  customHeight: number;
  landscape: boolean;
  margins: PrintMargins;
  /** WebView2's own header and footer on every page. */
  headerFooter: boolean;
  /** `current/total`, such as `2/12`, along the bottom of every page. */
  showPageNumbers: boolean;
  /** Where the page number sits. Used when `showPageNumbers` is on. */
  pageNumberLocation: PrintPageNumberLocation;
  printBackgrounds: boolean;
  /**
   * Suggested PDF file name. Placeholders such as `{title}`, `{author}`, and `{article}` fill in from the page.
   * Empty uses the page title.
   */
  fileNameTemplate: string;
};

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
  paper: PrintPaper;
  /** Sizes listed in the print editor. Empty means Letter, Tabloid, and Legal. */
  paperChoices: string[];
  /** Custom page width in inches. `0` means it is not set. */
  customWidth: number;
  /** Custom page height in inches. `0` means it is not set. */
  customHeight: number;
  landscape: boolean;
  margins: PrintMargins;
  /** WebView2's own header and footer on every page. */
  headerFooter: boolean;
  /** `current/total`, such as `2/12`, along the bottom of every page. */
  showPageNumbers: boolean;
  /** Where the page number sits. Used when `showPageNumbers` is on. */
  pageNumberLocation: PrintPageNumberLocation;
  printBackgrounds: boolean;
  openAfterSave: boolean;
  /** Named recipes for Quick clean PDF. Empty uses the editor's last choices. */
  presets: PrintPreset[];
};

export type PageCaptureSettings = {
  /** The whole document, not only what is on screen. */
  fullPage: boolean;
  /** Scroll through the page first so lazy images load. */
  loadLazy: boolean;
};

/** Last choices in the Recording popover. An older settings file omits this. */
export type RecordingResolution = "720" | "1080" | "1440" | "native";

export type RecordingCameraCorner = "bottom-left" | "bottom-right" | "top-left" | "top-right";

/** `system` keeps the pointer Windows draws. `hidden` leaves it out. `circle` draws a ring instead. */
export type RecordingCursor = "system" | "hidden" | "circle";

export type RecordingSettings = {
  microphone: boolean;
  systemAudio: boolean;
  camera: boolean;
  countdown: boolean;
  /** Long side of the picture: 720p, 1080p, 1440p, or the screen's own size. */
  resolution: RecordingResolution;
  /** Where the camera bubble sits on the recording. */
  cameraCorner: RecordingCameraCorner;
  /** How the pointer is drawn into the video. */
  cursor: RecordingCursor;
};

/** `article` keeps the main content. `page` keeps the whole document. */
export type MarkdownScope = "article" | "page";

export type PageMarkdownSettings = {
  scope: MarkdownScope;
  images: boolean;
  links: boolean;
  /** Title and source URL above the content. */
  header: boolean;
};

export type RecordingSource = "tab" | "browser" | "window" | "display";

export type RecordingTargetInfo = {
  id: string;
  kind: string;
  label: string;
};

export type CameraLayout = {
  /** Left edge, as a fraction of the frame width. */
  x: number;
  /** Top edge, as a fraction of the frame height. */
  y: number;
  /** Bubble width, as a fraction of the frame width. */
  size: number;
};

export type ScreenRecordingStatus = {
  phase: "idle" | "countdown" | "recording" | "paused" | "error" | string;
  elapsedMs: number;
  countdown: number;
  id: string;
  error: string | null;
  path: string | null;
  camera: boolean;
  microphone: boolean;
  micMuted: boolean;
  /** Microphone loudness from 0 to 1. */
  micLevel: number;
  drawing: boolean;
  strokes: number;
  tool: string;
  color: string;
  ownerWindow: string;
};

/** What Present is doing. Marks stay in the overlay; a lock is the rectangle it holds. */
export type PresentTool = "none" | "lasso" | "spotlight" | "cursor" | "zoom" | "highlight" | "pen" | "arrow" | "text" | "eraser";

/** Drawn on top of the window. `off` leaves the system pointer alone. */
export type PresentCursor = "dot" | "ring" | "arrow" | "crosshair" | "off";

/** A rectangle as fractions of the browser window. */
export type PresentRect = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export type PresentState = {
  on: boolean;
  tool: PresentTool | string;
  cursor: PresentCursor | string;
  color: string;
  /** Stroke scale. 1 is the medium pen. */
  width: number;
  /** Spotlight shade, from 0 (clear) to 1 (solid). */
  dim: number;
  /** Highlight corner radius in CSS pixels. */
  radius: number;
  /** Cursor-tool circle diameter in CSS pixels. */
  spread: number;
  /** Large circle around the pointer while the cursor tool is selected. */
  circle: boolean;
  /** The next drag locks the spotlight or zoom. */
  placing: boolean;
  /** Bumps when the palette asks the stage to drop every mark. */
  clear: number;
  /** Bumps when the palette asks the stage to drop the last mark. */
  undo: number;
  spotlight: PresentRect | null;
  zoom: PresentRect | null;
};

/** Cursor position in CSS pixels of the present stage. */
export type PresentPointer = {
  x: number;
  y: number;
  inside: boolean;
  /** Zoom glass, as fractions of the window. Null unless the zoom tool is showing it. */
  lens: PresentRect | null;
};

export type ScreenRecording = {
  id: string;
  title: string;
  createdAt: number;
  durationMs: number;
  source: string;
  sourceLabel: string;
  width: number;
  height: number;
  videoPath: string;
  cameraPath: string;
  camera: CameraLayout;
  trimStartMs: number;
  trimEndMs: number;
  hasMic: boolean;
  hasSystemAudio: boolean;
};

export type RecordingStroke = {
  tool: "pen" | "arrow" | "rectangle" | "highlighter" | string;
  color: string;
  width: number;
  points: { x: number; y: number }[];
  draft?: boolean;
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
      /** Named quick-clean recipe. Empty uses the editor's last choices. */
      presetId: string;
      window: string;
      tabId: number;
      title: string;
      url: string;
    }
  | {
      tool: "webpage-markdown";
      action: "download" | "copy";
      window: string;
      tabId: number;
      title: string;
      url: string;
      scope: MarkdownScope;
      images: boolean;
      links: boolean;
      header: boolean;
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
  | "platform"
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
  /** `current/total`, such as `2/12`, along the bottom of every page. */
  pageNumbers: boolean;
  /** Where the page number sits. Used when `pageNumbers` is on. */
  pageNumberLocation: PrintPageNumberLocation;
  /** Text size percent. Page numbers are 85% of the body size this scales. */
  textSize: number;
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
  /** The Default profile chooses the folder. Other profiles follow it. */
  editable: boolean;
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
  | "automation"
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
  /** Background color id. Empty keeps the icon's own color. */
  iconColor: string;
};

export type ProfileIconChoice = {
  id: string;
  label: string;
  /** PNG data URL. */
  icon: string;
  /** Background colors apply. Browser marks keep their own colors. */
  colorable: boolean;
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

/** One match for the results panel. */
export type FindHit = {
  index: number;
  before: string;
  text: string;
  after: string;
  /** A heading, a line in an editor, or how far down the page the match is. */
  location: string;
};

/** Text matches on one page. `index` is the highlighted match, or -1. */
export type FindResult = {
  count: number;
  index: number;
  /** Filled when the results panel is open. */
  hits?: FindHit[];
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

// Automation

export type AgentStepKind = "think" | "action" | "tool" | "approved" | "blocked" | "error" | "user" | "answer";

/** `ask` waits for approval on sensitive actions. `auto` lets sends and new sites go ahead. `full` asks only before payments and uploads. */
export type AgentAutonomy = "ask" | "auto" | "full";

export type AgentStep = {
  kind: AgentStepKind;
  text: string;
  detail: string;
  /** Small JPEG data URL of the page after this step. */
  thumb: string | null;
  /** Milliseconds since 1970. */
  at: number;
  /** Pointer, as fractions of the viewport. Null when this step didn't point. */
  px: number | null;
  py: number | null;
  px2: number | null;
  py2: number | null;
};

/** A dialog, download, or new tab the panel shows during a run. */
export type AgentNotice = {
  kind: string;
  text: string;
  detail: string;
  tabId: number;
};

export type AgentApproval = {
  id: number;
  /** `confirm` asks allow or deny. `question` asks for a typed answer. */
  kind: "confirm" | "question";
  prompt: string;
  /** What a `confirm` could do. Blank for questions. */
  risk: "" | "open" | "send" | "destructive" | "payment" | "upload" | "script" | "budget" | "desktop";
};

export type AgentRunStatus = "running" | "waiting" | "paused" | "done" | "failed" | "stopped";

export type AgentPlanItem = {
  text: string;
  status: "pending" | "active" | "done" | "skipped";
};

export type AgentRun = {
  id: number;
  /** `agent` for runs started from the panel, `mcp` for actions from an MCP client. */
  source: "agent" | "mcp";
  tabId: number;
  window: string;
  goal: string;
  status: AgentRunStatus;
  /** Empty for older finished runs in a list; `agentRun` loads them. */
  steps: AgentStep[];
  stepCount: number;
  approval: AgentApproval | null;
  result: string;
  started: number;
  /** Zero while the run is active. */
  finished: number;
  actions: number;
  maxSteps: number;
  autonomy: AgentAutonomy;
  /** Model override. Blank uses the agent model from Settings. */
  model: string;
  tokensIn: number;
  tokensOut: number;
  /** Tokens before the run asks to keep going. Zero has no limit. */
  budget: number;
  plan: AgentPlanItem[];
  /** The run that started this one as a helper. */
  parent: number | null;
  background: boolean;
  /** Page actions that can be saved as a flow. */
  flowSteps: number;
  startUrl: string;
  access: AgentAccess;
  /** A JavaScript dialog waiting on this run's tab. */
  dialog: AgentNotice | null;
  /** The download the last page action started. */
  download: AgentNotice | null;
  /** The tab or window the last page action opened. */
  openedTab: AgentNotice | null;
  /** Latest desktop screenshot, as a small JPEG data URL. */
  screen: string | null;
};

/** What a run may reach. Tools outside it are not offered to the model. */
export type AgentAccess = {
  /** `site` keeps the run on one site. `browser` lets it go anywhere automation may. */
  sites: "site" | "browser";
  /** The site a `site` run is held to. Blank fills in from the first web page. */
  site: string;
  /** Open, switch, read and close other tabs. */
  tabs: boolean;
  /** History, bookmarks, downloads, boards and saved flows. */
  browserData: boolean;
  /** Save files to Downloads and upload local files to pages. */
  files: boolean;
  clipboard: boolean;
  memory: boolean;
  helpers: boolean;
  /** Run JavaScript in pages, when Settings also allows it. */
  scripts: boolean;
  notifications: boolean;
  /** Use the mouse and keyboard outside the browser, when Settings also allows it. */
  desktop: boolean;
};

/** US dollars per million tokens. */
export type ModelPrice = {
  input: number;
  output: number;
  /** Runs on this computer, so it costs nothing per token. */
  local: boolean;
};

export type AgentStartOptions = {
  autonomy?: AgentAutonomy;
  model?: string;
  background?: boolean;
  /** Other tabs whose text the agent reads before it starts. */
  contextTabs?: number[];
  budget?: number;
  /** Guidance for the model, such as from a slash command. Not shown as part of the goal. */
  instructions?: string;
  access?: AgentAccess;
};

/** The model's reply as it is written. Empty text clears it. */
export type AgentDelta = {
  runId: number;
  text: string;
  thinking: string;
};

/** A note the agent keeps across runs. */
export type AgentMemoryItem = {
  id: string;
  text: string;
  /** Host the note is about. Blank applies everywhere. */
  site: string;
  /** Shown to the agent in every run. */
  pinned: boolean;
  created: number;
  used: number;
};

export type McpStatus = {
  enabled: boolean;
  running: boolean;
  port: number;
  /** Address MCP clients connect to. */
  url: string;
  /** Bearer token clients send. Stored with DPAPI on this computer. */
  token: string;
  error: string | null;
  /** `mcp.json` entry for Cursor and other MCP clients. */
  config: string;
  /** Tool calls served since the browser started. */
  calls: number;
};

/** How a recorded step finds its element again. Each field is tried until one matches a single element. */
export type FlowTarget = {
  role: string;
  name: string;
  text: string;
  testId: string;
  css: string;
  xpath: string;
  tag: string;
  /** Plain description for AI healing, such as `the Add to cart button`. */
  description: string;
  /** Origin of the frame that holds the element. Empty is the top page. */
  frame: string;
};

export type FlowStepKind =
  | "navigate"
  | "click"
  | "type"
  | "press"
  | "select"
  | "scroll"
  | "hover"
  | "wait"
  | "waitFor"
  | "back"
  | "extract"
  | "ai"
  | "assert"
  | "notify"
  | "upload"
  | "drag"
  | "dialog"
  | "desktopClick"
  | "desktopDrag"
  | "desktopType";

export type FlowStep = {
  id: string;
  /** Unknown kinds from newer builds are kept and skipped. */
  kind: FlowStepKind | (string & {});
  target: FlowTarget | null;
  url: string;
  /** Text to type, text to wait for, or the notification body. `{{var}}` and `{{secret:name}}` are filled in. */
  text: string;
  keys: string;
  values: string[];
  seconds: number;
  /** Instruction for `ai` and `extract` steps. */
  prompt: string;
  /** Saved selector for `extract` steps. */
  spec: ExtractSpec | null;
  /** Pages an `extract` step follows through the list's Next link. */
  pages: number;
  /** Keep going when this step fails. */
  optional: boolean;
  /** Skipped by runs and left out of exports, without deleting the step. */
  disabled: boolean;
  note: string;
  /** How much an `ai` step's agent may do without asking. Blank uses the default from Settings. */
  autonomy?: AgentAutonomy | "";
  /** 2 is a double-click. 0 and 1 are one click. */
  clicks: number;
  /** CSS pixels for a click or drag that has no element. Screen pixels for a desktop step. */
  x: number | null;
  y: number | null;
  /** Where a drag ends, when it ends on an element. */
  to: FlowTarget | null;
  toX: number | null;
  toY: number | null;
  /** After this click, continue the flow in the tab it opened. */
  follow: boolean;
};

export type FlowTriggerKind = "interval" | "daily" | "weekly" | "startup" | "visit" | "pageChange" | "download" | "hotkey";

export type FlowTrigger = {
  kind: FlowTriggerKind | (string & {});
  enabled: boolean;
  /** Minutes between runs for `interval` and `pageChange`. */
  minutes: number;
  /** Local time `HH:MM` for `daily` and `weekly`. */
  time: string;
  /** Days for `weekly`, 0 is Sunday. */
  days: number[];
  /** Address pattern for `visit` and file pattern for `download`. `*` matches anything. */
  pattern: string;
  /** Shortcut slot 1 to 9 for `hotkey`. */
  slot: number;
};

export type Flow = {
  id: string;
  name: string;
  description: string;
  startUrl: string;
  steps: FlowStep[];
  triggers: FlowTrigger[];
  /** Variables available as `{{name}}` in step text. */
  variables: Record<string, string>;
  /** Run in a hidden tab instead of the active tab. */
  background: boolean;
  /** Show a Windows notification with the result. */
  notify: boolean;
  /** For monitors: notify when the AI says this is true of the page. Blank notifies on any change. */
  monitorPrompt: string;
  /** Triggers don't fire while paused. Starting it by hand still works. */
  paused: boolean;
  lastRun: number;
  lastStatus: string;
  created: number;
  updated: number;
};

export type FlowRunStepStatus = "ok" | "healed" | "failed" | "skipped";

export type FlowRunStep = {
  index: number;
  kind: string;
  label: string;
  status: FlowRunStepStatus;
  detail: string;
  /** A screenshot was saved when this step failed. Read it with `flowRunShot`. */
  shot: boolean;
};

export type FlowRunOptions = {
  /** First step to run. 0 runs the whole flow. */
  fromStep?: number;
  /** Last step to run. Leave out to run to the end. */
  toStep?: number | null;
  /** Stay on the page open in the chosen tab instead of loading the start address. */
  here?: boolean;
};

export type FlowTargetTest = {
  found: boolean;
  /** The locator that matched: `testId`, `role`, `css`, `text` or `xpath`. */
  by: string;
};

export type FlowRun = {
  id: number;
  flowId: string;
  flowName: string;
  /** What started the run, such as `manual`, `daily` or `visit`. */
  trigger: string;
  status: "running" | "done" | "failed" | "stopped";
  started: number;
  finished: number;
  steps: FlowRunStep[];
  output: string;
  /** Dataset written by the run's extract steps. */
  datasetId: string;
  /** Index of the step running now. */
  current: number | null;
  currentLabel: string;
  /** First step the run started at. 0 is the whole flow. */
  fromStep: number;
  /** Last step the run stops after. `null` runs to the end. */
  toStep: number | null;
};

export type FlowRecording = {
  tabId: number;
  steps: FlowStep[];
  started: number;
  /** Flow the steps are added to when saved. Empty saves a new flow. */
  flowId: string;
};

export type DatasetInfo = {
  id: string;
  name: string;
  /** Page the rows came from. */
  source: string;
  columns: string[];
  rows: number;
  created: number;
  updated: number;
};

export type Dataset = DatasetInfo & {
  data: Record<string, string>[];
};

export type AutomationsSnapshot = {
  flows: Flow[];
  runs: FlowRun[];
  recording: FlowRecording | null;
  datasets: DatasetInfo[];
  /** When each flow's triggers next fire, by flow id. Missing for paused and unscheduled flows. */
  nextRuns: Record<string, number>;
  /** Why no trigger fires right now. Empty while the scheduler runs. */
  schedulerPaused: string;
};

export type ExtractField = {
  name: string;
  /** CSS selector inside each item. Empty uses the item itself. */
  selector: string;
  /** `text`, `href`, `src`, `value`, or an attribute name. */
  attr: string;
};

export type ExtractSpec = {
  /** CSS selector that matches every item in the list. */
  item: string;
  fields: ExtractField[];
};

export type ExtractPick = {
  spec: ExtractSpec;
  preview: Record<string, string>[];
};

export type ExtractOptions = {
  name: string;
  /** Pages to follow through the list's Next link. 1 reads only this page. */
  pages: number;
  /** Text of the Next link. Blank finds it automatically. */
  nextText: string;
};

export type SniffedEndpoint = {
  url: string;
  /** `fetch` or `xmlhttprequest`. */
  kind: string;
};

export type DatasetFormat = "csv" | "json" | "tsv";

// Debug HUD (dev builds only)

export type DebugIpcKind = "invoke" | "event";

export type DebugIpcRecord = {
  /** Given by the backend. 0 until it is recorded. */
  id: number;
  /** Label of the webview that made the call or received the event. */
  webview: string;
  kind: DebugIpcKind;
  /** Command or event name. */
  name: string;
  /** Unix ms. */
  startedAt: number;
  /** `null` for events. */
  durationMs: number | null;
  ok: boolean;
  argsPreview: string;
  resultPreview: string;
  error: string;
  /** Size of the result or event payload as JSON. */
  bytes: number;
};

/** `console` lines are console errors and warnings from frontend webviews. */
export type DebugLogStream = "stdout" | "stderr" | "console";

export type DebugConsoleLevel = "error" | "warn" | "uncaught" | "rejection";

/** A console message or uncaught error from a frontend webview, before the backend logs it. */
export type DebugConsoleEntry = {
  level: DebugConsoleLevel;
  text: string;
  /** Unix ms. */
  at: number;
  /** `file:line:column` when the browser gave one. Empty otherwise. */
  source: string;
};

export type DebugLogLine = {
  id: number;
  /** Unix ms. */
  at: number;
  stream: DebugLogStream;
  /** The `area` in `eprintln!("area: message")`. Empty when the line has none. */
  area: string;
  text: string;
};

export type DebugSnapshot = {
  ipc: DebugIpcRecord[];
  logs: DebugLogLine[];
};

export type DebugFeedTarget = "ipc" | "logs";

export type DebugFeed =
  | { kind: "ipc"; records: DebugIpcRecord[] }
  | { kind: "log"; lines: DebugLogLine[] }
  | { kind: "cleared"; target: DebugFeedTarget };

export type DebugWebviewKind = "ui" | "tab" | "popup" | "extpopup" | "devtools" | "agent" | "peek" | "script" | "debug" | "other";

export type DebugWebview = {
  label: string;
  /** Label of the native window that hosts it. */
  window: string;
  kind: DebugWebviewKind;
  tabId: number | null;
  url: string;
  /** Physical px inside the host window. */
  x: number;
  y: number;
  width: number;
  height: number;
  /** Physical px on the virtual screen. */
  screenX: number;
  screenY: number;
  /** Moved off screen or outside its window, such as a background tab or an idle popup. */
  parked: boolean;
  windowVisible: boolean;
  /** The debug outline is drawn around it now. */
  outlined: boolean;
};

export type DebugStoreFile = {
  name: string;
  path: string;
  bytes: number;
  /** Unix ms. 0 when the file system does not say. */
  modified: number;
};

export type DebugPaths = {
  profile: string;
  rootDataDir: string;
  dataRoot: string;
  dataDir: string;
  profileDir: string;
  /** Files directly inside `dataDir`, such as `settings.json` and `session.json`. */
  files: DebugStoreFile[];
};
