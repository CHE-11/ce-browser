import { useRef, type ReactNode } from "react";
import { IconButton } from "@/components/IconButton";
import { openPopup, restoreTranslation, translationActive } from "@/lib/browser-client";
import type { PopupKind, TabInfo } from "@type/browser";
import { capturable, tabParams } from "./shared";

type ToolButtonProps = {
  id: string;
  label: string;
  kind: PopupKind;
  icon: ReactNode;
  activeTab: TabInfo | undefined;
  /** When the page is already translated, a click reloads it. */
  restore?: boolean;
  /** Password does not need a website open. */
  needsPage?: boolean;
  /** Count drawn on the top-right of the icon. Hidden at zero. */
  badge?: number;
  /** Control drawn on the top-left of the icon, above the button. */
  corner?: ReactNode;
};

function badgeText(count: number) {
  return count > 99 ? "99+" : String(count);
}

export function ToolButton({ id, label, kind, icon, activeTab, restore, needsPage = true, badge = 0, corner }: ToolButtonProps) {
  const buttonRef = useRef<HTMLButtonElement>(null);

  const open = () => {
    const anchor = buttonRef.current;
    if (!anchor || (needsPage && !activeTab)) {
      return;
    }
    const tab = activeTab;
    const show = () => {
      void openPopup(kind, anchor, { params: tab ? tabParams(tab) : "" });
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
    <span id={`address-${id}-slot`} className="relative inline-flex">
      {corner ? (
        <span id={`address-${id}-corner`} className="absolute top-0 left-0 z-10 -translate-x-1/4 -translate-y-1/4">
          {corner}
        </span>
      ) : null}
      <IconButton id={`address-${id}`} ref={buttonRef} label={label} disabled={needsPage && !capturable(activeTab?.url)} className="relative" onClick={open}>
        {icon}
        {badge > 0 ? (
          <span
            id={`address-${id}-badge`}
            className="pointer-events-none absolute -top-0.5 -right-0.5 min-w-4 rounded-full bg-sky-600 px-1 text-center text-[9px] font-semibold leading-4 text-white"
            aria-hidden
          >
            {badgeText(badge)}
          </span>
        ) : null}
      </IconButton>
    </span>
  );
}
