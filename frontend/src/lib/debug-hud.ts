import type { DebugIpcRecord } from "@type/browser";

/** Matches `RING_SIZE` in `src-tauri/src/debug/mod.rs`. */
export const DEBUG_RING_SIZE = 2000;
/** Calls slower than this are flagged and kept by the Slow filter. */
export const SLOW_CALL_MS = 100;
/** One emission reaches every webview that listens. Arrivals this close together are one row. */
const SAME_EMISSION_MS = 100;

/** Appends `next` to `current`, keeping only the newest `limit` items. */
export function appendCapped<T>(current: T[], next: T[], limit = DEBUG_RING_SIZE) {
  if (next.length === 0) {
    return current;
  }
  const joined = current.concat(next);
  return joined.length > limit ? joined.slice(joined.length - limit) : joined;
}

export type IpcRow = {
  key: number;
  record: DebugIpcRecord;
  /** Every webview that made this call or received this event. */
  webviews: string[];
};

/** One row per call, and one row per event emission listing every webview that received it. */
export function groupIpcRows(records: DebugIpcRecord[]): IpcRow[] {
  const rows: IpcRow[] = [];
  const openEvents = new Map<string, IpcRow>();
  for (const record of records) {
    if (record.kind !== "event") {
      rows.push({ key: record.id, record, webviews: [record.webview] });
      continue;
    }
    const open = openEvents.get(record.name);
    if (open && Math.abs(record.startedAt - open.record.startedAt) <= SAME_EMISSION_MS && !open.webviews.includes(record.webview)) {
      open.webviews.push(record.webview);
      continue;
    }
    const row = { key: record.id, record, webviews: [record.webview] };
    openEvents.set(record.name, row);
    rows.push(row);
  }
  return rows;
}

/** Paths whose value changed between two JSON values, plus every ancestor of each. */
export function changedPaths(before: unknown, after: unknown, limit = 400): Set<string> {
  const found = new Set<string>();
  const visit = (left: unknown, right: unknown, path: string) => {
    if (found.size >= limit || Object.is(left, right)) {
      return;
    }
    const bothObjects = typeof left === "object" && typeof right === "object" && left !== null && right !== null && Array.isArray(left) === Array.isArray(right);
    if (!bothObjects) {
      markWithAncestors(found, path);
      return;
    }
    const leftRecord = left as Record<string, unknown>;
    const rightRecord = right as Record<string, unknown>;
    const keys = new Set([...Object.keys(leftRecord), ...Object.keys(rightRecord)]);
    for (const key of keys) {
      visit(leftRecord[key], rightRecord[key], joinPath(path, key));
    }
  };
  if (before !== undefined) {
    visit(before, after, "");
  }
  return found;
}

function markWithAncestors(found: Set<string>, path: string) {
  let current = path;
  while (true) {
    found.add(current);
    if (current === "") {
      return;
    }
    const cut = current.lastIndexOf(".");
    current = cut < 0 ? "" : current.slice(0, cut);
  }
}

export function joinPath(parent: string, key: string) {
  return parent === "" ? key : `${parent}.${key}`;
}

/** An `id` built from a JSON path. */
export function pathId(prefix: string, path: string) {
  return `${prefix}-${path.replace(/[^\w-]+/g, "_") || "root"}`;
}

export function formatMs(duration: number | null) {
  if (duration === null) {
    return "–";
  }
  if (duration < 10) {
    return `${duration.toFixed(1)} ms`;
  }
  if (duration < 1000) {
    return `${Math.round(duration)} ms`;
  }
  return `${(duration / 1000).toFixed(2)} s`;
}

/** `14:03:07.215` */
export function formatClockMs(time: number) {
  const date = new Date(time);
  const clock = date.toLocaleTimeString(undefined, { hour12: false, hour: "2-digit", minute: "2-digit", second: "2-digit" });
  return `${clock}.${String(date.getMilliseconds()).padStart(3, "0")}`;
}

/** Pretty JSON of a preview string. A cut-off preview is shown as it came. */
export function prettyPreview(text: string) {
  if (!text) {
    return "";
  }
  try {
    return JSON.stringify(JSON.parse(text), null, 2);
  } catch {
    return text;
  }
}
