import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import {
  Activity,
  ArrowLeft,
  ArrowRight,
  Columns2,
  Download,
  EllipsisVertical,
  History,
  House,
  Info,
  Lock,
  PanelRight,
  Pin,
  Puzzle,
  RotateCw,
  Search,
  Settings,
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
  TabInfo,
} from "../../../../../types/browser";
import {
  activateTab,
  closePopup,
  createTab,
  exitSplit,
  installExtensionFromStore,
  goBack,
  goForward,
  navigate,
  noteTyped,
  omniboxExtra,
  removeHistory,
  searchHistory,
  onBrowserCommand,
  onDownloadStarted,
  onOmniboxPick,
  onOmniboxReady,
  onOmniboxRemove,
  onPopupChanged,
  openPopup,
  openPopupBelow,
  reload,
  resizePopup,
  restorePinnedTabs,
  saveSettings,
  showNotice,
  sendOmniboxState,
  splitWith,
  stop,
  updateExtension,
  zoomTab,
} from "../../../lib/browser-client";
import { storeListing } from "../../../lib/extension-store";
import { accentTint } from "../../../lib/accent";
import { cn } from "../../../lib/cn";
import { reorderToolbar, splitToolbar, toolbarPinned } from "../../../lib/toolbar-pin";
import { buildSuggestions, inlineCompletion, looksLikeUrl, omniboxHeight } from "../../../lib/omnibox";
import { flattenBookmarks, type WindowTabsState } from "../../../lib/use-browser";
import { focusRing } from "../../../lib/focus-ring";
import { IconButton } from "../../IconButton";
import { LoadBar } from "../../LoadBar";
import { WindowControls } from "../../WindowControls";
import { ToolbarEntry } from "../VerticalToolbar/VerticalToolbar";

type AddressBarProps = {
  snapshot: WindowTabsState;
  bookmarks: BookmarkTree | null;
  downloads: DownloadItem[];
  history: HistoryItem[];
  extensions: ExtensionInfo[];
  settings: BrowserSettings | null;
  /** With vertical tabs there is no tab strip, so the address bar holds the window buttons. */
  windowControls?: boolean;
};

const EMPTY_OMNIBOX: OmniboxState = { suggestions: [], selected: 0 };

type PinParts = { scheme: string; host: string; port: string; path: string };

