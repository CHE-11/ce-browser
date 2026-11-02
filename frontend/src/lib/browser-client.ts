import { invoke } from "@tauri-apps/api/core";
import { emit, emitTo, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open, save } from "@tauri-apps/plugin-dialog";
import type {
  AdblockStats,
  BackdropSupport,
  Board,
  BoardFolder,
  BoardsSnapshot,
  Pin,
  BookmarkNode,
  ExternalBrowser,
  RewriteRule,
  ProductPrice,
  ReadingStats,
  ShoppingDeals,
  SiteCookie,
  StoragePair,
  BrowseResult,
  BookmarkTree,
  BrowserCommand,
  BrowserProfile,
  BrowserSettings,
  ClosedEntry,
  ProfileIconChoice,
  ReleaseNotes,
  UpdateStatus,
  DownloadItem,
  ExtractedPage,
  FindResult,
  FindTabResult,
  HistoryItem,
  Keyword,
  ShortcutBinding,
  PageCaptureResult,
  PageMedia,
  MediaElement,
  RecordingStatus,
  PageRules,
  PageTechnologies,
  PageToolRequest,
  SiteFavicon,
  SiteInfo,
  SitePatch,
  UserScript,
  PdfOptions,
  SyncStatus,
  SyncConflict,
  ImportedProfile,
  HistoryHit,
  BookmarkHealthReport,
  BookmarkHealthProgress,
  StatusSnapshot,
  StatusText,
  ExtensionInfo,
  OmniboxState,
  Container,
  NetworkEntry,
  OmniboxSuggestion,
  PeekInfo,
  PermissionGrant,
  PopupChange,
  PopupRect,
  PopupRequest,
  TabGroupColor,
  TabsSnapshot,
  UserStyles,
} from "../../../types/browser";

// Tabs

export function listTabs() {
  return invoke<TabsSnapshot>("list_tabs");
}

export function createTab(url?: string, background = false, index?: number) {
  return invoke<number>("create_tab", { url, background, index });
}

export function closeTab(id: number) {
  return invoke<void>("close_tab", { id });
}

export function closeSelectedTabs() {
  return invoke<void>("close_selected_tabs");
}

export function toggleTabSelection(id: number) {
  return invoke<void>("toggle_tab_selection", { id });
}

export function selectTabRange(id: number) {
  return invoke<void>("select_tab_range", { id });
}

export function closeOtherTabs(id: number) {
  return invoke<void>("close_other_tabs", { id });
}

export function closeTabsToRight(id: number) {
  return invoke<void>("close_tabs_to_right", { id });
}

export function activateTab(id: number) {
  return invoke<void>("activate_tab", { id });
}

export function navigate(id: number, input: string) {
  return invoke<void>("navigate", { id, input });
}

export function goBack(id: number) {
  return invoke<void>("go_back", { id });
}

export function goForward(id: number) {
  return invoke<void>("go_forward", { id });
}

export function reload(id: number) {
  return invoke<void>("reload", { id });
}

export function clearSiteCache(id: number) {
  return invoke<void>("clear_site_cache", { id });
}

export function getAdblockStats(id: number) {
  return invoke<AdblockStats>("get_adblock_stats", { id });
}

export function onAdblockStats(handler: (stats: AdblockStats) => void): Promise<UnlistenFn> {
  return listen<AdblockStats>("adblock-stats", (event) => handler(event.payload));
}

export function stop(id: number) {
  return invoke<void>("stop", { id });
}

/** Ends the process of a page that stopped responding. The tab then offers Reload. */
export function endPage(id: number) {
  return invoke<void>("end_page", { id });
}

/** Loads the tab's failed address again, passing its certificate error for this session. */
export function allowCertificate(id: number) {
  return invoke<void>("allow_certificate", { id });
}

export function moveTab(id: number, index: number) {
  return invoke<void>("move_tab", { id, index });
}

export function detachTab(id: number) {
  return invoke<void>("detach_tab", { id });
}

/** Outer edge of this browser window in CSS pixels, for drag-out hit testing. */
export async function windowOuterBounds() {
  const appWindow = getCurrentWindow();
  const [position, size, scale] = await Promise.all([
    appWindow.outerPosition(),
    appWindow.outerSize(),
    appWindow.scaleFactor(),
  ]);
  return {
    x: position.x / scale,
    y: position.y / scale,
    width: size.width / scale,
    height: size.height / scale,
  };
}

export function pinTab(id: number) {
  return invoke<void>("pin_tab", { id });
}

export function unpinTab(id: number) {
  return invoke<void>("unpin_tab", { id });
}

/** Reopens settings pins that were closed, in the saved order. */
export function restorePinnedTabs() {
  return invoke<void>("restore_pinned_tabs");
}

export function duplicateTab(id: number) {
  return invoke<number>("duplicate_tab", { id });
}

export function reopenClosedTab() {
  return invoke<number | null>("reopen_closed_tab");
}

/** Recently closed tabs and groups, newest first. */
export function listRecentlyClosed() {
  return invoke<ClosedEntry[]>("list_recently_closed");
}

