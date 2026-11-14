import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Check, ImagePlus, LoaderCircle, Plus } from "lucide-react";
import { Button } from "@/components/Button";
import { PopupFrame } from "@/components/PopupFrame";
import { closePopup, createBoard, sendBoardDropSignal, showNotice } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { PopoverHeader } from "../../PopoverParts";
import { readPopoverTab } from "../../shared";
import { errorText } from "../../use-page-tool";
import { boardChoices, boardLabel } from "../board-tree";
import { isBoardFileDrag, readBoardDrop, saveBoardDrop, savedDropMessage, type BoardDropPayload } from "../drop-save";
import { useBoards } from "../use-boards";

const SAVED_MS = 700;
const NEW_TARGET = "new";

const nameClass =
  "h-8 min-w-0 flex-1 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500";

type Row = { id: string; label: string };

type DropHit = { elementId: string; boardId: string; fresh: boolean };

function rowDomId(id: string) {
  return `board-drop-row-${id}`;
}

function boardIdFromTarget(value: string | undefined) {
  if (!value || value === "unsorted") {
    return "";
  }
  return value;
}

function hitAt(target: EventTarget | null): DropHit | null {
  const element = target instanceof Element ? target.closest<HTMLElement>("[data-drop-target]") : null;
  if (!element) {
    return null;
  }
  if (element.dataset.dropTarget === NEW_TARGET) {
    return { elementId: element.id, boardId: "", fresh: true };
  }
  return { elementId: element.id, boardId: boardIdFromTarget(element.dataset.dropTarget), fresh: false };
}

