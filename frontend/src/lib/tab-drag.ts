import { useEffect, useRef, useState, type DragEvent } from "react";
import type { TabInfo, TabStripLayout, TabStripSlot } from "../../../types/browser";
import {
  currentWindowLabel,
  dropTab,
  forgetTabStrip,
  moveTab,
  onTabDragHover,
  onWindowGeometry,
  reportTabStrip,
  sendTabDragHover,
  windowClientOrigin,
} from "./browser-client";

/** Drag payload other windows can recognize. Pages do not ask for this type. */
export const TAB_DRAG_TYPE = "application/x-ce-tab";

type Slot = { index: number; start: number; end: number };

export function isTabDrag(data: DataTransfer | null) {
  return data?.types.includes(TAB_DRAG_TYPE) ?? false;
}

/** Visible tabs, in strip order, measured in this webview's CSS pixels. */
function tabSlots(root: HTMLElement, axis: "x" | "y", tabs: TabInfo[]): Slot[] {
  const indexOf = new Map(tabs.map((tab, index) => [tab.id, index]));
  const slots: Slot[] = [];
  for (const item of root.querySelectorAll<HTMLElement>("[data-tab-id]")) {
    const index = indexOf.get(Number(item.dataset.tabId));
    if (index === undefined) {
      continue;
    }
    const box = item.getBoundingClientRect();
    slots.push({
      index,
      start: axis === "x" ? box.left : box.top,
      end: axis === "x" ? box.right : box.bottom,
    });
  }
  slots.sort((left, right) => left.start - right.start);
  return slots;
}

/** Insertion index under a point already in the strip, using each tab's midpoint. */
function indexFromSlots(slots: Slot[], along: number) {
  for (const slot of slots) {
    if (along < (slot.start + slot.end) / 2) {
      return slot.index;
    }
  }
  const last = slots[slots.length - 1];
  return last ? last.index + 1 : 0;
}

/** Strip index under a client point, or null when the point misses `root`. */
function indexOnStrip(root: HTMLElement, axis: "x" | "y", clientX: number, clientY: number, tabs: TabInfo[]) {
  const rect = root.getBoundingClientRect();
  if (clientX < rect.left || clientX > rect.right || clientY < rect.top || clientY > rect.bottom) {
    return null;
  }
  return indexFromSlots(tabSlots(root, axis, tabs), axis === "x" ? clientX : clientY);
}

function screenLayout(root: HTMLElement, axis: "x" | "y", originX: number, originY: number, tabs: TabInfo[]): TabStripLayout {
  const rect = root.getBoundingClientRect();
  const along = axis === "x" ? originX : originY;
  const slots: TabStripSlot[] = tabSlots(root, axis, tabs).map((slot) => ({
    index: slot.index,
    start: slot.start + along,
    end: slot.end + along,
  }));
  return {
    axis,
    x: originX + rect.left,
    y: originY + rect.top,
    width: rect.width,
    height: rect.height,
    slots,
  };
}

/**
 * Drag to reorder tabs along `axis`. Releasing anywhere but on a tab strip opens a new window.
 * Releasing on another window's strip moves the tab into that window.
 * Indexes are strip positions, the way `moveTab` counts them.
 */
