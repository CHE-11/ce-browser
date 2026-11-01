import { useRef } from "react";
import { Clapperboard } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { openPopupBelow } from "@/lib/browser-client";

export function MediaHubButton() {
  const buttonRef = useRef<HTMLButtonElement>(null);

  return (
    <IconButton
      id="address-media-hub"
      ref={buttonRef}
      label="Media"
      onClick={() => {
        const anchor = buttonRef.current;
        if (anchor) {
          void openPopupBelow(anchor, { kind: "media-hub", width: 360, height: 320 });
        }
      }}
    >
      <Clapperboard id="address-media-hub-icon" aria-hidden />
    </IconButton>
  );
}
