import { CheckCircle2, FolderOpen, Loader2, X, XCircle } from "lucide-react";
import { cn } from "@/lib/cn";
import type { BoardImportProgress } from "@type/browser";

type ImportProgressProps = {
  jobs: BoardImportProgress[];
  onCancel: (jobId: string) => void;
  onDismiss: (jobId: string) => void;
  onOpenFolder: (folderId: string) => void;
};

function summary(job: BoardImportProgress) {
  const parts = [`${job.imported} added`];
  if (job.duplicates > 0) {
    parts.push(`${job.duplicates} already there`);
  }
  if (job.failed > 0) {
    parts.push(`${job.failed} failed`);
  }
  return parts.join(" · ");
}

/** Progress cards for running imports, bottom right of the Boards page. */
export function ImportProgress({ jobs, onCancel, onDismiss, onOpenFolder }: ImportProgressProps) {
  if (jobs.length === 0) {
    return null;
  }
  return (
    <div id="boards-import-progress" className="pointer-events-none absolute bottom-4 right-4 z-30 flex w-80 flex-col gap-2">
      {jobs.map((job, index) => {
        const idBase = `boards-import-${index}`;
        const percent = job.total > 0 ? Math.min(100, Math.round((job.done / job.total) * 100)) : 0;
        const title = job.finished ? (job.cancelled ? "Import stopped" : "Import finished") : job.total > 0 ? `Importing ${job.done} of ${job.total}` : "Reading folders…";
        return (
          <section id={idBase} key={job.jobId} className="pointer-events-auto rounded-lg border border-zinc-800 bg-zinc-900/95 p-3 shadow-2xl backdrop-blur">
            <header id={`${idBase}-header`} className="flex items-center gap-2">
              {job.finished ? (
                job.cancelled || (job.imported === 0 && job.failed > 0) ? (
                  <XCircle id={`${idBase}-icon`} className="size-4 shrink-0 text-amber-400" aria-hidden />
                ) : (
                  <CheckCircle2 id={`${idBase}-icon`} className="size-4 shrink-0 text-emerald-400" aria-hidden />
                )
              ) : (
                <Loader2 id={`${idBase}-icon`} className="size-4 shrink-0 animate-spin text-sky-400" aria-hidden />
              )}
              <h3 id={`${idBase}-title`} className="min-w-0 flex-1 truncate text-sm font-medium text-zinc-100">
                {title}
              </h3>
              {job.finished ? (
                <button id={`${idBase}-dismiss`} type="button" aria-label="Dismiss" className="inline-flex size-6 items-center justify-center rounded text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200" onClick={() => onDismiss(job.jobId)}>
                  <X id={`${idBase}-dismiss-icon`} className="size-3.5" aria-hidden />
                </button>
              ) : (
                <button id={`${idBase}-cancel`} type="button" className="rounded px-2 py-0.5 text-xs text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100" onClick={() => onCancel(job.jobId)}>
                  Cancel
                </button>
              )}
            </header>
            {!job.finished ? (
              <div id={`${idBase}-bar`} className="mt-2 h-1.5 overflow-hidden rounded-full bg-zinc-800">
                <div id={`${idBase}-bar-fill`} className={cn("h-full rounded-full bg-sky-500 transition-[width]", job.total === 0 && "w-1/3 animate-pulse")} style={job.total > 0 ? { width: `${percent}%` } : undefined} />
              </div>
            ) : null}
            <p id={`${idBase}-summary`} className="mt-2 text-xs text-zinc-400">
              {summary(job)}
            </p>
            {job.current && !job.finished ? (
              <p id={`${idBase}-current`} className="mt-0.5 truncate text-xs text-zinc-500" title={job.current}>
                {job.current}
              </p>
            ) : null}
            {job.error ? (
              <p id={`${idBase}-error`} className="mt-1 line-clamp-2 text-xs text-amber-300" title={job.error}>
                {job.error}
              </p>
            ) : null}
            {job.finished && job.folderId ? (
              <button id={`${idBase}-open`} type="button" className="mt-2 inline-flex items-center gap-1.5 text-xs text-sky-400 hover:text-sky-300" onClick={() => onOpenFolder(job.folderId!)}>
                <FolderOpen id={`${idBase}-open-icon`} className="size-3.5" aria-hidden />
                Open folder
              </button>
            ) : null}
          </section>
        );
      })}
    </div>
  );
}
