import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type KeyboardEvent, type MouseEvent } from "react";
import { ChevronRight, ExternalLink, Folder, Group } from "lucide-react";
import { FavIcon } from "@/components/FavIcon";
import { PopupMenu, PopupMenuItem, PopupMenuSeparator, popupSurfaceClass } from "@/components/PopupFrame";
import { windowPoint, createTab, navigate, closePopup, openFolderAsGroup } from "@/lib/browser-client";
import { BOOKMARK_DROP_END, useBookmarkDrag } from "@/lib/bookmark-drag";
import { BOOKMARK_SIZE, bookmarkItemStyle } from "@/lib/bookmark-size";
import { cn } from "@/lib/cn";
import { faviconKey } from "@/lib/format";
import { POPUP_SHAPE_ATTR, usePopupShape } from "@/lib/popup-shape";
import { useBookmarksState, useTabs, findFolder, useSiteFavicons, useSettingsState } from "@/lib/use-browser";
import { BookmarkNode } from "@type/browser";
import { BookmarksBarMenuItems, type BookmarkMenuTarget } from "./BookmarksBarMenu";

type BookmarkFolderMenuProps = {
  folderId: string;
  /** Left edge of the folder button, in popup px. */
  left: number;
  /** The bookmarks bar opened this popup because a drag rested on the folder. */
  dragOpened?: boolean;
};

type Side = "left" | "right";

/** One folder's menu and where it sits in the popup. */
type Panel = {
  id: string;
  left: number;
  /** Wanted top. The panel moves up when it would run past the bottom. */
  top: number;
  /** Side its own submenus try first. */
  side: Side;
  /** Opened from the keyboard, so its first item takes focus. */
  focus: boolean;
};

type FolderContext = {
  x: number;
  y: number;
  windowX: number;
  windowY: number;
  parentId: string;
  target: BookmarkMenuTarget | null;
};

const PANEL_WIDTH = 300;
const SUBMENU_GAP = 2;
/** Matches the vertical padding of `PopupMenu`, so a submenu's first row lines up with its folder. */
const MENU_PADDING = 6;
const HOVER_DELAY = 250;
const DRAG_HOVER_DELAY = 600;
const MENU_ITEM = '[role="menuitem"]:not(:disabled)';

/** Gap and icon are in `em`, so they follow the text size setting. */
const rowClass =
  "flex w-full items-center gap-[0.85em] rounded-md text-left hover:bg-zinc-800 focus-visible:bg-zinc-800 focus-visible:outline-none";
const rowIconClass = "size-[1.15em] shrink-0";

function targetKind(node: BookmarkNode): BookmarkMenuTarget["kind"] {
  if (node.separator) {
    return "separator";
  }
  if (node.url) {
    return "link";
  }
  return "folder";
}

function isFolder(node: BookmarkNode) {
  return !node.url && !node.separator;
}

function rowId(node: BookmarkNode | Panel) {
  return `bookmark-folder-menu-${node.id}`;
}

/** Room kept below the cursor so the context menu is not clipped. */
function contextMenuReserve(target: BookmarkMenuTarget | null) {
  if (!target) {
    return 156;
  }
  if (target.kind === "separator") {
    return 204;
  }
  if (target.kind === "folder") {
    return 300;
  }
  return 252;
}

/** Beside `parent` on `preferred`, or the other side when that one has no room. */
function placeSubmenu(parent: DOMRect, row: DOMRect, preferred: Side): Omit<Panel, "id" | "focus"> {
  const width = window.innerWidth;
  const rightLeft = parent.right + SUBMENU_GAP;
  const leftLeft = parent.left - SUBMENU_GAP - PANEL_WIDTH;
  const fitsRight = rightLeft + PANEL_WIDTH <= width;
  const fitsLeft = leftLeft >= 0;
  const side: Side = preferred === "right" ? (fitsRight || !fitsLeft ? "right" : "left") : fitsLeft || !fitsRight ? "left" : "right";
  const left = side === "right" ? Math.min(rightLeft, Math.max(0, width - PANEL_WIDTH)) : Math.max(0, leftLeft);
  return { left, top: row.top - MENU_PADDING, side };
}

/** Moves the panel up when it would run past the bottom of the popup. */
function placePanel(element: HTMLElement, top: number) {
  element.style.top = `${Math.max(0, Math.min(top, window.innerHeight - element.offsetHeight))}px`;
}