/** Opens a recently closed entry again. Without an id the newest comes back. */
export function restoreClosed(id?: number) {
  return invoke<number | null>("restore_closed", { id });
}

/** A JPEG data URL of what the tab last showed, or null when none was captured. */
export function tabThumbnail(id: number) {
  return invoke<string | null>("tab_thumbnail", { id });
}

/** Bytes the tab's page uses now, or null when it can't be read. */
export function tabMemory(id: number) {
  return invoke<number | null>("tab_memory", { id });
}

export function undoAction(id: number) {
  return invoke<void>("undo_action", { id });
}

/** The toast closed without Undo, so its action becomes final. */
export function dismissToast(id: number) {
  return invoke<void>("dismiss_toast", { id });
}

/** Chrome insets in CSS pixels. `bottom` is the status bar. Zero means that edge is hidden. */
export function setUiInsets(top: number, left: number, right = 0, bottom = 0) {
  return invoke<void>("set_ui_insets", { top, left, right, bottom });
}

export function createEphemeralTab() {
  return invoke<number>("create_ephemeral_tab");
}

export function listContainers() {
  return invoke<Container[]>("list_containers");
}

export function createContainer(name: string, color: string) {
  return invoke<Container>("create_container", { name, color });
}

export function deleteContainer(id: string) {
  return invoke<void>("delete_container", { id });
}

export function setGroupContainer(groupId: number, container: string) {
  return invoke<void>("set_group_container", { groupId, container });
}

export function listPermissions() {
  return invoke<PermissionGrant[]>("list_permissions");
}

export function setPermission(origin: string, kind: string, allow: boolean) {
  return invoke<void>("set_permission", { origin, kind, allow });
}

export function onPermissionsChanged(handler: (grants: PermissionGrant[]) => void): Promise<UnlistenFn> {
  return listen<PermissionGrant[]>("permissions-changed", (event) => handler(event.payload));
}

export function onContainersChanged(handler: (containers: Container[]) => void): Promise<UnlistenFn> {
  return listen<Container[]>("containers-changed", (event) => handler(event.payload));
}

export function networkLog(id: number) {
  return invoke<NetworkEntry[]>("network_log", { id });
}

export function clearNetworkLog(id: number) {
  return invoke<void>("clear_network_log", { id });
}

export function setTabRefresh(id: number, secs: number, watch: boolean) {
  return invoke<void>("set_tab_refresh", { id, secs, watch });
}

export function openReader(id: number) {
  return invoke<void>("open_reader", { id });
}

export function readerPage() {
  return invoke<ExtractedPage | null>("reader_page");
}

export function showReaderOriginal() {
  return invoke<void>("show_reader_original");
}

export function startZapper(id: number) {
  return invoke<void>("start_zapper", { id });
}

export function startInspector(id: number) {
  return invoke<void>("start_inspector", { id });
}

export function translationActive(id: number) {
  return invoke<boolean>("translation_active", { id });
}

export function translatePage(id: number, language: string) {
  return invoke<void>("translate_page", { id, language });
}

export function restoreTranslation(id: number) {
  return invoke<void>("restore_translation", { id });
}

/** Translates text into the language from settings, with the configured provider. */
export function translateSelection(text: string) {
  return invoke<string>("translate_selection", { text });
}

export function generatePassword(
  length: number,
  upper: boolean,
  lower: boolean,
  digits: boolean,
  symbols: boolean,
  minSymbols: number,
  avoidAmbiguous: boolean,
) {
  return invoke<string>("generate_password", { length, upper, lower, digits, symbols, minSymbols, avoidAmbiguous });
}

export function copyText(text: string) {
  return invoke<void>("copy_text", { text });
}

export function showNotice(message: string) {
  return invoke<void>("show_notice", { message });
}

export function waybackUrl(save: boolean, url: string) {
  return invoke<string>("wayback_url", { save, url });
}

export function readingStats(id: number) {
  return invoke<ReadingStats>("reading_stats", { id });
}

// Boards

export function listBoards() {
  return invoke<BoardsSnapshot>("list_boards");
}

export function onBoardsChanged(handler: (boards: BoardsSnapshot) => void): Promise<UnlistenFn> {
  return listen<BoardsSnapshot>("boards-changed", (event) => handler(event.payload));
}

export function createBoardFolder(name: string, parentId: string | null) {
  return invoke<BoardFolder>("create_board_folder", { name, parentId });
}

export function renameBoardFolder(id: string, name: string) {
  return invoke<void>("rename_board_folder", { id, name });
}

export function moveBoardFolder(id: string, parentId: string | null) {
  return invoke<void>("move_board_folder", { id, parentId });
}

export function deleteBoardFolder(id: string) {
  return invoke<void>("delete_board_folder", { id });
}

export function createBoard(name: string, folderId: string | null) {
  return invoke<Board>("create_board", { name, folderId });
}

export function renameBoard(id: string, name: string) {
  return invoke<void>("rename_board", { id, name });
}

