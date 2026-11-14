import { bookmarksBarAlign, BOOKMARKS_BAR_ALIGN_OPTIONS } from "@/lib/bookmarks-bar-align";
import {
  BOOKMARK_DIVIDER_LINE_OPTIONS,
  BOOKMARK_DIVIDER_SHAPE_OPTIONS,
  bookmarkDividerLine,
  bookmarkDividerShape,
} from "@/lib/bookmark-divider";
import { CHROME_LIMITS, type ChromeKey } from "@/lib/chrome-metrics";
import { TAB_SIZE } from "@/lib/tab-size";
import type { BrowserSettings } from "@type/browser";

export type StyleControl = {
  key: ChromeKey;
  label: string;
  hint: string;
  unit: "rem" | "px" | "%";
  min: number;
  max: number;
  step: number;
};

export type StyleChoiceKey =
  | "bookmarksBarAlign"
  | "bookmarksBarDividerLine"
  | "bookmarksBarDividerShape"
  | "bookmarkDividerLine"
  | "bookmarkDividerShape";

export type StyleChoice = {
  key: StyleChoiceKey;
  label: string;
  hint: string;
  options: { value: string; label: string }[];
};

export type StyleSurface = {
  id: string;
  label: string;
  hint: string;
  controls: StyleControl[];
  choices?: StyleChoice[];
};

const TEXT = "In rem. 1rem is the browser's base text size.";
const SIDE = "Side padding in rem. Top and bottom get half.";
const ROW_PAD = "Top and bottom padding of a row, in rem.";

function rem(key: ChromeKey, label: string, hint: string, limits: { min: number; max: number }, step = 0.025): StyleControl {
  return { key, label, hint, unit: "rem", min: limits.min, max: limits.max, step };
}

function px(key: ChromeKey, label: string, hint: string, limits: { min: number; max: number }): StyleControl {
  return { key, label, hint, unit: "px", min: limits.min, max: limits.max, step: 1 };
}

function percent(key: ChromeKey, label: string, hint: string, limits: { min: number; max: number }): StyleControl {
  return { key, label, hint, unit: "%", min: limits.min, max: limits.max, step: 1 };
}

const LINE_OPTIONS = BOOKMARK_DIVIDER_LINE_OPTIONS;
const SHAPE_OPTIONS = BOOKMARK_DIVIDER_SHAPE_OPTIONS;

export function styleChoiceValue(settings: BrowserSettings, key: StyleChoiceKey) {
  switch (key) {
    case "bookmarksBarAlign":
      return bookmarksBarAlign(settings.bookmarksBarAlign);
    case "bookmarksBarDividerLine":
      return bookmarkDividerLine(settings.bookmarksBarDividerLine);
    case "bookmarksBarDividerShape":
      return bookmarkDividerShape(settings.bookmarksBarDividerShape);
    case "bookmarkDividerLine":
      return bookmarkDividerLine(settings.bookmarkDividerLine);
    case "bookmarkDividerShape":
      return bookmarkDividerShape(settings.bookmarkDividerShape);
  }
}

export function withStyleChoice(settings: BrowserSettings, key: StyleChoiceKey, value: string): BrowserSettings {
  switch (key) {
    case "bookmarksBarAlign":
      return { ...settings, bookmarksBarAlign: bookmarksBarAlign(value) };
    case "bookmarksBarDividerLine":
      return { ...settings, bookmarksBarDividerLine: bookmarkDividerLine(value) };
    case "bookmarksBarDividerShape":
      return { ...settings, bookmarksBarDividerShape: bookmarkDividerShape(value) };
    case "bookmarkDividerLine":
      return { ...settings, bookmarkDividerLine: bookmarkDividerLine(value) };
    case "bookmarkDividerShape":
      return { ...settings, bookmarkDividerShape: bookmarkDividerShape(value) };
  }
}

const text = CHROME_LIMITS.text;
const pad = CHROME_LIMITS.padding;
const icon = CHROME_LIMITS.icon;
const height = CHROME_LIMITS.height;

