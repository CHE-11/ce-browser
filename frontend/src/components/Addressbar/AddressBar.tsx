import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import {
  ArrowLeft,
  ArrowRight,
  Columns2,
  Download,
  EllipsisVertical,
  History,
  House,
  Lock,
  Puzzle,
  RotateCw,
  Search,
  Star,
  TriangleAlert,
  X,
} from "lucide-react";
import type {
  BookmarkTree,
  BrowserSettings,
  DownloadItem,
  ExtensionInfo,
  HistoryItem,
  OmniboxState,
  OmniboxSuggestion,
  SplitView,
  TabInfo,
  TabsSnapshot,
} from "../../../../types/browser";
import {
  activateTab,
  closePopup,
  createTab,
  exitSplit,
  goBack,
  goForward,
  navigate,
  onBrowserCommand,
  onDownloadStarted,
  onOmniboxPick,
  onOmniboxReady,
  onPopupChanged,
  openPopup,
  openPopupBelow,
  reload,
  resizePopup,
  sendOmniboxState,
  splitWith,
  stop,
} from "../../lib/browser-client";
import { cn } from "../../lib/cn";
import { openToolbarPinMenu, toolbarPinned } from "../../lib/toolbar-pin";
import { buildSuggestions, omniboxHeight } from "../../lib/omnibox";
import { flattenBookmarks } from "../../lib/use-browser";
import { BuiltinExtensions } from "../../extensions";
import { ExtensionIcon } from "../ExtensionIcon";
import { IconButton } from "../IconButton";
import { WindowControls } from "../WindowControls";

type AddressBarProps = {
  snapshot: TabsSnapshot & { split?: SplitView | null };
  bookmarks: BookmarkTree | null;
  downloads: DownloadItem[];
  history: HistoryItem[];
  extensions: ExtensionInfo[];
  settings: BrowserSettings | null;
  /** With vertical tabs there is no tab strip, so the address bar holds the window buttons. */
  windowControls?: boolean;
};

const EMPTY_OMNIBOX: OmniboxState = { suggestions: [], selected: 0 };

function addressText(tab: TabInfo | undefined) {
  if (!tab || tab.url === "ce://newtab") {
    return "";
  }
  return tab.url;
}