export function moveBoard(id: string, folderId: string | null) {
  return invoke<void>("move_board", { id, folderId });
}

export function setBoardCover(id: string, pinId: string | null) {
  return invoke<void>("set_board_cover", { id, pinId });
}

export function deleteBoard(id: string) {
  return invoke<void>("delete_board", { id });
}

export function updatePin(id: string, title: string, note: string) {
  return invoke<void>("update_pin", { id, title, note });
}

export function movePins(ids: string[], boardId: string) {
  return invoke<void>("move_pins", { ids, boardId });
}

export function deletePins(ids: string[]) {
  return invoke<void>("delete_pins", { ids });
}

/** An empty `boardId` saves to the last board used. */
export function pinPage(id: number, boardId: string, fullPage: boolean) {
  return invoke<Pin>("pin_page", { id, boardId, fullPage });
}

export function pinRegion(id: number, boardId: string) {
  return invoke<Pin>("pin_region", { id, boardId });
}

export function pinElement(id: number, boardId: string) {
  return invoke<Pin>("pin_element", { id, boardId });
}

export function pinLink(id: number, boardId: string) {
  return invoke<Pin>("pin_link", { id, boardId });
}

export function pinImages(id: number, boardId: string, urls: string[]) {
  return invoke<Pin[]>("pin_images", { id, boardId, urls });
}

/** JPEG data URLs by pin id. Pins without an image are left out. */
export function boardThumbnails(ids: string[]) {
  return invoke<Record<string, string>>("board_thumbnails", { ids });
}

/** The full PNG of a pin as a data URL. */
export function boardImage(id: string) {
  return invoke<string>("board_image", { id });
}

export function copyPinImage(id: string) {
  return invoke<void>("copy_pin_image", { id });
}

export function savePinImage(id: string, path: string) {
  return invoke<void>("save_pin_image", { id, path });
}

export function listPrices() {
  return invoke<ProductPrice[]>("list_prices");
}

export function trackPrice(id: number) {
  return invoke<ProductPrice>("track_price", { id });
}

export function checkPrices() {
  return invoke<ProductPrice[]>("check_prices");
}

export function askPage(id: number, question: string) {
  return invoke<string>("ask_page", { id, question });
}

export function findDeals(id: number) {
  return invoke<ShoppingDeals>("find_deals", { id });
}

export function installSite(id: number) {
  return invoke<void>("install_site", { id });
}

export function openDefaultApps() {
  return invoke<void>("open_default_apps");
}

export function setDevice(window: string, device: string) {
  return invoke<void>("set_device", { window, device });
}

export function listRewriteRules() {
  return invoke<RewriteRule[]>("list_rewrite_rules");
}

export function saveRewriteRules(rules: RewriteRule[]) {
  return invoke<RewriteRule[]>("save_rewrite_rules", { rules });
}

export function installedBrowsers() {
  return invoke<ExternalBrowser[]>("installed_browsers");
}

export function openInBrowser(kind: string, url: string) {
  return invoke<void>("open_in_browser", { kind, url });
}

export function listCookies(id: number) {
  return invoke<SiteCookie[]>("list_cookies", { id });
}

export function setCookie(id: number, name: string, value: string) {
  return invoke<void>("set_cookie", { id, name, value });
}

export function deleteCookie(id: number, name: string, domain: string, path: string) {
  return invoke<void>("delete_cookie", { id, name, domain, path });
}

export function listLocalStorage(id: number) {
  return invoke<StoragePair[]>("list_local_storage", { id });
}

export function setLocalStorage(id: number, key: string, value: string) {
  return invoke<void>("set_local_storage", { id, key, value });
}

export function deleteLocalStorage(id: number, key: string) {
  return invoke<void>("delete_local_storage", { id, key });
}

export function setTabsMuted(ids: number[], muted: boolean) {
  return invoke<void>("set_tabs_muted", { ids, muted });
}

export function toggleSelectedMuted() {
  return invoke<void>("toggle_selected_muted");
}

export function sleepTab(id: number) {
  return invoke<void>("sleep_tab", { id });
}

export function sleepOtherTabs(id: number) {
  return invoke<void>("sleep_other_tabs", { id });
}

export function neverSleepSite(id: number) {
  return invoke<void>("never_sleep_site", { id });
}

// Split view

export function splitWith(id: number) {
  return invoke<void>("split_with", { id });
}

export function openInSplit(url: string) {
  return invoke<number>("open_in_split", { url });
}

export function swapSplit() {
  return invoke<void>("swap_split");
}

export function exitSplit() {
  return invoke<void>("exit_split");
}

export function setSplitRatio(ratio: number) {
  return invoke<void>("set_split_ratio", { ratio });
}

// Tab groups

export function createGroup(ids: number[]) {
  return invoke<number>("create_group", { ids });
}

export function addToGroup(ids: number[], groupId: number) {
  return invoke<void>("add_to_group", { ids, groupId });
}

export function removeFromGroup(ids: number[]) {
  return invoke<void>("remove_from_group", { ids });
}

