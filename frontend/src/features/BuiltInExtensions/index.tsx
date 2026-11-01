import type { BrowserSettings, TabInfo } from "../../../../types/browser";
import { AdBlockerButton } from "./ad-blocker/AdBlockerButton";
import { PageControlsButton } from "./page-controls/PageControlsButton";
import { ClearCacheButton } from "./clear-cache/ClearCacheButton";
import { PageCaptureButton } from "./page-capture/PageCaptureButton";
import { MediaHubButton } from "../Media/MediaHubButton";
import { PrintPdfButton } from "./print-pdf/PrintPdfButton";
import { WhatRunsButton } from "./what-runs/WhatRunsButton";

/** One pinned built-in tool. Unknown ids render nothing. */
export function ToolbarIcon({ id, activeTab, settings }: { id: string; activeTab: TabInfo | undefined; settings: BrowserSettings | null }) {
  switch (id) {
    case "print-pdf":
      return <PrintPdfButton activeTab={activeTab} settings={settings} />;
    case "page-capture":
      return <PageCaptureButton activeTab={activeTab} settings={settings} />;
    case "media-hub":
      return <MediaHubButton />;
    case "clear-cache":
      return <ClearCacheButton activeTab={activeTab} />;
    case "what-runs":
      return <WhatRunsButton activeTab={activeTab} />;
    case "ad-blocker":
      return <AdBlockerButton activeTab={activeTab} settings={settings} />;
    case "page-controls":
      return <PageControlsButton activeTab={activeTab} />;
    default:
      return null;
  }
}
