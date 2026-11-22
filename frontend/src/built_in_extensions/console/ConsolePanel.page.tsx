import { useEffect, useRef, useState } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AppWindow, Clock, Crosshair, Minus, Plus, Terminal, Trash2, X } from "lucide-react";
import type { TabsSnapshot } from "@type/browser";
import { Button } from "@/components/Button";
import { IconButton } from "@/components/IconButton";
import { Toggle } from "@/components/Toggle";
import {
  closeWindow,
  consoleArm,
  consoleDisarm,
  consoleEvaluate,
  consolePick,
  currentWindowLabel,
  listTabs,
  notifyConsoleCleared,
  onConsoleCleared,
  onConsoleMessage,
  onConsolePicked,
  onConsoleReset,
  onTabsChanged,
  openConsoleWindow,
  reload,
  toggleConsolePanel,
} from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import { useAppearanceState } from "@/lib/use-browser";
import { ConsoleLog } from "./ConsoleLog";
import { ConsolePrompt } from "./ConsolePrompt";
import { afterNavigation, clearLog, counts, emptyLog, passes, pushMessage, pushRow, visibleEntries, type Entry, type TabLog } from "./log";

const FILTERS = [
  ["all", "All"],
  ["error", "Errors"],
  ["warn", "Warnings"],
  ["info", "Info"],
  ["log", "Logs"],
  ["debug", "Debug"],
] as const;

let localId = 1;

function readFlag(key: string, fallback: boolean) {
  try {
    const raw = localStorage.getItem(key);
    if (raw === "1") return true;
    if (raw === "0") return false;
  } catch {
    // The panel still works with the default.
  }
  return fallback;
}

function writeFlag(key: string, value: boolean) {
  try {
    localStorage.setItem(key, value ? "1" : "0");
  } catch {
    // A private window can refuse storage. The choice still applies until reload.
  }
}

const FONT_SCALES = [0.8, 0.9, 1, 1.1, 1.25, 1.4, 1.6, 1.8];
const FONT_SCALE_KEY = "ce-console-font-scale";

function readFontScale() {
  try {
    const saved = Number(localStorage.getItem(FONT_SCALE_KEY));
    if (FONT_SCALES.includes(saved)) return saved;
  } catch {
    // The panel still works at the default size.
  }
  return 1;
}

function writeFontScale(value: number) {
  try {
    localStorage.setItem(FONT_SCALE_KEY, String(value));
  } catch {
    // A private window can refuse storage. The size still applies until reload.
  }
}

function pageLabel(url: string) {
  if (!url) {
    return "No page";
  }
  try {
    const parsed = new URL(url);
    if (parsed.protocol === "http:" || parsed.protocol === "https:") {
      return parsed.host;
    }
  } catch {
    return url;
  }
  return url;
}

function pageOrigin(url: string) {
  try {
    return new URL(url).origin;
  } catch {
    return url || "blank";
  }
}

function inputRow(source: string): Entry {
  return {
    key: `i-${localId++}`,
    kind: "input",
    level: "input",
    timestamp: Date.now(),
    origin: "",
    args: [],
    stack: [],
    text: source,
    table: null,
    depth: 0,
    collected: false,
  };
}