export function updateGroup(id: number, change: { name?: string; color?: TabGroupColor; collapsed?: boolean }) {
  return invoke<void>("update_group", { id, ...change });
}

export function ungroup(id: number) {
  return invoke<void>("ungroup", { id });
}

export function closeGroup(id: number) {
  return invoke<void>("close_group", { id });
}

export function newTabInGroup(id: number) {
  return invoke<number>("new_tab_in_group", { id });
}

/** Saves the group's tabs as a new folder under Other bookmarks. */
export function saveGroupToBookmarks(id: number) {
  return invoke<BookmarkNode>("save_group_to_bookmarks", { id });
}

/** Opens every link in a bookmark folder as one tab group named after the folder. */
export function openFolderAsGroup(id: string) {
  return invoke<number>("open_folder_as_group", { id });
}

/** Groups loose tabs that share a site. Returns how many groups were made or grew. */
export function groupBySite() {
  return invoke<number>("group_by_site");
}

export function ctrlHeld() {
  return invoke<boolean>("ctrl_held");
}

export function onTabSwitcherStep(handler: (step: number) => void): Promise<UnlistenFn> {
  return getCurrentWebview().listen<number>("tab-switcher-step", (event) => handler(event.payload));
}

// Workspaces

export function createWorkspace(name: string, color: TabGroupColor) {
  return invoke<string>("create_workspace", { name, color });
}

export function renameWorkspace(id: string, name: string, color: TabGroupColor) {
  return invoke<void>("rename_workspace", { id, name, color });
}

export function deleteWorkspace(id: string) {
  return invoke<void>("delete_workspace", { id });
}

export function switchWorkspace(id: string) {
  return invoke<void>("switch_workspace", { id });
}

export function moveTabsToWorkspace(ids: number[], workspace: string) {
  return invoke<void>("move_tabs_to_workspace", { ids, workspace });
}

// Peek

export function peekState() {
  return invoke<PeekInfo | null>("peek_state");
}

export function closePeek() {
  return invoke<void>("close_peek");
}

export function peekToTab() {
  return invoke<number>("peek_to_tab");
}

export function peekToSplit() {
  return invoke<number>("peek_to_split");
}

export function onPeekChanged(handler: (info: PeekInfo | null) => void): Promise<UnlistenFn> {
  return listen<PeekInfo | null>("peek-changed", (event) => handler(event.payload));
}

export function zoomTab(id: number, change: "in" | "out" | "reset") {
  return invoke<number>("zoom_tab", { id, change });
}

export type FindOptions = {
  caseSensitive?: boolean;
  wholeWord?: boolean;
  /** Move to the next or previous match. False searches again from the top of the view. */
  step?: boolean;
};

export function findInPage(id: number, query: string, backwards = false, options: FindOptions = {}) {
  return invoke<FindResult>("find_in_page", { id, query, backwards, ...options });
}

/** Counts matches in every tab of this window's workspace. */
export function findInTabs(query: string, caseSensitive: boolean, wholeWord: boolean) {
  return invoke<FindTabResult[]>("find_in_tabs", { query, caseSensitive, wholeWord });
}

export function clearFind(id: number) {
  return invoke<void>("clear_find", { id });
}

export function openDevtools(id: number) {
  return invoke<void>("open_devtools", { id });
}

export function onTabsChanged(handler: (snapshot: TabsSnapshot) => void): Promise<UnlistenFn> {
  return listen<TabsSnapshot>("tabs-changed", (event) => handler(event.payload));
}

export function onStatusChanged(handler: (status: StatusSnapshot) => void): Promise<UnlistenFn> {
  return listen<StatusSnapshot>("status-changed", (event) => handler(event.payload));
}

export function onStatusText(handler: (status: StatusText) => void): Promise<UnlistenFn> {
  return listen<StatusText>("status-text", (event) => handler(event.payload));
}

export function onBrowserCommand(handler: (command: BrowserCommand) => void): Promise<UnlistenFn> {
  return listen<BrowserCommand>("browser-command", (event) => handler(event.payload));
}

/** Sends a command to this window's browser UI from a popup or a `ce://` page. */
export function sendUiCommand(command: BrowserCommand) {
  const label = getCurrentWindow().label;
  const ui = label === "main" ? "ui" : label.replace(/^main-/, "ui-");
  return emitTo(ui, "browser-command", command);
}

// Popups

export function openPopup(request: PopupRequest) {
  return invoke<boolean>("open_popup", { request: { focus: true, params: "", ...request } });
}

export function closePopup() {
  return invoke<void>("close_popup");
}

/** Resizes the open popup to its content height and slides it so the button stays put. */
export function fitPopup(height: number) {
  return invoke<void>("fit_popup", { height });
}

/** Draws only these parts of the popup. The rest shows, and passes clicks to, the page below. */
export function setPopupShape(shape: PopupRect[]) {
  return invoke<void>("set_popup_shape", { shape });
}

/** The parked popup host has applied the saved theme and can show a menu. */
export function popupHostReady() {
  return invoke<void>("popup_host_ready");
}

