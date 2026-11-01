import { useEffect, useState, type MouseEvent } from "react";
import { ArrowLeft, ExternalLink, Folder, FolderCog } from "lucide-react";
import { closePopup, createTab, navigate, windowPoint } from "../../lib/browser-client";
import { findFolder, useBookmarksState, useTabs } from "../../lib/use-browser";
import { FavIcon } from "../FavIcon";
import { PopupFrame, PopupMenu, PopupMenuItem, PopupMenuSeparator } from "../PopupFrame";
import { BookmarksBarMenuItems } from "./BookmarksBarMenu";

type BookmarkFolderMenuProps = {
  folderId: string;
};

type FolderContext = { x: number; y: number; windowX: number; windowY: number };

export function BookmarkFolderMenu({ folderId }: BookmarkFolderMenuProps) {
  const { tree, loaded } = useBookmarksState();
  const { activeId } = useTabs();
  const [path, setPath] = useState<string[]>([folderId]);
  const [context, setContext] = useState<FolderContext | null>(null);
  const currentId = path[path.length - 1];
  const folder = findFolder(tree, currentId);
  const children = folder?.children ?? [];
  const links = children.filter((node) => node.url);

  useEffect(() => {
    if (!context) {
      return;
    }
    const onKey = (event: KeyboardEvent) => {
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

  const openContext = (event: MouseEvent) => {
    event.preventDefault();
    event.stopPropagation();
    const x = event.clientX;
    const y = event.clientY;
    setContext({ x, y, windowX: x, windowY: y });
    void windowPoint(x, y).then((point) => {
      setContext((current) => (current && current.x === x && current.y === y ? { ...current, windowX: point.x, windowY: point.y } : current));
    });
  };

  const openLink = (url: string, event: MouseEvent) => {
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

  return (
    <PopupFrame id="bookmark-folder-menu" maxHeight={560} ready={loaded}>
      <div id="bookmark-folder-menu-body" onContextMenu={openContext}>
      <PopupMenu id="bookmark-folder-menu-items" label={folder?.title ?? "Bookmarks"}>
        {path.length > 1 ? (
          <>
            <PopupMenuItem
              id="bookmark-folder-menu-up"
              icon={<ArrowLeft />}
              label={folder?.title ?? "Back"}
              onClick={() => setPath((current) => current.slice(0, -1))}
            />
            <PopupMenuSeparator id="bookmark-folder-menu-up-separator" />
          </>
        ) : null}
        {children.length === 0 ? (
          <div id="bookmark-folder-menu-empty" className="px-3 py-2 text-sm text-zinc-500">
            This folder is empty.
          </div>
        ) : null}
        {children.map((node) =>
          node.separator ? (
            <div key={node.id} id={`bookmark-folder-menu-${node.id}`} role="separator" className="mx-3 my-1 h-px bg-zinc-600" />
          ) : node.url ? (
            <button
              key={node.id}
              id={`bookmark-folder-menu-${node.id}`}
              type="button"
              role="menuitem"
              title={node.url}
              className="flex h-8 w-full items-center gap-3 px-3 text-left text-sm hover:bg-zinc-800 focus-visible:bg-zinc-800 focus-visible:outline-none"
              onClick={(event) => openLink(node.url ?? "", event)}
              onAuxClick={(event) => {
                if (event.button === 1) {
                  event.preventDefault();
                  openLink(node.url ?? "", event);
                }
              }}
            >
              <FavIcon id={`bookmark-folder-menu-${node.id}-icon`} url={node.url} />
              <span id={`bookmark-folder-menu-${node.id}-title`} className="min-w-0 flex-1 truncate">
                {node.title || node.url}
              </span>
            </button>
          ) : (
            <PopupMenuItem
              key={node.id}
              id={`bookmark-folder-menu-${node.id}`}
              icon={<Folder className="text-amber-400" />}
              label={node.title}
              onClick={() => setPath((current) => [...current, node.id])}
            />
          ),
        )}
        <PopupMenuSeparator id="bookmark-folder-menu-separator" />
        <PopupMenuItem
          id="bookmark-folder-menu-open-all"
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
          id="bookmark-folder-menu-manage"
          icon={<FolderCog />}
          label="Bookmark manager"
          onClick={() => {
            void createTab("ce://bookmarks");
            void closePopup();
          }}
        />
      </PopupMenu>
      {context ? (
        <div
          id="bookmark-folder-context"
          className="fixed z-50 w-60 overflow-hidden rounded-xl bg-zinc-900 shadow-2xl ring-1 ring-inset ring-zinc-700"
          style={{
            left: Math.max(8, Math.min(context.x, window.innerWidth - 248)),
            top: Math.max(8, Math.min(context.y, window.innerHeight - 156)),
          }}
        >
          <BookmarksBarMenuItems
            idPrefix="bookmark-folder-context"
            parentId={currentId}
            divider="horizontal"
            x={context.windowX}
            y={context.windowY}
            onDone={() => setContext(null)}
          />
        </div>
      ) : null}
      </div>
    </PopupFrame>
  );
}
