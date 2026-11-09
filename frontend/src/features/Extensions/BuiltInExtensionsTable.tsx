import { ExtensionIcon, isComponentExtension } from "@/components/ExtensionIcon";
import { PinPlacement } from "@/components/PinPlacement";
import { Skeleton } from "@/components/Skeleton";
import { Toggle } from "@/components/Toggle";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import { builtinToolEnabled, toolbarPlacement } from "@/lib/toolbar-pin";
import type { BrowserSettings, ExtensionInfo, ToolbarPlacement } from "@type/browser";
import { ChevronDown, ChevronUp } from "lucide-react";
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
  if (key === "dev") return "Developer tool. It runs in dev builds and in release builds.";
  return "";
}

const FLAG = {
  builtin: "bg-sky-950 text-sky-300 ring-sky-800",
  thirdParty: "bg-amber-950 text-amber-200 ring-amber-800",
  chrome: "bg-sky-950 text-sky-200 ring-sky-700",
  edge: "bg-teal-950 text-teal-200 ring-teal-800",
  sideloaded: "bg-zinc-800 text-zinc-200 ring-zinc-600",
  component: "bg-violet-950 text-violet-200 ring-violet-800",
  dev: "bg-fuchsia-950 text-fuchsia-200 ring-fuchsia-800",
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
    flags: [
      { key: "builtin", label: "Built-in", className: FLAG.builtin },
      ...(tool.dev ? [{ key: "dev", label: "Dev tool", className: FLAG.dev }] : []),
    ],
    builtin: true,
    enabled: builtinToolEnabled(settings, tool.id),
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
  }));
  return [...installed, ...builtin];
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

export function BuiltInExtensionsTable({
  extensions,
  loaded,
  settings,
  onPlacement,
  onEnabled,
}: {
  extensions: ExtensionInfo[];
  loaded: boolean;
  settings: BrowserSettings | null;
  onPlacement: (id: string, placement: ToolbarPlacement) => void;
  onEnabled: (id: string, enabled: boolean) => void;
}) {
  const [sort, setSort] = useState<Sort | null>(null);
  const list = sortedRows(rows(loaded ? extensions : [], settings), sort);

  return (
    <section id="builtin-extensions" className="overflow-hidden rounded-lg border border-zinc-800 bg-zinc-900">
      <div id="builtin-extensions-heading" className="border-b border-zinc-800 p-4">
        <h2 id="builtin-extensions-title" className="text-sm font-semibold">
          Built-in extensions
        </h2>
        <p id="builtin-extensions-hint" className="mt-1 text-xs text-zinc-400">
          Tools that ship with the browser.
        </p>
      </div>
      <div id="extensions-table-wrap" className="overflow-x-auto">
      <table id="extensions-table" className="w-full min-w-[52rem] table-fixed border-collapse text-left text-sm">
        <thead id="extensions-table-head">
          <tr id="extensions-table-head-row" className="border-b border-zinc-800 text-xs text-zinc-500">
            <SortHeader id="extensions-table-name" label="Extension" column="name" sort={sort} onSort={(column) => setSort((current) => nextSort(current, column))} />
            <SortHeader id="extensions-table-source" label="Source" column="source" sort={sort} onSort={(column) => setSort((current) => nextSort(current, column))} className="w-44" />
            <th id="extensions-table-placement" className="w-28 px-3 py-2 font-medium">
              Placement
            </th>
            <SortHeader id="extensions-table-enabled" label="Enabled" column="enabled" sort={sort} onSort={(column) => setSort((current) => nextSort(current, column))} className="w-28" />
          </tr>
        </thead>
        <tbody id="extensions-table-body">
          {!loaded
            ? [0, 1, 2].map((index) => (
                <tr id={`extensions-table-loading-${index}`} key={`loading-${index}`} className="border-t border-zinc-800">
                  <td id={`extensions-table-loading-${index}-cell`} colSpan={4} className="px-3 py-2">
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
                        {row.version ? (
                          <span id={`${domId}-version`} className="font-normal text-zinc-500">
                            {` (${row.version})`}
                          </span>
                        ) : null}
                      </p>
                      <p id={`${domId}-description`} className="whitespace-normal text-xs text-zinc-400">
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
                <td id={`${domId}-placement`} className="px-3 py-2.5">
                  <PinPlacement
                    id={`${domId}-pin`}
                    placement={toolbarPlacement(settings, row.id)}
                    disabled={!settings}
                    onChange={(placement) => onPlacement(row.id, placement)}
                  />
                </td>
                <td id={`${domId}-enabled`} className="px-3 py-2.5 align-middle">
                  <Toggle
                    id={`${domId}-toggle`}
                    checked={row.enabled}
                    disabled={row.builtin && !settings}
                    title={row.enabled ? "Disable" : "Enable"}
                    onChange={(enabled) => onEnabled(row.id, enabled)}
                  />
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
      </div>
    </section>
  );
}
