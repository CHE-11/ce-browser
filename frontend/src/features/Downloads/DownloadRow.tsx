import { useRef, type PointerEvent as ReactPointerEvent } from "react";
import { IconButton } from "@/components/IconButton";
import { openDownload, createTab, pauseDownload, resumeDownload, cancelDownload, showDownloadInFolder, removeDownload, dragDownload } from "@/lib/browser-client";
import { downloadsIconClass, downloadsRowClass, downloadsTextClass } from "@/lib/chrome-metrics";
import { cn } from "@/lib/cn";
import { formatBytes, hostOf } from "@/lib/format";
import { relativeTime } from "@/lib/time";
import { useComputerName } from "@/lib/use-browser";
import { DownloadItem } from "@type/browser";
import {
  File,
  FileArchive,
  FileAudio,
  FileCode,
  FileImage,
  FileSpreadsheet,
  FileText,
  FileVideoCamera,
  Folder,
  FolderOpen,
  Pause,
  Play,
  RotateCw,
  SquareArrowOutUpRight,
  Trash2,
  X,
  type LucideIcon,
} from "lucide-react";


type DownloadRowProps = {
  item: DownloadItem;
  compact?: boolean;
};

const IMAGES = new Set(["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "ico", "avif", "heic"]);
const VIDEOS = new Set(["mp4", "webm", "mkv", "mov", "avi", "m4v"]);
const AUDIO = new Set(["mp3", "wav", "flac", "ogg", "m4a", "aac"]);
const ARCHIVES = new Set(["zip", "rar", "7z", "tar", "gz", "bz2", "tgz"]);
const SHEETS = new Set(["xls", "xlsx", "csv", "ods"]);
const CODE = new Set(["js", "ts", "tsx", "jsx", "json", "html", "css", "rs", "py", "xml", "md", "txt", "log"]);

function extension(name: string) {
  return name.split(".").pop()?.toLowerCase() ?? "";
}

function previewable(name: string) {
  const ext = extension(name);
  return ext === "json" || ext === "csv" || ext === "md" || ext === "markdown" || ext === "log" || ext === "txt";
}

function fileType(name: string): { icon: LucideIcon; className: string } {
  const ext = extension(name);
  if (ext === "pdf") {
    return { icon: FileText, className: "text-red-500" };
  }
  if (IMAGES.has(ext)) {
    return { icon: FileImage, className: "text-sky-400" };
  }
  if (VIDEOS.has(ext)) {
    return { icon: FileVideoCamera, className: "text-violet-400" };
  }
  if (AUDIO.has(ext)) {
    return { icon: FileAudio, className: "text-amber-400" };
  }
  if (ARCHIVES.has(ext)) {
    return { icon: FileArchive, className: "text-yellow-500" };
  }
  if (SHEETS.has(ext)) {
    return { icon: FileSpreadsheet, className: "text-emerald-500" };
  }
  if (CODE.has(ext)) {
    return { icon: FileCode, className: "text-zinc-300" };
  }
  return { icon: File, className: "text-zinc-400" };
}

function savedOn(item: DownloadItem) {
  return item.computerName?.trim() ?? "";
}

/** A download can be opened or dragged only on the computer that saved it. */
function onThisComputer(item: DownloadItem, computer: string | null) {
  const saved = savedOn(item);
  if (!saved) {
    return false;
  }
  if (computer === null) {
    return true;
  }
  return saved.localeCompare(computer, undefined, { sensitivity: "accent" }) === 0;
}

function statusText(item: DownloadItem, compact: boolean, here: boolean) {
  const received = formatBytes(item.receivedBytes);
  const total = item.totalBytes ? formatBytes(item.totalBytes) : null;
  if (!here) {
    const where = savedOn(item) ? `On ${savedOn(item)}` : "Not on this computer";
    const size = total ?? received;
    return compact ? `${where} • ${size}` : `${where}, ${size}`;
  }
  switch (item.state) {
    case "inProgress":
      return total ? `${received} of ${total}` : `${received} so far`;
    case "paused":
      return total ? `Paused, ${received} of ${total}` : `Paused, ${received}`;
    case "completed":
      return compact
        ? `${total ?? received} • Done`
        : `${total ?? received}, ${relativeTime(new Date(item.finishedAt ?? item.startedAt).toISOString())}`;
    case "cancelled":
      return "Cancelled";
    case "interrupted":
      return item.canResume ? "Interrupted, can resume" : "Failed";
  }
}

export function DownloadRow({ item, compact }: DownloadRowProps) {
  const computer = useComputerName();
  const here = onThisComputer(item, computer);
  const domId = `download-${item.id}`;
  const running = here && (item.state === "inProgress" || item.state === "paused");
  const fraction = item.totalBytes ? Math.min(1, item.receivedBytes / item.totalBytes) : null;
  const completed = item.state === "completed" && here;
  const dragging = useRef(false);
  const suppressClickUntil = useRef(0);
  const { icon: TypeIcon, className: typeClass } = fileType(item.fileName);
  const elsewhere = savedOn(item) ? `Saved on ${savedOn(item)}` : "Saved on another computer";
  let nameTitle = item.fileName;
  if (completed) {
    nameTitle = `Open ${item.fileName}`;
  } else if (!here) {
    nameTitle = elsewhere;
  }

  function openDownloaded() {
    if (!here || performance.now() < suppressClickUntil.current) {
      return;
    }
    if (previewable(item.fileName)) {
      void createTab(`ce://files?path=${encodeURIComponent(item.path)}`);
    } else {
      void openDownload(item.id);
    }
  }

  function beginFileDrag(event: ReactPointerEvent<HTMLDivElement>) {
    if (!completed || !here || event.button !== 0) {
      return;
    }
    const target = event.target;
    if (target instanceof Element && target.closest(`#${domId}-actions`)) {
      return;
    }
    const startX = event.clientX;
    const startY = event.clientY;
    const listeners = {
      move(ev: PointerEvent) {
        if (dragging.current) {
          return;
        }
        const dx = ev.clientX - startX;
        const dy = ev.clientY - startY;
        if (dx * dx + dy * dy < 16) {
          return;
        }
        listeners.stop();
        dragging.current = true;
        suppressClickUntil.current = Number.POSITIVE_INFINITY;
        void dragDownload(item.id).finally(() => {
          dragging.current = false;
          suppressClickUntil.current = performance.now() + 300;
        });
      },
      up() {
        listeners.stop();
      },
      stop() {
        window.removeEventListener("pointermove", listeners.move);
        window.removeEventListener("pointerup", listeners.up);
      },
    };
    window.addEventListener("pointermove", listeners.move);
    window.addEventListener("pointerup", listeners.up);
  }

  return (
    <div
      id={domId}
      className={cn(
        "group flex items-center gap-3 rounded-md hover:bg-zinc-800/70",
        compact ? downloadsRowClass : "px-3 py-2",
        completed && "cursor-grab",
        !here && "opacity-60",
        !compact && "border border-zinc-800 bg-zinc-900 py-3",
      )}
      aria-disabled={!here}
      onPointerDown={completed ? beginFileDrag : undefined}
    >
      <TypeIcon id={`${domId}-icon`} className={cn("shrink-0", compact ? downloadsIconClass : "size-6", typeClass)} aria-hidden />
      <div id={`${domId}-body`} className="min-w-0 flex-1">
        <button
          id={`${domId}-name`}
          type="button"
          disabled={!completed}
          title={nameTitle}
          className={cn(
            "block max-w-full truncate text-left",
            compact ? downloadsTextClass : "text-sm",
            completed ? "cursor-grab text-zinc-100 hover:underline" : "cursor-default text-zinc-300",
            !here && "cursor-not-allowed",
            (item.state === "cancelled" || item.state === "interrupted") && "line-through decoration-zinc-500",
          )}
          onClick={openDownloaded}
        >
          {item.fileName}
        </button>
        {!compact ? (
          <button
            id={`${domId}-url`}
            type="button"
            title={item.url}
            className="block max-w-full cursor-pointer truncate text-left text-xs text-zinc-500 hover:text-sky-400"
            onClick={() => void createTab(item.url)}
          >
            {hostOf(item.url)}
          </button>
        ) : null}
        <div id={`${domId}-status`} className="truncate text-xs text-zinc-400">
          {statusText(item, compact ?? false, here)}
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
      <div id={`${domId}-actions`} className="flex shrink-0 cursor-default items-center gap-0.5">
        {here && item.state === "inProgress" ? (
          <IconButton id={`${domId}-pause`} label="Pause" onClick={() => void pauseDownload(item.id)}>
            <Pause id={`${domId}-pause-icon`} aria-hidden />
          </IconButton>
        ) : null}
        {here && (item.state === "paused" || (item.state === "interrupted" && item.canResume)) ? (
          <IconButton id={`${domId}-resume`} label="Resume" onClick={() => void resumeDownload(item.id)}>
            <Play id={`${domId}-resume-icon`} aria-hidden />
          </IconButton>
        ) : null}
        {running ? (
          <IconButton id={`${domId}-cancel`} label="Cancel" onClick={() => void cancelDownload(item.id)}>
            <X id={`${domId}-cancel-icon`} aria-hidden />
          </IconButton>
        ) : null}
        {!here || item.state === "cancelled" || (item.state === "interrupted" && !item.canResume) ? (
          <IconButton id={`${domId}-retry`} label="Download again" onClick={() => void createTab(item.url, true)}>
            <RotateCw id={`${domId}-retry-icon`} aria-hidden />
          </IconButton>
        ) : null}
        {completed ? (
          <IconButton id={`${domId}-folder`} label="Show in folder" onClick={() => void showDownloadInFolder(item.id)}>
            {compact ? (
              <Folder id={`${domId}-folder-icon`} aria-hidden />
            ) : (
              <FolderOpen id={`${domId}-folder-icon`} aria-hidden />
            )}
          </IconButton>
        ) : null}
        {completed && compact ? (
          <IconButton id={`${domId}-open`} label="Open" onClick={openDownloaded}>
            <SquareArrowOutUpRight id={`${domId}-open-icon`} aria-hidden />
          </IconButton>
        ) : null}
        {!running && !compact ? (
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
