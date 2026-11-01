import { forwardRef } from "react";
import { PanelTop } from "lucide-react";
import type { BrowserSettings, ExtensionInfo, TabInfo } from "@type/browser";
import { ToolbarIcon } from "@features/BuiltInExtensions";
import { builtinTool } from "@features/BuiltInExtensions/builtin";
import { openPopupBelow, saveSettings } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { openToolbarPinMenu, reorderToolbar, splitToolbar, toolbarDragHandlers, toolbarPinned } from "@/lib/toolbar-pin";
import { ExtensionIcon } from "./ExtensionIcon";
import { IconButton } from "./IconButton";
import { Toggle } from "./Toggle";

type ExtensionToolbarButtonProps = {
  extension: ExtensionInfo;
  idPrefix: string;
};

export function ExtensionToolbarButton({ extension, idPrefix }: ExtensionToolbarButtonProps) {
  const domId = `${idPrefix}-extension-${extension.id}`;
  return (
    <IconButton
      id={domId}
      label={extension.name}
      disabled={!extension.popup}
      onClick={(event) =>
        void openPopupBelow(event.currentTarget, {
          kind: "extension",
          params: `id=${extension.id}`,
          width: 380,
          height: 600,
        })
      }
    >
      <ExtensionIcon
        id={`${domId}-icon`}
        extensionId={extension.id}
        name={extension.name}
        src={extension.icon}
        className="size-4"
      />
    </IconButton>
  );
}

type ToolbarEntryProps = {
  id: string;
  idPrefix: string;
  extension: ExtensionInfo | undefined;
  activeTab: TabInfo | undefined;
  settings: BrowserSettings | null;
  labeled: boolean;
  onMove: (from: string, to: string) => void;
};

export function ToolbarEntry({ id, idPrefix, extension, activeTab, settings, labeled, onMove }: ToolbarEntryProps) {
  const domId = `${idPrefix}-tool-${id}`;
  const caption = extension?.name ?? builtinTool(id)?.name ?? "";
  const stacked = idPrefix === "vertical";
  return (
    <span
      id={domId}
      title={caption || undefined}
      className={cn(
        stacked ? "flex w-full flex-col items-center py-0.5" : "inline-flex",
        labeled && "gap-0 px-0.5 py-0",
      )}
      {...toolbarDragHandlers(id, onMove)}
      onContextMenu={(event) => openToolbarPinMenu(event, id)}
    >
      {extension ? (
        <ExtensionToolbarButton extension={extension} idPrefix={idPrefix} />
      ) : (
        <ToolbarIcon id={id} activeTab={activeTab} settings={settings} />
      )}
      {labeled && caption ? (
        <span id={`${domId}-label`} className="pointer-events-none -mt-1.5 line-clamp-2 w-full min-w-0 break-words text-center text-[8px] leading-none text-zinc-400">
          {caption}
        </span>
      ) : null}
    </span>
  );
}

type VerticalToolbarProps = {
  activeTab: TabInfo | undefined;
  extensions: ExtensionInfo[];
  settings: BrowserSettings | null;
};

/** Pinned built-in tools, and extensions when that setting is on, along the right edge of the page. */
export const VerticalToolbar = forwardRef<HTMLElement, VerticalToolbarProps>(function VerticalToolbar(
  { activeTab, extensions, settings },
  ref,
) {
  const toolbarExtensions = extensions.filter((extension) => extension.enabled && toolbarPinned(settings, extension.id));
  const { vertical, all } = splitToolbar(
    settings,
    toolbarExtensions.map((extension) => extension.id),
  );
  const labels = settings?.verticalToolbarLabels ?? false;

  const moveToolbar = (from: string, to: string) => {
    if (!settings) {
      return;
    }
    const next = reorderToolbar(all, from, to);
    if (next) {
      void saveSettings({ ...settings, toolbarOrder: next });
    }
  };

  if (vertical.length === 0) {
    return null;
  }

  return (
    <aside
      id="vertical-toolbar"
      ref={ref}
      aria-label="Tools"
      className="flex h-full w-12 min-h-0 min-w-12 max-w-12 shrink-0 flex-col overflow-hidden border-l border-zinc-800 bg-zinc-950"
    >
      <div id="vertical-toolbar-dock" className="flex shrink-0 justify-center border-b border-zinc-800 py-1">
        <IconButton
          id="vertical-toolbar-to-address"
          label="Show on the address bar"
          disabled={!settings}
          onClick={() => settings && void saveSettings({ ...settings, verticalToolbar: false })}
        >
          <PanelTop id="vertical-toolbar-to-address-icon" aria-hidden />
        </IconButton>
      </div>
      <div
        id="vertical-toolbar-items"
        className={cn(
          "flex min-h-0 flex-1 flex-col items-center overflow-x-hidden overflow-y-auto py-1",
          labels && "gap-3",
        )}
      >
        {vertical.map((id) => (
          <ToolbarEntry
            key={id}
            id={id}
            idPrefix="vertical"
            extension={toolbarExtensions.find((item) => item.id === id)}
            activeTab={activeTab}
            settings={settings}
            labeled={labels}
            onMove={moveToolbar}
          />
        ))}
      </div>
      <div id="vertical-toolbar-labels" className="flex shrink-0 flex-col items-center gap-1 border-t border-zinc-800 px-1 py-1.5">
        <span id="vertical-toolbar-labels-caption" className="text-[9px] leading-none text-zinc-500">
          Labels
        </span>
        <Toggle
          id="vertical-toolbar-labels-toggle"
          size="sm"
          title={labels ? "Hide labels" : "Show labels"}
          aria-labelledby="vertical-toolbar-labels-caption"
          checked={labels}
          disabled={!settings}
          onChange={(verticalToolbarLabels) => settings && void saveSettings({ ...settings, verticalToolbarLabels })}
        />
      </div>
    </aside>
  );
});
