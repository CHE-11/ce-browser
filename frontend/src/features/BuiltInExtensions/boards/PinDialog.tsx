import { useEffect, useState, type ReactNode } from "react";
import { ChevronLeft, ChevronRight, Copy, ExternalLink, File, LoaderCircle, Save, Star, Trash2 } from "lucide-react";
import { Button } from "@/components/Button";
import { Dialog } from "@/components/Dialog";
import { FavIcon } from "@/components/FavIcon";
import { boardImage, chooseSavePath, copyPinImage, defaultDownloadDir, movePins, openPinFile, savePinImage, setBoardCover, showNotice, updatePin } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import type { BoardsSnapshot, Pin } from "@type/browser";
import { fileNameFor, joinPath } from "@features/BuiltInExtensions/shared";
import { PIN_KIND_LABELS, boardChoices, hostOf } from "./board-tree";
import { savePinFileAs } from "./file-pin";
import { cachedThumbnail } from "./thumbnails";

type PinDialogProps = {
  boards: BoardsSnapshot;
  pin: Pin | null;
  onClose: () => void;
  onPrevious?: () => void;
  onNext?: () => void;
  onOpenSource: (pin: Pin) => void;
  onDelete: (ids: string[]) => void;
  onError: (reason: unknown) => void;
};

const fieldClass = "w-full rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500";

function isTyping(target: EventTarget | null) {
  return target instanceof HTMLElement && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.tagName === "SELECT");
}

