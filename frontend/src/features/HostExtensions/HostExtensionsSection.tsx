import { ChevronDown, ChevronUp, FolderOpen, Trash2 } from "lucide-react";
import { Fragment, useEffect, useState } from "react";
import type { HostExtensionInfo, HostPanelLayout, ToolbarPlacement } from "@type/browser";
import { Button } from "@/components/Button";
import { ExtensionIcon } from "@/components/ExtensionIcon";
import { PinPlacement } from "@/components/PinPlacement";
import { Skeleton } from "@/components/Skeleton";
import { Toggle } from "@/components/Toggle";
import { installHostExtensionFolder, openHostPopout, removeHostExtension, setHostExtensionEnabled, setHostPanelLayout } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import { saveToolbarPlacement, toolbarPlacement } from "@/lib/toolbar-pin";
import { useSettings } from "@/lib/use-browser";

type HostExtensionsSectionProps = {
  extensions: HostExtensionInfo[];
  loaded: boolean;
  busy: boolean;
  onStatus: (status: { tone: "info" | "error"; text: string } | null) => void;
  onBusy: (busy: boolean) => void;
};

type SortColumn = "name" | "permissions" | "enabled";
type SortDirection = "asc" | "desc";
type Sort = { column: SortColumn; direction: SortDirection };

function permissionLabel(extension: HostExtensionInfo) {
  return extension.permissions.join(", ");
}

function compareExtensions(left: HostExtensionInfo, right: HostExtensionInfo, column: SortColumn) {
  if (column === "name") {
    return left.name.localeCompare(right.name, undefined, { sensitivity: "base" });
  }
  if (column === "permissions") {
    return permissionLabel(left).localeCompare(permissionLabel(right), undefined, { sensitivity: "base" });
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
            A folder with a manifest. It can add a toolbar button, a popover, a side panel, a window, and full pages.
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
        <table id="host-extensions-table" className="w-full min-w-[68rem] border-collapse text-left text-sm">
          <thead id="host-extensions-table-head">
            <tr id="host-extensions-table-head-row" className="border-b border-zinc-800 text-xs text-zinc-500">
              <SortHeader id="host-extensions-table-name" label="Extension" column="name" sort={sort} onSort={onSort} />
              <SortHeader id="host-extensions-table-permissions" label="Permissions" column="permissions" sort={sort} onSort={onSort} className="w-64" />
              <th id="host-extensions-table-version" scope="col" className="w-24 px-3 py-2 font-medium">
                Version
              </th>
              <th id="host-extensions-table-placement" scope="col" className="w-64 px-3 py-2 font-medium">
                Placement
              </th>
              <SortHeader id="host-extensions-table-enabled" label="Enabled" column="enabled" sort={sort} onSort={onSort} className="w-28" />
              <th id="host-extensions-table-actions" scope="col" className="px-3 py-2 font-medium">
                Actions
              </th>
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
                <Fragment key={extension.id}>
                <tr id={domId} className={cn("border-t border-zinc-800", !extension.enabled && "opacity-70")}>
                  <td id={`${domId}-name-cell`} className="px-3 py-2.5">
                    <div id={`${domId}-identity`} className="flex items-center gap-3">
                      <ExtensionIcon id={`${domId}-icon`} extensionId={extension.id} name={extension.name} src={extension.icon} className="size-8" />
                      <div id={`${domId}-text`} className="min-w-0">
                        <p id={`${domId}-name`} className="truncate font-medium text-zinc-100">
                          {extension.name}
                        </p>
                        <p id={`${domId}-description`} className="line-clamp-1 text-xs text-zinc-400">
                          {extension.description || "No description."}
                        </p>
                        <p id={`${domId}-id`} className="truncate font-mono text-[11px] text-zinc-600">
                          {extension.id}
                        </p>
                      </div>
                    </div>
                  </td>
                  <td id={`${domId}-permissions`} className="px-3 py-2.5">
                    {extension.permissions.length > 0 ? (
                      <div id={`${domId}-permission-flags`} className="flex flex-wrap gap-1">
                        {extension.permissions.map((permission) => (
                          <span
                            id={`${domId}-permission-${permission}`}
                            key={permission}
                            className="inline-flex items-center rounded bg-zinc-800 px-1.5 py-0.5 text-[10px] font-medium text-zinc-200 ring-1 ring-zinc-600"
                          >
                            {permission}
                          </span>
                        ))}
                      </div>
                    ) : (
                      <span id={`${domId}-permissions-none`} className="text-xs text-zinc-500">
                        No extra permissions
                      </span>
                    )}
                  </td>
                  <td id={`${domId}-version`} className="px-3 py-2.5 font-mono text-xs text-zinc-400">
                    {extension.version || "—"}
                  </td>
                  <td id={`${domId}-placement`} className="px-3 py-2.5">
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
                  <td id={`${domId}-enabled-cell`} className="px-3 py-2.5">
                    <Toggle
                      id={`${domId}-enabled`}
                      checked={extension.enabled}
                      disabled={busy}
                      title={extension.enabled ? "Disable" : "Enable"}
                      onChange={(enabled) => fail(setHostExtensionEnabled(extension.id, enabled))}
                    />
                  </td>
                  <td id={`${domId}-actions`} className="px-3 py-2.5">
                    <div id={`${domId}-action-row`} className="flex flex-wrap items-center gap-1">
                      {extension.surfaces.popout ? (
                        <Button
                          id={`${domId}-popout`}
                          size="sm"
                          variant="outline"
                          disabled={busy || !extension.enabled}
                          onClick={() => fail(openHostPopout(extension.id))}
                        >
                          Open window
                        </Button>
                      ) : null}
                      <Button
                        id={`${domId}-remove`}
                        size="sm"
                        variant="ghost"
                        disabled={busy}
                        onClick={() => fail(removeHostExtension(extension.id))}
                      >
                        <Trash2 id={`${domId}-remove-icon`} className="size-3.5" aria-hidden />
                        Remove
                      </Button>
                    </div>
                  </td>
                </tr>
                {extension.panel ? (
                  <tr id={`${domId}-layout-row`} className="border-t border-zinc-800/70">
                    <td id={`${domId}-layout`} colSpan={6} className="px-3 py-2">
                      <PanelControls extension={extension} busy={busy} onFail={fail} />
                    </td>
                  </tr>
                ) : null}
                </Fragment>
              );
            })}
          </tbody>
        </table>
      </div>
    </section>
  );
}

