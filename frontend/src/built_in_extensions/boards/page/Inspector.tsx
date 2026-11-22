import { useEffect, useMemo, useState, type ReactNode } from "react";
import { Copy, Download, ExternalLink, File, FolderOpen, FolderPlus, Maximize2, RotateCcw, Star, Trash2, X } from "lucide-react";
import { Button } from "@/components/Button";
import { FavIcon } from "@/components/FavIcon";
import { Toggle } from "@/components/Toggle";
import { addPinTags, removePinTags, removePinsFromFolder, renameBoardFolder, updateBoardFolder, updatePin } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import type { BoardFolder, BoardsSnapshot, Pin, PinSort } from "@type/browser";
import { FolderGlyph } from "../folder-icons";
import {
  FOLDER_COLORS,
  FOLDER_COLOR_CLASS,
  PIN_KIND_LABELS,
  PIN_SORTS,
  PIN_SORT_LABELS,
  folderLabel,
  formatBytes,
  formatDimensions,
  hasPreview,
  hostOf,
  pinName,
  pinsInFolder,
  previewUrl,
  tagCounts,
} from "../folder-tree";
import { scopeTitle, type BoardsScope } from "../scopes";
import type { PinMenuActions } from "./PinContextMenu";
import { TagInput } from "./TagInput";

type InspectorProps = {
  boards: BoardsSnapshot;
  scope: BoardsScope;
  selected: Pin[];
  /** Pins the current view lists, for the summary when nothing is selected. */
  shown: Pin[];
  actions: PinMenuActions;
  onScope: (scope: BoardsScope) => void;
  /** Filter the view by a palette color. */
  onColor: (color: string) => void;
  onExportFolder: (id: string) => void;
  onError: (reason: unknown) => void;
};

const fieldClass = "w-full rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500";

/** The right-hand panel: details of the selected pins, or of the folder being viewed. */
export function Inspector({ boards, scope, selected, shown, actions, onScope, onColor, onExportFolder, onError }: InspectorProps) {
  const knownTags = useMemo(() => tagCounts(boards).map((tag) => tag.name), [boards]);
  const folder = scope.kind === "folder" ? boards.folders.find((item) => item.id === scope.id) : undefined;
  let body: ReactNode;
  if (selected.length === 1) {
    body = <PinDetails key={selected[0].id} boards={boards} pin={selected[0]} knownTags={knownTags} actions={actions} onScope={onScope} onColor={onColor} onError={onError} />;
  } else if (selected.length > 1) {
    body = <ManyDetails boards={boards} pins={selected} knownTags={knownTags} actions={actions} onScope={onScope} onError={onError} />;
  } else if (folder) {
    body = <FolderDetails key={folder.id} boards={boards} folder={folder} onExport={() => onExportFolder(folder.id)} onError={onError} />;
  } else {
    body = <ScopeSummary boards={boards} scope={scope} shown={shown} />;
  }
  return (
    <aside id="boards-inspector" className="flex w-72 shrink-0 flex-col overflow-y-auto border-l border-zinc-800 bg-zinc-950 p-4">
      {body}
    </aside>
  );
}

function Section({ id, label, children, action }: { id: string; label: string; children: ReactNode; action?: ReactNode }) {
  return (
    <section id={id} className="flex flex-col gap-1.5">
      <div id={`${id}-header`} className="flex items-center justify-between">
        <h3 id={`${id}-label`} className="text-[11px] font-medium uppercase tracking-wide text-zinc-500">
          {label}
        </h3>
        {action}
      </div>
      {children}
    </section>
  );
}

function Rating({ id, value, onChange }: { id: string; value: number; onChange: (rating: number) => void }) {
  const [hover, setHover] = useState(0);
  const shownValue = hover || value;
  return (
    <div id={id} className="flex items-center gap-0.5" onMouseLeave={() => setHover(0)}>
      {[1, 2, 3, 4, 5].map((stars) => (
        <button
          id={`${id}-${stars}`}
          key={stars}
          type="button"
          aria-label={`${stars} ${stars === 1 ? "star" : "stars"}`}
          aria-pressed={stars <= value}
          className="inline-flex size-6 items-center justify-center rounded text-zinc-600 hover:bg-zinc-900"
          onMouseEnter={() => setHover(stars)}
          onClick={() => onChange(stars === value ? 0 : stars)}
        >
          <Star id={`${id}-${stars}-icon`} className={cn("size-4", stars <= shownValue && "fill-current text-amber-300")} aria-hidden />
        </button>
      ))}
    </div>
  );
}

