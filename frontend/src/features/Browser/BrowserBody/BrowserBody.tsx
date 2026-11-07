import type { RefObject } from "react";
import type { BrowserSettings, ExtensionInfo, SplitView, TabInfo } from "@type/browser";
import type { WindowTabsState } from "@/lib/use-browser";
import { DevtoolsDivider } from "@/components/DevtoolsDivider";
import { PageFailure } from "@/components/PageFailure";
import { SplitDivider } from "@/components/SplitDivider";
import { VerticalTabs } from "../BrowserHeader/Tabs/VerticalTabs";
import { PageLoading, pagePending } from "./PageLoading";
import { ToolsToolbar } from "./ToolsToolbar/ToolsToolbar";
import { ViewportBar } from "./ViewportBar";
import { ViewportHandles } from "./ViewportHandles";

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
  const devtools = snapshot.devtools;
  const pagesWidth = devtools?.visible ? `calc(${devtools.ratio * 100}% - 3px)` : "100%";
  const showViewportBar = Boolean(devtools?.visible || snapshot.viewport);
  return (
    <div id="browser-body" className="flex min-h-0 flex-1 pt-[var(--chrome-top-slack,0px)]">
      {vertical && settings && !snapshot.fullscreen ? (
        <VerticalTabs ref={sidebarRef} snapshot={snapshot} settings={settings} />
      ) : null}
      <div id="browser-page-area" className="relative flex min-w-0 flex-1 items-center justify-center bg-zinc-950">
        <div id="browser-pages" className="absolute inset-y-0 left-0" style={{ width: pagesWidth }}>
          {showViewportBar ? (
            <ViewportBar
              width={snapshot.viewport?.width ?? 0}
              height={snapshot.viewport?.height ?? 0}
              cutout={snapshot.viewport?.cutout ?? true}
              homeBar={snapshot.viewport?.homeBar ?? true}
              devtoolsTabId={devtools ? snapshot.activeId : null}
            />
          ) : null}
          {snapshot.viewport && snapshot.viewport.width > 0 && snapshot.viewport.height > 0 ? (
            <ViewportHandles
              width={snapshot.viewport.width}
              height={snapshot.viewport.height}
              cutout={snapshot.viewport.cutout}
              homeBar={snapshot.viewport.homeBar}
            />
          ) : null}
          {splitShown && split
            ? [
                { id: split.left, style: { left: 0, width: `calc(${split.ratio * 100}% - 3px)` } },
                { id: split.right, style: { right: 0, width: `calc(${(1 - split.ratio) * 100}% - 3px)` } },
              ].map(({ id, style }) => {
                const tab = snapshot.tabs.find((item) => item.id === id);
                if (tab?.failure) {
                  return <PageFailure key={id} tab={tab} failure={tab.failure} className="absolute inset-y-0" style={style} />;
                }
                if (pagePending(tab)) {
                  return <PageLoading key={id} tab={tab} className="absolute inset-y-0" style={style} />;
                }
                return null;
              })
            : activeTab?.failure
              ? <PageFailure tab={activeTab} failure={activeTab.failure} className="absolute inset-0" />
              : pagePending(activeTab)
                ? <PageLoading tab={activeTab} className="absolute inset-0" />
                : null}
          {splitShown && split ? <SplitDivider split={split} activeId={snapshot.activeId} /> : null}
        </div>
        {devtools?.visible ? <DevtoolsDivider ratio={devtools.ratio} /> : null}
      </div>
      {showToolStrip ? (
        <ToolsToolbar ref={toolbarRef} layout="vertical" activeTab={activeTab} extensions={extensions} settings={settings} />
      ) : null}
    </div>
  );
}