function PanelControls({
  extension,
  busy,
  onFail,
}: {
  extension: HostExtensionInfo;
  busy: boolean;
  onFail: (action: Promise<unknown>) => void;
}) {
  const panel = extension.panel;
  const [minWidth, setMinWidth] = useState(panel?.minWidth ?? 240);
  const [maxWidth, setMaxWidth] = useState(panel?.maxWidth ?? 720);
  const [defaultWidth, setDefaultWidth] = useState(panel?.defaultWidth ?? 360);
  useEffect(() => {
    if (!panel) {
      return;
    }
    setMinWidth(panel.minWidth);
    setMaxWidth(panel.maxWidth);
    setDefaultWidth(panel.defaultWidth);
  }, [extension.id, panel]);

  if (!panel) {
    return null;
  }

  const save = (next: HostPanelLayout) => {
    const ordered = next.resizable
      ? orderedWidths(next.minWidth, next.defaultWidth, next.maxWidth)
      : {
          minWidth: clampWidth(next.minWidth),
          maxWidth: Math.max(clampWidth(next.minWidth), clampWidth(next.maxWidth)),
          defaultWidth: clampWidth(next.defaultWidth),
        };
    const layout = { ...next, ...ordered };
    if (
      layout.side === panel.side &&
      layout.resizable === panel.resizable &&
      layout.minWidth === panel.minWidth &&
      layout.maxWidth === panel.maxWidth &&
      layout.defaultWidth === panel.defaultWidth
    ) {
      setMinWidth(layout.minWidth);
      setMaxWidth(layout.maxWidth);
      setDefaultWidth(layout.defaultWidth);
      return;
    }
    onFail(setHostPanelLayout(extension.id, layout));
  };

  const domId = `host-extension-${extension.id}`;
  return (
    <div id={`${domId}-panel`} className="flex flex-wrap items-center gap-x-6 gap-y-2">
      <div id={`${domId}-side`} className="flex items-center justify-between gap-3">
        <span id={`${domId}-side-label`} className="text-xs text-zinc-400">
          Panel side
        </span>
        <div id={`${domId}-side-choices`} className="flex gap-1">
          <SideButton
            id={`${domId}-side-left`}
            pressed={panel.side === "left"}
            disabled={busy}
            onClick={() => save({ ...panel, side: "left", minWidth, maxWidth, defaultWidth })}
          >
            Left
          </SideButton>
          <SideButton
            id={`${domId}-side-right`}
            pressed={panel.side === "right"}
            disabled={busy}
            onClick={() => save({ ...panel, side: "right", minWidth, maxWidth, defaultWidth })}
          >
            Right
          </SideButton>
        </div>
      </div>
      <div id={`${domId}-resizable`} className="flex items-center justify-between gap-3">
        <span id={`${domId}-resizable-label`} className="text-xs text-zinc-400">
          Resizable
        </span>
        <Toggle
          id={`${domId}-resizable-toggle`}
          size="sm"
          checked={panel.resizable}
          disabled={busy}
          onChange={(resizable) => save({ ...panel, resizable, minWidth, maxWidth, defaultWidth })}
        />
      </div>
      <div id={`${domId}-widths`} className="flex flex-wrap gap-2">
        {panel.resizable ? (
          <WidthField id={`${domId}-min`} label="Min" value={minWidth} disabled={busy} onChange={setMinWidth} onCommit={() => save({ ...panel, minWidth, maxWidth, defaultWidth })} />
        ) : null}
        <WidthField
          id={`${domId}-default`}
          label={panel.resizable ? "Default" : "Width"}
          value={defaultWidth}
          disabled={busy}
          onChange={setDefaultWidth}
          onCommit={() => save({ ...panel, minWidth, maxWidth, defaultWidth })}
        />
        {panel.resizable ? (
          <WidthField id={`${domId}-max`} label="Max" value={maxWidth} disabled={busy} onChange={setMaxWidth} onCommit={() => save({ ...panel, minWidth, maxWidth, defaultWidth })} />
        ) : null}
      </div>
    </div>
  );
}

