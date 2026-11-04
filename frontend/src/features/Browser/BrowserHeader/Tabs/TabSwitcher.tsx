import { useEffect, useMemo, useRef, useState } from "react";
import { Moon } from "lucide-react";
import { FavIcon } from "@/components/FavIcon";
import { PopupFrame } from "@/components/PopupFrame";
import { activateTab, closePopup, ctrlHeld, onTabSwitcherStep, tabThumbnail } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { hostOf } from "@/lib/format";
import { useTabs } from "@/lib/use-browser";
import type { TabInfo } from "@type/browser";

const RELEASE_POLL_MS = 60;
const MAX_TABS = 12;

type TabSwitcherProps = {
  params: URLSearchParams;
};

/** Ctrl+` or Ctrl+Tab overlay: tabs in recent-use order with thumbnails. Releasing Ctrl selects the highlighted tab. */
export function TabSwitcher({ params }: TabSwitcherProps) {
  const snapshot = useTabs();
  const firstStep = Number(params.get("step")) < 0 ? -1 : 1;
  const [order, setOrder] = useState<TabInfo[] | null>(null);
  const [selected, setSelected] = useState(0);
  const [thumbs, setThumbs] = useState<Record<number, string>>({});
  const selectedRef = useRef(0);
  const orderRef = useRef<TabInfo[]>([]);
  const done = useRef(false);

  useEffect(() => {
    if (order !== null || !snapshot.loaded) {
      return;
    }
    const sorted = [...snapshot.tabs]
      .sort((left, right) => Number(right.id === snapshot.activeId) - Number(left.id === snapshot.activeId) || right.lastActive - left.lastActive)
      .slice(0, MAX_TABS);
    setOrder(sorted);
    const start = sorted.length < 2 ? 0 : firstStep > 0 ? 1 : sorted.length - 1;
    setSelected(start);
  }, [firstStep, order, snapshot.activeId, snapshot.loaded, snapshot.tabs]);

  const activeRef = useRef<number | null>(null);
  selectedRef.current = selected;
  orderRef.current = order ?? [];
  activeRef.current = snapshot.activeId;

  const commit = (index = selectedRef.current) => {
    if (done.current) {
      return;
    }
    done.current = true;
    const tab = orderRef.current[index];
    const finish = tab && tab.id !== activeRef.current ? activateTab(tab.id) : Promise.resolve();
    void finish.finally(() => closePopup());
  };

  useEffect(() => {
    if (!order) {
      return;
    }
    let cancelled = false;
    for (const tab of order) {
      void tabThumbnail(tab.id)
        .then((url) => {
          if (!cancelled && url) {
            setThumbs((current) => ({ ...current, [tab.id]: url }));
          }
        })
        .catch(() => undefined);
    }
    return () => {
      cancelled = true;
    };
  }, [order]);

  useEffect(() => {
    const unlisten = onTabSwitcherStep((step) => {
      setSelected((index) => {
        const count = orderRef.current.length;
        return count === 0 ? 0 : (index + step + count) % count;
      });
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);

  useEffect(() => {
    const onKeyUp = (event: KeyboardEvent) => {
      if (event.key === "Control") {
        commit();
      }
    };
    const onKeyDown = (event: KeyboardEvent) => {
      const count = orderRef.current.length;
      if (event.key === "Escape") {
        done.current = true;
        void closePopup();
      } else if (event.key === "Enter") {
        commit();
      } else if (event.key === "ArrowRight" || event.key === "ArrowDown") {
        setSelected((index) => (count === 0 ? 0 : (index + 1) % count));
      } else if (event.key === "ArrowLeft" || event.key === "ArrowUp") {
        setSelected((index) => (count === 0 ? 0 : (index - 1 + count) % count));
      }
    };
    window.addEventListener("keyup", onKeyUp);
    window.addEventListener("keydown", onKeyDown);
    // The keyup can land in another view before this one has focus, so also ask Windows.
    const timer = window.setInterval(() => {
      void ctrlHeld().then((held) => {
        if (!held && orderRef.current.length > 0) {
          commit();
        }
      });
    }, RELEASE_POLL_MS);
    return () => {
      window.removeEventListener("keyup", onKeyUp);
      window.removeEventListener("keydown", onKeyDown);
      window.clearInterval(timer);
    };
  }, []);

  const cards = useMemo(() => order ?? [], [order]);

  return (
    <PopupFrame id="tab-switcher" width={760} maxHeight={440} ready={order !== null} className="p-3">
      <div id="tab-switcher-grid" role="listbox" aria-label="Recent tabs" className="grid grid-cols-4 content-start gap-2">
        {cards.map((tab, index) => (
          <div
            id={`tab-switcher-${tab.id}`}
            key={tab.id}
            role="option"
            aria-selected={index === selected}
            className={cn(
              "flex cursor-default flex-col overflow-hidden rounded-lg bg-zinc-950 ring-2 transition-colors",
              index === selected ? "ring-sky-500" : "ring-transparent hover:ring-zinc-700",
            )}
            onMouseEnter={() => setSelected(index)}
            onClick={() => commit(index)}
          >
            <div id={`tab-switcher-${tab.id}-thumb`} className="relative aspect-16/10 w-full bg-zinc-800">
              {thumbs[tab.id] ? (
                <img id={`tab-switcher-${tab.id}-image`} src={thumbs[tab.id]} alt="" className="size-full object-cover object-top" />
              ) : (
                <div id={`tab-switcher-${tab.id}-placeholder`} className="flex size-full items-center justify-center">
                  <FavIcon id={`tab-switcher-${tab.id}-placeholder-icon`} url={tab.url} src={tab.favicon} className="size-6" />
                </div>
              )}
              {tab.sleeping ? (
                <Moon id={`tab-switcher-${tab.id}-asleep`} className="absolute right-1.5 top-1.5 size-3.5 text-zinc-300" aria-label="Asleep" />
              ) : null}
            </div>
            <div id={`tab-switcher-${tab.id}-caption`} className="flex items-center gap-1.5 px-2 py-1.5">
              <FavIcon id={`tab-switcher-${tab.id}-icon`} url={tab.url} src={tab.favicon} className="size-3.5" />
              <span id={`tab-switcher-${tab.id}-title`} className="min-w-0 flex-1 truncate text-xs" title={`${tab.title}\n${hostOf(tab.url)}`}>
                {tab.title || hostOf(tab.url)}
              </span>
            </div>
          </div>
        ))}
      </div>
    </PopupFrame>
  );
}