export const STYLE_SURFACES: StyleSurface[] = [
  {
    id: "tabs",
    label: "Tabs",
    hint: "The tab strip. Width is the preferred size before tabs shrink to fit.",
    controls: [
      px("tabWidth", "Width", "Preferred width of a horizontal tab, in pixels.", TAB_SIZE.widthLimits),
      px("verticalTabsWidth", "Vertical width", "Width of the expanded vertical tab sidebar, in pixels.", CHROME_LIMITS.verticalTabs),
      rem("tabItemTextSize", "Text", TEXT, text),
      rem("tabItemPadding", "Padding", SIDE, pad),
      rem("tabItemHeight", "Height", "Minimum height of a horizontal tab, in rem.", height),
      rem("tabIconSize", "Icon", "Favicon size in rem. The close icon stays a little smaller.", icon),
    ],
  },
  {
    id: "tab-menu",
    label: "Tab menu",
    hint: "The menu on a tab. A height of 0 lets the text and padding decide.",
    controls: [
      rem("tabMenuTextSize", "Text", TEXT, text),
      rem("tabMenuPadding", "Padding", SIDE, pad),
      rem("tabMenuHeight", "Height", "Minimum row height in rem. Zero uses the text and padding.", CHROME_LIMITS.optionalHeight),
      rem("tabMenuIconSize", "Icon", "Icon size in rem.", icon),
      px("tabMenuWidth", "Width", "Menu width in pixels at 100% browser UI size.", CHROME_LIMITS.menuWidth),
    ],
  },
  {
    id: "bookmarks-bar",
    label: "Bookmarks bar",
    hint: "Links and folders on the bar under the address field.",
    choices: [
      {
        key: "bookmarksBarAlign",
        label: "Alignment",
        hint: "Where folders and bookmarks sit on the bar.",
        options: BOOKMARKS_BAR_ALIGN_OPTIONS,
      },
    ],
    controls: [
      rem("bookmarksBarTextSize", "Text", TEXT, text),
      rem("bookmarksBarPadding", "Padding", SIDE, pad),
      rem("bookmarksBarHeight", "Height", "Minimum height of a bar item, in rem.", height),
      rem("bookmarksBarIconSize", "Icon", "Favicon and folder icon, in rem.", icon),
    ],
  },
  {
    id: "bar-divider",
    label: "Bar divider",
    hint: "The vertical line between items on the bookmarks bar.",
    choices: [
      {
        key: "bookmarksBarDividerLine",
        label: "Line",
        hint: "Solid, dashed, dotted, or two parallel lines.",
        options: LINE_OPTIONS,
      },
      {
        key: "bookmarksBarDividerShape",
        label: "Shape",
        hint: "Square ends, rounded ends, or a pill.",
        options: SHAPE_OPTIONS,
      },
    ],
    controls: [
      rem("bookmarksBarDividerPadding", "Padding", "Space on each side of the line, in rem.", pad),
      px("bookmarksBarDividerThickness", "Thickness", "Width of the line, in pixels. A double line uses at least 3.", CHROME_LIMITS.dividerThickness),
      percent("bookmarksBarDividerLength", "Length", "How much of the bar height the line fills.", CHROME_LIMITS.dividerLength),
    ],
  },
  {
    id: "bookmark-folders",
    label: "Bookmark folders",
    hint: "Rows in a folder menu and the overflow menu.",
    controls: [
      rem("bookmarkFolderTextSize", "Text", TEXT, text),
      rem("bookmarkFolderPadding", "Padding", SIDE, pad),
      rem("bookmarkFolderIconSize", "Icon", "Favicon and folder icon, in rem.", icon),
    ],
  },
  {
    id: "folder-divider",
    label: "Folder divider",
    hint: "The horizontal line in bookmark folders, the overflow menu, and the bookmarks manager. A label sits in the middle of the line.",
    choices: [
      {
        key: "bookmarkDividerLine",
        label: "Line",
        hint: "Solid, dashed, dotted, or two parallel lines.",
        options: LINE_OPTIONS,
      },
      {
        key: "bookmarkDividerShape",
        label: "Shape",
        hint: "Square ends, rounded ends, or a pill.",
        options: SHAPE_OPTIONS,
      },
    ],
    controls: [
      rem("bookmarkDividerPaddingTop", "Top padding", "Space above a plain divider, in rem.", pad),
      rem("bookmarkDividerPaddingBottom", "Bottom padding", "Space below a plain divider, in rem.", pad),
      rem("bookmarkDividerLabelPaddingTop", "Label top padding", "Space above a divider that has a label, in rem.", pad),
      rem("bookmarkDividerLabelPaddingBottom", "Label bottom padding", "Space below a divider that has a label, in rem.", pad),
      rem("bookmarkDividerInset", "Inset", "Space at each end of the line, in rem.", CHROME_LIMITS.dividerInset),
      px("bookmarkDividerThickness", "Thickness", "Width of the line, in pixels. A double line uses at least 3.", CHROME_LIMITS.dividerThickness),
      percent("bookmarkDividerLength", "Length", "How much of the row the line fills.", CHROME_LIMITS.dividerLength),
    ],
  },
  {
    id: "bookmark-menu",
    label: "Bookmark menu",
    hint: "The menu for adding, renaming, and deleting a bookmark.",
    controls: [
      rem("bookmarksBarMenuTextSize", "Text", TEXT, text),
      rem("bookmarksBarMenuPadding", "Padding", SIDE, pad),
      rem("bookmarksBarMenuIconSize", "Icon", "Icon size in rem.", icon),
      px("bookmarksBarMenuWidth", "Width", "Menu width in pixels at 100% browser UI size.", CHROME_LIMITS.menuWidth),
    ],
  },
  {
    id: "bookmark-editor",
    label: "Bookmark editor",
    hint: "The panel for naming a bookmark and choosing where it sits.",
    controls: [
      rem("bookmarkEditorTextSize", "Text", TEXT, text),
      rem("bookmarkEditorPadding", "Padding", "Space around the name, list, and buttons, in rem.", pad),
      rem("bookmarkEditorRowPadding", "Row padding", SIDE, pad),
      rem("bookmarkEditorIconSize", "Icon", "Favicon size in the place list, in rem.", icon),
      rem("bookmarkEditorListHeight", "List height", "Height of the place list, in rem.", CHROME_LIMITS.listHeight),
      px("bookmarkEditorWidth", "Width", "Editor width in pixels at 100% browser UI size.", CHROME_LIMITS.panelWidth),
    ],
  },
  {
    id: "menus",
    label: "Menus",
    hint: "The app menu, profile list, theme and font lists, pin menus, and workspace menu.",
    controls: [
      rem("menuTextSize", "Text", TEXT, text),
      rem("menuPadding", "Padding", "Side padding of a row, in rem.", pad),
      rem("menuHeight", "Height", "Row height in rem.", height),
      rem("menuIconSize", "Icon", "Icon size in rem.", icon),
    ],
  },
  {
    id: "toolbar",
    label: "Toolbar",
    hint: "The address bar and the tool strip on the right. Other buttons stay their own size.",
    controls: [
      rem("toolbarHeight", "Bar height", "Height of the address bar, in rem.", height),
      rem("toolbarButtonSize", "Button", "Back, tools, and strip buttons, in rem.", height),
      rem("toolbarIconSize", "Icon", "Icons in those buttons, in rem.", icon),
      rem("addressHeight", "Field height", "Height of the address field, in rem.", height),
      rem("addressTextSize", "Field text", TEXT, text),
    ],
  },
  {
    id: "omnibox",
    label: "Omnibox",
    hint: "Suggestions under the address field.",
    controls: [
      rem("omniboxRowHeight", "Row height", "Height of a suggestion, in rem.", height),
      rem("omniboxTextSize", "Text", TEXT, text),
      rem("omniboxIconSize", "Icon", "Icon size in rem.", icon),
      rem("omniboxUrlSize", "Address", "The address beside a title, in rem.", text),
    ],
  },
  {
    id: "downloads",
    label: "Downloads",
    hint: "The downloads panel. The downloads page keeps its own layout.",
    controls: [
      px("downloadsPanelWidth", "Width", "Panel width in pixels at 100% browser UI size.", CHROME_LIMITS.panelWidth),
      rem("downloadsTextSize", "Text", TEXT, text),
      rem("downloadsPadding", "Side padding", "Left and right padding of a row, in rem.", pad),
      rem("downloadsRowPadding", "Row padding", ROW_PAD, pad),
      rem("downloadsIconSize", "Icon", "File icon size in rem.", icon),
    ],
  },
  {
    id: "history",
    label: "History",
    hint: "The history popover.",
    controls: [
      px("historyPanelWidth", "Width", "Panel width in pixels at 100% browser UI size.", CHROME_LIMITS.panelWidth),
      rem("historyTextSize", "Title", TEXT, text),
      rem("historyMetaSize", "Details", "The site and the time, in rem.", text),
      rem("historyPadding", "Side padding", "Left and right padding of a row, in rem.", pad),
      rem("historyRowPadding", "Row padding", ROW_PAD, pad),
      rem("historyIconSize", "Icon", "Favicon size in rem.", icon),
    ],
  },
  {
    id: "command-palette",
    label: "Command palette",
    hint: "The search palette.",
    controls: [
      px("commandPaletteWidth", "Width", "Palette width in pixels at 100% browser UI size.", CHROME_LIMITS.paletteWidth),
      rem("commandPaletteInputHeight", "Search height", "Height of the search field, in rem.", height),
      rem("commandPaletteRowHeight", "Row height", "Height of a result, in rem.", height),
      rem("commandPaletteTextSize", "Text", TEXT, text),
      rem("commandPaletteHintSize", "Hint", "The hint on the right, in rem.", text),
    ],
  },
];

export function styleSurface(id: string | null) {
  return STYLE_SURFACES.find((surface) => surface.id === id) ?? STYLE_SURFACES[0];
}
