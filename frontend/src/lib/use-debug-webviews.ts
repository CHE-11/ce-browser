import { useCallback, useEffect, useRef, useState } from "react";
import type { DebugWebview } from "@type/browser";
import { debugWebviews, onPopupChanged, onTabsChanged } from "./browser-client";

/** Every native webview, polled every `intervalMs` and again whenever the tabs or popups
 * change. Each listing also moves or drops the debug outline to match its webview. */
export function useDebugWebviews(intervalMs: number) {
  const [webviews, setWebviews] = useState<DebugWebview[] | null>(null);
  const [error, setError] = useState("");
  const loadRef = useRef<() => void>(() => undefined);

  useEffect(() => {
    let alive = true;
    let busy = false;
    const load = () => {
      if (busy) {
        return;
      }
      busy = true;
      void debugWebviews()
        .then((next) => {
          if (alive) {
            setWebviews(next);
            setError("");
          }
        })
        .catch((reason: unknown) => {
          if (alive) {
            setError(typeof reason === "string" ? reason : "The webviews could not be listed.");
          }
        })
        .finally(() => {
          busy = false;
        });
    };
    loadRef.current = load;
    load();
    const timer = window.setInterval(load, intervalMs);
    const unlisteners = [onTabsChanged(() => load()), onPopupChanged(() => load())];
    return () => {
      alive = false;
      window.clearInterval(timer);
      for (const unlisten of unlisteners) {
        void unlisten.then((stop) => stop());
      }
    };
  }, [intervalMs]);

  const refresh = useCallback(() => loadRef.current(), []);

  return { webviews, error, refresh };
}
