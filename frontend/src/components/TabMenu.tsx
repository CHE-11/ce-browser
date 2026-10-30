import { ArrowRightToLine, Copy, History, Pin, PinOff, Plus, RotateCw, Star, X, XCircle } from "lucide-react";
import {
  addBookmark,
  closeOtherTabs,
  closePopup,
  closeSelectedTabs,
  closeTab,
  closeTabsToRight,
  createTab,
  duplicateTab,
  pinTab,
  reload,
  reopenClosedTab,
  unpinTab,
} from "../lib/browser-client";
import { useTabs } from "../lib/use-browser";
import { PopupFrame, PopupMenu, PopupMenuItem, PopupMenuSeparator } from "./PopupFrame";

type TabMenuProps = {
  tabId: number;
};

export function TabMenu({ tabId }: TabMenuProps) {
  const snapshot = useTabs();
  const index = snapshot.tabs.findIndex((tab) => tab.id === tabId);
  const tab = snapshot.tabs[index];

  const run = (action: () => Promise<unknown>) => () => {
    void action().finally(() => void closePopup());
  };

  const grouped = snapshot.selectedIds.length > 1 && snapshot.selectedIds.includes(tabId);
  const closeLabel = grouped ? `Close ${snapshot.selectedIds.length} tabs` : "Close";

  if (!tab) {
    return (
      <PopupFrame id="tab-menu">
        <div id="tab-menu-missing" className="px-3 py-2 text-sm text-zinc-500">
          That tab is closed.
        </div>
      </PopupFrame>
    );
  }

  return (
    <PopupFrame id="tab-menu">
      <PopupMenu id="tab-menu-items" label="Tab">
        <PopupMenuItem
          id="tab-menu-new-right"
          icon={<Plus />}
          label="New tab to the right"
          onClick={run(() => createTab(undefined, false, index + 1))}
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
          id="tab-menu-bookmark"
          icon={<Star />}
          label="Bookmark tab"
          disabled={tab.url === "ce://newtab"}
          onClick={run(() => addBookmark("bar", tab.title || tab.url, tab.url))}
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
