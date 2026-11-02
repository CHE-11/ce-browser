import { useRef, type MouseEvent } from "react";
import { ChevronsRight, Folder } from "lucide-react";
import { FavIcon } from "@/components/FavIcon";
import { createTab, navigate, openPopup, closePopup } from "@/lib/browser-client";
import { BOOKMARK_DROP_END, useBookmarkDrag } from "@/lib/bookmark-drag";
import { BOOKMARK_SIZE, bookmarkItemStyle } from "@/lib/bookmark-size";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import { faviconKey } from "@/lib/format";
import { useSiteFavicons } from "@/lib/use-browser";
import { BookmarkTree, BookmarkNode } from "@type/browser";

type BookmarksBarProps = {
  tree: BookmarkTree | null;
  activeId: number | null;
  /** rem */
  textSize?: number;
  /** rem */
  padding?: number;
  /** Off hides folder icons. A folder without a title keeps its icon so it stays visible. */
  folderIcons?: boolean;
};

/** Gap, icon and width are in `em`, so they follow the text size setting. */
const itemClass = cn(
  "inline-flex max-w-[15em] shrink-0 items-center gap-[0.5em] rounded-md text-zinc-300 hover:bg-zinc-800 hover:text-zinc-100",
  focusRing,
);
const iconClass = "size-[1.2em] shrink-0";

/** The backend keeps a popup this far inside the window. */
const POPUP_INSET = 4;

/** The folder popup spans the window below `element` so submenus can open beside it. */
function openFolderPopup(element: HTMLElement, node: BookmarkNode, dragOpened = false) {
  const rect = element.getBoundingClientRect();
  const params = new URLSearchParams({ id: node.id, left: String(Math.round(rect.left - POPUP_INSET)) });
  if (dragOpened) {
    params.set("drag", "1");
  }
  return openPopup({
    kind: "bookmark-folder",
    params: params.toString(),
    x: POPUP_INSET,
    y: rect.bottom + 4,
    width: window.innerWidth - POPUP_INSET * 2,
    height: window.innerHeight,
    focus: !dragOpened,
  });
}

export function openBookmarkLink(url: string, activeId: number | null, event?: MouseEvent) {
  const background = !!event && (event.ctrlKey || event.button === 1);
  if (background || activeId === null) {
    void createTab(url, background);
  } else {
    void navigate(activeId, url);
  }
}

