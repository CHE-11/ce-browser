import { useEffect, useRef, useState, type DragEvent as ReactDragEvent } from "react";
import { Check, LayoutGrid, LoaderCircle } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { closePopup, currentWindowLabel, listBoards, onBoardDropSignal, openPopup, pinElement, pinLink, pinPage, pinRegion, showNotice } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import type { PageToolRequest, Pin, TabInfo } from "@type/browser";
import { errorText, usePageToolRequests, useToolStatus } from "../../use-page-tool";
import { tabParams } from "../../shared";
import { boardLabel } from "../board-tree";
import { isBoardFileDrag, readBoardDrop, saveBoardDrop, savedDropMessage } from "../drop-save";

type BoardsRequest = Extract<PageToolRequest, { tool: "boards" }>;

/** How long the drop popover stays open after the pointer leaves, so it can be reached. */
const DROP_CLOSE_MS = 600;

export function BoardsButton({ activeTab }: { activeTab: TabInfo | undefined }) {
  const buttonRef = useRef<HTMLButtonElement>(null);
  const [status, setStatus] = useToolStatus();
  const [armed, setArmed] = useState(false);
  const dropOpen = useRef(false);
  const overButton = useRef(false);
  const overPopup = useRef(false);
  const closeTimer = useRef<number | null>(null);
  const generation = useRef(0);
  const ignoreClick = useRef(false);
  const tabRef = useRef(activeTab);
  tabRef.current = activeTab;
  const scheduleCloseRef = useRef<() => void>(() => undefined);
  const lastOver = useRef(0);

  const cancelClose = () => {
    if (closeTimer.current !== null) {
      window.clearTimeout(closeTimer.current);
      closeTimer.current = null;
    }
  };

  const scheduleClose = () => {
    cancelClose();
    const ticket = generation.current;
    closeTimer.current = window.setTimeout(() => {
      closeTimer.current = null;
      if (ticket !== generation.current || overButton.current || overPopup.current || !dropOpen.current) {
        return;
      }
      dropOpen.current = false;
      setArmed(false);
      void closePopup();
    }, DROP_CLOSE_MS);
  };
  scheduleCloseRef.current = scheduleClose;

  useEffect(() => {
    const signals = onBoardDropSignal((signal) => {
      if (signal.window !== currentWindowLabel()) {
        return;
      }
      if (signal.state === "done") {
        overPopup.current = false;
        dropOpen.current = false;
        if (closeTimer.current !== null) {
          window.clearTimeout(closeTimer.current);
          closeTimer.current = null;
        }
        if (!overButton.current) {
          setArmed(false);
        }
        return;
      }
      overPopup.current = signal.state === "over";
      if (signal.state === "over") {
        if (closeTimer.current !== null) {
          window.clearTimeout(closeTimer.current);
          closeTimer.current = null;
        }
        return;
      }
      if (!overButton.current) {
        scheduleCloseRef.current();
      }
    });
    // A drag that leaves this webview for the popover does not always emit dragleave.
    const watch = window.setInterval(() => {
      if (!overButton.current || Date.now() - lastOver.current < 200) {
        return;
      }
      overButton.current = false;
      setArmed(false);
      scheduleCloseRef.current();
    }, 100);
    return () => {
      window.clearInterval(watch);
      cancelClose();
      void signals.then((stop) => stop());
    };
  }, []);

  const showPopover = (kind: "boards" | "board-images", params: URLSearchParams) => {
    const anchor = buttonRef.current;
    if (anchor) {
      void openPopup(kind, anchor, { params: params.toString() });
    }
  };

  const openDrop = () => {
    const tab = tabRef.current;
    const anchor = buttonRef.current;
    if (!tab || !anchor || dropOpen.current) {
      return;
    }
    dropOpen.current = true;
    generation.current += 1;
    void openPopup("board-drop", anchor, { params: tabParams(tab), focus: false }).then(
      (opened) => {
        if (!opened) {
          dropOpen.current = false;
        }
      },
      () => {
        dropOpen.current = false;
      },
    );
  };

  const open = () => {
    if (ignoreClick.current) {
      ignoreClick.current = false;
      return;
    }
    generation.current += 1;
    cancelClose();
    dropOpen.current = false;
    overPopup.current = false;
    setArmed(false);
    if (activeTab) {
      showPopover("boards", new URLSearchParams(tabParams(activeTab)));
    }
  };

  const allowDrop = (event: ReactDragEvent<HTMLButtonElement>) => {
    if (!isBoardFileDrag(event.dataTransfer)) {
      return;
    }
    event.preventDefault();
    event.dataTransfer.dropEffect = "copy";
    lastOver.current = Date.now();
    overButton.current = true;
    setArmed(true);
    cancelClose();
    openDrop();
  };

  const leaveDrop = (event: ReactDragEvent<HTMLButtonElement>) => {
    const next = event.relatedTarget;
    if (next instanceof Node && event.currentTarget.contains(next)) {
      return;
    }
    overButton.current = false;
    lastOver.current = 0;
    setArmed(false);
    scheduleClose();
  };

  const dropOnButton = (event: ReactDragEvent<HTMLButtonElement>) => {
    const tab = tabRef.current;
    if (!tab || !isBoardFileDrag(event.dataTransfer)) {
      return;
    }
    event.preventDefault();
    const drop = readBoardDrop(event.dataTransfer, tab.url);
    ignoreClick.current = true;
    generation.current += 1;
    cancelClose();
    dropOpen.current = false;
    overButton.current = false;
    overPopup.current = false;
    setArmed(false);
    void closePopup();
    setStatus("busy");
    void saveBoardDrop(drop, "", tab.id)
      .then(async (saved) => {
        const snapshot = await listBoards().catch(() => null);
        const savedId = saved.pins[0]?.boardId ?? "";
        const board = snapshot?.boards.find((item) => item.id === savedId);
        const name = board && snapshot ? boardLabel(snapshot, board) : "Unsorted";
        setStatus("done");
        await showNotice(savedDropMessage(saved.pins.length, name, saved.skipped)).catch(() => undefined);
      })
      .catch((error: unknown) => {
        setStatus("failed");
        void showNotice(errorText(error)).catch(() => undefined);
      });
  };

  const capture = (request: BoardsRequest): Promise<Pin> => {
    switch (request.action) {
      case "full-page":
        return pinPage(request.tabId, request.boardId, true);
      case "region":
        return pinRegion(request.tabId, request.boardId);
      case "element":
        return pinElement(request.tabId, request.boardId);
      case "link":
        return pinLink(request.tabId, request.boardId);
      default:
        return pinPage(request.tabId, request.boardId, false);
    }
  };

  const run = async (request: BoardsRequest) => {
    const result = new URLSearchParams({ tab: String(request.tabId), title: request.title, url: request.url });
    if (request.action === "images") {
      result.set("board", request.boardId);
      showPopover("board-images", result);
      return;
    }
    setStatus("busy");
    try {
      const pin = await capture(request);
      result.set("status", "saved");
      result.set("pin", pin.id);
      result.set("board", pin.boardId);
      setStatus("done");
    } catch (error) {
      const message = errorText(error);
      if (/cancelled/i.test(message)) {
        setStatus("idle");
        return;
      }
      result.set("status", "error");
      result.set("message", message);
      setStatus("failed");
    }
    showPopover("boards", result);
  };

  usePageToolRequests("boards", (request) => void run(request));

  const label = armed
    ? "Drop to save to a board"
    : status === "busy"
      ? "Saving to board…"
      : status === "done"
        ? "Saved to board"
        : status === "failed"
          ? "Couldn't save to board"
          : "Boards";

  return (
    <IconButton
      id="address-boards"
      ref={buttonRef}
      label={label}
      disabled={status === "busy" || !activeTab}
      className={cn(status === "done" && "text-sky-300", status === "failed" && "text-amber-300", armed && "bg-sky-500/20 text-sky-100 ring-2 ring-sky-400")}
      onClick={open}
      onDragEnter={allowDrop}
      onDragOver={allowDrop}
      onDragLeave={leaveDrop}
      onDrop={dropOnButton}
    >
      {status === "busy" ? (
        <LoaderCircle id="address-boards-icon" className="animate-spin" aria-hidden />
      ) : status === "done" ? (
        <Check id="address-boards-icon" aria-hidden />
      ) : (
        <LayoutGrid id="address-boards-icon" aria-hidden />
      )}
    </IconButton>
  );
}
