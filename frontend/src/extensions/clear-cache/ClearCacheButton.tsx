import { useEffect, useRef, useState } from "react";
import { Check, Eraser, LoaderCircle } from "lucide-react";
import type { TabInfo } from "../../../../types/browser";
import { IconButton } from "../../components/IconButton";
import { clearSiteCache } from "../../lib/browser-client";
import { cn } from "../../lib/cn";

type ClearCacheButtonProps = {
  activeTab: TabInfo | undefined;
};

type CacheNotice = "cleared" | "failed";

/** Host of an http(s) page, including a non-default port. Internal pages have no site cache. */
export function siteHost(url: string | undefined) {
  if (!url) {
    return null;
  }
  try {
    const parsed = new URL(url);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return null;
    }
    return parsed.host || null;
  } catch {
    return null;
  }
}

export function ClearCacheButton({ activeTab }: ClearCacheButtonProps) {
  const activeId = activeTab?.id ?? null;
  const [clearingCache, setClearingCache] = useState(false);
  const [cacheNotice, setCacheNotice] = useState<CacheNotice | null>(null);
  const clearingCacheRef = useRef(false);
  const cacheTimer = useRef<number | null>(null);

  useEffect(
    () => () => {
      if (cacheTimer.current !== null) {
        window.clearTimeout(cacheTimer.current);
      }
    },
    [],
  );

  const cacheHost = siteHost(activeTab?.url);
  const cacheLabel =
    cacheNotice === "cleared"
      ? "Cache cleared"
      : cacheNotice === "failed"
        ? "Couldn't clear cache"
        : clearingCache
          ? "Clearing cache"
          : cacheHost
            ? `Clear cache for ${cacheHost}`
            : "Clear cache for this site";

  const showCacheNotice = (notice: CacheNotice) => {
    if (cacheTimer.current !== null) {
      window.clearTimeout(cacheTimer.current);
    }
    setCacheNotice(notice);
    cacheTimer.current = window.setTimeout(() => {
      cacheTimer.current = null;
      setCacheNotice(null);
    }, 1600);
  };

  const onClearCache = () => {
    if (activeId === null || cacheHost === null || clearingCacheRef.current) {
      return;
    }
    clearingCacheRef.current = true;
    setClearingCache(true);
    setCacheNotice(null);
    void clearSiteCache(activeId)
      .then(() => showCacheNotice("cleared"))
      .catch(() => showCacheNotice("failed"))
      .finally(() => {
        clearingCacheRef.current = false;
        setClearingCache(false);
      });
  };

  return (
    <IconButton
      id="address-clear-cache"
      label={cacheLabel}
      disabled={cacheHost === null || clearingCache}
      className={cn(cacheNotice === "cleared" && "text-sky-300", cacheNotice === "failed" && "text-amber-300")}
      onClick={onClearCache}
    >
      {clearingCache ? (
        <LoaderCircle id="address-clear-cache-icon" className="animate-spin" aria-hidden />
      ) : cacheNotice === "cleared" ? (
        <Check id="address-clear-cache-icon" aria-hidden />
      ) : (
        <Eraser id="address-clear-cache-icon" aria-hidden />
      )}
    </IconButton>
  );
}