export function BookmarkFolderMenu({ folderId, left, dragOpened = false }: BookmarkFolderMenuProps) {
  const { tree } = useBookmarksState();
  const { settings } = useSettingsState();
  const rowStyle = bookmarkItemStyle(settings?.bookmarkFolderTextSize, settings?.bookmarkFolderPadding, {
    text: BOOKMARK_SIZE.folderText,
    padding: BOOKMARK_SIZE.folderPadding,
  });
  const { activeId } = useTabs();
  const [submenus, setSubmenus] = useState<Panel[]>([]);
  const [context, setContext] = useState<FolderContext | null>(null);
  const hover = useRef<{ level: number; timer: number } | null>(null);
  const shape = usePopupShape();

  const root: Panel = {
    id: folderId,
    left: Math.max(0, Math.min(left, window.innerWidth - PANEL_WIDTH)),
    top: 0,
    side: "right",
    focus: false,
  };
  const panels = [root];
  for (const submenu of submenus) {
    if (!findFolder(tree, submenu.id)) {
      break;
    }
    panels.push(submenu);
  }

  const cancelHover = () => {
    if (hover.current) {
      window.clearTimeout(hover.current.timer);
      hover.current = null;
    }
  };

  useEffect(() => cancelHover, []);

  const scheduleHover = (level: number, action: () => void) => {
    cancelHover();
    const timer = window.setTimeout(() => {
      hover.current = null;
      action();
    }, HOVER_DELAY);
    hover.current = { level, timer };
  };

  /** Shows `node` beside the panel at `level`, replacing any deeper submenus. */
  const openSubmenu = (level: number, node: BookmarkNode, row: HTMLElement, focus: boolean) => {
    cancelHover();
    const parent = row.closest<HTMLElement>(`[${POPUP_SHAPE_ATTR}]`);
    if (!parent || !row.isConnected) {
      return;
    }
    if (submenus[level]?.id === node.id) {
      if (focus) {
        document.getElementById(`bookmark-folder-panel-${node.id}`)?.querySelector<HTMLElement>(MENU_ITEM)?.focus();
      }
      return;
    }
    const place = placeSubmenu(parent.getBoundingClientRect(), row.getBoundingClientRect(), panels[level].side);
    setSubmenus((current) => [...current.slice(0, level), { id: node.id, ...place, focus }]);
  };

  /** Closes every submenu deeper than the panel at `level`. */
  const closeAfter = (level: number) => {
    setSubmenus((current) => (current.length > level ? current.slice(0, level) : current));
  };

  const hoverRow = (level: number, node: BookmarkNode | null, row: HTMLElement) => {
    const child = submenus[level];
    if (node && isFolder(node)) {
      if (child?.id === node.id) {
        cancelHover();
        return;
      }
      scheduleHover(level, () => openSubmenu(level, node, row, false));
      return;
    }
    if (!child) {
      cancelHover();
      return;
    }
    scheduleHover(level, () => closeAfter(level));
  };

  /** The pointer reached a submenu, so a pending change from a row on the way there is dropped. */
  const enterPanel = (level: number) => {
    if (hover.current && hover.current.level < level) {
      cancelHover();
    }
  };

  /** Closes the panel at `level` and everything past it, and focuses the folder row that opened it. */
  const back = (level: number) => {
    const panel = panels[level];
    if (level === 0 || !panel) {
      return;
    }
    cancelHover();
    closeAfter(level - 1);
    document.getElementById(rowId(panel))?.focus();
  };

  useEffect(() => {
    if (!context) {
      return;
    }
    const onKey = (event: globalThis.KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }
      event.stopPropagation();
      setContext(null);
    };
    const onPointer = (event: PointerEvent) => {
      const target = event.target;
      const menu = document.getElementById("bookmark-folder-context");
      if (target instanceof Node && menu?.contains(target)) {
        return;
      }
      setContext(null);
    };
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("pointerdown", onPointer, true);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("pointerdown", onPointer, true);
    };
  }, [context]);

  const { report } = shape;
  useLayoutEffect(report, [context, report]);

  const openContext = (event: MouseEvent, parentId: string, node?: BookmarkNode) => {
    event.preventDefault();
    event.stopPropagation();
    const x = event.clientX;
    const y = event.clientY;
    const target = node ? { id: node.id, kind: targetKind(node) } : null;
    setContext({ x, y, windowX: x, windowY: y, parentId, target });
    void windowPoint(x, y).then((point) => {
      setContext((current) => (current && current.x === x && current.y === y ? { ...current, windowX: point.x, windowY: point.y } : current));
    });
  };

  return (
    <div id="bookmark-folder-menu" className="fixed inset-0" onContextMenu={(event) => event.preventDefault()}>
      {panels.map((panel, level) => (
        <FolderPanel
          key={panel.id}
          panel={panel}
          folder={findFolder(tree, panel.id)}
          openChildId={panels[level + 1]?.id ?? null}
          rowStyle={rowStyle}
          activeId={activeId}
          dragOpened={dragOpened}
          onLayout={report}
          onEnter={() => enterPanel(level)}
          onHoverRow={(node, row) => hoverRow(level, node, row)}
          onOpenFolder={(node, row, focus) => openSubmenu(level, node, row, focus)}
          onBack={level > 0 ? () => back(level) : null}
          onContext={(event, node) => openContext(event, panel.id, node)}
        />
      ))}
      {context ? (
        <div
          id="bookmark-folder-context"
          data-popup-shape=""
          className={cn("fixed z-50 w-60", popupSurfaceClass)}
          style={{
            left: Math.max(8, Math.min(context.x, window.innerWidth - 248)),
            top: Math.max(8, Math.min(context.y, window.innerHeight - contextMenuReserve(context.target))),
          }}
        >
          <BookmarksBarMenuItems
            idPrefix="bookmark-folder-context"
            parentId={context.parentId}
            divider="horizontal"
            x={context.windowX}
            y={context.windowY}
            target={context.target}
            onDone={() => setContext(null)}
          />
        </div>
      ) : null}
    </div>
  );
}

