import type { BrowserSettings, TabInfo } from "../../../types/browser";
import { ClearCacheButton } from "./clear-cache/ClearCacheButton";
import { PageCaptureButton } from "./page-capture/PageCaptureButton";
import { PrintPdfButton } from "./print-pdf/PrintPdfButton";

type BuiltinExtensionsProps = {
  activeTab: TabInfo | undefined;
  settings: BrowserSettings | null;
};

/** Extensions that ship with the browser. They sit in the toolbar after History. */
export function BuiltinExtensions({ activeTab, settings }: BuiltinExtensionsProps) {
  return (
    <div id="builtin-extensions" className="flex items-center">
      <PrintPdfButton activeTab={activeTab} settings={settings} />
      <PageCaptureButton activeTab={activeTab} settings={settings} />
      <ClearCacheButton activeTab={activeTab} />
    </div>
  );
}
