import { Puzzle } from "lucide-react";
import type { HostExtensionInfo } from "@type/browser";
import { IconButton } from "@/components/IconButton";
import { openHostPopout, openPopup, toggleHostPanel } from "@/lib/browser-client";
import { useExtensionAction } from "@/lib/extension-action";
import { useTabs } from "@/lib/use-browser";
import { ExtensionActionBadge } from "./ExtensionActionBadge";

type HostToolbarButtonProps = {
  extension: HostExtensionInfo;
  idPrefix: string;
};

export function HostToolbarButton({ extension, idPrefix }: HostToolbarButtonProps) {
  const { sidePanels, tabs, activeId } = useTabs();
  const pageUrl = tabs.find((tab) => tab.id === activeId)?.url;
  const action = useExtensionAction(`host:${extension.id}`, pageUrl);
  const domId = `${idPrefix}-host-${extension.id.replace(/[^a-z0-9]+/g, "-")}`;
  const label = action.title || extension.name;
  const active = sidePanels.some((panel) => panel.extensionId === extension.id);
  return (
    <IconButton
      id={domId}
      label={action.badge ? `${label}, ${action.badge}` : label}
      active={active}
      data-host-id={extension.id}
      onClick={(event) => {
        if (extension.surfaces.popup) {
          void openPopup("host", event.currentTarget, { params: `id=${extension.id}` });
          return;
        }
        if (extension.surfaces.panel) {
          void toggleHostPanel(extension.id);
          return;
        }
        if (extension.surfaces.popout) {
          void openHostPopout(extension.id);
        }
      }}
    >
      <span id={`${domId}-glyph`} className="relative inline-flex">
        {action.icon || extension.icon ? (
          <img id={`${domId}-icon`} src={action.icon || extension.icon || ""} alt="" className="size-4" />
        ) : (
          <Puzzle id={`${domId}-icon`} aria-hidden />
        )}
        <ExtensionActionBadge
          id={`${domId}-badge`}
          text={action.badge}
          color={action.badgeColor}
          textColor={action.badgeTextColor}
        />
      </span>
    </IconButton>
  );
}
