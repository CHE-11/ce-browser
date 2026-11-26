import { useRef } from "react";
import { ListTree } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { openPopup } from "@/lib/browser-client";
import type { TabInfo } from "@type/browser";

export function NetworkButton({ activeTab }: { activeTab: TabInfo | undefined }) {
  const buttonRef = useRef<HTMLButtonElement>(null);

  return (
    <IconButton
      id="address-network"
      ref={buttonRef}
      label="Network"
      disabled={!activeTab}
      onClick={() => {
        const anchor = buttonRef.current;
        if (!anchor || !activeTab) {
          return;
        }
        void openPopup("network", anchor, { params: `id=${activeTab.id}` });
      }}
    >
      <ListTree id="address-network-icon" aria-hidden />
    </IconButton>
  );
}
