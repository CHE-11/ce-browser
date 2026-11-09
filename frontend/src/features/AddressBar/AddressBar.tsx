import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import {
  ArrowLeft,
  ArrowRight,
  Columns2,
  Download,
  EllipsisVertical,
  History,
  House,
  Info,
  Lock,
  Mic,
  Pin,
  RefreshCw,
  RotateCw,
  Search,
  Settings,
  Star,
  TriangleAlert,
  Volume2,
  VolumeX,
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
  PermissionGrant,
  TabInfo,
} from "../../../../types/browser";
import {
  activateTab,
  closePopup,
  createTab,
  exitSplit,
  installExtensionFromStore,
  installUpdate,
  goBack,
  goForward,
  listPermissions,
  navigate,
  noteTyped,
  omniboxExtra,
  removeHistory,
  searchHistory,
  onBrowserCommand,
  onDownloadStarted,
  onMicrophoneCapture,
  onOmniboxPick,
  onOmniboxReady,
  onOmniboxRemove,
  onPermissionsChanged,
  onPopupChanged,
  openPopup,
  reload,
  restorePinnedTabs,
  saveSettings,
  setTabsMuted,
  showNotice,
  sendOmniboxState,
  splitWith,
  stop,
  updateExtension,
  zoomTab,
} from "../../lib/browser-client";
import { storeListing } from "../../lib/extension-store";
import { accentTint } from "../../lib/accent";
import { cn } from "../../lib/cn";
import { useHostExtensions } from "@/lib/use-browser";
import { hostToolbarIds, reorderToolbarAt, splitToolbar } from "../../lib/toolbar-pin";
import { buildSuggestions, inlineCompletion, looksLikeUrl } from "../../lib/omnibox";
import { flattenBookmarks, useUpdateStatus, type WindowTabsState } from "../../lib/use-browser";
import { focusRing } from "../../lib/focus-ring";
import { registerNewTabTyping } from "../../lib/new-tab-typing";
import { useStickyAudio } from "../../lib/use-sticky-audio";
import { IconButton } from "../../components/IconButton";
import { LoadBar } from "../../components/LoadBar";
import { WindowControls } from "../../components/WindowControls";
import { AddressBarExtensions } from "./AddressBarExtensions";
import { ProfileSwitcherButton } from "./ProfileSwitcherButton";

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

function addressText(tab: TabInfo | undefined) {
  if (!tab || tab.url === "ce://newtab") {
    return "";
  }
  return tab.url;
}

/**
 * A selection paints its own color, so `text-transparent` does not hide it. After the field blurs,
 * that highlight stays on the full URL while the formatted address (scheme omitted) is drawn too.
 */
function clearAddressSelection(input: HTMLInputElement | null) {
  if (!input) {
    return;
  }
  if (input.selectionStart !== input.selectionEnd) {
    input.setSelectionRange(0, 0);
  }
  input.scrollLeft = 0;
}

function pageOrigin(url: string) {
  try {
    const parsed = new URL(url);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return "";
    }
    return parsed.origin;
  } catch {
    return "";
  }
}

