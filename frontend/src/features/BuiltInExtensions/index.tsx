import type { BrowserSettings, TabInfo } from "../../../../types/browser";
import { AdBlockerButton } from "./ad-blocker/AdBlockerButton";
import { BoardsButton } from "./boards/BoardsButton";
import { PageControlsButton } from "./page-controls/PageControlsButton";
import { ClearCacheButton } from "./clear-cache/ClearCacheButton";
import { PageCaptureButton } from "./page-capture/PageCaptureButton";
import { WebpageMarkdownButton } from "./webpage-markdown/WebpageMarkdownButton";
import { PasswordButton } from "./password/Password";
import { MediaHubButton } from "./media/MediaHubButton";
import { PrintPdfButton } from "./print-pdf/PrintPdfButton";
import { ReadingButton } from "./reading/Reading";
import { ShoppingButton } from "./shopping/ShoppingButton";
import { TranslateButton } from "./translate/Translate";
import { WhatRunsButton } from "./what-runs/WhatRunsButton";

/** One pinned built-in tool. Unknown ids render nothing. */
export function ToolbarIcon({ id, activeTab, settings }: { id: string; activeTab: TabInfo | undefined; settings: BrowserSettings | null }) {
  switch (id) {
    case "print-pdf":
      return <PrintPdfButton activeTab={activeTab} settings={settings} />;
    case "page-capture":
      return <PageCaptureButton activeTab={activeTab} settings={settings} />;
    case "webpage-markdown":
      return <WebpageMarkdownButton activeTab={activeTab} settings={settings} />;
    case "boards":
      return <BoardsButton activeTab={activeTab} />;
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
    case "shopping":
      return <ShoppingButton activeTab={activeTab} />;
    case "reading":
      return <ReadingButton activeTab={activeTab} />;
    default:
      return null;
  }
}
