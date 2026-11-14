import { PopupMenu, PopupMenuItem, PopupMenuSeparator, PopupFrame } from "@/components/PopupFrame";
import { openPopup, addBookmark, closePopup, closeThisPopup, createTab, deleteBookmark, getSettings, openFolderAsGroup, openInWindow, saveSettings } from "@/lib/browser-client";
import { BOOKMARK_SIZE, bookmarkItemStyle } from "@/lib/bookmark-size";
import { CHROME, metric } from "@/lib/chrome-metrics";
import { findFolder, useBookmarksState, useSettings } from "@/lib/use-browser";
import { BookmarkTree } from "@type/browser";
import { AppWindow, ExternalLink, FolderPlus, Group, Library, PencilLine, Pin, Plus, SeparatorHorizontal, SeparatorVertical, Trash2 } from "lucide-react";

type DividerOrientation = "vertical" | "horizontal";

export type BookmarkMenuTarget = {
  id: string;
  kind: "link" | "folder" | "separator";
  url?: string;
};

const PROTECTED_FOLDERS = new Set(["bar", "other"]);

type DividerAnchor = { id: string; index: number };

type BookmarksBarMenuItemsProps = {
  idPrefix: string;
  parentId: string;
  divider: DividerOrientation;
  target?: BookmarkMenuTarget | null;
  /** Bookmark the new divider sits against. Empty space uses the nearest row. */
  anchorId?: string | null;
  /** Index used when the anchor is gone, or the folder was empty. */
  index?: number | null;
  onDone: () => void;
};

/** The row closest to the pointer, so a click on empty space still has a place. */
export function nearestBookmarkAnchor(
  event: { clientX: number; clientY: number },
  rows: { id: string; index: number; elementId: string }[],
  axis: "x" | "y",
): DividerAnchor | null {
  const point = axis === "y" ? event.clientY : event.clientX;
  let best: (DividerAnchor & { distance: number }) | null = null;
  for (const row of rows) {
    const element = document.getElementById(row.elementId);
    if (!element) {
      continue;
    }
    const box = element.getBoundingClientRect();
    const start = axis === "y" ? box.top : box.left;
    const end = axis === "y" ? box.bottom : box.right;
    const distance = Math.abs(point - (start + end) / 2);
    if (!best || distance < best.distance) {
      best = { id: row.id, index: row.index, distance };
    }
  }
  return best ? { id: best.id, index: best.index } : null;
}

/** Records where a divider should land. `hit` is the row under the pointer. */
export function setDividerPlace(
  params: URLSearchParams,
  event: { clientX: number; clientY: number },
  rows: { id: string; index: number; elementId: string }[],
  axis: "x" | "y",
  hit?: DividerAnchor | null,
) {
  const place = hit ?? nearestBookmarkAnchor(event, rows, axis);
  if (!place) {
    params.set("index", "0");
    return;
  }
  params.set("anchor", place.id);
  params.set("index", String(place.index));
}

function dividerInsertIndex(
  tree: BookmarkTree | null,
  parentId: string,
  anchorId: string | null,
  fallback: number | null,
  after: boolean,
) {
  const folder = findFolder(tree, parentId);
  if (!folder) {
    const index = Math.max(0, fallback ?? 0);
    return after ? index + 1 : index;
  }
  const children = folder.children ?? [];
  if (anchorId) {
    const at = children.findIndex((child) => child.id === anchorId);
    if (at >= 0) {
      return after ? at + 1 : at;
    }
  }
  const index = Math.max(0, Math.min(children.length, fallback ?? children.length));
  return after ? Math.min(children.length, index + 1) : index;
}

/** Top-left of this menu. The add and rename forms replace it there. */
const MENU_CORNER = { x: 0, y: 0 };

function deleteLabel(kind: BookmarkMenuTarget["kind"]) {
  if (kind === "folder") {
    return "Delete folder";
  }
  if (kind === "separator") {
    return "Delete divider";
  }
  return "Delete bookmark";
}

/** Same comparison the settings list uses: one trailing slash still matches. */
function pinKey(url: string) {
  const trimmed = url.trim();
  if (!trimmed.endsWith("/")) {
    return trimmed;
  }
  const scheme = trimmed.indexOf("://");
  if (scheme < 0 || trimmed.length <= scheme + 4) {
    return trimmed;
  }
  return trimmed.slice(0, -1);
}

