import { useRef } from "react";
import { Check, LayoutGrid, LoaderCircle } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { openPopupBelow, pinElement, pinLink, pinPage, pinRegion } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import type { PageToolRequest, Pin, TabInfo } from "@type/browser";
import { capturable, tabParams } from "../shared";
import { errorText, usePageToolRequests, useToolStatus } from "../use-page-tool";

const POPOVER_WIDTH = 340;
/** Placeholder only. The popover stays hidden until it reports its real height. */
const POPOVER_HEIGHT = 560;
const IMAGES_WIDTH = 440;
const IMAGES_HEIGHT = 480;

type BoardsRequest = Extract<PageToolRequest, { tool: "boards" }>;

export function BoardsButton({ activeTab }: { activeTab: TabInfo | undefined }) {
  const buttonRef = useRef<HTMLButtonElement>(null);
  const [status, setStatus] = useToolStatus();

  const showPopover = (kind: "boards" | "board-images", params: URLSearchParams) => {
    const anchor = buttonRef.current;
    if (anchor) {
      void openPopupBelow(anchor, {
        kind,
        params: params.toString(),
        width: kind === "boards" ? POPOVER_WIDTH : IMAGES_WIDTH,
        height: kind === "boards" ? POPOVER_HEIGHT : IMAGES_HEIGHT,
      });
    }
  };

  const open = () => {
    if (activeTab) {
      showPopover("boards", new URLSearchParams(tabParams(activeTab)));
    }
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

  const label =
    status === "busy" ? "Saving to board…" : status === "done" ? "Saved to board" : status === "failed" ? "Couldn't save to board" : "Boards";

  return (
    <IconButton
      id="address-boards"
      ref={buttonRef}
      label={label}
      disabled={status === "busy" || !capturable(activeTab?.url)}
      className={cn(status === "done" && "text-sky-300", status === "failed" && "text-amber-300")}
      onClick={open}
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
