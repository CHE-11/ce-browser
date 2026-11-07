import { Button } from "@/components/Button";
import { ExtensionIcon, isComponentExtension } from "@/components/ExtensionIcon";
import { Skeleton } from "@/components/Skeleton";
import { Toggle } from "@/components/Toggle";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import { builtinToolEnabled, toolbarPlacement } from "@/lib/toolbar-pin";
import type { BrowserSettings, ExtensionInfo, ToolbarPlacement } from "@type/browser";
import { ChevronDown, ChevronUp, Settings2, Trash2 } from "lucide-react";
import { useState } from "react";
import { BUILTIN_TOOLS } from "../BuiltInExtensions/builtin";

type SourceFlag = {
  key: string;
  label: string;
  className: string;
};

type TableRow = {
  id: string;
  name: string;
  description: string;
  version: string | null;
  icon: string | null;
  builtinIcon: (typeof BUILTIN_TOOLS)[number]["icon"] | null;
  flags: SourceFlag[];
  builtin: boolean;
  enabled: boolean;
  optionsPage: string | null;
  updateVersion: string | null;
};

type SortColumn = "name" | "source" | "enabled";
type SortDirection = "asc" | "desc";
type Sort = { column: SortColumn; direction: SortDirection };

function flagTitle(key: string) {
  if (key === "builtin") return "Included with ce-browser";
  if (key === "third-party") return "Installed extension";
  if (key === "chrome") return "Chrome Web Store";
  if (key === "edge") return "Edge Add-ons";
  if (key === "sideloaded") return "Installed from a file or unpacked folder";
  if (key === "component") return "Ships with WebView2";
  return "";
}

const FLAG = {
  builtin: "bg-sky-950 text-sky-300 ring-sky-800",
  thirdParty: "bg-amber-950 text-amber-200 ring-amber-800",
  chrome: "bg-sky-950 text-sky-200 ring-sky-700",
  edge: "bg-teal-950 text-teal-200 ring-teal-800",
  sideloaded: "bg-zinc-800 text-zinc-200 ring-zinc-600",
  component: "bg-violet-950 text-violet-200 ring-violet-800",
};

function installedFlags(extension: ExtensionInfo): SourceFlag[] {
  if (isComponentExtension(extension.id, extension.name)) {
    return [{ key: "component", label: "Component", className: FLAG.component }];
  }
  const flags: SourceFlag[] = [{ key: "third-party", label: "3rd party", className: FLAG.thirdParty }];
  if (extension.store === "chrome") {
    flags.push({ key: "chrome", label: "Chrome", className: FLAG.chrome });
  } else if (extension.store === "edge") {
    flags.push({ key: "edge", label: "Edge", className: FLAG.edge });
  } else {
    flags.push({ key: "sideloaded", label: "Sideloaded", className: FLAG.sideloaded });
  }
  return flags;
}

function sourceLabel(row: TableRow) {
  return row.flags.map((flag) => flag.label).join(" ");
}

function compareRows(left: TableRow, right: TableRow, column: SortColumn) {
  if (column === "name") {
    return left.name.localeCompare(right.name, undefined, { sensitivity: "base" });
  }
  if (column === "source") {
    return sourceLabel(left).localeCompare(sourceLabel(right), undefined, { sensitivity: "base" });
  }
  return Number(left.enabled) - Number(right.enabled);
}

