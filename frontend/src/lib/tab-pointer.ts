import { useSyncExternalStore } from "react";

export type TabUnderPointer = {
  tabId: number | null;
  /** The tab slid under a still pointer, so the close button is shown at full size. */
  instant: boolean;
};

const EMPTY: TabUnderPointer = { tabId: null, instant: false };

let point: { x: number; y: number } | null = null;
let current: TabUnderPointer = EMPTY;
let transitioning = false;
let listening = false;
const listeners = new Set<() => void>();

function emit(next: TabUnderPointer) {
  if (next.tabId === current.tabId && next.instant === current.instant) {
    return;
  }
  current = next;
  for (const listener of listeners) {
    listener();
  }
}

/** The tab at the pointer, including one covered by a view-transition snapshot. */
function tabAt(x: number, y: number) {
  const stack = document.elementsFromPoint(x, y);
  for (const node of stack) {
    if (!(node instanceof Element)) {
      continue;
    }
    const tab = node.closest("[data-tab-id]");
    if (!(tab instanceof HTMLElement)) {
      continue;
    }
    const id = Number(tab.dataset.tabId);
    if (Number.isInteger(id)) {
      return id;
    }
  }
  return null;
}

function remember(event: PointerEvent) {
  point = { x: event.clientX, y: event.clientY };
}

function onPointerMove(event: PointerEvent) {
  remember(event);
  const tabId = tabAt(event.clientX, event.clientY);
  // A view-transition snapshot sits above the tabs and is not a tab. Keep the tab
  // that already landed under the pointer until the animation ends.
  if (tabId === null && transitioning) {
    return;
  }
  emit({ tabId, instant: false });
}

function onPointerOut(event: PointerEvent) {
  if (event.relatedTarget !== null) {
    return;
  }
  point = null;
  emit(EMPTY);
}

function ensure() {
  if (listening || typeof window === "undefined") {
    return;
  }
  listening = true;
  window.addEventListener("pointermove", onPointerMove, { passive: true });
  window.addEventListener("pointerdown", remember, { passive: true });
  window.addEventListener("pointerout", onPointerOut);
}

/** True while tab positions are sliding. Pointer moves then must not hide the close button. */
export function markTabTransition(active: boolean) {
  transitioning = active;
}

/**
 * The pointer did not move, but a tab may have slid under it. Shows that tab's close
 * button immediately so another click closes it.
 */
export function refreshTabUnderPointer() {
  if (!point) {
    return;
  }
  emit({ tabId: tabAt(point.x, point.y), instant: true });
}

export function useTabUnderPointer() {
  ensure();
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    () => current,
    () => EMPTY,
  );
}
