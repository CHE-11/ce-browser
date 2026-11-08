import { Puzzle } from "lucide-react";
import type { HostExtensionInfo } from "@type/browser";
import { IconButton } from "@/components/IconButton";
import { openHostPopout, openPopup, toggleHostPanel } from "@/lib/browser-client";
import { useTabs } from "@/lib/use-browser";

type HostToolbarButtonProps = {
  extension: HostExtensionInfo;
  idPrefix: string;
};

export function HostToolbarButton({ extension, idPrefix }: HostToolbarButtonProps) {
  const { hostPanel } = useTabs();
  const domId = `${idPrefix}-host-${extension.id.replace(/[^a-z0-9]+/g, "-")}`;
  const active = hostPanel === extension.id;
  return (
    <IconButton
      id={domId}
      label={extension.name}
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
      {extension.icon ? (
        <img id={`${domId}-icon`} src={extension.icon} alt="" className="size-4" />
      ) : (
        <Puzzle id={`${domId}-icon`} aria-hidden />
      )}
    </IconButton>
  );
}
