import { useLayoutEffect, useRef, useState, type DragEvent, type MouseEvent, type RefObject } from "react";
import { flushSync } from "react-dom";
import { ChevronsRight, Folder } from "lucide-react";
import { BookmarkDivider } from "@/components/BookmarkDivider";
import { FavIcon } from "@/components/FavIcon";
import { closePopup, createTab, moveBookmark, navigate, openPopup } from "@/lib/browser-client";
import { BOOKMARK_DROP_END, isBookmarkDrag, useBookmarkDrag } from "@/lib/bookmark-drag";
import { bookmarkDividerKey, type BookmarkDividerMetrics } from "@/lib/bookmark-divider";
import { BOOKMARK_SIZE, bookmarkItemStyle, bookmarkLabelStyle } from "@/lib/bookmark-size";
import { bookmarksBarAlign, bookmarksBarAlignClass } from "@/lib/bookmarks-bar-align";
import { CHROME, metric, remBox } from "@/lib/chrome-metrics";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import { faviconKey } from "@/lib/format";
import { BOOKMARK_DRAG_TYPE, useSiteFavicons } from "@/lib/use-browser";
import { BookmarkTree, BookmarkNode, type BookmarksBarAlign } from "@type/browser";
import { setDividerPlace } from "./BookmarksBarContextMenu";

type BookmarksBarProps = {
  tree: BookmarkTree | null;
  activeId: number | null;
  /** The selected tab is pinned, so a bookmark opens in a new tab beside it. */
  activePinned?: boolean;
  /** rem */
  textSize?: number;
  /** rem */
  padding?: number;
  /** rem */
  iconSize?: number;
  /** rem */
  height?: number;
  /** Off hides folder icons. A folder without a title keeps its icon so it stays visible. */
  folderIcons?: boolean;
  /** Where the folders and bookmarks sit in the bar. */
  align?: BookmarksBarAlign | string | null;
  /** Vertical line between bar items. */
  divider: BookmarkDividerMetrics;
};

/** Gap, icon and width are in `em`, so they follow the text size setting. */
const itemClass = cn(
  "inline-flex max-w-[15em] shrink-0 items-center gap-[0.5em] rounded-md text-zinc-300 hover:bg-zinc-800 hover:text-zinc-100 max-w-fit",
  focusRing,
);
const iconClass = "shrink-0";

function itemSignature(node: BookmarkNode) {
  return `${node.id}\0${node.title}\0${node.url ?? ""}\0${node.separator ? 1 : 0}`;
}

/** How many leading widths fit in `available`, including the gap before each item after the first. */
function fitCount(widths: number[], available: number, gap: number) {
  let used = 0;
  for (let index = 0; index < widths.length; index += 1) {
    const next = used + (index > 0 ? gap : 0) + widths[index];
    if (next > available + 0.5) {
      return index;
    }
    used = next;
  }
  return widths.length;
}

/** Width of the items slot with the chevron shown or hidden. Restores the chevron before returning. */
function slotWidth(slot: HTMLElement, chevron: HTMLButtonElement, showChevron: boolean) {
  const wasHidden = chevron.hidden;
  chevron.hidden = !showChevron;
  const width = slot.clientWidth;
  chevron.hidden = wasHidden;
  return width;
}

/**
 * How many bar items fit in the slot. Item width does not depend on the slot (`shrink-0`), so
 * widths are cached until the bookmarks or their text size change. The chevron stays mounted
 * and is hidden until a suffix does not fit; measuring it toggles that attribute before paint.
 */
