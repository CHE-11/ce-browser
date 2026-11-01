import { useEffect, useRef, useState, type DragEvent } from "react";
import type { BookmarkNode } from "../../../types/browser";
import { moveBookmark } from "./browser-client";
import { BOOKMARK_DRAG_TYPE, bookmarkDropPlace, type BookmarkDropPlace } from "./use-browser";

/** Drop target id for the empty space after the last item. */
export const BOOKMARK_DROP_END = "__end__";

export type BookmarkDropState = { id: string; place: BookmarkDropPlace };

type BookmarkDragOptions = {
  /** Folder whose children are listed. */
  parentId: string;
  axis: "x" | "y";
  /** A drag rests on a folder's middle long enough to open it. */
  onHoverFolder?: (node: BookmarkNode, element: HTMLElement) => void;
  hoverDelay?: number;
  /** A bookmark was dropped in this list. */
  onDropped?: () => void;
  /** A drag that started in this list ended, wherever it landed. */
  onDragEnd?: () => void;
};

export function isBookmarkDrag(event: DragEvent) {
  return event.dataTransfer.types.includes(BOOKMARK_DRAG_TYPE);
}

function isFolder(node: BookmarkNode) {
  return !node.url && !node.separator;
}

/** Drag to reorder the children of one bookmark folder, or to drop them into a subfolder. */
export function useBookmarkDrag({ parentId, axis, onHoverFolder, hoverDelay = 500, onDropped, onDragEnd }: BookmarkDragOptions) {
  const [drop, setDrop] = useState<BookmarkDropState | null>(null);
  const dragged = useRef(false);
  /** Item this list is dragging. `getData` is empty during `dragover`, so it is kept here. */
  const draggingId = useRef<string | null>(null);
  const hover = useRef<{ id: string; timer: number } | null>(null);
  const hoverCallback = useRef(onHoverFolder);
  hoverCallback.current = onHoverFolder;

  const cancelHover = () => {
    if (hover.current) {
      window.clearTimeout(hover.current.timer);
      hover.current = null;
    }
  };

  useEffect(() => cancelHover, []);

  const showDrop = (next: BookmarkDropState | null) => {
    setDrop((current) => {
      if (current === next || (current && next && current.id === next.id && current.place === next.place)) {
        return current;
      }
      return next;
    });
  };

  const trackHover = (node: BookmarkNode, place: BookmarkDropPlace, element: HTMLElement) => {
    if (place !== "into" || !hoverCallback.current || node.id === draggingId.current) {
      cancelHover();
      return;
    }
    if (hover.current?.id === node.id) {
      return;
    }
    cancelHover();
    const timer = window.setTimeout(() => {
      hover.current = null;
      hoverCallback.current?.(node, element);
    }, hoverDelay);
    hover.current = { id: node.id, timer };
  };

  const reset = () => {
    cancelHover();
    showDrop(null);
  };

  const finish = (move: Promise<void>) => {
    reset();
    void move.catch(() => undefined);
    onDropped?.();
  };

  /** Drop handlers for `node`, which sits at `index` in the list. Root folders only accept drops inside. */
  const targetProps = (node: BookmarkNode, index: number) => ({
    onDragOver: (event: DragEvent<HTMLElement>) => {
      if (!isBookmarkDrag(event)) {
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      event.dataTransfer.dropEffect = "move";
      const place = bookmarkDropPlace(event, node, axis);
      showDrop({ id: node.id, place });
      trackHover(node, place, event.currentTarget);
    },
    onDrop: (event: DragEvent<HTMLElement>) => {
      if (!isBookmarkDrag(event)) {
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      const id = event.dataTransfer.getData(BOOKMARK_DRAG_TYPE);
      const place = bookmarkDropPlace(event, node, axis);
      if (!id || id === node.id) {
        reset();
        return;
      }
      if (place === "into" && isFolder(node)) {
        finish(moveBookmark(id, node.id));
        return;
      }
      finish(moveBookmark(id, parentId, place === "after" ? index + 1 : index));
    },
  });

  /** Drag source and drop target props for one item of the list. */
  const itemProps = (node: BookmarkNode, index: number) => ({
    ...targetProps(node, index),
    draggable: true,
    onDragStart: (event: DragEvent) => {
      dragged.current = true;
      draggingId.current = node.id;
      event.dataTransfer.setData(BOOKMARK_DRAG_TYPE, node.id);
      if (node.url) {
        event.dataTransfer.setData("text/uri-list", node.url);
        event.dataTransfer.setData("text/plain", node.url);
        event.dataTransfer.effectAllowed = "copyMove";
      } else {
        event.dataTransfer.effectAllowed = "move";
      }
    },
    onDragEnd: () => {
      draggingId.current = null;
      reset();
      onDragEnd?.();
      window.setTimeout(() => {
        dragged.current = false;
      });
    },
  });

  /** Spread on the list itself: dropping on its empty space appends to the folder. */
  const listProps = {
    onDragOver: (event: DragEvent<HTMLElement>) => {
      if (!isBookmarkDrag(event)) {
        return;
      }
      event.preventDefault();
      event.dataTransfer.dropEffect = "move";
      cancelHover();
      showDrop({ id: BOOKMARK_DROP_END, place: "after" });
    },
    onDragLeave: (event: DragEvent<HTMLElement>) => {
      const next = event.relatedTarget;
      if (next instanceof Node && event.currentTarget.contains(next)) {
        return;
      }
      reset();
    },
    onDrop: (event: DragEvent<HTMLElement>) => {
      if (!isBookmarkDrag(event)) {
        return;
      }
      event.preventDefault();
      const id = event.dataTransfer.getData(BOOKMARK_DRAG_TYPE);
      if (!id) {
        reset();
        return;
      }
      finish(moveBookmark(id, parentId));
    },
  };

  /** Insert line or ring for the item under the pointer. */
  const dropChrome = (id: string) => {
    if (!drop || drop.id !== id) {
      return "";
    }
    if (drop.place === "into") {
      return "ring-1 ring-inset ring-sky-500";
    }
    if (axis === "x") {
      return drop.place === "before" ? "shadow-[inset_2px_0_0_0_#0ea5e9]" : "shadow-[inset_-2px_0_0_0_#0ea5e9]";
    }
    return drop.place === "before" ? "shadow-[inset_0_2px_0_0_#0ea5e9]" : "shadow-[inset_0_-2px_0_0_#0ea5e9]";
  };

  return { drop, dragged, itemProps, targetProps, listProps, dropChrome, reset };
}
