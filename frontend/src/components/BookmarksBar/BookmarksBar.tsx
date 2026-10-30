import type { MouseEvent } from "react";
import { ChevronsRight, Folder } from "lucide-react";
import type { BookmarkNode, BookmarkTree } from "../../../../types/browser";
import { createTab, navigate, openPopup, openPopupBelow } from "../../lib/browser-client";
import { FavIcon } from "../FavIcon";

type BookmarksBarProps = {
  tree: BookmarkTree | null;
  activeId: number | null;
};

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

export function BookmarksBar({ tree, activeId }: BookmarksBarProps) {
  const items = tree?.bar.children ?? [];

  const openFolder = (event: MouseEvent<HTMLElement>, node: BookmarkNode) => {
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

  return (
    <div
      id="bookmarks-bar"
      className="flex h-7 shrink-0 items-center gap-0.5 border-b border-zinc-800 bg-zinc-900 px-2"
      onContextMenu={openBarMenu}
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
              className="mx-1 h-4 w-px shrink-0 bg-zinc-600"
            />
          ) : node.url ? (
            <button
              key={node.id}
              id={`bookmarks-bar-${node.id}`}
              type="button"
              title={`${node.title}\n${node.url}`}
              className={itemClass}
              onClick={(event) => openBookmarkLink(node.url ?? "", activeId, event)}
              onAuxClick={(event) => {
                if (event.button === 1) {
                  event.preventDefault();
                  openBookmarkLink(node.url ?? "", activeId, event);
                }
              }}
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
              title={node.title}
              className={itemClass}
              onClick={(event) => openFolder(event, node)}
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
          className={itemClass}
          onClick={(event) => openFolder(event, tree.other)}
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
