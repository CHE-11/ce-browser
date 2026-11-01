import { useRef } from "react";
import { Layers } from "lucide-react";
import { capturable, tabParams } from "../shared";
import { IconButton } from "@/components/IconButton";
import { openPopupBelow } from "@/lib/browser-client";
import { TabInfo } from "../../../../../types/browser";

type WhatRunsButtonProps = {
  activeTab: TabInfo | undefined;
};

const POPOVER_WIDTH = 320;
/** Placeholder only. The popover stays hidden until it reports its real height. */
const POPOVER_HEIGHT = 300;

export function WhatRunsButton({ activeTab }: WhatRunsButtonProps) {
  const buttonRef = useRef<HTMLButtonElement>(null);

  const open = () => {
    const anchor = buttonRef.current;
    if (!activeTab || !anchor) {
      return;
    }
    void openPopupBelow(anchor, {
      kind: "what-runs",
      params: tabParams(activeTab),
      width: POPOVER_WIDTH,
      height: POPOVER_HEIGHT,
    });
  };

  return (
    <IconButton
      id="address-what-runs"
      ref={buttonRef}
      label="What runs this site"
      disabled={!capturable(activeTab?.url)}
      onClick={open}
    >
      <Layers id="address-what-runs-icon" aria-hidden />
    </IconButton>
  );
}
