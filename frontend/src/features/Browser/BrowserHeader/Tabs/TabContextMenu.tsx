import { useEffect, useState, type ComponentProps } from "react";
import { PopupFrame, PopupMenu, PopupMenuItem, PopupMenuSeparator } from "@/components/PopupFrame";
import { bookmarkItemStyle } from "@/lib/bookmark-size";
import { CHROME, metric } from "@/lib/chrome-metrics";
import { TAB_SIZE } from "@/lib/tab-size";
import { closePopup, createEphemeralTab, createWorkspace, moveTabsToWorkspace, createTab, reload, duplicateTab, unpinTab, pinTab, setTabsMuted, addBookmark, detachTab, exitSplit, splitWith, removeFromGroup, createGroup, addToGroup, sleepTab, sleepOtherTabs, neverSleepSite, closeSelectedTabs, closeTab, closeOtherTabs, closeTabsToRight, reopenClosedTab, installedBrowsers, installSite, openInBrowser, groupBySite } from "@/lib/browser-client";
import type { ExternalBrowser } from "@type/browser";
import { cn } from "@/lib/cn";
import { groupColorClasses, GROUP_COLORS, groupLabel } from "@/lib/tab-rows";
import { useSettings, useTabs } from "@/lib/use-browser";
import {
  AppWindow,
  AppWindowMac,
  ArrowRightToLine,
  Columns2,
  Copy,
  FolderMinus,
  FolderPlus,
  Group,
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

function TabMenuItem(props: Omit<ComponentProps<typeof PopupMenuItem>, "style">) {
  const settings = useSettings();
  return (
    <PopupMenuItem
      {...props}
      style={bookmarkItemStyle(
        settings?.tabMenuTextSize,
        settings?.tabMenuPadding,
        { text: TAB_SIZE.menuText, padding: TAB_SIZE.menuPadding },
        {
          height: metric(settings?.tabMenuHeight, CHROME.tabMenuHeight),
          icon: metric(settings?.tabMenuIconSize, CHROME.tabMenuIconSize),
        },
      )}
    />
  );
}

export function TabContextMenu({ tabId }: TabMenuProps) {
  const snapshot = useTabs();
  const settings = useSettings();
  const workspacesEnabled = settings?.workspacesEnabled !== false;
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
      <PopupFrame id="tab-menu" width={metric(settings?.tabMenuWidth, CHROME.tabMenuWidth)} ready={snapshot.loaded}>
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
    <PopupFrame id="tab-menu" width={metric(settings?.tabMenuWidth, CHROME.tabMenuWidth)} ready={snapshot.loaded}>
      <PopupMenu id="tab-menu-items" label="Tab">
        <TabMenuItem
          id="tab-menu-new-right"
          icon={<Plus />}
          label="New tab to the right"
          onClick={run(() => createTab(undefined, false, index + 1))}
        />
        <TabMenuItem
          id="tab-menu-ephemeral"
          icon={<Glasses />}
          label="New ephemeral tab"
          onClick={run(() => createEphemeralTab())}
        />
        <PopupMenuSeparator id="tab-menu-separator-1" />
        <TabMenuItem id="tab-menu-reload" icon={<RotateCw />} label="Reload" shortcut="Ctrl+R" onClick={run(() => reload(tab.id))} />
        <TabMenuItem id="tab-menu-duplicate" icon={<Copy />} label="Duplicate" onClick={run(() => duplicateTab(tab.id))} />
        {tab.pinned ? (
          <TabMenuItem id="tab-menu-unpin" icon={<PinOff />} label="Unpin" onClick={run(() => unpinTab(tab.id))} />
        ) : (
          <TabMenuItem id="tab-menu-pin" icon={<Pin />} label="Pin" onClick={run(() => pinTab(tab.id))} />
        )}
        <TabMenuItem
          id="tab-menu-mute"
          icon={allMuted ? <Volume2 /> : <VolumeX />}
          label={allMuted ? (grouped ? "Unmute tabs" : "Unmute tab") : grouped ? "Mute tabs" : "Mute tab"}
          onClick={run(() => setTabsMuted(targets, !allMuted))}
        />
        {(tab.url.startsWith("http://") || tab.url.startsWith("https://")) ? (
          <TabMenuItem
            id="tab-menu-install-site"
            icon={<AppWindowMac />}
            label="Install site"
            onClick={run(() => installSite(tab.id))}
          />
        ) : null}
        <TabMenuItem
          id="tab-menu-bookmark"
          icon={<Star />}
          label="Bookmark tab"
          disabled={tab.url === "ce://newtab"}
          onClick={run(() => addBookmark("bar", tab.title || tab.url, tab.url))}
        />
        {browsers.length > 0 && (tab.url.startsWith("http://") || tab.url.startsWith("https://")) ? (
          <>
            {browsers.map((browser) => (
              <TabMenuItem
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
          <TabMenuItem id="tab-menu-exit-split" icon={<Columns2 />} label="Exit split view" onClick={run(() => exitSplit())} />
        ) : (
          <TabMenuItem
            id="tab-menu-split"
            icon={<Columns2 />}
            label="Split with current tab"
            disabled={!canSplit}
            onClick={run(() => splitWith(tab.id))}
          />
        )}
        <TabMenuItem
          id="tab-menu-detach"
          icon={<AppWindowMac />}
          label="Move to new window"
          disabled={snapshot.windowTabs.length < 2}
          onClick={run(() => detachTab(tab.id))}
        />
        {tab.groupId !== null ? (
          <TabMenuItem
            id="tab-menu-ungroup"
            icon={<FolderMinus />}
            label={grouped ? "Remove tabs from group" : "Remove from group"}
            onClick={run(() => removeFromGroup(targets))}
          />
        ) : null}
        <TabMenuItem
          id="tab-menu-new-group"
          icon={<FolderPlus />}
          label={grouped ? "Add tabs to new group" : "Add to new group"}
          disabled={targetTabs.every((item) => item.pinned)}
          onClick={run(() => createGroup(targets))}
        />
        {otherGroups.map((group) => (
          <TabMenuItem
            key={group.id}
            id={`tab-menu-group-${group.id}`}
            icon={<Swatch id={`tab-menu-group-${group.id}-swatch`} color={group.color} />}
            label={`Add to ${groupLabel(group, tabsIn(group.id))}`}
            disabled={tab.pinned}
            onClick={run(() => addToGroup(targets, group.id))}
          />
        ))}
        <TabMenuItem
          id="tab-menu-group-by-site"
          icon={<Group />}
          label="Group tabs by site"
          title="Loose tabs that share a site go into one group named after it"
          onClick={run(() => groupBySite())}
        />
        {workspacesEnabled
          ? otherWorkspaces.map((workspace) => (
              <TabMenuItem
                key={workspace.id}
                id={`tab-menu-workspace-${workspace.id}`}
                icon={<Swatch id={`tab-menu-workspace-${workspace.id}-swatch`} color={workspace.color} />}
                label={`Move to ${workspace.name}`}
                onClick={run(() => moveTabsToWorkspace(targets, workspace.id))}
              />
            ))
          : null}
        {workspacesEnabled ? (
          <TabMenuItem
            id="tab-menu-workspace-new"
            icon={<Layers />}
            label={grouped ? "Move tabs to new workspace" : "Move to new workspace"}
            onClick={run(moveToNewWorkspace)}
          />
        ) : null}
        <PopupMenuSeparator id="tab-menu-separator-sleep" />
        <TabMenuItem
          id="tab-menu-sleep"
          icon={<Moon />}
          label="Sleep tab"
          title={tab.pinned ? "Pinned tabs stay awake" : undefined}
          disabled={foreground || tab.pinned || tab.sleeping || !tab.loaded}
          onClick={run(() => sleepTab(tab.id))}
        />
        <TabMenuItem id="tab-menu-sleep-others" icon={<MoonStar />} label="Sleep other tabs" onClick={run(() => sleepOtherTabs(tab.id))} />
        <TabMenuItem
          id="tab-menu-never-sleep"
          icon={<SunMedium />}
          label="Never sleep on this site"
          disabled={!web}
          onClick={run(() => neverSleepSite(tab.id))}
        />
        <PopupMenuSeparator id="tab-menu-separator-2" />
        <TabMenuItem
          id="tab-menu-close"
          icon={<X />}
          label={closeLabel}
          shortcut="Ctrl+W"
          onClick={run(() => (grouped ? closeSelectedTabs() : closeTab(tab.id)))}
        />
        <TabMenuItem
          id="tab-menu-close-others"
          icon={<XCircle />}
          label="Close other tabs"
          disabled={snapshot.tabs.length < 2}
          onClick={run(() => closeOtherTabs(tab.id))}
        />
        <TabMenuItem
          id="tab-menu-close-right"
          icon={<ArrowRightToLine />}
          label="Close tabs to the right"
          disabled={index >= snapshot.tabs.length - 1}
          onClick={run(() => closeTabsToRight(tab.id))}
        />
        <PopupMenuSeparator id="tab-menu-separator-3" />
        <TabMenuItem
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
