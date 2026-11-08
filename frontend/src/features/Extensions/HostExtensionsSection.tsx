import { ChevronDown, ChevronUp, FolderOpen, Trash2 } from "lucide-react";
import { useState } from "react";
import type { HostExtensionInfo, HostPanelSide, ToolbarPlacement, ToolbarSide } from "@type/browser";
import { Button } from "@/components/Button";
import { ExtensionIcon } from "@/components/ExtensionIcon";
import { PinPlacement } from "@/components/PinPlacement";
import { Skeleton } from "@/components/Skeleton";
import { Toggle } from "@/components/Toggle";
import { installHostExtensionFolder, removeHostExtension, setHostExtensionEnabled, setHostPageToolbarOpen, setHostPageToolbarSide, setHostPanelLayout } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import { saveToolbarPlacement, toolbarPlacement } from "@/lib/toolbar-pin";
import { useSettings } from "@/lib/use-browser";
import { ExtensionsToolbarSideControl } from "./ExtensionsToolbarSideControl";
import { RemoveExtensionDialog } from "./RemoveExtensionDialog";

type HostExtensionsSectionProps = {
  extensions: HostExtensionInfo[];
  loaded: boolean;
  busy: boolean;
  onStatus: (status: { tone: "info" | "error"; text: string } | null) => void;
  onBusy: (busy: boolean) => void;
};

type SortColumn = "name" | "enabled";
type SortDirection = "asc" | "desc";
type Sort = { column: SortColumn; direction: SortDirection };

function compareExtensions(left: HostExtensionInfo, right: HostExtensionInfo, column: SortColumn) {
  if (column === "name") {
    return left.name.localeCompare(right.name, undefined, { sensitivity: "base" });
  }
  return Number(left.enabled) - Number(right.enabled);
}

function sortedExtensions(list: HostExtensionInfo[], sort: Sort | null) {
  if (!sort) {
    return list;
  }
  const sign = sort.direction === "asc" ? 1 : -1;
  return list
    .map((extension, index) => ({ extension, index }))
    .sort((left, right) => compareExtensions(left.extension, right.extension, sort.column) * sign || left.index - right.index)
    .map((item) => item.extension);
}

function nextSort(current: Sort | null, column: SortColumn): Sort | null {
  if (current?.column !== column) {
    return { column, direction: "asc" };
  }
  if (current.direction === "asc") {
    return { column, direction: "desc" };
  }
  return null;
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
  const Icon = active && sort.direction === "desc" ? ChevronDown : ChevronUp;
  return (
    <th id={id} scope="col" aria-sort={active ? (sort.direction === "asc" ? "ascending" : "descending") : "none"} className={cn("px-3 py-2 font-medium", className)}>
      <button
        id={`${id}-sort`}
        type="button"
        className={cn("inline-flex items-center gap-1 rounded text-left text-zinc-500 hover:text-zinc-200", focusRing, active && "text-zinc-100")}
        onClick={() => onSort(column)}
      >
        {label}
        <Icon id={`${id}-sort-icon`} className={cn("size-3", active ? "text-zinc-200" : "text-zinc-600")} aria-hidden />
      </button>
    </th>
  );
}

