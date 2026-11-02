import { useEffect, useState } from "react";
import type { TabFailure, TabInfo } from "@type/browser";
import { onStatusChanged } from "./browser-client";
import { formatBytes } from "./format";

/** Working set or JavaScript heap above this is marked on the tab. */
export const HIGH_TAB_MEMORY = 800 * 1024 * 1024;

const MEMORY_STEP = 32 * 1024 * 1024;

export type HeavyTab = {
  bytes: number;
  shared: boolean;
};

type ProblemTab = Pick<TabInfo, "failure" | "errors" | "unresponsive">;

function failureReason(failure: TabFailure) {
  switch (failure.kind) {
    case "crashed":
      return "Page crashed";
    case "offline":
      return "Page is offline";
    case "dns":
      return "Site not found";
    case "certificate":
      return "Certificate error";
    case "timeout":
      return "Page timed out";
    case "refused":
      return "Connection refused";
    default:
      return "Page failed to load";
  }
}

/** Why a tab should show a red dot. Empty when the tab is fine. */
export function tabProblemTitle(tab: ProblemTab, heavy: HeavyTab | null) {
  const reasons: string[] = [];
  if (tab.unresponsive) {
    reasons.push("Page isn't responding");
  }
  if (tab.failure) {
    reasons.push(failureReason(tab.failure));
  }
  if (tab.errors > 0) {
    reasons.push(tab.errors === 1 ? "1 page error" : `${tab.errors} page errors`);
  }
  if (heavy) {
    const amount = formatBytes(heavy.bytes);
    reasons.push(heavy.shared ? `Using ${amount}, shared with other tabs` : `Using ${amount}`);
  }
  return reasons.length > 0 ? reasons.join(" · ") : null;
}

/** Problem text for tabs a collapsed group is hiding. The active tab stays in the strip. */
export function groupProblemTitle(tabs: TabInfo[], heavy: ReadonlyMap<number, HeavyTab>, activeId: number | null) {
  const notes = tabs.flatMap((tab) => {
    if (tab.id === activeId) {
      return [];
    }
    const problem = tabProblemTitle(tab, heavy.get(tab.id) ?? null);
    return problem ? [`${tab.title || tab.url}: ${problem}`] : [];
  });
  if (notes.length === 0) {
    return null;
  }
  if (notes.length <= 3) {
    return notes.join("\n");
  }
  return `${notes.length} tabs need attention`;
}

let heavyTabs: Map<number, HeavyTab> = new Map();
const heavyListeners = new Set<(value: Map<number, HeavyTab>) => void>();
let heavyStarted = false;

function sameHeavy(left: Map<number, HeavyTab>, right: Map<number, HeavyTab>) {
  if (left.size !== right.size) {
    return false;
  }
  for (const [id, item] of right) {
    const previous = left.get(id);
    if (!previous || previous.bytes !== item.bytes || previous.shared !== item.shared) {
      return false;
    }
  }
  return true;
}

function watchHeavyTabs() {
  if (heavyStarted) {
    return;
  }
  heavyStarted = true;
  void onStatusChanged((status) => {
    const next = new Map<number, HeavyTab>();
    for (const page of status.pages) {
      const bytes = page.tabUsed ?? page.tabHeap;
      if (bytes === null || bytes < HIGH_TAB_MEMORY) {
        continue;
      }
      const stepped = Math.round(bytes / MEMORY_STEP) * MEMORY_STEP;
      next.set(page.id, { bytes: Math.max(stepped, HIGH_TAB_MEMORY), shared: page.tabShared });
    }
    if (sameHeavy(heavyTabs, next)) {
      return;
    }
    heavyTabs = next;
    for (const listener of heavyListeners) {
      listener(heavyTabs);
    }
  });
}

/** Tabs whose renderer or heap is over the mark. One listener per webview. */
export function useHeavyTabs() {
  const [value, setValue] = useState(heavyTabs);
  useEffect(() => {
    watchHeavyTabs();
    const listener = (next: Map<number, HeavyTab>) => setValue(next);
    heavyListeners.add(listener);
    listener(heavyTabs);
    return () => {
      heavyListeners.delete(listener);
    };
  }, []);
  return value;
}
