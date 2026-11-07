import { forwardRef, useRef, useState, type DragEvent } from "react";
import { PanelRight, PanelTop } from "lucide-react";
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
  /** Column of icons, or a row. Address-bar icons omit this and stay in a row. */
  layout?: "vertical" | "horizontal";
  onMove?: (from: string, to: string) => void;
  /** Gap this icon shows while the list is being rearranged. */
  drop?: DropEdge | null;
  dragging?: boolean;
  /** Set on the tools toolbar, which places the drop itself. */
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
  layout = "horizontal",
  onMove,
  drop = null,
  dragging = false,
  toolbarIndex,
  onDragStart,
  onDragEnd,
}: ToolbarEntryProps) {
  const domId = `${idPrefix}-tool-${id}`;
  const caption = extension?.name ?? builtinTool(id)?.name ?? "";
  const vertical = layout === "vertical";
  const stacked = vertical || labeled;
  const listDrag = toolbarIndex !== undefined;
  return (
    <span
      id={domId}
      title={caption || undefined}
      data-toolbar-index={listDrag ? toolbarIndex : undefined}
      className={cn(
        "relative",
        stacked ? "flex shrink-0 flex-col items-center" : "inline-flex",
        vertical ? "w-full py-0.5" : stacked && (labeled ? "w-max px-1.5 py-0" : "w-12 py-0.5"),
        labeled && vertical && "gap-0 px-0.5 py-0",
        labeled && "group rounded-md hover:bg-zinc-800 hover:text-zinc-100 [&_.chrome-toolbar-button]:rounded-md [&_.chrome-toolbar-button]:hover:bg-transparent",
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
          className={cn(
            "pointer-events-none absolute z-10 rounded-full bg-sky-400 shadow-[0_0_6px_rgb(56_189_248)]",
            vertical ? "inset-x-1.5 -top-px h-0.5" : "inset-y-1.5 -left-px w-0.5",
          )}
        />
      ) : null}
      {drop === "after" ? (
        <span
          id={`${domId}-drop-after`}
          className={cn(
            "pointer-events-none absolute z-10 rounded-full bg-sky-400 shadow-[0_0_6px_rgb(56_189_248)]",
            vertical ? "inset-x-1.5 -bottom-px h-0.5" : "inset-y-1.5 -right-px w-0.5",
          )}
        />
      ) : null}
      {extension ? (
        <ExtensionToolbarButton extension={extension} idPrefix={idPrefix} />
      ) : (
        <ToolbarIcon id={id} activeTab={activeTab} settings={settings} />
      )}
      {labeled && caption ? (
        <span
          id={`${domId}-label`}
          className={cn(
            "pointer-events-none min-w-0 px-0.5 text-center text-[8px] leading-none text-zinc-400 group-hover:text-zinc-100",
            vertical
              ? "-mt-1.5 line-clamp-2 w-full wrap-break-word pb-1"
              : "-mt-1 w-max whitespace-nowrap pb-0.5",
          )}
        >
          {caption}
        </span>
      ) : null}
    </span>
  );
}