/** JavaScript console for the active tab. It listens to the page and runs in the page. */
export function ConsolePanelPage({ owner }: { owner: string }) {
  useAppearanceState();
  const poppedOut = getCurrentWindow().label === "popup-console";
  const ownerLabel = owner || currentWindowLabel();
  const logs = useRef(new Map<number, TabLog>());
  const preserveRef = useRef(false);
  const scroller = useRef<HTMLDivElement>(null);
  const stick = useRef(true);
  const [snapshot, setSnapshot] = useState<TabsSnapshot | null>(null);
  const [tick, setTick] = useState(0);
  const [closed, setClosed] = useState<Set<string>>(() => new Set());
  const [filter, setFilter] = useState<(typeof FILTERS)[number][0]>("all");
  const [query, setQuery] = useState("");
  const [preserve, setPreserve] = useState(() => readFlag("ce-console-preserve", false));
  const [timestamps, setTimestamps] = useState(() => readFlag("ce-console-timestamps", true));
  const [fontScale, setFontScale] = useState(readFontScale);
  const fontStep = FONT_SCALES.indexOf(fontScale);
  const [busy, setBusy] = useState(false);
  const [picking, setPicking] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  /** Tab whose current document has been watched since navigation started. */
  const [caughtTab, setCaughtTab] = useState<number | null>(null);
  const pickedTab = useRef<number | null>(null);
  const inputDrag = useRef<{ y: number; height: number } | null>(null);
  const [inputHeight, setInputHeight] = useState(120);
  preserveRef.current = preserve;

  const mine = snapshot?.windows.find((window) => window.label === ownerLabel);
  const activeId = mine?.activeId ?? null;
  const active = snapshot?.tabs.find((tab) => tab.id === activeId);
  const ready = snapshot !== null;
  const open = poppedOut || Boolean(mine?.consolePanel);
  const origin = pageOrigin(active?.url ?? "");
  const revision = tick;
  const entries = revision < 0 || activeId == null ? [] : (logs.current.get(activeId)?.entries ?? []);
  const totals = counts(entries);
  const shown = visibleEntries(entries, closed).filter((entry) => passes(entry, filter, query.trim().toLowerCase()));

  useEffect(() => {
    document.title = "Console";
  }, []);

  useEffect(() => {
    let alive = true;
    let stop: UnlistenFn | undefined;
    void listTabs().then((value) => {
      if (alive) setSnapshot(value);
    });
    void onTabsChanged((value) => {
      if (alive) setSnapshot(value);
    }).then((unlisten) => {
      if (alive) stop = unlisten;
      else unlisten();
    });
    return () => {
      alive = false;
      stop?.();
    };
  }, []);

  useEffect(() => {
    if (!ready) {
      return;
    }
    if (!open) {
      void consoleDisarm(ownerLabel);
      return;
    }
    if (activeId == null) {
      return;
    }
    void consoleArm(ownerLabel, activeId);
  }, [ready, open, activeId, ownerLabel]);

  useEffect(() => {
    setCaughtTab(null);
  }, [activeId]);

  useEffect(() => {
    const previous = pickedTab.current;
    if (previous != null && previous !== activeId) {
      void consolePick(previous, false);
    }
    pickedTab.current = null;
    setPicking(false);
  }, [activeId]);

  useEffect(() => {
    let alive = true;
    let stops: UnlistenFn[] = [];
    let frame = 0;
    const bump = () => {
      if (frame) return;
      frame = requestAnimationFrame(() => {
        frame = 0;
        setTick((value) => value + 1);
      });
    };
    const pending = Promise.all([
      onConsoleMessage((message) => {
        if (!alive) return;
        const current = logs.current.get(message.tabId) ?? emptyLog();
        const next = pushMessage(current, message);
        logs.current.set(message.tabId, next.log);
        if (next.collapse) {
          setClosed((groups) => new Set(groups).add(next.collapse as string));
        }
        bump();
      }),
      onConsoleCleared((message) => {
        if (!alive) return;
        logs.current.set(message.tabId, clearLog());
        bump();
      }),
      onConsoleReset((message) => {
        if (!alive) return;
        const current = logs.current.get(message.tabId) ?? emptyLog();
        logs.current.set(message.tabId, afterNavigation(current, message.at, preserveRef.current));
        setCaughtTab(message.tabId);
        bump();
      }),
      onConsolePicked((message) => {
        if (!alive) return;
        const current = logs.current.get(message.tabId) ?? emptyLog();
        logs.current.set(
          message.tabId,
          pushRow(current, {
            key: `p-${localId++}`,
            kind: "result",
            level: "result",
            timestamp: Date.now(),
            origin: "",
            args: [message.value],
            stack: [],
            text: "$0",
            table: null,
            depth: 0,
            collected: false,
          }),
        );
        setPicking(false);
        bump();
      }),
    ]).then((unlisten) => {
      if (!alive) {
        for (const stop of unlisten) stop();
        return;
      }
      stops = unlisten;
    });
    return () => {
      alive = false;
      cancelAnimationFrame(frame);
      void pending.then(() => {
        for (const stop of stops) stop();
      });
    };
  }, []);

  useEffect(() => {
    const node = scroller.current;
    if (node && stick.current) {
      node.scrollTop = node.scrollHeight;
    }
  }, [tick, activeId, filter, query, fontScale]);

  const run = (source: string) => {
    if (activeId == null) {
      return;
    }
    const tab = activeId;
    const current = logs.current.get(tab) ?? emptyLog();
    logs.current.set(tab, pushRow(current, inputRow(source)));
    setTick((value) => value + 1);
    setBusy(true);
    setStatus(null);
    void consoleEvaluate(tab, source)
      .then((result) => {
        const latest = logs.current.get(tab) ?? emptyLog();
        logs.current.set(
          tab,
          pushRow(latest, {
            key: `r-${localId++}`,
            kind: "result",
            level: result.error ? "error" : "result",
            timestamp: Date.now(),
            origin: "",
            args: [result.value],
            stack: result.stack,
            text: result.text,
            table: null,
            depth: 0,
            collected: false,
          }),
        );
        setTick((value) => value + 1);
      })
      .catch((reason: unknown) => setStatus(String(reason)))
      .finally(() => setBusy(false));
  };

  const clear = () => {
    if (activeId == null) {
      return;
    }
    logs.current.set(activeId, clearLog());
    setTick((value) => value + 1);
    void notifyConsoleCleared(activeId);
  };

  const stepFont = (delta: number) => {
    const next = FONT_SCALES[fontStep + delta];
    if (next === undefined) {
      return;
    }
    setFontScale(next);
    writeFontScale(next);
  };

  const pick = () => {
    if (activeId == null) {
      return;
    }
    void consolePick(activeId, !picking)
      .then((enabled) => {
        pickedTab.current = enabled ? activeId : null;
        setPicking(enabled);
      })
      .catch((reason: unknown) => setStatus(String(reason)));
  };

  return (
    <div id="console-panel" className="flex h-screen flex-col bg-zinc-950 text-zinc-100">
      <header id="console-panel-header" className="flex h-[var(--toolbar-height)] shrink-0 select-none items-center gap-2 border-b border-zinc-800 pl-4 pr-2">
        <Terminal id="console-panel-icon" className="size-4 shrink-0 text-sky-400" aria-hidden />
        <h1 id="console-panel-title" className="text-sm font-semibold">
          Console
        </h1>
        <span id="console-panel-origin" className="min-w-0 flex-1 truncate text-xs text-zinc-500" title={active?.url || "No page"}>
          {pageLabel(active?.url ?? "")}
        </span>
        <IconButton
          id="console-panel-font-smaller"
          label={`Smaller text (${Math.round(fontScale * 100)}%)`}
          className="size-7"
          disabled={fontStep <= 0}
          onClick={() => stepFont(-1)}
        >
          <Minus id="console-panel-font-smaller-icon" aria-hidden />
        </IconButton>
        <IconButton
          id="console-panel-font-larger"
          label={`Larger text (${Math.round(fontScale * 100)}%)`}
          className="size-7"
          disabled={fontStep >= FONT_SCALES.length - 1}
          onClick={() => stepFont(1)}
        >
          <Plus id="console-panel-font-larger-icon" aria-hidden />
        </IconButton>
        <IconButton id="console-panel-pick" label={picking ? "Stop picking an element" : "Pick an element"} active={picking} aria-pressed={picking} className="size-7" disabled={activeId == null} onClick={pick}>
          <Crosshair id="console-panel-pick-icon" aria-hidden />
        </IconButton>
        <IconButton id="console-panel-clear" label="Clear the console" className="size-7" disabled={entries.length === 0} onClick={clear}>
          <Trash2 id="console-panel-clear-icon" aria-hidden />
        </IconButton>
        {poppedOut ? null : (
          <IconButton id="console-panel-popout" label="Open in a new window" className="size-7" onClick={() => void openConsoleWindow()}>
            <AppWindow id="console-panel-popout-icon" aria-hidden />
          </IconButton>
        )}
        <IconButton
          id="console-panel-close"
          label={poppedOut ? "Close this window" : "Close the console"}
          className="size-7"
          onClick={() => void (poppedOut ? closeWindow() : toggleConsolePanel(false))}
        >
          <X id="console-panel-close-icon" aria-hidden />
        </IconButton>
      </header>
      <div id="console-toolbar" className="flex shrink-0 select-none flex-wrap items-center gap-1 border-b border-zinc-800 px-2 py-1">
        {FILTERS.map(([id, label]) => (
          <button
            id={`console-filter-${id}`}
            key={id}
            type="button"
            aria-pressed={filter === id}
            className={cn(
              "rounded-full px-2 py-0.5 text-[11px]",
              focusRing,
              filter === id ? "bg-zinc-800 text-zinc-100" : "text-zinc-500 hover:text-zinc-300",
            )}
            onClick={() => setFilter(id)}
          >
            {label}
            <span id={`console-filter-${id}-count`} className="ml-1 text-zinc-600">
              {totals[id]}
            </span>
          </button>
        ))}
        <input
          id="console-search"
          value={query}
          placeholder="Filter"
          aria-label="Filter messages"
          className={cn("ml-auto h-6 w-28 min-w-0 select-text rounded-md border border-zinc-800 bg-zinc-950 px-2 text-xs text-zinc-100 outline-none placeholder:text-zinc-600 focus:border-sky-500", focusRing)}
          onChange={(event) => setQuery(event.target.value)}
        />
        <IconButton
          id="console-timestamps"
          label="Timestamps"
          active={timestamps}
          aria-pressed={timestamps}
          className="size-6"
          onClick={() => {
            const next = !timestamps;
            setTimestamps(next);
            writeFlag("ce-console-timestamps", next);
          }}
        >
          <Clock id="console-timestamps-icon" aria-hidden />
        </IconButton>
        <label id="console-preserve" className="flex items-center gap-1.5 pr-1 text-[11px] text-zinc-400">
          <Toggle
            id="console-preserve-toggle"
            size="sm"
            checked={preserve}
            aria-label="Preserve log"
            onChange={(checked) => {
              setPreserve(checked);
              writeFlag("ce-console-preserve", checked);
            }}
          />
          Preserve
        </label>
      </div>
      <div
        id="console-scroller"
        ref={scroller}
        className="min-h-0 flex-1 overflow-auto"
        onScroll={(event) => {
          const node = event.currentTarget;
          stick.current = node.scrollHeight - node.scrollTop - node.clientHeight < 48;
        }}
      >
        {entries.length === 0 ? (
          <div id="console-empty" className="flex h-full flex-col items-start justify-center gap-2 px-4">
            <p id="console-empty-title" className="text-sm text-zinc-300">
              {activeId == null ? "Open a tab to use the console." : "Listening to this page."}
            </p>
            {activeId != null && caughtTab !== activeId ? (
              <>
                <p id="console-empty-hint" className="max-w-sm text-xs leading-5 text-zinc-500">
                  Messages from before the console opened are not here. Reload to capture them from the start.
                </p>
                <Button id="console-reload" size="sm" variant="outline" onClick={() => void reload(activeId)}>
                  Reload
                </Button>
              </>
            ) : null}
          </div>
        ) : shown.length === 0 ? (
          <p id="console-filter-empty" className="px-4 py-6 text-xs text-zinc-500">
            Nothing matches.
          </p>
        ) : (
          <div id="console-log-scale" style={{ zoom: fontScale }}>
            <ConsoleLog
              entries={shown}
              tabId={activeId ?? 0}
              timestamps={timestamps}
              pageOrigin={origin}
              closed={closed}
              onToggleGroup={(key) => {
                setClosed((groups) => {
                  const next = new Set(groups);
                  if (next.has(key)) next.delete(key);
                  else next.add(key);
                  return next;
                });
              }}
              onStatus={setStatus}
            />
          </div>
        )}
      </div>
      {status ? (
        <p id="console-status" role="status" className="shrink-0 border-t border-zinc-800 px-3 py-1 text-xs text-red-300">
          {status}
        </p>
      ) : null}
      {busy ? (
        <p id="console-running" role="status" className="shrink-0 px-3 py-1 text-[11px] text-zinc-500">
          Running…
        </p>
      ) : null}
      <div id="console-input" style={{ height: inputHeight }} className="flex shrink-0 flex-col bg-zinc-900">
        <div
          id="console-input-resize"
          role="separator"
          aria-orientation="horizontal"
          aria-label="Resize the prompt"
          className="h-1.5 shrink-0 cursor-ns-resize border-t border-zinc-700"
          onPointerDown={(event) => {
            event.currentTarget.setPointerCapture(event.pointerId);
            inputDrag.current = { y: event.clientY, height: inputHeight };
          }}
          onPointerMove={(event) => {
            const drag = inputDrag.current;
            if (!drag) {
              return;
            }
            const next = drag.height + (drag.y - event.clientY);
            setInputHeight(Math.min(window.innerHeight * 0.7, Math.max(72, next)));
          }}
          onPointerUp={() => {
            inputDrag.current = null;
          }}
        />
        <ConsolePrompt tabId={activeId} origin={origin} busy={busy} onRun={run} scale={fontScale} />
        <p id="console-hint" className="shrink-0 bg-zinc-900 px-3 py-1 text-[10px] text-zinc-500">
          {activeId == null
            ? "Open a tab to run JavaScript on a page."
            : "Enter runs. Shift+Enter adds a line. $ and $$ search this page. $_ is the last result."}
        </p>
      </div>
    </div>
  );
}
