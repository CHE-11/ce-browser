import { useEffect, useRef, useState, type DragEvent, type MouseEvent } from "react";
import { ArrowLeft, ExternalLink, Folder, Group } from "lucide-react";
import { FavIcon } from "@/components/FavIcon";
import { PopupFrame, PopupMenu, PopupMenuItem, PopupMenuSeparator } from "@/components/PopupFrame";
import { windowPoint, createTab, navigate, closePopup, openFolderAsGroup } from "@/lib/browser-client";
import { BOOKMARK_DROP_END, isBookmarkDrag, useBookmarkDrag } from "@/lib/bookmark-drag";
import { BOOKMARK_SIZE, bookmarkItemStyle } from "@/lib/bookmark-size";
import { cn } from "@/lib/cn";
import { faviconKey } from "@/lib/format";
import { useBookmarksState, useTabs, findFolder, useSiteFavicons, useSettingsState } from "@/lib/use-browser";
import { BookmarkNode } from "@type/browser";
import { BookmarksBarMenuItems, type BookmarkMenuTarget } from "./BookmarksBarMenu";

type BookmarkFolderMenuProps = {
  folderId: string;
  /** The bookmarks bar opened this popup because a drag rested on the folder. */
  dragOpened?: boolean;
};

type FolderContext = { x: number; y: number; windowX: number; windowY: number; target: BookmarkMenuTarget | null };

const HOVER_DELAY = 600;
/** The backend stops the popup at the bottom of the window. */
const FILL_WINDOW = 100_000;

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

