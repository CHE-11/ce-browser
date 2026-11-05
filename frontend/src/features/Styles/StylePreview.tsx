import { FileText, Folder, Globe, History, Plus, Search, Star, X } from "lucide-react";
import type { BrowserSettings } from "@type/browser";
import { IconButton } from "@/components/IconButton";
import { Menu, MenuItem } from "@/components/Menu";
import { bookmarkItemStyle } from "@/lib/bookmark-size";
import {
  CHROME,
  chromeVarStyle,
  downloadsIconClass,
  downloadsRowClass,
  downloadsTextClass,
  historyIconClass,
  historyMetaClass,
  historyRowClass,
  historyTitleClass,
  metric,
  omniboxIconClass,
  omniboxRowClass,
  omniboxRowStyle,
  omniboxUrlClass,
  paletteHintClass,
  paletteInputClass,
  paletteRowClass,
  remBox,
} from "@/lib/chrome-metrics";
import { cn } from "@/lib/cn";
import { tabItemStyle } from "@/lib/tab-size";
import type { StyleSurface } from "./surfaces";

const frameClass = "overflow-hidden rounded-lg border border-zinc-800 bg-zinc-950";

export function StylePreview({ surface, settings }: { surface: StyleSurface; settings: BrowserSettings }) {
  return (
    <div id="styles-preview-root" className="flex h-full min-h-0 flex-col" style={chromeVarStyle(settings)}>
      <div id="styles-preview-caption" className="px-6 pb-3 pt-5 text-xs text-zinc-500">
        Preview
      </div>
      <div id="styles-preview-stage" className="min-h-0 flex-1 overflow-auto px-6 pb-6">
        <PreviewSurface surface={surface} settings={settings} />
      </div>
    </div>
  );
}

function PreviewSurface({ surface, settings }: { surface: StyleSurface; settings: BrowserSettings }) {
  switch (surface.id) {
    case "tab-menu":
      return <TabMenuPreview settings={settings} />;
    case "bookmarks-bar":
      return <BookmarksBarPreview settings={settings} />;
    case "bookmark-folders":
      return <BookmarkFolderPreview settings={settings} />;
    case "bookmark-menu":
      return <BookmarkMenuPreview settings={settings} />;
    case "menus":
      return <MenusPreview />;
    case "toolbar":
      return <ToolbarPreview />;
    case "omnibox":
      return <OmniboxPreview />;
    case "downloads":
      return <DownloadsPreview settings={settings} />;
    case "history":
      return <HistoryPreview settings={settings} />;
    case "command-palette":
      return <PalettePreview settings={settings} />;
    default:
      return <TabsPreview settings={settings} />;
  }
}

function TabsPreview({ settings }: { settings: BrowserSettings }) {
  const icon = remBox(metric(settings.tabIconSize, CHROME.tabIconSize));
  const close = remBox(metric(settings.tabIconSize, CHROME.tabIconSize) * 0.875);
  const style = tabItemStyle(settings.tabItemTextSize, settings.tabItemPadding, settings.tabItemHeight);
  const titles = ["Example", "Notes", "Mail"];
  return (
    <div id="styles-preview-tabs" className={cn(frameClass, "flex items-end gap-px bg-zinc-950 p-3 pt-6")}>
      {titles.map((title, index) => (
        <div
          id={`styles-preview-tab-${index}`}
          key={title}
          className={cn("flex items-center gap-2 rounded-t-lg", index === 0 ? "bg-zinc-800 text-zinc-100" : "text-zinc-400")}
          style={{ ...style, width: settings.tabWidth }}
        >
          <Globe id={`styles-preview-tab-${index}-icon`} className="shrink-0 text-zinc-400" style={icon} aria-hidden />
          <span id={`styles-preview-tab-${index}-title`} className="min-w-0 flex-1 truncate">
            {title}
          </span>
          <X id={`styles-preview-tab-${index}-close`} className="shrink-0" style={close} aria-hidden />
        </div>
      ))}
    </div>
  );
}

function TabMenuPreview({ settings }: { settings: BrowserSettings }) {
  const style = bookmarkItemStyle(
    settings.tabMenuTextSize,
    settings.tabMenuPadding,
    { text: CHROME.tabMenuTextSize, padding: CHROME.tabMenuPadding },
    { height: settings.tabMenuHeight, icon: settings.tabMenuIconSize },
  );
  return (
    <div id="styles-preview-tab-menu" className={cn(frameClass, "bg-zinc-900 p-1.5")} style={{ width: `${settings.tabMenuWidth / 16}rem` }}>
      <Menu id="styles-preview-tab-menu-list" label="Tab">
        <MenuItem id="styles-preview-tab-menu-new" icon={<Plus />} label="New tab to the right" style={style} onClick={() => undefined} />
        <MenuItem id="styles-preview-tab-menu-pin" icon={<Star />} label="Pin" style={style} onClick={() => undefined} />
        <MenuItem id="styles-preview-tab-menu-close" icon={<X />} label="Close" style={style} onClick={() => undefined} />
      </Menu>
    </div>
  );
}

