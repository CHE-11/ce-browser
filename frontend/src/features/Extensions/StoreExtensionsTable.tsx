import { Button } from "@/components/Button";
import { ExtensionIcon } from "@/components/ExtensionIcon";
import { OpenAs } from "@/components/OpenAs";
import { PanelSide } from "@/components/PanelSide";
import { PinPlacement } from "@/components/PinPlacement";
import { Skeleton } from "@/components/Skeleton";
import { Toggle } from "@/components/Toggle";
import { cn } from "@/lib/cn";
import { extensionOpenAs, extensionPanelResizable, extensionPanelSide, extensionPanelWidth, STORE_PANEL_WIDTH } from "@/lib/extension-open";
import { focusRing } from "@/lib/focus-ring";
import { toolbarPlacement } from "@/lib/toolbar-pin";
import type { BrowserSettings, ExtensionInfo, ExtensionOpenAs, HostPanelSide, ToolbarPlacement } from "@type/browser";
import { ChevronDown, ChevronUp, Settings2, Trash2 } from "lucide-react";
import { useState } from "react";
import { RemoveExtensionDialog } from "./RemoveExtensionDialog";

type SortColumn = "name" | "enabled";
type Sort = { column: SortColumn; direction: "asc" | "desc" };

function nextSort(current: Sort | null, column: SortColumn): Sort | null {
  if (current?.column !== column) {
    return { column, direction: "asc" };
  }
  if (current.direction === "asc") {
    return { column, direction: "desc" };
  }
  return null;
}

function sourceOf(extension: ExtensionInfo) {
  if (extension.store === "edge") {
    return { key: "edge", label: "Edge", className: "bg-teal-950 text-teal-200 ring-teal-800" };
  }
  if (extension.store === "chrome") {
    return { key: "chrome", label: "Chrome", className: "bg-sky-950 text-sky-200 ring-sky-700" };
  }
  return { key: "sideloaded", label: "Sideloaded", className: "bg-zinc-800 text-zinc-200 ring-zinc-600" };
}

function sorted(extensions: ExtensionInfo[], sort: Sort | null) {
  if (!sort) {
    return extensions;
  }
  const direction = sort.direction === "asc" ? 1 : -1;
  return [...extensions].sort((left, right) => {
    if (sort.column === "enabled" && left.enabled !== right.enabled) {
      return (left.enabled ? -1 : 1) * direction;
    }
    return left.name.localeCompare(right.name) * direction;
  });
}

function WidthField({
  id,
  value,
  disabled,
  onChange,
}: {
  id: string;
  value: number;
  disabled: boolean;
  onChange: (width: number) => void;
}) {
  const [draft, setDraft] = useState<string | null>(null);
  const shown = draft ?? String(value);
  const commit = () => {
    const next = Number(shown);
    setDraft(null);
    if (!Number.isFinite(next) || Math.round(next) === value) {
      return;
    }
    onChange(next);
  };
  return (
    <input
      id={id}
      type="number"
      min={STORE_PANEL_WIDTH.min}
      max={STORE_PANEL_WIDTH.max}
      value={shown}
      disabled={disabled}
      title={`Default panel width, ${STORE_PANEL_WIDTH.min} to ${STORE_PANEL_WIDTH.max} pixels`}
      className={cn(
        "h-7 w-20 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-xs text-zinc-100 outline-none focus:border-sky-500 disabled:opacity-40",
        focusRing,
      )}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={commit}
      onKeyDown={(event) => {
        if (event.key === "Enter") {
          event.currentTarget.blur();
        }
      }}
    />
  );
}

function SortHeader({
  id,
  label,
  column,
  sort,
  onSort,
  className,
}: {
  id: string;
  label: string;
  column: SortColumn;
  sort: Sort | null;
  onSort: (column: SortColumn) => void;
  className?: string;
}) {
  const active = sort?.column === column;
  const Icon = sort?.direction === "desc" ? ChevronDown : ChevronUp;
  return (
    <th id={id} scope="col" className={cn("px-3 py-2 font-medium", className)}>
      <button
        id={`${id}-button`}
        type="button"
        className={cn("inline-flex items-center gap-1 rounded text-left hover:text-zinc-200", focusRing)}
        aria-sort={active ? (sort.direction === "asc" ? "ascending" : "descending") : "none"}
        onClick={() => onSort(column)}
      >
        <span id={`${id}-label`}>{label}</span>
        {active ? <Icon id={`${id}-icon`} className="size-3" aria-hidden /> : null}
      </button>
    </th>
  );
}

