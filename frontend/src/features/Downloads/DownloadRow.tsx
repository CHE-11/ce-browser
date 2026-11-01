import { IconButton } from "@/components/IconButton";
import { openDownload, createTab, pauseDownload, resumeDownload, cancelDownload, showDownloadInFolder, removeDownload } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { formatBytes, hostOf } from "@/lib/format";
import { relativeTime } from "@/lib/time";
import { DownloadItem } from "@type/browser";
import { File, FolderOpen, Pause, Play, RotateCw, Trash2, X } from "lucide-react";


type DownloadRowProps = {
  item: DownloadItem;
  compact?: boolean;
};

function statusText(item: DownloadItem) {
  const received = formatBytes(item.receivedBytes);
  const total = item.totalBytes ? formatBytes(item.totalBytes) : null;
  switch (item.state) {
    case "inProgress":
      return total ? `${received} of ${total}` : `${received} so far`;
    case "paused":
      return total ? `Paused, ${received} of ${total}` : `Paused, ${received}`;
    case "completed":
      return `${total ?? received}, ${relativeTime(new Date(item.finishedAt ?? item.startedAt).toISOString())}`;
    case "cancelled":
      return "Cancelled";
    case "interrupted":
      return item.canResume ? "Interrupted, can resume" : "Failed";
  }
}

export function DownloadRow({ item, compact }: DownloadRowProps) {
  const domId = `download-${item.id}`;
  const running = item.state === "inProgress" || item.state === "paused";
  const fraction = item.totalBytes ? Math.min(1, item.receivedBytes / item.totalBytes) : null;
  const completed = item.state === "completed";

  return (
    <div
      id={domId}
      className={cn(
        "group flex items-center gap-3 rounded-md px-3 py-2 hover:bg-zinc-800/70",
        !compact && "border border-zinc-800 bg-zinc-900 py-3",
      )}
    >
      <File id={`${domId}-icon`} className={cn("size-6 shrink-0", completed ? "text-sky-400" : "text-zinc-500")} aria-hidden />
      <div id={`${domId}-body`} className="min-w-0 flex-1">
        <button
          id={`${domId}-name`}
          type="button"
          disabled={!completed}
          title={completed ? `Open ${item.fileName}` : item.fileName}
          className={cn(
            "block max-w-full truncate text-left text-sm",
            completed ? "text-zinc-100 hover:underline" : "cursor-default text-zinc-300",
            (item.state === "cancelled" || item.state === "interrupted") && "line-through decoration-zinc-500",
          )}
          onClick={() => void openDownload(item.id)}
        >
          {item.fileName}
        </button>
        {!compact ? (
          <button
            id={`${domId}-url`}
            type="button"
            title={item.url}
            className="block max-w-full truncate text-left text-xs text-zinc-500 hover:text-sky-400"
            onClick={() => void createTab(item.url)}
          >
            {hostOf(item.url)}
          </button>
        ) : null}
        <div id={`${domId}-status`} className="truncate text-xs text-zinc-400">
          {statusText(item)}
        </div>
        {running ? (
          <div id={`${domId}-progress`} className="mt-1 h-1 overflow-hidden rounded-full bg-zinc-800">
            <div
              id={`${domId}-progress-value`}
              className={cn(
                "h-full rounded-full bg-sky-500 transition-[width]",
                fraction === null && "w-1/3 animate-pulse",
                item.state === "paused" && "bg-zinc-500",
              )}
              style={fraction === null ? undefined : { width: `${fraction * 100}%` }}
            />
          </div>
        ) : null}
      </div>
      <div id={`${domId}-actions`} className="flex shrink-0 items-center gap-0.5">
        {item.state === "inProgress" ? (
          <IconButton id={`${domId}-pause`} label="Pause" onClick={() => void pauseDownload(item.id)}>
            <Pause id={`${domId}-pause-icon`} aria-hidden />
          </IconButton>
        ) : null}
        {item.state === "paused" || (item.state === "interrupted" && item.canResume) ? (
          <IconButton id={`${domId}-resume`} label="Resume" onClick={() => void resumeDownload(item.id)}>
            <Play id={`${domId}-resume-icon`} aria-hidden />
          </IconButton>
        ) : null}
        {running ? (
          <IconButton id={`${domId}-cancel`} label="Cancel" onClick={() => void cancelDownload(item.id)}>
            <X id={`${domId}-cancel-icon`} aria-hidden />
          </IconButton>
        ) : null}
        {(item.state === "cancelled" || (item.state === "interrupted" && !item.canResume)) ? (
          <IconButton id={`${domId}-retry`} label="Download again" onClick={() => void createTab(item.url, true)}>
            <RotateCw id={`${domId}-retry-icon`} aria-hidden />
          </IconButton>
        ) : null}
        {completed ? (
          <IconButton id={`${domId}-folder`} label="Show in folder" onClick={() => void showDownloadInFolder(item.id)}>
            <FolderOpen id={`${domId}-folder-icon`} aria-hidden />
          </IconButton>
        ) : null}
        {!running ? (
          <IconButton
            id={`${domId}-remove`}
            label="Remove from list"
            className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
            onClick={() => void removeDownload(item.id)}
          >
            <Trash2 id={`${domId}-remove-icon`} aria-hidden />
          </IconButton>
        ) : null}
      </div>
    </div>
  );
}
