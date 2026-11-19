import { useRef } from "react";
import { SlidersHorizontal } from "lucide-react";

import { controlFor, siteActive, siteKey } from "./site";
import { IconButton } from "@/components/IconButton";
import { openPopup } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { usePageRules } from "@/lib/use-browser";
import type { TabInfo } from "@type/browser";

type PageControlsButtonProps = {
  activeTab: TabInfo | undefined;
};

export function PageControlsButton({ activeTab }: PageControlsButtonProps) {
  const buttonRef = useRef<HTMLButtonElement>(null);
  const rules = usePageRules();
  const host = siteKey(activeTab?.url);
  const site = controlFor(rules, host);
  const on = host !== "" && (siteActive(site) || (activeTab?.refreshSecs ?? 0) > 0);

  const open = () => {
    const anchor = buttonRef.current;
    if (!activeTab || !anchor || !host) {
      return;
    }
    const params = new URLSearchParams({
      tab: String(activeTab.id),
      title: activeTab.title || activeTab.url,
      url: activeTab.url,
      host,
    });
    void openPopup("page-controls", anchor, { params: params.toString() });
  };

  return (
    <IconButton
      id="address-page-controls"
      ref={buttonRef}
      label={host ? `Page controls for ${host}` : "Page controls"}
      disabled={!host}
      className={cn("relative", on && "text-sky-300")}
      onClick={open}
    >
      <SlidersHorizontal id="address-page-controls-icon" aria-hidden />
      {on ? (
        <span id="address-page-controls-badge" className="pointer-events-none absolute right-1 top-1 size-1.5 rounded-full bg-sky-400" />
      ) : null}
    </IconButton>
  );
}