export function StoreExtensionsTable({
  extensions,
  loaded,
  settings,
  busy,
  onPlacement,
  onOpenAs,
  onPanelSide,
  onPanelWidth,
  onPanelResizable,
  onEnabled,
  onOptions,
  onRemove,
  onUpdate,
}: {
  extensions: ExtensionInfo[];
  loaded: boolean;
  settings: BrowserSettings | null;
  busy: boolean;
  onPlacement: (id: string, placement: ToolbarPlacement) => void;
  onOpenAs: (id: string, mode: ExtensionOpenAs) => void;
  onPanelSide: (id: string, side: HostPanelSide) => void;
  onPanelWidth: (id: string, width: number) => void;
  onPanelResizable: (id: string, resizable: boolean) => void;
  onEnabled: (id: string, enabled: boolean) => void;
  onOptions: (id: string) => void;
  onRemove: (id: string) => void;
  onUpdate: (id: string) => void;
}) {
  const [sort, setSort] = useState<Sort | null>(null);
  const [pending, setPending] = useState<{ id: string; name: string } | null>(null);
  const list = sorted(loaded ? extensions : [], sort);

  return (
    <section id="store-extensions" className="overflow-hidden rounded-lg border border-zinc-800 bg-zinc-900">
      <div id="store-extensions-heading" className="border-b border-zinc-800 p-4">
        <h2 id="store-extensions-title" className="text-sm font-semibold">
          3rd Party Extension Store
        </h2>
        <p id="store-extensions-hint" className="mt-1 text-xs text-zinc-400">
          Installed from a Chrome Web Store or Edge Add-ons link, or loaded from a file or folder. A popup can open as a popover or a side panel. A panel opens at its default width, and Resizable lets you drag that edge.
        </p>
      </div>
      <div id="store-extensions-table-wrap" className="overflow-x-auto">
        <table id="store-extensions-table" className="w-full min-w-[64rem] table-fixed border-collapse text-left text-sm">
          <thead id="store-extensions-table-head">
            <tr id="store-extensions-table-head-row" className="border-b border-zinc-800 text-xs text-zinc-500">
              <SortHeader id="store-extensions-table-name" label="Extension" column="name" sort={sort} onSort={(column) => setSort((current) => nextSort(current, column))} />
              <th id="store-extensions-table-source" scope="col" className="w-28 px-3 py-2 font-medium">
                Source
              </th>
              <th id="store-extensions-table-open-as" scope="col" className="w-72 px-3 py-2 font-medium">
                Open as
              </th>
              <th id="store-extensions-table-placement" scope="col" className="w-28 px-3 py-2 font-medium">
                Placement
              </th>
              <th id="store-extensions-table-actions" scope="col" className="w-[80px] px-1 py-2 font-medium">
                Actions
              </th>
              <SortHeader
                id="store-extensions-table-enabled"
                label="Enabled"
                column="enabled"
                sort={sort}
                onSort={(column) => setSort((current) => nextSort(current, column))}
                className="w-28"
              />
            </tr>
          </thead>
          <tbody id="store-extensions-table-body">
            {!loaded
              ? [0, 1, 2].map((index) => (
                  <tr id={`store-extensions-table-loading-${index}`} key={`loading-${index}`} className="border-t border-zinc-800">
                    <td id={`store-extensions-table-loading-${index}-cell`} colSpan={6} className="px-3 py-2">
                      <Skeleton id={`store-extensions-table-loading-${index}-bar`} className="h-10 rounded-md" />
                    </td>
                  </tr>
                ))
              : null}
            {loaded && list.length === 0 ? (
              <tr id="store-extensions-empty-row">
                <td id="store-extensions-empty" colSpan={6} className="px-3 py-4 text-xs text-zinc-400">
                  No third-party extensions installed.
                </td>
              </tr>
            ) : null}
            {list.map((extension) => {
              const domId = `store-extension-${extension.id}`;
              const hasPopup = Boolean(extension.popup);
              const hasPanel = Boolean(extension.sidePanel);
              const panel = (hasPopup && extensionOpenAs(settings, extension.id) === "panel") || (!hasPopup && hasPanel);
              const resizable = extensionPanelResizable(settings, extension.id);
              const source = sourceOf(extension);
              return (
                <tr id={domId} key={extension.id} className={cn("border-t border-zinc-800", !extension.enabled && "opacity-70")}>
                  <td id={`${domId}-name`} className="px-3 py-2.5">
                    <div id={`${domId}-identity`} className="flex items-center gap-3">
                      <ExtensionIcon
                        id={`${domId}-icon`}
                        extensionId={extension.id}
                        name={extension.name}
                        src={extension.icon}
                        className="size-8"
                      />
                      <div id={`${domId}-text`} className="min-w-0">
                        <p id={`${domId}-title`} className="truncate font-medium text-zinc-100">
                          {extension.name}
                          <span id={`${domId}-version`} className="font-normal text-zinc-500">
                            {` (${extension.version})`}
                          </span>
                        </p>
                        <p id={`${domId}-description`} className="whitespace-normal text-xs text-zinc-400">
                          {extension.description}
                        </p>
                        <p id={`${domId}-id`} className="truncate font-mono text-[11px] text-zinc-600">
                          {extension.id}
                        </p>
                      </div>
                    </div>
                  </td>
                  <td id={`${domId}-source`} className="px-3 py-2.5">
                    <span
                      id={`${domId}-source-flag`}
                      className={cn("inline-flex items-center rounded px-1.5 py-0.5 text-[10px] font-medium ring-1", source.className)}
                    >
                      {source.label}
                    </span>
                  </td>
                  <td id={`${domId}-open-as`} className="px-3 py-2.5">
                    {hasPopup || hasPanel ? (
                      <div id={`${domId}-open-as-controls`} className="flex flex-col items-start gap-1.5">
                        <div id={`${domId}-open-as-row`} className="flex items-center gap-1">
                          {hasPopup ? (
                            <OpenAs
                              id={`${domId}-open-as-choice`}
                              value={extensionOpenAs(settings, extension.id)}
                              disabled={!settings || busy}
                              onChange={(mode) => onOpenAs(extension.id, mode)}
                            />
                          ) : (
                            <span id={`${domId}-open-as-side`} className="text-xs text-zinc-400">
                              Side panel
                            </span>
                          )}
                          {panel ? (
                            <PanelSide
                              id={`${domId}-side`}
                              side={extensionPanelSide(settings, extension.id)}
                              disabled={!settings || busy}
                              onChange={(side) => onPanelSide(extension.id, side)}
                            />
                          ) : null}
                        </div>
                        {panel ? (
                          <div id={`${domId}-resizable-row`} className="flex items-center gap-2">
                            <span id={`${domId}-resizable-label`} className="text-xs text-zinc-400">
                              Resizable
                            </span>
                            <Toggle
                              id={`${domId}-resizable-toggle`}
                              size="sm"
                              checked={resizable}
                              disabled={!settings || busy}
                              title={resizable ? "Drag to resize" : "Fixed width"}
                              onChange={(next) => onPanelResizable(extension.id, next)}
                            />
                            {resizable ? (
                              <WidthField
                                id={`${domId}-width-input`}
                                value={extensionPanelWidth(settings, extension.id)}
                                disabled={!settings || busy}
                                onChange={(width) => onPanelWidth(extension.id, width)}
                              />
                            ) : null}
                          </div>
                        ) : null}
                      </div>
                    ) : (
                      <span id={`${domId}-open-as-none`} className="text-xs text-zinc-500">
                        —
                      </span>
                    )}
                  </td>
                  <td id={`${domId}-placement`} className="px-3 py-2.5">
                    <PinPlacement
                      id={`${domId}-pin`}
                      placement={toolbarPlacement(settings, extension.id)}
                      disabled={!settings}
                      onChange={(placement) => onPlacement(extension.id, placement)}
                    />
                  </td>
                  <td id={`${domId}-actions`} className="w-[80px] px-1 py-2.5 align-middle">
                    <div id={`${domId}-action-row`} className="flex flex-wrap items-center gap-1">
                      {extension.updateVersion ? (
                        <Button id={`${domId}-update`} size="sm" variant="primary" disabled={busy} onClick={() => onUpdate(extension.id)}>
                          Update to {extension.updateVersion}
                        </Button>
                      ) : null}
                      {extension.optionsPage ? (
                        <Button id={`${domId}-options`} size="sm" variant="ghost" onClick={() => onOptions(extension.id)}>
                          <Settings2 id={`${domId}-options-icon`} className="size-3.5" aria-hidden />
                          Options
                        </Button>
                      ) : null}
                      <Button
                        id={`${domId}-remove`}
                        size="sm"
                        variant="destructive"
                        className="size-7 px-0"
                        aria-label="Remove"
                        title="Remove"
                        onClick={() => setPending({ id: extension.id, name: extension.name })}
                      >
                        <Trash2 id={`${domId}-remove-icon`} className="size-3.5" aria-hidden />
                      </Button>
                    </div>
                  </td>
                  <td id={`${domId}-enabled`} className="px-3 py-2.5 align-middle">
                    <Toggle
                      id={`${domId}-toggle`}
                      checked={extension.enabled}
                      title={extension.enabled ? "Disable" : "Enable"}
                      onChange={(enabled) => onEnabled(extension.id, enabled)}
                    />
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      <RemoveExtensionDialog
        id="store-extensions-remove-dialog"
        name={pending?.name ?? null}
        open={pending !== null}
        onCancel={() => setPending(null)}
        onConfirm={() => {
          if (!pending) {
            return;
          }
          onRemove(pending.id);
          setPending(null);
        }}
      />
    </section>
  );
}
