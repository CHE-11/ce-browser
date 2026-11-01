import { useEffect, useState } from "react";
import { getAdblockStats, onAdblockStats } from "../../lib/browser-client";

/** Requests blocked in tab `tabId` since its current page started loading. Updates live. */
export function useAdblockStats(tabId: number | null) {
  const [blocked, setBlocked] = useState(0);

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
      .catch(() => undefined);
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

  return blocked;
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
