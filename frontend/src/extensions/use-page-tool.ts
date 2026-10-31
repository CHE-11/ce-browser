import { useEffect, useRef, useState } from "react";
import type { PageToolRequest } from "../../../types/browser";
import { currentWindowLabel, onPageToolRequest } from "../lib/browser-client";

export type ToolStatus = "idle" | "busy" | "done" | "failed";

/** Runs `handler` for requests to `tool` that target this browser window. */
export function usePageToolRequests<T extends PageToolRequest["tool"]>(
  tool: T,
  handler: (request: Extract<PageToolRequest, { tool: T }>) => void,
) {
  const handlerRef = useRef(handler);
  handlerRef.current = handler;

  useEffect(() => {
    const window = currentWindowLabel();
    const unlisten = onPageToolRequest((request) => {
      if (request.tool === tool && request.window === window) {
        handlerRef.current(request as Extract<PageToolRequest, { tool: T }>);
      }
    });
    return () => {
      void unlisten.then((stop) => stop());
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
