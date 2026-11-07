import { useEffect, useState } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { changedPaths } from "./debug-hud";

type LiveJson<T> = {
  value: T | undefined;
  /** Unix ms of the last load or change event. */
  updatedAt: number;
  /** JSON paths that differ from the value before the last update, with their ancestors. */
  changed: Set<string>;
  error: string;
};

/** A backend snapshot loaded once and kept current from its change event while `active`. */
export function useLiveJson<T>(load: () => Promise<T>, subscribe: (handler: (value: T) => void) => Promise<UnlistenFn>, active: boolean) {
  const [state, setState] = useState<LiveJson<T>>({ value: undefined, updatedAt: 0, changed: new Set(), error: "" });

  useEffect(() => {
    if (!active) {
      return;
    }
    let alive = true;
    const apply = (next: T) => {
      if (!alive) {
        return;
      }
      setState((previous) => ({ value: next, updatedAt: Date.now(), changed: changedPaths(previous.value, next), error: "" }));
    };
    const unlisten = subscribe(apply);
    void load()
      .then(apply)
      .catch((reason: unknown) => {
        if (alive) {
          setState((previous) => ({ ...previous, error: typeof reason === "string" ? reason : "This snapshot could not be read." }));
        }
      });
    return () => {
      alive = false;
      void unlisten.then((stop) => stop());
    };
  }, [active, load, subscribe]);

  return state;
}
