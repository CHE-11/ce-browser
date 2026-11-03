import { useEffect, useRef, useState } from "react";
import { Moon } from "lucide-react";
import { FavIcon } from "@/components/FavIcon";
import { PopupFrame } from "@/components/PopupFrame";
import { activateTab, closePopup, openPopup, tabThumbnail } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { hostOf } from "@/lib/format";
import { groupColorClasses } from "@/lib/tab-rows";
import { useTabs } from "@/lib/use-browser";
import { zoomWheelStep } from "@/lib/zoom";
import type { TabGroup, TabInfo } from "@type/browser";

const THUMB_BATCH = 4;
/** `p-3` on the frame, in the same units as `PopupFrame`'s width. */
const FRAME_PADDING = 24;
const WINDOW_MARGIN = 32;
const MIN_SCALE = 0.5;
const MAX_SCALE = 1;
const SCALE_STEP = 0.08;
const DEFAULT_SCALE = 1;

type TabOverviewProps = {
  params: URLSearchParams;
};

/** Kept for the session, so the next open uses the size last chosen with Ctrl+scroll. */
let overviewScale = DEFAULT_SCALE;

/** CSS px from the browser window, expressed the way `PopupFrame` sizes its content. */
function layoutPx(value: number) {
  const font = parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
  return Math.round((value * 16) / font);
}

function frameLimit(raw: string | null, fallback: number) {
  const value = Number(raw);
  return Number.isFinite(value) && value >= 240 ? value : fallback;
}

/** Centers the all-tabs grid and gives it nearly the whole window to grow into. */
export function openTabOverview() {
  const width = layoutPx(Math.max(640, window.innerWidth - WINDOW_MARGIN * 2));
  const height = layoutPx(Math.max(420, window.innerHeight - WINDOW_MARGIN * 2));
  return openPopup("tab-overview", { x: 0, y: 0 }, { placement: "center", params: `width=${width}&height=${height}` });
}

/** Every tab in this workspace, as a thumbnail grid. Click or Enter switches; Escape closes. */
export function TabOverview({ params }: TabOverviewProps) {
  const snapshot = useTabs();
  const [selectedId, setSelectedId] = useState<number | null>(snapshot.activeId);
  const [thumbs, setThumbs] = useState<Record<number, string>>({});
  const [scale, setScale] = useState(overviewScale);
  const zoomBucket = useRef(0);
  const tabsRef = useRef<TabInfo[]>(snapshot.tabs);
  const selectedRef = useRef(selectedId);
  const activeRef = useRef(snapshot.activeId);
  const done = useRef(false);
  tabsRef.current = snapshot.tabs;
  selectedRef.current = selectedId;
  activeRef.current = snapshot.activeId;

  const room = {
    width: frameLimit(params.get("width"), 880),
    height: frameLimit(params.get("height"), 560),
  };
  const size = {
    width: Math.round(room.width * scale),
    height: Math.round(room.height * scale),
  };

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
    const onWheel = (event: WheelEvent) => {
      if (!event.ctrlKey) {
        zoomBucket.current = 0;
        return;
      }
      event.preventDefault();
      const stepped = zoomWheelStep(zoomBucket.current, event.deltaY, event.deltaMode);
      zoomBucket.current = stepped.bucket;
      if (!stepped.change) {
        return;
      }
      setScale((current) => {
        const next = Math.min(MAX_SCALE, Math.max(MIN_SCALE, current + (stepped.change === "in" ? SCALE_STEP : -SCALE_STEP)));
        const rounded = Math.round(next * 100) / 100;
        overviewScale = rounded;
        return rounded;
      });
    };
    window.addEventListener("wheel", onWheel, { passive: false });
    return () => window.removeEventListener("wheel", onWheel);
  }, []);

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
    <PopupFrame id="tab-overview" width={size.width} maxHeight={size.height} ready={snapshot.loaded} className="flex flex-col p-3">
      <div id="tab-overview-body" className="flex flex-1 flex-col" style={{ minHeight: `${(size.height - FRAME_PADDING) / 16}rem` }}>
        {snapshot.tabs.length === 0 ? (
          <div id="tab-overview-empty" className="px-3 py-6 text-center text-sm text-zinc-500">
            No tabs.
          </div>
        ) : (
          <div id="tab-overview-grid" role="listbox" aria-label="All tabs. Ctrl+scroll changes the size." className="grid grid-cols-4 content-start gap-2">
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
