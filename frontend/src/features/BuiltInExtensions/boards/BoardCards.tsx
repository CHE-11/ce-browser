import { Folder, LayoutGrid } from "lucide-react";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import type { Board, BoardFolder, BoardsSnapshot } from "@type/browser";
import { boardsIn, childFolders, coverPin, folderContents, pinCounts, type BoardsScope } from "./board-tree";
import { useThumbnail } from "./thumbnails";

type BoardCardsProps = {
  boards: BoardsSnapshot;
  folderId: string;
  onScope: (scope: BoardsScope) => void;
};

/** The folders and boards directly inside a folder, as cards. */
export function BoardCards({ boards, folderId, onScope }: BoardCardsProps) {
  const folders = childFolders(boards, folderId);
  const items = boardsIn(boards, folderId);
  const counts = pinCounts(boards);

  if (folders.length === 0 && items.length === 0) {
    return null;
  }

  return (
    <div id="boards-cards" className="grid grid-cols-[repeat(auto-fill,minmax(13rem,1fr))] gap-4">
      {folders.map((folder) => (
        <FolderCard key={folder.id} boards={boards} folder={folder} onOpen={() => onScope({ kind: "folder", id: folder.id })} />
      ))}
      {items.map((board) => (
        <BoardCard key={board.id} boards={boards} board={board} count={counts.get(board.id) ?? 0} onOpen={() => onScope({ kind: "board", id: board.id })} />
      ))}
    </div>
  );
}

const cardClass = cn(
  "group flex flex-col overflow-hidden rounded-xl bg-zinc-900 text-left ring-1 ring-zinc-800 transition hover:ring-zinc-600",
  focusRing,
);

function FolderCard({ boards, folder, onOpen }: { boards: BoardsSnapshot; folder: BoardFolder; onOpen: () => void }) {
  const contents = folderContents(boards, folder.id);
  const parts = [
    contents.folders ? `${contents.folders} ${contents.folders === 1 ? "folder" : "folders"}` : "",
    `${contents.boards} ${contents.boards === 1 ? "board" : "boards"}`,
    `${contents.pins} ${contents.pins === 1 ? "pin" : "pins"}`,
  ].filter(Boolean);
  return (
    <button id={`boards-card-folder-${folder.id}`} type="button" className={cardClass} onClick={onOpen}>
      <span id={`boards-card-folder-${folder.id}-art`} className="flex aspect-[4/3] items-center justify-center bg-zinc-800/60 text-zinc-500">
        <Folder id={`boards-card-folder-${folder.id}-icon`} className="size-10" aria-hidden />
      </span>
      <span id={`boards-card-folder-${folder.id}-text`} className="flex flex-col gap-0.5 px-3 py-2.5">
        <span id={`boards-card-folder-${folder.id}-name`} className="truncate text-sm font-medium text-zinc-100">
          {folder.name}
        </span>
        <span id={`boards-card-folder-${folder.id}-meta`} className="truncate text-xs text-zinc-500">
          {parts.join(" · ")}
        </span>
      </span>
    </button>
  );
}

function BoardCard({ boards, board, count, onOpen }: { boards: BoardsSnapshot; board: Board; count: number; onOpen: () => void }) {
  const cover = coverPin(boards, board);
  const thumbnail = useThumbnail(cover?.hasImage ? cover.id : null);
  return (
    <button id={`boards-card-board-${board.id}`} type="button" className={cardClass} onClick={onOpen}>
      <span id={`boards-card-board-${board.id}-art`} className="relative flex aspect-[4/3] items-center justify-center overflow-hidden bg-zinc-800/60 text-zinc-500">
        {thumbnail ? (
          <img id={`boards-card-board-${board.id}-image`} src={thumbnail} alt="" className="absolute inset-0 size-full object-cover object-top transition group-hover:scale-[1.02]" />
        ) : (
          <LayoutGrid id={`boards-card-board-${board.id}-icon`} className="size-10" aria-hidden />
        )}
      </span>
      <span id={`boards-card-board-${board.id}-text`} className="flex flex-col gap-0.5 px-3 py-2.5">
        <span id={`boards-card-board-${board.id}-name`} className="truncate text-sm font-medium text-zinc-100">
          {board.name}
        </span>
        <span id={`boards-card-board-${board.id}-meta`} className="truncate text-xs text-zinc-500">
          {count === 1 ? "1 pin" : `${count} pins`}
        </span>
      </span>
    </button>
  );
}