export function BookmarksBarContextMenuItems({
  idPrefix,
  parentId,
  divider,
  target,
  anchorId = null,
  index = null,
  onDone,
}: BookmarksBarMenuItemsProps) {
  const settings = useSettings();
  const { tree } = useBookmarksState();
  const itemStyle = bookmarkItemStyle(
    settings?.bookmarksBarMenuTextSize,
    settings?.bookmarksBarMenuPadding,
    { text: BOOKMARK_SIZE.menuText, padding: BOOKMARK_SIZE.menuPadding },
    { icon: metric(settings?.bookmarksBarMenuIconSize, CHROME.bookmarksBarMenuIconSize) },
  );
  const openAdd = (mode: "link" | "folder") => {
    const params = new URLSearchParams({ mode, parent: parentId });
    void openPopup("bookmark-add", MENU_CORNER, { params: params.toString() });
  };

  const addDivider = (after: boolean) => {
    const at = dividerInsertIndex(tree, parentId, anchorId, index, after);
    void addBookmark(parentId, "", null, at, true).catch(() => undefined);
  };

  const vertical = divider === "vertical";
  const beforeLabel = vertical ? "Add vertical divider before" : "Add divider above";
  const afterLabel = vertical ? "Add vertical divider after" : "Add divider below";
  const canDelete = !!target && !PROTECTED_FOLDERS.has(target.id);
  const canRename = !!target && target.kind !== "separator" && !PROTECTED_FOLDERS.has(target.id);
  const linkUrl = target?.kind === "link" ? target.url?.trim() : "";

  const remove = () => {
    if (!target || !canDelete) {
      return;
    }
    // Close this menu only. The folder it opened from stays up.
    void deleteBookmark(target.id).then(() => closeThisPopup());
  };

  const openRename = () => {
    if (!target || !canRename) {
      return;
    }
    const params = new URLSearchParams({ id: target.id });
    void openPopup("bookmark-rename", MENU_CORNER, { params: params.toString() });
  };

  const addToStartupPins = async () => {
    if (!linkUrl) {
      return;
    }
    const base = settings ?? (await getSettings());
    const url = linkUrl.trim();
    const key = pinKey(url);
    if (base.pinnedTabs.some((pin) => pinKey(pin) === key)) {
      return;
    }
    await saveSettings({ ...base, pinnedTabs: [...base.pinnedTabs, url] });
  };

  return (
    <PopupMenu id={`${idPrefix}-items`} label={vertical ? "Bookmarks bar" : "Bookmark folder"}>
      {linkUrl ? (
        <>
          <PopupMenuItem
            id={`${idPrefix}-open-tab`}
            icon={<ExternalLink />}
            label="Open in new tab"
            style={itemStyle}
            onClick={() => void createTab(linkUrl).finally(onDone)}
          />
          <PopupMenuItem
            id={`${idPrefix}-open-window`}
            icon={<AppWindow />}
            label="Open in new window"
            style={itemStyle}
            onClick={() => void openInWindow(linkUrl).finally(onDone)}
          />
          <PopupMenuItem
            id={`${idPrefix}-startup-pin`}
            icon={<Pin />}
            label="Add to startup pinned tabs"
            style={itemStyle}
            onClick={() => void addToStartupPins().finally(onDone)}
          />
          <PopupMenuSeparator id={`${idPrefix}-open-separator`} />
        </>
      ) : null}
      <PopupMenuItem id={`${idPrefix}-add`} icon={<Plus />} label="Add bookmark" style={itemStyle} onClick={() => openAdd("link")} />
      <PopupMenuItem
        id={`${idPrefix}-add-folder`}
        icon={<FolderPlus />}
        label="Add folder"
        style={itemStyle}
        onClick={() => openAdd("folder")}
      />
      <PopupMenuSeparator id={`${idPrefix}-separator`} />
      <PopupMenuItem
        id={`${idPrefix}-add-divider-before`}
        icon={vertical ? <SeparatorVertical /> : <SeparatorHorizontal />}
        label={beforeLabel}
        style={itemStyle}
        onClick={() => addDivider(false)}
      />
      <PopupMenuItem
        id={`${idPrefix}-add-divider-after`}
        icon={vertical ? <SeparatorVertical /> : <SeparatorHorizontal />}
        label={afterLabel}
        style={itemStyle}
        onClick={() => addDivider(true)}
      />
      {target?.kind === "folder" ? (
        <>
          <PopupMenuSeparator id={`${idPrefix}-group-separator`} />
          <PopupMenuItem
            id={`${idPrefix}-open-group`}
            icon={<Group />}
            label="Open all as a tab group"
            style={itemStyle}
            onClick={() => void openFolderAsGroup(target.id).finally(onDone)}
          />
        </>
      ) : null}
      {canDelete && target ? (
        <>
          <PopupMenuSeparator id={`${idPrefix}-delete-separator`} />
          {canRename ? (
            <PopupMenuItem
              id={`${idPrefix}-rename`}
              icon={<PencilLine />}
              label="Rename"
              style={itemStyle}
              onClick={openRename}
            />
          ) : null}
          <PopupMenuItem
            id={`${idPrefix}-delete`}
            icon={<Trash2 />}
            label={deleteLabel(target.kind)}
            style={itemStyle}
            danger
            onClick={remove}
          />
        </>
      ) : null}
      <PopupMenuSeparator id={`${idPrefix}-manager-separator`} />
      <PopupMenuItem
        id={`${idPrefix}-manager`}
        icon={<Library />}
        label="Bookmarks Manager"
        style={itemStyle}
        onClick={() => void createTab("ce://bookmarks").finally(onDone)}
      />
    </PopupMenu>
  );
}

type BookmarksBarMenuProps = {
  parentId: string;
  divider: DividerOrientation;
  target?: BookmarkMenuTarget | null;
  anchorId?: string | null;
  index?: number | null;
};

export function BookmarksBarContextMenu({ parentId, divider, target, anchorId, index }: BookmarksBarMenuProps) {
  const settings = useSettings();
  return (
    <PopupFrame id="bookmarks-bar-menu" width={metric(settings?.bookmarksBarMenuWidth, CHROME.bookmarksBarMenuWidth)}>
      <BookmarksBarContextMenuItems
        idPrefix="bookmarks-bar-menu"
        parentId={parentId}
        divider={divider}
        target={target}
        anchorId={anchorId}
        index={index}
        onDone={() => void closePopup()}
      />
    </PopupFrame>
  );
}