export function AddressBar({ snapshot, bookmarks, downloads, history, extensions, settings, windowControls = false }: AddressBarProps) {
  const activeTab = snapshot.tabs.find((tab) => tab.id === snapshot.activeId);
  const activeId = activeTab?.id ?? null;
  const split = snapshot.split ?? null;
  const inSplit = !!split && activeId !== null && (split.left === activeId || split.right === activeId);
  const activeIndex = snapshot.tabs.findIndex((tab) => tab.id === activeId);
  const splitPartner =
    snapshot.tabs.slice(activeIndex + 1).find((tab) => !tab.pinned) ??
    snapshot.tabs
      .slice(0, Math.max(0, activeIndex))
      .reverse()
      .find((tab) => !tab.pinned);
  const canSplit = !!activeTab && !activeTab.pinned && !!splitPartner;

  const toggleSplit = () => {
    if (inSplit) {
      void exitSplit();
    } else if (splitPartner) {
      void splitWith(splitPartner.id);
    }
  };

  const inputRef = useRef<HTMLInputElement>(null);
  const activeRef = useRef(activeTab);
  activeRef.current = activeTab;
  const fieldRef = useRef<HTMLDivElement>(null);
  const starRef = useRef<HTMLButtonElement>(null);
  const downloadsRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLButtonElement>(null);

  const [text, setText] = useState(() => addressText(activeTab));
  const [editing, setEditing] = useState(false);
  const [omnibox, setOmnibox] = useState<OmniboxState>(EMPTY_OMNIBOX);
  const [downloadFlash, setDownloadFlash] = useState(false);
  const omniboxRef = useRef(omnibox);
  const popupOpenRef = useRef(false);
  const blurTimer = useRef<number | null>(null);

  const bookmarkLinks = useMemo(() => flattenBookmarks(bookmarks), [bookmarks]);
  const bookmarked = !!activeTab && bookmarkLinks.some((link) => link.url === activeTab.url);

  useEffect(() => {
    if (!editing) {
      setText(addressText(activeTab));
    }
  }, [activeTab?.url, activeId, editing]);

  useLayoutEffect(() => {
    if (activeRef.current?.url === "ce://newtab") {
      setText("");
      setEditing(true);
      inputRef.current?.focus();
      return;
    }
    setEditing(false);
  }, [activeId]);

  const hideOmnibox = useCallback(() => {
    omniboxRef.current = EMPTY_OMNIBOX;
    setOmnibox(EMPTY_OMNIBOX);
    if (popupOpenRef.current) {
      popupOpenRef.current = false;
      void closePopup();
    }
  }, []);

  const showOmnibox = useCallback((state: OmniboxState) => {
    omniboxRef.current = state;
    setOmnibox(state);
    const field = fieldRef.current;
    if (!field || state.suggestions.length === 0) {
      return;
    }
    const height = omniboxHeight(state.suggestions.length);
    if (popupOpenRef.current) {
      void resizePopup(height);
      void sendOmniboxState(state);
      return;
    }
    const rect = field.getBoundingClientRect();
    popupOpenRef.current = true;
    void openPopup({
      kind: "omnibox",
      focus: false,
      x: rect.left,
      y: rect.bottom + 2,
      width: rect.width,
      height,
    });
  }, []);

  const finishEditing = useCallback(() => {
    setEditing(false);
    hideOmnibox();
    inputRef.current?.blur();
  }, [hideOmnibox]);

  const pick = useCallback(
    (suggestion: OmniboxSuggestion | null, inNewTab = false) => {
      const target = suggestion ?? { kind: "url" as const, title: text, url: text.trim() };
      if (!target.url) {
        return;
      }
      finishEditing();
      if (target.kind === "tab" && target.tabId !== undefined) {
        void activateTab(target.tabId);
      } else if (inNewTab || activeId === null) {
        void createTab(target.url);
      } else {
        setText(target.url);
        void navigate(activeId, target.url);
      }
    },
    [activeId, finishEditing, text],
  );

  const pickRef = useRef(pick);
  pickRef.current = pick;

  useEffect(() => {
    const unlisteners = [
      onOmniboxReady(() => {
        void sendOmniboxState(omniboxRef.current);
      }),
      onOmniboxPick((suggestion) => pickRef.current(suggestion)),
      onPopupChanged((change) => {
        if (change.kind !== "omnibox") {
          popupOpenRef.current = false;
        }
      }),
      onBrowserCommand((command) => {
        if (command === "focus-address") {
          const input = inputRef.current;
          if (!input) {
            return;
          }
          const typing = document.activeElement === input && input.value !== addressText(activeRef.current);
          input.focus();
          if (!typing) {
            input.select();
          }
        } else if (command === "bookmark" && starRef.current) {
          openBookmarkEditor(starRef.current);
        } else if (command === "downloads") {
          const anchor = downloadsRef.current ?? menuRef.current;
          if (anchor) {
            void openPopupBelow(anchor, { kind: "downloads", width: 380, height: 440 });
          }
        }
      }),
      onDownloadStarted(() => {
        setDownloadFlash(true);
        window.setTimeout(() => setDownloadFlash(false), 1600);
      }),
    ];
    return () => {
      for (const unlisten of unlisteners) {
        void unlisten.then((stopListening) => stopListening());
      }
    };
  }, []);

  function openBookmarkEditor(anchor: HTMLElement) {
    const tab = activeRef.current;
    if (!tab || tab.url === "ce://newtab") {
      return;
    }
    const params = new URLSearchParams({ url: tab.url, title: tab.title || tab.url });
    void openPopupBelow(anchor, { kind: "bookmark-edit", params: params.toString(), width: 340, height: 236 });
  }

  const onChange = (value: string) => {
    setText(value);
    setEditing(true);
    const suggestions = buildSuggestions(value, bookmarkLinks, history, snapshot.tabs, activeId);
    if (suggestions.length === 0) {
      hideOmnibox();
    } else {
      showOmnibox({ suggestions, selected: 0 });
    }
  };

  const moveSelection = (delta: number) => {
    const { suggestions, selected } = omniboxRef.current;
    if (suggestions.length === 0) {
      return;
    }
    const next = (selected + delta + suggestions.length) % suggestions.length;
    const state = { suggestions, selected: next };
    omniboxRef.current = state;
    setOmnibox(state);
    void sendOmniboxState(state);
    const suggestion = suggestions[next];
    if (suggestion.kind !== "search") {
      setText(suggestion.url);
    }
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      moveSelection(1);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      moveSelection(-1);
    } else if (event.key === "Enter") {
      event.preventDefault();
      const { suggestions, selected } = omniboxRef.current;
      pick(suggestions[selected] ?? null, event.altKey);
    } else if (event.key === "Escape") {
      event.preventDefault();
      if (omniboxRef.current.suggestions.length > 0) {
        hideOmnibox();
      } else {
        setText(addressText(activeTab));
        finishEditing();
      }
    }
  };

  const activeDownloads = downloads.filter((item) => item.state === "inProgress");
  const progress = useMemo(() => {
    const sized = activeDownloads.filter((item) => item.totalBytes);
    if (sized.length === 0) {
      return null;
    }
    const total = sized.reduce((sum, item) => sum + (item.totalBytes ?? 0), 0);
    const received = sized.reduce((sum, item) => sum + item.receivedBytes, 0);
    return total > 0 ? Math.min(1, received / total) : null;
  }, [activeDownloads]);

  const toolbarExtensions = extensions.filter(
    (extension) => extension.enabled && toolbarPinned(settings, extension.id),
  );
  const secure = activeTab?.url.startsWith("https://");
  const insecure = activeTab?.url.startsWith("http://");

  return (
    <div
      id="address-bar"
      className={cn("flex h-10 shrink-0 items-center gap-1 bg-zinc-900 px-2", windowControls && "pr-0")}
      data-tauri-drag-region={windowControls ? true : undefined}
    >
      <IconButton
        id="address-back"
        label="Back (Alt+Left)"
        disabled={!activeTab?.canGoBack}
        onClick={() => activeId !== null && void goBack(activeId)}
      >
        <ArrowLeft id="address-back-icon" aria-hidden />
      </IconButton>
      <IconButton
        id="address-forward"
        label="Forward (Alt+Right)"
        disabled={!activeTab?.canGoForward}
        onClick={() => activeId !== null && void goForward(activeId)}
      >
        <ArrowRight id="address-forward-icon" aria-hidden />
      </IconButton>
      {activeTab?.loading ? (
        <IconButton id="address-stop" label="Stop loading" onClick={() => activeId !== null && void stop(activeId)}>
          <X id="address-stop-icon" aria-hidden />
        </IconButton>
      ) : (
        <IconButton
          id="address-reload"
          label="Reload (Ctrl+R)"
          disabled={activeId === null}
          onClick={() => activeId !== null && void reload(activeId)}
        >
          <RotateCw id="address-reload-icon" aria-hidden />
        </IconButton>
      )}
      <IconButton
        id="address-home"
        label="Home"
        disabled={activeId === null}
        onClick={() => activeId !== null && void navigate(activeId, settings?.homePage || "ce://bookmarks")}
      >
        <House id="address-home-icon" aria-hidden />
      </IconButton>

      <div
        id="address-field"
        ref={fieldRef}
        className={cn(
          "mx-1 flex h-8 min-w-0 flex-1 items-center gap-2 rounded-full bg-zinc-800 px-3 ring-sky-500/70 transition-shadow",
          editing && "ring-2",
        )}
      >
        {editing || !activeTab || activeTab.url === "ce://newtab" ? (
          <Search id="address-field-search" className="size-3.5 shrink-0 text-zinc-400" aria-hidden />
        ) : secure ? (
          <Lock id="address-field-secure" className="size-3.5 shrink-0 text-zinc-400" aria-label="Secure connection" />
        ) : insecure ? (
          <TriangleAlert id="address-field-insecure" className="size-3.5 shrink-0 text-amber-400" aria-label="Not secure" />
        ) : (
          <Search id="address-field-search" className="size-3.5 shrink-0 text-zinc-400" aria-hidden />
        )}
        <input
          id="address-input"
          ref={inputRef}
          value={text}
          spellCheck={false}
          autoComplete="off"
          placeholder="Search or enter address"
          className="min-w-0 flex-1 bg-transparent text-sm text-zinc-100 outline-none placeholder:text-zinc-500"
          onChange={(event) => onChange(event.target.value)}
          onKeyDown={onKeyDown}
          onFocus={(event) => {
            if (blurTimer.current !== null) {
              window.clearTimeout(blurTimer.current);
              blurTimer.current = null;
            }
            setEditing(true);
            event.currentTarget.select();
          }}
          onBlur={() => {
            blurTimer.current = window.setTimeout(() => {
              blurTimer.current = null;
              setEditing(false);
              hideOmnibox();
            }, 250);
          }}
        />
        {activeTab && activeTab.zoom !== 1 ? (
          <span id="address-zoom" className="shrink-0 rounded bg-zinc-700 px-1.5 text-[11px] text-zinc-300">
            {Math.round(activeTab.zoom * 100)}%
          </span>
        ) : null}
        <button
          id="address-star"
          ref={starRef}
          type="button"
          title={bookmarked ? "Edit bookmark (Ctrl+D)" : "Bookmark this tab (Ctrl+D)"}
          aria-label="Bookmark this tab"
          disabled={!activeTab || activeTab.url === "ce://newtab"}
          className="inline-flex size-6 shrink-0 items-center justify-center rounded-full text-zinc-400 hover:bg-zinc-700 hover:text-zinc-100 disabled:opacity-30"
          onClick={(event) => openBookmarkEditor(event.currentTarget)}
        >
          <Star
            id="address-star-icon"
            className={cn("size-3.5", bookmarked && "fill-sky-400 text-sky-400")}
            aria-hidden
          />
        </button>
      </div>

      <div id="address-extensions" className="flex items-center">
        {toolbarExtensions.map((extension) => (
          <span
            key={extension.id}
            id={`address-extension-${extension.id}-pin`}
            className="inline-flex"
            onContextMenu={(event) => openToolbarPinMenu(event, extension.id)}
          >
            <IconButton
              id={`address-extension-${extension.id}`}
              label={extension.name}
              disabled={!extension.popup}
              onClick={(event) =>
                void openPopupBelow(event.currentTarget, {
                  kind: "extension",
                  params: `id=${extension.id}`,
                  width: 380,
                  height: 600,
                })
              }
            >
              <ExtensionIcon
                id={`address-extension-${extension.id}-icon`}
                extensionId={extension.id}
                name={extension.name}
                src={extension.icon}
                className="size-4"
              />
            </IconButton>
          </span>
        ))}
        <IconButton
          id="address-extensions-page"
          label="Extensions"
          onClick={() => void createTab("ce://extensions")}
        >
          <Puzzle id="address-extensions-page-icon" aria-hidden />
        </IconButton>
      </div>
      <div id="address-extensions-end" className="mx-0.5 h-4 w-px shrink-0 bg-zinc-600" />

      <IconButton
        id="address-downloads"
        ref={downloadsRef}
        label="Downloads (Ctrl+J)"
        className={cn("relative", downloadFlash && "animate-pulse bg-sky-500/25 text-sky-300")}
        onClick={(event) =>
          void openPopupBelow(event.currentTarget, { kind: "downloads", width: 380, height: 440 })
        }
      >
        <Download id="address-downloads-icon" aria-hidden />
        {activeDownloads.length > 0 ? (
          <svg id="address-downloads-ring" className="pointer-events-none absolute inset-0.5 -rotate-90" viewBox="0 0 36 36">
            <circle id="address-downloads-ring-track" cx="18" cy="18" r="16" fill="none" stroke="rgb(63 63 70)" strokeWidth="2.5" />
            <circle
              id="address-downloads-ring-value"
              cx="18"
              cy="18"
              r="16"
              fill="none"
              stroke="rgb(56 189 248)"
              strokeWidth="2.5"
              strokeLinecap="round"
              strokeDasharray={`${(progress ?? 0.25) * 100.5} 100.5`}
              className={progress === null ? "origin-center animate-spin" : undefined}
            />
          </svg>
        ) : null}
      </IconButton>
      <IconButton id="address-history" label="History" onClick={() => void createTab("ce://history")}>
        <History id="address-history-icon" aria-hidden />
      </IconButton>
      <IconButton
        id="address-split"
        label={inSplit ? "Exit split view" : "Split view with the next tab"}
        active={inSplit}
        disabled={!inSplit && !canSplit}
        onClick={toggleSplit}
      >
        <Columns2 id="address-split-icon" aria-hidden />
      </IconButton>

      <div id="address-builtin-start" className="mx-0.5 h-4 w-px shrink-0 bg-zinc-600" />
      <BuiltinExtensions activeTab={activeTab} settings={settings} />
      <div id="address-builtin-end" className="mx-0.5 h-4 w-px shrink-0 bg-zinc-600" />

      <IconButton
        id="address-menu"
        ref={menuRef}
        label="Menu"
        onClick={(event) => void openPopupBelow(event.currentTarget, { kind: "app-menu", width: 280, height: 460 })}
      >
        <EllipsisVertical id="address-menu-icon" aria-hidden />
      </IconButton>
      <span id="address-omnibox-count" className="sr-only">
        {omnibox.suggestions.length} suggestions
      </span>
      {windowControls ? (
        <>
          <div id="address-drag-space" className="h-full w-6 shrink-0" data-tauri-drag-region />
          <WindowControls />
        </>
      ) : null}
    </div>
  );
}
