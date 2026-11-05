import { useEffect, useRef, useState, type KeyboardEvent, type MouseEvent } from "react";
import { ChevronRight, ExternalLink, Folder, Group } from "lucide-react";
import { FavIcon } from "@/components/FavIcon";
import { PopupFrame, PopupMenu, PopupMenuItem, PopupMenuSeparator } from "@/components/PopupFrame";
import {
  closePopup,
  closeSubmenus,
  closeThisPopup,
  createTab,
  navigate,
  openFolderAsGroup,
  openPopup,
} from "@/lib/browser-client";
import { BOOKMARK_DROP_END, useBookmarkDrag } from "@/lib/bookmark-drag";
import { BOOKMARK_SIZE, bookmarkItemStyle } from "@/lib/bookmark-size";
import { CHROME, metric, remBox } from "@/lib/chrome-metrics";
import { cn } from "@/lib/cn";
import { faviconKey } from "@/lib/format";
import { usePopupHost } from "@/lib/popup-host";
import { useBookmarksState, useTabs, findFolder, useSiteFavicons, useSettingsState } from "@/lib/use-browser";
import { BookmarkNode } from "@type/browser";
import type { BookmarkMenuTarget } from "./BookmarksBarContextMenu";

type BookmarkFolderMenuProps = {
  folderId: string;
  /** The bookmarks bar opened this popup because a drag rested on the folder. */
  dragOpened?: boolean;
};

/** Matches the vertical padding of `PopupMenu`, so a submenu's first row lines up with its folder. */
const MENU_PADDING = 6;
const HOVER_DELAY = 250;
const DRAG_HOVER_DELAY = 600;
const MENU_ITEM = '[role="menuitem"]:not(:disabled)';

/** Gap and icon are in `em`, so they follow the text size setting. */
const rowClass =
  "flex w-full items-center gap-[0.85em] rounded-md text-left hover:bg-zinc-800 focus-visible:bg-zinc-800 focus-visible:outline-none";
const rowIconClass = "shrink-0";

function targetKind(node: BookmarkNode): BookmarkMenuTarget["kind"] {
  if (node.separator) {
    return "separator";
  }
  if (node.url) {
    return "link";
  }
  return "folder";
}

function rowId(node: BookmarkNode) {
  return `bookmark-folder-menu-${node.id}`;
}

/**
 * One bookmark folder. Its subfolders open as their own popups beside it, each one level up
 * the popup stack, so every menu can reach the edge of the screen.
 */
