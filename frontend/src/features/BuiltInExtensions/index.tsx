import type { BrowserSettings, TabInfo } from "../../../../types/browser";
import { AdBlockerButton } from "./ad-blocker/AdBlockerButton";
import { AssistantButton } from "./assistant/Assistant";
import { PageControlsButton } from "./page-controls/PageControlsButton";
import { ClearCacheButton } from "./clear-cache/ClearCacheButton";
import { PageCaptureButton } from "./page-capture/PageCaptureButton";
import { PasswordButton } from "./password/Password";
import { PriceButton } from "./price/Price";
import { MediaHubButton } from "../Media/MediaHubButton";
import { PrintPdfButton } from "./print-pdf/PrintPdfButton";
import { ReadingButton } from "./reading/Reading";
import { TranslateButton } from "./translate/Translate";
import { WaybackButton } from "./wayback/Wayback";
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
    case "translate":
      return <TranslateButton activeTab={activeTab} />;
    case "password":
      return <PasswordButton activeTab={activeTab} />;
    case "price":
      return <PriceButton activeTab={activeTab} />;
    case "wayback":
      return <WaybackButton activeTab={activeTab} />;
    case "reading":
      return <ReadingButton activeTab={activeTab} />;
    case "assistant":
      return <AssistantButton activeTab={activeTab} />;
    default:
      return null;
  }
}
