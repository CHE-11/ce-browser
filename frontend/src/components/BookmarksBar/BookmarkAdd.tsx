import { useEffect, useMemo, useRef, useState } from "react";
import { addBookmark, closePopup } from "../../lib/browser-client";
import { listFolders, useBookmarks, useTabs } from "../../lib/use-browser";
import { Button } from "../Button";
import { PopupFrame } from "../PopupFrame";

type BookmarkAddProps = {
  mode: "link" | "folder";
  parentId: string;
};

export function BookmarkAdd({ mode, parentId }: BookmarkAddProps) {
  const tree = useBookmarks();
  const snapshot = useTabs();
  const folders = useMemo(() => listFolders(tree), [tree]);
  const isLink = mode === "link";
  const [name, setName] = useState("");
  const [url, setUrl] = useState("https://");
  const [folderId, setFolderId] = useState(parentId);
  const [error, setError] = useState<string | null>(null);
  const filled = useRef(false);
  const nameRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    nameRef.current?.focus();
  }, []);

  useEffect(() => {
    if (!isLink || filled.current) {
      return;
    }
    const active = snapshot.tabs.find((tab) => tab.id === snapshot.activeId);
    if (!active) {
      return;
    }
    filled.current = true;
    if (active.url === "ce://newtab") {
      return;
    }
    setName(active.title || active.url);
    setUrl(active.url);
    requestAnimationFrame(() => {
      nameRef.current?.focus();
      nameRef.current?.select();
    });
  }, [isLink, snapshot.activeId, snapshot.tabs]);

  const valid = name.trim().length > 0 && (!isLink || url.trim().length > 0);

  const save = async () => {
    if (!valid) {
      return;
    }
    try {
      await addBookmark(folderId, name.trim(), isLink ? url.trim() : null);
      await closePopup();
    } catch (reason) {
      setError(String(reason));
    }
  };

  return (
    <PopupFrame id="bookmark-add">
      <form
        id="bookmark-add-form"
        className="flex flex-col gap-3 p-4"
        onSubmit={(event) => {
          event.preventDefault();
          void save();
        }}
      >
        <h2 id="bookmark-add-title" className="text-sm font-semibold">
          {isLink ? "Add bookmark" : "Add folder"}
        </h2>
        <label id="bookmark-add-name-label" className="flex flex-col gap-1 text-xs text-zinc-400">
          Name
          <input
            id="bookmark-add-name"
            ref={nameRef}
            value={name}
            onChange={(event) => {
              filled.current = true;
              setName(event.target.value);
            }}
            className="h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500"
          />
        </label>
        {isLink ? (
          <label id="bookmark-add-url-label" className="flex flex-col gap-1 text-xs text-zinc-400">
            URL
            <input
              id="bookmark-add-url"
              value={url}
              spellCheck={false}
              onChange={(event) => {
                filled.current = true;
                setUrl(event.target.value);
              }}
              className="h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500"
            />
          </label>
        ) : null}
        <label id="bookmark-add-folder-label" className="flex flex-col gap-1 text-xs text-zinc-400">
          Folder
          <select
            id="bookmark-add-folder"
            value={folderId}
            onChange={(event) => setFolderId(event.target.value)}
            className="h-8 rounded-md border border-zinc-700 bg-zinc-950 px-1.5 text-sm text-zinc-100 outline-none focus:border-sky-500"
          >
            {folders.map((folder) => (
              <option key={folder.id} id={`bookmark-add-folder-${folder.id}`} value={folder.id}>
                {`${"\u00a0\u00a0".repeat(folder.depth)}${folder.title}`}
              </option>
            ))}
          </select>
        </label>
        {error ? (
          <p id="bookmark-add-error" className="text-xs text-red-300">
            {error}
          </p>
        ) : null}
        <div id="bookmark-add-actions" className="flex justify-end gap-2">
          <Button id="bookmark-add-cancel" variant="ghost" onClick={() => void closePopup()}>
            Cancel
          </Button>
          <Button id="bookmark-add-save" type="submit" variant="primary" disabled={!valid}>
            Save
          </Button>
        </div>
      </form>
    </PopupFrame>
  );
}
