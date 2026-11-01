import type { ReactNode } from "react";
import type { BrowserSettings, TabInfo } from "../../../types/browser";
import { openToolbarPinMenu, toolbarPinned } from "../lib/toolbar-pin";
import { AdBlockerButton } from "./ad-blocker/AdBlockerButton";
import { ClearCacheButton } from "./clear-cache/ClearCacheButton";
import { PageCaptureButton } from "./page-capture/PageCaptureButton";
import { PrintPdfButton } from "./print-pdf/PrintPdfButton";
import { WhatRunsButton } from "./what-runs/WhatRunsButton";

type BuiltinExtensionsProps = {
  activeTab: TabInfo | undefined;
  settings: BrowserSettings | null;
};

function pinnedTool(settings: BrowserSettings | null, id: string, button: ReactNode) {
  if (!toolbarPinned(settings, id)) {
    return null;
  }
  return (
    <span id={`toolbar-pin-${id}`} className="inline-flex" onContextMenu={(event) => openToolbarPinMenu(event, id)}>
      {button}
    </span>
  );
}

/** Extensions that ship with the browser. They sit in the toolbar after History. */
export function BuiltinExtensions({ activeTab, settings }: BuiltinExtensionsProps) {
  return (
    <div id="builtin-extensions" className="flex items-center">
      {pinnedTool(settings, "print-pdf", <PrintPdfButton activeTab={activeTab} settings={settings} />)}
      {pinnedTool(settings, "page-capture", <PageCaptureButton activeTab={activeTab} settings={settings} />)}
      {pinnedTool(settings, "clear-cache", <ClearCacheButton activeTab={activeTab} />)}
      {pinnedTool(settings, "what-runs", <WhatRunsButton activeTab={activeTab} />)}
      {pinnedTool(settings, "ad-blocker", <AdBlockerButton activeTab={activeTab} settings={settings} />)}
    </div>
  );
}
