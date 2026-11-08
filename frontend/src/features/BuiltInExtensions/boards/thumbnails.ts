import { useEffect, useState } from "react";
import { boardThumbnails } from "@/lib/browser-client";

const BATCH_DELAY_MS = 30;
const BATCH_SIZE = 40;

/** Data URL by pin id. Null means the pin has no image. Pin images never change, so entries stay. */
const cache = new Map<string, string | null>();
const waiting = new Map<string, ((src: string | null) => void)[]>();
let timer: number | null = null;

function flush() {
  timer = null;
  const ids = [...waiting.keys()];
  for (let start = 0; start < ids.length; start += BATCH_SIZE) {
    const batch = ids.slice(start, start + BATCH_SIZE);
    const settle = (found: Record<string, string>) => {
      for (const id of batch) {
        const src = found[id] ?? null;
        cache.set(id, src);
        for (const resolve of waiting.get(id) ?? []) {
          resolve(src);
        }
        waiting.delete(id);
      }
    };
    void boardThumbnails(batch)
      .then(settle)
      .catch(() => settle({}));
  }
}

/** Thumbnail for one pin. Requests made close together share one call. */
export function loadThumbnail(id: string): Promise<string | null> {
  if (cache.has(id)) {
    return Promise.resolve(cache.get(id) ?? null);
  }
  return new Promise((resolve) => {
    const list = waiting.get(id);
    if (list) {
      list.push(resolve);
    } else {
      waiting.set(id, [resolve]);
    }
    if (timer === null) {
      timer = window.setTimeout(flush, BATCH_DELAY_MS);
    }
  });
}

export function cachedThumbnail(id: string) {
  return cache.get(id) ?? null;
}

/** Loads the thumbnail once `enabled` turns true, such as when its card scrolls into view. */
export function useThumbnail(id: string | null | undefined, enabled = true) {
  const [src, setSrc] = useState<string | null>(() => (id ? cachedThumbnail(id) : null));
  useEffect(() => {
    if (!id) {
      setSrc(null);
      return;
    }
    if (cache.has(id)) {
      setSrc(cache.get(id) ?? null);
      return;
    }
    if (!enabled) {
      return;
    }
    let gone = false;
    void loadThumbnail(id).then((next) => {
      if (!gone) {
        setSrc(next);
      }
    });
    return () => {
      gone = true;
    };
  }, [id, enabled]);
  return src;
}
