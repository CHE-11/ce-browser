import type { SplitView, TabGroup, TabGroupColor, TabInfo } from "../../../types/browser";

export type TabRow =
  | {
      kind: "tab";
      tab: TabInfo;
      /** Position in the strip, as `moveTab` counts it. */
      index: number;
      group: TabGroup | null;
      /** Which half of a split this tab is on screen as. */
      split: "left" | "right" | null;
    }
  | {
      kind: "group";
      group: TabGroup;
      tabs: TabInfo[];
      /** Strip position of the group's first tab. */
      index: number;
    };

/**
 * The strip in display order: a group row before each group's tabs. A collapsed group shows
 * only its active tab, the way Chrome does.
 */
export function buildTabRows(tabs: TabInfo[], groups: TabGroup[], activeId: number | null, split: SplitView | null) {
  const rows: TabRow[] = [];
  const seen = new Set<number>();
  tabs.forEach((tab, index) => {
    const group = tab.groupId === null ? null : (groups.find((item) => item.id === tab.groupId) ?? null);
    if (group && !seen.has(group.id)) {
      seen.add(group.id);
      rows.push({ kind: "group", group, tabs: tabs.filter((item) => item.groupId === group.id), index });
    }
    if (group?.collapsed && tab.id !== activeId) {
      return;
    }
    const half = split?.left === tab.id ? "left" : split?.right === tab.id ? "right" : null;
    rows.push({ kind: "tab", tab, index, group, split: half });
  });
  return rows;
}

export const GROUP_COLORS: TabGroupColor[] = ["grey", "blue", "red", "yellow", "green", "pink", "purple", "cyan", "orange"];

/** Tailwind classes for each group color: a solid swatch, a chip and the line under grouped tabs. */
export const GROUP_COLOR_CLASSES: Record<TabGroupColor, { swatch: string; chip: string; line: string }> = {
  grey: { swatch: "bg-zinc-400", chip: "bg-zinc-500/25 text-zinc-200", line: "bg-zinc-400" },
  blue: { swatch: "bg-sky-500", chip: "bg-sky-500/25 text-sky-200", line: "bg-sky-500" },
  red: { swatch: "bg-red-500", chip: "bg-red-500/25 text-red-200", line: "bg-red-500" },
  yellow: { swatch: "bg-amber-400", chip: "bg-amber-400/25 text-amber-100", line: "bg-amber-400" },
  green: { swatch: "bg-emerald-500", chip: "bg-emerald-500/25 text-emerald-200", line: "bg-emerald-500" },
  pink: { swatch: "bg-pink-500", chip: "bg-pink-500/25 text-pink-200", line: "bg-pink-500" },
  purple: { swatch: "bg-violet-500", chip: "bg-violet-500/25 text-violet-200", line: "bg-violet-500" },
  cyan: { swatch: "bg-cyan-400", chip: "bg-cyan-400/25 text-cyan-100", line: "bg-cyan-400" },
  orange: { swatch: "bg-orange-500", chip: "bg-orange-500/25 text-orange-200", line: "bg-orange-500" },
};

export function groupColorClasses(color: string) {
  return GROUP_COLOR_CLASSES[color as TabGroupColor] ?? GROUP_COLOR_CLASSES.grey;
}

export function groupLabel(group: TabGroup, count: number) {
  return group.name.trim() || `${count} ${count === 1 ? "tab" : "tabs"}`;
}
