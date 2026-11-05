import { FavIcon } from "@/components/FavIcon";
import { bookmarkItemStyle } from "@/lib/bookmark-size";
import { focusRow } from "@/lib/focus-ring";
import { faviconKey } from "@/lib/format";
import { cn } from "@/lib/cn";
import { CHROME, CHROME_LIMITS } from "@/lib/chrome-metrics";
import { findFolder, useSiteFavicons } from "@/lib/use-browser";
import { BookmarkNode, BookmarkTree } from "@type/browser";
import { ChevronRight } from "lucide-react";
import { createContext, useContext, useEffect, useMemo, useRef, useState, type CSSProperties, type MouseEvent, type ReactNode, type RefObject } from "react";

/** Where the bookmark will sit. `index` is in that folder's current children, counting this bookmark if it is already there. */
export type BookmarkPlace = {
  parentId: string;
  index: number;
};

type BookmarkPlacePickerProps = {
  tree: BookmarkTree | null;
  bookmarkId: string;
  name: string;
  url: string;
  place: BookmarkPlace;
  textSize: number;
  rowPadding: number;
  iconSize: number;
  listHeight: number;
  onChange: (place: BookmarkPlace) => void;
  /** `done` is true when the drag ends or the height is reset. */
  onListHeight: (height: number, done: boolean) => void;
};

type PlaceContextValue = {
  bookmarkId: string;
  previewName: string;
  previewUrl: string;
  previewIcon: string | null;
  place: BookmarkPlace;
  needle: string;
  expanded: ReadonlySet<string>;
  icons: Record<string, string>;
  rowStyle: CSSProperties;
  iconSize: number;
  previewRef: RefObject<HTMLDivElement | null>;
  onToggle: (id: string) => void;
  onOpen: (id: string) => void;
  onChange: (place: BookmarkPlace) => void;
};

const PlaceContext = createContext<PlaceContextValue | null>(null);

function usePlace() {
  const value = useContext(PlaceContext);
  if (!value) {
    throw new Error("Bookmark place picker is missing its context.");
  }
  return value;
}

function isFolder(node: BookmarkNode) {
  return !node.url && !node.separator;
}

function indent(depth: number) {
  return { paddingLeft: `${depth * 1.15}em` };
}

/** A place row, with the tree indent added to its side padding. Vertical padding stays tight. */
function placedStyle(depth: number, rowStyle: CSSProperties): CSSProperties {
  const side = typeof rowStyle.paddingInline === "string" ? rowStyle.paddingInline : "0.5rem";
  return {
    ...rowStyle,
    lineHeight: 1.2,
    paddingBlock: "0.2rem",
    paddingLeft: `calc(${depth * 1.15}em + ${side})`,
  };
}

function keepFocus(event: MouseEvent) {
  event.preventDefault();
}

function textHit(node: BookmarkNode, needle: string) {
  if (!needle || node.separator) {
    return false;
  }
  return node.title.toLowerCase().includes(needle) || (node.url ?? "").toLowerCase().includes(needle);
}

/** True when `node` is the chosen folder or contains it. */
function holdsPlace(node: BookmarkNode, parentId: string): boolean {
  if (node.id === parentId) {
    return true;
  }
  return (node.children ?? []).some((child) => isFolder(child) && holdsPlace(child, parentId));
}

function containsMatch(node: BookmarkNode, needle: string, bookmarkId: string): boolean {
  for (const child of node.children ?? []) {
    if (child.id === bookmarkId) {
      continue;
    }
    if (!child.separator && textHit(child, needle)) {
      return true;
    }
    if (isFolder(child) && containsMatch(child, needle, bookmarkId)) {
      return true;
    }
  }
  return false;
}

function shown(node: BookmarkNode, needle: string, bookmarkId: string, parentId: string) {
  if (node.id === bookmarkId) {
    return false;
  }
  if (!needle) {
    return true;
  }
  if (isFolder(node) && holdsPlace(node, parentId)) {
    return true;
  }
  if (node.separator) {
    return false;
  }
  if (textHit(node, needle)) {
    return true;
  }
  return isFolder(node) && containsMatch(node, needle, bookmarkId);
}

