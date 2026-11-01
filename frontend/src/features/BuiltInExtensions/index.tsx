import { Archive, BookOpen, KeyRound, Languages, Sparkles, Tags } from "lucide-react";
import type { BrowserSettings, TabInfo } from "../../../../types/browser";
import { ToolButton } from "@features/Tools/ToolButton";
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
    case "translate":
      return <ToolButton id="translate" label="Translate page" kind="translate" icon={<Languages id="address-translate-icon" aria-hidden />} activeTab={activeTab} restore />;
    case "password":
      return <ToolButton id="password" label="Password" kind="password" icon={<KeyRound id="address-password-icon" aria-hidden />} activeTab={activeTab} needsPage={false} />;
    case "price":
      return <ToolButton id="price" label="Prices" kind="price" icon={<Tags id="address-price-icon" aria-hidden />} activeTab={activeTab} height={420} />;
    case "wayback":
      return <ToolButton id="wayback" label="Wayback Machine" kind="wayback" icon={<Archive id="address-wayback-icon" aria-hidden />} activeTab={activeTab} height={220} />;
    case "reading":
      return <ToolButton id="reading" label="Reading" kind="reading" icon={<BookOpen id="address-reading-icon" aria-hidden />} activeTab={activeTab} height={220} />;
    case "assistant":
      return <ToolButton id="assistant" label="Assistant" kind="assistant" icon={<Sparkles id="address-assistant-icon" aria-hidden />} activeTab={activeTab} height={460} />;
    default:
      return null;
  }
}