export function onPopupChanged(handler: (change: PopupChange) => void): Promise<UnlistenFn> {
  return listen<PopupChange>("popup-changed", (event) => handler(event.payload));
}

/** Opens a popup from `element`. The vertical tool strip opens it to the left;
 * every other button hangs it underneath, right edges aligned unless `align` is `start`. */
export function openPopupBelow(
  element: HTMLElement,
  request: Omit<PopupRequest, "x" | "y">,
  align: "start" | "end" = "end",
) {
  const rect = element.getBoundingClientRect();
  const edge = element.closest("#vertical-toolbar") ? "left" : "below";
  return openPopup({
    ...request,
    x: 0,
    y: 0,
    anchorX: rect.left,
    anchorY: rect.top,
    anchorWidth: rect.width,
    anchorHeight: rect.height,
    edge,
    align,
  });
}

export function sendOmniboxState(state: OmniboxState) {
  return emit("omnibox-state", state);
}

export function onOmniboxState(handler: (state: OmniboxState) => void): Promise<UnlistenFn> {
  return listen<OmniboxState>("omnibox-state", (event) => handler(event.payload));
}

export function sendOmniboxReady() {
  return emit("omnibox-ready");
}

export function onOmniboxReady(handler: () => void): Promise<UnlistenFn> {
  return listen("omnibox-ready", () => handler());
}

export function sendOmniboxPick(suggestion: OmniboxSuggestion) {
  return emit("omnibox-pick", suggestion);
}

export function onOmniboxPick(handler: (suggestion: OmniboxSuggestion) => void): Promise<UnlistenFn> {
  return listen<OmniboxSuggestion>("omnibox-pick", (event) => handler(event.payload));
}

// Built-in page tools

export function capturePagePng(id: number, fullPage: boolean, loadLazy: boolean, path: string) {
  return invoke<PageCaptureResult>("capture_page_png", { id, fullPage, loadLazy, path });
}

export function copyPagePng(id: number, fullPage: boolean, loadLazy: boolean) {
  return invoke<PageCaptureResult>("copy_page_png", { id, fullPage, loadLazy });
}

export function captureRegion(id: number) {
  return invoke<PageCaptureResult>("capture_region", { id });
}

export function storeVisiblePng(id: number) {
  return invoke<PageCaptureResult>("store_visible_png", { id });
}

export function lastCapturePng() {
  return invoke<string>("last_capture_png");
}

export function saveEditedPng(data: string, path: string) {
  return invoke<void>("save_edited_png", { data, path });
}

export function copyEditedPng(data: string) {
  return invoke<void>("copy_edited_png", { data });
}

export function startRecording(id: number, kind: "tab" | "area" | "window", path: string) {
  return invoke<void>("start_recording", { id, kind, path });
}

export function stopRecording() {
  return invoke<void>("stop_recording");
}

export function onRecordingStatus(handler: (status: RecordingStatus) => void): Promise<UnlistenFn> {
  return listen<RecordingStatus>("recording-status", (event) => handler(event.payload));
}

export function listMedia() {
  return invoke<MediaElement[]>("list_media");
}

export function controlMedia(tab: number, index: number, action: "play" | "pause" | "seek", time?: number) {
  return invoke<void>("control_media", { tab, index, action, time });
}

export function pageMedia(id: number) {
  return invoke<PageMedia[]>("page_media", { id });
}

export function downloadPageMedia(id: number, urls: string[]) {
  return invoke<number>("download_page_media", { id, urls });
}

export function savePageHtml(id: number, path: string) {
  return invoke<void>("save_page_html", { id, path });
}

export function savePageMarkdown(id: number, path: string) {
  return invoke<void>("save_page_markdown", { id, path });
}

export function ocrVisible(id: number) {
  return invoke<string>("ocr_visible", { id });
}

export function ocrRegion(id: number) {
  return invoke<string>("ocr_region", { id });
}

export function extractPage(id: number) {
  return invoke<ExtractedPage>("extract_page", { id });
}

export function detectTechnologies(id: number) {
  return invoke<PageTechnologies>("detect_technologies", { id });
}

export function printTabToPdf(id: number, options: PdfOptions, path: string) {
  return invoke<void>("print_tab_to_pdf", { id, options, path });
}

export function printHtmlToPdf(html: string, options: PdfOptions, path: string) {
  return invoke<void>("print_html_to_pdf", { html, options, path });
}

export function printHtmlDialog(html: string) {
  return invoke<void>("print_html_dialog", { html });
}

export function openSavedFile(path: string) {
  return invoke<void>("open_saved_file", { path });
}

export function showSavedFile(path: string) {
  return invoke<void>("show_saved_file", { path });
}

export function requestPageTool(request: PageToolRequest) {
  return emit("page-tool-request", request);
}

export function onPageToolRequest(handler: (request: PageToolRequest) => void): Promise<UnlistenFn> {
  return listen<PageToolRequest>("page-tool-request", (event) => handler(event.payload));
}

