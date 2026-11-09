import { forwardRef, useRef, useState, type DragEvent } from "react";
import { ListCollapse } from "lucide-react";
import type { BrowserSettings, ExtensionInfo, TabInfo, ToolbarSide } from "@type/browser";
import { ToolbarIcon } from "@features/BuiltInExtensions";
import { builtinTool } from "@features/BuiltInExtensions/builtin";
import { HostToolbarButton } from "@/features/Extensions/HostToolbarButton";
import { openExtensionPanel, openPopup, saveSettings } from "@/lib/browser-client";
import { extensionOpenAs } from "@/lib/extension-open";
import { useHostExtensions, useSettings } from "@/lib/use-browser";
import { cn } from "@/lib/cn";
import {
  openToolbarPinMenu,
  reorderToolbarAt,
  hostToolbarIds,
  splitToolbar,
  toolbarDragHandlers,
  toolbarDropEdge,
  toolbarIndexAt,
} from "@/lib/toolbar-pin";
import { toolbarSidePatch } from "@/lib/toolbar-side";
import { ExtensionIcon } from "@/components/ExtensionIcon";
import { IconButton } from "@/components/IconButton";
import { Toggle } from "@/components/Toggle";
import { ExtensionsToolbarScrollbar } from "./ExtensionsToolbarScrollbar";
import { ExtensionsToolbarSideControl } from "./ExtensionsToolbarSideControl";


type ExtensionToolbarButtonProps = {
  extension: ExtensionInfo;
  idPrefix: string;
};

export function ExtensionToolbarButton({ extension, idPrefix }: ExtensionToolbarButtonProps) {
  const settings = useSettings();
  const domId = `${idPrefix}-extension-${extension.id}`;
  const label = extension.defaultTitle || extension.name;
  return (
    <IconButton
      id={domId}
      label={label}
      onClick={(event) => {
        if (extension.popup && extensionOpenAs(settings, extension.id) === "panel") {
          void openExtensionPanel(extension.id);
          return;
        }
        if (extension.popup) {
          void openPopup("extension", event.currentTarget, { params: `id=${extension.id}` });
          return;
        }
        if (extension.sidePanel) {
          void openExtensionPanel(extension.id);
        }
      }}
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
  /** Column of icons, or a row. Address-bar icons omit this and stay in a row. */
  layout?: "vertical" | "horizontal";
  onMove?: (from: string, to: string) => void;
  /** Gap this icon shows while the list is being rearranged. */
  drop?: "before" | "after" | null;
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
  const hostExtensions = useHostExtensions().extensions;
  const host = hostExtensions.find((item) => `host:${item.id}` === id);
  const domId = `${idPrefix}-tool-${id}`;
  const caption = extension?.name ?? host?.name ?? builtinTool(id)?.name ?? "";
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
        labeled &&
          "group rounded-md hover:bg-zinc-800 hover:text-zinc-100 has-[[data-active=true]]:bg-zinc-800 has-[[data-active=true]]:text-zinc-100 [&_.chrome-toolbar-button]:rounded-md [&_.chrome-toolbar-button]:hover:bg-transparent has-[[data-active=true]]:[&_.chrome-toolbar-button]:bg-transparent",
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
      ) : host ? (
        <HostToolbarButton extension={host} idPrefix={idPrefix} />
      ) : (
        <ToolbarIcon id={id} activeTab={activeTab} settings={settings} />
      )}
      {labeled && caption ? (
        <span
          id={`${domId}-label`}
          className={cn(
            "pointer-events-none min-w-0 px-0.5 text-center text-[8px] leading-none text-zinc-400 group-hover:text-zinc-100 group-has-[[data-active=true]]:text-zinc-100",
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

type ExtensionsToolbarProps = {
  activeTab: TabInfo | undefined;
  extensions: ExtensionInfo[];
  settings: BrowserSettings | null;
  side: ToolbarSide;
};

/** Tools pinned as secondary, on one edge of the window. */
export const ExtensionsToolbar = forwardRef<HTMLElement, ExtensionsToolbarProps>(function ToolsToolbar(
  { activeTab, extensions, settings, side },
  ref,
) {
  const toolbarExtensions = extensions.filter((extension) => extension.enabled);
  const hostIds = hostToolbarIds(useHostExtensions().extensions);
  const { tools, all } = splitToolbar(settings, [...toolbarExtensions.map((extension) => extension.id), ...hostIds]);
  const labels = settings?.verticalToolbarLabels ?? false;
  const vertical = side === "left" || side === "right";
  const setToolbarSide = (next: ToolbarSide) => {
    if (settings) {
      void saveSettings({ ...settings, ...toolbarSidePatch(next) });
    }
  };
  const draggingRef = useRef<string | null>(null);
  const [scroller, setScroller] = useState<HTMLDivElement | null>(null);
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
      data-layout={vertical ? "vertical" : "horizontal"}
      data-side={side}
      className={cn(
        "chrome-tool-strip relative flex shrink-0 bg-zinc-950",
        side === "right" && "h-full min-h-0 flex-col overflow-hidden border-l border-zinc-800",
        side === "left" && "h-full min-h-0 flex-col overflow-hidden border-r border-zinc-800",
        (side === "top" || side === "bottom") &&
          cn(
            "chrome-tool-strip-horizontal grid w-full min-w-0 grid-cols-[auto_minmax(0,1fr)_auto] items-center overflow-hidden border-zinc-800",
            side === "top" ? "border-b" : "border-t",
            labels && "chrome-tool-strip-labeled",
          ),
      )}
    >
      <div
        id="tools-toolbar-dock"
        className={cn(
          "flex shrink-0 items-center justify-center gap-1",
          vertical
            ? "w-full flex-col border-b border-zinc-800 py-1"
            : "z-10 flex-row justify-self-start self-stretch bg-zinc-950 px-1",
        )}
      >
        <ExtensionsToolbarSideControl
          id="tools-toolbar-side"
          value={side}
          compact
          disabled={!settings}
          onChange={setToolbarSide}
        />
        <IconButton
          id="tools-toolbar-collapse"
          label="Collapse toolbar - Ctrl + Shift + T will show it again"
          disabled={!settings}
          onClick={() => settings && void saveSettings({ ...settings, showToolsToolbar: false })}
        >
          <ListCollapse id="tools-toolbar-collapse-icon" aria-hidden />
        </IconButton>
      </div>
      <div
        id="tools-toolbar-items"
        ref={setScroller}
        className={cn(
          "no-scrollbar flex min-h-0 min-w-0 items-center",
          vertical
            ? "flex-1 flex-col overflow-x-hidden overflow-y-auto py-1"
            : "min-w-0 flex-row gap-4 self-stretch overflow-x-auto overflow-y-hidden px-1 [justify-content:safe_center]",
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
            layout={vertical ? "vertical" : "horizontal"}
            toolbarIndex={index}
            dragging={dragging === id}
            drop={toolbarDropEdge(index, tools.length, fromIndex, dropIndex)}
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
          "flex shrink-0 gap-1 border-zinc-800",
          vertical
            ? "flex-col items-center border-t px-1 py-1.5"
            : "z-10 items-center justify-self-end self-stretch bg-zinc-950 px-2",
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
      <ExtensionsToolbarScrollbar scroller={scroller} axis={vertical ? "y" : "x"} items={tools.join("\0")} />
    </aside>
  );
});
