import { useRef, useState, type DragEvent } from "react";
import type { TabInfo } from "../../../types/browser";
import { detachTab, moveTab, windowOuterBounds } from "./browser-client";

/**
 * Drag to reorder tabs along `axis`. A tab dropped outside the window moves into a new window.
 * Indexes are strip positions, the way `moveTab` counts them.
 */
export function useTabDrag(tabs: TabInfo[], axis: "x" | "y") {
  const [dragging, setDragging] = useState<number | null>(null);
  const [dropIndex, setDropIndex] = useState<number | null>(null);
  const droppedInside = useRef(false);
  const windowBounds = useRef<{ x: number; y: number; width: number; height: number } | null>(null);

  const start = (id: number) => {
    droppedInside.current = false;
    windowBounds.current = null;
    setDragging(id);
    void windowOuterBounds().then((bounds) => {
      windowBounds.current = bounds;
    });
  };

  const over = (event: DragEvent, index: number) => {
    if (dragging === null) {
      return;
    }
    event.preventDefault();
    const rect = event.currentTarget.getBoundingClientRect();
    const after = axis === "x" ? event.clientX > rect.left + rect.width / 2 : event.clientY > rect.top + rect.height / 2;
    setDropIndex(after ? index + 1 : index);
  };

  const drop = (event: DragEvent) => {
    event.preventDefault();
    droppedInside.current = true;
    if (dragging !== null && dropIndex !== null) {
      const from = tabs.findIndex((tab) => tab.id === dragging);
      const target = from >= 0 && from < dropIndex ? dropIndex - 1 : dropIndex;
      void moveTab(dragging, target);
    }
    setDragging(null);
    setDropIndex(null);
  };

  const end = (event: DragEvent) => {
    const id = dragging;
    const landedInside = droppedInside.current;
    droppedInside.current = false;
    setDragging(null);
    setDropIndex(null);
    if (id === null || landedInside) {
      return;
    }
    const bounds = windowBounds.current;
    if (!bounds) {
      return;
    }
    const outside =
      event.screenX < bounds.x ||
      event.screenY < bounds.y ||
      event.screenX > bounds.x + bounds.width ||
      event.screenY > bounds.y + bounds.height;
    if (outside) {
      void detachTab(id);
    }
  };

  /** Lets the list accept a drop between tabs. */
  const containerProps = {
    onDragOver: (event: DragEvent) => {
      if (dragging !== null) {
        event.preventDefault();
      }
    },
    onDrop: drop,
  };

  return { dragging, dropIndex, start, over, end, containerProps };
}
