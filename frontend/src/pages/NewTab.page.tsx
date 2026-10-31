import { useEffect, useRef, useState, type DragEvent, type MouseEvent } from "react";
import { Folder } from "lucide-react";
import type { BookmarkNode } from "../../../types/browser";
import { FavIcon } from "../components/FavIcon";
import { createTab, moveBookmark, navigate } from "../lib/browser-client";
import { cn } from "../lib/cn";
import { BOOKMARK_DRAG_TYPE, bookmarkDropPlace, useAppearanceFromSettings, useBookmarks, useTabs, type BookmarkDropPlace } from "../lib/use-browser";

type DropState = { id: string; place: BookmarkDropPlace };

const itemClass =
  "inline-flex h-8 max-w-56 shrink-0 items-center gap-1.5 rounded-md px-2 text-sm text-zinc-200 hover:bg-zinc-800";

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

export function NewTabPage() {
  useAppearanceFromSettings();
  const tree = useBookmarks();
  const { activeId } = useTabs();
  const items = tree?.bar.children ?? [];
  const dragged = useRef(false);
  const [drop, setDrop] = useState<DropState | null>(null);

  useEffect(() => {
    document.title = "New Tab";
  }, []);

  const openLink = (url: string, event?: MouseEvent) => {
    const background = !!event && (event.button === 1);
    if (background || activeId === null) {
      void createTab(url, background);
    } else {
      void navigate(activeId, url);
    }
  };

  const openFolder = (node: BookmarkNode) => {
    window.location.hash = `#/bookmarks?folder=${encodeURIComponent(node.id)}`;
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
    <div id="newtab" className="flex h-full flex-col bg-zinc-950 text-zinc-100">
      <div
        id="newtab-bar"
        className="flex min-h-0 flex-1 flex-wrap content-start items-center gap-0.5 overflow-y-auto px-2 py-2"
        onDragLeave={(event) => {
          const next = event.relatedTarget;
          if (next instanceof Node && event.currentTarget.contains(next)) {
            return;
          }
          setDrop(null);
        }}
      >
        {items.length === 0 ? (
          <p id="newtab-empty" className="px-2 text-sm text-zinc-500">
            Press Ctrl+D to bookmark a page. Bookmarks in the bar show here.
          </p>
        ) : null}
        {items.map((node) =>
          node.separator ? (
            <div
              key={node.id}
              id={`newtab-bar-${node.id}`}
              role="separator"
              aria-orientation="vertical"
              title="Divider"
              draggable
              className={cn("flex h-8 w-3 shrink-0 items-center justify-center", dropChrome(drop, node.id))}
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
              <div id={`newtab-bar-${node.id}-line`} className="h-4 w-px bg-zinc-600" />
            </div>
          ) : (
            <button
              key={node.id}
              id={`newtab-bar-${node.id}`}
              type="button"
              draggable
              title={node.url ? `${node.title}\n${node.url}` : node.title}
              className={cn(itemClass, dropChrome(drop, node.id))}
              onClick={(event) => {
                if (dragged.current) {
                  dragged.current = false;
                  return;
                }
                if (node.url) {
                  openLink(node.url, event);
                } else {
                  openFolder(node);
                }
              }}
              onAuxClick={(event) => {
                if (event.button === 1 && node.url) {
                  event.preventDefault();
                  openLink(node.url, event);
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
              {node.url ? (
                <FavIcon id={`newtab-bar-${node.id}-icon`} url={node.url} className="size-4" />
              ) : (
                <Folder id={`newtab-bar-${node.id}-icon`} className="size-4 shrink-0 text-amber-400" aria-hidden />
              )}
              <span id={`newtab-bar-${node.id}-title`} className="truncate">
                {node.title || node.url}
              </span>
            </button>
          ),
        )}
      </div>
    </div>
  );
}
