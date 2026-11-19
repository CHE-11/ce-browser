import { Button } from "@/components/Button";
import { PopupFrame } from "@/components/PopupFrame";
import { bookmarkEditorFrameStyle } from "@/lib/bookmark-size";
import { findBookmark, addBookmark, updateBookmark, moveBookmark, closePopup, deleteBookmark, saveSettings } from "@/lib/browser-client";
import { CHROME, CHROME_LIMITS, metric } from "@/lib/chrome-metrics";
import { findBookmarkParent, findFolder, useBookmarksState, useSettingsState } from "@/lib/use-browser";
import { BookmarkEditorLastPlace, BookmarkNode, BookmarkTree } from "@type/browser";
import { ChevronsDownUp, ChevronsUpDown, TrashIcon } from "lucide-react";
import { useCallback, useEffect, useRef, useState, type MouseEvent } from "react";
import { BookmarkEditorCompose, BookmarkEditorComposeDialog, BookmarkEditorContextMenu, type BookmarkEditorMenuTarget } from "./BookmarkEditorContextMenu";
import { BookmarkPlace, BookmarkPlacePicker } from "./BookmarkEditorPlacePicker";

type BookmarkEditorProps = {
  url: string;
  title: string;
};

/** Where a new bookmark goes: beside the last one this editor saved, or the end of the bar. */
function placeBesideLast(tree: BookmarkTree | null, last: BookmarkEditorLastPlace | null | undefined): { parentId: string; index?: number } {
  if (!tree) {
    return { parentId: "bar" };
  }
  if (last?.bookmarkId) {
    const found = findBookmarkParent(tree, last.bookmarkId);
    if (found) {
      return { parentId: found.parentId, index: found.index + 1 };
    }
  }
  const folder = last?.parentId ? findFolder(tree, last.parentId) : null;
  if (folder) {
    const length = folder.children?.length ?? 0;
    return { parentId: folder.id, index: Math.max(0, Math.min(last?.index ?? length, length)) };
  }
  return { parentId: "bar", index: tree.bar.children?.length ?? 0 };
}

/** Index the bookmark occupies after a move to `place`. Moving down in the same folder shifts the index back by one. */
function indexAfterMove(tree: BookmarkTree | null, bookmarkId: string, place: BookmarkPlace) {
  const current = findBookmarkParent(tree, bookmarkId);
  if (current && current.parentId === place.parentId && current.index < place.index) {
    return place.index - 1;
  }
  return place.index;
}