/** Label of the browser window this webview belongs to. */
export function currentWindowLabel() {
  return getCurrentWindow().label;
}

/** Asks where to save a file. Returns null when the dialog is cancelled. */
export async function chooseSavePath(defaultPath: string, name: string, extensions: string[]) {
  const path = await save({ defaultPath, filters: [{ name, extensions }] });
  return path ?? null;
}

// Bookmarks

export function listBookmarks() {
  return invoke<BookmarkTree>("list_bookmarks");
}

export function addBookmark(
  parentId: string,
  title: string,
  url: string | null,
  index?: number,
  separator = false,
) {
  return invoke<BookmarkNode>("add_bookmark", { parentId, title, url, index, separator });
}

/** Maps a click inside this webview to logical pixels in the browser window. */
export async function windowPoint(clientX: number, clientY: number) {
  try {
    const webview = getCurrentWebview();
    const appWindow = getCurrentWindow();
    const [webviewPos, windowPos, scale] = await Promise.all([
      webview.position(),
      appWindow.innerPosition(),
      appWindow.scaleFactor(),
    ]);
    return {
      x: (webviewPos.x - windowPos.x) / scale + clientX,
      y: (webviewPos.y - windowPos.y) / scale + clientY,
    };
  } catch {
    return { x: clientX, y: clientY };
  }
}

export function updateBookmark(id: string, title: string, url: string | null) {
  return invoke<void>("update_bookmark", { id, title, url });
}

export function moveBookmark(id: string, parentId: string, index?: number) {
  return invoke<void>("move_bookmark", { id, parentId, index });
}

export function deleteBookmark(id: string) {
  return invoke<void>("delete_bookmark", { id });
}

/** Deletes several bookmarks with one Undo toast. */
export function deleteBookmarks(ids: string[]) {
  return invoke<void>("delete_bookmarks", { ids });
}

export function findBookmark(url: string) {
  return invoke<BookmarkNode | null>("find_bookmark", { url });
}

export async function importBookmarksHtml() {
  const path = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "Bookmark file", extensions: ["html", "htm"] }],
  });
  if (typeof path !== "string") {
    return null;
  }
  return invoke<number>("import_bookmarks_html", { path });
}

export async function exportBookmarksHtml() {
  const path = await save({
    defaultPath: "bookmarks.html",
    filters: [{ name: "Bookmark file", extensions: ["html"] }],
  });
  if (!path) {
    return false;
  }
  await invoke<void>("export_bookmarks_html", { path });
  return true;
}

export function onBookmarksChanged(handler: (tree: BookmarkTree) => void): Promise<UnlistenFn> {
  return listen<BookmarkTree>("bookmarks-changed", (event) => handler(event.payload));
}

/** Cached icons by host. Missing hosts are fetched in the background and arrive through `onFaviconsChanged`. */
export function siteFavicons(urls: string[]) {
  return invoke<Record<string, string>>("site_favicons", { urls });
}

export function onFaviconsChanged(handler: (icon: SiteFavicon) => void): Promise<UnlistenFn> {
  return listen<SiteFavicon>("favicons-changed", (event) => handler(event.payload));
}

// Downloads

export function listDownloads() {
  return invoke<DownloadItem[]>("list_downloads");
}

export function pauseDownload(id: number) {
  return invoke<void>("pause_download", { id });
}

export function resumeDownload(id: number) {
  return invoke<void>("resume_download", { id });
}

export function cancelDownload(id: number) {
  return invoke<void>("cancel_download", { id });
}

export function openDownload(id: number) {
  return invoke<void>("open_download", { id });
}

export function showDownloadInFolder(id: number) {
  return invoke<void>("show_download_in_folder", { id });
}

/** Starts a Windows file drag for a finished download. Resolves when the drop ends. */
export function dragDownload(id: number) {
  return invoke<void>("drag_download", { id });
}

export function removeDownload(id: number) {
  return invoke<void>("remove_download", { id });
}

export function clearDownloads() {
  return invoke<void>("clear_downloads");
}

export function browseLocal(path?: string) {
  return invoke<BrowseResult>("browse_local", { path });
}

export function openLocalFile(path: string) {
  return invoke<void>("open_local_file", { path });
}

export function onDownloadsChanged(handler: (items: DownloadItem[]) => void): Promise<UnlistenFn> {
  return listen<DownloadItem[]>("downloads-changed", (event) => handler(event.payload));
}

export function onDownloadStarted(handler: () => void): Promise<UnlistenFn> {
  return listen("download-started", () => handler());
}

// History

export function listHistory() {
  return invoke<HistoryItem[]>("list_history");
}

export function removeHistory(id: number) {
  return invoke<void>("remove_history", { id });
}

export function clearHistory() {
  return invoke<void>("clear_history");
}

/** The next visit to what `text` resolves to began with the address typed into the address bar. */
export function noteTyped(text: string) {
  return invoke<void>("note_typed", { text });
}

export function siteInfo(id: number) {
  return invoke<SiteInfo>("site_info", { id });
}

