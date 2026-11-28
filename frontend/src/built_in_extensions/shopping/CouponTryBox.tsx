import { Square, WandSparkles } from "lucide-react";
import { Button } from "@/components/Button";
import { cn } from "@/lib/cn";
import type { CouponTryProgress } from "@type/browser";
import { formatPrice } from "./format";

/** Try all codes button, its progress while codes are tried on the page, and the result. */
export function CouponTryBox({
  id,
  count,
  progress,
  running,
  fieldFound,
  onTryAll,
  onCancel,
}: {
  id: string;
  /** Codes that a run would try. */
  count: number;
  progress: CouponTryProgress | null;
  running: boolean;
  fieldFound: boolean;
  onTryAll: () => void;
  onCancel: () => void;
}) {
  const finished = progress && !running ? progress : null;
  const tried = progress ? progress.index + (progress.status === "trying" ? 0 : 1) : 0;
  const percent = progress && progress.total > 0 ? Math.round((Math.min(tried, progress.total) / progress.total) * 100) : 0;

  return (
    <section id={id} className="flex flex-col gap-2 rounded-md border border-zinc-800 bg-zinc-900/60 px-2 py-2">
      {running && progress ? (
        <>
          <div id={`${id}-row`} className="flex items-center gap-2">
            <p id={`${id}-status`} className="min-w-0 flex-1 truncate text-xs text-zinc-300">
              Trying {Math.min(progress.index + 1, progress.total)} of {progress.total}
              {progress.code ? `: ${progress.code}` : ""}
            </p>
            <Button id={`${id}-cancel`} size="xs" variant="ghost" onClick={onCancel}>
              <Square id={`${id}-cancel-icon`} aria-hidden className="size-3" />
              Stop
            </Button>
          </div>
          <div id={`${id}-bar`} className="h-1 w-full overflow-hidden rounded bg-zinc-800">
            <div id={`${id}-bar-fill`} className="h-full bg-sky-500 transition-[width]" style={{ width: `${percent}%` }} />
          </div>
          {progress.bestCode ? (
            <p id={`${id}-best`} className="text-xs text-emerald-300">
              Best so far: {progress.bestCode} saves {formatPrice(progress.bestSaving, progress.currency)}
            </p>
          ) : null}
        </>
      ) : (
        <>
          <Button id={`${id}-all`} variant="primary" disabled={!fieldFound || count === 0} onClick={onTryAll}>
            <WandSparkles id={`${id}-all-icon`} aria-hidden className="size-4" />
            {count === 0 ? "No codes to try" : count === 1 ? "Try the code" : `Try all ${count} codes`}
          </Button>
          {!fieldFound ? (
            <p id={`${id}-hint`} className="text-xs text-zinc-500">
              Open your cart or checkout page to try codes automatically.
            </p>
          ) : null}
        </>
      )}
      {finished ? (
        <p
          id={`${id}-result`}
          className={cn(
            "text-xs",
            finished.status === "done" && finished.bestCode && "text-emerald-300",
            finished.status === "error" && "text-amber-300",
            finished.status !== "error" && !(finished.status === "done" && finished.bestCode) && "text-zinc-400",
          )}
        >
          {finished.message}
        </p>
      ) : null}
    </section>
  );
}
