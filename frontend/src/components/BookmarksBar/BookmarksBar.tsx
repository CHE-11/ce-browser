import { useRef, useState, type DragEvent, type MouseEvent } from "react";
import { ChevronsRight, Folder } from "lucide-react";
import type { BookmarkNode, BookmarkTree } from "../../../../types/browser";
import { createTab, moveBookmark, navigate, openPopup, openPopupBelow } from "../../lib/browser-client";
import { cn } from "../../lib/cn";
import { BOOKMARK_DRAG_TYPE, bookmarkDropPlace, type BookmarkDropPlace } from "../../lib/use-browser";
import { FavIcon } from "../FavIcon";

type BookmarksBarProps = {
  tree: BookmarkTree | null;
  activeId: number | null;
};

type DropState = { id: string; place: BookmarkDropPlace };

const itemClass =
  "inline-flex h-6 max-w-44 shrink-0 items-center gap-1.5 rounded-md px-2 text-xs text-zinc-300 hover:bg-zinc-800 hover:text-zinc-100";

export function openBookmarkLink(url: string, activeId: number | null, event?: MouseEvent) {
  const background = !!event && (event.ctrlKey || event.button === 1);
  if (background || activeId === null) {
    void createTab(url, background);
  } else {
    void navigate(activeId, url);
  }
}

function dropChrome(drop: DropState | null, id: string) {
  if (!drop || drop.id !== id) {
    return "";
  }
  if (drop.place === "into") {
    return "ring-1 ring-sky-500";
  }
  if (drop.place === "before") {
    return "shadow-[inset_2px_0_0_0_#0ea5e9]";
  }
  return "shadow-[inset_-2px_0_0_0_#0ea5e9]";
}