export function sendOmniboxRemove(index: number) {
  return emit("omnibox-remove", index);
}

export function onOmniboxRemove(handler: (index: number) => void): Promise<UnlistenFn> {
  return listen<number>("omnibox-remove", (event) => handler(event.payload));
}

export function onHistoryChanged(handler: (items: HistoryItem[]) => void): Promise<UnlistenFn> {
  return listen<HistoryItem[]>("history-changed", (event) => handler(event.payload));
}

// Extensions

export function listExtensions() {
  return invoke<ExtensionInfo[]>("list_extensions");
}

export async function installExtensionFolder() {
  const path = await open({ directory: true, multiple: false, title: "Choose an unpacked extension folder" });
  if (typeof path !== "string") {
    return null;
  }
  return invoke<ExtensionInfo>("install_extension_folder", { path });
}

export async function installExtensionZip() {
  const path = await open({
    multiple: false,
    directory: false,
    title: "Choose an extension .zip or .crx",
    filters: [{ name: "Extension package", extensions: ["zip", "crx"] }],
  });
  if (typeof path !== "string") {
    return null;
  }
  return invoke<ExtensionInfo>("install_extension_zip", { path });
}

export function setExtensionEnabled(id: string, enabled: boolean) {
  return invoke<void>("set_extension_enabled", { id, enabled });
}

export function removeExtension(id: string) {
  return invoke<void>("remove_extension", { id });
}

export function openExtensionOptions(id: string) {
  return invoke<number>("open_extension_options", { id });
}

export function installExtensionFromStore(url: string) {
  return invoke<ExtensionInfo>("install_extension_from_store", { url });
}

export function checkExtensionUpdates() {
  return invoke<ExtensionInfo[]>("check_extension_updates");
}

export function updateExtension(id: string) {
  return invoke<ExtensionInfo>("update_extension", { id });
}

export function onExtensionsChanged(handler: (extensions: ExtensionInfo[]) => void): Promise<UnlistenFn> {
  return listen<ExtensionInfo[]>("extensions-changed", (event) => handler(event.payload));
}

// Settings

export function listKeywords() {
  return invoke<Keyword[]>("list_keywords");
}

export function saveKeywords(keywords: Keyword[]) {
  return invoke<Keyword[]>("save_keywords", { keywords });
}

export function onKeywordsChanged(handler: (keywords: Keyword[]) => void): Promise<UnlistenFn> {
  return listen<Keyword[]>("keywords-changed", (event) => handler(event.payload));
}

export function omniboxExtra(text: string) {
  return invoke<{ title: string; url: string; answer: boolean } | null>("omnibox_extra", { text });
}

export function listShortcuts() {
  return invoke<ShortcutBinding[]>("list_shortcuts");
}

export function saveShortcuts(bindings: ShortcutBinding[]) {
  return invoke<ShortcutBinding[]>("save_shortcuts", { bindings });
}

export function resetShortcut(action: string) {
  return invoke<ShortcutBinding[]>("reset_shortcut", { action });
}

export function resetShortcuts() {
  return invoke<ShortcutBinding[]>("reset_shortcuts");
}

export function onShortcutsChanged(handler: (bindings: ShortcutBinding[]) => void): Promise<UnlistenFn> {
  return listen<ShortcutBinding[]>("shortcuts-changed", (event) => handler(event.payload));
}

export function getPageRules() {
  return invoke<PageRules>("get_page_rules");
}

export function updateSiteControl(host: string, patch: SitePatch) {
  return invoke<PageRules>("update_site_control", { host, patch });
}

export function saveUserScript(script: UserScript) {
  return invoke<PageRules>("save_user_script", { script });
}

export function deleteUserScript(id: string) {
  return invoke<PageRules>("delete_user_script", { id });
}

export function onPageRulesChanged(handler: (rules: PageRules) => void): Promise<UnlistenFn> {
  return listen<PageRules>("page-rules-changed", (event) => handler(event.payload));
}

export function getSettings() {
  return invoke<BrowserSettings>("get_settings");
}

export function saveSettings(settings: BrowserSettings) {
  return invoke<BrowserSettings>("save_settings", { settings });
}

export function defaultDownloadDir() {
  return invoke<string>("default_download_dir");
}

/** Settings as a new profile gets them. */
export function defaultSettings() {
  return invoke<BrowserSettings>("default_settings");
}

export function backdropSupport() {
  return invoke<BackdropSupport>("backdrop_support");
}

export function updateStatus() {
  return invoke<UpdateStatus>("update_status");
}

export function checkForUpdate() {
  return invoke<UpdateStatus>("check_for_update");
}

/** Downloads and runs the installer. The browser closes and the new version opens. */
export function installUpdate() {
  return invoke<void>("install_update");
}

export function releaseNotes(version: string) {
  return invoke<ReleaseNotes>("release_notes", { version });
}

export function onUpdateStatus(handler: (status: UpdateStatus) => void): Promise<UnlistenFn> {
  return listen<UpdateStatus>("update-status", (event) => handler(event.payload));
}

