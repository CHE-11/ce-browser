import { useEffect, useState } from "react";
import { Columns2, SquareArrowOutUpRight, X } from "lucide-react";
import type { PeekInfo } from "../../../types/browser";
import { FavIcon } from "../components/FavIcon";
import { IconButton } from "../components/IconButton";
import { closePeek, currentWindowLabel, onPeekChanged, peekState, peekToSplit, peekToTab } from "../lib/browser-client";
import { useAppearanceState } from "../lib/use-browser";

/**
 * The card around a peek. The backend lays the peeked page over this view's body, so only
 * the header and a thin border show.
 */
export function PeekPage() {
  useAppearanceState();
  const [info, setInfo] = useState<PeekInfo | null>(null);

  useEffect(() => {
    document.title = "Peek";
    const label = currentWindowLabel();
    let disposed = false;
    void peekState().then((next) => {
      if (!disposed) {
        setInfo(next);
      }
    });
    const unlisten = onPeekChanged((next) => {
      if (!disposed && next && next.window === label) {
        setInfo(next);
      }
    });
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        void closePeek();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      disposed = true;
      window.removeEventListener("keydown", onKey);
      void unlisten.then((stop) => stop());
    };
  }, []);

  return (
    <div
      id="peek-frame"
      className="flex h-screen flex-col overflow-hidden rounded-[10px] bg-zinc-900 text-zinc-100 ring-1 ring-inset ring-zinc-700"
      onContextMenu={(event) => event.preventDefault()}
    >
      <header id="peek-header" className="flex h-10 shrink-0 items-center gap-2 pl-3 pr-1.5">
        <FavIcon id="peek-header-icon" url={info?.url ?? ""} src={info?.favicon ?? null} />
        <div id="peek-header-text" className="flex min-w-0 flex-1 items-baseline gap-2">
          <span id="peek-header-title" className="min-w-0 shrink truncate text-sm text-zinc-100">
            {info?.title || info?.url || "Loading"}
          </span>
          <span id="peek-header-url" className="min-w-0 flex-1 truncate text-xs text-zinc-500">
            {info?.url ?? ""}
          </span>
        </div>
        <IconButton id="peek-open-tab" label="Open as tab" className="size-7" onClick={() => void peekToTab()}>
          <SquareArrowOutUpRight id="peek-open-tab-icon" aria-hidden />
        </IconButton>
        <IconButton id="peek-open-split" label="Open in split view" className="size-7" onClick={() => void peekToSplit()}>
          <Columns2 id="peek-open-split-icon" aria-hidden />
        </IconButton>
        <IconButton id="peek-close" label="Close (Esc)" className="size-7" onClick={() => void closePeek()}>
          <X id="peek-close-icon" aria-hidden />
        </IconButton>
      </header>
      <div id="peek-body" className="min-h-0 flex-1" />
    </div>
  );
}
