import type { RefObject } from "react";
import type { BrowserSettings, ExtensionInfo, SplitView, TabInfo } from "@type/browser";
import type { WindowTabsState } from "@/lib/use-browser";
import { PageFailure } from "@/components/PageFailure";
import { SplitDivider } from "@/components/SplitDivider";
import { VerticalTabs } from "../Browser/BrowserHeader/Tabs/VerticalTabs";
import { VerticalToolbar } from "../Browser/BrowserBody/VerticalToolbar/VerticalToolbar";

type BrowserBodyProps = {
  snapshot: WindowTabsState;
  settings: BrowserSettings | null;
  vertical: boolean;
  sidebarRef: RefObject<HTMLElement | null>;
  toolbarRef: RefObject<HTMLElement | null>;
  activeTab: TabInfo | undefined;
  split: SplitView | null;
  splitShown: boolean;
  showToolStrip: boolean;
  extensions: ExtensionInfo[];
};

export function BrowserBody({
  snapshot,
  settings,
  vertical,
  sidebarRef,
  toolbarRef,
  activeTab,
  split,
  splitShown,
  showToolStrip,
  extensions,
}: BrowserBodyProps) {
  return (
    <div id="browser-body" className="flex min-h-0 flex-1">
      {vertical && settings && !snapshot.fullscreen ? (
        <VerticalTabs ref={sidebarRef} snapshot={snapshot} settings={settings} />
      ) : null}
      <div id="browser-page-area" className="relative flex min-w-0 flex-1 items-center justify-center bg-zinc-950">
        {activeTab && !activeTab.loaded && !activeTab.failure ? (
          <span id="browser-page-loading" className="text-sm text-zinc-500">
            Loading {activeTab.title || activeTab.url}
          </span>
        ) : null}
        {splitShown && split
          ? [
              { id: split.left, style: { left: 0, width: `calc(${split.ratio * 100}% - 3px)` } },
              { id: split.right, style: { right: 0, width: `calc(${(1 - split.ratio) * 100}% - 3px)` } },
            ].map(({ id, style }) => {
              const tab = snapshot.tabs.find((item) => item.id === id);
              return tab?.failure ? (
                <PageFailure key={id} tab={tab} failure={tab.failure} className="absolute inset-y-0" style={style} />
              ) : null;
            })
          : activeTab?.failure
            ? <PageFailure tab={activeTab} failure={activeTab.failure} className="absolute inset-0" />
            : null}
        {splitShown && split ? <SplitDivider split={split} activeId={snapshot.activeId} /> : null}
      </div>
      {showToolStrip ? (
        <VerticalToolbar ref={toolbarRef} activeTab={activeTab} extensions={extensions} settings={settings} />
      ) : null}
    </div>
  );
}