function useFittedCount(
  items: BookmarkNode[],
  textSize: number | undefined,
  padding: number | undefined,
  iconSize: number,
  height: number,
  folderIcons: boolean,
  dividerKey: string,
  dropMarker: boolean,
  slotRef: RefObject<HTMLDivElement | null>,
  chevronRef: RefObject<HTMLButtonElement | null>,
) {
  const signature = `${dividerKey}|${textSize ?? ""}|${padding ?? ""}|${iconSize}|${height}|${folderIcons ? 1 : 0}|${items.map(itemSignature).join("\n")}`;
  const widths = useRef(new Map<string, number>());
  const fontsApplied = useRef("");
  const itemsRef = useRef(items);
  const visibleRef = useRef(items.length);
  const [visibleCount, setVisibleCount] = useState(items.length);
  itemsRef.current = items;
  visibleRef.current = visibleCount;

  const seen = useRef(signature);
  if (seen.current !== signature) {
    seen.current = signature;
    widths.current.clear();
    if (visibleCount !== items.length) {
      visibleRef.current = items.length;
      setVisibleCount(items.length);
    }
  }

  useLayoutEffect(() => {
    const slot = slotRef.current;
    if (!slot) {
      return;
    }

    const setCount = (count: number, sync: boolean) => {
      if (visibleRef.current === count) {
        return;
      }
      visibleRef.current = count;
      if (sync) {
        flushSync(() => setVisibleCount(count));
      } else {
        setVisibleCount(count);
      }
    };

    const measure = (sync: boolean) => {
      const currentItems = itemsRef.current;
      const chevron = chevronRef.current;
      if (currentItems.length === 0 || !chevron) {
        setCount(0, sync);
        return;
      }
      for (const element of slot.querySelectorAll<HTMLElement>("[data-bar-item]")) {
        const id = element.dataset.barItem;
        if (!id) {
          continue;
        }
        const width = element.getBoundingClientRect().width;
        if (width > 0) {
          widths.current.set(id, width);
        }
      }
      if (currentItems.some((node) => !widths.current.has(node.id))) {
        setCount(currentItems.length, sync);
        return;
      }
      const gap = Number.parseFloat(getComputedStyle(slot).columnGap) || 0;
      const marker = slot.querySelector<HTMLElement>("#bookmarks-bar-drop-end");
      const reserved = marker ? marker.getBoundingClientRect().width + gap : 0;
      const measured = currentItems.map((node) => widths.current.get(node.id) ?? 0);
      const withoutChevron = fitCount(measured, Math.max(0, slotWidth(slot, chevron, false) - reserved), gap);
      const count =
        withoutChevron === currentItems.length
          ? currentItems.length
          : fitCount(measured, Math.max(0, slotWidth(slot, chevron, true) - reserved), gap);
      setCount(count, sync);
    };

    measure(false);
    const observer = new ResizeObserver(() => measure(true));
    observer.observe(slot);
    if (slot.parentElement) {
      observer.observe(slot.parentElement);
    }
    let alive = true;
    if (document.fonts && fontsApplied.current !== signature) {
      void document.fonts.ready.then(() => {
        if (!alive || fontsApplied.current === signature) {
          return;
        }
        fontsApplied.current = signature;
        widths.current.clear();
        measure(false);
      });
    }
    return () => {
      alive = false;
      observer.disconnect();
    };
  }, [signature, visibleCount, dropMarker, slotRef, chevronRef]);

  return Math.min(visibleCount, items.length);
}

/** Hangs the folder's menu under `element`, left edges aligned. */
function openFolderPopup(element: HTMLElement, node: BookmarkNode, dragOpened = false) {
  const params = new URLSearchParams({ id: node.id });
  if (dragOpened) {
    params.set("drag", "1");
  }
  return openPopup("bookmark-folder", element, {
    params: params.toString(),
    placement: "bottom-start",
    focus: !dragOpened,
  });
}

export function openBookmarkLink(url: string, activeId: number | null, event?: MouseEvent, pinned = false) {
  const background = !!event && (event.ctrlKey || event.button === 1);
  if (background || activeId === null || pinned) {
    void createTab(url, background);
  } else {
    void navigate(activeId, url);
  }
}