type FolderPanelProps = {
  panel: Panel;
  folder: BookmarkNode | null;
  /** Folder whose submenu is open beside this panel. */
  openChildId: string | null;
  rowStyle: CSSProperties;
  activeId: number | null;
  dragOpened: boolean;
  /** The panel moved, resized, mounted or unmounted. */
  onLayout: () => void;
  onEnter: () => void;
  /** `node` is null for rows that are not bookmarks. */
  onHoverRow: (node: BookmarkNode | null, row: HTMLElement) => void;
  onOpenFolder: (node: BookmarkNode, row: HTMLElement, focus: boolean) => void;
  /** Null for the first panel, which has nothing to go back to. */
  onBack: (() => void) | null;
  onContext: (event: MouseEvent, node?: BookmarkNode) => void;
};

function FolderPanel({
  panel,
  folder,
  openChildId,
  rowStyle,
  activeId,
  dragOpened,
  onLayout,
  onEnter,
  onHoverRow,
  onOpenFolder,
  onBack,
  onContext,
}: FolderPanelProps) {
  const ref = useRef<HTMLDivElement>(null);
  const children = folder?.children ?? [];
  const links = children.filter((node) => node.url);
  const icons = useSiteFavicons(children.map((node) => node.url));
  const prefix = `bookmark-folder-panel-${panel.id}`;

  const drag = useBookmarkDrag({
    parentId: panel.id,
    axis: "y",
    hoverDelay: DRAG_HOVER_DELAY,
    onHoverFolder: (node, element) => onOpenFolder(node, element, false),
    onDropped: dragOpened ? () => void closePopup() : undefined,
  });

  const wantedTop = useRef(panel.top);
  wantedTop.current = panel.top;

  useLayoutEffect(() => {
    if (ref.current) {
      placePanel(ref.current, panel.top);
      onLayout();
    }
  });

  useLayoutEffect(() => {
    const element = ref.current;
    if (!element) {
      return;
    }
    const observer = new ResizeObserver(() => {
      placePanel(element, wantedTop.current);
      onLayout();
    });
    observer.observe(element);
    return () => {
      observer.disconnect();
      onLayout();
    };
  }, [onLayout]);

  useEffect(() => {
    if (panel.focus) {
      ref.current?.querySelector<HTMLElement>(MENU_ITEM)?.focus();
    }
  }, [panel.focus]);

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
    onOpenFolder(node, event.currentTarget, event.detail === 0);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "ArrowRight") {
      const row = document.activeElement;
      const id = row instanceof HTMLElement ? row.dataset.folderId : undefined;
      const node = id ? children.find((child) => child.id === id) : undefined;
      if (row instanceof HTMLElement && node) {
        event.preventDefault();
        onOpenFolder(node, row, true);
      }
      return;
    }
    if (event.key === "ArrowLeft" && onBack) {
      event.preventDefault();
      onBack();
    }
  };

  return (
    <div
      id={prefix}
      ref={ref}
      data-popup-shape=""
      className={cn("absolute flex max-h-full flex-col", popupSurfaceClass)}
      style={{ left: panel.left, width: PANEL_WIDTH }}
      onPointerEnter={onEnter}
      onContextMenu={(event) => onContext(event)}
    >
      <div id={`${prefix}-scroller`} className="min-h-0 flex-1 overflow-y-auto">
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
                  onPointerEnter={(event) => onHoverRow(null, event.currentTarget)}
                  onContextMenu={(event) => onContext(event, node)}
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
                  onPointerEnter={(event) => onHoverRow(node, event.currentTarget)}
                  onContextMenu={(event) => onContext(event, node)}
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
                  aria-expanded={openChildId === node.id}
                  data-folder-id={node.id}
                  className={cn(rowClass, openChildId === node.id && "bg-zinc-800", drag.dropChrome(node.id))}
                  style={rowStyle}
                  onPointerEnter={(event) => onHoverRow(node, event.currentTarget)}
                  onContextMenu={(event) => onContext(event, node)}
                  onClick={(event) => clickFolder(event, node)}
                  {...drag.itemProps(node, index)}
                >
                  <Folder id={`${rowId(node)}-icon`} className={cn(rowIconClass, "text-amber-400")} aria-hidden />
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
            onPointerEnter={(event) => onHoverRow(null, event.currentTarget)}
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
              onClick={() => void openFolderAsGroup(panel.id).finally(() => closePopup())}
            />
          </div>
        </PopupMenu>
      </div>
    </div>
  );
}
