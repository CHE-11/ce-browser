import { useEffect, useState } from "react";
import { AppWindow, Calculator, Code, Globe, History, Search, Star, X } from "lucide-react";
import { FavIcon } from "@/components/FavIcon";
import { PopupFrame, PopupMenuSeparator } from "@/components/PopupFrame";
import { closePopup, onOmniboxState, openDevtools, openUiDevtools, sendOmniboxReady, sendOmniboxPick, sendOmniboxRemove } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { OMNIBOX_ROW_HEIGHT } from "@/lib/omnibox";
import { useTabs } from "@/lib/use-browser";
import { OmniboxSuggestion, OmniboxState } from "@type/browser";


const KIND_LABEL: Record<OmniboxSuggestion["kind"], string> = {
  search: "Search",
  url: "Go to",
  bookmark: "Bookmark",
  tab: "Switch to tab",
  history: "History",
  answer: "Answer",
};

/** Shown under the address bar, as wide as the address field. The input keeps focus in the UI
 * view; this view only mirrors its state. */
export function AddressBarEllipsisMenu() {
  const [state, setState] = useState<OmniboxState>({ suggestions: [], selected: 0 });
  const snapshot = useTabs();
  const active = snapshot.tabs.find((tab) => tab.id === snapshot.activeId);

  const openTools = (action: () => Promise<unknown>) => {
    void action().finally(() => void closePopup());
  };

  useEffect(() => {
    const unlisten = onOmniboxState((next) => {
      setState(next);
    });
    void unlisten.then(() => sendOmniboxReady());
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);

  return (
    <PopupFrame id="address-bar-ellipsis-frame" width="fill" maxHeight={2000} ready={state.suggestions.length > 0}>
      <div id="address-bar-ellipsis-menu" role="listbox" aria-label="Suggestions" className="py-1.5">
        {state.suggestions.map((suggestion, index) => {
          const domId = `address-bar-ellipsis-menu-${index}`;
          const Icon = suggestion.kind === "answer" ? Calculator : suggestion.kind === "search" ? Search : suggestion.kind === "tab" ? AppWindow : suggestion.kind === "bookmark" ? Star : Globe;
          return (
            <div
              key={`${suggestion.kind}-${suggestion.url}-${index}`}
              id={domId}
              role="option"
              aria-selected={index === state.selected}
              style={{ height: OMNIBOX_ROW_HEIGHT }}
              className={cn(
                "group mx-1.5 flex cursor-default items-center gap-3 rounded-lg px-2.5 text-sm",
                index === state.selected ? "bg-zinc-800" : "hover:bg-zinc-800/60",
              )}
              onMouseDown={(event) => {
                event.preventDefault();
                void sendOmniboxPick(suggestion);
              }}
            >
              {suggestion.kind === "search" || suggestion.kind === "url" || suggestion.kind === "answer" ? (
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
              <span
                id={`${domId}-kind`}
                className={cn(
                  "flex shrink-0 items-center gap-1 text-xs",
                  suggestion.kind === "tab" ? "rounded-full bg-sky-500/15 px-2 py-0.5 text-sky-300" : "text-zinc-500",
                  suggestion.kind === "history" && suggestion.historyId !== undefined && "group-hover:hidden",
                )}
              >
                {suggestion.kind === "tab" ? <AppWindow id={`${domId}-kind-icon`} className="size-3" aria-hidden /> : null}
                {suggestion.kind === "history" ? <History id={`${domId}-kind-icon`} className="size-3" aria-hidden /> : null}
                {KIND_LABEL[suggestion.kind]}
                {suggestion.kind === "tab" && suggestion.workspace ? ` in ${suggestion.workspace}` : ""}
              </span>
              {suggestion.kind === "history" && suggestion.historyId !== undefined ? (
                <button
                  id={`${domId}-remove`}
                  type="button"
                  tabIndex={-1}
                  title="Remove from history (Shift+Delete)"
                  aria-label={`Remove ${suggestion.title} from history`}
                  className="hidden size-6 shrink-0 items-center justify-center rounded text-zinc-400 hover:bg-zinc-700 hover:text-zinc-100 group-hover:inline-flex"
                  onMouseDown={(event) => {
                    event.preventDefault();
                    event.stopPropagation();
                    void sendOmniboxRemove(index);
                  }}
                >
                  <X id={`${domId}-remove-icon`} className="size-3.5" aria-hidden />
                </button>
              ) : null}
            </div>
          );
        })}
      </div>
      <PopupMenuSeparator id="address-bar-ellipsis-devtools-separator" />
      <div id="address-bar-ellipsis-devtools" className="flex flex-col gap-0.5 px-1.5 pb-1.5">
        <button
          id="address-bar-ellipsis-tauri-devtools"
          type="button"
          className="flex h-9 w-full items-center gap-3 rounded-lg px-2.5 text-left text-sm text-zinc-100 hover:bg-zinc-800"
          onMouseDown={(event) => {
            event.preventDefault();
            openTools(openUiDevtools);
          }}
        >
          <AppWindow id="address-bar-ellipsis-tauri-devtools-icon" className="size-4 shrink-0 text-zinc-400" aria-hidden />
          <span id="address-bar-ellipsis-tauri-devtools-label">Open Tauri Developer Tools</span>
        </button>
        <button
          id="address-bar-ellipsis-web-devtools"
          type="button"
          disabled={!active}
          className="flex h-9 w-full items-center gap-3 rounded-lg px-2.5 text-left text-sm text-zinc-100 hover:bg-zinc-800 disabled:opacity-40 disabled:hover:bg-transparent"
          onMouseDown={(event) => {
            event.preventDefault();
            if (active) {
              openTools(() => openDevtools(active.id));
            }
          }}
        >
          <Code id="address-bar-ellipsis-web-devtools-icon" className="size-4 shrink-0 text-zinc-400" aria-hidden />
          <span id="address-bar-ellipsis-web-devtools-label">Open Web Developer Tools</span>
        </button>
      </div>
    </PopupFrame>
  );
}
