import { useEffect, useState } from "react";
import { PopupFrame, PopupMenu, PopupMenuItem, PopupMenuSeparator } from "@/components/PopupFrame";
import { closePopup, createEphemeralTab, createWorkspace, moveTabsToWorkspace, createTab, reload, duplicateTab, unpinTab, pinTab, setTabsMuted, addBookmark, exitSplit, splitWith, removeFromGroup, createGroup, addToGroup, sleepTab, sleepOtherTabs, neverSleepSite, closeSelectedTabs, closeTab, closeOtherTabs, closeTabsToRight, reopenClosedTab, installedBrowsers, openInBrowser } from "@/lib/browser-client";
import type { ExternalBrowser } from "@type/browser";
import { cn } from "@/lib/cn";
import { groupColorClasses, GROUP_COLORS, groupLabel } from "@/lib/tab-rows";
import { useTabs } from "@/lib/use-browser";
import {
  AppWindow,
  ArrowRightToLine,
  Columns2,
  Copy,
  FolderMinus,
  FolderPlus,
  History,
  Layers,
  Moon,
  MoonStar,
  Pin,
  PinOff,
  Glasses,
  Plus,
  RotateCw,
  Star,
  SunMedium,
  Volume2,
  VolumeX,
  X,
  XCircle,
} from "lucide-react";


type TabMenuProps = {
  tabId: number;
};

function Swatch({ id, color }: { id: string; color: string }) {
  return <span id={id} className={cn("size-2.5 rounded-full", groupColorClasses(color).swatch)} />;
}

