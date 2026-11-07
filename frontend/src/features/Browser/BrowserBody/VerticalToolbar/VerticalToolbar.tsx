import { forwardRef, useRef, useState, type DragEvent } from "react";
import { PanelTop } from "lucide-react";
import type { BrowserSettings, ExtensionInfo, TabInfo } from "@type/browser";
import { ToolbarIcon } from "@features/BuiltInExtensions";
import { builtinTool } from "@features/BuiltInExtensions/builtin";
import { openPopup, saveSettings } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { openToolbarPinMenu, reorderToolbarAt, splitToolbar, toolbarDragHandlers, toolbarPinned } from "@/lib/toolbar-pin";
import { ExtensionIcon } from "@/components/ExtensionIcon";
import { IconButton } from "@/components/IconButton";
import { Toggle } from "@/components/Toggle";


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
        void openPopup("extension", event.currentTarget, { params: `id=${extension.id}` })
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

type DropEdge = "before" | "after";

type ToolbarEntryProps = {
  id: string;
  idPrefix: string;
  extension: ExtensionInfo | undefined;
  activeTab: TabInfo | undefined;
  settings: BrowserSettings | null;
  labeled: boolean;
  onMove?: (from: string, to: string) => void;
  /** Gap this icon shows while the vertical list is being rearranged. */
  drop?: DropEdge | null;
  dragging?: boolean;
  /** Set on the vertical list, which places the drop itself. */
  toolbarIndex?: number;
  onDragStart?: (event: DragEvent<HTMLElement>) => void;
  onDragEnd?: () => void;
};

export function ToolbarEntry({
  id,
  idPrefix,
  extension,
  activeTab,
  settings,
  labeled,
  onMove,
  drop = null,
  dragging = false,
  toolbarIndex,
  onDragStart,
  onDragEnd,
}: ToolbarEntryProps) {
  const domId = `${idPrefix}-tool-${id}`;
  const caption = extension?.name ?? builtinTool(id)?.name ?? "";
  const stacked = idPrefix === "vertical";
  const listDrag = toolbarIndex !== undefined;
  return (
    <span
      id={domId}
      title={caption || undefined}
      data-toolbar-index={listDrag ? toolbarIndex : undefined}
      className={cn(
        "relative",
        stacked ? "flex w-full flex-col items-center py-0.5" : "inline-flex",
        labeled && "gap-0 px-0.5 py-0",
        labeled && stacked && "group rounded-md hover:bg-zinc-800 hover:text-zinc-100 [&_.chrome-toolbar-button]:rounded-md [&_.chrome-toolbar-button]:hover:bg-transparent",
        dragging && "opacity-40",
      )}
      {...(listDrag
        ? { draggable: true, onDragStart, onDragEnd }
        : toolbarDragHandlers(id, onMove ?? (() => undefined)))}
      onContextMenu={(event) => openToolbarPinMenu(event, id)}
    >
      {drop === "before" ? (
        <span
          id={`${domId}-drop-before`}
          className="pointer-events-none absolute inset-x-1.5 -top-px z-10 h-0.5 rounded-full bg-sky-400 shadow-[0_0_6px_rgb(56_189_248)]"
        />
      ) : null}
      {drop === "after" ? (
        <span
          id={`${domId}-drop-after`}
          className="pointer-events-none absolute inset-x-1.5 -bottom-px z-10 h-0.5 rounded-full bg-sky-400 shadow-[0_0_6px_rgb(56_189_248)]"
        />
      ) : null}
      {extension ? (
        <ExtensionToolbarButton extension={extension} idPrefix={idPrefix} />
      ) : (
        <ToolbarIcon id={id} activeTab={activeTab} settings={settings} />
      )}
      {labeled && caption ? (
        <span id={`${domId}-label`} className="pointer-events-none -mt-1.5 line-clamp-2 w-full min-w-0 wrap-break-word px-0.5 pb-1 text-center text-[8px] leading-none text-zinc-400 group-hover:text-zinc-100">
          {caption}
        </span>
      ) : null}
    </span>
  );
}

/** Gap under the pointer. Counts the slots between icons, including the slot after the last one. */
function toolbarIndexAt(container: HTMLElement, clientY: number) {
  const items = container.querySelectorAll<HTMLElement>(":scope > [data-toolbar-index]");
  for (const item of items) {
    const rect = item.getBoundingClientRect();
    if (clientY < rect.top + rect.height / 2) {
      return Number(item.dataset.toolbarIndex);
    }
  }
  return items.length;
}

/** Hides the line when the icon would land in the gap it already occupies. */
function dropEdge(index: number, count: number, fromIndex: number, dropIndex: number | null): DropEdge | null {
  if (dropIndex === null || fromIndex < 0 || dropIndex === fromIndex || dropIndex === fromIndex + 1) {
    return null;
  }
  if (dropIndex === index) {
    return "before";
  }
  if (index === count - 1 && dropIndex === count) {
    return "after";
  }
  return null;
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
  const draggingRef = useRef<string | null>(null);
  const [dragging, setDragging] = useState<string | null>(null);
  const [dropIndex, setDropIndex] = useState<number | null>(null);
  const fromIndex = dragging ? vertical.indexOf(dragging) : -1;

  const rememberDrop = (index: number) => {
    setDropIndex((current) => (current === index ? current : index));
  };

  const trackDrag = (event: DragEvent<HTMLElement>) => {
    if (!draggingRef.current) {
      return;
    }
    event.preventDefault();
    event.dataTransfer.dropEffect = "move";
    rememberDrop(toolbarIndexAt(event.currentTarget, event.clientY));
  };

  const clearDrag = () => {
    draggingRef.current = null;
    setDragging(null);
    setDropIndex(null);
  };

  const finishDrop = (event: DragEvent<HTMLElement>) => {
    const from = draggingRef.current;
    if (!from) {
      return;
    }
    event.preventDefault();
    const index = toolbarIndexAt(event.currentTarget, event.clientY);
    if (settings) {
      const next = reorderToolbarAt(all, vertical, from, index);
      if (next) {
        void saveSettings({ ...settings, toolbarOrder: next });
      }
    }
    clearDrag();
  };

  if (vertical.length === 0) {
    return null;
  }

  return (
    <aside
      id="vertical-toolbar"
      ref={ref}
      aria-label="Tools"
      className="chrome-tool-strip flex h-full w-12 min-h-0 min-w-12 max-w-12 shrink-0 flex-col overflow-hidden border-l border-zinc-800 bg-zinc-950"
    >
      <div id="vertical-toolbar-dock" className="flex h-[var(--toolbar-height)] shrink-0 items-center justify-center border-b border-zinc-800">
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
        onDragEnter={trackDrag}
        onDragOver={trackDrag}
        onDrop={finishDrop}
        onDragLeave={(event) => {
          const next = event.relatedTarget;
          if (!(next instanceof Node) || event.currentTarget.contains(next)) {
            return;
          }
          setDropIndex(null);
        }}
      >
        {vertical.map((id, index) => (
          <ToolbarEntry
            key={id}
            id={id}
            idPrefix="vertical"
            extension={toolbarExtensions.find((item) => item.id === id)}
            activeTab={activeTab}
            settings={settings}
            labeled={labels}
            toolbarIndex={index}
            dragging={dragging === id}
            drop={dropEdge(index, vertical.length, fromIndex, dropIndex)}
            onDragStart={(event) => {
              event.dataTransfer.effectAllowed = "move";
              event.dataTransfer.setData("text/plain", id);
              draggingRef.current = id;
              setDragging(id);
              setDropIndex(null);
            }}
            onDragEnd={clearDrag}
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
