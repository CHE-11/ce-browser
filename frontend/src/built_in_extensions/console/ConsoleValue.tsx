import { useState } from "react";
import { ChevronRight, Copy, LocateFixed } from "lucide-react";
import type { ConsoleValue as ConsoleArg } from "@type/browser";
import { consoleJson, consoleProperties, consoleReveal } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";

type ConsoleValueProps = {
  id: string;
  tabId: number;
  arg: ConsoleArg;
  collected: boolean;
  onStatus: (text: string | null) => void;
};

function summary(arg: ConsoleArg) {
  if (arg.type === "string") {
    const text = typeof arg.value === "string" ? arg.value : arg.description;
    return JSON.stringify(text);
  }
  if (arg.type === "undefined") {
    return "undefined";
  }
  if (arg.subtype === "null") {
    return "null";
  }
  if ((arg.type === "number" || arg.type === "boolean") && (typeof arg.value === "number" || typeof arg.value === "boolean")) {
    return String(arg.value);
  }
  if (arg.preview.length > 0 && arg.objectId) {
    const body = arg.preview.map((prop) => `${prop.name}: ${prop.value || prop.type}`).join(", ");
    const tail = arg.overflow ? ", …" : "";
    const array = arg.subtype === "array" || arg.description.startsWith("Array");
    return array ? `[${body}${tail}]` : `{${body}${tail}}`;
  }
  return arg.description || arg.className || arg.type || "undefined";
}

function tone(arg: ConsoleArg) {
  if (arg.type === "string") {
    return "text-emerald-300";
  }
  if (arg.type === "number") {
    return "text-sky-300";
  }
  if (arg.type === "boolean") {
    return "text-amber-300";
  }
  if (arg.type === "undefined" || arg.subtype === "null") {
    return "text-zinc-500";
  }
  if (arg.type === "function" || arg.subtype === "node") {
    return "text-sky-200";
  }
  if (arg.type === "getter") {
    return "text-zinc-500 italic";
  }
  return "text-zinc-100";
}

function expandable(arg: ConsoleArg) {
  return arg.objectId !== "" && arg.type !== "string" && arg.type !== "number" && arg.type !== "boolean" && arg.type !== "undefined" && arg.subtype !== "null";
}

/** One live value. Objects open into their own properties. */
export function ConsoleValueView({ id, tabId, arg, collected, onStatus }: ConsoleValueProps) {
  const [open, setOpen] = useState(false);
  const [rows, setRows] = useState<{ name: string; value: ConsoleArg }[] | null>(null);
  const [error, setError] = useState("");
  const canOpen = expandable(arg) && !collected;

  const toggle = () => {
    const next = !open;
    setOpen(next);
    if (!next || rows || !canOpen) {
      return;
    }
    void consoleProperties(tabId, arg.objectId)
      .then((list) => setRows(list))
      .catch((reason: unknown) => setError(String(reason)));
  };

  const copy = () => {
    void (async () => {
      try {
        const text = canOpen ? await consoleJson(tabId, arg.objectId) : summary(arg);
        await navigator.clipboard.writeText(text || summary(arg));
        onStatus(null);
      } catch (reason: unknown) {
        onStatus(String(reason));
      }
    })();
  };

  const reveal = () => {
    void consoleReveal(tabId, arg.objectId).catch((reason: unknown) => onStatus(String(reason)));
  };

  return (
    <span id={id} className="group/value inline align-top">
      <span id={`${id}-line`} className="inline-flex max-w-full items-start gap-1">
        {canOpen ? (
          <button
            id={`${id}-toggle`}
            type="button"
            aria-expanded={open}
            aria-label={open ? "Collapse value" : "Expand value"}
            className={cn("mt-0.5 text-zinc-500 hover:text-zinc-200", focusRing)}
            onClick={toggle}
          >
            <ChevronRight id={`${id}-chevron`} className={cn("size-3", open && "rotate-90")} aria-hidden />
          </button>
        ) : null}
        <button
          id={`${id}-preview`}
          type="button"
          className={cn("min-w-0 break-all text-left font-mono", tone(arg), canOpen && "hover:underline", focusRing)}
          onClick={canOpen ? toggle : undefined}
        >
          {summary(arg)}
        </button>
        {collected && arg.objectId ? (
          <span id={`${id}-collected`} className="text-[10px] text-zinc-600">
            collected
          </span>
        ) : null}
        <span id={`${id}-actions`} className="ml-1 hidden gap-0.5 group-hover/value:inline-flex">
          <button id={`${id}-copy`} type="button" title="Copy value" aria-label="Copy value" className={cn("text-zinc-500 hover:text-zinc-200", focusRing)} onClick={copy}>
            <Copy id={`${id}-copy-icon`} className="size-3" aria-hidden />
          </button>
          {arg.subtype === "node" && !collected ? (
            <button id={`${id}-reveal`} type="button" title="Scroll into view" aria-label="Scroll into view" className={cn("text-zinc-500 hover:text-zinc-200", focusRing)} onClick={reveal}>
              <LocateFixed id={`${id}-reveal-icon`} className="size-3" aria-hidden />
            </button>
          ) : null}
        </span>
      </span>
      {open && error ? (
        <span id={`${id}-error`} className="mt-1 block pl-4 text-red-300">
          {error}
        </span>
      ) : null}
      {open && rows ? (
        <span id={`${id}-props`} className="mt-0.5 flex flex-col pl-4">
          {rows.length === 0 ? (
            <span id={`${id}-empty`} className="text-zinc-600">
              No properties
            </span>
          ) : (
            rows.map((row, index) => (
              <span id={`${id}-prop-${index}`} key={`${row.name}-${index}`} className="flex min-w-0 gap-1">
                <span id={`${id}-name-${index}`} className="shrink-0 text-purple-200">
                  {row.name}:
                </span>
                <ConsoleValueView
                  id={`${id}-child-${index}`}
                  tabId={tabId}
                  arg={row.value}
                  collected={collected}
                  onStatus={onStatus}
                />
              </span>
            ))
          )}
        </span>
      ) : null}
    </span>
  );
}