export function HostExtensionsSection({ extensions, loaded, busy, onStatus, onBusy }: HostExtensionsSectionProps) {
  const settings = useSettings();
  const [sort, setSort] = useState<Sort | null>(null);
  const [pending, setPending] = useState<{ id: string; name: string } | null>(null);
  const list = sortedExtensions(loaded ? extensions : [], sort);

  const install = async () => {
    onBusy(true);
    onStatus(null);
    try {
      const extension = await installHostExtensionFolder();
      if (extension) {
        onStatus({ tone: "info", text: `Installed ${extension.name} ${extension.version}.` });
      }
    } catch (reason) {
      onStatus({ tone: "error", text: String(reason) });
    } finally {
      onBusy(false);
    }
  };

  const fail = (action: Promise<unknown>) => {
    action.catch((reason: unknown) => onStatus({ tone: "error", text: String(reason) }));
  };

  const setPlacement = (id: string, placement: ToolbarPlacement) => {
    if (settings) {
      fail(saveToolbarPlacement(settings, `host:${id}`, placement));
    }
  };

  const onSort = (column: SortColumn) => setSort((current) => nextSort(current, column));

  return (
    <section id="host-extensions" className="overflow-hidden rounded-lg border border-zinc-800 bg-zinc-900">
      <div id="host-extensions-heading" className="flex items-start justify-between gap-4 border-b border-zinc-800 p-4">
        <div id="host-extensions-heading-text">
          <h2 id="host-extensions-title" className="text-sm font-semibold">
            Host extensions
          </h2>
          <p id="host-extensions-hint" className="mt-1 text-xs text-zinc-400">
            A folder with a manifest. It can add a toolbar button, a popover, a side panel, a page toolbar, a window, and full pages.
          </p>
        </div>
        <Button id="host-extensions-install" variant="outline" disabled={busy} onClick={() => void install()}>
          <FolderOpen id="host-extensions-install-icon" className="size-4" aria-hidden />
          <span id="host-extensions-install-label" className="text-nowrap sm:block">
            Load folder
          </span>
        </Button>
      </div>
      <div id="host-extensions-table-wrap" className="overflow-x-auto">
        <table id="host-extensions-table" className="w-full min-w-[52rem] table-fixed border-collapse text-left text-sm">
          <thead id="host-extensions-table-head">
            <tr id="host-extensions-table-head-row" className="border-b border-zinc-800 text-xs text-zinc-500">
              <SortHeader id="host-extensions-table-name" label="Extension" column="name" sort={sort} onSort={onSort} />
              <th id="host-extensions-table-panel" scope="col" className="w-20 px-3 py-2 font-medium">
                Panel
              </th>
              <th id="host-extensions-table-page-toolbar" scope="col" className="w-40 whitespace-nowrap px-3 py-2 font-medium">
                Page toolbar
              </th>
              <th id="host-extensions-table-placement" scope="col" className="w-28 px-3 py-2 font-medium">
                Placement
              </th>
              <th id="host-extensions-table-actions" scope="col" className="w-[80px] px-1 py-2 font-medium">
                Actions
              </th>
              <SortHeader id="host-extensions-table-enabled" label="Enabled" column="enabled" sort={sort} onSort={onSort} className="w-28" />
            </tr>
          </thead>
          <tbody id="host-extensions-table-body">
            {!loaded
              ? [0, 1, 2].map((index) => (
                  <tr id={`host-extensions-table-loading-${index}`} key={`loading-${index}`} className="border-t border-zinc-800">
                    <td id={`host-extensions-table-loading-${index}-cell`} colSpan={6} className="px-3 py-2">
                      <Skeleton id={`host-extensions-table-loading-${index}-bar`} className="h-10 rounded-md" />
                    </td>
                  </tr>
                ))
              : null}
            {loaded && list.length === 0 ? (
              <tr id="host-extensions-empty-row">
                <td id="host-extensions-empty" colSpan={6} className="px-3 py-4 text-xs text-zinc-400">
                  No host extensions installed.
                </td>
              </tr>
            ) : null}
            {list.map((extension) => {
              const domId = `host-extension-${extension.id}`;
              return (
                <tr id={domId} key={extension.id} className={cn("border-t border-zinc-800", !extension.enabled && "opacity-70")}>
                  <td id={`${domId}-name-cell`} className="px-3 py-2.5">
                    <div id={`${domId}-identity`} className="flex items-center gap-3">
                      <ExtensionIcon id={`${domId}-icon`} extensionId={extension.id} name={extension.name} src={extension.icon} className="size-8" />
                      <div id={`${domId}-text`} className="min-w-0">
                        <p id={`${domId}-name`} className="truncate font-medium text-zinc-100">
                          {extension.name}
                        </p>
                        <p id={`${domId}-permissions`} className="truncate text-xs text-zinc-400" title={extension.permissions.join(", ")}>
                          {extension.permissions.length > 0 ? extension.permissions.join(", ") : "No extra permissions"}
                        </p>
                        <p id={`${domId}-description`} className="whitespace-normal text-xs text-zinc-500">
                          {extension.description || "No description."}
                        </p>
                        <p id={`${domId}-id`} className="truncate font-mono text-[11px] text-zinc-600">
                          {extension.version ? `${extension.id} (${extension.version})` : extension.id}
                        </p>
                      </div>
                    </div>
                  </td>
                  <td id={`${domId}-panel`} className="px-3 py-2.5">
                    {extension.panel ? (
                      <PanelSide
                        id={`${domId}-side`}
                        side={extension.panel.side}
                        disabled={busy}
                        onChange={(side) => fail(setHostPanelLayout(extension.id, side))}
                      />
                    ) : (
                      <span id={`${domId}-panel-none`} className="text-xs text-zinc-500">
                        —
                      </span>
                    )}
                  </td>
                  <td id={`${domId}-page-toolbar`} className="px-3 py-2.5">
                    {extension.pageToolbar ? (
                      <div id={`${domId}-page-toolbar-controls`} className="flex items-center gap-2">
                        <ExtensionsToolbarSideControl
                          id={`${domId}-page-toolbar-side`}
                          value={extension.pageToolbar.side}
                          compact
                          disabled={busy || !extension.enabled}
                          onChange={(side: ToolbarSide) => fail(setHostPageToolbarSide(extension.id, side))}
                        />
                        {extension.pageToolbar.open ? null : (
                          <Button
                            id={`${domId}-page-toolbar-show`}
                            size="xs"
                            variant="outline"
                            disabled={busy || !extension.enabled}
                            onClick={() => fail(setHostPageToolbarOpen(extension.id, true))}
                          >
                            Show
                          </Button>
                        )}
                      </div>
                    ) : (
                      <span id={`${domId}-page-toolbar-none`} className="text-xs text-zinc-500">
                        —
                      </span>
                    )}
                  </td>
                  <td id={`${domId}-placement`} className="px-3 py-2.5 align-middle">
                    {extension.surfaces.toolbar ? (
                      <PinPlacement
                        id={`${domId}-pin`}
                        placement={toolbarPlacement(settings, `host:${extension.id}`)}
                        disabled={!settings}
                        onChange={(placement) => setPlacement(extension.id, placement)}
                      />
                    ) : (
                      <span id={`${domId}-placement-none`} className="text-xs text-zinc-500">
                        —
                      </span>
                    )}
                  </td>
                  <td id={`${domId}-actions`} className="w-[80px] px-1 py-2.5 align-middle">
                    <div id={`${domId}-action-row`} className="flex flex-wrap items-center gap-1">
                      <Button
                        id={`${domId}-remove`}
                        size="sm"
                        variant="destructive"
                        className="size-7 px-0"
                        disabled={busy}
                        aria-label="Remove"
                        title="Remove"
                        onClick={() => setPending({ id: extension.id, name: extension.name })}
                      >
                        <Trash2 id={`${domId}-remove-icon`} className="size-3.5" aria-hidden />
                      </Button>
                    </div>
                  </td>
                  <td id={`${domId}-enabled-cell`} className="px-3 py-2.5 align-middle">
                    <Toggle
                      id={`${domId}-enabled`}
                      checked={extension.enabled}
                      disabled={busy}
                      title={extension.enabled ? "Disable" : "Enable"}
                      onChange={(enabled) => fail(setHostExtensionEnabled(extension.id, enabled))}
                    />
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      <RemoveExtensionDialog
        id="host-extensions-remove-dialog"
        name={pending?.name ?? null}
        open={pending !== null}
        onCancel={() => setPending(null)}
        onConfirm={() => {
          if (!pending) {
            return;
          }
          const id = pending.id;
          setPending(null);
          fail(removeHostExtension(id));
        }}
      />
    </section>
  );
}

function PanelSide({
  id,
  side,
  disabled,
  onChange,
}: {
  id: string;
  side: HostPanelSide;
  disabled: boolean;
  onChange: (side: HostPanelSide) => void;
}) {
  const options = [
    { value: "left" as const, label: "L", name: "Left" },
    { value: "right" as const, label: "R", name: "Right" },
  ];
  return (
    <div id={id} role="group" aria-label="Panel side" className={cn("flex w-fit rounded-md border border-zinc-700 p-0.5", disabled && "opacity-40")}>
      {options.map((option) => (
        <button
          key={option.value}
          id={`${id}-${option.value}`}
          type="button"
          aria-pressed={side === option.value}
          aria-label={option.name}
          title={option.name}
          disabled={disabled}
          className={cn(
            "grid size-6 place-items-center rounded text-xs text-zinc-400 hover:text-zinc-100 disabled:hover:text-zinc-400",
            focusRing,
            side === option.value && "bg-zinc-700 text-zinc-100",
          )}
          onClick={() => {
            if (side !== option.value) {
              onChange(option.value);
            }
          }}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}