export function TabMenu({ tabId }: TabMenuProps) {
  const snapshot = useTabs();
  const [browsers, setBrowsers] = useState<ExternalBrowser[]>([]);
  useEffect(() => {
    void installedBrowsers().then(setBrowsers).catch(() => setBrowsers([]));
  }, []);
  const index = snapshot.tabs.findIndex((tab) => tab.id === tabId);
  const tab = snapshot.tabs[index];

  const run = (action: () => Promise<unknown>) => () => {
    void action().finally(() => void closePopup());
  };

  const grouped = snapshot.selectedIds.length > 1 && snapshot.selectedIds.includes(tabId);
  const targets = grouped ? snapshot.selectedIds : [tabId];
  const closeLabel = grouped ? `Close ${snapshot.selectedIds.length} tabs` : "Close";

  if (!tab) {
    return (
      <PopupFrame id="tab-menu" ready={snapshot.loaded}>
        <div id="tab-menu-missing" className="px-3 py-2 text-sm text-zinc-500">
          That tab is closed.
        </div>
      </PopupFrame>
    );
  }

  const targetTabs = snapshot.tabs.filter((item) => targets.includes(item.id));
  const allMuted = targetTabs.length > 0 && targetTabs.every((item) => item.muted);
  const split = snapshot.split;
  const inSplit = !!split && (split.left === tab.id || split.right === tab.id);
  const active = snapshot.tabs.find((item) => item.id === snapshot.activeId);
  const canSplit = !inSplit && !!active && active.id !== tab.id && !active.pinned && !tab.pinned;
  const foreground =
    tab.id === snapshot.activeId ||
    (!!split && inSplit && (snapshot.activeId === split.left || snapshot.activeId === split.right));
  const web = tab.url.startsWith("http://") || tab.url.startsWith("https://");
  const otherGroups = snapshot.groups.filter((group) => group.id !== tab.groupId);
  const otherWorkspaces = snapshot.workspaces.filter((workspace) => workspace.id !== snapshot.workspace);
  const tabsIn = (groupId: number) => snapshot.tabs.filter((item) => item.groupId === groupId).length;

  const moveToNewWorkspace = async () => {
    const used = new Set(snapshot.workspaces.map((workspace) => workspace.color));
    const color = GROUP_COLORS.find((item) => !used.has(item)) ?? "blue";
    const id = await createWorkspace("", color);
    await moveTabsToWorkspace(targets, id);
  };

  return (
    <PopupFrame id="tab-menu" ready={snapshot.loaded}>
      <PopupMenu id="tab-menu-items" label="Tab">
        <PopupMenuItem
          id="tab-menu-new-right"
          icon={<Plus />}
          label="New tab to the right"
          onClick={run(() => createTab(undefined, false, index + 1))}
        />
        <PopupMenuItem
          id="tab-menu-ephemeral"
          icon={<Glasses />}
          label="New ephemeral tab"
          onClick={run(() => createEphemeralTab())}
        />
        <PopupMenuSeparator id="tab-menu-separator-1" />
        <PopupMenuItem id="tab-menu-reload" icon={<RotateCw />} label="Reload" shortcut="Ctrl+R" onClick={run(() => reload(tab.id))} />
        <PopupMenuItem id="tab-menu-duplicate" icon={<Copy />} label="Duplicate" onClick={run(() => duplicateTab(tab.id))} />
        {tab.pinned ? (
          <PopupMenuItem id="tab-menu-unpin" icon={<PinOff />} label="Unpin" onClick={run(() => unpinTab(tab.id))} />
        ) : (
          <PopupMenuItem id="tab-menu-pin" icon={<Pin />} label="Pin" onClick={run(() => pinTab(tab.id))} />
        )}
        <PopupMenuItem
          id="tab-menu-mute"
          icon={allMuted ? <Volume2 /> : <VolumeX />}
          label={allMuted ? (grouped ? "Unmute tabs" : "Unmute tab") : grouped ? "Mute tabs" : "Mute tab"}
          onClick={run(() => setTabsMuted(targets, !allMuted))}
        />
        <PopupMenuItem
          id="tab-menu-bookmark"
          icon={<Star />}
          label="Bookmark tab"
          disabled={tab.url === "ce://newtab"}
          onClick={run(() => addBookmark("bar", tab.title || tab.url, tab.url))}
        />
        {browsers.length > 0 && (tab.url.startsWith("http://") || tab.url.startsWith("https://")) ? (
          <>
            {browsers.map((browser) => (
              <PopupMenuItem
                id={`tab-menu-browser-${browser.id}`}
                key={browser.id}
                icon={<AppWindow />}
                label={`Open in ${browser.name}`}
                onClick={run(() => openInBrowser(browser.id, tab.url))}
              />
            ))}
          </>
        ) : null}
        <PopupMenuSeparator id="tab-menu-separator-split" />
        {inSplit ? (
          <PopupMenuItem id="tab-menu-exit-split" icon={<Columns2 />} label="Exit split view" onClick={run(() => exitSplit())} />
        ) : (
          <PopupMenuItem
            id="tab-menu-split"
            icon={<Columns2 />}
            label="Split with current tab"
            disabled={!canSplit}
            onClick={run(() => splitWith(tab.id))}
          />
        )}
        {tab.groupId !== null ? (
          <PopupMenuItem
            id="tab-menu-ungroup"
            icon={<FolderMinus />}
            label={grouped ? "Remove tabs from group" : "Remove from group"}
            onClick={run(() => removeFromGroup(targets))}
          />
        ) : null}
        <PopupMenuItem
          id="tab-menu-new-group"
          icon={<FolderPlus />}
          label={grouped ? "Add tabs to new group" : "Add to new group"}
          disabled={targetTabs.every((item) => item.pinned)}
          onClick={run(() => createGroup(targets))}
        />
        {otherGroups.map((group) => (
          <PopupMenuItem
            key={group.id}
            id={`tab-menu-group-${group.id}`}
            icon={<Swatch id={`tab-menu-group-${group.id}-swatch`} color={group.color} />}
            label={`Add to ${groupLabel(group, tabsIn(group.id))}`}
            disabled={tab.pinned}
            onClick={run(() => addToGroup(targets, group.id))}
          />
        ))}
        {otherWorkspaces.map((workspace) => (
          <PopupMenuItem
            key={workspace.id}
            id={`tab-menu-workspace-${workspace.id}`}
            icon={<Swatch id={`tab-menu-workspace-${workspace.id}-swatch`} color={workspace.color} />}
            label={`Move to ${workspace.name}`}
            onClick={run(() => moveTabsToWorkspace(targets, workspace.id))}
          />
        ))}
        <PopupMenuItem
          id="tab-menu-workspace-new"
          icon={<Layers />}
          label={grouped ? "Move tabs to new workspace" : "Move to new workspace"}
          onClick={run(moveToNewWorkspace)}
        />
        <PopupMenuSeparator id="tab-menu-separator-sleep" />
        <PopupMenuItem
          id="tab-menu-sleep"
          icon={<Moon />}
          label="Sleep tab"
          disabled={foreground || tab.sleeping || !tab.loaded}
          onClick={run(() => sleepTab(tab.id))}
        />
        <PopupMenuItem id="tab-menu-sleep-others" icon={<MoonStar />} label="Sleep other tabs" onClick={run(() => sleepOtherTabs(tab.id))} />
        <PopupMenuItem
          id="tab-menu-never-sleep"
          icon={<SunMedium />}
          label="Never sleep on this site"
          disabled={!web}
          onClick={run(() => neverSleepSite(tab.id))}
        />
        <PopupMenuSeparator id="tab-menu-separator-2" />
        <PopupMenuItem
          id="tab-menu-close"
          icon={<X />}
          label={closeLabel}
          shortcut="Ctrl+W"
          onClick={run(() => (grouped ? closeSelectedTabs() : closeTab(tab.id)))}
        />
        <PopupMenuItem
          id="tab-menu-close-others"
          icon={<XCircle />}
          label="Close other tabs"
          disabled={snapshot.tabs.length < 2}
          onClick={run(() => closeOtherTabs(tab.id))}
        />
        <PopupMenuItem
          id="tab-menu-close-right"
          icon={<ArrowRightToLine />}
          label="Close tabs to the right"
          disabled={index >= snapshot.tabs.length - 1}
          onClick={run(() => closeTabsToRight(tab.id))}
        />
        <PopupMenuSeparator id="tab-menu-separator-3" />
        <PopupMenuItem
          id="tab-menu-reopen"
          icon={<History />}
          label="Reopen closed tab"
          shortcut="Ctrl+Shift+T"
          disabled={!snapshot.canReopen}
          onClick={run(() => reopenClosedTab())}
        />
      </PopupMenu>
    </PopupFrame>
  );
}