export function BookmarksBar({ tree, activeId, textSize, padding, folderIcons = true }: BookmarksBarProps) {
  const items = tree?.bar.children ?? [];
  const icons = useSiteFavicons(items.map((node) => node.url));
  const itemStyle = bookmarkItemStyle(textSize, padding, { text: BOOKMARK_SIZE.barText, padding: BOOKMARK_SIZE.barPadding });
  /** Folder whose popup this drag opened by hovering it. */
  const dragOpened = useRef<string | null>(null);

  const openFolderWhileDragging = (node: BookmarkNode, element: HTMLElement) => {
    if (!drag.dragged.current || dragOpened.current === node.id) {
      return;
    }
    dragOpened.current = node.id;
    void openFolderPopup(element, node, true);
  };

  const closeDragPopup = () => {
    if (dragOpened.current) {
      dragOpened.current = null;
      void closePopup();
    }
  };

  const drag = useBookmarkDrag({
    parentId: "bar",
    axis: "x",
    onHoverFolder: openFolderWhileDragging,
    onDragEnd: closeDragPopup,
  });

  const openFolder = (event: MouseEvent<HTMLElement>, node: BookmarkNode) => {
    if (drag.dragged.current) {
      drag.dragged.current = false;
      return;
    }
    void openFolderPopup(event.currentTarget, node);
  };

  const openBarMenu = (event: MouseEvent, node?: BookmarkNode) => {
    event.preventDefault();
    event.stopPropagation();
    const params = new URLSearchParams({
      x: String(Math.round(event.clientX)),
      y: String(Math.round(event.clientY)),
      parent: "bar",
      divider: "vertical",
    });
    if (node && node.id !== "bar" && node.id !== "other") {
      const kind = node.separator ? "separator" : node.url ? "link" : "folder";
      params.set("target", node.id);
      params.set("kind", kind);
      if (kind === "link" && node.url) {
        params.set("url", node.url);
      }
    }
    const kind = params.get("kind");
    const height = !params.has("target") ? 148 : kind === "folder" ? 300 : kind === "separator" ? 196 : 340;
    void openPopup({
      kind: "bookmarks-bar-menu",
      params: params.toString(),
      x: event.clientX,
      y: event.clientY,
      width: 240,
      height,
    });
  };

  return (
    <div
      id="bookmarks-bar"
      className="flex shrink-0 items-center gap-0.5 border-b border-zinc-800 bg-zinc-900 px-2 py-0.5"
      onContextMenu={openBarMenu}
      onDragLeave={drag.listProps.onDragLeave}
    >
      <div
        id="bookmarks-bar-items"
        className="flex min-w-0 flex-1 items-center gap-0.5 self-stretch overflow-hidden"
        onDragOver={drag.listProps.onDragOver}
        onDrop={drag.listProps.onDrop}
      >
        {items.length === 0 ? (
          <span id="bookmarks-bar-empty" className="px-2 text-xs text-zinc-500">
            Press Ctrl+D to bookmark a page. Bookmarks in the bar folder show here.
          </span>
        ) : null}
        {items.map((node, index) =>
          node.separator ? (
            <div
              key={node.id}
              id={`bookmarks-bar-${node.id}`}
              role="separator"
              aria-orientation="vertical"
              title="Divider"
              className={cn("flex w-3 shrink-0 items-center justify-center self-stretch", drag.dropChrome(node.id))}
              onContextMenu={(event) => openBarMenu(event, node)}
              {...drag.itemProps(node, index)}
            >
              <div id={`bookmarks-bar-${node.id}-line`} className="h-2/3 w-px bg-zinc-600" />
            </div>
          ) : node.url ? (
            <button
              key={node.id}
              id={`bookmarks-bar-${node.id}`}
              type="button"
              title={`${node.title}\n${node.url}`}
              className={cn(itemClass, drag.dropChrome(node.id))}
              style={itemStyle}
              onContextMenu={(event) => openBarMenu(event, node)}
              onClick={(event) => {
                if (drag.dragged.current) {
                  drag.dragged.current = false;
                  return;
                }
                openBookmarkLink(node.url ?? "", activeId, event);
              }}
              onAuxClick={(event) => {
                if (event.button === 1) {
                  event.preventDefault();
                  openBookmarkLink(node.url ?? "", activeId, event);
                }
              }}
              {...drag.itemProps(node, index)}
            >
              <FavIcon
                id={`bookmarks-bar-${node.id}-icon`}
                url={node.url}
                src={icons[faviconKey(node.url) ?? ""] ?? null}
                className={iconClass}
              />
              <span id={`bookmarks-bar-${node.id}-title`} className="truncate">
                {node.title || node.url}
              </span>
            </button>
          ) : (
            <button
              key={node.id}
              id={`bookmarks-bar-${node.id}`}
              type="button"
              title={node.title}
              aria-haspopup="menu"
              className={cn(itemClass, drag.dropChrome(node.id))}
              style={itemStyle}
              onContextMenu={(event) => openBarMenu(event, node)}
              onClick={(event) => openFolder(event, node)}
              {...drag.itemProps(node, index)}
            >
              {folderIcons || !node.title ? (
                <Folder id={`bookmarks-bar-${node.id}-icon`} className={cn(iconClass, "text-amber-400")} aria-hidden />
              ) : null}
              <span id={`bookmarks-bar-${node.id}-title`} className="truncate">
                {node.title}
              </span>
            </button>
          ),
        )}
        {drag.drop?.id === BOOKMARK_DROP_END ? (
          <div id="bookmarks-bar-drop-end" className="h-2/3 w-0.5 shrink-0 rounded bg-sky-500" />
        ) : null}
      </div>
      {tree && items.length > 6 ? (
        <button
          id="bookmarks-bar-all"
          type="button"
          title="All bar bookmarks"
          aria-label="All bar bookmarks"
          aria-haspopup="menu"
          className={cn(itemClass, drag.dropChrome(tree.bar.id))}
          style={itemStyle}
          onClick={(event) => openFolder(event, tree.bar)}
          {...drag.targetProps(tree.bar, items.length)}
        >
          <ChevronsRight id="bookmarks-bar-all-icon" className={iconClass} aria-hidden />
        </button>
      ) : null}
      {tree && (tree.other.children?.length ?? 0) > 0 ? (
        <button
          id="bookmarks-bar-other"
          type="button"
          title="Other bookmarks"
          aria-haspopup="menu"
          className={cn(itemClass, drag.dropChrome(tree.other.id))}
          style={itemStyle}
          onClick={(event) => openFolder(event, tree.other)}
          {...drag.targetProps(tree.other, 0)}
        >
          {folderIcons ? <Folder id="bookmarks-bar-other-icon" className={cn(iconClass, "text-amber-400")} aria-hidden /> : null}
          <span id="bookmarks-bar-other-title" className="truncate">
            {tree.other.title || "Other bookmarks"}
          </span>
        </button>
      ) : null}
    </div>
  );
}
