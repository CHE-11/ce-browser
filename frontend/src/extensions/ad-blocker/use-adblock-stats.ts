import { useEffect, useState } from "react";
import { getAdblockStats, onAdblockStats } from "../../lib/browser-client";

/** Requests blocked in tab `tabId` since its current page started loading. Updates live. */
export function useAdblockStats(tabId: number | null) {
  return useAdblockStatsState(tabId).blocked;
}

/** Like `useAdblockStats`, plus whether the first count for `tabId` has arrived. */
export function useAdblockStatsState(tabId: number | null) {
  const [blocked, setBlocked] = useState(0);
  const [loadedFor, setLoadedFor] = useState<number | null>(null);

  useEffect(() => {
    setBlocked(0);
    if (tabId === null) {
      return;
    }
    let live = true;
    void getAdblockStats(tabId)
      .then((stats) => {
        if (live) {
          setBlocked(stats.blocked);
        }
      })
      .catch(() => undefined)
      .finally(() => {
        if (live) {
          setLoadedFor(tabId);
        }
      });
    const unlisten = onAdblockStats((stats) => {
      if (live && stats.tabId === tabId) {
        setBlocked(stats.blocked);
      }
    });
    return () => {
      live = false;
      void unlisten.then((stop) => stop());
    };
  }, [tabId]);

  return { blocked, loaded: tabId === null || loadedFor === tabId };
}

/** The form a host takes on the paused-sites list. Matches `adblock::site_key` in Rust. */
export function siteKey(host: string) {
  const key = host.trim().replace(/\.+$/, "").toLowerCase();
  return key.startsWith("www.") && key.length > 4 ? key.slice(4) : key;
}

/** The site is paused when it or a parent domain is on the list. */
export function sitePaused(allowedSites: string[], host: string) {
  let candidate = siteKey(host);
  while (candidate.includes(".")) {
    if (allowedSites.includes(candidate)) {
      return true;
    }
    candidate = candidate.slice(candidate.indexOf(".") + 1);
  }
  return false;
}
