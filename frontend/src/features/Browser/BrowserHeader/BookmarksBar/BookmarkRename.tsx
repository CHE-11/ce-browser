import { Button } from "@/components/Button";
import { PopupFrame } from "@/components/PopupFrame";
import { closePopup, updateBookmark } from "@/lib/browser-client";
import { findFolder, useBookmarksState } from "@/lib/use-browser";
import { useEffect, useRef, useState } from "react";

type BookmarkRenameProps = {
  id: string;
};

/** Renames a bookmark or folder. The address is left as it is. */
export function BookmarkRename({ id }: BookmarkRenameProps) {
  const { tree, loaded } = useBookmarksState();
  const node = findFolder(tree, id);
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const filled = useRef(false);
  const nameRef = useRef<HTMLInputElement>(null);
  const isFolder = !!node && !node.url && !node.separator;
  const valid = name.trim().length > 0;

  useEffect(() => {
    if (!node || filled.current) {
      return;
    }
    filled.current = true;
    setName(node.title);
    requestAnimationFrame(() => {
      nameRef.current?.focus();
      nameRef.current?.select();
    });
  }, [node]);

  const save = async () => {
    if (!node || !valid) {
      return;
    }
    try {
      await updateBookmark(node.id, name.trim(), node.url);
      await closePopup();
    } catch (reason) {
      setError(String(reason));
    }
  };

  return (
    <PopupFrame id="bookmark-rename" width={340} ready={loaded}>
      <form
        id="bookmark-rename-form"
        className="flex flex-col gap-3 p-4"
        onSubmit={(event) => {
          event.preventDefault();
          void save();
        }}
      >
        <h2 id="bookmark-rename-title" className="text-sm font-semibold">
          {isFolder ? "Rename folder" : "Rename bookmark"}
        </h2>
        {loaded && !node ? (
          <p id="bookmark-rename-missing" className="text-xs text-red-300">
            That bookmark no longer exists.
          </p>
        ) : (
          <label id="bookmark-rename-name-label" className="flex flex-col gap-1 text-xs text-zinc-400">
            Name
            <input
              id="bookmark-rename-name"
              ref={nameRef}
              value={name}
              onChange={(event) => setName(event.target.value)}
              className="h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500"
            />
          </label>
        )}
        {error ? (
          <p id="bookmark-rename-error" className="text-xs text-red-300">
            {error}
          </p>
        ) : null}
        <div id="bookmark-rename-actions" className="flex justify-end gap-2">
          <Button id="bookmark-rename-cancel" variant="ghost" onClick={() => void closePopup()}>
            Cancel
          </Button>
          <Button id="bookmark-rename-save" type="submit" variant="primary" disabled={!node || !valid}>
            Save
          </Button>
        </div>
      </form>
    </PopupFrame>
  );
}
