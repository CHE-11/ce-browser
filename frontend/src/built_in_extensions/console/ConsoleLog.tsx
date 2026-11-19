import { useState } from "react";
import { ChevronRight } from "lucide-react";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import { ConsoleValueView } from "./ConsoleValue";
import type { Entry } from "./log";

type ConsoleLogProps = {
  entries: Entry[];
  tabId: number;
  timestamps: boolean;
  pageOrigin: string;
  closed: Set<string>;
  onToggleGroup: (key: string) => void;
  onStatus: (text: string | null) => void;
};

const rowTone: Record<string, string> = {
  error: "border-red-400",
  assert: "border-red-400",
  warn: "border-amber-300",
  info: "border-sky-400",
  debug: "border-zinc-700",
  result: "border-sky-300",
};

function clock(ms: number) {
  const date = new Date(ms);
  const time = date.toLocaleTimeString([], { hour: "numeric", minute: "2-digit", second: "2-digit" });
  return `${time}.${String(date.getMilliseconds()).padStart(3, "0")}`;
}

function ConsoleRow({
  entry,
  tabId,
  timestamps,
  pageOrigin,
  closed,
  onToggleGroup,
  onStatus,
}: {
  entry: Entry;
  tabId: number;
  timestamps: boolean;
  pageOrigin: string;
  closed: Set<string>;
  onToggleGroup: (key: string) => void;
  onStatus: (text: string | null) => void;
}) {
  const [stackOpen, setStackOpen] = useState(false);
  const grouped = entry.level === "group" || entry.level === "groupCollapsed";
  const tone = rowTone[entry.level] ?? "border-transparent";
  const showOrigin = entry.origin !== "" && pageOrigin !== "" && entry.origin !== pageOrigin;

  return (
    <article
      id={`console-row-${entry.key}`}
      style={{ paddingLeft: 8 + entry.depth * 12 }}
      className={cn("border-l-2 px-2 py-0.5 [content-visibility:auto] [contain-intrinsic-size:auto_22px]", tone)}
    >
      <div id={`console-row-${entry.key}-line`} className="flex min-w-0 items-start gap-2">
        {timestamps ? (
          <time id={`console-row-${entry.key}-time`} dateTime={new Date(entry.timestamp).toISOString()} className="shrink-0 pt-px font-mono text-[10px] text-zinc-600">
            {clock(entry.timestamp)}
          </time>
        ) : null}
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
          <div id={`console-row-${entry.key}-body`} className="flex min-w-0 flex-wrap items-start gap-x-2 gap-y-0.5 font-mono text-[12px] leading-5">
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
              <span id={`console-row-${entry.key}-text`} className="whitespace-pre-wrap text-zinc-200">
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
    </article>
  );
}

/** The scrolling list of console rows. */
export function ConsoleLog({ entries, tabId, timestamps, pageOrigin, closed, onToggleGroup, onStatus }: ConsoleLogProps) {
  return (
    <div id="console-log" className="flex flex-col py-1">
      {entries.map((entry) => (
        <ConsoleRow
          key={entry.key}
          entry={entry}
          tabId={tabId}
          timestamps={timestamps}
          pageOrigin={pageOrigin}
          closed={closed}
          onToggleGroup={onToggleGroup}
          onStatus={onStatus}
        />
      ))}
    </div>
  );
}
