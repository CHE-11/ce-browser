import { useEffect, useState } from "react";
import { CircleAlert, Crop, ExternalLink, Images, LayoutGrid, Link2, Monitor, MousePointerClick, RotateCcw, ScrollText } from "lucide-react";
import { Button } from "@/components/Button";
import { IconButton } from "@/components/IconButton";
import { PopupFrame } from "@/components/PopupFrame";
import { closePopup, createTab, currentWindowLabel, movePins, requestPageTool } from "@/lib/browser-client";
import type { PageToolRequest } from "@type/browser";

import { ActionTile, PopoverHeader } from "../../PopoverParts";
import { capturable, readPopoverTab } from "../../shared";
import { errorText } from "../../use-page-tool";
import { BoardPicker } from "../BoardPicker";
import { boardLabel } from "../board-tree";
import { useThumbnail } from "../thumbnails";
import { useBoards } from "../use-boards";

type BoardsAction = Extract<PageToolRequest, { tool: "boards" }>["action"];

function openBoardsPage(boardId?: string) {
  void createTab(boardId ? `ce://boards?board=${encodeURIComponent(boardId)}` : "ce://boards");
  void closePopup();
}

export function BoardsPopover({ params }: { params: URLSearchParams }) {
  const tab = readPopoverTab(params);
  const canCapture = capturable(tab.url);
  const status = params.get("status");
  const { boards, loaded } = useBoards();
  const [boardId, setBoardId] = useState(params.get("board") ?? "");
  const [showResult, setShowResult] = useState(status !== null);

  useEffect(() => {
    if (!loaded) {
      return;
    }
    setBoardId((current) => {
      if (current && boards.boards.some((board) => board.id === current)) {
        return current;
      }
      return boards.lastBoardId ?? boards.boards[0]?.id ?? "";
    });
  }, [loaded, boards]);

  const send = (action: BoardsAction) => {
    void requestPageTool({
      tool: "boards",
      action,
      window: currentWindowLabel(),
      tabId: tab.id,
      title: tab.title,
      url: tab.url,
      boardId,
    });
    void closePopup();
  };

  return (
    <PopupFrame id="boards-popover" width={340} maxHeight={680} ready={loaded}>
      <PopoverHeader
        id="boards-header"
        icon={<LayoutGrid aria-hidden />}
        title="Save to board"
        subtitle={tab.title || tab.url}
        action={
          <IconButton id="boards-open" label="Open board" onClick={() => openBoardsPage(undefined)}>
            <ExternalLink id="boards-open-icon" aria-hidden />
          </IconButton>
        }
      />
      {showResult && status ? (
        <SaveResult params={params} onAgain={() => setShowResult(false)} />
      ) : (
        <div id="boards-form" className="flex flex-col gap-2 px-3 pb-3">
          <BoardPicker id="boards-picker" boards={boards} value={boardId} onChange={setBoardId} />
          {canCapture ? null : (
            <p id="boards-not-web" className="rounded-lg bg-zinc-800/60 px-3 py-2 text-xs text-zinc-400">
              Saving works on web pages. Open a site to save from it, or open your boards below.
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
        </div>
      )}
    </PopupFrame>
  );
}

function SaveResult({ params, onAgain }: { params: URLSearchParams; onAgain: () => void }) {
  const { boards } = useBoards();
  const failed = params.get("status") === "error";
  const pinId = params.get("pin");
  const [boardId, setBoardId] = useState(params.get("board") ?? "");
  const [moveError, setMoveError] = useState("");
  const thumbnail = useThumbnail(failed ? null : pinId);
  const board = boards.boards.find((item) => item.id === boardId);

  const changeBoard = (next: string) => {
    if (!pinId || next === boardId) {
      setBoardId(next);
      return;
    }
    const previous = boardId;
    setBoardId(next);
    setMoveError("");
    void movePins([pinId], next).catch((reason: unknown) => {
      setBoardId(previous);
      setMoveError(errorText(reason));
    });
  };

  return (
    <div id="boards-result" className="flex flex-col gap-2 px-3 pb-3">
      {failed ? (
        <div id="boards-result-error" className="rounded-lg bg-amber-500/10 p-3 text-amber-200">
          <p id="boards-result-error-title" className="flex items-center gap-2 text-sm font-medium">
            <CircleAlert id="boards-result-error-icon" className="size-4 shrink-0" aria-hidden />
            Couldn't save to the board
          </p>
          <p id="boards-result-error-detail" className="mt-1 wrap-break-word text-xs opacity-80">
            {params.get("message")}
          </p>
        </div>
      ) : (
        <>
          {thumbnail ? (
            <img id="boards-result-thumb" src={thumbnail} alt="" className="max-h-56 w-full rounded-lg bg-zinc-800 object-contain" />
          ) : null}
          <p id="boards-result-message" className="truncate text-sm text-sky-100">
            Saved to <span id="boards-result-board" className="font-medium">{board ? boardLabel(boards, board) : "board"}</span>
          </p>
          <BoardPicker id="boards-result-picker" boards={boards} value={boardId} onChange={changeBoard} />
          {moveError ? (
            <p id="boards-result-move-error" className="text-xs text-amber-200">
              {moveError}
            </p>
          ) : null}
        </>
      )}
      <div id="boards-result-actions" className="flex gap-2">
        {failed ? null : (
          <Button id="boards-result-open" variant="primary" className="flex-1" onClick={() => openBoardsPage(boardId || undefined)}>
            <ExternalLink id="boards-result-open-icon" className="size-4" aria-hidden />
            Open board
          </Button>
        )}
        <Button id="boards-result-again" className="flex-1" onClick={onAgain}>
          <RotateCcw id="boards-result-again-icon" className="size-4" aria-hidden />
          {failed ? "Try again" : "Save another"}
        </Button>
      </div>
    </div>
  );
}
