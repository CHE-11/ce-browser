import { useEffect, useState } from "react";
import { AppWindow, Globe, History, Search, Star } from "lucide-react";
import { FavIcon } from "@/components/FavIcon";
import { onOmniboxState, sendOmniboxReady, sendOmniboxPick } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { OMNIBOX_ROW_HEIGHT } from "@/lib/omnibox";
import { usePopupPainted } from "@/lib/popup-ready";
import { OmniboxSuggestion, OmniboxState } from "@type/browser";


const KIND_LABEL: Record<OmniboxSuggestion["kind"], string> = {
  search: "Search",
  url: "Go to",
  bookmark: "Bookmark",
  tab: "Switch to tab",
  history: "History",
};

/** Shown under the address bar. The input keeps focus in the UI view; this view only mirrors its state. */
export function AddressBarEllipsisMenu() {
  const [state, setState] = useState<OmniboxState>({ suggestions: [], selected: 0 });
  const [received, setReceived] = useState(false);

  usePopupPainted(received);

  useEffect(() => {
    const unlisten = onOmniboxState((next) => {
      setState(next);
      setReceived(true);
    });
    void unlisten.then(() => sendOmniboxReady());
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);

  return (
    <div
      id="address-bar-ellipsis-menu"
      role="listbox"
      aria-label="Suggestions"
      className="h-full overflow-hidden rounded-xl bg-zinc-900 py-1.5 text-zinc-100 ring-1 ring-inset ring-zinc-700"
      onContextMenu={(event) => event.preventDefault()}
    >
      {state.suggestions.map((suggestion, index) => {
        const domId = `address-bar-ellipsis-menu-${index}`;
        const Icon = suggestion.kind === "search" ? Search : suggestion.kind === "tab" ? AppWindow : suggestion.kind === "bookmark" ? Star : Globe;
        return (
          <div
            key={`${suggestion.kind}-${suggestion.url}-${index}`}
            id={domId}
            role="option"
            aria-selected={index === state.selected}
            style={{ height: OMNIBOX_ROW_HEIGHT }}
            className={cn(
              "mx-1.5 flex cursor-default items-center gap-3 rounded-lg px-2.5 text-sm",
              index === state.selected ? "bg-zinc-800" : "hover:bg-zinc-800/60",
            )}
            onMouseDown={(event) => {
              event.preventDefault();
              void sendOmniboxPick(suggestion);
            }}
          >
            {suggestion.kind === "search" || suggestion.kind === "url" ? (
              <Icon id={`${domId}-icon`} className="size-4 shrink-0 text-zinc-400" aria-hidden />
            ) : (
              <FavIcon id={`${domId}-icon`} url={suggestion.url} />
            )}
            <span id={`${domId}-title`} className="min-w-0 truncate">
              {suggestion.title}
            </span>
            {suggestion.kind === "bookmark" || suggestion.kind === "tab" || suggestion.kind === "history" ? (
              <span id={`${domId}-url`} className="min-w-0 flex-1 truncate text-xs text-sky-400">
                {suggestion.url}
              </span>
            ) : (
              <span id={`${domId}-spacer`} className="flex-1" />
            )}
            <span id={`${domId}-kind`} className="flex shrink-0 items-center gap-1 text-xs text-zinc-500">
              {suggestion.kind === "tab" ? <AppWindow id={`${domId}-kind-icon`} className="size-3" aria-hidden /> : null}
              {suggestion.kind === "history" ? <History id={`${domId}-kind-icon`} className="size-3" aria-hidden /> : null}
              {KIND_LABEL[suggestion.kind]}
            </span>
          </div>
        );
      })}
    </div>
  );
}
