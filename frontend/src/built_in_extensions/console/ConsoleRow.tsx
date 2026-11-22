import { useState } from "react";
import { ChevronRight } from "lucide-react";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import { ConsoleValueView } from "./ConsoleValue";
import { bucket, plainText, type Entry } from "./log";
import { useCopyOnRightClick } from "./use-copy-on-right-click";

export type ConsoleRowProps = {
  entry: Entry;
  tabId: number;
  timestamps: boolean;
  pageOrigin: string;
  closed: Set<string>;
  onToggleGroup: (key: string) => void;
  onStatus: (text: string | null) => void;
};

const rowTone: Record<string, string> = {
  error: "border-red-900/70 bg-red-950/60",
  warn: "border-amber-900/60 bg-amber-950/50",
};

const textTone: Record<string, string> = {
  error: "text-red-200",
  warn: "text-amber-100",
};

/** Row frame classes. Errors and warnings get a tinted background. */
export function rowClass(level: string) {
  return cn("mx-1 my-px rounded border px-2 py-0.5", rowTone[bucket(level)] ?? "border-transparent");
}

/** Plain message text color for a level. */
export function messageTone(level: string) {
  return textTone[bucket(level)] ?? "text-zinc-200";
}

const badgeTone: Record<string, string> = {
  error: "bg-red-500/80 text-white",
  warn: "bg-amber-400/90 text-zinc-950",
  info: "bg-sky-500/80 text-white",
  debug: "bg-zinc-700 text-zinc-200",
  log: "bg-zinc-600 text-zinc-100",
};

export function clock(ms: number) {
  const date = new Date(ms);
  const time = date.toLocaleTimeString([], { hour: "numeric", minute: "2-digit", second: "2-digit" });
  return `${time}.${String(date.getMilliseconds()).padStart(3, "0")}`;
}

/** How many times a message repeated, colored by its level. */
export function RepeatBadge({ id, level, count }: { id: string; level: string; count: number }) {
  return (
    <span id={id} className={cn("shrink-0 rounded-full px-1.5 font-sans text-[10px] font-semibold leading-4", badgeTone[bucket(level)])}>
      {count}
    </span>
  );
}

/** Sits in the row's bottom right corner. The row must be `relative`. */
export function CopiedNote({ id }: { id: string }) {
  return (
    <span id={id} role="status" className="pointer-events-none absolute bottom-0.5 right-1.5 rounded bg-zinc-900/90 px-1 font-sans text-[10px] leading-4 text-emerald-400">
      Copied
    </span>
  );
}

/** The time a message was logged, at the right end of its row. */
export function RowTime({ id, timestamp }: { id: string; timestamp: number }) {
  return (
    <time id={id} dateTime={new Date(timestamp).toISOString()} className="ml-auto shrink-0 pl-2 pt-px font-mono text-[10px] text-zinc-600">
      {clock(timestamp)}
    </time>
  );
}