function sortedRows(list: TableRow[], sort: Sort | null) {
  if (!sort) {
    return list;
  }
  const sign = sort.direction === "asc" ? 1 : -1;
  return list
    .map((row, index) => ({ row, index }))
    .sort((left, right) => compareRows(left.row, right.row, sort.column) * sign || left.index - right.index)
    .map((item) => item.row);
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

function rows(extensions: ExtensionInfo[], settings: BrowserSettings | null): TableRow[] {
  const builtin: TableRow[] = BUILTIN_TOOLS.map((tool) => ({
    id: tool.id,
    name: tool.name,
    description: tool.description,
    version: null,
    icon: null,
    builtinIcon: tool.icon,
    flags: [{ key: "builtin", label: "Built-in", className: FLAG.builtin }],
    builtin: true,
    enabled: builtinToolEnabled(settings, tool.id),
    optionsPage: null,
    updateVersion: null,
  }));
  const installed: TableRow[] = extensions.map((extension) => ({
    id: extension.id,
    name: extension.name,
    description: extension.description || "No description.",
    version: extension.version || null,
    icon: extension.icon,
    builtinIcon: null,
    flags: installedFlags(extension),
    builtin: false,
    enabled: extension.enabled,
    optionsPage: extension.optionsPage,
    updateVersion: extension.updateVersion,
  }));
  return [...installed, ...builtin];
}

function PinPlacement({
  id,
  placement,
  disabled,
  onChange,
}: {
  id: string;
  placement: ToolbarPlacement;
  disabled?: boolean;
  onChange: (placement: ToolbarPlacement) => void;
}) {
  const unpinned = !placement.primary && !placement.secondary;
  const options = [
    { value: "primary" as const, label: "Primary", pressed: placement.primary, title: "Show next to the address bar" },
    { value: "secondary" as const, label: "Secondary", pressed: placement.secondary, title: "Show on the tools toolbar" },
    { value: "unpinned" as const, label: "Unpinned", pressed: unpinned, title: "Hide from both bars" },
  ];
  return (
    <div id={id} role="group" aria-label="Where to show this" className={cn("flex w-fit rounded-md border border-zinc-700 p-0.5", disabled && "opacity-40")}>
      {options.map((option) => (
        <button
          key={option.value}
          id={`${id}-${option.value}`}
          type="button"
          aria-pressed={option.pressed}
          disabled={disabled}
          title={option.title}
          className={cn(
            "rounded px-2 py-1 text-xs text-zinc-400 hover:text-zinc-100 disabled:hover:text-zinc-400",
            focusRing,
            option.pressed && "bg-zinc-700 text-zinc-100",
          )}
          onClick={() => {
            if (option.value === "unpinned") {
              if (!unpinned) {
                onChange({ primary: false, secondary: false });
              }
              return;
            }
            onChange({ ...placement, [option.value]: !placement[option.value] });
          }}
        >
          {option.label}
        </button>
      ))}
    </div>
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

export function ExtensionsTable({
  extensions,
  loaded,
  settings,
  busy,
  onPlacement,
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
  onEnabled: (id: string, enabled: boolean) => void;
  onOptions: (id: string) => void;
  onRemove: (id: string) => void;
  onUpdate: (id: string) => void;
}) {
  const [sort, setSort] = useState<Sort | null>(null);
  const list = sortedRows(rows(loaded ? extensions : [], settings), sort);

  return (
    <div id="extensions-table-wrap" className="overflow-x-auto rounded-lg border border-zinc-800 bg-zinc-900">
      <table id="extensions-table" className="w-full min-w-[64rem] border-collapse text-left text-sm">
        <thead id="extensions-table-head">
          <tr id="extensions-table-head-row" className="border-b border-zinc-800 text-xs text-zinc-500">
            <SortHeader id="extensions-table-name" label="Extension" column="name" sort={sort} onSort={(column) => setSort((current) => nextSort(current, column))} />
            <SortHeader id="extensions-table-source" label="Source" column="source" sort={sort} onSort={(column) => setSort((current) => nextSort(current, column))} className="w-44" />
            <th id="extensions-table-version" className="w-24 px-3 py-2 font-medium">
              Version
            </th>
            <th id="extensions-table-placement" className="w-64 px-3 py-2 font-medium">
              Placement
            </th>
            <SortHeader id="extensions-table-enabled" label="Enabled" column="enabled" sort={sort} onSort={(column) => setSort((current) => nextSort(current, column))} className="w-28" />
            <th id="extensions-table-actions" className="px-3 py-2 font-medium">
              Actions
            </th>
          </tr>
        </thead>
        <tbody id="extensions-table-body">
          {!loaded
            ? [0, 1, 2].map((index) => (
                <tr id={`extensions-table-loading-${index}`} key={`loading-${index}`} className="border-t border-zinc-800">
                  <td id={`extensions-table-loading-${index}-cell`} colSpan={6} className="px-3 py-2">
                    <Skeleton id={`extensions-table-loading-${index}-bar`} className="h-10 rounded-md" />
                  </td>
                </tr>
              ))
            : null}
          {list.map((row) => {
            const domId = `extension-${row.id}`;
            const Icon = row.builtinIcon;
            return (
              <tr id={domId} key={row.id} className={cn("border-t border-zinc-800", !row.enabled && "opacity-70")}>
                <td id={`${domId}-name`} className="px-3 py-2.5">
                  <div id={`${domId}-identity`} className="flex items-center gap-3">
                    {Icon ? (
                      <Icon id={`${domId}-icon`} className="size-8 shrink-0 text-zinc-300" aria-hidden />
                    ) : (
                      <ExtensionIcon
                        id={`${domId}-icon`}
                        extensionId={row.id}
                        name={row.name}
                        src={row.icon}
                        className="size-8"
                      />
                    )}
                    <div id={`${domId}-text`} className="min-w-0">
                      <p id={`${domId}-title`} className="truncate font-medium text-zinc-100">
                        {row.name}
                      </p>
                      <p id={`${domId}-description`} className="line-clamp-1 text-xs text-zinc-400">
                        {row.description}
                      </p>
                      {!row.builtin ? (
                        <p id={`${domId}-id`} className="truncate font-mono text-[11px] text-zinc-600">
                          {row.id}
                        </p>
                      ) : null}
                    </div>
                  </div>
                </td>
                <td id={`${domId}-source`} className="px-3 py-2.5">
                  <div id={`${domId}-flags`} className="flex flex-wrap gap-1">
                    {row.flags.map((flag) => (
                      <span
                        id={`${domId}-flag-${flag.key}`}
                        key={flag.key}
                        title={flagTitle(flag.key)}
                        className={cn("inline-flex items-center rounded px-1.5 py-0.5 text-[10px] font-medium ring-1", flag.className)}
                      >
                        {flag.label}
                      </span>
                    ))}
                  </div>
                </td>
                <td id={`${domId}-version`} className="px-3 py-2.5 font-mono text-xs text-zinc-400">
                  {row.version ?? "—"}
                </td>
                <td id={`${domId}-placement`} className="px-3 py-2.5">
                  <PinPlacement
                    id={`${domId}-pin`}
                    placement={toolbarPlacement(settings, row.id)}
                    disabled={!settings}
                    onChange={(placement) => onPlacement(row.id, placement)}
                  />
                </td>
                <td id={`${domId}-enabled`} className="px-3 py-2.5">
                  <Toggle
                    id={`${domId}-toggle`}
                    checked={row.enabled}
                    disabled={row.builtin && !settings}
                    title={row.enabled ? "Disable" : "Enable"}
                    onChange={(enabled) => onEnabled(row.id, enabled)}
                  />
                </td>
                <td id={`${domId}-actions`} className="px-3 py-2.5">
                  <div id={`${domId}-action-row`} className="flex flex-wrap items-center gap-1">
                    {row.updateVersion ? (
                      <Button id={`${domId}-update`} size="sm" variant="primary" disabled={busy} onClick={() => onUpdate(row.id)}>
                        Update to {row.updateVersion}
                      </Button>
                    ) : null}
                    {row.optionsPage ? (
                      <Button id={`${domId}-options`} size="sm" variant="ghost" onClick={() => onOptions(row.id)}>
                        <Settings2 id={`${domId}-options-icon`} className="size-3.5" aria-hidden />
                        Options
                      </Button>
                    ) : null}
                    {!row.builtin ? (
                      <Button id={`${domId}-remove`} size="sm" variant="ghost" onClick={() => onRemove(row.id)}>
                        <Trash2 id={`${domId}-remove-icon`} className="size-3.5" aria-hidden />
                        Remove
                      </Button>
                    ) : null}
                  </div>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
