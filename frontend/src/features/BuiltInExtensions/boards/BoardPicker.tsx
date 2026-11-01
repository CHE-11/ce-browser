import { useMemo, useState } from "react";
import { Check, ChevronDown, LayoutGrid, Plus } from "lucide-react";
import { Button } from "@/components/Button";
import { createBoard } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import type { BoardsSnapshot } from "@type/browser";
import { boardChoices } from "@features/Boards/board-tree";
import { errorText } from "../use-page-tool";

type BoardPickerProps = {
  id: string;
  boards: BoardsSnapshot;
  /** Empty saves to "Unsorted", which the first save creates. */
  value: string;
  onChange: (boardId: string) => void;
};

const inputClass = "h-8 min-w-0 flex-1 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500";

export function BoardPicker({ id, boards, value, onChange }: BoardPickerProps) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [name, setName] = useState("");
  const [error, setError] = useState("");
  const choices = useMemo(() => boardChoices(boards), [boards]);
  const selected = choices.find((choice) => choice.board.id === value);
  const needle = query.trim().toLowerCase();
  const shown = needle ? choices.filter((choice) => choice.label.toLowerCase().includes(needle)) : choices;

  const create = async () => {
    const trimmed = name.trim() || query.trim();
    if (!trimmed) {
      return;
    }
    try {
      const board = await createBoard(trimmed, null);
      onChange(board.id);
      setName("");
      setQuery("");
      setOpen(false);
      setError("");
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
        onClick={() => setOpen((current) => !current)}
      >
        <LayoutGrid id={`${id}-toggle-icon`} className="size-4 shrink-0 text-zinc-400" aria-hidden />
        <span id={`${id}-toggle-label`} className="min-w-0 flex-1 truncate">
          {selected ? selected.label : "Unsorted"}
        </span>
        <ChevronDown id={`${id}-toggle-chevron`} className={cn("size-4 shrink-0 text-zinc-500 transition-transform", open && "rotate-180")} aria-hidden />
      </button>
      {open ? (
        <div id={`${id}-panel`} className="flex flex-col gap-1.5 rounded-lg border border-zinc-800 bg-zinc-950/60 p-1.5">
          {choices.length > 6 ? (
            <input
              id={`${id}-search`}
              autoFocus
              value={query}
              placeholder="Find a board"
              className={inputClass}
              onChange={(event) => setQuery(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter" && shown[0]) {
                  onChange(shown[0].board.id);
                  setOpen(false);
                }
              }}
            />
          ) : null}
          <ul id={`${id}-list`} className="flex max-h-48 flex-col overflow-y-auto">
            {shown.length === 0 ? (
              <li id={`${id}-empty`} className="px-2 py-1.5 text-xs text-zinc-500">
                {choices.length === 0 ? "No boards yet. Pins go to Unsorted." : "No board matches."}
              </li>
            ) : (
              shown.map(({ board, label }) => (
                <li id={`${id}-item-${board.id}`} key={board.id}>
                  <button
                    id={`${id}-item-${board.id}-button`}
                    type="button"
                    title={label}
                    className={cn(
                      "flex h-8 w-full items-center gap-2 rounded-md px-2 text-left text-sm hover:bg-zinc-800",
                      board.id === value && "text-sky-300",
                    )}
                    onClick={() => {
                      onChange(board.id);
                      setOpen(false);
                    }}
                  >
                    <span id={`${id}-item-${board.id}-label`} className="min-w-0 flex-1 truncate">
                      {label}
                    </span>
                    {board.id === value ? <Check id={`${id}-item-${board.id}-check`} className="size-3.5 shrink-0" aria-hidden /> : null}
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
            <input
              id={`${id}-create-name`}
              value={name}
              placeholder="New board name"
              autoFocus={choices.length <= 6}
              className={inputClass}
              onChange={(event) => setName(event.target.value)}
            />
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
