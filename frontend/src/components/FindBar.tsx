import { useEffect, useRef, useState } from "react";
import { ChevronDown, ChevronUp, X } from "lucide-react";
import { clearFind, findInPage } from "../lib/browser-client";
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
  const [found, setFound] = useState<boolean | null>(null);

  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, [focusToken]);

  useEffect(() => {
    setFound(null);
  }, [tabId]);

  const search = async (backwards: boolean, value = query) => {
    if (!value) {
      setFound(null);
      void clearFind(tabId);
      return;
    }
    try {
      setFound(await findInPage(tabId, value, backwards));
    } catch {
      setFound(false);
    }
  };

  const close = () => {
    void clearFind(tabId);
    onClose();
  };

  return (
    <div id="find-bar" className="flex h-9 shrink-0 items-center justify-end gap-1 border-b border-zinc-800 bg-zinc-900 px-3">
      <div
        id="find-bar-field"
        className={cn(
          "flex h-7 w-72 items-center gap-2 rounded-md bg-zinc-800 px-2 ring-1 ring-zinc-700",
          found === false && "ring-red-500/70",
        )}
      >
        <input
          id="find-bar-input"
          ref={inputRef}
          value={query}
          placeholder="Find in page"
          spellCheck={false}
          className="min-w-0 flex-1 bg-transparent text-sm text-zinc-100 outline-none placeholder:text-zinc-500"
          onChange={(event) => {
            setQuery(event.target.value);
            void search(false, event.target.value);
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              void search(event.shiftKey);
            } else if (event.key === "Escape") {
              event.preventDefault();
              close();
            }
          }}
        />
        {found === false ? (
          <span id="find-bar-status" className="shrink-0 text-xs text-red-300">
            No matches
          </span>
        ) : null}
      </div>
      <IconButton id="find-bar-previous" label="Previous match (Shift+Enter)" disabled={!query} onClick={() => void search(true)}>
        <ChevronUp id="find-bar-previous-icon" aria-hidden />
      </IconButton>
      <IconButton id="find-bar-next" label="Next match (Enter)" disabled={!query} onClick={() => void search(false)}>
        <ChevronDown id="find-bar-next-icon" aria-hidden />
      </IconButton>
      <IconButton id="find-bar-close" label="Close (Escape)" onClick={close}>
        <X id="find-bar-close-icon" aria-hidden />
      </IconButton>
    </div>
  );
}