/** Gap under the pointer. Counts the slots between icons, including the slot after the last one. */
function toolbarIndexAt(container: HTMLElement, client: number, axis: "x" | "y") {
  const items = container.querySelectorAll<HTMLElement>(":scope > [data-toolbar-index]");
  for (const item of items) {
    const rect = item.getBoundingClientRect();
    const start = axis === "x" ? rect.left : rect.top;
    const size = axis === "x" ? rect.width : rect.height;
    if (client < start + size / 2) {
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

type ToolsToolbarProps = {
  activeTab: TabInfo | undefined;
  extensions: ExtensionInfo[];
  settings: BrowserSettings | null;
  layout: "vertical" | "horizontal";
};

/** Pinned built-in tools, on the right or in a row above the bookmarks bar. */
export const ToolsToolbar = forwardRef<HTMLElement, ToolsToolbarProps>(function ToolsToolbar(
  { activeTab, extensions, settings, layout },
  ref,
) {
  const toolbarExtensions = extensions.filter((extension) => extension.enabled && toolbarPinned(settings, extension.id));
  const { tools, all } = splitToolbar(
    settings,
    toolbarExtensions.map((extension) => extension.id),
  );
  const labels = settings?.verticalToolbarLabels ?? false;
  const vertical = layout === "vertical";
  const draggingRef = useRef<string | null>(null);
  const [dragging, setDragging] = useState<string | null>(null);
  const [dropIndex, setDropIndex] = useState<number | null>(null);
  const fromIndex = dragging ? tools.indexOf(dragging) : -1;

  const rememberDrop = (index: number) => {
    setDropIndex((current) => (current === index ? current : index));
  };

  const trackDrag = (event: DragEvent<HTMLElement>) => {
    if (!draggingRef.current) {
      return;
    }
    event.preventDefault();
    event.dataTransfer.dropEffect = "move";
    rememberDrop(toolbarIndexAt(event.currentTarget, vertical ? event.clientY : event.clientX, vertical ? "y" : "x"));
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
    const index = toolbarIndexAt(event.currentTarget, vertical ? event.clientY : event.clientX, vertical ? "y" : "x");
    if (settings) {
      const next = reorderToolbarAt(all, tools, from, index);
      if (next) {
        void saveSettings({ ...settings, toolbarOrder: next });
      }
    }
    clearDrag();
  };

  if (tools.length === 0) {
    return null;
  }

  return (
    <aside
      id="tools-toolbar"
      ref={ref}
      aria-label="Tools"
      data-layout={layout}
      className={cn(
        "chrome-tool-strip flex shrink-0 bg-zinc-950",
        vertical
          ? "h-full min-h-0 flex-col overflow-hidden border-l border-zinc-800"
          : cn(
              "chrome-tool-strip-horizontal grid w-full grid-cols-[minmax(0,1fr)_minmax(0,max-content)_minmax(0,1fr)] items-center border-b border-zinc-800",
            ),
      )}
    >
      <div
        id="tools-toolbar-dock"
        className={cn(
          "flex shrink-0 items-center justify-center",
          vertical ? "h-[var(--toolbar-height)] border-b border-zinc-800" : "justify-self-start px-1",
        )}
      >
        <IconButton
          id="tools-toolbar-layout"
          label={vertical ? "Show above the bookmarks bar" : "Show on the right"}
          disabled={!settings}
          onClick={() => settings && void saveSettings({ ...settings, verticalToolbar: !vertical })}
        >
          {vertical ? (
            <PanelTop id="tools-toolbar-layout-icon" aria-hidden />
          ) : (
            <PanelRight id="tools-toolbar-layout-icon" aria-hidden />
          )}
        </IconButton>
      </div>
      <div
        id="tools-toolbar-items"
        className={cn(
          "flex min-h-0 min-w-0 items-center",
          vertical ? "flex-1 flex-col overflow-x-hidden overflow-y-auto py-1" : "min-w-0 flex-row justify-center gap-4 overflow-x-auto overflow-y-hidden px-1",
          labels && vertical && "gap-1",
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
        {tools.map((id, index) => (
          <ToolbarEntry
            key={id}
            id={id}
            idPrefix="tools"
            extension={toolbarExtensions.find((item) => item.id === id)}
            activeTab={activeTab}
            settings={settings}
            labeled={labels}
            layout={layout}
            toolbarIndex={index}
            dragging={dragging === id}
            drop={dropEdge(index, tools.length, fromIndex, dropIndex)}
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
      <div
        id="tools-toolbar-labels"
        className={cn(
          "flex shrink-0 items-center gap-1 border-zinc-800",
          vertical ? "flex-col border-t px-1 py-1.5" : "justify-self-end px-2",
        )}
      >
        <span id="tools-toolbar-labels-caption" className="text-[9px] leading-none text-zinc-500">
          Labels
        </span>
        <Toggle
          id="tools-toolbar-labels-toggle"
          size="sm"
          title={labels ? "Hide labels" : "Show labels"}
          aria-labelledby="tools-toolbar-labels-caption"
          checked={labels}
          disabled={!settings}
          onChange={(verticalToolbarLabels) => settings && void saveSettings({ ...settings, verticalToolbarLabels })}
        />
      </div>
    </aside>
  );
});