function BookmarksBarPreview({ settings }: { settings: BrowserSettings }) {
  const style = bookmarkItemStyle(
    settings.bookmarksBarTextSize,
    settings.bookmarksBarPadding,
    { text: CHROME.bookmarksBarTextSize, padding: CHROME.bookmarksBarPadding },
    { height: settings.bookmarksBarHeight },
  );
  const icon = remBox(settings.bookmarksBarIconSize);
  const items = [
    { id: "news", label: "News", folder: false },
    { id: "work", label: "Work", folder: true },
    { id: "maps", label: "Maps", folder: false },
  ];
  return (
    <div id="styles-preview-bookmarks-bar" className={cn(frameClass, "flex items-center gap-1 bg-zinc-900 px-2 py-1")}>
      {items.map((item) => (
        <div id={`styles-preview-bookmark-${item.id}`} key={item.id} className="inline-flex items-center gap-[0.5em] rounded-md text-zinc-300" style={style}>
          {item.folder ? (
            <Folder id={`styles-preview-bookmark-${item.id}-icon`} className="shrink-0 text-amber-400" style={icon} aria-hidden />
          ) : (
            <Globe id={`styles-preview-bookmark-${item.id}-icon`} className="shrink-0 text-zinc-400" style={icon} aria-hidden />
          )}
          <span id={`styles-preview-bookmark-${item.id}-label`} className="truncate">
            {item.label}
          </span>
        </div>
      ))}
    </div>
  );
}

function BookmarkFolderPreview({ settings }: { settings: BrowserSettings }) {
  const style = bookmarkItemStyle(settings.bookmarkFolderTextSize, settings.bookmarkFolderPadding, {
    text: CHROME.bookmarkFolderTextSize,
    padding: CHROME.bookmarkFolderPadding,
  });
  const icon = remBox(settings.bookmarkFolderIconSize);
  const rows = ["Inbox", "Design notes", "Travel"];
  return (
    <div id="styles-preview-bookmark-folders" className={cn(frameClass, "flex w-64 flex-col bg-zinc-900 p-1.5")}>
      {rows.map((label, index) => (
        <div id={`styles-preview-folder-row-${index}`} key={label} className="flex items-center gap-[0.5em] rounded-md text-zinc-100" style={style}>
          <Globe id={`styles-preview-folder-row-${index}-icon`} className="shrink-0 text-zinc-400" style={icon} aria-hidden />
          <span id={`styles-preview-folder-row-${index}-label`} className="min-w-0 flex-1 truncate">
            {label}
          </span>
        </div>
      ))}
    </div>
  );
}

function BookmarkMenuPreview({ settings }: { settings: BrowserSettings }) {
  const style = bookmarkItemStyle(
    settings.bookmarksBarMenuTextSize,
    settings.bookmarksBarMenuPadding,
    { text: CHROME.bookmarksBarMenuTextSize, padding: CHROME.bookmarksBarMenuPadding },
    { icon: settings.bookmarksBarMenuIconSize },
  );
  return (
    <div id="styles-preview-bookmark-menu" className={cn(frameClass, "bg-zinc-900 p-1.5")} style={{ width: `${settings.bookmarksBarMenuWidth / 16}rem` }}>
      <Menu id="styles-preview-bookmark-menu-list" label="Bookmark">
        <MenuItem id="styles-preview-bookmark-menu-edit" icon={<Star />} label="Edit" style={style} onClick={() => undefined} />
        <MenuItem id="styles-preview-bookmark-menu-folder" icon={<Folder />} label="New folder" style={style} onClick={() => undefined} />
        <MenuItem id="styles-preview-bookmark-menu-delete" icon={<X />} label="Delete" danger style={style} onClick={() => undefined} />
      </Menu>
    </div>
  );
}

function MenusPreview() {
  return (
    <div id="styles-preview-menus" className={cn(frameClass, "w-72 bg-zinc-900 p-1.5")}>
      <Menu id="styles-preview-menus-list" label="Menu">
        <MenuItem id="styles-preview-menus-new" icon={<Plus />} label="New tab" shortcut="Ctrl+T" onClick={() => undefined} />
        <MenuItem id="styles-preview-menus-history" icon={<History />} label="History" shortcut="Ctrl+H" onClick={() => undefined} />
        <MenuItem id="styles-preview-menus-star" icon={<Star />} label="Bookmark this tab" shortcut="Ctrl+D" onClick={() => undefined} />
      </Menu>
    </div>
  );
}