export function useTabDrag(tabs: TabInfo[], axis: "x" | "y", rootId: string) {
  const [dragging, setDragging] = useState<number | null>(null);
  const [dropIndex, setDropIndex] = useState<number | null>(null);
  const droppedInside = useRef(false);
  const cancelled = useRef(false);
  const draggingRef = useRef<number | null>(null);
  const tabsRef = useRef(tabs);
  const originRef = useRef<{ x: number; y: number } | null>(null);
  /** screenX minus the client-area origin, measured once while the pointer is still inside. */
  const align = useRef<{ x: number; y: number } | null>(null);
  const token = useRef(0);
  const hoverFrame = useRef(0);
  const hoverPoint = useRef<{ x: number; y: number } | null>(null);
  const stopKeys = useRef<(() => void) | null>(null);
  tabsRef.current = tabs;
  const orderKey = tabs.map((tab) => tab.id).join(",");

  useEffect(() => {
    const root = document.getElementById(rootId);
    if (!root) {
      return;
    }
    let stopped = false;
    let generation = 0;
    const report = () => {
      const ticket = ++generation;
      void windowClientOrigin().then((origin) => {
        if (stopped || ticket !== generation) {
          return;
        }
        originRef.current = origin;
        const current = document.getElementById(rootId);
        if (!current) {
          return;
        }
        const rect = current.getBoundingClientRect();
        if (rect.width < 1 || rect.height < 1) {
          return;
        }
        void reportTabStrip(currentWindowLabel(), screenLayout(current, axis, origin.x, origin.y, tabsRef.current));
      });
    };
    report();
    const resize = new ResizeObserver(report);
    resize.observe(root);
    root.addEventListener("scroll", report, { capture: true, passive: true });
    let unlistenGeometry: (() => void) | undefined;
    let geometryGone = false;
    void onWindowGeometry(report).then((unlisten) => {
      if (geometryGone) {
        unlisten();
      } else {
        unlistenGeometry = unlisten;
      }
    });
    return () => {
      stopped = true;
      geometryGone = true;
      unlistenGeometry?.();
      resize.disconnect();
      root.removeEventListener("scroll", report, true);
      void forgetTabStrip(currentWindowLabel());
    };
  }, [axis, orderKey, rootId]);

  useEffect(() => {
    let seen = 0;
    let unlisten: (() => void) | undefined;
    let gone = false;
    void onTabDragHover((hover) => {
      if (hover.token < seen) {
        return;
      }
      seen = hover.token;
      if (draggingRef.current !== null) {
        return;
      }
      if (hover.id === null) {
        setDropIndex(null);
        return;
      }
      const root = document.getElementById(rootId);
      const origin = originRef.current;
      if (!root || !origin) {
        return;
      }
      const index = indexOnStrip(root, axis, hover.x - origin.x, hover.y - origin.y, tabsRef.current);
      setDropIndex(index);
    }).then((fn) => {
      if (gone) {
        fn();
      } else {
        unlisten = fn;
      }
    });
    return () => {
      gone = true;
      unlisten?.();
    };
  }, [axis, rootId]);

  useEffect(() => {
    return () => {
      stopKeys.current?.();
      if (hoverFrame.current) {
        cancelAnimationFrame(hoverFrame.current);
      }
      if (draggingRef.current !== null) {
        draggingRef.current = null;
        token.current += 1;
        void sendTabDragHover({ id: null, x: 0, y: 0, token: token.current });
      }
    };
  }, []);

  const publishHover = (x: number, y: number) => {
    if (x === 0 && y === 0) {
      return;
    }
    hoverPoint.current = { x, y };
    if (hoverFrame.current) {
      return;
    }
    hoverFrame.current = requestAnimationFrame(() => {
      hoverFrame.current = 0;
      const point = hoverPoint.current;
      const id = draggingRef.current;
      if (!point || id === null) {
        return;
      }
      token.current += 1;
      void sendTabDragHover({ id, x: point.x, y: point.y, token: token.current });
    });
  };

  const finishHover = () => {
    if (hoverFrame.current) {
      cancelAnimationFrame(hoverFrame.current);
      hoverFrame.current = 0;
    }
    hoverPoint.current = null;
    token.current += 1;
    void sendTabDragHover({ id: null, x: 0, y: 0, token: token.current });
  };

  const start = (id: number) => {
    droppedInside.current = false;
    cancelled.current = false;
    align.current = null;
    draggingRef.current = id;
    setDragging(id);
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        cancelled.current = true;
      }
    };
    document.addEventListener("keydown", onKey);
    stopKeys.current = () => document.removeEventListener("keydown", onKey);
  };

  /** Drag `screenX` mapped into the same CSS space as the reported strip rectangles. */
  const screenPoint = (event: DragEvent) => {
    const origin = originRef.current;
    if (origin && align.current === null && (event.screenX !== 0 || event.screenY !== 0)) {
      align.current = {
        x: event.screenX - event.clientX - origin.x,
        y: event.screenY - event.clientY - origin.y,
      };
    }
    const shift = align.current ?? { x: 0, y: 0 };
    return { x: event.screenX - shift.x, y: event.screenY - shift.y };
  };

  const track = (event: DragEvent) => {
    if (draggingRef.current === null) {
      return;
    }
    const root = document.getElementById(rootId);
    if (root) {
      setDropIndex(indexOnStrip(root, axis, event.clientX, event.clientY, tabsRef.current));
    }
    const point = screenPoint(event);
    publishHover(point.x, point.y);
  };

  const over = (event: DragEvent, index: number) => {
    if (draggingRef.current === null && !isTabDrag(event.dataTransfer)) {
      return;
    }
    event.preventDefault();
    event.dataTransfer.dropEffect = "move";
    const rect = event.currentTarget.getBoundingClientRect();
    const after = axis === "x" ? event.clientX > rect.left + rect.width / 2 : event.clientY > rect.top + rect.height / 2;
    setDropIndex(after ? index + 1 : index);
  };

  const drop = (event: DragEvent) => {
    if (draggingRef.current === null) {
      if (isTabDrag(event.dataTransfer)) {
        event.preventDefault();
      }
      return;
    }
    event.preventDefault();
    droppedInside.current = true;
    const id = draggingRef.current;
    if (dropIndex !== null) {
      const from = tabs.findIndex((tab) => tab.id === id);
      const target = from >= 0 && from < dropIndex ? dropIndex - 1 : dropIndex;
      void moveTab(id, target);
    }
    draggingRef.current = null;
    setDragging(null);
    setDropIndex(null);
  };

  const end = (event: DragEvent) => {
    const id = draggingRef.current ?? dragging;
    const landedInside = droppedInside.current;
    const aborted = cancelled.current;
    const point = event.screenX === 0 && event.screenY === 0 ? hoverPoint.current : screenPoint(event);
    droppedInside.current = false;
    cancelled.current = false;
    draggingRef.current = null;
    stopKeys.current?.();
    stopKeys.current = null;
    setDragging(null);
    setDropIndex(null);
    finishHover();
    if (id === null || landedInside || aborted || !point) {
      return;
    }
    void dropTab(id, point.x, point.y);
  };

  /** Lets the list accept a drop between tabs, including a tab dragged from another window. */
  const containerProps = {
    onDragOver: (event: DragEvent) => {
      if (draggingRef.current !== null || isTabDrag(event.dataTransfer)) {
        event.preventDefault();
        event.dataTransfer.dropEffect = "move";
      }
    },
    onDrop: drop,
  };

  return { dragging, dropIndex, start, track, over, end, containerProps };
}