function FolderChips({
  id,
  boards,
  folderIds,
  onScope,
  onRemove,
}: {
  id: string;
  boards: BoardsSnapshot;
  folderIds: string[];
  onScope: (scope: BoardsScope) => void;
  onRemove: (folderId: string) => void;
}) {
  if (folderIds.length === 0) {
    return (
      <p id={`${id}-none`} className="text-xs text-zinc-500">
        Uncategorized
      </p>
    );
  }
  return (
    <ul id={id} className="flex flex-col gap-1">
      {folderIds.map((folderId) => {
        const folder = boards.folders.find((item) => item.id === folderId);
        if (!folder) {
          return null;
        }
        return (
          <li id={`${id}-${folderId}`} key={folderId} className="group flex items-center gap-1.5 rounded-md bg-zinc-900 py-1 pl-2 pr-1 text-xs text-zinc-200">
            <FolderGlyph id={`${id}-${folderId}-icon`} folder={folder} className="size-3.5 text-zinc-500" />
            <button id={`${id}-${folderId}-open`} type="button" className="min-w-0 flex-1 truncate text-left hover:text-sky-300" title={folderLabel(boards, folderId)} onClick={() => onScope({ kind: "folder", id: folderId })}>
              {folderLabel(boards, folderId)}
            </button>
            <button
              id={`${id}-${folderId}-remove`}
              type="button"
              aria-label={`Remove from ${folder.name}`}
              className="inline-flex size-5 items-center justify-center rounded text-zinc-500 opacity-0 hover:bg-zinc-800 hover:text-zinc-100 group-hover:opacity-100"
              onClick={() => onRemove(folderId)}
            >
              <X id={`${id}-${folderId}-remove-icon`} className="size-3" aria-hidden />
            </button>
          </li>
        );
      })}
    </ul>
  );
}

function Property({ id, label, value }: { id: string; label: string; value: ReactNode }) {
  if (value === "" || value === null || value === undefined) {
    return null;
  }
  return (
    <>
      <dt id={`${id}-label`} className="text-zinc-500">
        {label}
      </dt>
      <dd id={id} className="min-w-0 truncate text-right text-zinc-300">
        {value}
      </dd>
    </>
  );
}

function formatDate(time: number) {
  return time > 0 ? new Date(time).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" }) : "";
}

type PinDetailsProps = {
  boards: BoardsSnapshot;
  pin: Pin;
  knownTags: string[];
  actions: PinMenuActions;
  onScope: (scope: BoardsScope) => void;
  onColor: (color: string) => void;
  onError: (reason: unknown) => void;
};

