import { Moon, Volume2, VolumeX } from "lucide-react";
import { activateTab, closePopup } from "../../../lib/browser-client";
import { cn } from "../../../lib/cn";
import { tabProblemTitle, useHeavyTabs } from "../../../lib/tab-alert";
import { groupColorClasses, groupLabel } from "../../../lib/tab-rows";
import { useTabs } from "../../../lib/use-browser";
import { FavIcon } from "../../../components/FavIcon";
import { PopupFrame } from "../../../components/PopupFrame";
import { TabAlertDot } from "./TabAlertDot";

type TabGroupFlyoutProps = {
  groupId: number;
};

/** Hovering a collapsed group lists its tabs. Opens without focus and closes when the mouse leaves. */
export function TabGroupFlyout({ groupId }: TabGroupFlyoutProps) {
  const snapshot = useTabs();
  const heavy = useHeavyTabs();
  const group = snapshot.groups.find((item) => item.id === groupId);
  const tabs = snapshot.tabs.filter((tab) => tab.groupId === groupId);

  return (
    <div id="tab-group-flyout-hover" className="h-full" onMouseLeave={() => void closePopup()}>
      <PopupFrame id="tab-group-flyout" ready={snapshot.loaded}>
        {group ? (
          <div id="tab-group-flyout-body" className="flex flex-col gap-0.5 p-1.5">
            <div id="tab-group-flyout-title" className="flex items-center gap-2 px-2 pb-1 pt-0.5 text-xs font-medium text-zinc-400">
              <span id="tab-group-flyout-swatch" className={cn("size-2 rounded-full", groupColorClasses(group.color).swatch)} />
              <span id="tab-group-flyout-name" className="truncate">
                {groupLabel(group, tabs.length)}
              </span>
            </div>
            {tabs.map((tab) => {
              const problem = tabProblemTitle(tab, heavy.get(tab.id) ?? null);
              return (
                <button
                  key={tab.id}
                  id={`tab-group-flyout-tab-${tab.id}`}
                  type="button"
                  title={problem ?? undefined}
                  aria-label={problem ? `${tab.title || tab.url}, ${problem}` : undefined}
                  className={cn(
                    "flex h-8 w-full items-center gap-2 rounded-lg px-2 text-left text-sm text-zinc-200 hover:bg-zinc-800",
                    tab.id === snapshot.activeId && "bg-zinc-800",
                    tab.sleeping && "opacity-60",
                  )}
                  onClick={() => {
                    void activateTab(tab.id).finally(() => void closePopup());
                  }}
                >
                  <span id={`tab-group-flyout-tab-${tab.id}-icon-wrap`} className="relative inline-flex shrink-0">
                    <FavIcon id={`tab-group-flyout-tab-${tab.id}-icon`} url={tab.url} src={tab.favicon} />
                    {problem ? (
                      <TabAlertDot id={`tab-group-flyout-tab-${tab.id}-alert`} title={problem} className="-right-0.5 -top-0.5 ring-zinc-900" />
                    ) : null}
                  </span>
                  <span id={`tab-group-flyout-tab-${tab.id}-title`} className="min-w-0 flex-1 truncate">
                    {tab.title || tab.url}
                  </span>
                  {tab.sleeping ? <Moon id={`tab-group-flyout-tab-${tab.id}-sleeping`} className="size-3 text-zinc-500" aria-hidden /> : null}
                  {tab.muted ? (
                    <VolumeX id={`tab-group-flyout-tab-${tab.id}-muted`} className="size-3.5 text-zinc-500" aria-hidden />
                  ) : tab.audible ? (
                    <Volume2 id={`tab-group-flyout-tab-${tab.id}-audible`} className="size-3.5 text-sky-400" aria-hidden />
                  ) : null}
                </button>
              );
            })}
          </div>
        ) : (
          <div id="tab-group-flyout-missing" className="px-3 py-2 text-sm text-zinc-500">
            That group is gone.
          </div>
        )}
      </PopupFrame>
    </div>
  );
}