export async function chooseFolder(title: string, defaultPath?: string) {
  const path = await open({ directory: true, multiple: false, title, defaultPath });
  return typeof path === "string" ? path : null;
}

export function onSettingsChanged(handler: (settings: BrowserSettings) => void): Promise<UnlistenFn> {
  return listen<BrowserSettings>("settings-changed", (event) => handler(event.payload));
}

export function getUserStyles() {
  return invoke<UserStyles>("get_user_styles");
}

export function saveUserStyles(css: string) {
  return invoke<UserStyles>("save_user_styles", { css });
}

export function reloadUserStyles() {
  return invoke<UserStyles>("reload_user_styles");
}

export function showUserStyles() {
  return invoke<void>("show_user_styles");
}

export function showSyncFolder() {
  return invoke<void>("show_sync_folder");
}

export function defaultUserStyles() {
  return invoke<string>("default_user_styles");
}

export function onUserStylesChanged(handler: (styles: UserStyles) => void): Promise<UnlistenFn> {
  return listen<UserStyles>("user-styles-changed", (event) => handler(event.payload));
}

export function syncStatus() {
  return invoke<SyncStatus>("sync_status");
}

export function createSyncFolder(path: string) {
  return invoke<SyncStatus>("create_sync_folder", { path });
}

export function selectSyncFolder(path: string) {
  return invoke<SyncStatus>("select_sync_folder", { path });
}

export function clearSyncFolder() {
  return invoke<SyncStatus>("clear_sync_folder");
}

export function onSyncStatus(handler: (status: SyncStatus) => void): Promise<UnlistenFn> {
  return listen<SyncStatus>("sync-status", (event) => handler(event.payload));
}

export function listSyncConflicts() {
  return invoke<SyncConflict[]>("list_sync_conflicts");
}

export function resolveSyncConflict(id: string, choice: "mine" | "theirs" | "merge") {
  return invoke<void>("resolve_sync_conflict", { id, choice });
}

export function onSyncConflicts(handler: (conflicts: SyncConflict[]) => void): Promise<UnlistenFn> {
  return listen<SyncConflict[]>("sync-conflicts", (event) => handler(event.payload));
}

export function exportProfileArchive(path: string, password: string) {
  return invoke<void>("export_profile_archive", { path, password });
}

export function importProfileArchive(path: string, password: string) {
  return invoke<ImportedProfile>("import_profile_archive", { path, password });
}

export function searchHistory(query: string) {
  return invoke<HistoryHit[]>("search_history", { query });
}

export function checkBookmarkHealth() {
  return invoke<BookmarkHealthReport>("check_bookmark_health");
}

export function onBookmarkHealth(handler: (progress: BookmarkHealthProgress) => void): Promise<UnlistenFn> {
  return listen<BookmarkHealthProgress>("bookmark-health", (event) => handler(event.payload));
}

export async function pickSyncFolder(title: string) {
  const path = await open({ directory: true, multiple: false, title });
  if (path == null) {
    return null;
  }
  if (typeof path === "string" && path.trim()) {
    return path;
  }
  const detail = typeof path === "string" ? "an empty path" : JSON.stringify(path);
  throw new Error(`Couldn't read the selected folder. The dialog returned ${detail}.`);
}

export function listProfiles() {
  return invoke<BrowserProfile[]>("list_profiles");
}

export function createProfile(name: string) {
  return invoke<BrowserProfile[]>("create_profile", { name });
}

export function openProfile(id: string) {
  return invoke<void>("open_profile", { id });
}

export function installProfileShortcut(id: string) {
  return invoke<BrowserProfile[]>("install_profile_shortcut", { id });
}

export function deleteProfile(id: string) {
  return invoke<BrowserProfile[]>("delete_profile", { id });
}

export function listProfileIcons() {
  return invoke<ProfileIconChoice[]>("list_profile_icons");
}

export function setProfileIcon(id: string, iconId: string, path?: string) {
  return invoke<BrowserProfile[]>("set_profile_icon", { id, iconId, path });
}

export function pinProfileTaskbar(id: string) {
  return invoke<BrowserProfile[]>("pin_profile_taskbar", { id });
}

export function unpinProfileTaskbar(id: string) {
  return invoke<BrowserProfile[]>("unpin_profile_taskbar", { id });
}

export async function pickProfileIcon() {
  const path = await open({
    title: "Choose an icon",
    multiple: false,
    directory: false,
    filters: [{ name: "Image", extensions: ["png", "jpg", "jpeg", "gif", "bmp", "ico"] }],
  });
  return typeof path === "string" ? path : null;
}

// Window

export function minimizeWindow() {
  return getCurrentWindow().minimize();
}

export function toggleMaximizeWindow() {
  return getCurrentWindow().toggleMaximize();
}

export function closeWindow() {
  return getCurrentWindow().close();
}

export function isWindowMaximized() {
  return getCurrentWindow().isMaximized();
}

export function onWindowResized(handler: () => void) {
  return getCurrentWindow().onResized(() => handler());
}
