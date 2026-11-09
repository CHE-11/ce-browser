import { useMemo, useState } from "react";
import { ChevronRight } from "lucide-react";
import { cn } from "@/lib/cn";
import { joinPath, pathId } from "@/lib/debug-hud";

/** Children drawn before a "show more" row. */
const CHILD_LIMIT = 200;
const STRING_LIMIT = 300;

type JsonTreeProps = {
  id: string;
  value: unknown;
  changed: Set<string>;
  query: string;
};

function isBranch(value: unknown): value is Record<string, unknown> | unknown[] {
  return typeof value === "object" && value !== null;
}

function entriesOf(value: Record<string, unknown> | unknown[]): [string, unknown][] {
  return Array.isArray(value) ? value.map((item, index) => [String(index), item]) : Object.entries(value);
}

/** Paths whose key or value contains `needle`, plus every ancestor of each. */
function matchingPaths(value: unknown, needle: string) {
  const found = new Set<string>();
  const visit = (node: unknown, key: string, path: string): boolean => {
    let hit = key.toLowerCase().includes(needle);
    if (isBranch(node)) {
      for (const [childKey, child] of entriesOf(node)) {
        if (visit(child, childKey, joinPath(path, childKey))) {
          hit = true;
        }
      }
    } else if (String(node).toLowerCase().includes(needle)) {
      hit = true;
    }
    if (hit) {
      found.add(path);
    }
    return hit;
  };
  visit(value, "", "");
  return found;
}

function leafText(value: unknown) {
  if (typeof value === "string") {
    const quoted = JSON.stringify(value);
    return quoted.length > STRING_LIMIT ? `${quoted.slice(0, STRING_LIMIT)}…"` : quoted;
  }
  return String(value);
}

function leafClass(value: unknown) {
  if (typeof value === "string") return "text-emerald-300";
  if (typeof value === "number") return "text-amber-300";
  if (typeof value === "boolean") return "text-violet-300";
  return "text-zinc-500";
}

type NodeProps = {
  idPrefix: string;
  name: string;
  path: string;
  value: unknown;
  depth: number;
  changed: Set<string>;
  visible: Set<string> | null;
};

function JsonNode({ idPrefix, name, path, value, depth, changed, visible }: NodeProps) {
  const [open, setOpen] = useState(depth < 1);
  const [showAll, setShowAll] = useState(false);
  if (visible && !visible.has(path)) {
    return null;
  }
  const nodeId = pathId(idPrefix, path);
  const highlight = changed.has(path);

  if (!isBranch(value)) {
    return (
      <li id={nodeId} className={cn("flex min-w-0 gap-1 rounded px-1 py-px", highlight && "bg-amber-400/15")}>
        <span id={`${nodeId}-key`} className="shrink-0 text-zinc-400">
          {name}:
        </span>
        <span id={`${nodeId}-value`} className={cn("min-w-0 break-all", leafClass(value))}>
          {leafText(value)}
        </span>
      </li>
    );
  }

  const entries = entriesOf(value);
  const expanded = open || visible !== null;
  const candidates = visible ? entries.filter(([key]) => visible.has(joinPath(path, key))) : entries;
  const shown = showAll ? candidates : candidates.slice(0, CHILD_LIMIT);
  return (
    <li id={nodeId}>
      <button
        id={`${nodeId}-toggle`}
        type="button"
        aria-expanded={expanded}
        className={cn("flex items-center gap-0.5 rounded px-1 py-px text-left hover:bg-zinc-800/70", highlight && "bg-amber-400/15")}
        onClick={() => setOpen((current) => !current)}
      >
        <ChevronRight id={`${nodeId}-chevron`} aria-hidden className={cn("size-3 shrink-0 text-zinc-500 transition-transform", expanded && "rotate-90")} />
        <span id={`${nodeId}-key`} className="text-zinc-300">
          {name}
        </span>
        <span id={`${nodeId}-summary`} className="text-zinc-600">
          {Array.isArray(value) ? `[${entries.length}]` : `{${entries.length}}`}
        </span>
      </button>
      {expanded ? (
        <ul id={`${nodeId}-children`} className="ml-2 border-l border-zinc-800 pl-2">
          {shown.map(([key, child]) => (
            <JsonNode key={key} idPrefix={idPrefix} name={key} path={joinPath(path, key)} value={child} depth={depth + 1} changed={changed} visible={visible} />
          ))}
          {shown.length < candidates.length ? (
            <li id={`${nodeId}-more`}>
              <button id={`${nodeId}-more-button`} type="button" className="px-1 text-sky-300 hover:underline" onClick={() => setShowAll(true)}>
                Show {candidates.length - shown.length} more
              </button>
            </li>
          ) : null}
        </ul>
      ) : null}
    </li>
  );
}

export function JsonTree({ id, value, changed, query }: JsonTreeProps) {
  const needle = query.trim().toLowerCase();
  const visible = useMemo(() => (needle ? matchingPaths(value, needle) : null), [needle, value]);

  if (visible && visible.size === 0) {
    return (
      <p id={`${id}-no-match`} className="px-3 py-4 text-xs text-zinc-500">
        Nothing matches this filter.
      </p>
    );
  }
  if (!isBranch(value)) {
    return (
      <p id={`${id}-leaf`} className={cn("px-3 py-2 font-mono text-[11px]", leafClass(value))}>
        {leafText(value)}
      </p>
    );
  }
  return (
    <ul id={id} className="px-2 py-1 font-mono text-[11px] leading-[1.15rem]">
      {entriesOf(value).map(([key, child]) => (
        <JsonNode key={key} idPrefix={id} name={key} path={key} value={child} depth={0} changed={changed} visible={visible} />
      ))}
    </ul>
  );
}
