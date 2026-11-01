
import { openTabMenu, closeTabOrSelection } from "@/lib/tab-actions";
import { useTabDrag } from "@/lib/tab-drag";
import { TabRow } from "@/lib/tab-rows";
import { WindowTabsState } from "@/lib/use-browser";
import { TabGroupChip } from "./TabGroupChip";
import { TabItem } from "./TabItem";

type TabRowsViewProps = {
  snapshot: WindowTabsState;
  rows: TabRow[];
  orientation: "horizontal" | "vertical";
  /** Icons only, for the collapsed vertical sidebar. Pinned tabs are always icons. */
  compact: boolean;
  drag: ReturnType<typeof useTabDrag>;
};

/** Group chips and tabs in strip order, for the horizontal strip or the vertical sidebar. */
export function TabRowsView({ snapshot, rows, orientation, compact, drag }: TabRowsViewProps) {
  const multi = (id: number) => snapshot.selectedIds.length > 1 && snapshot.selectedIds.includes(id);
  const lastIndex = snapshot.tabs.length - 1;

  return (
    <>
      {rows.map((row) => {
        if (row.kind === "group") {
          return (
            <TabGroupChip
              key={`group-${row.group.id}`}
              group={row.group}
              tabs={row.tabs}
              orientation={orientation}
              compact={compact}
            />
          );
        }
        const { tab, index } = row;
        const drop =
          drag.dropIndex === index && drag.dragging !== tab.id
            ? "before"
            : drag.dropIndex === index + 1 && index === lastIndex
              ? "after"
              : null;
        return (
          <TabItem
            key={tab.id}
            tab={tab}
            active={tab.id === snapshot.activeId}
            selected={snapshot.selectedIds.includes(tab.id)}
            grouped={multi(tab.id)}
            orientation={orientation}
            compact={compact || tab.pinned}
            groupColor={row.group?.color ?? null}
            split={row.split}
            drop={drop}
            onContextMenu={(event) => openTabMenu(event, tab, multi(tab.id))}
            onClose={() => closeTabOrSelection(tab.id, multi(tab.id))}
            onDragStart={() => drag.start(tab.id)}
            onDragEnd={drag.end}
            onDragOver={(event) => drag.over(event, index)}
          />
        );
      })}
    </>
  );
}
