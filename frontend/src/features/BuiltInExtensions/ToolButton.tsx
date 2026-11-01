import { useRef, type ReactNode } from "react";
import { IconButton } from "@/components/IconButton";
import { openPopupBelow, restoreTranslation, translationActive } from "@/lib/browser-client";
import type { PopupKind, TabInfo } from "@type/browser";
import { capturable, tabParams } from "./shared";

type ToolButtonProps = {
  id: string;
  label: string;
  kind: PopupKind;
  icon: ReactNode;
  activeTab: TabInfo | undefined;
  width?: number;
  height?: number;
  /** When the page is already translated, a click reloads it. */
  restore?: boolean;
  /** Password does not need a website open. */
  needsPage?: boolean;
};

export function ToolButton({ id, label, kind, icon, activeTab, width = 320, height = 360, restore, needsPage = true }: ToolButtonProps) {
  const buttonRef = useRef<HTMLButtonElement>(null);

  const open = () => {
    const anchor = buttonRef.current;
    if (!anchor || (needsPage && !activeTab)) {
      return;
    }
    const tab = activeTab;
    const show = () => {
      void openPopupBelow(anchor, {
        kind,
        params: tab ? tabParams(tab) : "",
        width,
        height,
      });
    };
    if (restore && tab) {
      void translationActive(tab.id).then((active) => {
        if (active) {
          void restoreTranslation(tab.id);
        } else {
          show();
        }
      });
      return;
    }
    show();
  };

  return (
    <IconButton id={`address-${id}`} ref={buttonRef} label={label} disabled={needsPage && !capturable(activeTab?.url)} onClick={open}>
      {icon}
    </IconButton>
  );
}