function SideButton({
  id,
  pressed,
  disabled,
  onClick,
  children,
}: {
  id: string;
  pressed: boolean;
  disabled: boolean;
  onClick: () => void;
  children: string;
}) {
  return (
    <button
      id={id}
      type="button"
      aria-pressed={pressed}
      disabled={disabled}
      className={cn(
        "rounded-md px-2 py-1 text-xs disabled:opacity-40",
        pressed ? "bg-sky-600 text-white" : "bg-zinc-800 text-zinc-300",
      )}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

function WidthField({
  id,
  label,
  value,
  disabled,
  onChange,
  onCommit,
}: {
  id: string;
  label: string;
  value: number;
  disabled: boolean;
  onChange: (value: number) => void;
  onCommit: () => void;
}) {
  return (
    <label id={`${id}-field`} className="flex items-center gap-1 text-xs text-zinc-400">
      <span id={`${id}-label`}>{label}</span>
      <input
        id={id}
        type="number"
        min={160}
        max={1600}
        disabled={disabled}
        value={value}
        className="w-16 rounded border border-zinc-700 bg-zinc-950 px-1.5 py-1 text-xs text-zinc-100"
        onChange={(event) => onChange(Number(event.target.value))}
        onBlur={onCommit}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.currentTarget.blur();
          }
        }}
      />
    </label>
  );
}

function clampWidth(value: number) {
  return Math.min(1600, Math.max(160, Math.round(Number.isFinite(value) ? value : 360)));
}

function orderedWidths(minWidth: number, defaultWidth: number, maxWidth: number) {
  const clamp = clampWidth;
  let min = clamp(minWidth);
  let max = clamp(maxWidth);
  if (max < min) {
    max = min;
  }
  let width = clamp(defaultWidth);
  if (width < min) {
    width = min;
  }
  if (width > max) {
    width = max;
  }
  return { minWidth: min, maxWidth: max, defaultWidth: width };
}
