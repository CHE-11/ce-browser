import { useEffect, useState } from "react";

/** State kept in this webview's `localStorage` under `key`. `accept` rejects stale stored values. */
export function useStoredState<T>(key: string, fallback: T, accept: (value: unknown) => value is T) {
  const [value, setValue] = useState<T>(() => {
    try {
      const raw = localStorage.getItem(key);
      const parsed: unknown = raw === null ? fallback : JSON.parse(raw);
      return accept(parsed) ? parsed : fallback;
    } catch {
      return fallback;
    }
  });

  useEffect(() => {
    try {
      localStorage.setItem(key, JSON.stringify(value));
    } catch {
      // Storage can be full or disabled. The value still holds for this session.
    }
  }, [key, value]);

  return [value, setValue] as const;
}

export function isBoolean(value: unknown): value is boolean {
  return typeof value === "boolean";
}