function PinDetails({ boards, pin, knownTags, actions, onScope, onColor, onError }: PinDetailsProps) {
  const [title, setTitle] = useState(pin.title);
  const [note, setNote] = useState(pin.note);
  const [link, setLink] = useState(pin.linkUrl);
  const src = previewUrl(pin);
  const inTrash = pin.deletedAt !== null;

  useEffect(() => setTitle(pin.title), [pin.title]);
  useEffect(() => setNote(pin.note), [pin.note]);
  useEffect(() => setLink(pin.linkUrl), [pin.linkUrl]);

  const save = (patch: Parameters<typeof updatePin>[1]) => void updatePin(pin.id, patch).catch(onError);
  const isFile = pin.kind === "file" || pin.kind === "image";

  return (
    <div id="boards-inspector-pin" className="flex flex-col gap-4">
      {src ? (
        <button id="boards-inspector-preview" type="button" className="overflow-hidden rounded-md bg-zinc-900 ring-1 ring-zinc-800" onClick={() => actions.open(pin)}>
          <img id="boards-inspector-preview-image" src={src} alt="" className="max-h-56 w-full object-contain" />
        </button>
      ) : null}

      {pin.palette.length > 0 ? (
        <div id="boards-inspector-palette" className="flex flex-wrap gap-1.5">
          {pin.palette.map((entry, index) => (
            <button
              id={`boards-inspector-palette-${index}`}
              key={`${entry.color}-${index}`}
              type="button"
              title={`${entry.color} · ${Math.round(entry.ratio * 100)}% — show items with this color`}
              aria-label={`Filter by ${entry.color}`}
              className="size-5 rounded-full ring-1 ring-white/15 transition hover:scale-110"
              style={{ backgroundColor: entry.color }}
              onClick={() => onColor(entry.color)}
            />
          ))}
        </div>
      ) : null}

      <div id="boards-inspector-fields" className="flex flex-col gap-2">
        <input
          id="boards-inspector-title"
          value={title}
          aria-label="Name"
          placeholder="Name"
          className={cn(fieldClass, "h-8 font-medium")}
          onChange={(event) => setTitle(event.target.value)}
          onBlur={() => title !== pin.title && save({ title })}
          onKeyDown={(event) => event.key === "Enter" && event.currentTarget.blur()}
        />
        <textarea
          id="boards-inspector-note"
          value={note}
          rows={3}
          aria-label="Notes"
          placeholder="Notes"
          className={cn(fieldClass, "resize-y py-1.5 text-xs")}
          onChange={(event) => setNote(event.target.value)}
          onBlur={() => note !== pin.note && save({ note })}
        />
        <label id="boards-inspector-link-label" className="flex h-8 items-center gap-1.5 rounded-md border border-zinc-700 bg-zinc-950 pl-2 focus-within:border-sky-500">
          <FavIcon id="boards-inspector-link-icon" url={link || pin.pageUrl} className="size-3.5 shrink-0" />
          <input
            id="boards-inspector-link"
            value={link}
            aria-label="Address"
            placeholder="https://"
            className="h-full min-w-0 flex-1 bg-transparent text-xs text-zinc-100 outline-none"
            onChange={(event) => setLink(event.target.value)}
            onBlur={() => link !== pin.linkUrl && save({ linkUrl: link })}
            onKeyDown={(event) => event.key === "Enter" && event.currentTarget.blur()}
          />
          {link || pin.pageUrl ? (
            <button id="boards-inspector-link-open" type="button" aria-label="Open address" className="inline-flex size-7 items-center justify-center text-zinc-500 hover:text-zinc-100" onClick={() => actions.openSource([pin])}>
              <ExternalLink id="boards-inspector-link-open-icon" className="size-3.5" aria-hidden />
            </button>
          ) : null}
        </label>
      </div>

      <Section id="boards-inspector-tags" label="Tags">
        <TagInput
          id="boards-inspector-tag-input"
          tags={pin.tags}
          known={knownTags}
          onAdd={(tags) => void addPinTags([pin.id], tags).catch(onError)}
          onRemove={(tag) => void removePinTags([pin.id], [tag]).catch(onError)}
          onOpenTag={(tag) => onScope({ kind: "tag", name: tag })}
        />
      </Section>

      <Section
        id="boards-inspector-folders"
        label="Folders"
        action={
          <button
            id="boards-inspector-folders-add"
            type="button"
            aria-label="Add to folder"
            className="inline-flex size-6 items-center justify-center rounded text-zinc-500 hover:bg-zinc-900 hover:text-zinc-100"
            onClick={(event) => {
              const rect = event.currentTarget.getBoundingClientRect();
              actions.addToFolder([pin], rect.left - 220, rect.bottom + 4);
            }}
          >
            <FolderPlus id="boards-inspector-folders-add-icon" className="size-3.5" aria-hidden />
          </button>
        }
      >
        <FolderChips id="boards-inspector-folder-list" boards={boards} folderIds={pin.folderIds} onScope={onScope} onRemove={(folderId) => void removePinsFromFolder([pin.id], folderId).catch(onError)} />
      </Section>

      <Section id="boards-inspector-rating" label="Rating">
        <Rating id="boards-inspector-rating-stars" value={pin.rating} onChange={(rating) => actions.rate([pin], rating)} />
      </Section>

      <Section id="boards-inspector-properties" label="Properties">
        <dl id="boards-inspector-properties-list" className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1 text-xs">
          <Property id="boards-inspector-prop-type" label="Type" value={pin.ext ? `${pin.ext.toUpperCase()} · ${PIN_KIND_LABELS[pin.kind]}` : PIN_KIND_LABELS[pin.kind]} />
          <Property id="boards-inspector-prop-dimensions" label="Dimensions" value={formatDimensions(pin)} />
          <Property id="boards-inspector-prop-size" label="File size" value={formatBytes(pin.size)} />
          <Property id="boards-inspector-prop-file" label="File name" value={pin.fileName ? <span title={pin.fileName}>{pin.fileName}</span> : ""} />
          <Property id="boards-inspector-prop-added" label="Added" value={formatDate(pin.createdAt)} />
          <Property id="boards-inspector-prop-modified" label="Modified" value={formatDate(pin.fileModifiedAt)} />
          <Property id="boards-inspector-prop-opened" label="Last opened" value={formatDate(pin.lastOpenedAt)} />
          <Property
            id="boards-inspector-prop-source"
            label="From"
            value={pin.pageUrl ? <span title={pin.pageUrl}>{hostOf(pin.pageUrl) || pin.pageUrl}</span> : ""}
          />
        </dl>
      </Section>

      {pin.text && hasPreview(pin) ? (
        <Section id="boards-inspector-text" label="Text">
          <p id="boards-inspector-text-value" className="max-h-40 overflow-y-auto whitespace-pre-line rounded-md bg-zinc-900 p-2 text-xs text-zinc-400">
            {pin.text}
          </p>
        </Section>
      ) : null}

      <div id="boards-inspector-actions" className="flex flex-col gap-2">
        {inTrash ? (
          <>
            <Button id="boards-inspector-restore" variant="primary" onClick={() => actions.restore([pin])}>
              <RotateCcw id="boards-inspector-restore-icon" className="size-4" aria-hidden />
              Restore
            </Button>
            <Button id="boards-inspector-delete" variant="destructiveOutline" onClick={() => actions.deleteForever([pin])}>
              <Trash2 id="boards-inspector-delete-icon" className="size-4" aria-hidden />
              Delete forever
            </Button>
          </>
        ) : (
          <>
            <Button id="boards-inspector-open" variant="primary" onClick={() => actions.open(pin)}>
              <Maximize2 id="boards-inspector-open-icon" className="size-4" aria-hidden />
              Open
            </Button>
            <div id="boards-inspector-actions-row" className="grid grid-cols-2 gap-2">
              {isFile ? (
                <>
                  <Button id="boards-inspector-open-file" size="sm" onClick={() => actions.openFile(pin)}>
                    <File id="boards-inspector-open-file-icon" className="size-3.5" aria-hidden />
                    Open file
                  </Button>
                  <Button id="boards-inspector-reveal" size="sm" onClick={() => actions.reveal(pin)}>
                    <FolderOpen id="boards-inspector-reveal-icon" className="size-3.5" aria-hidden />
                    Show file
                  </Button>
                </>
              ) : null}
              {hasPreview(pin) ? (
                <Button id="boards-inspector-copy" size="sm" onClick={() => actions.copyImage(pin)}>
                  <Copy id="boards-inspector-copy-icon" className="size-3.5" aria-hidden />
                  Copy
                </Button>
              ) : null}
              <Button id="boards-inspector-export" size="sm" onClick={() => actions.exportPins([pin])}>
                <Download id="boards-inspector-export-icon" className="size-3.5" aria-hidden />
                Export
              </Button>
            </div>
            <Button id="boards-inspector-trash" variant="ghost" size="sm" className="text-red-300 hover:bg-red-950/60" onClick={() => actions.trash([pin])}>
              <Trash2 id="boards-inspector-trash-icon" className="size-3.5" aria-hidden />
              Move to Trash
            </Button>
          </>
        )}
      </div>
    </div>
  );
}

