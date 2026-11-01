import {
  Bookmark,
  Code,
  Download,
  Glasses,
  History,
  LogOut,
  Minus,
  PanelLeft,
  PanelTop,
  Plus,
  Puzzle,
  Settings,
  SquarePlus,
  ZoomIn,
} from "lucide-react";
import {
  closePopup,
  closeWindow,
  createEphemeralTab,
  createTab,
  getSettings,
  openDevtools,
  reopenClosedTab,
  saveSettings,
  zoomTab,
} from "../lib/browser-client";
import { useTabs } from "../lib/use-browser";
import { IconButton } from "./IconButton";
import { PopupFrame, PopupMenu, PopupMenuItem, PopupMenuSeparator } from "./PopupFrame";

export function AppMenu() {
  const snapshot = useTabs();
  const active = snapshot.tabs.find((tab) => tab.id === snapshot.activeId);

  const run = (action: () => Promise<unknown>) => () => {
    void action().finally(() => void closePopup());
  };

  const openPage = (url: string) => run(() => createTab(url));

  const toggleBookmarksBar = async () => {
    const settings = await getSettings();
    await saveSettings({ ...settings, showBookmarksBar: !settings.showBookmarksBar });
  };

  const toggleVerticalTabs = async () => {
    const settings = await getSettings();
    await saveSettings({ ...settings, tabLayout: settings.tabLayout === "vertical" ? "horizontal" : "vertical" });
  };

  return (
    <PopupFrame id="app-menu" ready={snapshot.loaded}>
      <PopupMenu id="app-menu-items" label="Browser menu">
        <PopupMenuItem id="app-menu-new-tab" icon={<SquarePlus />} label="New tab" shortcut="Ctrl+T" onClick={openPage("ce://newtab")} />
        <PopupMenuItem id="app-menu-ephemeral" icon={<Glasses />} label="New ephemeral tab" onClick={run(() => createEphemeralTab())} />
        <PopupMenuItem
          id="app-menu-reopen"
          icon={<History />}
          label="Reopen closed tab"
          shortcut="Ctrl+Shift+T"
          disabled={!snapshot.canReopen}
          onClick={run(() => reopenClosedTab())}
        />
        <PopupMenuSeparator id="app-menu-separator-1" />
        <PopupMenuItem id="app-menu-bookmarks" icon={<Bookmark />} label="Bookmarks" onClick={openPage("ce://bookmarks")} />
        <PopupMenuItem
          id="app-menu-bookmarks-bar"
          icon={<PanelTop />}
          label="Show or hide bookmarks bar"
          shortcut="Ctrl+Shift+B"
          onClick={run(toggleBookmarksBar)}
        />
        <PopupMenuItem
          id="app-menu-vertical-tabs"
          icon={<PanelLeft />}
          label="Vertical tabs on or off"
          onClick={run(toggleVerticalTabs)}
        />
        <PopupMenuItem id="app-menu-downloads" icon={<Download />} label="Downloads" shortcut="Ctrl+J" onClick={openPage("ce://downloads")} />
        <PopupMenuItem id="app-menu-history" icon={<History />} label="History" onClick={openPage("ce://history")} />
        <PopupMenuItem id="app-menu-extensions" icon={<Puzzle />} label="Extensions" onClick={openPage("ce://extensions")} />
        <PopupMenuSeparator id="app-menu-separator-2" />
        <div id="app-menu-zoom" className="flex h-9 items-center gap-2.5 rounded-lg px-2.5 text-sm">
          <ZoomIn id="app-menu-zoom-icon" className="size-4 shrink-0 text-zinc-400" aria-hidden />
          <span id="app-menu-zoom-label" className="flex-1">
            Zoom
          </span>
          <IconButton
            id="app-menu-zoom-out"
            label="Zoom out (Ctrl+-)"
            disabled={!active}
            onClick={() => active && void zoomTab(active.id, "out")}
          >
            <Minus id="app-menu-zoom-out-icon" aria-hidden />
          </IconButton>
          <button
            id="app-menu-zoom-reset"
            type="button"
            title="Reset zoom (Ctrl+0)"
            disabled={!active}
            className="w-12 rounded text-center text-xs text-zinc-300 hover:bg-zinc-800"
            onClick={() => active && void zoomTab(active.id, "reset")}
          >
            {Math.round((active?.zoom ?? 1) * 100)}%
          </button>
          <IconButton
            id="app-menu-zoom-in"
            label="Zoom in (Ctrl++)"
            disabled={!active}
            onClick={() => active && void zoomTab(active.id, "in")}
          >
            <Plus id="app-menu-zoom-in-icon" aria-hidden />
          </IconButton>
        </div>
        <PopupMenuItem
          id="app-menu-devtools"
          icon={<Code />}
          label="Developer tools"
          shortcut="F12"
          disabled={!active}
          onClick={run(async () => {
            if (active) {
              await openDevtools(active.id);
            }
          })}
        />
        <PopupMenuSeparator id="app-menu-separator-3" />
        <PopupMenuItem id="app-menu-settings" icon={<Settings />} label="Settings" onClick={openPage("ce://settings")} />
        <PopupMenuItem id="app-menu-exit" icon={<LogOut />} label="Exit" onClick={() => void closeWindow()} />
      </PopupMenu>
    </PopupFrame>
  );
}
