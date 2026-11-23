import { useEffect, useRef, useState } from "react";
import type { PageToolRequest } from "../../../types/browser";
import { currentWindowLabel, onPageToolRequest } from "../lib/browser-client";

export type ToolStatus = "idle" | "busy" | "done" | "failed";

type ToolHandler = (request: PageToolRequest) => void;

const listeners = new Map<string, Set<ToolHandler>>();
let listening = false;

function listenerKey(windowLabel: string, tool: string) {
  return `${windowLabel}\0${tool}`;
}

/** One app-wide listener. A tool pinned on two bars would otherwise save twice. */
function ensurePageToolListener() {
  if (listening) {
    return;
  }
  listening = true;
  void onPageToolRequest((request) => {
    const handlers = listeners.get(listenerKey(request.window, request.tool));
    const handler = handlers?.values().next().value;
    handler?.(request);
  });
}

/** Runs `handler` for requests to `tool` that target this browser window. */
export function usePageToolRequests<T extends PageToolRequest["tool"]>(
  tool: T,
  handler: (request: Extract<PageToolRequest, { tool: T }>) => void,
) {
  const handlerRef = useRef(handler);
  handlerRef.current = handler;

  useEffect(() => {
    ensurePageToolListener();
    const key = listenerKey(currentWindowLabel(), tool);
    const wrapped: ToolHandler = (request) => {
      handlerRef.current(request as Extract<PageToolRequest, { tool: T }>);
    };
    const handlers = listeners.get(key) ?? new Set<ToolHandler>();
    handlers.add(wrapped);
    listeners.set(key, handlers);
    return () => {
      handlers.delete(wrapped);
      if (handlers.size === 0) {
        listeners.delete(key);
      }
    };
  }, [tool]);
}

/** Busy / done / failed state for a toolbar button. Done and failed fade back to idle. */
export function useToolStatus() {
  const [status, setStatus] = useState<ToolStatus>("idle");
  const timer = useRef<number | null>(null);

  useEffect(
    () => () => {
      if (timer.current !== null) {
        window.clearTimeout(timer.current);
      }
    },
    [],
  );

  const show = (next: ToolStatus) => {
    if (timer.current !== null) {
      window.clearTimeout(timer.current);
      timer.current = null;
    }
    setStatus(next);
    if (next === "done" || next === "failed") {
      timer.current = window.setTimeout(() => {
        timer.current = null;
        setStatus("idle");
      }, 2400);
    }
  };

  return [status, show] as const;
}

export function errorText(error: unknown) {
  if (typeof error === "string") {
    return error;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return "Something went wrong.";
}
