import { useRef } from "react";
import { ListTree } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { openPopup } from "@/lib/browser-client";
import type { TabInfo } from "@type/browser";
import { useNetworkSpeed } from "../network-speed/use-network-speed";
import { connectionLabel, connectionTone, useInternetProbeSettled, type ConnectionTone } from "./connection";

const TONE_CLASS: Record<ConnectionTone, string> = {
  offline: "text-red-400 hover:text-red-300",
  poor: "text-orange-400 hover:text-orange-300",
};

export function NetworkButton({ activeTab }: { activeTab: TabInfo | undefined }) {
  const buttonRef = useRef<HTMLButtonElement>(null);
  const speed = useNetworkSpeed();
  const tone = connectionTone(speed, useInternetProbeSettled(speed));

  return (
    <IconButton
      id="address-network"
      ref={buttonRef}
      label={connectionLabel(tone)}
      disabled={!activeTab}
      className={tone ? TONE_CLASS[tone] : undefined}
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
