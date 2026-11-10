import { useEffect, useRef, useState } from "react";
import { Moon } from "lucide-react";
import { FavIcon } from "@/components/FavIcon";
import { PopupFrame } from "@/components/PopupFrame";
import { activateTab, closePopup, ctrlHeld, onTabOverviewZoom, openPopup, tabThumbnail } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { hostOf } from "@/lib/format";
import { groupColorClasses } from "@/lib/tab-rows";
import { useTabs } from "@/lib/use-browser";
import { PAGE_ZOOM_LEVELS, stepPageZoom } from "@/lib/zoom";
import type { TabGroup, TabInfo } from "@type/browser";

const THUMB_BATCH = 4;
const WINDOW_MARGIN = 32;
const CARD_REM = 14;
const GAP_REM = 0.5;
const PAD_REM = 0.75;
const ZOOM_KEY = "ce-browser.tab-overview-zoom";
const RELEASE_POLL_MS = 60;

type TabOverviewProps = {
  params: URLSearchParams;
};

function readOverviewZoom() {
  try {
    const value = Number(localStorage.getItem(ZOOM_KEY));
    if (PAGE_ZOOM_LEVELS.includes(value)) {
      return value;
    }
  } catch {
    // Missing or unreadable.
  }
  return 100;
}

function writeOverviewZoom(zoom: number) {
  try {
    localStorage.setItem(ZOOM_KEY, String(zoom));
  } catch {
    // The level still applies until the overview closes.
  }
}

function rootFont() {
  return parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
}

/** PopupFrame sizes in px at a 16px root. The popup's own font zoom turns that back into CSS px. */
function layoutPx(value: number) {
  return Math.round((value * 16) / rootFont());
}

function roomAxis(raw: string | null, fallback: number) {
  const value = Number(raw);
  return Number.isFinite(value) && value >= 240 ? Math.round(value) : fallback;
}

/** How many cards fit across, and the window width that wraps those columns. */
function gridWidth(count: number, zoom: number, maxWidth: number) {
  const font = rootFont();
  const factor = zoom / 100;
  const card = CARD_REM * font * factor;
  const gap = GAP_REM * font * factor;
  const pad = PAD_REM * font * 2;
  const innerMax = Math.max(card, maxWidth - pad);
  const cols = Math.min(Math.max(count, 1), Math.max(1, Math.floor((innerMax + gap) / (card + gap))));
  const innerRem = cols * CARD_REM + Math.max(0, cols - 1) * GAP_REM;
  const width = Math.min(maxWidth, Math.ceil(innerRem * font * factor + pad));
  return { cols, innerRem, width };
}

function applyZoomChange(current: number, change: "in" | "out" | "reset") {
  const next = change === "reset" ? 100 : stepPageZoom(current, change === "in" ? 1 : -1);
  writeOverviewZoom(next);
  return next;
}

/** Opens the all-tabs grid in the middle of the window. The grid sizes the popup, up to this room.
 * `hold` is the Ctrl+` shortcut: the grid closes when Ctrl is released. A click stays until Escape. */
export function openTabOverview(hold = false) {
  const width = Math.round(Math.max(320, window.innerWidth - WINDOW_MARGIN * 2));
  const height = Math.round(Math.max(240, window.innerHeight - WINDOW_MARGIN * 2));
  const held = hold ? "&hold=1" : "";
  return openPopup("tab-overview", { x: 0, y: 0 }, { placement: "center", params: `maxWidth=${width}&maxHeight=${height}${held}` });
}

