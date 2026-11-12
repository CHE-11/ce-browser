import { Code } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { createTab } from "@/lib/browser-client";
import type { TabInfo } from "@type/browser";
import { siteKey } from "../page-controls/site";

export function ScriptsButton({ activeTab }: { activeTab: TabInfo | undefined }) {
  return (
    <IconButton
      id="address-scripts"
      label="Site scripts"
      onClick={() => {
        const host = siteKey(activeTab?.url);
        void createTab(host ? `ce://scripts?host=${encodeURIComponent(host)}` : "ce://scripts");
      }}
    >
      <Code id="address-scripts-icon" aria-hidden />
    </IconButton>
  );
}
