import { ChevronRight } from "lucide-react";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import { ConsoleRow, CopiedNote, RepeatBadge, RowTime, messageTone, rowClass, type ConsoleRowProps } from "./ConsoleRow";
import { plainText, type Run } from "./log";
import { useCopyOnRightClick } from "./use-copy-on-right-click";

type ConsoleRunProps = Omit<ConsoleRowProps, "entry"> & {
  run: Run;
  open: boolean;
  onToggle: () => void;
};

/** Messages that only differ by their URLs, folded under one row that opens to show each. */
export function ConsoleRun({ run, open, onToggle, ...row }: ConsoleRunProps) {
  const { copied, copy } = useCopyOnRightClick(row.onStatus);
  const first = run.entries[0];
  const id = `console-run-${first.key}`;

  return (
    <section id={id}>
      <article
        id={`${id}-header`}
        style={{ paddingLeft: 8 + first.depth * 12 }}
        className={cn(rowClass(first.level), "relative", copied && "bg-zinc-800/60")}
        onContextMenu={(event) => copy(event, run.entries.map(plainText).join("\n"))}
      >
        <div id={`${id}-line`} className="flex min-w-0 items-start gap-2">
          <button
            id={`${id}-toggle`}
            type="button"
            aria-expanded={open}
            className={cn("flex min-w-0 flex-1 items-start gap-1.5 text-left", focusRing)}
            onClick={onToggle}
          >
            <ChevronRight id={`${id}-chevron`} className={cn("mt-1 size-3 shrink-0 text-zinc-500", open && "rotate-90")} aria-hidden />
            <RepeatBadge id={`${id}-count`} level={first.level} count={run.entries.length} />
            <span id={`${id}-summary`} className={cn("min-w-0 whitespace-pre-wrap break-all font-mono text-[12px] leading-5", messageTone(first.level))}>
              {run.summary}
            </span>
          </button>
          {row.timestamps ? <RowTime id={`${id}-time`} timestamp={first.timestamp} /> : null}
        </div>
        {copied ? <CopiedNote id={`${id}-copied`} /> : null}
      </article>
      {open ? (
        <div id={`${id}-entries`} className="pl-3">
          {run.entries.map((entry) => (
            <ConsoleRow key={entry.key} entry={entry} {...row} />
          ))}
        </div>
      ) : null}
    </section>
  );
}