/** Every tab in this workspace, as a thumbnail grid. Click or Enter switches; Escape closes. */
export function TabOverview({ params }: TabOverviewProps) {
  const snapshot = useTabs();
  const [selectedId, setSelectedId] = useState<number | null>(snapshot.activeId);
  const [thumbs, setThumbs] = useState<Record<number, string>>({});
  const [zoom, setZoom] = useState(readOverviewZoom);
  const tabsRef = useRef<TabInfo[]>(snapshot.tabs);
  const selectedRef = useRef(selectedId);
  const activeRef = useRef(snapshot.activeId);
  const done = useRef(false);
  tabsRef.current = snapshot.tabs;
  selectedRef.current = selectedId;
  activeRef.current = snapshot.activeId;

  const maxWidth = roomAxis(params.get("maxWidth"), 960);
  const maxHeight = roomAxis(params.get("maxHeight"), 640);
  const hold = params.get("hold") === "1";
  const fitted = gridWidth(snapshot.tabs.length, zoom, maxWidth);

  const tabKey = snapshot.tabs.map((tab) => tab.id).join(",");

  useEffect(() => {
    setSelectedId((current) => {
      if (current !== null && snapshot.tabs.some((tab) => tab.id === current)) {
        return current;
      }
      return snapshot.activeId ?? snapshot.tabs[0]?.id ?? null;
    });
  }, [snapshot.activeId, snapshot.tabs]);

  useEffect(() => {
    if (selectedId === null) {
      return;
    }
    document.getElementById(`tab-overview-${selectedId}`)?.scrollIntoView({ block: "nearest", inline: "nearest" });
  }, [selectedId]);

  useEffect(() => {
    const unlisten = onTabOverviewZoom((change) => {
      setZoom((current) => applyZoomChange(current, change));
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);

  useEffect(() => {
    if (!hold) {
      return;
    }
    let cancelled = false;
    const close = () => {
      if (cancelled || done.current) {
        return;
      }
      done.current = true;
      void closePopup();
    };
    const onKeyUp = (event: KeyboardEvent) => {
      if (event.key === "Control") {
        close();
      }
    };
    window.addEventListener("keyup", onKeyUp);
    // The keyup can land in the page that had focus before this popup, so also ask Windows.
    const timer = window.setInterval(() => {
      void ctrlHeld().then((held) => {
        if (!held) {
          close();
        }
      });
    }, RELEASE_POLL_MS);
    return () => {
      cancelled = true;
      window.removeEventListener("keyup", onKeyUp);
      window.clearInterval(timer);
    };
  }, [hold]);

  useEffect(() => {
    if (!snapshot.loaded) {
      return;
    }
    const ids = tabKey === "" ? [] : tabKey.split(",").map(Number);
    const selected = selectedRef.current;
    if (selected !== null) {
      const at = ids.indexOf(selected);
      if (at > 0) {
        ids.splice(at, 1);
        ids.unshift(selected);
      }
    }
    let cancelled = false;
    let index = 0;
    let active = 0;
    const start = () => {
      while (!cancelled && active < THUMB_BATCH && index < ids.length) {
        const id = ids[index];
        index += 1;
        active += 1;
        void tabThumbnail(id)
          .then((url) => {
            if (!cancelled && url) {
              setThumbs((current) => (current[id] === url ? current : { ...current, [id]: url }));
            }
          })
          .catch(() => undefined)
          .finally(() => {
            active -= 1;
            start();
          });
      }
    };
    start();
    return () => {
      cancelled = true;
    };
  }, [snapshot.loaded, tabKey]);

  const commit = (id: number) => {
    if (done.current) {
      return;
    }
    done.current = true;
    const finish = id !== activeRef.current ? activateTab(id) : Promise.resolve();
    void finish.finally(() => closePopup());
  };
  const commitRef = useRef(commit);
  commitRef.current = commit;

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const tabs = tabsRef.current;
      const count = tabs.length;
      if (event.key === "ArrowRight" || event.key === "ArrowDown") {
        event.preventDefault();
        if (count === 0) {
          return;
        }
        setSelectedId((current) => {
          const index = tabs.findIndex((tab) => tab.id === current);
          return tabs[(index < 0 ? 0 : index + 1) % count].id;
        });
      } else if (event.key === "ArrowLeft" || event.key === "ArrowUp") {
        event.preventDefault();
        if (count === 0) {
          return;
        }
        setSelectedId((current) => {
          const index = tabs.findIndex((tab) => tab.id === current);
          const next = index <= 0 ? count - 1 : index - 1;
          return tabs[next].id;
        });
      } else if (event.key === "Enter") {
        event.preventDefault();
        const id = selectedRef.current;
        if (id !== null) {
          commitRef.current(id);
        }
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  const groups = new Map<number, TabGroup>(snapshot.groups.map((group) => [group.id, group]));

  return (
    <PopupFrame
      id="tab-overview"
      width={layoutPx(fitted.width)}
      maxHeight={layoutPx(maxHeight)}
      ready={snapshot.loaded}
      className="relative flex items-center justify-center p-3"
    >
      {zoom === 100 ? null : (
        <button
          id="tab-overview-zoom"
          type="button"
          title="Reset zoom"
          aria-label={`Tab overview zoom ${zoom} percent. Reset`}
          className="absolute right-2 top-2 z-10 rounded bg-zinc-700 px-1.5 py-0.5 text-[11px] text-zinc-300 hover:bg-zinc-600 hover:text-zinc-100"
          onClick={() => setZoom((current) => applyZoomChange(current, "reset"))}
        >
          {zoom}%
        </button>
      )}
      <div id="tab-overview-body" className="flex w-full items-center justify-center">
        {snapshot.tabs.length === 0 ? (
          <div id="tab-overview-empty" className="px-3 py-6 text-center text-sm text-zinc-500">
            No tabs.
          </div>
        ) : (
          <div
            id="tab-overview-grid"
            role="listbox"
            aria-label="All tabs. Ctrl+scroll zooms."
            className="flex flex-wrap items-center justify-center gap-2"
            style={{ zoom: zoom / 100, width: `${fitted.innerRem}rem` }}
          >
            {snapshot.tabs.map((tab) => {
              const group = tab.groupId === null ? null : (groups.get(tab.groupId) ?? null);
              const selected = tab.id === selectedId;
              return (
                <div
                  id={`tab-overview-${tab.id}`}
                  key={tab.id}
                  role="option"
                  aria-selected={selected}
                  className={cn(
                    "flex cursor-default flex-col overflow-hidden rounded-lg bg-zinc-950 ring-2 transition-colors",
                    selected ? "ring-sky-500" : "ring-transparent hover:ring-zinc-700",
                  )}
                  style={{ width: `${CARD_REM}rem` }}
                  onMouseEnter={() => setSelectedId(tab.id)}
                  onClick={() => commit(tab.id)}
                >
                  {group ? (
                    <div id={`tab-overview-${tab.id}-group`} className={cn("h-1 w-full", groupColorClasses(group.color).line)} />
                  ) : null}
                  <div id={`tab-overview-${tab.id}-thumb`} className="relative aspect-16/10 w-full bg-zinc-800">
                    {thumbs[tab.id] ? (
                      <img id={`tab-overview-${tab.id}-image`} src={thumbs[tab.id]} alt="" className="size-full object-cover object-top" />
                    ) : (
                      <div id={`tab-overview-${tab.id}-placeholder`} className="flex size-full items-center justify-center">
                        <FavIcon id={`tab-overview-${tab.id}-placeholder-icon`} url={tab.url} src={tab.favicon} className="size-6" />
                      </div>
                    )}
                    {tab.sleeping ? (
                      <Moon id={`tab-overview-${tab.id}-asleep`} className="absolute right-1.5 top-1.5 size-3.5 text-zinc-300" aria-label="Asleep" />
                    ) : null}
                  </div>
                  <div id={`tab-overview-${tab.id}-caption`} className="flex items-center gap-1.5 px-2 py-1.5">
                    <FavIcon id={`tab-overview-${tab.id}-icon`} url={tab.url} src={tab.favicon} className="size-3.5" />
                    <span id={`tab-overview-${tab.id}-title`} className="min-w-0 flex-1 truncate text-xs" title={`${tab.title}\n${hostOf(tab.url)}`}>
                      {tab.title || hostOf(tab.url)}
                    </span>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </PopupFrame>
  );
}
