import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { DebugConsoleEntry, DebugConsoleLevel } from "@type/browser";
import { debugRecordConsole } from "./browser-client";

/**
 * In a dev build, copies this webview's console errors and warnings and its uncaught errors into
 * the backend log, where the debug HUD and passage read them. Imported for its side effect.
 */

const FLUSH_MS = 250;
/** Entries kept while the backend is slow to take them. Older ones are dropped. */
const QUEUE_LIMIT = 200;
const TEXT_LIMIT = 4000;

function currentLabel() {
  try {
    return getCurrentWebview().label;
  } catch {
    return "";
  }
}

const label = import.meta.env.DEV ? currentLabel() : "";
/** The HUD does not record itself, or its own failures would fill the feed. */
const capturing = import.meta.env.DEV && label !== "" && !label.startsWith("debug-");

let queue: DebugConsoleEntry[] = [];
let flushTimer = 0;

function flush() {
  flushTimer = 0;
  const batch = queue;
  queue = [];
  if (batch.length > 0) {
    void debugRecordConsole(batch).catch(() => undefined);
  }
}

function describe(value: unknown): string {
  if (typeof value === "string") {
    return value;
  }
  if (value instanceof Error) {
    return value.stack || `${value.name}: ${value.message}`;
  }
  try {
    return JSON.stringify(value) ?? String(value);
  } catch {
    return String(value);
  }
}

function record(level: DebugConsoleLevel, parts: unknown[], source = "") {
  const text = parts.map(describe).join(" ");
  queue.push({
    level,
    text: text.length > TEXT_LIMIT ? `${text.slice(0, TEXT_LIMIT)}…` : text,
    at: Date.now(),
    source,
  });
  if (queue.length > QUEUE_LIMIT) {
    queue.splice(0, queue.length - QUEUE_LIMIT);
  }
  if (!flushTimer) {
    flushTimer = window.setTimeout(flush, FLUSH_MS);
  }
}

function install() {
  const originalError = console.error.bind(console);
  const originalWarn = console.warn.bind(console);
  console.error = (...parts: unknown[]) => {
    originalError(...parts);
    record("error", parts);
  };
  console.warn = (...parts: unknown[]) => {
    originalWarn(...parts);
    record("warn", parts);
  };
  window.addEventListener("error", (event) => {
    const source = event.filename ? `${event.filename}:${event.lineno}:${event.colno}` : "";
    record("uncaught", [event.error ?? event.message], source);
  });
  window.addEventListener("unhandledrejection", (event) => {
    record("rejection", [event.reason]);
  });
}

if (capturing) {
  install();
}
