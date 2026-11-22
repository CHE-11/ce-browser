import { useEffect, useMemo, useRef, useState } from "react";
import { FolderPlus, Inbox } from "lucide-react";
import { ContextMenu, ContextMenuItem, ContextMenuSeparator } from "@/components/ContextMenu";
import { createBoardFolder } from "@/lib/browser-client";
import type { BoardsSnapshot } from "@type/browser";
import { FolderGlyph } from "./folder-icons";
import { UNCATEGORIZED, folderChoices } from "./folder-tree";

const DEPTH_INDENT = ["", "pl-3", "pl-6", "pl-9", "pl-12", "pl-15", "pl-18"];

type MoveMenuProps = {
  boards: BoardsSnapshot;
  x: number;
  y: number;
  /** "Add to folder" keeps the pins where they are too; "Move to folder" takes them out. */
  label: string;
  /** Folders the pins are already in, shown disabled. */
  current?: string[];
  /** Offer Uncategorized, which takes the pins out of every folder. */
  allowUncategorized?: boolean;
  /** A folder id, or `UNCATEGORIZED`. */
  onPick: (folderId: string) => void;
  onClose: () => void;
  onError: (reason: unknown) => void;
};

/** Every folder by path, with a search box and a way to make a new folder. */
export function MoveMenu({ boards, x, y, label, current = [], allowUncategorized, onPick, onClose, onError }: MoveMenuProps) {
  const [query, setQuery] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const choices = useMemo(() => folderChoices(boards), [boards]);
  const needle = query.trim().toLowerCase();
  const shown = needle ? choices.filter((choice) => choice.label.toLowerCase().includes(needle)) : choices;
  const exact = choices.some((choice) => choice.folder.name.toLowerCase() === needle);

  useEffect(() => {
    const frame = requestAnimationFrame(() => inputRef.current?.focus());
    return () => cancelAnimationFrame(frame);
  }, []);

  const pick = (folderId: string) => {
    onPick(folderId);
    onClose();
  };

  const create = () => {
    const name = query.trim();
    if (!name) {
      return;
    }
    void createBoardFolder(name, null)
      .then((folder) => pick(folder.id))
      .catch(onError);
  };

  return (
    <ContextMenu id="boards-move-menu" label={label} x={x} y={y} onClose={onClose} className="flex max-h-96 w-72 flex-col">
      <div id="boards-move-menu-search" className="px-1.5 pb-1 pt-1">
        <input
          id="boards-move-menu-search-input"
          ref={inputRef}
          value={query}
          placeholder={`${label}…`}
          className="h-7 w-full rounded border border-zinc-700 bg-zinc-950 px-2 text-xs text-zinc-100 outline-none focus:border-sky-500"
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              event.stopPropagation();
              if (shown[0]) {
                pick(shown[0].folder.id);
              } else {
                create();
              }
            } else if (event.key !== "ArrowDown" && event.key !== "ArrowUp" && event.key !== "Escape") {
              event.stopPropagation();
            }
          }}
        />
      </div>
      <div id="boards-move-menu-list" className="min-h-0 flex-1 overflow-y-auto">
        {allowUncategorized && !needle ? (
          <>
            <ContextMenuItem
              id="boards-move-menu-uncategorized"
              icon={<Inbox />}
              label="Uncategorized"
              title="Take the pins out of every folder"
              onClick={() => pick(UNCATEGORIZED)}
            />
            <ContextMenuSeparator id="boards-move-menu-uncategorized-separator" />
          </>
        ) : null}
        {shown.length === 0 && !needle ? (
          <div id="boards-move-menu-empty" className="px-2 py-1 text-xs text-zinc-500">
            No folders yet.
          </div>
        ) : null}
        {shown.map(({ folder, depth, label: path }) => (
          <ContextMenuItem
            key={folder.id}
            id={`boards-move-menu-${folder.id}`}
            icon={<FolderGlyph id={`boards-move-menu-${folder.id}-glyph`} folder={folder} />}
            label={needle ? path : folder.name}
            title={path}
            disabled={current.includes(folder.id)}
            labelClassName={needle ? undefined : DEPTH_INDENT[Math.min(depth, DEPTH_INDENT.length - 1)]}
            onClick={() => pick(folder.id)}
          />
        ))}
      </div>
      {needle && !exact ? (
        <>
          <ContextMenuSeparator id="boards-move-menu-create-separator" />
          <ContextMenuItem id="boards-move-menu-create" icon={<FolderPlus />} label={`New folder "${query.trim()}"`} onClick={create} />
        </>
      ) : null}
    </ContextMenu>
  );
}