export function BoardDropPopover({ params }: { params: URLSearchParams }) {
  const tab = readPopoverTab(params);
  const { boards, loaded } = useBoards();
  const [hot, setHot] = useState<string | null>(null);
  const [phase, setPhase] = useState<"ready" | "name" | "saving" | "saved" | "error">("ready");
  const [message, setMessage] = useState("");
  const [name, setName] = useState("");
  const hold = useRef(false);
  const signaled = useRef(false);
  const closeTimer = useRef<number | null>(null);
  const pending = useRef<BoardDropPayload | null>(null);
  const createdId = useRef<string | null>(null);
  const creating = useRef(false);
  const alive = useRef(true);
  const pageUrlRef = useRef(tab.url);
  const boardsRef = useRef(boards);
  const tabIdRef = useRef(tab.id);
  const nameRef = useRef<HTMLInputElement>(null);
  pageUrlRef.current = tab.url;
  boardsRef.current = boards;
  tabIdRef.current = tab.id;

  const choices = boardChoices(boards);
  const lastId = boards.lastBoardId && choices.some((choice) => choice.board.id === boards.lastBoardId) ? boards.lastBoardId : "";
  const rows: Row[] = choices
    .map((choice) => ({ id: choice.board.id, label: choice.label }))
    .sort((left, right) => {
      if (left.id === lastId) {
        return -1;
      }
      if (right.id === lastId) {
        return 1;
      }
      return left.label.localeCompare(right.label, undefined, { sensitivity: "base", numeric: true });
    });
  const hotRow = rows.find((row) => rowDomId(row.id) === hot);

  useEffect(
    () => () => {
      alive.current = false;
      if (closeTimer.current !== null) {
        window.clearTimeout(closeTimer.current);
      }
    },
    [],
  );

  useEffect(() => {
    if (phase !== "name") {
      return;
    }
    void getCurrentWindow()
      .setFocus()
      .catch(() => undefined)
      .finally(() => nameRef.current?.focus());
  }, [phase]);

  useEffect(() => {
    const alive = { current: true };
    const mark = (state: "over" | "idle") => {
      if (state === "over") {
        if (signaled.current) {
          return;
        }
        signaled.current = true;
      } else {
        signaled.current = false;
      }
      void sendBoardDropSignal(state);
    };

    const nameFor = (boardId: string) => {
      const snapshot = boardsRef.current;
      const board = snapshot.boards.find((item) => item.id === boardId);
      if (board) {
        return boardLabel(snapshot, board);
      }
      return boardId ? "board" : "Unsorted";
    };

    const saved = (boardId: string, result: { pins: { boardId: string }[]; skipped: number }) => {
      if (!alive.current) {
        return;
      }
      const savedId = result.pins[0]?.boardId || boardId;
      const text = savedDropMessage(result.pins.length, nameFor(savedId), result.skipped);
      setPhase("saved");
      setMessage(text);
      void showNotice(text).catch(() => undefined);
      closeTimer.current = window.setTimeout(() => {
        closeTimer.current = null;
        void sendBoardDropSignal("done");
        void closePopup();
      }, SAVED_MS);
    };

    const fail = (error: unknown) => {
      if (!alive.current) {
        return;
      }
      hold.current = false;
      pending.current = null;
      setPhase("error");
      setMessage(errorText(error));
    };

    const finish = (boardId: string, drop: BoardDropPayload) => {
      hold.current = true;
      mark("over");
      setPhase("saving");
      setMessage("");
      void saveBoardDrop(drop, boardId, tabIdRef.current)
        .then((result) => saved(boardId, result))
        .catch(fail);
    };

    let idleTimer: number | null = null;
    const cancelIdle = () => {
      if (idleTimer !== null) {
        window.clearTimeout(idleTimer);
        idleTimer = null;
      }
    };

    const onDragOver = (event: DragEvent) => {
      if (hold.current || !event.dataTransfer || !isBoardFileDrag(event.dataTransfer)) {
        return;
      }
      event.preventDefault();
      event.dataTransfer.dropEffect = "copy";
      cancelIdle();
      const next = hitAt(event.target)?.elementId ?? null;
      setHot((current) => (current === next ? current : next));
      if (next) {
        mark("over");
      }
    };

    const onDrop = (event: DragEvent) => {
      if (hold.current || !event.dataTransfer || !isBoardFileDrag(event.dataTransfer)) {
        return;
      }
      event.preventDefault();
      cancelIdle();
      const target = hitAt(event.target);
      if (!target) {
        return;
      }
      const drop = readBoardDrop(event.dataTransfer, pageUrlRef.current);
      hold.current = true;
      mark("over");
      setHot(target.elementId);
      if (target.fresh) {
        if (drop.files.length === 0 && drop.paths.length === 0 && drop.imageUrls.length === 0 && drop.links.length === 0) {
          hold.current = false;
          setPhase("error");
          setMessage("Drop an image, file, or link.");
          return;
        }
        pending.current = drop;
        createdId.current = null;
        setName("");
        setMessage("");
        setPhase("name");
        return;
      }
      finish(target.boardId, drop);
    };

    const onDragLeave = (event: DragEvent) => {
      if (hold.current) {
        return;
      }
      const next = event.relatedTarget;
      if (next instanceof Node && document.documentElement.contains(next)) {
        return;
      }
      cancelIdle();
      idleTimer = window.setTimeout(() => {
        idleTimer = null;
        setHot(null);
        mark("idle");
      }, 80);
    };

    window.addEventListener("dragenter", onDragOver);
    window.addEventListener("dragover", onDragOver);
    window.addEventListener("drop", onDrop);
    window.addEventListener("dragleave", onDragLeave);
    return () => {
      alive.current = false;
      cancelIdle();
      window.removeEventListener("dragenter", onDragOver);
      window.removeEventListener("dragover", onDragOver);
      window.removeEventListener("drop", onDrop);
      window.removeEventListener("dragleave", onDragLeave);
    };
  }, []);

  const createNamed = () => {
    const trimmed = name.trim();
    const drop = pending.current;
    if (creating.current || !trimmed || !drop) {
      return;
    }
    creating.current = true;
    setPhase("saving");
    setMessage("");
    void (async () => {
      try {
        const boardId = createdId.current ?? (await createBoard(trimmed, null)).id;
        createdId.current = boardId;
        const result = await saveBoardDrop(drop, boardId, tabIdRef.current);
        if (!alive.current) {
          return;
        }
        const text = savedDropMessage(result.pins.length, trimmed, result.skipped);
        setPhase("saved");
        setMessage(text);
        await showNotice(text).catch(() => undefined);
        if (!alive.current) {
          return;
        }
        closeTimer.current = window.setTimeout(() => {
          closeTimer.current = null;
          void sendBoardDropSignal("done");
          void closePopup();
        }, SAVED_MS);
      } catch (error) {
        creating.current = false;
        if (!alive.current) {
          return;
        }
        setPhase("name");
        setMessage(errorText(error));
      }
    })();
  };

  const dismiss = () => {
    void sendBoardDropSignal("done");
    void closePopup();
  };

  const subtitle =
    phase === "name"
      ? "Name the new board"
      : phase === "saving"
        ? "Saving…"
        : phase === "saved"
          ? message
          : phase === "error"
            ? "Couldn't save that"
            : hot === "board-drop-new"
              ? "Release to add a new board"
              : hotRow
                ? `Release to save to ${hotRow.label}`
                : "Drop on a board";

  return (
    <PopupFrame id="board-drop" width={300} maxHeight={520} ready={loaded}>
      <div id="board-drop-surface" className="flex w-full flex-col">
        <PopoverHeader id="board-drop-header" icon={<ImagePlus aria-hidden />} title="Drop to save" subtitle={subtitle} />
        <div id="board-drop-body" className="flex flex-col gap-2 px-3 pb-3">
          {phase === "name" ? (
            <form
              id="board-drop-name-form"
              className="flex flex-col gap-2"
              onSubmit={(event) => {
                event.preventDefault();
                createNamed();
              }}
            >
              <label id="board-drop-name-label" htmlFor="board-drop-name" className="text-xs text-zinc-400">
                Board name
              </label>
              <input
                id="board-drop-name"
                ref={nameRef}
                value={name}
                placeholder="New board name"
                className={nameClass}
                onChange={(event) => setName(event.target.value)}
              />
              {message ? (
                <p id="board-drop-name-error" className="text-xs text-amber-300">
                  {message}
                </p>
              ) : null}
              <div id="board-drop-name-actions" className="flex gap-2">
                <Button id="board-drop-name-cancel" variant="ghost" className="flex-1" type="button" onClick={dismiss}>
                  Cancel
                </Button>
                <Button id="board-drop-name-create" variant="primary" className="flex-1" type="submit" disabled={!name.trim()}>
                  <Plus id="board-drop-name-create-icon" className="size-4" aria-hidden />
                  Create
                </Button>
              </div>
            </form>
          ) : phase === "saving" || phase === "saved" ? (
            <div
              id="board-drop-status"
              className={cn(
                "flex min-h-16 flex-col items-center justify-center gap-1 rounded-xl px-3 py-4 text-center",
                phase === "saved" ? "bg-sky-500/10 text-sky-100" : "bg-zinc-950/40 text-zinc-300",
              )}
            >
              {phase === "saving" ? (
                <LoaderCircle id="board-drop-status-icon" className="size-5 animate-spin" aria-hidden />
              ) : (
                <Check id="board-drop-status-icon" className="size-5" aria-hidden />
              )}
              <p id="board-drop-status-label" className="text-sm font-medium">
                {phase === "saving" ? "Saving…" : message}
              </p>
            </div>
          ) : (
            <ul id="board-drop-list" className="flex max-h-72 flex-col gap-0.5 overflow-y-auto" aria-label="Boards">
              {rows.map((row) => {
                const domId = rowDomId(row.id);
                const active = hot === domId;
                return (
                  <li id={`${domId}-item`} key={row.id}>
                    <div
                      id={domId}
                      data-drop-target={row.id}
                      className={cn(
                        "flex h-9 items-center gap-2 rounded-lg px-2.5 text-sm",
                        active ? "bg-sky-500/20 text-sky-100 ring-1 ring-sky-400" : "text-zinc-200",
                      )}
                    >
                      <span id={`${domId}-label`} className="min-w-0 flex-1 truncate">
                        {row.label}
                      </span>
                      {row.id === lastId ? (
                        <span id={`${domId}-mark`} className="shrink-0 text-[10px] uppercase tracking-wide text-zinc-500">
                          Last used
                        </span>
                      ) : null}
                    </div>
                  </li>
                );
              })}
              <li id="board-drop-new-item">
                <div
                  id="board-drop-new"
                  data-drop-target={NEW_TARGET}
                  className={cn(
                    "flex h-9 items-center gap-2 rounded-lg px-2.5 text-sm",
                    hot === "board-drop-new" ? "bg-sky-500/20 text-sky-100 ring-1 ring-sky-400" : "text-zinc-300",
                  )}
                >
                  <Plus id="board-drop-new-icon" className="size-4 shrink-0" aria-hidden />
                  <span id="board-drop-new-label" className="min-w-0 flex-1 truncate">
                    Add new board
                  </span>
                </div>
              </li>
            </ul>
          )}
          {phase === "error" ? (
            <>
              <p id="board-drop-error" className="text-xs text-amber-300">
                {message}
              </p>
              <Button id="board-drop-close" variant="ghost" onClick={dismiss}>
                Close
              </Button>
            </>
          ) : null}
        </div>
      </div>
    </PopupFrame>
  );
}
