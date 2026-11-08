import { PopupFrame } from "@/components/PopupFrame";
import { NetworkLog } from "./NetworkLog";

type NetworkPopoverProps = {
  tabId: number;
};

/**
 * The window is sized from this content. The height stays fixed so new requests
 * scroll inside it instead of moving the popup.
 */
export function NetworkPopover({ tabId }: NetworkPopoverProps) {
  return (
    <PopupFrame id="network-popover" width={880} maxHeight={640}>
      <NetworkLog tabId={tabId} variant="popup" />
    </PopupFrame>
  );
}