export function BookmarksBar({ tree, activeId, activePinned = false, textSize, padding, iconSize, height, folderIcons = true, align, divider }: BookmarksBarProps) {
  const items = tree?.bar.children ?? [];
  const icons = useSiteFavicons(items.map((node) => node.url));
  const icon = metric(iconSize, CHROME.bookmarksBarIconSize);
  const barHeight = metric(height, CHROME.bookmarksBarHeight);
  const iconStyle = remBox(icon);
  const itemStyle = bookmarkItemStyle(
    textSize,
    padding,
    { text: BOOKMARK_SIZE.barText, padding: BOOKMARK_SIZE.barPadding },
    { height: barHeight },
  );
  /** Folder whose popup this drag opened by hovering it. */
  const dragOpened = useRef<string | null>(null);
  const slotRef = useRef<HTMLDivElement>(null);
  const chevronRef = useRef<HTMLButtonElement>(null);

  const openFolderWhileDragging = (node: BookmarkNode, element: HTMLElement) => {
    if (node.id === "bar" || !drag.dragged.current || dragOpened.current === node.id) {
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

  const dropMarker = drag.drop?.id === BOOKMARK_DROP_END;
  const visibleCount = useFittedCount(
    items,
    textSize,
    padding,
    icon,
    barHeight,
    folderIcons,
    bookmarkDividerKey(divider),
    dropMarker,
    slotRef,
    chevronRef,
  );
  const shown = items.slice(0, visibleCount);

  const openFolder = (event: MouseEvent<HTMLElement>, node: BookmarkNode) => {
    if (drag.dragged.current) {
      drag.dragged.current = false;
      return;
    }
    void openFolderPopup(event.currentTarget, node);
  };

  const openOverflow = (event: MouseEvent<HTMLElement>) => {
    if (drag.dragged.current) {
      drag.dragged.current = false;
      return;
    }
    const ids = items.slice(visibleCount).map((node) => node.id).join(",");
    if (!ids) {
      return;
    }
    void openPopup("bookmarks-bar-overflow", event.currentTarget, {
      params: new URLSearchParams({ ids }).toString(),
      placement: "bottom-start",
    });
  };

  const openBarMenu = (event: MouseEvent, node?: BookmarkNode, index?: number) => {
    event.preventDefault();
    event.stopPropagation();
    const params = new URLSearchParams({ parent: "bar", divider: "vertical" });
    const rows = items.map((item, itemIndex) => ({
      id: item.id,
      index: itemIndex,
      elementId: `bookmarks-bar-${item.id}`,
    }));
    const hit = node && index !== undefined ? { id: node.id, index } : null;
    setDividerPlace(params, event, rows, "x", hit);
    if (node && hit) {
      const kind = node.separator ? "separator" : node.url ? "link" : "folder";
      params.set("target", node.id);
      params.set("kind", kind);
      if (kind === "link" && node.url) {
        params.set("url", node.url);
      }
    }
    void openPopup("bookmarks-bar-menu", { x: event.clientX, y: event.clientY }, { params: params.toString() });
  };

  const dropAtVisibleEnd = (event: DragEvent<HTMLDivElement>) => {
    if (!isBookmarkDrag(event)) {
      return;
    }
    event.preventDefault();
    const id = event.dataTransfer.getData(BOOKMARK_DRAG_TYPE);
    drag.reset();
    if (!id) {
      return;
    }
    void moveBookmark(id, "bar", visibleCount).catch(() => undefined);
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
        ref={slotRef}
        className={cn(
          "flex min-w-0 flex-1 items-center gap-0.5 self-stretch overflow-x-clip overflow-y-visible",
          bookmarksBarAlignClass(bookmarksBarAlign(align)),
        )}
        onDragOver={drag.listProps.onDragOver}
        onDrop={dropAtVisibleEnd}
      >
        {items.length === 0 ? (
          <span id="bookmarks-bar-empty" className="px-2 text-xs text-zinc-500">
            Press Ctrl+D to bookmark a page. Bookmarks in the bar folder show here.
          </span>
        ) : null}
        {shown.map((node, index) =>
          node.separator ? (
            <BookmarkDivider
              key={node.id}
              id={`bookmarks-bar-${node.id}`}
              data-bar-item={node.id}
              orientation="vertical"
              metrics={divider}
              className={drag.dropChrome(node.id)}
              onContextMenu={(event) => openBarMenu(event, node, index)}
              {...drag.itemProps(node, index)}
            />
          ) : node.url ? (
            <button
              key={node.id}
              id={`bookmarks-bar-${node.id}`}
              data-bar-item={node.id}
              type="button"
              title={`${node.title}\n${node.url}`}
              className={cn(itemClass, drag.dropChrome(node.id))}
              style={itemStyle}
              onContextMenu={(event) => openBarMenu(event, node, index)}
              onClick={(event) => {
                if (drag.dragged.current) {
                  drag.dragged.current = false;
                  return;
                }
                openBookmarkLink(node.url ?? "", activeId, event, activePinned);
              }}
              onAuxClick={(event) => {
                if (event.button === 1) {
                  event.preventDefault();
                  openBookmarkLink(node.url ?? "", activeId, event, activePinned);
                }
              }}
              {...drag.itemProps(node, index)}
            >
              <FavIcon
                id={`bookmarks-bar-${node.id}-icon`}
                url={node.url}
                src={icons[faviconKey(node.url) ?? ""] ?? null}
                className={iconClass}
                style={iconStyle}
              />
              <span id={`bookmarks-bar-${node.id}-title`} className="truncate" style={bookmarkLabelStyle}>
                {node.title || node.url}
              </span>
            </button>
          ) : (
            <button
              key={node.id}
              id={`bookmarks-bar-${node.id}`}
              data-bar-item={node.id}
              type="button"
              title={node.title}
              aria-haspopup="menu"
              className={cn(itemClass, drag.dropChrome(node.id))}
              style={itemStyle}
              onContextMenu={(event) => openBarMenu(event, node, index)}
              onClick={(event) => openFolder(event, node)}
              {...drag.itemProps(node, index)}
            >
              {folderIcons || !node.title ? (
                <Folder id={`bookmarks-bar-${node.id}-icon`} className={cn(iconClass, "text-amber-400")} style={iconStyle} aria-hidden />
              ) : null}
              <span id={`bookmarks-bar-${node.id}-title`} className="truncate" style={bookmarkLabelStyle}>
                {node.title}
              </span>
            </button>
          ),
        )}
        {dropMarker ? <div id="bookmarks-bar-drop-end" className="h-2/3 w-0.5 shrink-0 rounded bg-sky-500" /> : null}
      </div>
      {tree && items.length > 0 ? (
        <button
          id="bookmarks-bar-all"
          ref={chevronRef}
          type="button"
          hidden={visibleCount >= items.length}
          title="More bookmarks"
          aria-label="More bookmarks"
          aria-haspopup="menu"
          className={cn(itemClass, drag.dropChrome(tree.bar.id))}
          style={itemStyle}
          onClick={openOverflow}
          {...drag.targetProps(tree.bar, items.length)}
        >
          <ChevronsRight id="bookmarks-bar-all-icon" className={iconClass} style={iconStyle} aria-hidden />
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
          {folderIcons ? <Folder id="bookmarks-bar-other-icon" className={cn(iconClass, "text-amber-400")} style={iconStyle} aria-hidden /> : null}
          <span id="bookmarks-bar-other-title" className="truncate" style={bookmarkLabelStyle}>
            {tree.other.title || "Other bookmarks"}
          </span>
        </button>
      ) : null}
    </div>
  );
}
