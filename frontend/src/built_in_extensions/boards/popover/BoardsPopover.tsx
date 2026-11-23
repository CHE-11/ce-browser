import { useEffect, useRef, useState } from "react";
import { Check, CircleAlert, ClipboardPaste, Crop, ExternalLink, ImagePlus, Images, LayoutGrid, Link2, LoaderCircle, Monitor, MousePointerClick, RotateCcw, ScrollText, Trash2 } from "lucide-react";
import { Button } from "@/components/Button";
import { IconButton } from "@/components/IconButton";
import { PopupFrame } from "@/components/PopupFrame";
import { closePopup, createTab, currentWindowLabel, movePins, pasteBoardClipboard, requestPageTool, showNotice, trashPins } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import type { PageToolRequest } from "@type/browser";

import { ActionTile, PopoverHeader } from "../../PopoverParts";
import { capturable, readPopoverTab } from "../../shared";
import { errorText } from "../../use-page-tool";
import { FolderPicker, folderTargetName, resolveFolder } from "../FolderPicker";
import { isBoardFileDrag, readBoardDrop, saveBoardDrop, savedDropMessage } from "../drop-save";
import { UNCATEGORIZED, hasPreview, previewUrl } from "../folder-tree";
import { useBoards } from "../use-boards";

type DropState = { phase: "saving" | "saved" | "error"; message: string };

type BoardsAction = Extract<PageToolRequest, { tool: "boards" }>["action"];

function openBoardsPage(folderId?: string) {
  void createTab(folderId && folderId !== UNCATEGORIZED ? `ce://boards?folder=${encodeURIComponent(folderId)}` : "ce://boards");
  void closePopup();
}

function requestedFolder(params: URLSearchParams) {
  return params.get("folder") ?? params.get("board") ?? "";
}