/** One console line. Right click copies it. */
export function ConsoleRow({ entry, tabId, timestamps, pageOrigin, closed, onToggleGroup, onStatus, count = 1 }: ConsoleRowProps & { count?: number }) {
  const [stackOpen, setStackOpen] = useState(false);
  const { copied, copy } = useCopyOnRightClick(onStatus);
  const grouped = entry.level === "group" || entry.level === "groupCollapsed";
  const tone = entry.kind === "message" || entry.level === "error" ? entry.level : "";
  const showOrigin = entry.origin !== "" && pageOrigin !== "" && entry.origin !== pageOrigin;

  return (
    <article
      id={`console-row-${entry.key}`}
      style={{ paddingLeft: 8 + entry.depth * 12 }}
      className={cn(rowClass(tone), "relative [content-visibility:auto] [contain-intrinsic-size:auto_22px]", copied && "bg-zinc-800/60")}
      onContextMenu={(event) => copy(event, plainText(entry))}
    >
      <div id={`console-row-${entry.key}-line`} className="flex min-w-0 items-start gap-2">
        {count > 1 ? <RepeatBadge id={`console-row-${entry.key}-count`} level={entry.level} count={count} /> : null}
        {grouped ? (
          <button
            id={`console-row-${entry.key}-group`}
            type="button"
            aria-expanded={!closed.has(entry.key)}
            className={cn("inline-flex min-w-0 items-center gap-1 text-left text-zinc-200", focusRing)}
            onClick={() => onToggleGroup(entry.key)}
          >
            <ChevronRight
              id={`console-row-${entry.key}-group-icon`}
              className={cn("size-3 shrink-0 text-zinc-500", !closed.has(entry.key) && "rotate-90")}
              aria-hidden
            />
            <span id={`console-row-${entry.key}-group-label`} className="truncate font-mono">
              {entry.text || "Group"}
            </span>
          </button>
        ) : (
          <div id={`console-row-${entry.key}-body`} className="flex min-w-0 flex-1 flex-wrap items-start gap-x-2 gap-y-0.5 font-mono text-[12px] leading-5">
            {entry.kind === "input" ? (
              <span id={`console-row-${entry.key}-input`} className="whitespace-pre-wrap text-zinc-400">
                <span id={`console-row-${entry.key}-mark`} className="mr-1 text-sky-400">
                  ›
                </span>
                {entry.text}
              </span>
            ) : (
              <>
                {entry.kind === "result" && entry.text === "$0" ? (
                  <span id={`console-row-${entry.key}-name`} className="text-zinc-500">
                    $0 =
                  </span>
                ) : null}
                {entry.args.map((arg, index) => (
                  <ConsoleValueView
                    key={`${entry.key}-${index}`}
                    id={`console-row-${entry.key}-arg-${index}`}
                    tabId={tabId}
                    arg={arg}
                    collected={entry.collected}
                    onStatus={onStatus}
                  />
                ))}
              </>
            )}
            {entry.kind !== "input" && entry.args.length === 0 ? (
              <span id={`console-row-${entry.key}-text`} className={cn("whitespace-pre-wrap break-all", messageTone(tone))}>
                {entry.text}
              </span>
            ) : null}
            {showOrigin ? (
              <span id={`console-row-${entry.key}-origin`} className="text-[10px] text-zinc-500">
                {entry.origin}
              </span>
            ) : null}
          </div>
        )}
        {timestamps ? <RowTime id={`console-row-${entry.key}-time`} timestamp={entry.timestamp} /> : null}
      </div>
      {entry.table && entry.table.columns.length > 0 ? (
        <div id={`console-table-wrap-${entry.key}`} className="mt-1 max-w-full overflow-auto">
          <table id={`console-table-${entry.key}`} className="border-collapse font-mono text-[11px]">
            <thead id={`console-table-${entry.key}-head`}>
              <tr id={`console-table-${entry.key}-head-row`}>
                {entry.table.columns.map((column, index) => (
                  <th id={`console-table-${entry.key}-col-${index}`} key={`${column}-${index}`} className="border border-zinc-800 px-1.5 py-0.5 text-left font-medium text-zinc-400">
                    {column}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody id={`console-table-${entry.key}-body`}>
              {entry.table.rows.map((row, rowIndex) => (
                <tr id={`console-table-${entry.key}-row-${rowIndex}`} key={rowIndex}>
                  {entry.table?.columns.map((_, cellIndex) => (
                    <td id={`console-table-${entry.key}-cell-${rowIndex}-${cellIndex}`} key={cellIndex} className="border border-zinc-800 px-1.5 py-0.5 text-zinc-200">
                      {row[cellIndex] ?? ""}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}
      {(entry.level === "error" || entry.level === "assert" || entry.level === "trace") && entry.stack.length > 0 ? (
        <div id={`console-stack-${entry.key}`} className="mt-0.5">
          <button
            id={`console-stack-${entry.key}-toggle`}
            type="button"
            aria-expanded={stackOpen}
            className={cn("text-[10px] text-zinc-500 hover:text-zinc-300", focusRing)}
            onClick={() => setStackOpen((open) => !open)}
          >
            {stackOpen ? "Hide stack" : "Stack"}
          </button>
          {stackOpen ? (
            <ol id={`console-stack-${entry.key}-frames`} className="mt-0.5 font-mono text-[10px] text-zinc-500">
              {entry.stack.map((frame, index) => (
                <li id={`console-stack-${entry.key}-frame-${index}`} key={`${frame.url}-${frame.line}-${index}`}>
                  {frame.functionName || "(anonymous)"} {frame.url}:{frame.line}:{frame.column}
                </li>
              ))}
            </ol>
          ) : null}
        </div>
      ) : null}
      {copied ? <CopiedNote id={`console-row-${entry.key}-copied`} /> : null}
    </article>
  );
}
