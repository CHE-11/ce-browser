import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { CaseSensitive, ChevronDown, ChevronUp, Layers, PanelRight, WholeWord, X } from "lucide-react";
import type { FindResult } from "@type/browser";
import { clearFind, findInPage, openPopup } from "../lib/browser-client";
import { cn } from "../lib/cn";
import { FindResultsPanel } from "./FindResultsPanel";
import { IconButton } from "./IconButton";

type FindBarProps = {
  tabId: number;
  /** Bumped when find opens, so the input focuses. Ctrl+F closes it again. */
  focusToken: number;
  /** Bumped by Ctrl+Shift+F. Opens Find in all tabs when the field has a query. */
  openAllToken: number;
  /** The results list beside the page. */
  panelOpen: boolean;
  onPanelChange: (open: boolean) => void;
  /** Fullscreen covers the page edge, so the panel stays closed until it ends. */
  fullscreen: boolean;
  onClose: () => void;
};

type SearchOptions = {
  caseSensitive: boolean;
  wholeWord: boolean;
  select?: number;
};

export function FindBar({ tabId, focusToken, openAllToken, panelOpen, onPanelChange, fullscreen, onClose }: FindBarProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const [caseSensitive, setCaseSensitive] = useState(false);
  const [wholeWord, setWholeWord] = useState(false);
  const [result, setResult] = useState<FindResult | null>(null);
  const request = useRef(0);
  const optionsRef = useRef({ query, caseSensitive, wholeWord });
  const seenAll = useRef(openAllToken);
  const searchRef = useRef<(backwards: boolean, step: boolean, value?: string, options?: SearchOptions) => void>(() => {});
  optionsRef.current = { query, caseSensitive, wholeWord };

  const openAllTabs = (anchor: HTMLElement) => {
    const current = optionsRef.current;
    if (!current.query) {
      return;
    }
    const params = new URLSearchParams({
      query: current.query,
      case: current.caseSensitive ? "1" : "0",
      word: current.wholeWord ? "1" : "0",
    });
    void openPopup("find-all", anchor, { params: params.toString() });
  };

  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, [focusToken]);

  useEffect(() => {
    if (seenAll.current === openAllToken) {
      return;
    }
    seenAll.current = openAllToken;
    const button = document.getElementById("find-bar-all-tabs");
    if (button instanceof HTMLElement) {
      openAllTabs(button);
    }
  }, [openAllToken]);

  useEffect(() => {
    setResult(null);
    if (optionsRef.current.query) {
      void searchRef.current(false, false);
    }
  }, [tabId]);

  const search = async (backwards: boolean, step: boolean, value = query, options: SearchOptions = { caseSensitive, wholeWord }) => {
    const ticket = ++request.current;
    if (!value) {
      setResult(null);
      void clearFind(tabId);
      return;
    }
    try {
      const found = await findInPage(tabId, value, backwards, {
        caseSensitive: options.caseSensitive,
        wholeWord: options.wholeWord,
        step: options.select == null ? step : false,
        list: panelOpen,
        ...(options.select == null ? {} : { select: options.select }),
      });
      if (ticket === request.current) {
        setResult(found);
      }
    } catch {
      if (ticket === request.current) {
        setResult({ count: 0, index: -1 });
      }
    }
  };
  searchRef.current = search;

  useEffect(() => {
    if (!panelOpen || !optionsRef.current.query) {
      return;
    }
    void searchRef.current(false, false);
  }, [panelOpen]);

  const toggle = (option: "caseSensitive" | "wholeWord") => {
    const options = { caseSensitive, wholeWord, [option]: !(option === "caseSensitive" ? caseSensitive : wholeWord) };
    setCaseSensitive(options.caseSensitive);
    setWholeWord(options.wholeWord);
    void search(false, false, query, options);
    inputRef.current?.focus();
  };

  const close = () => {
    void clearFind(tabId);
    onClose();
  };

  const showPanel = panelOpen && !fullscreen;
  const slot = showPanel ? document.getElementById("find-results-slot") : null;
  const none = !!query && result?.count === 0;

  return (
    <div id="find-bar" role="search" className="flex h-9 shrink-0 items-center justify-end gap-1 border-b border-zinc-800 bg-zinc-900 px-3">
      <div
        id="find-bar-field"
        className={cn(
          "flex h-7 w-80 items-center gap-2 rounded-md bg-zinc-800 px-2 ring-1 ring-zinc-700 focus-within:ring-sky-500",
          none && "ring-red-500/70",
        )}
      >
        <input
          id="find-bar-input"
          ref={inputRef}
          value={query}
          placeholder="Find in page"
          aria-label="Find in page"
          spellCheck={false}
          className="min-w-0 flex-1 bg-transparent text-sm text-zinc-100 outline-none placeholder:text-zinc-500"
          onChange={(event) => {
            setQuery(event.target.value);
            void search(false, false, event.target.value);
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              void search(event.shiftKey, true);
            } else if (event.key === "Escape") {
              event.preventDefault();
              close();
            }
          }}
        />
        {query && result ? (
          <span id="find-bar-status" aria-live="polite" className={cn("shrink-0 text-xs tabular-nums", none ? "text-red-300" : "text-zinc-400")}>
            {none ? "No matches" : `${result.index + 1} of ${result.count}${result.count >= 2000 ? "+" : ""}`}
          </span>
        ) : null}
      </div>
      <IconButton
        id="find-bar-case"
        label="Match case"
        aria-pressed={caseSensitive}
        active={caseSensitive}
        onClick={() => toggle("caseSensitive")}
      >
        <CaseSensitive id="find-bar-case-icon" aria-hidden />
      </IconButton>
      <IconButton
        id="find-bar-word"
        label="Whole words only"
        aria-pressed={wholeWord}
        active={wholeWord}
        onClick={() => toggle("wholeWord")}
      >
        <WholeWord id="find-bar-word-icon" aria-hidden />
      </IconButton>
      <IconButton
        id="find-bar-results"
        label="Results panel"
        aria-pressed={panelOpen}
        active={panelOpen}
        onClick={() => onPanelChange(!panelOpen)}
      >
        <PanelRight id="find-bar-results-icon" aria-hidden />
      </IconButton>
      <IconButton
        id="find-bar-all-tabs"
        label="Find in all tabs (Ctrl+Shift+F)"
        disabled={!query}
        onClick={(event) => openAllTabs(event.currentTarget)}
      >
        <Layers id="find-bar-all-tabs-icon" aria-hidden />
      </IconButton>
      <IconButton id="find-bar-previous" label="Previous match (Shift+Enter)" disabled={!query || none} onClick={() => void search(true, true)}>
        <ChevronUp id="find-bar-previous-icon" aria-hidden />
      </IconButton>
      <IconButton id="find-bar-next" label="Next match (Enter)" disabled={!query || none} onClick={() => void search(false, true)}>
        <ChevronDown id="find-bar-next-icon" aria-hidden />
      </IconButton>
      <IconButton id="find-bar-close" label="Close (Escape)" onClick={close}>
        <X id="find-bar-close-icon" aria-hidden />
      </IconButton>
      {slot
        ? createPortal(
            <FindResultsPanel
              query={query}
              result={result}
              onClose={() => onPanelChange(false)}
              onSelect={(index) => {
                void search(false, false, query, { caseSensitive, wholeWord, select: index });
                inputRef.current?.focus();
              }}
            />,
            slot,
          )
        : null}
    </div>
  );
}
