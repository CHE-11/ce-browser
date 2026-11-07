import { invoke as tauriInvoke, type InvokeArgs, type InvokeOptions } from "@tauri-apps/api/core";
import { listen as tauriListen, type EventCallback, type EventName, type Options, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { DebugIpcRecord } from "@type/browser";

/**
 * `invoke` and `listen` for `browser-client.ts`. In a dev build every call and every event this
 * webview receives is sent to the debug HUD. In a release build these are Tauri's own functions.
 */

const PREVIEW_LIMIT = 2048;
const FLUSH_MS = 200;
/** Records kept while the backend is slow to take them. Older ones are dropped. */
const QUEUE_LIMIT = 500;
/** One event delivered to several listeners in this webview is recorded once. */
const SAME_EVENT_MS = 20;

function currentLabel() {
  try {
    return getCurrentWebview().label;
  } catch {
    return "";
  }
}

const label = import.meta.env.DEV ? currentLabel() : "";
/** The HUD does not record itself, or its own refreshes would fill the feed. */
const recording = import.meta.env.DEV && label !== "" && !label.startsWith("debug-");

let queue: DebugIpcRecord[] = [];
let flushTimer = 0;
let lastEvent = { name: "", bytes: -1, at: 0 };

function flush() {
  flushTimer = 0;
  const batch = queue;
  queue = [];
  if (batch.length > 0) {
    void tauriInvoke("debug_record_ipc", { batch }).catch(() => undefined);
  }
}

function record(entry: Omit<DebugIpcRecord, "id" | "webview">) {
  queue.push({ ...entry, id: 0, webview: label });
  if (queue.length > QUEUE_LIMIT) {
    queue.splice(0, queue.length - QUEUE_LIMIT);
  }
  if (!flushTimer) {
    flushTimer = window.setTimeout(flush, FLUSH_MS);
  }
}

/** JSON of `value`, cut to `PREVIEW_LIMIT`, and the size of the full JSON. */
function preview(value: unknown): { text: string; bytes: number } {
  if (value === undefined) {
    return { text: "", bytes: 0 };
  }
  if (value instanceof ArrayBuffer || ArrayBuffer.isView(value)) {
    return { text: `<${value.byteLength} bytes>`, bytes: value.byteLength };
  }
  let json: string;
  try {
    json = JSON.stringify(value) ?? "";
  } catch {
    return { text: "<not serializable>", bytes: 0 };
  }
  const text = json.length > PREVIEW_LIMIT ? `${json.slice(0, PREVIEW_LIMIT)}…` : json;
  return { text, bytes: json.length };
}

function errorText(reason: unknown) {
  if (typeof reason === "string") {
    return reason;
  }
  if (reason instanceof Error) {
    return reason.message;
  }
  return preview(reason).text || "Unknown error";
}

export function invoke<T>(cmd: string, args?: InvokeArgs, options?: InvokeOptions): Promise<T> {
  if (!recording || cmd.startsWith("debug_")) {
    return tauriInvoke<T>(cmd, args, options);
  }
  const startedAt = Date.now();
  const start = performance.now();
  const argsPreview = preview(args).text;
  return tauriInvoke<T>(cmd, args, options).then(
    (result) => {
      const shown = preview(result);
      record({
        kind: "invoke",
        name: cmd,
        startedAt,
        durationMs: performance.now() - start,
        ok: true,
        argsPreview,
        resultPreview: shown.text,
        error: "",
        bytes: shown.bytes,
      });
      return result;
    },
    (reason: unknown) => {
      record({
        kind: "invoke",
        name: cmd,
        startedAt,
        durationMs: performance.now() - start,
        ok: false,
        argsPreview,
        resultPreview: "",
        error: errorText(reason),
        bytes: 0,
      });
      throw reason;
    },
  );
}

function recordEvent(name: string, payload: unknown) {
  const shown = preview(payload);
  const at = Date.now();
  if (lastEvent.name === name && lastEvent.bytes === shown.bytes && at - lastEvent.at < SAME_EVENT_MS) {
    return;
  }
  lastEvent = { name, bytes: shown.bytes, at };
  record({
    kind: "event",
    name,
    startedAt: at,
    durationMs: null,
    ok: true,
    argsPreview: "",
    resultPreview: shown.text,
    error: "",
    bytes: shown.bytes,
  });
}

export function listen<T>(event: EventName, handler: EventCallback<T>, options?: Options): Promise<UnlistenFn> {
  if (!recording) {
    return tauriListen<T>(event, handler, options);
  }
  return tauriListen<T>(
    event,
    (received) => {
      recordEvent(received.event, received.payload);
      handler(received);
    },
    options,
  );
}