function foldersToOpen(tree: BookmarkTree, needle: string, bookmarkId: string, parentId: string) {
  const ids: string[] = [];
  const walk = (node: BookmarkNode) => {
    if (!isFolder(node)) {
      return;
    }
    if (holdsPlace(node, parentId) || containsMatch(node, needle, bookmarkId)) {
      ids.push(node.id);
    }
    for (const child of node.children ?? []) {
      walk(child);
    }
  };
  walk(tree.bar);
  walk(tree.other);
  return ids;
}

/** Folder ids from the root down to the folder that holds `id`. */
function ancestorFolderIds(tree: BookmarkTree | null, id: string) {
  if (!tree) {
    return null;
  }
  const walk = (node: BookmarkNode, trail: string[]): string[] | null => {
    if (node.id === id) {
      return trail;
    }
    const next = isFolder(node) ? [...trail, node.id] : trail;
    for (const child of node.children ?? []) {
      const found = walk(child, next);
      if (found) {
        return found;
      }
    }
    return null;
  };
  return walk(tree.bar, []) ?? walk(tree.other, []);
}

function collectUrls(tree: BookmarkTree | null, url: string) {
  const urls: string[] = [];
  if (url) {
    urls.push(url);
  }
  const walk = (node: BookmarkNode) => {
    if (node.url) {
      urls.push(node.url);
    }
    for (const child of node.children ?? []) {
      walk(child);
    }
  };
  if (tree) {
    walk(tree.bar);
    walk(tree.other);
  }
  return urls;
}

function iconSrc(icons: Record<string, string>, url: string | null) {
  if (!url) {
    return null;
  }
  return icons[faviconKey(url) ?? ""] ?? null;
}

function successorLabel(node: BookmarkNode) {
  if (node.separator) {
    return "divider";
  }
  return node.title || node.url || "bookmark";
}

function placeCaption(tree: BookmarkTree | null, place: BookmarkPlace, bookmarkId: string) {
  const folder = findFolder(tree, place.parentId);
  if (!folder) {
    return "";
  }
  const children = folder.children ?? [];
  let successor: BookmarkNode | null = null;
  for (let index = place.index; index < children.length; index++) {
    if (children[index].id !== bookmarkId) {
      successor = children[index];
      break;
    }
  }
  const where = successor ? `before ${successorLabel(successor)}` : "at the end";
  return `${folder.title}, ${where}`;
}

function Preview({ depth }: { depth: number }) {
  const { previewName, previewUrl, previewIcon, previewRef, rowStyle, iconSize } = usePlace();
  return (
    <div
      id="bookmark-place-preview"
      ref={previewRef}
      aria-current="true"
      title={previewName}
      className="flex min-w-0 items-center gap-[0.5em] rounded-md bg-sky-500/15 text-sky-100 ring-1 ring-inset ring-sky-500"
      style={placedStyle(depth, rowStyle)}
    >
      <FavIcon id="bookmark-place-preview-icon" url={previewUrl} src={previewIcon} style={{ width: `${iconSize}rem`, height: `${iconSize}rem` }} />
      <span id="bookmark-place-preview-name" className="truncate">
        {previewName}
      </span>
    </div>
  );
}

function gapLabel(node: BookmarkNode) {
  if (node.separator) {
    return "Place before divider";
  }
  return `Place before ${node.title || node.url || "bookmark"}`;
}

/** The line between two rows. Hovering shows it. Clicking it chooses that slot. */
function PlaceGap({ id, depth, label, onChoose }: { id: string; depth: number; label: string; onChoose: () => void }) {
  return (
    <button
      id={id}
      type="button"
      aria-label={label}
      title={label}
      className="group relative z-10 -my-1 flex h-2 w-full items-center"
      style={indent(depth)}
      onMouseDown={keepFocus}
      onClick={onChoose}
    >
      <span id={`${id}-line`} className="h-0.5 w-full rounded-full bg-transparent group-hover:bg-sky-500 group-focus-visible:bg-sky-500" />
    </button>
  );
}

