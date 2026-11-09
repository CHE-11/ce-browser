import { useState } from "react";
import { ChevronRight, Copy } from "lucide-react";
import { copyText } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { SLOW_CALL_MS, formatClockMs, formatMs, prettyPreview, type IpcRow as IpcRowData } from "@/lib/debug-hud";
import { formatBytes } from "@/lib/format";
import { focusRow } from "@/lib/focus-ring";

function durationClass(duration: number | null) {
  if (duration === null) {
    return "text-zinc-600";
  }
  if (duration >= SLOW_CALL_MS * 5) {
    return "text-red-300";
  }
  if (duration >= SLOW_CALL_MS) {
    return "text-amber-300";
  }
  return "text-zinc-400";
}

function PreviewBlock({ id, title, text, tone = "plain" }: { id: string; title: string; text: string; tone?: "plain" | "error" }) {
  const [copied, setCopied] = useState(false);
  if (!text) {
    return null;
  }
  return (
    <section id={id} className="mt-1.5">
      <div id={`${id}-bar`} className="flex items-center gap-2">
        <h4 id={`${id}-title`} className="text-[10px] uppercase tracking-wide text-zinc-500">
          {title}
        </h4>
        <button
          id={`${id}-copy`}
          type="button"
          className="ml-auto inline-flex h-5 items-center gap-1 rounded px-1 text-[10px] text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200"
          onClick={() => {
            void copyText(text).then(() => {
              setCopied(true);
              window.setTimeout(() => setCopied(false), 1200);
            });
          }}
        >
          <Copy id={`${id}-copy-icon`} aria-hidden className="size-3" />
          {copied ? "Copied" : "Copy"}
        </button>
      </div>
      <pre
        id={`${id}-text`}
        className={cn(
          "mt-1 max-h-48 overflow-auto whitespace-pre-wrap break-all rounded bg-zinc-950 px-2 py-1 font-mono text-[11px]",
          tone === "error" ? "text-red-300" : "text-zinc-300",
        )}
      >
        {prettyPreview(text)}
      </pre>
    </section>
  );
}

export function IpcRow({ row }: { row: IpcRowData }) {
  const [open, setOpen] = useState(false);
  const { record, webviews } = row;
  const rowId = `debug-ipc-row-${row.key}`;
  const isEvent = record.kind === "event";

  return (
    <li id={rowId} className="border-t border-zinc-800/70">
      <button
        id={`${rowId}-toggle`}
        type="button"
        aria-expanded={open}
        className={cn("flex w-full items-center gap-1.5 px-2 py-1 text-left hover:bg-zinc-800/60", focusRow)}
        onClick={() => setOpen((current) => !current)}
      >
        <ChevronRight id={`${rowId}-chevron`} aria-hidden className={cn("size-3 shrink-0 text-zinc-500 transition-transform", open && "rotate-90")} />
        <span
          id={`${rowId}-kind`}
          className={cn(
            "w-8 shrink-0 rounded px-1 text-center text-[9px] font-semibold uppercase",
            isEvent ? "bg-violet-500/15 text-violet-300" : record.ok ? "bg-sky-500/15 text-sky-300" : "bg-red-500/15 text-red-300",
          )}
        >
          {isEvent ? "evt" : record.ok ? "inv" : "err"}
        </span>
        <span id={`${rowId}-name`} className={cn("min-w-0 flex-1 truncate font-mono text-[11px]", record.ok ? "text-zinc-100" : "text-red-200")} title={record.name}>
          {record.name}
        </span>
        <span id={`${rowId}-webviews`} className="max-w-28 shrink-0 truncate text-[10px] text-zinc-500" title={webviews.join(", ")}>
          {webviews.length > 1 ? `${webviews[0]} +${webviews.length - 1}` : webviews[0]}
        </span>
        <span id={`${rowId}-duration`} className={cn("w-14 shrink-0 text-right text-[10px] tabular-nums", durationClass(record.durationMs))}>
          {formatMs(record.durationMs)}
        </span>
        <span id={`${rowId}-bytes`} className="w-12 shrink-0 text-right text-[10px] tabular-nums text-zinc-500">
          {record.bytes > 0 ? formatBytes(record.bytes) : "–"}
        </span>
        <span id={`${rowId}-time`} className="w-[4.75rem] shrink-0 text-right text-[10px] tabular-nums text-zinc-600">
          {formatClockMs(record.startedAt)}
        </span>
      </button>
      {open ? (
        <div id={`${rowId}-details`} className="border-t border-zinc-800/60 bg-zinc-950/40 px-3 py-1.5">
          <p id={`${rowId}-meta`} className="text-[10px] text-zinc-500">
            {isEvent ? "Received by" : "Called from"} {webviews.join(", ")}
          </p>
          <PreviewBlock id={`${rowId}-args`} title="Arguments" text={record.argsPreview} />
          <PreviewBlock id={`${rowId}-error`} title="Error" text={record.error} tone="error" />
          <PreviewBlock id={`${rowId}-result`} title={isEvent ? "Payload" : "Result"} text={record.resultPreview} />
        </div>
      ) : null}
    </li>
  );
}