export function AddressBar({ snapshot, bookmarks, downloads, history, extensions, settings, windowControls = false }: AddressBarProps) {
  const activeTab = snapshot.tabs.find((tab) => tab.id === snapshot.activeId);
  const activeId = activeTab?.id ?? null;
  const activePinned = !!activeTab?.pinned;
  const showSpeakers = useStickyAudio(activeId, !!activeTab?.audible, !!activeTab?.muted);
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
  const restoringPins = useRef(false);
  const restorePins = () => {
    if (restoringPins.current) {
      return;
    }
    restoringPins.current = true;
    void restorePinnedTabs().finally(() => {
      restoringPins.current = false;
    });
  };

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
  const historyRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLButtonElement>(null);

  const [text, setText] = useState(() => addressText(activeTab));
  const [editing, setEditing] = useState(false);
  const [omnibox, setOmnibox] = useState<OmniboxState>(EMPTY_OMNIBOX);
  const [downloadFlash, setDownloadFlash] = useState(false);
  const [storeBusy, setStoreBusy] = useState(false);
  const [grants, setGrants] = useState<PermissionGrant[]>([]);
  const [micCapture, setMicCapture] = useState<ReadonlySet<number>>(() => new Set());
  const omniboxRef = useRef(omnibox);
  const popupOpenRef = useRef(false);
  /** Bumped when the suggestions are dismissed, so a late search result cannot open them again. */
  const suggestEpoch = useRef(0);
  /** Identifies the open in flight. Dismissing the list makes that open close itself. */
  const popupTicket = useRef(0);
  const blurTimer = useRef<number | null>(null);
  /** What the user typed. Anything after it in `text` is an inline completion. */
  const typedRef = useRef("");
  const textRef = useRef(text);
  textRef.current = text;
  const editingStateRef = useRef(editing);
  editingStateRef.current = editing;
  /** The new tab was requested, and keystrokes belong to it before that tab is active. */
  const armRef = useRef(false);
  /** Keystrokes that miss the field still count, until the new page view stops taking focus. */
  const redirectKeys = useRef(false);
  const redirectStamp = useRef(0);
  const pendingNav = useRef<string | null>(null);
  const restoreRef = useRef<{ text: string; editing: boolean } | null>(null);
  const dismissRef = useRef<() => void>(() => {});
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
  const micOrigin = activeTab ? pageOrigin(activeTab.url) : "";
  const micGrant = grants.find((grant) => grant.origin === micOrigin && grant.kind === "microphone");
  const micPermitted = micGrant?.state === "allow" || micGrant?.state === "session";
  const micLive = activeId !== null && micCapture.has(activeId);
  const showMicrophone = !!activeTab && !!micOrigin && (micPermitted || micLive);
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

  useEffect(() => {
    let closed = false;
    void listPermissions()
      .then((next) => {
        if (!closed) {
          setGrants(next);
        }
      })
      .catch(() => undefined);
    const grantsListen = onPermissionsChanged((next) => setGrants(next));
    const captureListen = onMicrophoneCapture((event) => {
      setMicCapture((current) => {
        const next = new Set(current);
        if (event.active) {
          next.add(event.tabId);
        } else {
          next.delete(event.tabId);
        }
        return next;
      });
    });
    return () => {
      closed = true;
      void grantsListen.then((stop) => stop());
      void captureListen.then((stop) => stop());
    };
  }, []);

  useLayoutEffect(() => {
    const tab = activeRef.current;
    const input = inputRef.current;
    if (tab?.url === "ce://newtab") {
      const keep = (armRef.current || redirectKeys.current) && typedRef.current !== "";
      setEditing(true);
      if (!keep) {
        typedRef.current = "";
        setText("");
      }
      if (blurTimer.current !== null) {
        window.clearTimeout(blurTimer.current);
        blurTimer.current = null;
      }
      input?.focus();
      const queued = pendingNav.current;
      pendingNav.current = null;
      armRef.current = false;
      restoreRef.current = null;
      if (queued) {
        redirectKeys.current = false;
        typedRef.current = "";
        setText(queued);
        setEditing(false);
        input?.blur();
        void navigate(tab.id, queued);
      }
      return;
    }
    if (!armRef.current) {
      setEditing(false);
    }
  }, [activeId]);

  useEffect(() => {
    if (activeRef.current?.url !== "ce://newtab") {
      return;
    }
    const stamp = redirectStamp.current;
    const timer = window.setTimeout(() => {
      if (redirectStamp.current === stamp) {
        redirectKeys.current = false;
      }
    }, 1500);
    return () => window.clearTimeout(timer);
  }, [activeId]);

  const beginNewTab = (force: boolean) => {
    const input = inputRef.current;
    if (!force && (armRef.current || redirectKeys.current)) {
      input?.focus();
      return;
    }
    armRef.current = true;
    redirectKeys.current = true;
    redirectStamp.current += 1;
    pendingNav.current = null;
    restoreRef.current = { text: textRef.current, editing: editingStateRef.current };
    typedRef.current = "";
    if (blurTimer.current !== null) {
      window.clearTimeout(blurTimer.current);
      blurTimer.current = null;
    }
    if (input) {
      input.value = "";
      input.focus();
      input.setSelectionRange(0, 0);
    }
    setText("");
    setEditing(true);
    dismissRef.current();
  };

  const cancelNewTab = () => {
    if (!armRef.current && !redirectKeys.current && pendingNav.current === null) {
      return;
    }
    const queued = pendingNav.current;
    pendingNav.current = null;
    const restore = restoreRef.current;
    armRef.current = false;
    redirectKeys.current = false;
    restoreRef.current = null;
    if (queued && activeRef.current) {
      void navigate(activeRef.current.id, queued);
      return;
    }
    if (typedRef.current === "" && restore) {
      setText(restore.text);
      setEditing(restore.editing);
      if (!restore.editing) {
        inputRef.current?.blur();
      }
    }
  };

  const beginNewTabRef = useRef(beginNewTab);
  const cancelNewTabRef = useRef(cancelNewTab);
  beginNewTabRef.current = beginNewTab;
  cancelNewTabRef.current = cancelNewTab;

  useEffect(() => {
    return registerNewTabTyping({
      begin: (force) => beginNewTabRef.current(force),
      cancel: () => cancelNewTabRef.current(),
    });
  }, []);

  const wasEditingRef = useRef(false);
  useLayoutEffect(() => {
    const wasEditing = wasEditingRef.current;
    wasEditingRef.current = editing;
    if (wasEditing && !editing) {
      clearAddressSelection(inputRef.current);
    }
  }, [editing]);

  const hideOmnibox = useCallback(() => {
    popupTicket.current += 1;
    omniboxRef.current = EMPTY_OMNIBOX;
    setOmnibox(EMPTY_OMNIBOX);
    const wasOpen = popupOpenRef.current;
    popupOpenRef.current = false;
    if (wasOpen) {
      void closePopup();
    }
  }, []);

  const dismissSuggestions = useCallback(() => {
    suggestEpoch.current += 1;
    hideOmnibox();
  }, [hideOmnibox]);
  dismissRef.current = dismissSuggestions;

  const showOmnibox = useCallback((state: OmniboxState) => {
    omniboxRef.current = state;
    setOmnibox(state);
    const field = fieldRef.current;
    if (!field || state.suggestions.length === 0) {
      return;
    }
    if (popupOpenRef.current) {
      void sendOmniboxState(state);
      return;
    }
    popupOpenRef.current = true;
    const ticket = ++popupTicket.current;
    void openPopup("omnibox", field, {
      placement: "bottom-start",
      focus: false,
      minWidth: field.getBoundingClientRect().width,
    }).finally(() => {
      if (ticket !== popupTicket.current && !popupOpenRef.current) {
        void closePopup();
      }
    });
  }, []);

  const finishEditing = useCallback(() => {
    typedRef.current = "";
    clearAddressSelection(inputRef.current);
    setEditing(false);
    dismissSuggestions();
    inputRef.current?.blur();
  }, [dismissSuggestions]);

  const pick = useCallback(
    (suggestion: OmniboxSuggestion | null, inNewTab = false) => {
      const target = suggestion ?? { kind: "url" as const, title: text, url: text.trim() };
      if (!target.url) {
        return;
      }
      if (target.kind === "answer") {
        typedRef.current = target.url;
        setText(target.url);
        setEditing(true);
        dismissSuggestions();
        window.setTimeout(() => inputRef.current?.select(), 0);
        return;
      }
      if ((armRef.current || redirectKeys.current) && activeRef.current?.url !== "ce://newtab") {
        if (target.kind === "tab" && target.tabId !== undefined) {
          armRef.current = false;
          redirectKeys.current = false;
          pendingNav.current = null;
          finishEditing();
          void activateTab(target.tabId);
          return;
        }
        pendingNav.current = target.url;
        dismissSuggestions();
        return;
      }
      finishEditing();
      if (target.kind === "url" && looksLikeUrl(target.url)) {
        void noteTyped(target.url);
      }
      if (target.kind === "tab" && target.tabId !== undefined) {
        void activateTab(target.tabId);
      } else if (inNewTab || activeId === null || (target.kind === "bookmark" && activePinned)) {
        void createTab(target.url);
      } else {
        setText(target.url);
        void navigate(activeId, target.url);
      }
    },
    [activeId, activePinned, dismissSuggestions, finishEditing, text],
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
        if (command === "new-tab") {
          beginNewTabRef.current(false);
          return;
        }
        if (command === "focus-address") {
          const input = inputRef.current;
          if (!input) {
            return;
          }
          const resting = addressText(activeRef.current);
          const inProgress =
            input.value !== resting && (document.activeElement === input || redirectKeys.current || armRef.current);
          input.focus();
          if (!inProgress) {
            input.select();
          }
          if (activeRef.current?.url === "ce://newtab") {
            redirectKeys.current = false;
          }
        } else if (command === "bookmark" && starRef.current) {
          openBookmarkEditor(starRef.current);
        } else if (command === "downloads") {
          const anchor = downloadsRef.current ?? menuRef.current;
          if (anchor) {
            void openPopup("downloads", anchor);
          }
        }
      }),
      onDownloadStarted(() => {
        setDownloadFlash(true);
        window.setTimeout(() => setDownloadFlash(false), 1600);
        const anchor = downloadsRef.current;
        if (anchor) {
          // The page keeps the keyboard. A click outside the panel still closes it.
          void openPopup("downloads", anchor, { focus: false });
        }
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
    void openPopup("bookmark-edit", anchor, { params: params.toString() });
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
    const epoch = suggestEpoch.current;
    if (asked.trim()) {
      void searchHistory(asked).then((hits) => {
        if (epoch !== suggestEpoch.current || typedRef.current !== asked || hits.length === 0) {
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
      if (!extra || epoch !== suggestEpoch.current || typedRef.current !== asked) {
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
  const changeRef = useRef(onChange);
  changeRef.current = onChange;

  useEffect(() => {
    const onKey = (event: globalThis.KeyboardEvent) => {
      if (!redirectKeys.current) {
        return;
      }
      const input = inputRef.current;
      if (!input || document.activeElement === input || event.isComposing) {
        return;
      }
      if (event.ctrlKey || event.metaKey || event.altKey) {
        return;
      }
      const target = event.target;
      if (target instanceof HTMLElement && target.closest("input, textarea, [contenteditable='true']")) {
        return;
      }
      if (event.key === "Enter") {
        event.preventDefault();
        event.stopPropagation();
        input.focus();
        const { suggestions, selected } = omniboxRef.current;
        pickRef.current(suggestions[selected] ?? null, event.altKey);
        return;
      }
      if (event.key === "Backspace") {
        event.preventDefault();
        event.stopPropagation();
        input.focus();
        const start = input.selectionStart ?? input.value.length;
        const end = input.selectionEnd ?? start;
        const next =
          start === end
            ? input.value.slice(0, Math.max(0, start - 1)) + input.value.slice(end)
            : input.value.slice(0, start) + input.value.slice(end);
        changeRef.current(next, false);
        return;
      }
      if (event.key.length !== 1) {
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      input.focus();
      const start = input.selectionStart ?? input.value.length;
      const end = input.selectionEnd ?? start;
      changeRef.current(input.value.slice(0, start) + event.key + input.value.slice(end), true);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, []);

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
        dismissSuggestions();
      } else {
        setText(addressText(activeTab));
        finishEditing();
      }
    }
  };

  const update = useUpdateStatus();
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

  const toolbarExtensions = extensions.filter((extension) => extension.enabled);
  const hostIds = hostToolbarIds(useHostExtensions().extensions);
  const { address: addressExtensions, all: toolbarOrderIds } = splitToolbar(settings, [
    ...toolbarExtensions.map((extension) => extension.id),
    ...hostIds,
  ]);
  const moveToolbar = (from: string, dropIndex: number) => {
    if (!settings) {
      return;
    }
    const next = reorderToolbarAt(toolbarOrderIds, addressExtensions, from, dropIndex);
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
      className={cn("chrome-toolbar relative flex h-10 shrink-0 items-center gap-1 bg-zinc-900 px-2", windowControls && "pr-0")}
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
          "chrome-address-field mx-1 flex h-8 min-w-0 flex-1 items-center gap-2 rounded-full bg-zinc-800 px-3 ring-sky-500/70 transition-shadow",
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
              void openPopup("site-info", anchor, { params: `id=${activeTab.id}`, placement: "bottom-start" });
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
            <div
              id="address-display"
              aria-hidden
              className="chrome-address-text pointer-events-none absolute inset-0 z-10 flex items-center overflow-hidden whitespace-pre bg-zinc-800 text-sm"
            >
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
            "chrome-address-text min-w-0 flex-1 bg-transparent text-sm text-zinc-100 outline-none placeholder:text-zinc-500",
            emphasis &&
              "text-transparent caret-transparent selection:bg-transparent selection:text-transparent [-webkit-text-fill-color:transparent]",
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
            const input = event.currentTarget;
            if (input.value === addressText(activeRef.current)) {
              input.select();
            }
          }}
          onBlur={() => {
            if (armRef.current || redirectKeys.current) {
              return;
            }
            blurTimer.current = window.setTimeout(() => {
              blurTimer.current = null;
              clearAddressSelection(inputRef.current);
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
        {activeTab && showSpeakers ? (
          <button
            id="address-speakers"
            type="button"
            title={activeTab.muted ? "Unmute this tab" : "Mute this tab"}
            aria-label={activeTab.muted ? "Unmute this tab" : "Mute this tab"}
            className={cn(
              "inline-flex size-6 shrink-0 items-center justify-center rounded-full hover:bg-zinc-700",
              focusRing,
              activeTab.muted ? "text-zinc-400 hover:text-zinc-100" : "text-sky-400",
            )}
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => activeId !== null && void setTabsMuted([activeId], !activeTab.muted)}
          >
            {activeTab.muted ? (
              <VolumeX id="address-speakers-icon" className="size-3.5" aria-hidden />
            ) : (
              <Volume2 id="address-speakers-icon" className="size-3.5" aria-hidden />
            )}
          </button>
        ) : null}
        {showMicrophone ? (
          <button
            id="address-microphone"
            type="button"
            title={micLive ? "This site is using your microphone" : "Microphone is allowed on this site"}
            aria-label={micLive ? "This site is using your microphone" : "Microphone is allowed on this site"}
            aria-haspopup="dialog"
            className={cn(
              "inline-flex size-6 shrink-0 items-center justify-center rounded-full hover:bg-zinc-700",
              focusRing,
              micLive ? "text-sky-400" : "text-zinc-400 hover:text-zinc-100",
            )}
            onMouseDown={(event) => event.preventDefault()}
            onClick={(event) => {
              if (!activeTab) {
                return;
              }
              const anchor = fieldRef.current ?? event.currentTarget;
              void openPopup("site-info", anchor, { params: `id=${activeTab.id}&view=microphone`, placement: "bottom-start" });
            }}
          >
            <Mic id="address-microphone-icon" className="size-3.5" aria-hidden />
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

      <AddressBarExtensions
        addressExtensions={addressExtensions}
        toolbarExtensions={toolbarExtensions}
        activeTab={activeTab}
        settings={settings}
        onMove={moveToolbar}
      />
      
      <div id="address-extensions-end" className="mx-0.5 h-4 w-px shrink-0 bg-zinc-600" />

      <IconButton
        id="address-downloads"
        ref={downloadsRef}
        label="Downloads (Ctrl+J)"
        className={cn("relative", downloadFlash && "animate-pulse bg-sky-500/25 text-sky-300")}
        onClick={(event) =>
          void openPopup("downloads", event.currentTarget)
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
      <IconButton
        id="address-history"
        ref={historyRef}
        label="History"
        onClick={(event) => void openPopup("history", event.currentTarget)}
      >
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
      <div
        id="address-pin-menu-anchor"
        className="shrink-0"
        onContextMenu={(event) => {
          event.preventDefault();
          const params = new URLSearchParams();
          if (activeTab?.url) {
            params.set("url", activeTab.url);
          }
          void openPopup("pin-menu", { x: event.clientX, y: event.clientY }, { params: params.toString() });
        }}
      >
        <IconButton id="address-restore-pins" label="Refresh pinned tabs" onClick={restorePins}>
          <Pin id="address-restore-pins-icon" aria-hidden />
        </IconButton>
      </div>

      <ProfileSwitcherButton />
      <IconButton id="address-settings" label="Settings" onClick={() => void createTab("ce://settings")}>
        <Settings id="address-settings-icon" aria-hidden />
      </IconButton>
      {update?.available ? (
        <IconButton
          id="address-update"
          label={update.installing ? "Downloading the update…" : `Restart to update to ${update.available}`}
          disabled={update.installing}
          className="text-sky-300 hover:bg-sky-500/20 hover:text-sky-200"
          onClick={() => void installUpdate()}
        >
          <RefreshCw id="address-update-icon" aria-hidden className={update.installing ? "animate-spin" : undefined} />
        </IconButton>
      ) : null}

      <IconButton
        id="address-menu"
        ref={menuRef}
        label="Menu"
        onClick={(event) => void openPopup("address-bar-ellipsis-menu", event.currentTarget)}
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
