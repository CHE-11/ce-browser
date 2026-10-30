import { useEffect, useMemo, useRef, useState } from "react";
import type { BookmarkNode, BookmarkTree } from "../../../../types/browser";
import {
  addBookmark,
  closePopup,
  deleteBookmark,
  findBookmark,
  moveBookmark,
  updateBookmark,
} from "../../lib/browser-client";
import { listFolders, useBookmarks } from "../../lib/use-browser";
import { Button } from "../Button";
import { PopupFrame } from "../PopupFrame";

type BookmarkEditorProps = {
  url: string;
  title: string;
};

export function findParentId(tree: BookmarkTree | null, id: string): string | null {
  if (!tree) {
    return null;
  }
  const search = (node: BookmarkNode): string | null => {
    for (const child of node.children ?? []) {
      if (child.id === id) {
        return node.id;
      }
      const found = search(child);
      if (found) {
        return found;
      }
    }
    return null;
  };
  return search(tree.bar) ?? search(tree.other);
}

/** The star adds the page right away, like Chrome. The editor then renames, moves or removes it. */
export function BookmarkEditor({ url, title }: BookmarkEditorProps) {
  const tree = useBookmarks();
  const folders = useMemo(() => listFolders(tree), [tree]);
  const [bookmark, setBookmark] = useState<BookmarkNode | null>(null);
  const [created, setCreated] = useState(false);
  const [name, setName] = useState(title);
  const [folderId, setFolderId] = useState("bar");
  const [error, setError] = useState<string | null>(null);
  const started = useRef(false);
  const nameRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (started.current) {
      return;
    }
    started.current = true;
    void (async () => {
      try {
        const existing = await findBookmark(url);
        if (existing) {
          setBookmark(existing);
          setName(existing.title);
          return;
        }
        const node = await addBookmark("bar", title || url, url);
        setBookmark(node);
        setCreated(true);
      } catch (reason) {
        setError(String(reason));
      }
    })();
  }, [title, url]);

  const folderKnown = useRef(false);
  useEffect(() => {
    if (!bookmark || folderKnown.current) {
      return;
    }
    const parent = findParentId(tree, bookmark.id);
    if (parent) {
      folderKnown.current = true;
      setFolderId(parent);
    }
  }, [bookmark, tree]);

  useEffect(() => {
    if (bookmark) {
      nameRef.current?.focus();
      nameRef.current?.select();
    }
  }, [bookmark]);

  const save = async () => {
    if (!bookmark) {
      return;
    }
    try {
      if (name.trim() && name !== bookmark.title) {
        await updateBookmark(bookmark.id, name.trim(), bookmark.url);
      }
      if (folderId !== findParentId(tree, bookmark.id)) {
        await moveBookmark(bookmark.id, folderId);
      }
      await closePopup();
    } catch (reason) {
      setError(String(reason));
    }
  };

  const remove = async () => {
    if (bookmark) {
      await deleteBookmark(bookmark.id);
    }
    await closePopup();
  };

  return (
    <PopupFrame id="bookmark-editor">
      <form
        id="bookmark-editor-form"
        className="flex flex-col gap-3 p-4"
        onSubmit={(event) => {
          event.preventDefault();
          void save();
        }}
      >
        <h2 id="bookmark-editor-title" className="text-sm font-semibold">
          {created ? "Bookmark added" : "Edit bookmark"}
        </h2>
        <label id="bookmark-editor-name-label" className="flex flex-col gap-1 text-xs text-zinc-400">
          Name
          <input
            id="bookmark-editor-name"
            ref={nameRef}
            value={name}
            onChange={(event) => setName(event.target.value)}
            className="h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500"
          />
        </label>
        <label id="bookmark-editor-folder-label" className="flex flex-col gap-1 text-xs text-zinc-400">
          Folder
          <select
            id="bookmark-editor-folder"
            value={folderId}
            onChange={(event) => setFolderId(event.target.value)}
            className="h-8 rounded-md border border-zinc-700 bg-zinc-950 px-1.5 text-sm text-zinc-100 outline-none focus:border-sky-500"
          >
            {folders.map((folder) => (
              <option key={folder.id} id={`bookmark-editor-folder-${folder.id}`} value={folder.id}>
                {`${"\u00a0\u00a0".repeat(folder.depth)}${folder.title}`}
              </option>
            ))}
          </select>
        </label>
        {error ? (
          <p id="bookmark-editor-error" className="text-xs text-red-300">
            {error}
          </p>
        ) : null}
        <div id="bookmark-editor-actions" className="flex justify-end gap-2">
          <Button id="bookmark-editor-remove" variant="ghost" disabled={!bookmark} onClick={() => void remove()}>
            Remove
          </Button>
          <Button id="bookmark-editor-done" type="submit" variant="primary" disabled={!bookmark}>
            Done
          </Button>
        </div>
      </form>
    </PopupFrame>
  );
}
