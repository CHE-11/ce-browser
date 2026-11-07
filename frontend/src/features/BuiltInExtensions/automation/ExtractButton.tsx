import { useRef } from "react";
import { TableProperties } from "lucide-react";
import type { TabInfo } from "@type/browser";
import { IconButton } from "@/components/IconButton";
import { openPopup } from "@/lib/browser-client";
import { capturable, tabParams } from "../shared";

/** Opens the extractor for lists and tables on the current page. */
export function ExtractButton({ activeTab }: { activeTab: TabInfo | undefined }) {
  const buttonRef = useRef<HTMLButtonElement>(null);
  const open = () => {
    const anchor = buttonRef.current;
    if (!activeTab || !anchor) {
      return;
    }
    void openPopup("extract", anchor, { params: tabParams(activeTab) });
  };
  return (
    <IconButton id="address-extract" ref={buttonRef} label="Extract data from this page" disabled={!capturable(activeTab?.url)} onClick={open}>
      <TableProperties id="address-extract-icon" aria-hidden />
    </IconButton>
  );
}