export function PinDialog({ boards, pin, onClose, onPrevious, onNext, onOpenSource, onDelete, onError }: PinDialogProps) {
  const [image, setImage] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [title, setTitle] = useState("");
  const [note, setNote] = useState("");
  const pinId = pin?.id ?? null;

  useEffect(() => {
    setTitle(pin?.title ?? "");
    setNote(pin?.note ?? "");
    // Only when another pin opens; edits arriving through boards-changed keep what is being typed.
  }, [pinId]);

  useEffect(() => {
    if (!pinId || !pin?.hasImage) {
      setImage(null);
      return;
    }
    let gone = false;
    setImage(cachedThumbnail(pinId));
    setLoading(true);
    void boardImage(pinId)
      .then((src) => {
        if (!gone) {
          setImage(src);
        }
      })
      .catch((reason: unknown) => {
        if (!gone) {
          onError(reason);
        }
      })
      .finally(() => {
        if (!gone) {
          setLoading(false);
        }
      });
    return () => {
      gone = true;
    };
  }, [pinId, pin?.hasImage]);

  useEffect(() => {
    if (!pin) {
      return;
    }
    const onKey = (event: KeyboardEvent) => {
      if (isTyping(event.target) || event.altKey || event.ctrlKey || event.metaKey) {
        return;
      }
      if (event.key === "ArrowLeft" && onPrevious) {
        event.preventDefault();
        onPrevious();
      } else if (event.key === "ArrowRight" && onNext) {
        event.preventDefault();
        onNext();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [pin, onPrevious, onNext]);

  if (!pin) {
    return null;
  }

  const save = () => {
    if (title !== pin.title || note !== pin.note) {
      void updatePin(pin.id, title, note).catch(onError);
    }
  };

  const saveAs = async () => {
    try {
      const folder = await defaultDownloadDir();
      const path = await chooseSavePath(joinPath(folder, fileNameFor(pin.title, pin.pageUrl, "png")), "PNG image", ["png"]);
      if (path) {
        await savePinImage(pin.id, path);
        await showNotice("Image saved");
      }
    } catch (reason) {
      onError(reason);
    }
  };

  const host = hostOf(pin.pageUrl);
  const board = boards.boards.find((item) => item.id === pin.boardId);
  const isCover = board?.coverPinId === pin.id;
  const linkDiffers = pin.linkUrl && pin.linkUrl !== pin.pageUrl;

  return (
    <Dialog
      id="boards-pin-dialog"
      title={pin.title || host || PIN_KIND_LABELS[pin.kind]}
      open
      onClose={() => {
        save();
        onClose();
      }}
      panelClassName="h-[85vh] max-w-6xl"
      bodyClassName="flex gap-0 p-0"
    >
      <div id="boards-pin-dialog-stage" className="relative flex min-w-0 flex-1 items-start justify-center overflow-auto bg-zinc-950 p-4">
        {pin.hasImage ? (
          image ? (
            <img id="boards-pin-dialog-image" src={image} alt="" className="max-w-full rounded-md shadow-lg" />
          ) : (
            <div id="boards-pin-dialog-image-loading" className="flex h-full w-full items-center justify-center text-zinc-500">
              <LoaderCircle id="boards-pin-dialog-image-spinner" className="size-6 animate-spin" aria-hidden />
            </div>
          )
        ) : pin.kind === "file" ? (
          <div id="boards-pin-dialog-file" className="flex max-w-md flex-col items-center gap-3 text-center text-zinc-300">
            <File id="boards-pin-dialog-file-icon" className="size-12 text-zinc-500" aria-hidden />
            <p id="boards-pin-dialog-file-name" className="text-sm">
              {pin.fileName || pin.title || "File"}
            </p>
          </div>
        ) : (
          <p id="boards-pin-dialog-text" className="max-w-2xl whitespace-pre-line text-sm leading-relaxed text-zinc-300">
            {pin.text || "No preview."}
          </p>
        )}
        {loading && image ? (
          <LoaderCircle id="boards-pin-dialog-image-refresh" className="absolute right-6 top-6 size-4 animate-spin text-zinc-400" aria-hidden />
        ) : null}
        {onPrevious ? (
          <NavButton id="boards-pin-dialog-previous" label="Previous pin" side="left" onClick={onPrevious}>
            <ChevronLeft aria-hidden />
          </NavButton>
        ) : null}
        {onNext ? (
          <NavButton id="boards-pin-dialog-next" label="Next pin" side="right" onClick={onNext}>
            <ChevronRight aria-hidden />
          </NavButton>
        ) : null}
      </div>

      <aside id="boards-pin-dialog-details" className="flex w-80 shrink-0 flex-col gap-3 overflow-y-auto border-l border-zinc-800 p-4">
        <label id="boards-pin-dialog-title-label" className="flex flex-col gap-1 text-xs text-zinc-400">
          Title
          <input
            id="boards-pin-dialog-title"
            value={title}
            className={cn(fieldClass, "h-8")}
            onChange={(event) => setTitle(event.target.value)}
            onBlur={save}
          />
        </label>
        <label id="boards-pin-dialog-note-label" className="flex flex-col gap-1 text-xs text-zinc-400">
          Note
          <textarea
            id="boards-pin-dialog-note"
            value={note}
            rows={5}
            placeholder="Add a note"
            className={cn(fieldClass, "resize-y py-1.5")}
            onChange={(event) => setNote(event.target.value)}
            onBlur={save}
          />
        </label>
        <label id="boards-pin-dialog-board-label" className="flex flex-col gap-1 text-xs text-zinc-400">
          Board
          <select
            id="boards-pin-dialog-board"
            value={pin.boardId}
            className={cn(fieldClass, "h-8")}
            onChange={(event) => void movePins([pin.id], event.target.value).catch(onError)}
          >
            {boardChoices(boards).map(({ board: choice, label }) => (
              <option id={`boards-pin-dialog-board-${choice.id}`} key={choice.id} value={choice.id}>
                {label}
              </option>
            ))}
          </select>
        </label>

        <dl id="boards-pin-dialog-meta" className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1.5 text-xs">
          <dt id="boards-pin-dialog-kind-label" className="text-zinc-500">
            Type
          </dt>
          <dd id="boards-pin-dialog-kind" className="text-zinc-300">
            {PIN_KIND_LABELS[pin.kind]}
            {pin.hasImage ? ` · ${pin.width} × ${pin.height}` : ""}
          </dd>
          <dt id="boards-pin-dialog-saved-label" className="text-zinc-500">
            Saved
          </dt>
          <dd id="boards-pin-dialog-saved" className="text-zinc-300">
            {new Date(pin.createdAt).toLocaleString()}
          </dd>
          <dt id="boards-pin-dialog-source-label" className="text-zinc-500">
            From
          </dt>
          <dd id="boards-pin-dialog-source" className="flex min-w-0 items-center gap-1.5 text-zinc-300">
            <FavIcon id="boards-pin-dialog-source-icon" url={pin.pageUrl} className="size-3.5" />
            <span id="boards-pin-dialog-source-url" className="truncate" title={pin.pageUrl}>
              {host || pin.pageUrl}
            </span>
          </dd>
          {pin.kind === "file" ? (
            <>
              <dt id="boards-pin-dialog-file-label" className="text-zinc-500">
                File
              </dt>
              <dd id="boards-pin-dialog-file-value" className="truncate text-zinc-300" title={pin.fileName}>
                {pin.fileName || pin.title}
              </dd>
            </>
          ) : null}
          {linkDiffers ? (
            <>
              <dt id="boards-pin-dialog-link-label" className="text-zinc-500">
                Link
              </dt>
              <dd id="boards-pin-dialog-link" className="truncate text-zinc-300" title={pin.linkUrl}>
                {pin.linkUrl}
              </dd>
            </>
          ) : null}
        </dl>

        {pin.hasImage && pin.text ? (
          <p id="boards-pin-dialog-excerpt" className="max-h-40 overflow-y-auto whitespace-pre-line rounded-md bg-zinc-950 p-2 text-xs text-zinc-400">
            {pin.text}
          </p>
        ) : null}

        <div id="boards-pin-dialog-actions" className="mt-auto flex flex-col gap-2 pt-2">
          {pin.kind === "file" ? (
            <div id="boards-pin-dialog-file-actions" className="flex gap-2">
              <Button
                id="boards-pin-dialog-open-file"
                variant="primary"
                className="flex-1"
                onClick={() =>
                  void openPinFile(pin.id)
                    .then(() => showNotice("Opened file"))
                    .catch(onError)
                }
              >
                <File id="boards-pin-dialog-open-file-icon" className="size-4" aria-hidden />
                Open file
              </Button>
              <Button
                id="boards-pin-dialog-save-file"
                className="flex-1"
                onClick={() =>
                  void savePinFileAs(pin)
                    .then((saved) => {
                      if (saved) {
                        return showNotice("File saved");
                      }
                    })
                    .catch(onError)
                }
              >
                <Save id="boards-pin-dialog-save-file-icon" className="size-4" aria-hidden />
                Save as…
              </Button>
            </div>
          ) : (
            <Button id="boards-pin-dialog-open" variant="primary" onClick={() => onOpenSource(pin)}>
              <ExternalLink id="boards-pin-dialog-open-icon" className="size-4" aria-hidden />
              Open source
            </Button>
          )}
          {pin.hasImage ? (
            <div id="boards-pin-dialog-image-actions" className="flex gap-2">
              <Button
                id="boards-pin-dialog-copy"
                className="flex-1"
                onClick={() =>
                  void copyPinImage(pin.id)
                    .then(() => showNotice("Image copied"))
                    .catch(onError)
                }
              >
                <Copy id="boards-pin-dialog-copy-icon" className="size-4" aria-hidden />
                Copy
              </Button>
              <Button id="boards-pin-dialog-save" className="flex-1" onClick={() => void saveAs()}>
                <Save id="boards-pin-dialog-save-icon" className="size-4" aria-hidden />
                Save as…
              </Button>
            </div>
          ) : null}
          {pin.hasImage && board ? (
            <Button
              id="boards-pin-dialog-cover"
              variant="ghost"
              disabled={isCover}
              onClick={() => void setBoardCover(board.id, pin.id).catch(onError)}
            >
              <Star id="boards-pin-dialog-cover-icon" className={cn("size-4", isCover && "fill-current text-amber-300")} aria-hidden />
              {isCover ? "Board cover" : "Use as board cover"}
            </Button>
          ) : null}
          <Button id="boards-pin-dialog-delete" variant="ghost" className="text-red-300 hover:bg-red-950/60" onClick={() => onDelete([pin.id])}>
            <Trash2 id="boards-pin-dialog-delete-icon" className="size-4" aria-hidden />
            Delete pin
          </Button>
        </div>
      </aside>
    </Dialog>
  );
}

function NavButton({ id, label, side, onClick, children }: { id: string; label: string; side: "left" | "right"; onClick: () => void; children: ReactNode }) {
  return (
    <button
      id={id}
      type="button"
      title={label}
      aria-label={label}
      className={cn(
        "fixed top-1/2 z-50 inline-flex size-10 -translate-y-1/2 items-center justify-center rounded-full bg-black/60 text-white hover:bg-black/80 [&_svg]:size-5",
        side === "left" ? "left-4" : "right-4",
      )}
      onClick={(event) => {
        event.stopPropagation();
        onClick();
      }}
    >
      {children}
    </button>
  );
}
