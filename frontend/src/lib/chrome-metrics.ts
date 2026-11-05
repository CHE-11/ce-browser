import type { CSSProperties } from "react";
import type { BrowserSettings } from "@type/browser";
import { BOOKMARK_SIZE } from "./bookmark-size";
import { TAB_SIZE } from "./tab-size";

/** Defaults and limits. They match `BrowserSettings::normalize` in settings.rs. */
export const CHROME_LIMITS = {
  text: { min: 0.5, max: 1.5 },
  padding: { min: 0, max: 1.5 },
  icon: { min: 0.5, max: 3 },
  height: { min: 1, max: 4 },
  optionalHeight: { min: 0, max: 4 },
  menuWidth: { min: 160, max: 480 },
  panelWidth: { min: 240, max: 720 },
  listHeight: { min: 8, max: 36 },
  paletteWidth: { min: 360, max: 880 },
  verticalTabs: { min: 160, max: 400 },
} as const;

export const CHROME = {
  tabWidth: TAB_SIZE.width,
  verticalTabsWidth: 240,
  tabItemTextSize: TAB_SIZE.itemText,
  tabItemPadding: TAB_SIZE.itemPadding,
  tabItemHeight: 1.875,
  tabIconSize: 1,
  tabMenuTextSize: TAB_SIZE.menuText,
  tabMenuPadding: TAB_SIZE.menuPadding,
  tabMenuIconSize: 1,
  tabMenuHeight: 0,
  tabMenuWidth: 260,
  bookmarksBarTextSize: BOOKMARK_SIZE.barText,
  bookmarksBarPadding: BOOKMARK_SIZE.barPadding,
  bookmarksBarIconSize: 0.9,
  bookmarksBarHeight: 1.5,
  bookmarkFolderTextSize: BOOKMARK_SIZE.folderText,
  bookmarkFolderPadding: BOOKMARK_SIZE.folderPadding,
  bookmarkFolderIconSize: 1,
  bookmarksBarMenuTextSize: BOOKMARK_SIZE.menuText,
  bookmarksBarMenuPadding: BOOKMARK_SIZE.menuPadding,
  bookmarksBarMenuIconSize: 1,
  bookmarksBarMenuWidth: 280,
  bookmarkEditorTextSize: 0.75,
  bookmarkEditorPadding: 0.5,
  bookmarkEditorRowPadding: 0.5,
  bookmarkEditorIconSize: 1,
  bookmarkEditorListHeight: 24,
  bookmarkEditorWidth: 400,
  menuTextSize: 0.875,
  menuPadding: 0.625,
  menuHeight: 2.25,
  menuIconSize: 1,
  toolbarHeight: 2.5,
  toolbarButtonSize: 2,
  toolbarIconSize: 1,
  addressHeight: 2,
  addressTextSize: 0.875,
  omniboxRowHeight: 2.25,
  omniboxTextSize: 0.875,
  omniboxIconSize: 1,
  omniboxUrlSize: 0.75,
  downloadsPanelWidth: 380,
  downloadsTextSize: 0.875,
  downloadsPadding: 0.75,
  downloadsRowPadding: 0.5,
  downloadsIconSize: 2,
  historyPanelWidth: 380,
  historyTextSize: 0.875,
  historyMetaSize: 0.75,
  historyPadding: 0.75,
  historyRowPadding: 0.5,
  historyIconSize: 1,
  commandPaletteWidth: 560,
  commandPaletteInputHeight: 3,
  commandPaletteRowHeight: 2.5,
  commandPaletteTextSize: 0.875,
  commandPaletteHintSize: 0.75,
} as const satisfies Partial<Record<keyof BrowserSettings, number>>;

export type ChromeKey = keyof typeof CHROME;

