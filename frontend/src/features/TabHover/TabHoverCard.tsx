import { useEffect, useState } from "react";
import { Glasses, Moon, Volume2 } from "lucide-react";
import { FavIcon } from "@/components/FavIcon";
import { PopupFrame } from "@/components/PopupFrame";
import { closePopup, tabMemory, tabThumbnail } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { formatBytes, hostOf } from "@/lib/format";
import { HIGH_TAB_MEMORY, tabProblemTitle } from "@/lib/tab-alert";
import { groupColorClasses, groupLabel } from "@/lib/tab-rows";
import { useTabs } from "@/lib/use-browser";

type TabHoverCardProps = {
  tabId: number;
};

/** Title, address, a thumbnail, memory and sleep state for the tab under the pointer. */
export function TabHoverCard({ tabId }: TabHoverCardProps) {
  const snapshot = useTabs();
  const tab = snapshot.windowTabs.find((item) => item.id === tabId);
  const group = tab?.groupId != null ? snapshot.groups.find((item) => item.id === tab.groupId) : undefined;
  const [thumbnail, setThumbnail] = useState<string | null | undefined>(undefined);
  const [memory, setMemory] = useState<number | null>(null);
  const sleeping = tab?.sleeping ?? false;
  const loaded = tab?.loaded ?? false;

  useEffect(() => {
    let cancelled = false;
    void tabThumbnail(tabId)
      .then((url) => {
        if (!cancelled) {
          setThumbnail(url);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setThumbnail(null);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [tabId]);

  useEffect(() => {
    if (sleeping || !loaded) {
      setMemory(null);
      return;
    }
    let cancelled = false;
    void tabMemory(tabId)
      .then((bytes) => {
        if (!cancelled) {
          setMemory(bytes);
        }
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [tabId, sleeping, loaded]);

  const missing = snapshot.loaded && !tab;
  useEffect(() => {
    if (missing) {
      void closePopup();
    }
  }, [missing]);

  if (!tab) {
    return <PopupFrame id="tab-hover" ready={false}>{null}</PopupFrame>;
  }

  const heavy = memory !== null && memory >= HIGH_TAB_MEMORY;
  const problem = tabProblemTitle(tab, null);
  const state = sleeping
    ? tab.sleepSaved > 0
      ? `Asleep, about ${formatBytes(tab.sleepSaved)} saved`
      : "Asleep"
    : !loaded
      ? "Not loaded yet"
      : memory !== null
        ? `Memory ${formatBytes(memory)}`
        : null;

  return (
    <PopupFrame id="tab-hover" ready={snapshot.loaded && thumbnail !== undefined}>
      <div id="tab-hover-body" className="flex flex-col gap-2 p-3">
        <div id="tab-hover-head" className="flex items-start gap-2">
          <FavIcon id="tab-hover-icon" url={tab.url} src={tab.favicon} className="mt-0.5" />
          <div id="tab-hover-text" className="min-w-0 flex-1">
            <p id="tab-hover-title" className="line-clamp-2 text-sm font-medium leading-snug text-zinc-100">
              {tab.title || tab.url}
            </p>
            <p id="tab-hover-url" className="truncate text-xs text-zinc-500" title={tab.url}>
              {tab.url.startsWith("ce://") ? tab.url : hostOf(tab.url)}
            </p>
          </div>
        </div>
        {thumbnail ? (
          <img
            id="tab-hover-thumbnail"
            src={thumbnail}
            alt=""
            className={cn("aspect-[16/10] w-full rounded-md bg-zinc-800 object-cover object-top", sleeping && "opacity-60 grayscale")}
          />
        ) : null}
        <div id="tab-hover-meta" className="flex flex-wrap items-center gap-1.5 text-[11px] text-zinc-400">
          {group ? (
            <span id="tab-hover-group" className={cn("inline-flex items-center gap-1 rounded px-1.5 py-0.5", groupColorClasses(group.color).chip)}>
              {groupLabel(group, snapshot.windowTabs.filter((item) => item.groupId === group.id).length)}
            </span>
          ) : null}
          {sleeping ? <Moon id="tab-hover-asleep-icon" className="size-3" aria-hidden /> : null}
          {state ? (
            <span id="tab-hover-state" className={heavy ? "text-red-400" : undefined}>
              {state}
            </span>
          ) : null}
          {problem ? (
            <span id="tab-hover-problem" className="text-red-400">
              {problem}
            </span>
          ) : null}
          {tab.audible ? <Volume2 id="tab-hover-audible" className="size-3 text-sky-400" aria-label="Playing sound" /> : null}
          {tab.ephemeral ? <Glasses id="tab-hover-ephemeral" className="size-3 text-violet-300" aria-label="Ephemeral tab" /> : null}
        </div>
      </div>
    </PopupFrame>
  );
}