function LinkRow({ node, depth }: { node: BookmarkNode; depth: number }) {
  const { icons, rowStyle, iconSize } = usePlace();
  const label = node.title || node.url || "Bookmark";
  return (
    <div
      id={`bookmark-place-${node.id}`}
      title={node.url ? `${label} — ${node.url}` : label}
      className="flex w-full min-w-0 items-center gap-[0.5em] rounded-md text-left text-zinc-200"
      style={placedStyle(depth, rowStyle)}
    >
      <FavIcon id={`bookmark-place-${node.id}-icon`} url={node.url ?? ""} src={iconSrc(icons, node.url)} style={{ width: `${iconSize}rem`, height: `${iconSize}rem` }} />
      <span id={`bookmark-place-${node.id}-title`} className="truncate">
        {label}
      </span>
    </div>
  );
}

function DividerRow({ node, depth }: { node: BookmarkNode; depth: number }) {
  const { rowStyle } = usePlace();
  return (
    <div
      id={`bookmark-place-${node.id}`}
      title="Divider"
      className="flex w-full items-center rounded-md"
      style={placedStyle(depth, rowStyle)}
    >
      <span id={`bookmark-place-${node.id}-rule`} className="h-px w-full bg-zinc-700" />
    </div>
  );
}

function PlaceFolder({ node, depth }: { node: BookmarkNode; depth: number }) {
  const picker = usePlace();
  const open = picker.expanded.has(node.id);
  const children = node.children ?? [];
  const childDepth = depth + 1;
  const selectedHere = picker.place.parentId === node.id;
  const items: ReactNode[] = [];
  let previewPlaced = false;

  if (open) {
    for (let index = 0; index < children.length; index++) {
      const child = children[index];
      if (selectedHere && index === picker.place.index) {
        items.push(<Preview key="bookmark-place-preview" depth={childDepth} />);
        previewPlaced = true;
      }
      if (!shown(child, picker.needle, picker.bookmarkId, picker.place.parentId)) {
        continue;
      }
      if (!selectedHere || index !== picker.place.index) {
        items.push(
          <PlaceGap
            key={`gap-${child.id}`}
            id={`bookmark-place-gap-${node.id}-${child.id}`}
            depth={childDepth}
            label={gapLabel(child)}
            onChoose={() => picker.onChange({ parentId: node.id, index })}
          />,
        );
      }
      if (isFolder(child)) {
        items.push(<PlaceFolder key={child.id} node={child} depth={childDepth} />);
      } else if (child.separator) {
        items.push(<DividerRow key={child.id} node={child} depth={childDepth} />);
      } else {
        items.push(<LinkRow key={child.id} node={child} depth={childDepth} />);
      }
    }
    const chooseEnd = () => picker.onChange({ parentId: node.id, index: children.length });
    const hasOther = children.some((child) => child.id !== picker.bookmarkId);
    if (selectedHere && !previewPlaced) {
      items.push(<Preview key="bookmark-place-preview" depth={childDepth} />);
    } else if (!hasOther && !selectedHere) {
      items.push(
        <button
          key="empty"
          id={`bookmark-place-empty-${node.id}`}
          type="button"
          aria-label={`Place at the end of ${node.title}`}
          className={cn(focusRow, "flex w-full items-center rounded-md text-left text-[0.85em] text-zinc-500 hover:bg-zinc-800 hover:text-zinc-300")}
          style={placedStyle(childDepth, picker.rowStyle)}
          onMouseDown={keepFocus}
          onClick={chooseEnd}
        >
          Empty
        </button>,
      );
    } else {
      items.push(
        <PlaceGap
          key="end"
          id={`bookmark-place-end-${node.id}`}
          depth={childDepth}
          label={`Place at the end of ${node.title}`}
          onChoose={chooseEnd}
        />,
      );
    }
  }

  return (
    <div id={`bookmark-place-branch-${node.id}`} className="min-w-0">
      <div id={`bookmark-place-folder-${node.id}`} className="flex min-w-0 items-center" style={indent(depth)}>
        <button
          id={`bookmark-place-toggle-${node.id}`}
          type="button"
          aria-expanded={open}
          aria-label={`${open ? "Collapse" : "Expand"} ${node.title}`}
          className={cn(focusRow, "flex shrink-0 items-center justify-center rounded-md text-zinc-400 hover:bg-zinc-800")}
          style={{ width: `${picker.iconSize + 0.35}rem`, height: `${picker.iconSize + 0.35}rem` }}
          onMouseDown={keepFocus}
          onClick={() => picker.onToggle(node.id)}
        >
          <ChevronRight
            id={`bookmark-place-toggle-${node.id}-icon`}
            className={cn("transition-transform", open && "rotate-90")}
            style={{ width: `${picker.iconSize * 0.85}rem`, height: `${picker.iconSize * 0.85}rem` }}
          />
        </button>
        <button
          id={`bookmark-place-into-${node.id}`}
          type="button"
          title={node.title}
          className={cn(
            focusRow,
            "flex min-w-0 flex-1 items-center rounded-md text-left text-zinc-100 hover:bg-zinc-800",
            selectedHere && !open && "bg-sky-500/15 text-sky-100 ring-1 ring-inset ring-sky-500 hover:bg-sky-500/20",
          )}
          style={picker.rowStyle}
          onMouseDown={keepFocus}
          onClick={() => {
            picker.onOpen(node.id);
            picker.onChange({ parentId: node.id, index: children.length });
          }}
        >
          <span id={`bookmark-place-into-${node.id}-title`} className="truncate">
            {node.title}
          </span>
        </button>
      </div>
      {open ? <div id={`bookmark-place-children-${node.id}`}>{items}</div> : null}
    </div>
  );
}

