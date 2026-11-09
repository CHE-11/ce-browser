import { useLayoutEffect, useReducer, useRef } from "react";
import { flushSync } from "react-dom";
import type { TabGroup, TabInfo, SplitView } from "@type/browser";
import { markTabTransition, refreshTabUnderPointer } from "./tab-pointer";

type TabLayoutSource = {
  tabs: TabInfo[];
  groups: TabGroup[];
  split: SplitView | null;
  workspace: string;
};

/** What decides where tabs sit. A title or favicon change is not a layout change. */
function layoutKey(snapshot: TabLayoutSource) {
  const tabs = snapshot.tabs.map((tab) => `${tab.id}:${tab.pinned ? "p" : ""}${tab.groupId ?? ""}`).join(",");
  const groups = snapshot.groups.map((group) => `${group.id}${group.collapsed ? "c" : ""}`).join(",");
  const split = snapshot.split ? `${snapshot.split.left}|${snapshot.split.right}` : "";
  return `${snapshot.workspace};${tabs};${groups};${split}`;
}

export function motionAllowed() {
  return (
    typeof document.startViewTransition === "function" &&
    document.visibilityState === "visible" &&
    !window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

/**
 * Animates tabs opening, closing, moving and groups collapsing with a view transition. While a
 * layout change is waiting for its transition the previous snapshot is returned, so the browser
 * can capture the old positions first. `paused` turns motion off, for example during a drag.
 */
export function useTabMotion<T extends TabLayoutSource>(snapshot: T, paused: boolean): T {
  const key = layoutKey(snapshot);
  const shown = useRef({ key, snapshot });
  const latest = useRef(snapshot);
  const pending = useRef(false);
  const motion = useRef(0);
  const [, rerender] = useReducer((count: number) => count + 1, 0);
  latest.current = snapshot;
  const changed = key !== shown.current.key;
  const animate = changed && (pending.current || (!paused && motionAllowed()));

  useLayoutEffect(() => {
    if (!changed) {
      shown.current = { key, snapshot };
      return;
    }
    if (!animate) {
      shown.current = { key, snapshot };
      refreshTabUnderPointer();
      return;
    }
    if (pending.current) {
      return;
    }
    pending.current = true;
    const ticket = ++motion.current;
    markTabTransition(true);
    const transition = document.startViewTransition(() => {
      flushSync(() => {
        pending.current = false;
        shown.current = { key: layoutKey(latest.current), snapshot: latest.current };
        rerender();
      });
      // Capture the close button on the tab that now sits under the pointer,
      // before the slide starts, so it is visible when the tab arrives.
      flushSync(() => {
        refreshTabUnderPointer();
      });
    });
    void transition.finished.finally(() => {
      if (ticket !== motion.current) {
        return;
      }
      markTabTransition(false);
      refreshTabUnderPointer();
    });
  });

  return animate ? shown.current.snapshot : snapshot;
}
