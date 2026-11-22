import { useLayoutEffect, useMemo, useRef, useState } from "react";
import { Check, ChevronDown, Inbox, Plus } from "lucide-react";
import { Button } from "@/components/Button";
import { createBoardFolder } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import type { BoardsSnapshot } from "@type/browser";
import { errorText } from "../use-page-tool";
import { FolderGlyph } from "./folder-icons";
import { UNCATEGORIZED, folderChoices, folderLabel } from "./folder-tree";

type FolderPickerProps = {
  id: string;
  boards: BoardsSnapshot;
  /** A folder id, `UNCATEGORIZED`, or empty for the folder used last. */
  value: string;
  onChange: (folderId: string) => void;
};

const inputClass = "h-8 min-w-0 flex-1 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500";
const DEPTH_INDENT = ["pl-2", "pl-5", "pl-8", "pl-11", "pl-14", "pl-16"];

/** The folder a save goes to. Empty `value` means the folder used last. */
export function resolveFolder(boards: BoardsSnapshot, value: string) {
  const id = value || boards.lastFolderId || UNCATEGORIZED;
  return id === UNCATEGORIZED || !boards.folders.some((folder) => folder.id === id) ? UNCATEGORIZED : id;
}

export function folderTargetName(boards: BoardsSnapshot, value: string) {
  const id = resolveFolder(boards, value);
  return id === UNCATEGORIZED ? "Uncategorized" : folderLabel(boards, id);
}

/** Choose the folder a save goes to, or make a new one. */
export function FolderPicker({ id, boards, value, onChange }: FolderPickerProps) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [name, setName] = useState("");
  const [error, setError] = useState("");
  const searchRef = useRef<HTMLInputElement>(null);
  const choices = useMemo(() => folderChoices(boards), [boards]);
  const current = resolveFolder(boards, value);
  const currentFolder = boards.folders.find((folder) => folder.id === current);
  const needle = query.trim().toLowerCase();
  const shown = needle ? choices.filter((choice) => choice.label.toLowerCase().includes(needle)) : choices;
  const showSearch = choices.length > 6;

  // The popup is still the closed height on this frame. A normal focus would scroll the
  // field into view and pull the header off screen until the window grows.
  useLayoutEffect(() => {
    if (open && showSearch) {
      searchRef.current?.focus({ preventScroll: true });
    }
  }, [open, showSearch]);

  const pick = (folderId: string) => {
    onChange(folderId);
    setOpen(false);
    setQuery("");
  };

  const create = async () => {
    const trimmed = name.trim() || query.trim();
    if (!trimmed) {
      return;
    }
    try {
      const folder = await createBoardFolder(trimmed, null);
      setName("");
      setError("");
      pick(folder.id);
    } catch (reason) {
      setError(errorText(reason));
    }
  };

  return (
    <div id={id} className="flex flex-col gap-1.5">
      <button
        id={`${id}-toggle`}
        type="button"
        aria-expanded={open}
        className="flex h-10 w-full items-center gap-2 rounded-lg bg-zinc-800 px-3 text-left text-sm hover:bg-zinc-700"
        onClick={() => setOpen((currentOpen) => !currentOpen)}
      >
        {currentFolder ? (
          <FolderGlyph id={`${id}-toggle-icon`} folder={currentFolder} className="size-4 shrink-0" />
        ) : (
          <Inbox id={`${id}-toggle-icon`} className="size-4 shrink-0 text-zinc-400" aria-hidden />
        )}
        <span id={`${id}-toggle-label`} className="min-w-0 flex-1 truncate">
          {folderTargetName(boards, value)}
        </span>
        <ChevronDown id={`${id}-toggle-chevron`} className={cn("size-4 shrink-0 text-zinc-500 transition-transform", open && "rotate-180")} aria-hidden />
      </button>
      {open ? (
        <div id={`${id}-panel`} className="flex flex-col gap-1.5 rounded-lg border border-zinc-800 bg-zinc-950/60 p-1.5">
          {showSearch ? (
            <input
              id={`${id}-search`}
              ref={searchRef}
              value={query}
              placeholder="Find a folder"
              className={inputClass}
              onChange={(event) => setQuery(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter" && shown[0]) {
                  pick(shown[0].folder.id);
                }
              }}
            />
          ) : null}
          <ul id={`${id}-list`} className="flex max-h-56 flex-col overflow-y-auto">
            {!needle ? (
              <li id={`${id}-item-uncategorized`}>
                <button
                  id={`${id}-item-uncategorized-button`}
                  type="button"
                  className={cn("flex h-8 w-full items-center gap-2 rounded-md px-2 text-left text-sm hover:bg-zinc-800", current === UNCATEGORIZED && "text-sky-300")}
                  onClick={() => pick(UNCATEGORIZED)}
                >
                  <Inbox id={`${id}-item-uncategorized-icon`} className="size-4 shrink-0 text-zinc-400" aria-hidden />
                  <span id={`${id}-item-uncategorized-label`} className="min-w-0 flex-1 truncate">
                    Uncategorized
                  </span>
                  {current === UNCATEGORIZED ? <Check id={`${id}-item-uncategorized-check`} className="size-3.5 shrink-0" aria-hidden /> : null}
                </button>
              </li>
            ) : null}
            {shown.length === 0 && needle ? (
              <li id={`${id}-empty`} className="px-2 py-1.5 text-xs text-zinc-500">
                No folder matches.
              </li>
            ) : (
              shown.map(({ folder, depth, label }) => (
                <li id={`${id}-item-${folder.id}`} key={folder.id}>
                  <button
                    id={`${id}-item-${folder.id}-button`}
                    type="button"
                    title={label}
                    className={cn(
                      "flex h-8 w-full items-center gap-2 rounded-md pr-2 text-left text-sm hover:bg-zinc-800",
                      needle ? "pl-2" : DEPTH_INDENT[Math.min(depth, DEPTH_INDENT.length - 1)],
                      folder.id === current && "text-sky-300",
                    )}
                    onClick={() => pick(folder.id)}
                  >
                    <FolderGlyph id={`${id}-item-${folder.id}-icon`} folder={folder} className="size-4 shrink-0" />
                    <span id={`${id}-item-${folder.id}-label`} className="min-w-0 flex-1 truncate">
                      {needle ? label : folder.name}
                    </span>
                    {folder.id === current ? <Check id={`${id}-item-${folder.id}-check`} className="size-3.5 shrink-0" aria-hidden /> : null}
                  </button>
                </li>
              ))
            )}
          </ul>
          <form
            id={`${id}-create`}
            className="flex gap-1.5 border-t border-zinc-800 pt-1.5"
            onSubmit={(event) => {
              event.preventDefault();
              void create();
            }}
          >
            <input id={`${id}-create-name`} value={name} placeholder="New folder name" className={inputClass} onChange={(event) => setName(event.target.value)} />
            <Button id={`${id}-create-submit`} type="submit" size="default" disabled={!name.trim() && !query.trim()}>
              <Plus id={`${id}-create-icon`} className="size-4" aria-hidden />
              Create
            </Button>
          </form>
          {error ? (
            <p id={`${id}-error`} className="px-1 text-xs text-amber-300">
              {error}
            </p>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