type ManyDetailsProps = {
  boards: BoardsSnapshot;
  pins: Pin[];
  knownTags: string[];
  actions: PinMenuActions;
  onScope: (scope: BoardsScope) => void;
  onError: (reason: unknown) => void;
};

function ManyDetails({ boards, pins, knownTags, actions, onScope, onError }: ManyDetailsProps) {
  const ids = pins.map((pin) => pin.id);
  const common = (lists: string[][]) => (lists[0] ?? []).filter((value) => lists.every((list) => list.includes(value)));
  const sharedTags = common(pins.map((pin) => pin.tags));
  const sharedFolders = common(pins.map((pin) => pin.folderIds));
  const rating = pins.every((pin) => pin.rating === pins[0].rating) ? pins[0].rating : 0;
  const totalSize = pins.reduce((total, pin) => total + pin.size, 0);
  const previews = pins.filter(hasPreview).slice(0, 6);
  const inTrash = pins.every((pin) => pin.deletedAt !== null);

  return (
    <div id="boards-inspector-many" className="flex flex-col gap-4">
      {previews.length > 0 ? (
        <div id="boards-inspector-many-stack" className="grid grid-cols-3 gap-1">
          {previews.map((pin) => (
            <img id={`boards-inspector-many-${pin.id}`} key={pin.id} src={previewUrl(pin)} alt="" className="aspect-square w-full rounded object-cover" />
          ))}
        </div>
      ) : null}
      <p id="boards-inspector-many-count" className="text-sm font-medium text-zinc-100">
        {pins.length} items selected
        {totalSize > 0 ? <span id="boards-inspector-many-size" className="ml-1.5 text-xs font-normal text-zinc-500">{formatBytes(totalSize)}</span> : null}
      </p>
      {inTrash ? null : (
        <>
          <Section id="boards-inspector-many-tags" label="Tags on every item">
            <TagInput
              id="boards-inspector-many-tag-input"
              tags={sharedTags}
              known={knownTags}
              placeholder="Add tags to all"
              onAdd={(tags) => void addPinTags(ids, tags).catch(onError)}
              onRemove={(tag) => void removePinTags(ids, [tag]).catch(onError)}
              onOpenTag={(tag) => onScope({ kind: "tag", name: tag })}
            />
          </Section>
          <Section
            id="boards-inspector-many-folders"
            label="Folders of every item"
            action={
              <button
                id="boards-inspector-many-folders-add"
                type="button"
                aria-label="Add to folder"
                className="inline-flex size-6 items-center justify-center rounded text-zinc-500 hover:bg-zinc-900 hover:text-zinc-100"
                onClick={(event) => {
                  const rect = event.currentTarget.getBoundingClientRect();
                  actions.addToFolder(pins, rect.left - 220, rect.bottom + 4);
                }}
              >
                <FolderPlus id="boards-inspector-many-folders-add-icon" className="size-3.5" aria-hidden />
              </button>
            }
          >
            <FolderChips id="boards-inspector-many-folder-list" boards={boards} folderIds={sharedFolders} onScope={onScope} onRemove={(folderId) => void removePinsFromFolder(ids, folderId).catch(onError)} />
          </Section>
          <Section id="boards-inspector-many-rating" label="Rating">
            <Rating id="boards-inspector-many-rating-stars" value={rating} onChange={(stars) => actions.rate(pins, stars)} />
          </Section>
        </>
      )}
      <div id="boards-inspector-many-actions" className="flex flex-col gap-2">
        {inTrash ? (
          <>
            <Button id="boards-inspector-many-restore" variant="primary" onClick={() => actions.restore(pins)}>
              <RotateCcw id="boards-inspector-many-restore-icon" className="size-4" aria-hidden />
              Restore {pins.length} items
            </Button>
            <Button id="boards-inspector-many-delete" variant="destructiveOutline" onClick={() => actions.deleteForever(pins)}>
              <Trash2 id="boards-inspector-many-delete-icon" className="size-4" aria-hidden />
              Delete forever
            </Button>
          </>
        ) : (
          <>
            <Button id="boards-inspector-many-export" onClick={() => actions.exportPins(pins)}>
              <Download id="boards-inspector-many-export-icon" className="size-4" aria-hidden />
              Export {pins.length} items
            </Button>
            <Button id="boards-inspector-many-trash" variant="ghost" size="sm" className="text-red-300 hover:bg-red-950/60" onClick={() => actions.trash(pins)}>
              <Trash2 id="boards-inspector-many-trash-icon" className="size-3.5" aria-hidden />
              Move to Trash
            </Button>
          </>
        )}
      </div>
    </div>
  );
}