function ToolbarPreview() {
  return (
    <div id="styles-preview-toolbar" className="chrome-toolbar flex items-center gap-1 rounded-lg border border-zinc-800 bg-zinc-900 px-2">
      <IconButton id="styles-preview-toolbar-back" label="Back">
        <History id="styles-preview-toolbar-back-icon" aria-hidden />
      </IconButton>
      <div id="styles-preview-toolbar-field" className="chrome-address-field mx-1 flex min-w-0 flex-1 items-center gap-2 rounded-full bg-zinc-800 px-3">
        <Search id="styles-preview-toolbar-search" className="size-3.5 shrink-0 text-zinc-400" aria-hidden />
        <span id="styles-preview-toolbar-text" className="chrome-address-text truncate text-zinc-300">
          example.com
        </span>
      </div>
      <IconButton id="styles-preview-toolbar-star" label="Bookmark">
        <Star id="styles-preview-toolbar-star-icon" aria-hidden />
      </IconButton>
    </div>
  );
}

function OmniboxPreview() {
  const rows = [
    { id: "search", title: "example search", url: "" },
    { id: "site", title: "Example", url: "example.com/docs" },
    { id: "history", title: "Example history", url: "example.com/notes" },
  ];
  return (
    <div id="styles-preview-omnibox" className={cn(frameClass, "bg-zinc-900 py-1.5")}>
      {rows.map((row) => (
        <div id={`styles-preview-omnibox-${row.id}`} key={row.id} className={cn("mx-1.5", omniboxRowClass)} style={omniboxRowStyle}>
          <Search id={`styles-preview-omnibox-${row.id}-icon`} className={cn(omniboxIconClass, "text-zinc-400")} aria-hidden />
          <span id={`styles-preview-omnibox-${row.id}-title`} className="min-w-0 truncate">
            {row.title}
          </span>
          {row.url ? (
            <span id={`styles-preview-omnibox-${row.id}-url`} className={cn(omniboxUrlClass, "text-sky-400")}>
              {row.url}
            </span>
          ) : (
            <span id={`styles-preview-omnibox-${row.id}-spacer`} className="flex-1" />
          )}
        </div>
      ))}
    </div>
  );
}

function DownloadsPreview({ settings }: { settings: BrowserSettings }) {
  return (
    <div id="styles-preview-downloads" className={cn(frameClass, "bg-zinc-900 py-1")} style={{ width: `${settings.downloadsPanelWidth / 16}rem` }}>
      <div id="styles-preview-downloads-row" className={cn("flex items-center gap-3", downloadsRowClass)}>
        <FileText id="styles-preview-downloads-icon" className={cn(downloadsIconClass, "text-sky-400")} aria-hidden />
        <span id="styles-preview-downloads-name" className={cn(downloadsTextClass, "min-w-0 flex-1 truncate text-zinc-100")}>
          notes.pdf
        </span>
      </div>
    </div>
  );
}

function HistoryPreview({ settings }: { settings: BrowserSettings }) {
  return (
    <div id="styles-preview-history" className={cn(frameClass, "bg-zinc-900 py-1")} style={{ width: `${settings.historyPanelWidth / 16}rem` }}>
      <div id="styles-preview-history-row" className={cn("flex items-center gap-3", historyRowClass)}>
        <Globe id="styles-preview-history-icon" className={cn(historyIconClass, "shrink-0 text-zinc-400")} aria-hidden />
        <span id="styles-preview-history-body" className="min-w-0 flex-1">
          <span id="styles-preview-history-title" className={historyTitleClass}>
            Example
          </span>
          <span id="styles-preview-history-host" className={cn(historyMetaClass, "block")}>
            example.com
          </span>
        </span>
        <span id="styles-preview-history-time" className={cn(historyMetaClass, "shrink-0")}>
          2m ago
        </span>
      </div>
    </div>
  );
}

function PalettePreview({ settings }: { settings: BrowserSettings }) {
  return (
    <div id="styles-preview-palette" className={cn(frameClass, "bg-zinc-900")} style={{ width: `${settings.commandPaletteWidth / 16}rem` }}>
      <div id="styles-preview-palette-input" className={cn(paletteInputClass, "flex items-center text-zinc-400")}>
        Search actions, tabs, settings
      </div>
      <div id="styles-preview-palette-list" className="p-1.5">
        <div id="styles-preview-palette-row" className={cn(paletteRowClass, "bg-zinc-800")}>
          <span id="styles-preview-palette-group" className="w-24 shrink-0 truncate text-[11px] uppercase tracking-wide text-zinc-500">
            Action
          </span>
          <span id="styles-preview-palette-title" className="min-w-0 flex-1 truncate">
            New tab
          </span>
          <span id="styles-preview-palette-hint" className={paletteHintClass}>
            Ctrl+T
          </span>
        </div>
      </div>
    </div>
  );
}