export function BookmarkFolderMenu({ folderId, dragOpened = false }: BookmarkFolderMenuProps) {
  const { tree, loaded } = useBookmarksState();
  const { settings } = useSettingsState();
  const folderIcon = remBox(metric(settings?.bookmarkFolderIconSize, CHROME.bookmarkFolderIconSize));
  const rowStyle = bookmarkItemStyle(
    settings?.bookmarkFolderTextSize,
    settings?.bookmarkFolderPadding,
    { text: BOOKMARK_SIZE.folderText, padding: BOOKMARK_SIZE.folderPadding },
    { icon: metric(settings?.bookmarkFolderIconSize, CHROME.bookmarkFolderIconSize) },
  );
  const { activeId } = useTabs();
  const host = usePopupHost();
  const level = host?.level ?? 0;
  const folder = findFolder(tree, folderId);
  const children = folder?.children ?? [];
  const links = children.filter((node) => node.url);
  const icons = useSiteFavicons(children.map((node) => node.url));
  const prefix = `bookmark-folder-panel-${folderId}`;
  /** Folder whose submenu is open beside this one. */
  const [openChild, setOpenChild] = useState<string | null>(null);
  const openChildRef = useRef(openChild);
  openChildRef.current = openChild;
  const hover = useRef<number | null>(null);

  const cancelHover = () => {
    if (hover.current !== null) {
      window.clearTimeout(hover.current);
      hover.current = null;
    }
  };

  useEffect(() => cancelHover, []);

  // A pointer on its way to a submenu crosses other rows. Leaving the window drops what they asked.
  useEffect(() => {
    document.documentElement.addEventListener("mouseleave", cancelHover);
    return () => document.documentElement.removeEventListener("mouseleave", cancelHover);
  }, []);

  // A submenu opened from the keyboard gets focus, and so does its parent when it closes.
  useEffect(() => {
    const onFocus = () => {
      const child = openChildRef.current;
      if (child) {
        setOpenChild(null);
        document.getElementById(`bookmark-folder-menu-${child}`)?.focus();
        return;
      }
      if (level > 0 && document.activeElement === document.body) {
        document.getElementById(prefix)?.querySelector<HTMLElement>(MENU_ITEM)?.focus();
      }
    };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [level, prefix]);

  const openSubmenu = (node: BookmarkNode, row: HTMLElement, focus: boolean) => {
    cancelHover();
    if (!row.isConnected) {
      return;
    }
    const box = row.getBoundingClientRect();
    const params = new URLSearchParams({ id: node.id });
    if (dragOpened) {
      params.set("drag", "1");
    }
    setOpenChild(node.id);
    void openPopup(
      "bookmark-folder",
      { x: 0, y: box.top - MENU_PADDING, width: window.innerWidth, height: box.height },
      { params: params.toString(), placement: "right-start", child: true, focus: focus && !dragOpened },
    );
  };

  const closeChild = () => {
    cancelHover();
    if (openChildRef.current) {
      setOpenChild(null);
      void closeSubmenus();
    }
  };

  const hoverRow = (node: BookmarkNode | null, row: HTMLElement) => {
    cancelHover();
    if (node && !node.url && !node.separator) {
      if (openChild === node.id) {
        return;
      }
      hover.current = window.setTimeout(() => {
        hover.current = null;
        openSubmenu(node, row, false);
      }, HOVER_DELAY);
      return;
    }
    if (!openChild) {
      return;
    }
    hover.current = window.setTimeout(() => {
      hover.current = null;
      closeChild();
    }, HOVER_DELAY);
  };

  const drag = useBookmarkDrag({
    parentId: folderId,
    axis: "y",
    hoverDelay: DRAG_HOVER_DELAY,
    onHoverFolder: (node, element) => openSubmenu(node, element, false),
    onDropped: dragOpened ? () => void closePopup() : undefined,
  });

  const openLink = (url: string, event: MouseEvent) => {
    if (drag.dragged.current) {
      drag.dragged.current = false;
      return;
    }
    const background = event.ctrlKey || event.button === 1;
    if (background || activeId === null) {
      void createTab(url, background);
    } else {
      void navigate(activeId, url);
    }
    if (!background) {
      void closePopup();
    }
  };

  const clickFolder = (event: MouseEvent<HTMLElement>, node: BookmarkNode) => {
    if (drag.dragged.current) {
      drag.dragged.current = false;
      return;
    }
    openSubmenu(node, event.currentTarget, event.detail === 0);
  };

  const openContext = (event: MouseEvent, node?: BookmarkNode) => {
    event.preventDefault();
    event.stopPropagation();
    cancelHover();
    setOpenChild(null);
    const params = new URLSearchParams({ parent: folderId, divider: "horizontal" });
    if (node) {
      const kind = targetKind(node);
      params.set("target", node.id);
      params.set("kind", kind);
      if (kind === "link" && node.url) {
        params.set("url", node.url);
      }
    }
    void openPopup("bookmarks-bar-menu", { x: event.clientX, y: event.clientY }, { params: params.toString(), child: true });
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "ArrowRight") {
      const row = document.activeElement;
      const id = row instanceof HTMLElement ? row.dataset.folderId : undefined;
      const node = id ? children.find((child) => child.id === id) : undefined;
      if (row instanceof HTMLElement && node) {
        event.preventDefault();
        openSubmenu(node, row, true);
      }
      return;
    }
    if (event.key === "ArrowLeft" && level > 0) {
      event.preventDefault();
      void closeThisPopup();
    }
  };

  return (
    <PopupFrame id="bookmark-folder-menu" width="menu" maxHeight={2000} ready={loaded}>
      <div id={prefix} onContextMenu={(event) => openContext(event)}>
        <PopupMenu id={`${prefix}-items`} label={folder?.title ?? "Bookmarks"} onKeyDown={onKeyDown}>
          <div
            id={`${prefix}-list`}
            className="flex min-h-8 flex-col"
            onDragOver={drag.listProps.onDragOver}
            onDragLeave={drag.listProps.onDragLeave}
            onDrop={drag.listProps.onDrop}
          >
            {children.length === 0 ? (
              <div id={`${prefix}-empty`} className="px-3 py-2 text-sm text-zinc-500">
                This folder is empty.
              </div>
            ) : null}
            {children.map((node, index) =>
              node.separator ? (
                <div
                  key={node.id}
                  id={rowId(node)}
                  role="separator"
                  className={cn("flex h-3 shrink-0 items-center px-3", drag.dropChrome(node.id))}
                  onPointerEnter={(event) => hoverRow(null, event.currentTarget)}
                  onContextMenu={(event) => openContext(event, node)}
                  {...drag.itemProps(node, index)}
                >
                  <div id={`${rowId(node)}-line`} className="h-px w-full bg-zinc-600" />
                </div>
              ) : node.url ? (
                <button
                  key={node.id}
                  id={rowId(node)}
                  type="button"
                  role="menuitem"
                  title={node.url}
                  className={cn(rowClass, drag.dropChrome(node.id))}
                  style={rowStyle}
                  onPointerEnter={(event) => hoverRow(node, event.currentTarget)}
                  onContextMenu={(event) => openContext(event, node)}
                  onClick={(event) => openLink(node.url ?? "", event)}
                  onAuxClick={(event) => {
                    if (event.button === 1) {
                      event.preventDefault();
                      openLink(node.url ?? "", event);
                    }
                  }}
                  {...drag.itemProps(node, index)}
                >
                  <FavIcon
                    id={`${rowId(node)}-icon`}
                    url={node.url}
                    src={icons[faviconKey(node.url) ?? ""] ?? null}
                    className={rowIconClass}
                    style={folderIcon}
                  />
                  <span id={`${rowId(node)}-title`} className="min-w-0 flex-1 truncate">
                    {node.title || node.url}
                  </span>
                </button>
              ) : (
                <button
                  key={node.id}
                  id={rowId(node)}
                  type="button"
                  role="menuitem"
                  title={node.title}
                  aria-haspopup="menu"
                  aria-expanded={openChild === node.id}
                  data-folder-id={node.id}
                  className={cn(rowClass, openChild === node.id && "bg-zinc-800", drag.dropChrome(node.id))}
                  style={rowStyle}
                  onPointerEnter={(event) => hoverRow(node, event.currentTarget)}
                  onContextMenu={(event) => openContext(event, node)}
                  onClick={(event) => clickFolder(event, node)}
                  {...drag.itemProps(node, index)}
                >
                  <Folder id={`${rowId(node)}-icon`} className={cn(rowIconClass, "text-amber-400")} style={folderIcon} aria-hidden />
                  <span id={`${rowId(node)}-title`} className="min-w-0 flex-1 truncate">
                    {node.title}
                  </span>
                  <ChevronRight id={`${rowId(node)}-chevron`} className="size-[1em] shrink-0 text-zinc-500" aria-hidden />
                </button>
              ),
            )}
            {drag.drop?.id === BOOKMARK_DROP_END ? (
              <div id={`${prefix}-drop-end`} className="mx-3 my-0.5 h-0.5 shrink-0 rounded bg-sky-500" />
            ) : null}
          </div>
          <PopupMenuSeparator id={`${prefix}-separator`} />
          <div
            id={`${prefix}-actions`}
            className="flex flex-col gap-0.5"
            onPointerEnter={(event) => hoverRow(null, event.currentTarget)}
          >
            <PopupMenuItem
              id={`${prefix}-open-all`}
              icon={<ExternalLink />}
              label={`Open all (${links.length})`}
              disabled={links.length === 0}
              onClick={() => {
                for (const link of links) {
                  void createTab(link.url ?? "", true);
                }
                void closePopup();
              }}
            />
            <PopupMenuItem
              id={`${prefix}-open-group`}
              icon={<Group />}
              label="Open all as a tab group"
              disabled={links.length === 0}
              onClick={() => void openFolderAsGroup(folderId).finally(() => closePopup())}
            />
          </div>
        </PopupMenu>
      </div>
    </PopupFrame>
  );
}
