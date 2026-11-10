import { Button } from "@/components/Button";
import { PopupFrame } from "@/components/PopupFrame";
import { bookmarkEditorFrameStyle } from "@/lib/bookmark-size";
import { findBookmark, addBookmark, updateBookmark, moveBookmark, closePopup, deleteBookmark, saveSettings } from "@/lib/browser-client";
import { CHROME, CHROME_LIMITS, metric } from "@/lib/chrome-metrics";
import { findBookmarkParent, findFolder, useBookmarksState, useSettings } from "@/lib/use-browser";
import { BookmarkNode } from "@type/browser";
import { useEffect, useRef, useState, type MouseEvent } from "react";
import { BookmarkEditorCompose, BookmarkEditorComposeDialog, BookmarkEditorContextMenu, type BookmarkEditorMenuTarget } from "./BookmarkEditorContextMenu";
import { BookmarkPlace, BookmarkPlacePicker } from "./BookmarkEditorPlacePicker";

type BookmarkEditorProps = {
  url: string;
  title: string;
};

/** The star adds the page right away, like Chrome. The editor then renames, moves or removes it. */
export function BookmarkEditor({ url, title }: BookmarkEditorProps) {
  const { tree, loaded } = useBookmarksState();
  const settings = useSettings();
  const textSize = metric(settings?.bookmarkEditorTextSize, CHROME.bookmarkEditorTextSize);
  const padding = metric(settings?.bookmarkEditorPadding, CHROME.bookmarkEditorPadding);
  const rowPadding = metric(settings?.bookmarkEditorRowPadding, CHROME.bookmarkEditorRowPadding);
  const iconSize = metric(settings?.bookmarkEditorIconSize, CHROME.bookmarkEditorIconSize);
  const savedHeight = metric(settings?.bookmarkEditorListHeight, CHROME.bookmarkEditorListHeight);
  const [dragHeight, setDragHeight] = useState<number | null>(null);
  const listHeight = dragHeight ?? savedHeight;
  const width = metric(settings?.bookmarkEditorWidth, CHROME.bookmarkEditorWidth);
  const [bookmark, setBookmark] = useState<BookmarkNode | null>(null);
  const [created, setCreated] = useState(false);
  const [name, setName] = useState(title);
  const [place, setPlace] = useState<BookmarkPlace | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; target: BookmarkEditorMenuTarget } | null>(null);
  const [compose, setCompose] = useState<BookmarkEditorCompose | null>(null);
  const [expandRequest, setExpandRequest] = useState<{ id: string; token: number } | null>(null);
  const started = useRef(false);
  const placeKnown = useRef(false);
  const seenBookmark = useRef(false);
  const expandToken = useRef(0);
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

  useEffect(() => {
    if (!bookmark || placeKnown.current) {
      return;
    }
    const found = findBookmarkParent(tree, bookmark.id);
    if (found) {
      placeKnown.current = true;
      setPlace({ parentId: found.parentId, index: found.index });
    }
  }, [bookmark, tree]);

  useEffect(() => {
    if (bookmark) {
      nameRef.current?.focus();
      nameRef.current?.select();
    }
  }, [bookmark]);

  useEffect(() => {
    setDragHeight(null);
  }, [savedHeight]);

  useEffect(() => {
    if (!bookmark || !tree) {
      return;
    }
    if (findFolder(tree, bookmark.id)) {
      seenBookmark.current = true;
      return;
    }
    if (!seenBookmark.current) {
      return;
    }
    seenBookmark.current = false;
    setBookmark(null);
    setMenu(null);
    setCompose(null);
    setError("That bookmark was removed.");
  }, [tree, bookmark]);

  useEffect(() => {
    if (!tree || !place || findFolder(tree, place.parentId)) {
      return;
    }
    const still = bookmark ? findBookmarkParent(tree, bookmark.id) : null;
    const next = still ?? { parentId: "bar", index: 0 };
    if (next.parentId === place.parentId && next.index === place.index) {
      return;
    }
    setPlace(next);
  }, [tree, place, bookmark]);

  useEffect(() => {
    if (!menu?.target.node || !tree) {
      return;
    }
    if (!findFolder(tree, menu.target.node.id)) {
      setMenu(null);
    }
  }, [tree, menu]);

  useEffect(() => {
    if (!menu) {
      return;
    }
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [menu]);

  const onListHeight = (height: number, done: boolean) => {
    const next = Math.min(CHROME_LIMITS.listHeight.max, Math.max(CHROME_LIMITS.listHeight.min, height));
    setDragHeight(next);
    if (done && settings && next !== settings.bookmarkEditorListHeight) {
      void saveSettings({ ...settings, bookmarkEditorListHeight: next });
    }
  };

  const save = async () => {
    if (!bookmark || !place) {
      return;
    }
    try {
      if (name.trim() && name !== bookmark.title) {
        await updateBookmark(bookmark.id, name.trim(), bookmark.url);
      }
      const current = findBookmarkParent(tree, bookmark.id);
      if (!current || current.parentId !== place.parentId || current.index !== place.index) {
        await moveBookmark(bookmark.id, place.parentId, place.index);
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

  const reveal = (parentId: string) => {
    expandToken.current += 1;
    setExpandRequest({ id: parentId, token: expandToken.current });
  };

  const shiftForInsert = (parentId: string, index: number) => {
    setPlace((current) => {
      if (!current || current.parentId !== parentId || index > current.index) {
        return current;
      }
      return { parentId, index: current.index + 1 };
    });
    reveal(parentId);
  };

  const shiftForDelete = (parentId: string, index: number) => {
    setPlace((current) => {
      if (!current || current.parentId !== parentId || index >= current.index) {
        return current;
      }
      return { parentId, index: current.index - 1 };
    });
  };

  const onRowMenu = (event: MouseEvent, node: BookmarkNode, parentId: string | null, index: number) => {
    setCompose(null);
    setMenu({ x: event.clientX, y: event.clientY, target: { node, parentId, index } });
  };

  const onBlankMenu = (event: MouseEvent) => {
    if (!place) {
      return;
    }
    setCompose(null);
    setMenu({
      x: event.clientX,
      y: event.clientY,
      target: { node: null, parentId: place.parentId, index: place.index },
    });
  };

  const addDivider = async (parentId: string, index: number) => {
    try {
      await addBookmark(parentId, "", null, index, true);
      shiftForInsert(parentId, index);
      setError(null);
    } catch (reason) {
      setError(String(reason));
    }
  };

  const removeNode = async (node: BookmarkNode, parentId: string, index: number) => {
    try {
      await deleteBookmark(node.id);
      shiftForDelete(parentId, index);
      setError(null);
    } catch (reason) {
      setError(String(reason));
    }
  };

  const saveCompose = async (title: string, link: string) => {
    if (!compose) {
      return;
    }
    if (compose.kind === "rename") {
      await updateBookmark(compose.node.id, title, compose.node.url ? link : null);
    } else if (compose.kind === "add-folder") {
      await addBookmark(compose.parentId, title, null, compose.index);
      shiftForInsert(compose.parentId, compose.index);
    } else {
      await addBookmark(compose.parentId, title, link, compose.index);
      shiftForInsert(compose.parentId, compose.index);
    }
    setCompose(null);
    setError(null);
  };

  return (
    <PopupFrame
      id="bookmark-editor"
      width={width}
      maxHeight={Math.ceil(listHeight * 16 + 200 + textSize * 96)}
      ready={loaded && (bookmark !== null || error !== null)}
    >
      <form
        id="bookmark-editor-form"
        className="flex flex-col"
        style={bookmarkEditorFrameStyle(textSize, padding)}
        onSubmit={(event) => {
          event.preventDefault();
          void save();
        }}
      >
        <h2 id="bookmark-editor-title" className="font-semibold">
          {created ? "Bookmark added" : "Edit bookmark"}
        </h2>
        <label id="bookmark-editor-name-label" className="flex flex-col gap-[0.25em] text-[0.8em] text-zinc-400">
          Name
          <input
            id="bookmark-editor-name"
            ref={nameRef}
            value={name}
            onChange={(event) => setName(event.target.value)}
            className="rounded-md border border-zinc-700 bg-zinc-950 px-[0.55em] text-zinc-100 outline-none focus:border-sky-500"
            style={{ height: "2.15em", fontSize: `${textSize}rem` }}
          />
        </label>
        <div id="bookmark-editor-place" className="flex min-h-0 flex-col gap-[0.25em]">
          <span id="bookmark-editor-place-label" className="text-[0.8em] text-zinc-400">
            Place
          </span>
          {bookmark && place ? (
            <BookmarkPlacePicker
              tree={tree}
              bookmarkId={bookmark.id}
              name={name}
              url={bookmark.url ?? url}
              place={place}
              textSize={textSize}
              rowPadding={rowPadding}
              iconSize={iconSize}
              listHeight={listHeight}
              onChange={setPlace}
              onListHeight={onListHeight}
              onRowMenu={onRowMenu}
              onBlankMenu={onBlankMenu}
              expandRequest={expandRequest}
            />
          ) : (
            <p id="bookmark-editor-place-loading" className="text-[0.85em] text-zinc-500">
              Loading bookmarks…
            </p>
          )}
        </div>
        {error ? (
          <p id="bookmark-editor-error" className="text-[0.85em] text-red-300">
            {error}
          </p>
        ) : null}
        <div id="bookmark-editor-actions" className="flex justify-end gap-[0.5em]">
          <Button id="bookmark-editor-remove" className="h-[2.15em] px-[0.8em] text-[1em]" variant="ghost" disabled={!bookmark} onClick={() => void remove()}>
            Remove
          </Button>
          <Button id="bookmark-editor-done" className="h-[2.15em] px-[0.8em] text-[1em]" type="submit" variant="primary" disabled={!bookmark || !place}>
            Done
          </Button>
        </div>
      </form>
      {menu && bookmark ? (
        <BookmarkEditorContextMenu
          x={menu.x}
          y={menu.y}
          target={menu.target}
          bookmarkId={bookmark.id}
          tree={tree}
          onClose={() => setMenu(null)}
          onAddBookmark={(parentId, index) => setCompose({ kind: "add-link", parentId, index })}
          onAddFolder={(parentId, index) => setCompose({ kind: "add-folder", parentId, index })}
          onAddDivider={(parentId, index) => void addDivider(parentId, index)}
          onRename={(node) => setCompose({ kind: "rename", node })}
          onDelete={(node, parentId, index) => void removeNode(node, parentId, index)}
        />
      ) : null}
      {compose ? (
        <BookmarkEditorComposeDialog
          key={compose.kind === "rename" ? `rename-${compose.node.id}` : `${compose.kind}-${compose.parentId}-${compose.index}`}
          compose={compose}
          onClose={() => setCompose(null)}
          onSave={saveCompose}
        />
      ) : null}
    </PopupFrame>
  );
}
