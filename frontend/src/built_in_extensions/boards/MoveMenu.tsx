import { LayoutGrid } from "lucide-react";
import { ContextMenu, ContextMenuItem } from "@/components/ContextMenu";
import type { BoardsSnapshot } from "@type/browser";
import { boardChoices } from "./board-tree";

type MoveMenuProps = {
  boards: BoardsSnapshot;
  x: number;
  y: number;
  /** Board the pins are on now, shown disabled. */
  currentBoardId?: string;
  onMove: (boardId: string) => void;
  onClose: () => void;
};

/** "Move to" list of every board, by folder path. */
export function MoveMenu({ boards, x, y, currentBoardId, onMove, onClose }: MoveMenuProps) {
  const choices = boardChoices(boards);
  return (
    <ContextMenu id="boards-move-menu" label="Move to board" x={x} y={y} onClose={onClose} className="max-h-80 max-w-80 overflow-y-auto">
      {choices.length === 0 ? (
        <div id="boards-move-menu-empty" className="px-2 py-1 text-xs text-zinc-500">
          No boards yet.
        </div>
      ) : (
        choices.map(({ board, label }) => (
          <ContextMenuItem
            key={board.id}
            id={`boards-move-menu-${board.id}`}
            icon={<LayoutGrid />}
            label={label}
            title={label}
            disabled={board.id === currentBoardId}
            onClick={() => {
              onMove(board.id);
              onClose();
            }}
          />
        ))
      )}
    </ContextMenu>
  );
}
