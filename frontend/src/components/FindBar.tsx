import { useEffect, useRef, useState } from "react";
import { CaseSensitive, ChevronDown, ChevronUp, Layers, WholeWord, X } from "lucide-react";
import type { FindResult } from "@type/browser";
import { clearFind, findInPage, openPopup } from "../lib/browser-client";
import { cn } from "../lib/cn";
import { IconButton } from "./IconButton";

type FindBarProps = {
  tabId: number;
  /** Bumped each time Ctrl+F is pressed so the input refocuses. */
  focusToken: number;
  onClose: () => void;
};

export function FindBar({ tabId, focusToken, onClose }: FindBarProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const [caseSensitive, setCaseSensitive] = useState(false);
  const [wholeWord, setWholeWord] = useState(false);
  const [result, setResult] = useState<FindResult | null>(null);
  const request = useRef(0);

  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, [focusToken]);

  useEffect(() => {
    setResult(null);
  }, [tabId]);

  const search = async (backwards: boolean, step: boolean, value = query, options = { caseSensitive, wholeWord }) => {
    const ticket = ++request.current;
    if (!value) {
      setResult(null);
      void clearFind(tabId);
      return;
    }
    try {
      const found = await findInPage(tabId, value, backwards, { ...options, step });
      if (ticket === request.current) {
        setResult(found);
      }
    } catch {
      if (ticket === request.current) {
        setResult({ count: 0, index: -1 });
      }
    }
  };

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
        id="find-bar-all-tabs"
        label="Find in all tabs"
        disabled={!query}
        onClick={(event) => {
          const params = new URLSearchParams({ query, case: caseSensitive ? "1" : "0", word: wholeWord ? "1" : "0" });
          void openPopup("find-all", event.currentTarget, { params: params.toString() });
        }}
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
    </div>
  );
}