export function BookmarksBar({ tree, activeId }: BookmarksBarProps) {
  const items = tree?.bar.children ?? [];
  const dragged = useRef(false);
  const [drop, setDrop] = useState<DropState | null>(null);

  const openFolder = (event: MouseEvent<HTMLElement>, node: BookmarkNode) => {
    if (dragged.current) {
      dragged.current = false;
      return;
    }
    void openPopupBelow(
      event.currentTarget,
      { kind: "bookmark-folder", params: `id=${encodeURIComponent(node.id)}`, width: 300, height: 360 },
      "start",
    );
  };

  const openBarMenu = (event: MouseEvent) => {
    event.preventDefault();
    const params = new URLSearchParams({
      x: String(Math.round(event.clientX)),
      y: String(Math.round(event.clientY)),
      parent: "bar",
      divider: "vertical",
    });
    void openPopup({
      kind: "bookmarks-bar-menu",
      params: params.toString(),
      x: event.clientX,
      y: event.clientY,
      width: 240,
      height: 148,
    });
  };

  const rememberDrag = (event: DragEvent, node: BookmarkNode) => {
    dragged.current = true;
    event.dataTransfer.setData(BOOKMARK_DRAG_TYPE, node.id);
    if (node.url) {
      event.dataTransfer.setData("text/uri-list", node.url);
    }
    event.dataTransfer.effectAllowed = "move";
  };

  const allowDrop = (event: DragEvent, node: BookmarkNode) => {
    if (!event.dataTransfer.types.includes(BOOKMARK_DRAG_TYPE)) {
      return;
    }
    event.preventDefault();
    const place = bookmarkDropPlace(event, node, "x");
    setDrop((current) => (current?.id === node.id && current.place === place ? current : { id: node.id, place }));
  };

  const finishDrop = (event: DragEvent, node: BookmarkNode) => {
    event.preventDefault();
    const id = event.dataTransfer.getData(BOOKMARK_DRAG_TYPE);
    const place = bookmarkDropPlace(event, node, "x");
    setDrop(null);
    if (!id || id === node.id) {
      return;
    }
    if (place === "into" && !node.url && !node.separator) {
      void moveBookmark(id, node.id);
      return;
    }
    const index = items.findIndex((item) => item.id === node.id);
    if (index < 0) {
      return;
    }
    void moveBookmark(id, "bar", place === "after" ? index + 1 : index);
  };

  return (
    <div
      id="bookmarks-bar"
      className="flex h-7 shrink-0 items-center gap-0.5 border-b border-zinc-800 bg-zinc-900 px-2"
      onContextMenu={openBarMenu}
      onDragLeave={(event) => {
        const next = event.relatedTarget;
        if (next instanceof Node && event.currentTarget.contains(next)) {
          return;
        }
        setDrop(null);
      }}
    >
      <div id="bookmarks-bar-items" className="flex min-w-0 flex-1 items-center gap-0.5 overflow-hidden">
        {items.length === 0 ? (
          <span id="bookmarks-bar-empty" className="px-2 text-xs text-zinc-500">
            Press Ctrl+D to bookmark a page. Bookmarks in the bar folder show here.
          </span>
        ) : null}
        {items.map((node) =>
          node.separator ? (
            <div
              key={node.id}
              id={`bookmarks-bar-${node.id}`}
              role="separator"
              aria-orientation="vertical"
              title="Divider"
              draggable
              className={cn("flex h-6 w-3 shrink-0 items-center justify-center", dropChrome(drop, node.id))}
              onDragStart={(event) => rememberDrag(event, node)}
              onDragEnd={() => {
                setDrop(null);
                window.setTimeout(() => {
                  dragged.current = false;
                });
              }}
              onDragOver={(event) => allowDrop(event, node)}
              onDrop={(event) => finishDrop(event, node)}
            >
              <div id={`bookmarks-bar-${node.id}-line`} className="h-4 w-px bg-zinc-600" />
            </div>
          ) : node.url ? (
            <button
              key={node.id}
              id={`bookmarks-bar-${node.id}`}
              type="button"
              draggable
              title={`${node.title}\n${node.url}`}
              className={cn(itemClass, dropChrome(drop, node.id))}
              onClick={(event) => {
                if (dragged.current) {
                  dragged.current = false;
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
              onDragStart={(event) => rememberDrag(event, node)}
              onDragEnd={() => {
                setDrop(null);
                window.setTimeout(() => {
                  dragged.current = false;
                });
              }}
              onDragOver={(event) => allowDrop(event, node)}
              onDrop={(event) => finishDrop(event, node)}
            >
              <FavIcon id={`bookmarks-bar-${node.id}-icon`} url={node.url} className="size-3.5" />
              <span id={`bookmarks-bar-${node.id}-title`} className="truncate">
                {node.title || node.url}
              </span>
            </button>
          ) : (
            <button
              key={node.id}
              id={`bookmarks-bar-${node.id}`}
              type="button"
              draggable
              title={node.title}
              className={cn(itemClass, dropChrome(drop, node.id))}
              onClick={(event) => openFolder(event, node)}
              onDragStart={(event) => rememberDrag(event, node)}
              onDragEnd={() => {
                setDrop(null);
                window.setTimeout(() => {
                  dragged.current = false;
                });
              }}
              onDragOver={(event) => allowDrop(event, node)}
              onDrop={(event) => finishDrop(event, node)}
            >
              <Folder id={`bookmarks-bar-${node.id}-icon`} className="size-3.5 shrink-0 text-amber-400" aria-hidden />
              <span id={`bookmarks-bar-${node.id}-title`} className="truncate">
                {node.title}
              </span>
            </button>
          ),
        )}
      </div>
      {items.length > 6 ? (
        <button
          id="bookmarks-bar-all"
          type="button"
          title="All bar bookmarks"
          className={itemClass}
          onClick={(event) => tree && openFolder(event, tree.bar)}
        >
          <ChevronsRight id="bookmarks-bar-all-icon" className="size-3.5" aria-hidden />
        </button>
      ) : null}
      {tree && (tree.other.children?.length ?? 0) > 0 ? (
        <button
          id="bookmarks-bar-other"
          type="button"
          title="Other bookmarks"
          className={cn(itemClass, dropChrome(drop, tree.other.id))}
          onClick={(event) => openFolder(event, tree.other)}
          onDragOver={(event) => allowDrop(event, tree.other)}
          onDrop={(event) => finishDrop(event, tree.other)}
        >
          <Folder id="bookmarks-bar-other-icon" className="size-3.5 shrink-0 text-amber-400" aria-hidden />
          <span id="bookmarks-bar-other-title" className="truncate">
            {tree.other.title || "Other bookmarks"}
          </span>
        </button>
      ) : null}
    </div>
  );
}