export function BoardsPopover({ params }: { params: URLSearchParams }) {
  const tab = readPopoverTab(params);
  const canCapture = capturable(tab.url);
  const status = params.get("status");
  const { boards, loaded } = useBoards();
  const [folderId, setFolderId] = useState(() => requestedFolder(params));
  const [showResult, setShowResult] = useState(status !== null);
  const [pasting, setPasting] = useState(false);
  const [pasteError, setPasteError] = useState("");
  const [pasted, setPasted] = useState<URLSearchParams | null>(null);
  const [dragging, setDragging] = useState(false);
  const [dropState, setDropState] = useState<DropState | null>(null);
  const latest = useRef({ folderId, boards, tab });
  latest.current = { folderId, boards, tab };

  useEffect(() => {
    let alive = true;
    let leaveTimer: number | null = null;
    const cancelLeave = () => {
      if (leaveTimer !== null) {
        window.clearTimeout(leaveTimer);
        leaveTimer = null;
      }
    };

    const onDragOver = (event: DragEvent) => {
      if (!event.dataTransfer || !isBoardFileDrag(event.dataTransfer)) {
        return;
      }
      event.preventDefault();
      event.dataTransfer.dropEffect = "copy";
      cancelLeave();
      setDragging(true);
    };

    const onDragLeave = (event: DragEvent) => {
      const next = event.relatedTarget;
      if (next instanceof Node && document.documentElement.contains(next)) {
        return;
      }
      cancelLeave();
      leaveTimer = window.setTimeout(() => {
        leaveTimer = null;
        setDragging(false);
      }, 80);
    };

    const onDrop = (event: DragEvent) => {
      if (!event.dataTransfer || !isBoardFileDrag(event.dataTransfer)) {
        return;
      }
      event.preventDefault();
      cancelLeave();
      setDragging(false);
      const { folderId: target, tab: source } = latest.current;
      const drop = readBoardDrop(event.dataTransfer, source.url);
      setPasted(null);
      setShowResult(false);
      setDropState({ phase: "saving", message: "" });
      void saveBoardDrop(drop, target, source.id)
        .then((result) => {
          if (!alive) {
            return;
          }
          const text = savedDropMessage(result, folderTargetName(latest.current.boards, target));
          setDropState({ phase: "saved", message: text });
          void showNotice(text).catch(() => undefined);
        })
        .catch((reason: unknown) => {
          if (alive) {
            setDropState({ phase: "error", message: errorText(reason) });
          }
        });
    };

    window.addEventListener("dragenter", onDragOver);
    window.addEventListener("dragover", onDragOver);
    window.addEventListener("dragleave", onDragLeave);
    window.addEventListener("drop", onDrop);
    return () => {
      alive = false;
      cancelLeave();
      window.removeEventListener("dragenter", onDragOver);
      window.removeEventListener("dragover", onDragOver);
      window.removeEventListener("dragleave", onDragLeave);
      window.removeEventListener("drop", onDrop);
    };
  }, []);

  const paste = () => {
    setPasting(true);
    setPasteError("");
    void pasteBoardClipboard(folderId)
      .then((result) => {
        const pin = result.pins[0];
        const next = new URLSearchParams();
        next.set("status", "saved");
        next.set("pin", pin?.id ?? "");
        next.set("folder", pin?.folderIds[0] ?? resolveFolder(boards, folderId));
        setPasted(next);
      })
      .catch((reason: unknown) => setPasteError(errorText(reason)))
      .finally(() => setPasting(false));
  };

  const send = (action: BoardsAction) => {
    void requestPageTool({
      tool: "boards",
      action,
      window: currentWindowLabel(),
      tabId: tab.id,
      title: tab.title,
      url: tab.url,
      folderId,
    });
    void closePopup();
  };

  return (
    <PopupFrame id="boards-popover" width={340} maxHeight={760} ready={loaded}>
      <PopoverHeader
        id="boards-header"
        icon={<LayoutGrid aria-hidden />}
        title="Save to Boards"
        subtitle={tab.title || tab.url}
        action={
          <IconButton id="boards-open" label="Open Boards" onClick={() => openBoardsPage(undefined)}>
            <ExternalLink id="boards-open-icon" aria-hidden />
          </IconButton>
        }
      />
      {pasted ? (
        <SaveResult params={pasted} onAgain={() => setPasted(null)} />
      ) : showResult && status ? (
        <SaveResult params={params} onAgain={() => setShowResult(false)} />
      ) : (
        <div id="boards-form" className="flex flex-col gap-2 px-3 pb-3">
          <FolderPicker id="boards-picker" boards={boards} value={folderId} onChange={setFolderId} />
          <div
            id="boards-drop-zone"
            className={cn(
              "flex min-h-11 items-center justify-center gap-2 rounded-lg border border-dashed px-3 py-2 text-center text-xs",
              dragging
                ? "border-sky-400 bg-sky-500/15 text-sky-100"
                : dropState?.phase === "error"
                  ? "border-amber-500/50 bg-amber-500/10 text-amber-200"
                  : dropState?.phase === "saved"
                    ? "border-sky-500/40 bg-sky-500/10 text-sky-100"
                    : "border-zinc-700 text-zinc-500",
            )}
          >
            {dragging ? (
              <>
                <ImagePlus id="boards-drop-zone-icon" className="size-4 shrink-0" aria-hidden />
                <span id="boards-drop-zone-label">Release to save to {folderTargetName(boards, folderId)}</span>
              </>
            ) : dropState?.phase === "saving" ? (
              <>
                <LoaderCircle id="boards-drop-zone-icon" className="size-4 shrink-0 animate-spin" aria-hidden />
                <span id="boards-drop-zone-label">Saving…</span>
              </>
            ) : dropState ? (
              <>
                {dropState.phase === "saved" ? (
                  <Check id="boards-drop-zone-icon" className="size-4 shrink-0" aria-hidden />
                ) : (
                  <CircleAlert id="boards-drop-zone-icon" className="size-4 shrink-0" aria-hidden />
                )}
                <span id="boards-drop-zone-label" className="wrap-break-word">
                  {dropState.message}
                </span>
              </>
            ) : (
              <>
                <ImagePlus id="boards-drop-zone-icon" className="size-4 shrink-0" aria-hidden />
                <span id="boards-drop-zone-label">Drag an image, GIF, file or link here</span>
              </>
            )}
          </div>
          {canCapture ? null : (
            <p id="boards-not-web" className="rounded-lg bg-zinc-800/60 px-3 py-2 text-xs text-zinc-400">
              Saving works on web pages. Open a site to save from it, or open Boards below.
            </p>
          )}
          <ActionTile
            id="boards-visible"
            primary={canCapture}
            disabled={!canCapture}
            icon={<Monitor aria-hidden />}
            label="Visible area"
            hint="What is on screen now"
            onClick={() => send("page")}
          />
          <ActionTile
            id="boards-full"
            disabled={!canCapture}
            icon={<ScrollText aria-hidden />}
            label="Entire page"
            hint="The whole page, top to bottom"
            onClick={() => send("full-page")}
          />
          <ActionTile id="boards-region" disabled={!canCapture} icon={<Crop aria-hidden />} label="Region" hint="Drag a rectangle on the page" onClick={() => send("region")} />
          <ActionTile
            id="boards-element"
            disabled={!canCapture}
            icon={<MousePointerClick aria-hidden />}
            label="Page section"
            hint="Point at a part of the page and click it"
            onClick={() => send("element")}
          />
          <ActionTile
            id="boards-link"
            disabled={!canCapture}
            icon={<Link2 aria-hidden />}
            label="Link card"
            hint="Title, description and preview image"
            onClick={() => send("link")}
          />
          <ActionTile
            id="boards-images"
            disabled={!canCapture}
            icon={<Images aria-hidden />}
            label="Images on this page"
            hint="Pick one or more images"
            onClick={() => send("images")}
          />
          <ActionTile
            id="boards-paste"
            disabled={pasting}
            icon={<ClipboardPaste aria-hidden />}
            label="Paste from clipboard"
            hint={pasting ? "Pasting…" : "Image, file, or link"}
            onClick={paste}
          />
          {pasteError ? (
            <p id="boards-paste-error" className="text-xs text-amber-300">
              {pasteError}
            </p>
          ) : null}
        </div>
      )}
    </PopupFrame>
  );
}