function clampListHeight(value: number) {
  const next = Math.min(CHROME_LIMITS.listHeight.max, Math.max(CHROME_LIMITS.listHeight.min, value));
  return Math.round(next * 100) / 100;
}

export function BookmarkPlacePicker({
  tree,
  bookmarkId,
  name,
  url,
  place,
  textSize,
  rowPadding,
  iconSize,
  listHeight,
  onChange,
  onListHeight,
}: BookmarkPlacePickerProps) {
  const [query, setQuery] = useState("");
  const needle = query.trim().toLowerCase();
  const previewRef = useRef<HTMLDivElement>(null);
  const resizeDrag = useRef<{ y: number; height: number } | null>(null);
  const resizeHeight = useRef(listHeight);
  resizeHeight.current = listHeight;
  const urls = useMemo(() => collectUrls(tree, url), [tree, url]);
  const icons = useSiteFavicons(urls);
  const previewName = name.trim() || url || "Bookmark";
  const previewIcon = iconSrc(icons, url);
  const rowStyle = {
    ...bookmarkItemStyle(textSize, rowPadding, { text: textSize, padding: rowPadding }),
    lineHeight: 1.2,
    paddingBlock: "0.2rem",
  };
  const seeded = useRef(ancestorFolderIds(tree, bookmarkId) !== null);
  const [expanded, setExpanded] = useState(() => new Set(ancestorFolderIds(tree, bookmarkId) ?? []));

  useEffect(() => {
    if (seeded.current || !tree) {
      return;
    }
    const path = ancestorFolderIds(tree, bookmarkId);
    if (!path) {
      return;
    }
    seeded.current = true;
    setExpanded(new Set(path));
  }, [tree, bookmarkId]);

  useEffect(() => {
    const q = query.trim().toLowerCase();
    if (!q || !tree) {
      return;
    }
    const ids = foldersToOpen(tree, q, bookmarkId, place.parentId);
    setExpanded((current) => {
      let changed = false;
      const next = new Set(current);
      for (const id of ids) {
        if (!next.has(id)) {
          next.add(id);
          changed = true;
        }
      }
      return changed ? next : current;
    });
  }, [query, tree, bookmarkId, place.parentId]);

  useEffect(() => {
    previewRef.current?.scrollIntoView({ block: "nearest" });
  }, [place.parentId, place.index]);

  const toggleFolder = (id: string) => {
    setExpanded((current) => {
      const next = new Set(current);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const openFolder = (id: string) => {
    setExpanded((current) => {
      if (current.has(id)) {
        return current;
      }
      const next = new Set(current);
      next.add(id);
      return next;
    });
  };

  const context = {
    bookmarkId,
    previewName,
    previewUrl: url,
    previewIcon,
    place,
    needle,
    expanded,
    icons,
    rowStyle,
    iconSize,
    previewRef,
    onToggle: toggleFolder,
    onOpen: openFolder,
    onChange,
  };
  const roots = tree ? [tree.bar, tree.other] : [];
  const visibleRoots = roots.filter((root) => shown(root, needle, bookmarkId, place.parentId));
  const matched = !needle || !tree || textHit(tree.bar, needle) || textHit(tree.other, needle) || containsMatch(tree.bar, needle, bookmarkId) || containsMatch(tree.other, needle, bookmarkId);
  const caption = placeCaption(tree, place, bookmarkId);

  return (
    <PlaceContext.Provider value={context}>
      <div id="bookmark-place" className="flex min-h-0 flex-col gap-[0.4em]">
        <input
          id="bookmark-place-filter"
          value={query}
          aria-label="Filter bookmarks"
          placeholder="Filter bookmarks"
          autoComplete="off"
          onChange={(event) => setQuery(event.target.value)}
          className="rounded-md border border-zinc-700 bg-zinc-950 px-[0.6em] text-[0.9em] text-zinc-100 outline-none placeholder:text-zinc-500 focus:border-sky-500"
          style={{ height: "2.15em" }}
        />
        <div id="bookmark-place-list" className="relative">
          <div
            id="bookmark-place-tree"
            role="group"
            aria-label="Bookmark place"
            className="overflow-x-hidden overflow-y-auto rounded-md border border-zinc-700 bg-zinc-950 p-1"
            style={{ height: `${listHeight}rem` }}
          >
            {needle && !matched ? (
              <p id="bookmark-place-no-matches" className="px-[0.6em] py-[0.35em] text-[0.85em] text-zinc-500">
                No matches
              </p>
            ) : null}
            {visibleRoots.map((root) => (
              <PlaceFolder key={root.id} node={root} depth={0} />
            ))}
          </div>
          <div
            id="bookmark-place-resize"
            role="separator"
            aria-orientation="horizontal"
            aria-label="Resize place list"
            aria-valuemin={CHROME_LIMITS.listHeight.min}
            aria-valuemax={CHROME_LIMITS.listHeight.max}
            aria-valuenow={Math.round(listHeight)}
            title="Drag to resize. Double-click to reset."
            className="group absolute inset-x-1 -bottom-2 z-10 flex h-4 cursor-row-resize items-center justify-center"
            onPointerDown={(event) => {
              event.preventDefault();
              event.currentTarget.setPointerCapture(event.pointerId);
              resizeDrag.current = { y: event.clientY, height: listHeight };
            }}
            onPointerMove={(event) => {
              const start = resizeDrag.current;
              if (!start) {
                return;
              }
              const rem = parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
              const next = clampListHeight(start.height + (event.clientY - start.y) / rem);
              resizeHeight.current = next;
              onListHeight(next, false);
            }}
            onPointerUp={(event) => {
              if (!resizeDrag.current) {
                return;
              }
              resizeDrag.current = null;
              event.currentTarget.releasePointerCapture(event.pointerId);
              onListHeight(resizeHeight.current, true);
            }}
            onPointerCancel={() => {
              resizeDrag.current = null;
            }}
            onDoubleClick={() => onListHeight(CHROME.bookmarkEditorListHeight, true)}
          >
            <span id="bookmark-place-resize-bar" className="h-1 w-10 rounded-full bg-zinc-600 group-hover:bg-sky-500" />
          </div>
        </div>
        <p id="bookmark-place-caption" className="truncate text-[0.85em] text-zinc-500" title={caption}>
          {caption}
        </p>
      </div>
    </PlaceContext.Provider>
  );
}
