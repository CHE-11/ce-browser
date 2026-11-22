import { useMemo, useState, type MouseEvent, type ReactNode } from "react";
import { Check, ChevronDown, Copy, Star, X } from "lucide-react";
import { ContextMenu, ContextMenuItem, ContextMenuSeparator } from "@/components/ContextMenu";
import { cn } from "@/lib/cn";
import type { Pin, PinKind } from "@type/browser";
import { PIN_KINDS, PIN_KIND_LABELS } from "../folder-tree";
import {
  DATE_LABELS,
  EMPTY_FILTER,
  FILTER_SWATCHES,
  SHAPE_LABELS,
  SIZE_LABELS,
  activeFilterCount,
  filterFacets,
  type DateFilter,
  type PinFilter,
  type ShapeFilter,
  type SizeFilter,
} from "../filters";

type FilterBarProps = {
  filter: PinFilter;
  onFilter: (filter: PinFilter) => void;
  /** Pins in the current view before filtering, for the choices each menu offers. */
  pins: Pin[];
};

type Open = { key: string; x: number; y: number } | null;

const FACET_LIMIT = 40;

function toggle<T>(list: T[], value: T) {
  return list.includes(value) ? list.filter((item) => item !== value) : [...list, value];
}

/** Filter chips under the toolbar. Each opens a menu of choices drawn from the current view. */
export function FilterBar({ filter, onFilter, pins }: FilterBarProps) {
  const [open, setOpen] = useState<Open>(null);
  const facets = useMemo(() => filterFacets(pins), [pins]);
  const set = (patch: Partial<PinFilter>) => onFilter({ ...filter, ...patch });
  const close = () => setOpen(null);

  const chip = (key: string, label: string, value: string, active: boolean, onClear: () => void) => (
    <span id={`boards-filter-${key}`} className={cn("flex h-7 items-center rounded-full border text-xs", active ? "border-sky-700 bg-sky-950 text-sky-100" : "border-zinc-800 bg-zinc-900 text-zinc-300")}>
      <button
        id={`boards-filter-${key}-button`}
        type="button"
        aria-expanded={open?.key === key}
        className="flex h-full items-center gap-1 rounded-full pl-3 pr-2 hover:text-zinc-100"
        onClick={(event: MouseEvent<HTMLButtonElement>) => {
          const rect = event.currentTarget.getBoundingClientRect();
          setOpen(open?.key === key ? null : { key, x: rect.left, y: rect.bottom + 4 });
        }}
      >
        <span id={`boards-filter-${key}-label`} className="text-zinc-500">
          {label}
        </span>
        {value ? (
          <span id={`boards-filter-${key}-value`} className="max-w-40 truncate">
            {value}
          </span>
        ) : null}
        <ChevronDown id={`boards-filter-${key}-chevron`} className="size-3 text-zinc-500" aria-hidden />
      </button>
      {active ? (
        <button id={`boards-filter-${key}-clear`} type="button" aria-label={`Clear ${label}`} className="mr-1 inline-flex size-5 items-center justify-center rounded-full hover:bg-sky-900" onClick={onClear}>
          <X id={`boards-filter-${key}-clear-icon`} className="size-3" aria-hidden />
        </button>
      ) : null}
    </span>
  );

  const check = (on: boolean, id: string) => (on ? <Check id={`${id}-check`} className="size-3.5 text-sky-400" aria-hidden /> : null);
  const list = <T extends string>(key: string, values: T[], chosen: T[], label: (value: T) => string, count?: (value: T) => number): ReactNode =>
    values.map((value) => (
      <ContextMenuItem
        id={`boards-filter-${key}-option-${value}`}
        key={value}
        icon={<span id={`boards-filter-${key}-option-${value}-blank`} />}
        label={label(value)}
        trailing={
          <span id={`boards-filter-${key}-option-${value}-trailing`} className="flex items-center gap-2 text-zinc-500">
            {count ? count(value) : null}
            {check(chosen.includes(value), `boards-filter-${key}-option-${value}`)}
          </span>
        }
        onClick={() => {
          if (key === "kind") {
            set({ kinds: toggle(filter.kinds, value as PinKind) });
          } else if (key === "ext") {
            set({ exts: toggle(filter.exts, value) });
          } else if (key === "tags") {
            set({ tags: toggle(filter.tags, value) });
          }
        }}
      />
    ));

  const single = <T extends string>(key: string, labels: Record<T, string>, value: T, onPick: (value: T) => void) => (
    <ContextMenu id={`boards-filter-${key}-menu`} label={key} x={open?.x ?? 0} y={open?.y ?? 0} onClose={close}>
      {(Object.keys(labels) as T[]).map((option) => (
        <ContextMenuItem
          id={`boards-filter-${key}-menu-${option}`}
          key={option}
          icon={value === option ? <Check /> : <span id={`boards-filter-${key}-menu-${option}-blank`} />}
          label={labels[option]}
          onClick={() => {
            onPick(option);
            close();
          }}
        />
      ))}
    </ContextMenu>
  );

  const kindCounts = new Map<PinKind, number>();
  for (const pin of pins) {
    kindCounts.set(pin.kind, (kindCounts.get(pin.kind) ?? 0) + 1);
  }
  const extCounts = new Map(facets.exts.map((facet) => [facet.value, facet.count]));
  const tagCounts = new Map(facets.tags.map((facet) => [facet.value, facet.count]));

  return (
    <div id="boards-filter-bar" className="flex shrink-0 flex-wrap items-center gap-1.5 border-b border-zinc-800 px-4 py-2">
      {chip("kind", "Type", filter.kinds.map((kind) => PIN_KIND_LABELS[kind]).join(", "), filter.kinds.length > 0, () => set({ kinds: [] }))}
      {chip("tags", "Tags", filter.tags.join(filter.tagMode === "all" ? " + " : " / "), filter.tags.length > 0, () => set({ tags: [] }))}
      {chip("rating", "Rating", filter.rating > 0 ? `${filter.rating}★ or more` : "", filter.rating > 0, () => set({ rating: 0 }))}
      <span id="boards-filter-color" className={cn("flex h-7 items-center gap-1.5 rounded-full border pl-3 pr-1 text-xs", filter.color ? "border-sky-700 bg-sky-950" : "border-zinc-800 bg-zinc-900")}>
        <button
          id="boards-filter-color-button"
          type="button"
          className="flex items-center gap-1.5 text-zinc-500 hover:text-zinc-200"
          onClick={(event) => {
            const rect = event.currentTarget.getBoundingClientRect();
            setOpen(open?.key === "color" ? null : { key: "color", x: rect.left, y: rect.bottom + 4 });
          }}
        >
          Color
          {filter.color ? <span id="boards-filter-color-swatch" className="size-3.5 rounded-full ring-1 ring-white/30" style={{ backgroundColor: filter.color }} /> : null}
          <ChevronDown id="boards-filter-color-chevron" className="size-3" aria-hidden />
        </button>
        {filter.color ? (
          <button id="boards-filter-color-clear" type="button" aria-label="Clear color" className="inline-flex size-5 items-center justify-center rounded-full text-sky-100 hover:bg-sky-900" onClick={() => set({ color: "" })}>
            <X id="boards-filter-color-clear-icon" className="size-3" aria-hidden />
          </button>
        ) : (
          <span id="boards-filter-color-spacer" className="w-1" />
        )}
      </span>
      {chip("ext", "Format", filter.exts.map((ext) => ext.toUpperCase()).join(", "), filter.exts.length > 0, () => set({ exts: [] }))}
      {chip("size", "Size", filter.size === "any" ? "" : SIZE_LABELS[filter.size], filter.size !== "any", () => set({ size: "any" }))}
      {chip("shape", "Shape", filter.shape === "any" ? "" : SHAPE_LABELS[filter.shape], filter.shape !== "any", () => set({ shape: "any" }))}
      {chip("date", "Added", filter.date === "any" ? "" : DATE_LABELS[filter.date], filter.date !== "any", () => set({ date: "any" }))}
      {chip("host", "Source", filter.host, Boolean(filter.host), () => set({ host: "" }))}
      <button
        id="boards-filter-duplicates"
        type="button"
        aria-pressed={filter.duplicates}
        className={cn(
          "flex h-7 items-center gap-1.5 rounded-full border px-3 text-xs",
          filter.duplicates ? "border-sky-700 bg-sky-950 text-sky-100" : "border-zinc-800 bg-zinc-900 text-zinc-400 hover:text-zinc-200",
        )}
        onClick={() => set({ duplicates: !filter.duplicates })}
      >
        <Copy id="boards-filter-duplicates-icon" className="size-3" aria-hidden />
        Duplicates
      </button>
      {activeFilterCount(filter) > 0 ? (
        <button id="boards-filter-clear-all" type="button" className="ml-1 text-xs text-zinc-500 hover:text-zinc-200" onClick={() => onFilter(EMPTY_FILTER)}>
          Clear all
        </button>
      ) : null}

      {open?.key === "kind" ? (
        <ContextMenu id="boards-filter-kind-menu" label="Type" x={open.x} y={open.y} onClose={close}>
          {list("kind", PIN_KINDS.filter((kind) => kindCounts.has(kind) || filter.kinds.includes(kind)), filter.kinds, (kind) => PIN_KIND_LABELS[kind], (kind) => kindCounts.get(kind) ?? 0)}
        </ContextMenu>
      ) : null}
      {open?.key === "tags" ? (
        <ContextMenu id="boards-filter-tags-menu" label="Tags" x={open.x} y={open.y} onClose={close} className="max-h-96 overflow-y-auto">
          <ContextMenuItem
            id="boards-filter-tags-mode"
            icon={<span id="boards-filter-tags-mode-blank" />}
            label={filter.tagMode === "all" ? "Match every tag" : "Match any tag"}
            title="Switch between needing every chosen tag and any of them"
            onClick={() => set({ tagMode: filter.tagMode === "all" ? "any" : "all" })}
          />
          <ContextMenuSeparator id="boards-filter-tags-separator" />
          {facets.tags.length === 0 ? (
            <p id="boards-filter-tags-empty" className="px-2 py-1 text-xs text-zinc-500">
              Nothing here is tagged.
            </p>
          ) : (
            list(
              "tags",
              [...new Set([...filter.tags, ...facets.tags.slice(0, FACET_LIMIT).map((facet) => facet.value)])],
              filter.tags,
              (tag) => tag,
              (tag) => tagCounts.get(tag) ?? 0,
            )
          )}
        </ContextMenu>
      ) : null}
      {open?.key === "rating" ? (
        <ContextMenu id="boards-filter-rating-menu" label="Rating" x={open.x} y={open.y} onClose={close}>
          {[0, 1, 2, 3, 4, 5].map((stars) => (
            <ContextMenuItem
              id={`boards-filter-rating-menu-${stars}`}
              key={stars}
              icon={filter.rating === stars ? <Check /> : <Star />}
              label={stars === 0 ? "Any rating" : stars === 5 ? "5 stars" : `${stars} stars or more`}
              onClick={() => {
                set({ rating: stars });
                close();
              }}
            />
          ))}
        </ContextMenu>
      ) : null}
      {open?.key === "color" ? (
        <ContextMenu id="boards-filter-color-menu" label="Color" x={open.x} y={open.y} onClose={close}>
          <div id="boards-filter-color-swatches" className="grid grid-cols-7 gap-1.5 p-2">
            {FILTER_SWATCHES.map((color) => (
              <button
                id={`boards-filter-color-swatch-${color.slice(1)}`}
                key={color}
                type="button"
                role="menuitem"
                aria-label={color}
                className={cn("size-6 rounded-full ring-1 ring-white/20", filter.color === color && "ring-2 ring-sky-400")}
                style={{ backgroundColor: color }}
                onClick={() => {
                  set({ color });
                  close();
                }}
              />
            ))}
          </div>
          <label id="boards-filter-color-custom" className="flex items-center justify-between gap-2 border-t border-zinc-800 px-2 py-1.5 text-xs text-zinc-400">
            Pick a color
            <input
              id="boards-filter-color-custom-input"
              type="color"
              value={filter.color || "#3b82f6"}
              className="h-6 w-10 cursor-pointer rounded border-0 bg-transparent"
              onChange={(event) => set({ color: event.target.value })}
            />
          </label>
        </ContextMenu>
      ) : null}
      {open?.key === "ext" ? (
        <ContextMenu id="boards-filter-ext-menu" label="Format" x={open.x} y={open.y} onClose={close} className="max-h-96 overflow-y-auto">
          {facets.exts.length === 0 ? (
            <p id="boards-filter-ext-empty" className="px-2 py-1 text-xs text-zinc-500">
              No files here.
            </p>
          ) : (
            list(
              "ext",
              [...new Set([...filter.exts, ...facets.exts.slice(0, FACET_LIMIT).map((facet) => facet.value)])],
              filter.exts,
              (ext) => ext.toUpperCase(),
              (ext) => extCounts.get(ext) ?? 0,
            )
          )}
        </ContextMenu>
      ) : null}
      {open?.key === "size" ? single<SizeFilter>("size", SIZE_LABELS, filter.size, (size) => set({ size })) : null}
      {open?.key === "shape" ? single<ShapeFilter>("shape", SHAPE_LABELS, filter.shape, (shape) => set({ shape })) : null}
      {open?.key === "date" ? single<DateFilter>("date", DATE_LABELS, filter.date, (date) => set({ date })) : null}
      {open?.key === "host" ? (
        <ContextMenu id="boards-filter-host-menu" label="Source" x={open.x} y={open.y} onClose={close} className="max-h-96 overflow-y-auto">
          <ContextMenuItem
            id="boards-filter-host-menu-any"
            icon={filter.host ? <span id="boards-filter-host-menu-any-blank" /> : <Check />}
            label="Any site"
            onClick={() => {
              set({ host: "" });
              close();
            }}
          />
          {facets.hosts.slice(0, FACET_LIMIT).map((facet) => (
            <ContextMenuItem
              id={`boards-filter-host-menu-${facet.value}`}
              key={facet.value}
              icon={filter.host === facet.value ? <Check /> : <span id={`boards-filter-host-menu-${facet.value}-blank`} />}
              label={facet.value}
              trailing={
                <span id={`boards-filter-host-menu-${facet.value}-count`} className="text-zinc-500">
                  {facet.count}
                </span>
              }
              onClick={() => {
                set({ host: facet.value });
                close();
              }}
            />
          ))}
        </ContextMenu>
      ) : null}
    </div>
  );
}
