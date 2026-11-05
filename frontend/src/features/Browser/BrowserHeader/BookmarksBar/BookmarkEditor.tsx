import { Button } from "@/components/Button";
import { PopupFrame } from "@/components/PopupFrame";
import { bookmarkEditorFrameStyle } from "@/lib/bookmark-size";
import { findBookmark, addBookmark, updateBookmark, moveBookmark, closePopup, deleteBookmark, saveSettings } from "@/lib/browser-client";
import { CHROME, CHROME_LIMITS, metric } from "@/lib/chrome-metrics";
import { findBookmarkParent, useBookmarksState, useSettings } from "@/lib/use-browser";
import { BookmarkNode } from "@type/browser";
import { useEffect, useRef, useState } from "react";
import { BookmarkPlacePicker, type BookmarkPlace } from "./BookmarkPlacePicker";

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
  const started = useRef(false);
  const placeKnown = useRef(false);
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
          <Button id="bookmark-editor-remove" className="h-[2.15em] px-[0.8em] text-[length:1em]" variant="ghost" disabled={!bookmark} onClick={() => void remove()}>
            Remove
          </Button>
          <Button id="bookmark-editor-done" className="h-[2.15em] px-[0.8em] text-[length:1em]" type="submit" variant="primary" disabled={!bookmark || !place}>
            Done
          </Button>
        </div>
      </form>
    </PopupFrame>
  );
}