/** The star adds the page right away, beside the last bookmark this editor saved. Done keeps the rename, move, or address and remembers that place. Cancel drops a bookmark this editor just added and leaves an existing one as it was. */
export function BookmarkEditor({ url, title }: BookmarkEditorProps) {
  const { tree, loaded } = useBookmarksState();
  const { settings, loaded: settingsLoaded } = useSettingsState();
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
  const [link, setLink] = useState(url);
  const [place, setPlace] = useState<BookmarkPlace | null>(null);
  const [foldersOpen, setFoldersOpen] = useState(true);
  const [foldRequest, setFoldRequest] = useState<{ expand: boolean; token: number } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; target: BookmarkEditorMenuTarget } | null>(null);
  const [compose, setCompose] = useState<BookmarkEditorCompose | null>(null);
  const [expandRequest, setExpandRequest] = useState<{ id: string; token: number } | null>(null);
  const started = useRef(false);
  const cancelled = useRef(false);
  const placeKnown = useRef(false);
  const seenBookmark = useRef(false);
  const expandToken = useRef(0);
  const foldToken = useRef(0);
  const nameRef = useRef<HTMLInputElement>(null);
  const settingsRef = useRef(settings);
  const rememberChain = useRef(Promise.resolve());
  settingsRef.current = settings;
  const onFoldersOpen = useCallback((open: boolean) => setFoldersOpen(open), []);
  const rememberPlace = useCallback((parentId: string, bookmarkId: string, index: number) => {
    const run = rememberChain.current.catch(() => undefined).then(async () => {
      const current = settingsRef.current;
      if (!current) {
        return;
      }
      const last = current.bookmarkEditorLastPlace;
      if (last?.parentId === parentId && last.bookmarkId === bookmarkId && last.index === index) {
        return;
      }
      const saved = await saveSettings({
        ...current,
        bookmarkEditorLastPlace: { parentId, bookmarkId, index },
      });
      if (settingsRef.current === current) {
        settingsRef.current = saved;
      }
    });
    rememberChain.current = run;
    return run;
  }, []);

  useEffect(() => {
    if (started.current || !loaded || !settingsLoaded) {
      return;
    }
    started.current = true;
    const spot = placeBesideLast(tree, settings?.bookmarkEditorLastPlace);
    void (async () => {
      try {
        const existing = await findBookmark(url);
        if (cancelled.current) {
          return;
        }
        if (existing) {
          setBookmark(existing);
          setName(existing.title);
          setLink(existing.url ?? url);
          return;
        }
        const node = await addBookmark(spot.parentId, title || url, url, spot.index);
        if (cancelled.current) {
          await deleteBookmark(node.id);
          return;
        }
        setBookmark(node);
        setCreated(true);
        if (spot.index !== undefined) {
          void rememberPlace(spot.parentId, node.id, spot.index).catch(() => undefined);
        }
      } catch (reason) {
        setError(String(reason));
      }
    })();
  }, [loaded, rememberPlace, settings, settingsLoaded, title, tree, url]);

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
    if (!done) {
      return;
    }
    const run = rememberChain.current.catch(() => undefined).then(async () => {
      const current = settingsRef.current;
      if (!current || next === current.bookmarkEditorListHeight) {
        return;
      }
      const saved = await saveSettings({ ...current, bookmarkEditorListHeight: next });
      if (settingsRef.current === current) {
        settingsRef.current = saved;
      }
    });
    rememberChain.current = run;
  };

  const save = async () => {
    if (!bookmark || !place) {
      return;
    }
    const nextUrl = link.trim();
    if (!nextUrl) {
      setError("A bookmark needs an address.");
      return;
    }
    try {
      const nextTitle = name.trim() || bookmark.title;
      if (nextTitle !== bookmark.title || nextUrl !== (bookmark.url ?? "")) {
        await updateBookmark(bookmark.id, nextTitle, nextUrl);
      }
      const current = findBookmarkParent(tree, bookmark.id);
      const moved = !current || current.parentId !== place.parentId || current.index !== place.index;
      if (moved) {
        await moveBookmark(bookmark.id, place.parentId, place.index);
      }
      if (created || moved) {
        await rememberPlace(place.parentId, bookmark.id, indexAfterMove(tree, bookmark.id, place));
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

  const cancel = async () => {
    cancelled.current = true;
    if (created && bookmark) {
      try {
        await deleteBookmark(bookmark.id);
      } catch (reason) {
        cancelled.current = false;
        setError(String(reason));
        return;
      }
    }
    await closePopup();
  };

  const toggleAllFolders = () => {
    foldToken.current += 1;
    setFoldRequest({ expand: !foldersOpen, token: foldToken.current });
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
      maxHeight={Math.ceil(listHeight * 16 + 260 + textSize * 96)}
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
        <label id="bookmark-editor-url-label" className="flex flex-col gap-[0.25em] text-[0.8em] text-zinc-400">
          URL
          <input
            id="bookmark-editor-url"
            value={link}
            spellCheck={false}
            onChange={(event) => setLink(event.target.value)}
            className="rounded-md border border-zinc-700 bg-zinc-950 px-[0.55em] text-zinc-100 outline-none focus:border-sky-500"
            style={{ height: "2.15em", fontSize: `${textSize}rem` }}
          />
        </label>
        <div id="bookmark-editor-place" className="flex min-h-0 flex-col gap-[0.25em]">
          <div id="bookmark-editor-place-heading" className="flex items-center justify-between gap-[0.5em]">
            <span id="bookmark-editor-place-label" className="text-[0.8em] text-zinc-400">
              Place
            </span>
            {bookmark && place ? (
              <Button
                id="bookmark-editor-fold-all"
                variant="ghost"
                className="h-[1.8em] gap-[0.3em] px-[0.45em] text-[0.75em]"
                onClick={toggleAllFolders}
              >
                {foldersOpen ? (
                  <ChevronsDownUp id="bookmark-editor-fold-all-icon" style={{ width: "1.1em", height: "1.1em" }} aria-hidden />
                ) : (
                  <ChevronsUpDown id="bookmark-editor-fold-all-icon" style={{ width: "1.1em", height: "1.1em" }} aria-hidden />
                )}
                {foldersOpen ? "Collapse all" : "Expand all"}
              </Button>
            ) : null}
          </div>
          {bookmark && place ? (
            <BookmarkPlacePicker
              tree={tree}
              bookmarkId={bookmark.id}
              name={name}
              url={link.trim() || bookmark.url || url}
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
              foldRequest={foldRequest}
              onFoldersOpen={onFoldersOpen}
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
        <div id="bookmark-editor-actions" className="flex justify-between gap-[0.5em] mt-2">
          <Button id="bookmark-editor-remove" className="h-[2.15em] px-[0.8em] text-[1em]" variant="destructiveOutline" disabled={!bookmark} onClick={() => void remove()}>
            <TrashIcon id="bookmark-editor-remove-icon" style={{ width: "1.1em", height: "1.1em" }} aria-hidden />
            Remove
          </Button>
          <div id="bookmark-editor-save-actions" className="flex justify-end gap-[0.5em]">
            <Button id="bookmark-editor-cancel" className="h-[2.15em] px-[0.8em] text-[1em]" type="button" variant="ghost" onClick={() => void cancel()}>
              Cancel
            </Button>
            <Button id="bookmark-editor-done" className="h-[2.15em] px-[0.8em] text-[1em]" type="submit" variant="primary" disabled={!bookmark || !place || !link.trim()}>
              Done
            </Button>
          </div>
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