function FolderDetails({ boards, folder, onExport, onError }: { boards: BoardsSnapshot; folder: BoardFolder; onExport: () => void; onError: (reason: unknown) => void }) {
  const [name, setName] = useState(folder.name);
  const [description, setDescription] = useState(folder.description);
  useEffect(() => setName(folder.name), [folder.name]);
  useEffect(() => setDescription(folder.description), [folder.description]);
  const update = (patch: Parameters<typeof updateBoardFolder>[1]) => void updateBoardFolder(folder.id, patch).catch(onError);
  const own = pinsInFolder(boards, folder.id, false).length;
  const all = pinsInFolder(boards, folder.id, true);
  const subfolders = boards.folders.filter((item) => item.parentId === folder.id).length;
  const cover = folder.coverPinId ? boards.pins.find((pin) => pin.id === folder.coverPinId) : undefined;
  const size = all.reduce((total, pin) => total + pin.size, 0);

  return (
    <div id="boards-inspector-folder" className="flex flex-col gap-4">
      <div id="boards-inspector-folder-heading" className="flex items-center gap-2">
        <FolderGlyph id="boards-inspector-folder-icon" folder={folder} className="size-5 text-zinc-400" />
        <input
          id="boards-inspector-folder-name"
          value={name}
          aria-label="Folder name"
          className={cn(fieldClass, "h-8 font-medium")}
          onChange={(event) => setName(event.target.value)}
          onBlur={() => name.trim() && name !== folder.name && void renameBoardFolder(folder.id, name.trim()).catch(onError)}
          onKeyDown={(event) => event.key === "Enter" && event.currentTarget.blur()}
        />
      </div>
      <textarea
        id="boards-inspector-folder-description"
        value={description}
        rows={3}
        aria-label="Description"
        placeholder="Description"
        className={cn(fieldClass, "resize-y py-1.5 text-xs")}
        onChange={(event) => setDescription(event.target.value)}
        onBlur={() => description !== folder.description && update({ description })}
      />

      <Section id="boards-inspector-folder-color" label="Color">
        <div id="boards-inspector-folder-colors" className="flex flex-wrap gap-1.5">
          <button
            id="boards-inspector-folder-color-none"
            type="button"
            aria-label="No color"
            className={cn("size-5 rounded-full border border-zinc-600", !folder.color && "ring-2 ring-sky-400 ring-offset-1 ring-offset-zinc-950")}
            onClick={() => update({ color: "" })}
          />
          {FOLDER_COLORS.map((color) => (
            <button
              id={`boards-inspector-folder-color-${color}`}
              key={color}
              type="button"
              aria-label={color}
              className={cn("size-5 rounded-full", FOLDER_COLOR_CLASS[color].swatch, folder.color === color && "ring-2 ring-sky-400 ring-offset-1 ring-offset-zinc-950")}
              onClick={() => update({ color })}
            />
          ))}
        </div>
      </Section>

      <Section id="boards-inspector-folder-sort" label="Sort">
        <select
          id="boards-inspector-folder-sort-select"
          value={folder.sort}
          className={cn(fieldClass, "h-8 text-xs")}
          onChange={(event) => update({ sort: event.target.value as PinSort | "" })}
        >
          <option id="boards-inspector-folder-sort-default" value="">
            Use the page sort
          </option>
          {PIN_SORTS.map((sort) => (
            <option id={`boards-inspector-folder-sort-${sort}`} key={sort} value={sort}>
              {PIN_SORT_LABELS[sort]}
            </option>
          ))}
        </select>
      </Section>

      <label id="boards-inspector-folder-subfolders" className="flex items-center justify-between gap-2 text-xs text-zinc-300">
        Show subfolder contents
        <Toggle id="boards-inspector-folder-subfolders-toggle" checked={folder.showSubfolders} aria-label="Show subfolder contents" onChange={(checked) => update({ showSubfolders: checked })} />
      </label>

      {cover ? (
        <Section
          id="boards-inspector-folder-cover"
          label="Cover"
          action={
            <button id="boards-inspector-folder-cover-clear" type="button" className="text-[11px] text-zinc-500 hover:text-zinc-200" onClick={() => update({ coverPinId: "" })}>
              Clear
            </button>
          }
        >
          <img id="boards-inspector-folder-cover-image" src={previewUrl(cover)} alt="" className="max-h-32 w-full rounded-md object-cover" />
        </Section>
      ) : null}

      <Section id="boards-inspector-folder-properties" label="Properties">
        <dl id="boards-inspector-folder-properties-list" className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1 text-xs">
          <Property id="boards-inspector-folder-prop-items" label="Items" value={String(own)} />
          <Property id="boards-inspector-folder-prop-all" label="With subfolders" value={subfolders > 0 ? String(all.length) : ""} />
          <Property id="boards-inspector-folder-prop-subfolders" label="Subfolders" value={subfolders > 0 ? String(subfolders) : ""} />
          <Property id="boards-inspector-folder-prop-size" label="Size" value={formatBytes(size)} />
          <Property id="boards-inspector-folder-prop-created" label="Created" value={formatDate(folder.createdAt)} />
        </dl>
      </Section>

      <Button id="boards-inspector-folder-export" onClick={onExport}>
        <Download id="boards-inspector-folder-export-icon" className="size-4" aria-hidden />
        Export folder…
      </Button>
    </div>
  );
}

