import { useEffect, useRef, useState } from "react";
import { onConsoleCleared, onConsoleMessage, onConsoleReset } from "@/lib/browser-client";
import { useTabs } from "@/lib/use-browser";
import { bucket } from "./log";

function pageKey(url: string) {
  try {
    const parsed = new URL(url);
    return `${parsed.origin}${parsed.pathname}${parsed.search}`;
  } catch {
    return url;
  }
}

function zeroCount(current: Record<number, number>, tabId: number) {
  if (!current[tabId]) {
    return current;
  }
  return { ...current, [tabId]: 0 };
}

/** Errors and failed asserts for the active tab while the console is listening. */
export function useConsoleErrorCount() {
  const { activeId, tabs } = useTabs();
  const [counts, setCounts] = useState<Record<number, number>>({});
  const pages = useRef(new Map<number, string>());
  const watching = useRef<number | null>(null);

  useEffect(() => {
    let alive = true;
    const pending = Promise.all([
      onConsoleMessage((message) => {
        if (!alive || bucket(message.level) !== "error") {
          return;
        }
        setCounts((current) => ({
          ...current,
          [message.tabId]: (current[message.tabId] ?? 0) + 1,
        }));
      }),
      onConsoleCleared((message) => {
        if (!alive) {
          return;
        }
        setCounts((current) => zeroCount(current, message.tabId));
      }),
      onConsoleReset((message) => {
        if (!alive) {
          return;
        }
        setCounts((current) => zeroCount(current, message.tabId));
      }),
    ]);
    return () => {
      alive = false;
      void pending.then((stops) => {
        for (const stop of stops) stop();
      });
    };
  }, []);

  useEffect(() => {
    const previousId = watching.current;
    watching.current = activeId;
    if (activeId == null) {
      return;
    }
    const url = tabs.find((tab) => tab.id === activeId)?.url ?? "";
    const key = pageKey(url);
    const previous = pages.current.get(activeId);
    pages.current.set(activeId, key);
    if (previousId === activeId || previous === undefined || previous === key) {
      return;
    }
    setCounts((current) => zeroCount(current, activeId));
  }, [activeId, tabs]);

  return activeId == null ? 0 : (counts[activeId] ?? 0);
}
