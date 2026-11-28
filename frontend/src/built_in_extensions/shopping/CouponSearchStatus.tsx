import { Check, Loader2, Minus, TriangleAlert } from "lucide-react";
import { cn } from "@/lib/cn";
import type { CouponSearchProgress } from "@type/browser";

function StepIcon({ id, outcome }: { id: string; outcome: CouponSearchProgress["outcome"] }) {
  if (outcome === "ok") {
    return <Check id={id} aria-hidden className="size-3 shrink-0 text-emerald-400" />;
  }
  if (outcome === "blocked") {
    return <TriangleAlert id={id} aria-hidden className="size-3 shrink-0 text-amber-400" />;
  }
  return <Minus id={id} aria-hidden className="size-3 shrink-0 text-zinc-600" />;
}

function StepList({ id, log }: { id: string; log: CouponSearchProgress[] }) {
  return (
    <ol id={id} className="flex max-h-44 flex-col-reverse gap-0.5 overflow-auto">
      {[...log].reverse().map((entry, index) => (
        <li id={`${id}-${index}`} key={`${entry.step}-${entry.message}`} className="flex items-center gap-1.5 text-[11px] text-zinc-400">
          <StepIcon id={`${id}-${index}-icon`} outcome={entry.outcome} />
          <span id={`${id}-${index}-text`} className="min-w-0 truncate" title={entry.message}>
            {entry.message}
          </span>
        </li>
      ))}
    </ol>
  );
}

function summaryOf(log: CouponSearchProgress[]) {
  const searches = log.filter((entry) => entry.stage === "searching").length;
  const pages = log.filter((entry) => entry.stage === "reading" && entry.outcome !== "blocked").length;
  const blocked = log.filter((entry) => entry.outcome === "blocked").length;
  const parts = [`${searches} ${searches === 1 ? "search" : "searches"}`, `${pages} ${pages === 1 ? "page" : "pages"} read`];
  if (blocked > 0) {
    parts.push(`${blocked} blocked`);
  }
  return parts.join(" · ");
}

/** Live steps of a coupon search, then a collapsed record of where it looked. */
export function CouponSearchStatus({
  id,
  live,
  progress,
  log,
}: {
  id: string;
  /** The search is still running. */
  live: boolean;
  progress: CouponSearchProgress | null;
  log: CouponSearchProgress[];
}) {
  if (!live) {
    if (log.length === 0) {
      return null;
    }
    return (
      <details id={id} className="rounded-md border border-zinc-800 px-2 py-1">
        <summary id={`${id}-summary`} className="cursor-pointer text-[11px] text-zinc-500 hover:text-zinc-300">
          Where it looked: {summaryOf(log)}
        </summary>
        <div id={`${id}-body`} className="mt-1">
          <StepList id={`${id}-steps`} log={log} />
        </div>
      </details>
    );
  }

  const steps = Math.max(progress?.steps ?? 0, 1);
  const percent = Math.min(100, Math.round(((progress?.step ?? 0) / steps) * 100));
  return (
    <section id={id} className="flex flex-col gap-2 rounded-md border border-zinc-800 bg-zinc-900/60 px-2 py-2" aria-live="polite">
      <div id={`${id}-head`} className="flex items-center gap-2">
        <Loader2 id={`${id}-spinner`} aria-hidden className="size-3.5 shrink-0 animate-spin text-sky-400" />
        <p id={`${id}-message`} className="min-w-0 flex-1 truncate text-xs text-zinc-200" title={progress?.message}>
          {progress?.message || "Starting the coupon search"}
        </p>
      </div>
      <div id={`${id}-bar`} className="h-1 w-full overflow-hidden rounded bg-zinc-800">
        <div id={`${id}-bar-fill`} className="h-full bg-sky-500 transition-[width]" style={{ width: `${percent}%` }} />
      </div>
      <p id={`${id}-counts`} className={cn("text-[11px]", progress && progress.codes > 0 ? "text-emerald-300" : "text-zinc-500")}>
        {progress?.store ? `${progress.store} · ` : ""}
        {progress?.codes ?? 0} {progress?.codes === 1 ? "code" : "codes"} so far · {progress?.results ?? 0} results
      </p>
      {log.length > 0 ? <StepList id={`${id}-steps`} log={log} /> : null}
    </section>
  );
}
