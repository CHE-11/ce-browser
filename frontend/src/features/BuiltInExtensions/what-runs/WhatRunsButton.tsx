import { useRef } from "react";
import { Layers } from "lucide-react";
import { capturable, tabParams } from "../shared";
import { IconButton } from "@/components/IconButton";
import { openPopup } from "@/lib/browser-client";
import { TabInfo } from "../../../../../types/browser";

type WhatRunsButtonProps = {
  activeTab: TabInfo | undefined;
};

export function WhatRunsButton({ activeTab }: WhatRunsButtonProps) {
  const buttonRef = useRef<HTMLButtonElement>(null);

  const open = () => {
    const anchor = buttonRef.current;
    if (!activeTab || !anchor) {
      return;
    }
    void openPopup("what-runs", anchor, { params: tabParams(activeTab) });
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