function SaveResult({ params, onAgain }: { params: URLSearchParams; onAgain: () => void }) {
  const { boards } = useBoards();
  const failed = params.get("status") === "error";
  const pinId = params.get("pin");
  const [folderId, setFolderId] = useState(() => requestedFolder(params) || UNCATEGORIZED);
  const [moveError, setMoveError] = useState("");
  const [removing, setRemoving] = useState(false);
  const pin = pinId && !failed ? boards.pins.find((item) => item.id === pinId) : undefined;
  const thumbnail = pin && hasPreview(pin) ? previewUrl(pin) : "";

  const remove = () => {
    if (!pinId || removing) {
      return;
    }
    setRemoving(true);
    setMoveError("");
    void trashPins([pinId])
      .then(() => onAgain())
      .catch((reason: unknown) => {
        setMoveError(errorText(reason));
        setRemoving(false);
      });
  };

  const changeFolder = (next: string) => {
    if (!pinId || next === folderId) {
      setFolderId(next);
      return;
    }
    const previous = folderId;
    setFolderId(next);
    setMoveError("");
    void movePins([pinId], null, next).catch((reason: unknown) => {
      setFolderId(previous);
      setMoveError(errorText(reason));
    });
  };

  return (
    <div id="boards-result" className="flex flex-col gap-2 px-3 pb-3">
      {failed ? (
        <div id="boards-result-error" className="rounded-lg bg-amber-500/10 p-3 text-amber-200">
          <p id="boards-result-error-title" className="flex items-center gap-2 text-sm font-medium">
            <CircleAlert id="boards-result-error-icon" className="size-4 shrink-0" aria-hidden />
            Couldn't save to Boards
          </p>
          <p id="boards-result-error-detail" className="mt-1 wrap-break-word text-xs opacity-80">
            {params.get("message")}
          </p>
        </div>
      ) : (
        <>
          {thumbnail ? <img id="boards-result-thumb" src={thumbnail} alt="" className="max-h-56 w-full rounded-lg bg-zinc-800 object-contain" /> : null}
          <p id="boards-result-message" className="truncate text-sm text-sky-100">
            Saved to{" "}
            <span id="boards-result-board" className="font-medium">
              {folderTargetName(boards, folderId)}
            </span>
          </p>
          <FolderPicker id="boards-result-picker" boards={boards} value={folderId} onChange={changeFolder} />
          {moveError ? (
            <p id="boards-result-move-error" className="text-xs text-amber-200">
              {moveError}
            </p>
          ) : null}
        </>
      )}
      <div id="boards-result-actions" className="flex flex-col gap-2">
        {failed || !pinId ? null : (
          <Button id="boards-result-delete" variant="destructiveOutline" disabled={removing} onClick={remove}>
            <Trash2 id="boards-result-delete-icon" className="size-4" aria-hidden />
            {removing ? "Moving to Trash…" : "Move to Trash"}
          </Button>
        )}
        <div id="boards-result-actions-row" className="flex gap-2">
          {failed ? null : (
            <Button id="boards-result-open" variant="primary" className="flex-1" disabled={removing} onClick={() => openBoardsPage(folderId)}>
              <ExternalLink id="boards-result-open-icon" className="size-4" aria-hidden />
              Open folder
            </Button>
          )}
          <Button id="boards-result-again" className="flex-1" disabled={removing} onClick={onAgain}>
            <RotateCcw id="boards-result-again-icon" className="size-4" aria-hidden />
            {failed ? "Try again" : "Save another"}
          </Button>
        </div>
      </div>
    </div>
  );
}
