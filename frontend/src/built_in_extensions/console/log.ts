import type { ConsoleMessage, ConsoleStackFrame, ConsoleTable, ConsoleValue } from "@type/browser";

const LIMIT = 1000;

export type EntryKind = "message" | "input" | "result";

export type Entry = {
  key: string;
  kind: EntryKind;
  level: string;
  timestamp: number;
  origin: string;
  args: ConsoleValue[];
  stack: ConsoleStackFrame[];
  text: string;
  table: ConsoleTable | null;
  depth: number;
  collected: boolean;
};

export type TabLog = {
  entries: Entry[];
  depth: number;
};

export function emptyLog(): TabLog {
  return { entries: [], depth: 0 };
}

export function wallTime(timestamp: number) {
  if (!timestamp) {
    return Date.now();
  }
  return timestamp > 1e12 ? timestamp : timestamp * 1000;
}

function trim(entries: Entry[]) {
  return entries.length > LIMIT ? entries.slice(entries.length - LIMIT) : entries;
}

export function pushMessage(log: TabLog, message: ConsoleMessage): { log: TabLog; collapse: string | null } {
  if (message.level === "groupEnd") {
    return { log: { ...log, depth: Math.max(0, log.depth - 1) }, collapse: null };
  }
  const entry: Entry = {
    key: `m-${message.id}`,
    kind: "message",
    level: message.level,
    timestamp: wallTime(message.timestamp),
    origin: message.origin,
    args: message.args,
    stack: message.stack,
    text: message.text,
    table: message.table,
    depth: log.depth,
    collected: false,
  };
  const grouped = message.level === "group" || message.level === "groupCollapsed";
  return {
    log: {
      entries: trim([...log.entries, entry]),
      depth: grouped ? log.depth + 1 : log.depth,
    },
    collapse: message.level === "groupCollapsed" ? entry.key : null,
  };
}

export function pushRow(log: TabLog, entry: Entry): TabLog {
  return { ...log, entries: trim([...log.entries, { ...entry, depth: log.depth }]) };
}

export function clearLog(): TabLog {
  return emptyLog();
}

/** Keeps the lines and marks their objects as gone after a navigation. */
export function collectLog(log: TabLog): TabLog {
  return {
    entries: log.entries.map((entry) => ({ ...entry, collected: true })),
    depth: 0,
  };
}

/**
 * A navigation started at `at` (milliseconds). Lines from the new document stay.
 * Older lines are dropped, or kept and marked collected when preserve is on.
 */
export function afterNavigation(log: TabLog, at: number, preserve: boolean): TabLog {
  if (!at) {
    return preserve ? collectLog(log) : clearLog();
  }
  const entries: Entry[] = [];
  for (const entry of log.entries) {
    if (entry.timestamp >= at) {
      entries.push(entry);
      continue;
    }
    if (preserve) {
      entries.push({ ...entry, collected: true });
    }
  }
  return { entries, depth: 0 };
}

export function bucket(level: string) {
  if (level === "error" || level === "assert") {
    return "error";
  }
  if (level === "warn") {
    return "warn";
  }
  if (level === "info") {
    return "info";
  }
  if (level === "debug") {
    return "debug";
  }
  return "log";
}

export function haystack(entry: Entry) {
  return [
    entry.text,
    entry.level,
    ...entry.args.map((arg) => `${arg.description} ${arg.className} ${typeof arg.value === "string" ? arg.value : ""}`),
    ...entry.stack.map((frame) => `${frame.functionName} ${frame.url}`),
  ]
    .join(" ")
    .toLowerCase();
}

export function passes(entry: Entry, filter: string, query: string) {
  if (query && !haystack(entry).includes(query)) {
    return false;
  }
  if (entry.kind === "input") {
    return filter === "all" || query.length > 0;
  }
  if (filter === "all") {
    return true;
  }
  return bucket(entry.level) === filter;
}

export function counts(entries: Entry[]) {
  const totals = { all: 0, error: 0, warn: 0, info: 0, log: 0, debug: 0 };
  for (const entry of entries) {
    if (entry.kind === "input") {
      continue;
    }
    totals.all += 1;
    const group = bucket(entry.level);
    totals[group] += 1;
  }
  return totals;
}

/** Consecutive messages that only differ by the URLs in them. */
export type Run = {
  entries: Entry[];
  /** Every entry has exactly the same text, so the run shows as one row with a count. */
  identical: boolean;
  /** The shared text with each URL replaced by `<URL>`. */
  summary: string;
};

const URL_PATTERN = /\b[a-z][a-z0-9+.-]*:\/\/[^\s"'<>)]+/gi;

function runKey(entry: Entry) {
  if (entry.kind !== "message" || entry.level === "group" || entry.level === "groupCollapsed" || entry.table) {
    return null;
  }
  if (entry.args.some((arg) => arg.objectId !== "")) {
    return null;
  }
  return `${entry.level}\u0000${entry.depth}\u0000${entry.collected}\u0000${entry.text.replace(URL_PATTERN, "<URL>")}`;
}

/** Folds repeated messages into runs, like the DevTools console does. */
export function collapseRuns(entries: Entry[]): Run[] {
  const runs: Run[] = [];
  let lastKey: string | null = null;
  for (const entry of entries) {
    const key = runKey(entry);
    const last = runs[runs.length - 1];
    if (key !== null && key === lastKey && last) {
      last.entries.push(entry);
      last.identical = last.identical && entry.text === last.entries[0].text;
      continue;
    }
    runs.push({ entries: [entry], identical: true, summary: entry.text.replace(URL_PATTERN, "<URL>") });
    lastKey = key;
  }
  return runs;
}

function valueText(arg: Entry["args"][number]) {
  if (arg.type === "string" && typeof arg.value === "string") {
    return arg.value;
  }
  return arg.description || arg.className || arg.type;
}

/** The row as plain text for the clipboard, with the stack for errors. */
export function plainText(entry: Entry) {
  const body = entry.kind === "input" || entry.args.length === 0 ? entry.text : entry.args.map(valueText).join(" ");
  if (bucket(entry.level) !== "error" || entry.stack.length === 0) {
    return body;
  }
  const frames = entry.stack.map((frame) => `    at ${frame.functionName || "(anonymous)"} (${frame.url}:${frame.line}:${frame.column})`);
  return [body, ...frames].join("\n");
}

/** Drops rows that sit inside a collapsed group. */
export function visibleEntries(entries: Entry[], closed: Set<string>) {
  const floors: number[] = [];
  const shown: Entry[] = [];
  for (const entry of entries) {
    while (floors.length > 0 && entry.depth <= floors[floors.length - 1]) {
      floors.pop();
    }
    if (floors.length === 0) {
      shown.push(entry);
    }
    if ((entry.level === "group" || entry.level === "groupCollapsed") && closed.has(entry.key)) {
      floors.push(entry.depth);
    }
  }
  return shown;
}
