import { useState } from "react";
import { Archive } from "lucide-react";
import { Button } from "@/components/Button";
import { PopupFrame } from "@/components/PopupFrame";
import { closePopup, createTab, waybackUrl } from "@/lib/browser-client";
import type { TabInfo } from "@type/browser";
import { PopoverHeader } from "../PopoverParts";
import { readPopoverTab } from "../shared";
import { ToolButton } from "../ToolButton";

export function WaybackButton({ activeTab }: { activeTab: TabInfo | undefined }) {
  return (
    <ToolButton
      id="wayback"
      label="Wayback Machine"
      kind="wayback"
      icon={<Archive id="address-wayback-icon" aria-hidden />}
      activeTab={activeTab}
      height={220}
    />
  );
}

export function WaybackPopover({ params }: { params: URLSearchParams }) {
  const tab = readPopoverTab(params);
  const [error, setError] = useState("");

  const open = (save: boolean) => {
    void waybackUrl(save, tab.url)
      .then((url) => createTab(url))
      .then(() => closePopup())
      .catch((reason: unknown) => setError(reason instanceof Error ? reason.message : "That page can't be archived."));
  };

  return (
    <PopupFrame id="wayback-popover" maxHeight={240}>
      <PopoverHeader id="wayback-header" icon={<Archive aria-hidden />} title="Wayback" subtitle={tab.url} />
      <div id="wayback-body" className="flex flex-col gap-2 px-3 pb-3">
        {error ? (
          <p id="wayback-error" className="text-xs text-amber-300">
            {error}
          </p>
        ) : null}
        <Button id="wayback-open" variant="primary" onClick={() => open(false)}>
          Open in the archive
        </Button>
        <Button id="wayback-save" onClick={() => open(true)}>
          Save this page
        </Button>
      </div>
    </PopupFrame>
  );
}