export function BookmarkFolderMenu({ folderId, dragOpened = false }: BookmarkFolderMenuProps) {
  const { tree, loaded } = useBookmarksState();
  const { settings, loaded: settingsLoaded } = useSettingsState();
  const rowStyle = bookmarkItemStyle(settings?.bookmarkFolderTextSize, settings?.bookmarkFolderPadding, {
    text: BOOKMARK_SIZE.folderText,
    padding: BOOKMARK_SIZE.folderPadding,
  });
  const { activeId } = useTabs();
  const [path, setPath] = useState<string[]>([folderId]);
  const [context, setContext] = useState<FolderContext | null>(null);
  const backTimer = useRef<number | null>(null);
  const currentId = path[path.length - 1];
  const folder = findFolder(tree, currentId);
  const children = folder?.children ?? [];
  const links = children.filter((node) => node.url);
  const icons = useSiteFavicons(children.map((node) => node.url));

  const drag = useBookmarkDrag({
    parentId: currentId,
    axis: "y",
    hoverDelay: HOVER_DELAY,
    onHoverFolder: (node) => setPath((current) => [...current, node.id]),
    onDropped: dragOpened ? () => void closePopup() : undefined,
  });

  const cancelBack = () => {
    if (backTimer.current !== null) {
      window.clearTimeout(backTimer.current);
      backTimer.current = null;
    }
  };

  useEffect(() => cancelBack, []);

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

  const openContext = (event: MouseEvent, node?: BookmarkNode) => {
    event.preventDefault();
    event.stopPropagation();
    const x = event.clientX;
    const y = event.clientY;
    const target = node ? { id: node.id, kind: targetKind(node) } : null;
    setContext({ x, y, windowX: x, windowY: y, target });
    void windowPoint(x, y).then((point) => {
      setContext((current) => (current && current.x === x && current.y === y ? { ...current, windowX: point.x, windowY: point.y } : current));
    });
  };

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

  const enterFolder = (node: BookmarkNode) => {
    if (drag.dragged.current) {
      drag.dragged.current = false;
      return;
    }
    setPath((current) => [...current, node.id]);
  };

  /** A drag resting on the Back row steps out to the parent folder. */
  const hoverBack = (event: DragEvent) => {
    if (!isBookmarkDrag(event) || backTimer.current !== null) {
      return;
    }
    drag.reset();
    backTimer.current = window.setTimeout(() => {
      backTimer.current = null;
      setPath((current) => (current.length > 1 ? current.slice(0, -1) : current));
    }, HOVER_DELAY);
  };

  const leaveBack = (event: DragEvent<HTMLElement>) => {
    const next = event.relatedTarget;
    if (next instanceof Node && event.currentTarget.contains(next)) {
      return;
    }
    cancelBack();
  };

  return (
    <PopupFrame id="bookmark-folder-menu" maxHeight={FILL_WINDOW} ready={loaded && settingsLoaded}>
      <div id="bookmark-folder-menu-body" onContextMenu={openContext}>
      <PopupMenu id="bookmark-folder-menu-items" label={folder?.title ?? "Bookmarks"}>
        {path.length > 1 ? (
          <>
            <div id="bookmark-folder-menu-up-target" onDragOver={hoverBack} onDragLeave={leaveBack} onDrop={cancelBack}>
              <PopupMenuItem
                id="bookmark-folder-menu-up"
                icon={<ArrowLeft />}
                label={folder?.title ?? "Back"}
                onClick={() => setPath((current) => current.slice(0, -1))}
              />
            </div>
            <PopupMenuSeparator id="bookmark-folder-menu-up-separator" />
          </>
        ) : null}
        <div
          id="bookmark-folder-menu-list"
          className="flex min-h-8 flex-col"
          onDragOver={drag.listProps.onDragOver}
          onDragLeave={drag.listProps.onDragLeave}
          onDrop={drag.listProps.onDrop}
        >
          {children.length === 0 ? (
            <div id="bookmark-folder-menu-empty" className="px-3 py-2 text-sm text-zinc-500">
              This folder is empty.
            </div>
          ) : null}
          {children.map((node, index) =>
            node.separator ? (
              <div
                key={node.id}
                id={`bookmark-folder-menu-${node.id}`}
                role="separator"
                className={cn("flex h-3 shrink-0 items-center px-3", drag.dropChrome(node.id))}
                onContextMenu={(event) => openContext(event, node)}
                {...drag.itemProps(node, index)}
              >
                <div id={`bookmark-folder-menu-${node.id}-line`} className="h-px w-full bg-zinc-600" />
              </div>
            ) : node.url ? (
              <button
                key={node.id}
                id={`bookmark-folder-menu-${node.id}`}
                type="button"
                role="menuitem"
                title={node.url}
                className={cn(rowClass, drag.dropChrome(node.id))}
                style={rowStyle}
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
                  id={`bookmark-folder-menu-${node.id}-icon`}
                  url={node.url}
                  src={icons[faviconKey(node.url) ?? ""] ?? null}
                  className={rowIconClass}
                />
                <span id={`bookmark-folder-menu-${node.id}-title`} className="min-w-0 flex-1 truncate">
                  {node.title || node.url}
                </span>
              </button>
            ) : (
              <button
                key={node.id}
                id={`bookmark-folder-menu-${node.id}`}
                type="button"
                role="menuitem"
                title={node.title}
                className={cn(rowClass, drag.dropChrome(node.id))}
                style={rowStyle}
                onContextMenu={(event) => openContext(event, node)}
                onClick={() => enterFolder(node)}
                {...drag.itemProps(node, index)}
              >
                <Folder id={`bookmark-folder-menu-${node.id}-icon`} className={cn(rowIconClass, "text-amber-400")} aria-hidden />
                <span id={`bookmark-folder-menu-${node.id}-title`} className="min-w-0 flex-1 truncate">
                  {node.title}
                </span>
              </button>
            ),
          )}
          {drag.drop?.id === BOOKMARK_DROP_END ? (
            <div id="bookmark-folder-menu-drop-end" className="mx-3 my-0.5 h-0.5 shrink-0 rounded bg-sky-500" />
          ) : null}
        </div>
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
          id="bookmark-folder-menu-open-group"
          icon={<Group />}
          label="Open all as a tab group"
          disabled={links.length === 0}
          onClick={() => void openFolderAsGroup(currentId).finally(() => closePopup())}
        />
      </PopupMenu>
      {context ? (
        <div
          id="bookmark-folder-context"
          className="fixed z-50 w-60 overflow-hidden rounded-xl bg-zinc-900 shadow-2xl ring-1 ring-inset ring-zinc-700"
          style={{
            left: Math.max(8, Math.min(context.x, window.innerWidth - 248)),
            top: Math.max(8, Math.min(context.y, window.innerHeight - (context.target ? 204 : 156))),
          }}
        >
          <BookmarksBarMenuItems
            idPrefix="bookmark-folder-context"
            parentId={currentId}
            divider="horizontal"
            x={context.windowX}
            y={context.windowY}
            target={context.target}
            onDone={() => setContext(null)}
          />
        </div>
      ) : null}
      </div>
    </PopupFrame>
  );
}