/** Host, port, and path used to tell two pins apart. `www` and a trailing slash are ignored. */
function pinParts(raw: string): PinParts | null {
  const trimmed = raw.trim();
  if (!trimmed) {
    return null;
  }
  if (trimmed.slice(0, 5).toLowerCase() === "ce://") {
    const page = trimmed.slice(5).split(/[?#]/)[0].replace(/^\/+|\/+$/g, "").toLowerCase();
    return { scheme: "ce", host: page, port: "", path: "" };
  }
  let parsed: URL;
  try {
    parsed = new URL(trimmed);
  } catch {
    return null;
  }
  const scheme = parsed.protocol.replace(":", "").toLowerCase();
  let host = parsed.hostname.toLowerCase();
  if (host.startsWith("www.")) {
    host = host.slice(4);
  }
  let port = parsed.port;
  if (!port && (scheme === "http" || scheme === "https")) {
    port = scheme === "https" ? "443" : "80";
  }
  const path = parsed.pathname === "/" ? "" : parsed.pathname.replace(/\/+$/, "");
  return { scheme, host, port, path };
}

function schemesMatch(left: string, right: string) {
  return left === right || ((left === "http" || left === "https") && (right === "http" || right === "https"));
}

function portsMatch(left: PinParts, right: PinParts) {
  if (left.port === right.port) {
    return true;
  }
  const usual = (parts: PinParts) =>
    (parts.scheme === "http" && parts.port === "80") || (parts.scheme === "https" && parts.port === "443");
  return left.scheme !== right.scheme && schemesMatch(left.scheme, right.scheme) && usual(left) && usual(right);
}

function pathsAlign(left: string, right: string) {
  if (left === right) {
    return true;
  }
  const shorter = left.length <= right.length ? left : right;
  const longer = left.length <= right.length ? right : left;
  return longer.startsWith(shorter) && (shorter.length === 0 || longer[shorter.length] === "/");
}

/** A redirect, `www`, or a path under the saved pin is still that pin. */
function samePin(pin: string, url: string) {
  const left = pinParts(pin);
  const right = pinParts(url);
  if (!left || !right) {
    return false;
  }
  return schemesMatch(left.scheme, right.scheme) && left.host === right.host && portsMatch(left, right) && pathsAlign(left.path, right.path);
}

function samePinPage(pin: string, url: string) {
  const left = pinParts(pin);
  const right = pinParts(url);
  if (!left || !right) {
    return false;
  }
  return schemesMatch(left.scheme, right.scheme) && left.host === right.host && portsMatch(left, right) && left.path === right.path;
}

/** A pin sent to a login page is still on its site. */
function samePinSite(pin: string, url: string) {
  const left = pinParts(pin);
  const right = pinParts(url);
  if (!left || !right) {
    return false;
  }
  return schemesMatch(left.scheme, right.scheme) && left.host === right.host && portsMatch(left, right);
}

/**
 * Saved pins that have no open tab yet. A tab under the saved address counts as open, and a pinned
 * tab elsewhere on the pin's site is the fallback. Mirrors `paths::match_pins`.
 */
function closedPinUrls(pins: string[], tabs: { url: string; pinned: boolean }[]) {
  const claimed = new Set<number>();
  const bound = pins.map(() => "missing" as "tab" | "duplicate" | "missing");
  const order = pins
    .map((_, index) => index)
    .sort((left, right) => (pinParts(pins[right])?.path.length ?? 0) - (pinParts(pins[left])?.path.length ?? 0));
  for (const index of order) {
    if (!pinParts(pins[index])) {
      bound[index] = "duplicate";
      continue;
    }
    let best: number | null = null;
    let bestPinned = false;
    let bestDistance = Number.POSITIVE_INFINITY;
    tabs.forEach((tab, tabIndex) => {
      if (claimed.has(tabIndex) || !samePin(pins[index], tab.url)) {
        return;
      }
      const distance = Math.abs((pinParts(tab.url)?.path.length ?? 0) - (pinParts(pins[index])?.path.length ?? 0));
      const closer = tab.pinned === bestPinned && distance < bestDistance;
      const pinnedWins = tab.pinned && !bestPinned;
      if (best === null || pinnedWins || closer) {
        best = tabIndex;
        bestPinned = tab.pinned;
        bestDistance = distance;
      }
    });
    if (best !== null) {
      claimed.add(best);
      bound[index] = "tab";
    }
  }
  for (const index of order) {
    if (bound[index] !== "missing") {
      continue;
    }
    const fallback = tabs.findIndex((tab, tabIndex) => tab.pinned && !claimed.has(tabIndex) && samePinSite(pins[index], tab.url));
    if (fallback !== -1) {
      claimed.add(fallback);
      bound[index] = "tab";
    }
  }
  for (const index of order) {
    if (bound[index] !== "missing") {
      continue;
    }
    const duplicate =
      bound.some((kind, other) => kind === "tab" && samePinPage(pins[index], pins[other])) ||
      tabs.some((tab, tabIndex) => claimed.has(tabIndex) && samePinPage(pins[index], tab.url));
    if (duplicate) {
      bound[index] = "duplicate";
    }
  }
  return pins.filter((_, index) => bound[index] === "missing");
}

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
  const savedPins = settings?.pinnedTabs ?? [];
  const closedPins = closedPinUrls(savedPins, snapshot.windowTabs);
  const [restoringPins, setRestoringPins] = useState(false);
  const restorePins = () => {
    if (restoringPins || closedPins.length === 0) {
      return;
    }
    setRestoringPins(true);
    void restorePinnedTabs().finally(() => setRestoringPins(false));
  };
  const pinLabel =
    savedPins.length === 0
      ? "No pinned tabs in settings"
      : closedPins.length === 0
        ? "Pinned tabs are open"
        : "Reopen closed pinned tabs";

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
  const [storeBusy, setStoreBusy] = useState(false);
  const omniboxRef = useRef(omnibox);
  const popupOpenRef = useRef(false);
  const blurTimer = useRef<number | null>(null);
  /** What the user typed. Anything after it in `text` is an inline completion. */
  const typedRef = useRef("");
  const completionRange = useRef<[number, number] | null>(null);
  const [selectionTick, setSelectionTick] = useState(0);

  useLayoutEffect(() => {
    const range = completionRange.current;
    if (range && inputRef.current) {
      inputRef.current.setSelectionRange(range[0], range[1], "backward");
      completionRange.current = null;
    }
  }, [text, selectionTick]);

  const bookmarkLinks = useMemo(() => flattenBookmarks(bookmarks), [bookmarks]);
  const bookmarked = !!activeTab && bookmarkLinks.some((link) => link.url === activeTab.url);
  const listing = activeTab ? storeListing(activeTab.url) : null;
  const listed = listing ? extensions.find((extension) => extension.id === listing.id) : undefined;
  const storeAction = !listing ? null : listed?.updateVersion ? "update" : listed ? null : "install";

  const runStoreAction = () => {
    if (!listing || !storeAction || storeBusy) {
      return;
    }
    setStoreBusy(true);
    const work =
      storeAction === "update" && listed
        ? updateExtension(listed.id)
        : installExtensionFromStore(activeTab?.url ?? listing.id);
    void work
      .then((extension) => {
        const message = storeAction === "update" ? `Updated ${extension.name}` : `Installed ${extension.name}`;
        void showNotice(message).catch(() => undefined);
      })
      .catch((reason: unknown) => {
        void showNotice(String(reason)).catch(() => undefined);
      })
      .finally(() => setStoreBusy(false));
  };

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
      if (target.kind === "answer") {
        setText(target.url);
        setEditing(true);
        hideOmnibox();
        window.setTimeout(() => inputRef.current?.select(), 0);
        return;
      }
      finishEditing();
      if (target.kind === "url" && looksLikeUrl(target.url)) {
        void noteTyped(target.url);
      }
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

  /** Shift+Delete or the row's remove button: drops a history suggestion and its history entry. */
  const removeSuggestion = useCallback(
    (index: number) => {
      const { suggestions, selected } = omniboxRef.current;
      const suggestion = suggestions[index];
      if (!suggestion || suggestion.kind !== "history" || suggestion.historyId === undefined) {
        return false;
      }
      void removeHistory(suggestion.historyId);
      const next = suggestions.filter((_, at) => at !== index);
      if (next.length === 0) {
        hideOmnibox();
      } else {
        showOmnibox({ suggestions: next, selected: Math.min(selected, next.length - 1) });
      }
      return true;
    },
    [hideOmnibox, showOmnibox],
  );
  const removeRef = useRef(removeSuggestion);
  removeRef.current = removeSuggestion;

  useEffect(() => {
    const unlisteners = [
      onOmniboxReady(() => {
        void sendOmniboxState(omniboxRef.current);
      }),
      onOmniboxPick((suggestion) => pickRef.current(suggestion)),
      onOmniboxRemove((index) => {
        removeRef.current(index);
        inputRef.current?.focus();
      }),
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

  const onChange = (value: string, inserted: boolean) => {
    typedRef.current = value;
    const input = inputRef.current;
    const atEnd = !input || input.selectionStart === value.length;
    const completion = inserted && atEnd ? inlineCompletion(value, history, bookmarkLinks) : null;
    if (completion) {
      completionRange.current = [value.length, completion.length];
      setSelectionTick((tick) => tick + 1);
    }
    setText(completion ?? value);
    setEditing(true);
    const suggestions = buildSuggestions(
      value,
      bookmarkLinks,
      history,
      snapshot.windowTabs,
      activeId,
      snapshot.workspaces,
      snapshot.workspace,
      completion,
    );
    if (suggestions.length === 0) {
      hideOmnibox();
    } else {
      showOmnibox({ suggestions, selected: 0 });
    }
    const asked = value;
    if (asked.trim()) {
      void searchHistory(asked).then((hits) => {
        if (typedRef.current !== asked || hits.length === 0) {
          return;
        }
        const current = omniboxRef.current.suggestions;
        const seen = new Set(current.map((item) => item.url));
        const extra = hits
          .filter((hit) => !seen.has(hit.url))
          .slice(0, 4)
          .map((hit) => ({ kind: "history" as const, title: hit.title || hit.url, url: hit.url, historyId: hit.id }));
        if (extra.length === 0) {
          return;
        }
        showOmnibox({ suggestions: [...current, ...extra].slice(0, 8), selected: omniboxRef.current.selected });
      });
    }
    void omniboxExtra(asked).then((extra) => {
      if (!extra || typedRef.current !== asked) {
        return;
      }
      const row = {
        kind: extra.answer ? ("answer" as const) : ("url" as const),
        title: extra.title,
        url: extra.url,
      };
      const current = omniboxRef.current.suggestions.filter((item) => item.kind !== "answer" && item.title !== extra.title);
      const withoutSearch = extra.answer ? current : current.filter((item) => item.kind !== "search");
      showOmnibox({ suggestions: [row, ...withoutSearch].slice(0, 8), selected: 0 });
    });
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
    if (event.key === "Delete" && event.shiftKey && omniboxRef.current.suggestions.length > 0) {
      if (removeSuggestion(omniboxRef.current.selected)) {
        event.preventDefault();
      }
      return;
    }
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
  const { address: toolbarIds, all: toolbarOrderIds } = splitToolbar(
    settings,
    toolbarExtensions.map((extension) => extension.id),
  );
  const extensionIds = new Set(toolbarExtensions.map((extension) => extension.id));
  const addressBuiltins = toolbarIds.filter((id) => !extensionIds.has(id));
  const addressExtensions = toolbarIds.filter((id) => extensionIds.has(id));
  const moveToolbar = (from: string, to: string) => {
    if (!settings) {
      return;
    }
    const next = reorderToolbar(toolbarOrderIds, from, to);
    if (next) {
      void saveSettings({ ...settings, toolbarOrder: next });
    }
  };
  const secure = activeTab?.url.startsWith("https://");
  const insecure = activeTab?.url.startsWith("http://");
  const defaultZoom = !activeTab || activeTab.url.startsWith("ce://") ? 1 : (settings?.pageZoom ?? 100) / 100;
  const zoomPercent = activeTab ? Math.round(activeTab.zoom * 100) : 100;
  const showZoom = !!activeTab && Math.abs(activeTab.zoom - defaultZoom) > 0.001;
  const emphasis = useMemo(() => {
    if (editing || !activeTab || text !== activeTab.url) {
      return null;
    }
    let parsed: URL;
    try {
      parsed = new URL(activeTab.url);
    } catch {
      return null;
    }
    if (parsed.protocol !== "https:" && parsed.protocol !== "http:") {
      return null;
    }
    const host = parsed.hostname;
    const hostAt = activeTab.url.toLowerCase().indexOf(host, activeTab.url.indexOf("//") + 2);
    if (!host || hostAt < 0) {
      return null;
    }
    const site = activeTab.site && host.endsWith(activeTab.site) ? activeTab.site : host;
    const afterHost = activeTab.url.slice(hostAt + host.length);
    return {
      scheme: parsed.protocol === "http:" ? "http://" : "",
      sub: host.slice(0, host.length - site.length),
      site,
      rest: afterHost === "/" ? "" : afterHost,
    };
  }, [activeTab, editing, text]);

  return (
    <div
      id="address-bar"
      className={cn("relative flex h-10 shrink-0 items-center gap-1 bg-zinc-900 px-2", windowControls && "pr-0")}
      style={
        settings?.siteAccent && activeTab?.accent
          ? { backgroundColor: accentTint(activeTab.accent, 0.28) }
          : undefined
      }
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
        ) : (
          <button
            id="address-site-info"
            type="button"
            title={secure ? "Connection is secure. View site information" : insecure ? "Not secure. View site information" : "View site information"}
            aria-label="Site information"
            aria-haspopup="dialog"
            className={cn(
              "-ml-1.5 inline-flex h-6 shrink-0 items-center gap-1 rounded-full px-1.5 text-zinc-400 hover:bg-zinc-700 hover:text-zinc-100",
              focusRing,
              insecure && "text-amber-400",
            )}
            onClick={(event) => {
              const anchor = fieldRef.current ?? event.currentTarget;
              const rect = anchor.getBoundingClientRect();
              void openPopup({ kind: "site-info", params: `id=${activeTab.id}`, x: rect.left, y: rect.bottom + 4, width: 360, height: 480 });
            }}
          >
            {secure || activeTab.url.startsWith("ce://") ? (
              <Lock id="address-field-secure" className="size-3.5" aria-hidden />
            ) : insecure ? (
              <>
                <TriangleAlert id="address-field-insecure" className="size-3.5" aria-hidden />
                <span id="address-field-insecure-label" className="text-[11px]">
                  Not secure
                </span>
              </>
            ) : (
              <Info id="address-field-info" className="size-3.5" aria-hidden />
            )}
          </button>
        )}
        <div id="address-input-wrap" className="relative flex min-w-0 flex-1 items-center">
          {emphasis ? (
            <div id="address-display" aria-hidden className="pointer-events-none absolute inset-0 flex items-center overflow-hidden whitespace-pre text-sm">
              {emphasis.scheme ? (
                <span id="address-display-scheme" className="text-zinc-500">
                  {emphasis.scheme}
                </span>
              ) : null}
              <span id="address-display-sub" className="text-zinc-400">
                {emphasis.sub}
              </span>
              <span id="address-display-site" className="font-medium text-zinc-100">
                {emphasis.site}
              </span>
              <span id="address-display-rest" className="truncate text-zinc-500">
                {emphasis.rest}
              </span>
            </div>
          ) : null}
          <input
          id="address-input"
          ref={inputRef}
          value={text}
          spellCheck={false}
          autoComplete="off"
          placeholder="Search or enter address"
          aria-label="Address and search bar"
          className={cn(
            "min-w-0 flex-1 bg-transparent text-sm text-zinc-100 outline-none placeholder:text-zinc-500",
            emphasis && "text-transparent caret-transparent",
          )}
          onChange={(event) => {
            const inputType = (event.nativeEvent as InputEvent).inputType ?? "";
            onChange(event.target.value, inputType.startsWith("insert"));
          }}
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
        </div>
        {showZoom ? (
          <button
            id="address-zoom"
            type="button"
            title="Reset to default zoom"
            aria-label={`Page zoom ${zoomPercent} percent. Reset to default`}
            className={cn(
              "shrink-0 rounded bg-zinc-700 px-1.5 text-[11px] text-zinc-300 hover:bg-zinc-600 hover:text-zinc-100",
              focusRing,
            )}
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => activeId !== null && void zoomTab(activeId, "reset")}
          >
            {zoomPercent}%
          </button>
        ) : null}
        {storeAction ? (
          <button
            id="address-store-extension"
            type="button"
            disabled={storeBusy}
            title={storeAction === "update" ? `Update to ${listed?.updateVersion}` : "Install this extension"}
            className="shrink-0 rounded-full bg-sky-600 px-2 py-0.5 text-[11px] font-medium text-white hover:bg-sky-500 disabled:opacity-40"
            onClick={runStoreAction}
          >
            {storeBusy ? "Working…" : storeAction === "update" ? "Update" : "Install"}
          </button>
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

      <div id="address-tools" className="flex items-center">
        {settings?.verticalToolbar === false ? (
          <IconButton
            id="address-tools-side"
            label="Show tools on the side"
            onClick={() => void saveSettings({ ...settings, verticalToolbar: true })}
          >
            <PanelRight id="address-tools-side-icon" aria-hidden />
          </IconButton>
        ) : null}
        {addressBuiltins.map((id) => (
          <ToolbarEntry
            key={id}
            id={id}
            idPrefix="address"
            extension={undefined}
            activeTab={activeTab}
            settings={settings}
            labeled={false}
            onMove={moveToolbar}
          />
        ))}
        {addressBuiltins.length > 0 ? (
          <div id="address-tools-end" className="mx-0.5 h-4 w-px shrink-0 bg-zinc-600" />
        ) : null}
        {addressExtensions.map((id) => (
          <ToolbarEntry
            key={id}
            id={id}
            idPrefix="address"
            extension={toolbarExtensions.find((item) => item.id === id)}
            activeTab={activeTab}
            settings={settings}
            labeled={false}
            onMove={moveToolbar}
          />
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
      <IconButton
        id="address-restore-pins"
        label={pinLabel}
        disabled={closedPins.length === 0 || restoringPins}
        onClick={restorePins}
      >
        <Pin id="address-restore-pins-icon" aria-hidden />
      </IconButton>
      <IconButton
        id="address-status"
        label="Browser status"
        onClick={(event) => void openPopupBelow(event.currentTarget, { kind: "status", width: 300, height: 420 })}
      >
        <Activity id="address-status-icon" aria-hidden />
      </IconButton>
      <IconButton id="address-settings" label="Settings" onClick={() => void createTab("ce://settings")}>
        <Settings id="address-settings-icon" aria-hidden />
      </IconButton>

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
      <LoadBar id="address-load-bar" progress={activeTab?.progress ?? 0} loading={activeTab?.loading ?? false} />
    </div>
  );
}