function ScopeSummary({ boards, scope, shown }: { boards: BoardsSnapshot; scope: BoardsScope; shown: Pin[] }) {
  const size = shown.reduce((total, pin) => total + pin.size, 0);
  const kinds = new Map<string, number>();
  for (const pin of shown) {
    const key = pin.ext ? pin.ext.toUpperCase() : PIN_KIND_LABELS[pin.kind];
    kinds.set(key, (kinds.get(key) ?? 0) + 1);
  }
  const top = [...kinds.entries()].sort((left, right) => right[1] - left[1]).slice(0, 8);
  return (
    <div id="boards-inspector-summary" className="flex flex-col gap-4">
      <div id="boards-inspector-summary-heading">
        <h2 id="boards-inspector-summary-title" className="text-sm font-semibold text-zinc-100">
          {scopeTitle(boards, scope)}
        </h2>
        <p id="boards-inspector-summary-count" className="text-xs text-zinc-500">
          {shown.length === 1 ? "1 item" : `${shown.length} items`}
          {size > 0 ? ` · ${formatBytes(size)}` : ""}
        </p>
      </div>
      {top.length > 0 ? (
        <Section id="boards-inspector-summary-types" label="Types">
          <ul id="boards-inspector-summary-types-list" className="flex flex-col gap-1 text-xs">
            {top.map(([kind, count]) => (
              <li id={`boards-inspector-summary-type-${kind}`} key={kind} className="flex justify-between text-zinc-300">
                <span id={`boards-inspector-summary-type-${kind}-name`}>{kind}</span>
                <span id={`boards-inspector-summary-type-${kind}-count`} className="tabular-nums text-zinc-500">
                  {count}
                </span>
              </li>
            ))}
          </ul>
        </Section>
      ) : null}
      <p id="boards-inspector-summary-hint" className="text-xs leading-relaxed text-zinc-500">
        Select an item to see its details. Ctrl-click or drag a box to select several. Press Space to view.
      </p>
      {shown[0] ? (
        <p id="boards-inspector-summary-newest" className="truncate text-xs text-zinc-500" title={pinName(shown[0])}>
          First: {pinName(shown[0])}
        </p>
      ) : null}
    </div>
  );
}