const CHROME_VARS: [string, ChromeKey][] = [
  ["--menu-text", "menuTextSize"],
  ["--menu-padding", "menuPadding"],
  ["--menu-height", "menuHeight"],
  ["--menu-icon", "menuIconSize"],
  ["--toolbar-height", "toolbarHeight"],
  ["--toolbar-button", "toolbarButtonSize"],
  ["--toolbar-icon", "toolbarIconSize"],
  ["--address-height", "addressHeight"],
  ["--address-text", "addressTextSize"],
  ["--omnibox-row-height", "omniboxRowHeight"],
  ["--omnibox-text", "omniboxTextSize"],
  ["--omnibox-icon", "omniboxIconSize"],
  ["--omnibox-url", "omniboxUrlSize"],
  ["--downloads-text", "downloadsTextSize"],
  ["--downloads-padding", "downloadsPadding"],
  ["--downloads-row-padding", "downloadsRowPadding"],
  ["--downloads-icon", "downloadsIconSize"],
  ["--history-text", "historyTextSize"],
  ["--history-meta", "historyMetaSize"],
  ["--history-padding", "historyPadding"],
  ["--history-row-padding", "historyRowPadding"],
  ["--history-icon", "historyIconSize"],
  ["--palette-input-height", "commandPaletteInputHeight"],
  ["--palette-row-height", "commandPaletteRowHeight"],
  ["--palette-text", "commandPaletteTextSize"],
  ["--palette-hint", "commandPaletteHintSize"],
];

/** Shared menu rows. Tab and bookmark menus pass their own style instead. Defaults live on `:root`. */
export const menuRowClass = "h-[var(--menu-height)] px-[var(--menu-padding)] text-[length:var(--menu-text)]";

export const menuIconClass = "size-[var(--menu-icon)] shrink-0 [&_svg]:size-[var(--menu-icon)]";

export const omniboxRowClass = "flex items-center gap-3 rounded-lg px-2.5 text-[length:var(--omnibox-text)]";

export const omniboxRowStyle: CSSProperties = { height: "var(--omnibox-row-height)" };

export const omniboxIconClass = "size-[var(--omnibox-icon)] shrink-0";

export const omniboxUrlClass = "min-w-0 flex-1 truncate text-[length:var(--omnibox-url)]";

export const downloadsRowClass = "px-[var(--downloads-padding)] py-[var(--downloads-row-padding)]";

export const downloadsIconClass = "size-[var(--downloads-icon)] shrink-0";

export const downloadsTextClass = "text-[length:var(--downloads-text)]";

export const historyRowClass = "px-[var(--history-padding)] py-[var(--history-row-padding)]";

export const historyTitleClass = "block truncate text-[length:var(--history-text)] text-zinc-100";

export const historyMetaClass = "truncate text-[length:var(--history-meta)] text-zinc-500";

export const historyIconClass = "size-[var(--history-icon)]";

export const paletteInputClass =
  "h-[var(--palette-input-height)] shrink-0 border-b border-zinc-800 bg-transparent px-4 text-[length:var(--palette-text)] outline-none";

export const paletteRowClass =
  "flex h-[var(--palette-row-height)] w-full items-center gap-3 rounded-lg px-2.5 text-left text-[length:var(--palette-text)]";

export const paletteHintClass = "max-w-56 shrink-0 truncate text-[length:var(--palette-hint)] text-zinc-500";

export function metric(value: number | undefined, fallback: number) {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

export function remBox(size: number): CSSProperties {
  return { width: `${size}rem`, height: `${size}rem` };
}

/** Variables for menus, the toolbar, and the panels. Tabs and bookmarks use inline styles. */
export function chromeVarStyle(settings: Partial<BrowserSettings> | null | undefined): CSSProperties {
  const style: Record<string, string> = {};
  for (const [name, key] of CHROME_VARS) {
    style[name] = `${metric(settings?.[key], CHROME[key])}rem`;
  }
  return style as CSSProperties;
}

/** The browser UI and popup documents. A ce:// page keeps its own layout. */
export function isChromeDocument() {
  const hash = window.location.hash;
  return hash === "" || hash === "#" || hash.startsWith("#/popup");
}

/** Paints chrome sizes on this document. A ce:// page, including Styles, is left alone. */
export function applyChromeMetrics(settings: BrowserSettings | null) {
  if (!isChromeDocument() || !settings) {
    return;
  }
  const root = document.documentElement;
  const style = chromeVarStyle(settings);
  for (const [name, value] of Object.entries(style)) {
    if (typeof value === "string") {
      root.style.setProperty(name, value);
    }
  }
}
