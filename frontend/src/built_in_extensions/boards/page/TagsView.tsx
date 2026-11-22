import { useMemo, useState } from "react";
import { Hash, Pencil, Tags, Trash2 } from "lucide-react";
import { Button } from "@/components/Button";
import { ContextMenu, ContextMenuItem } from "@/components/ContextMenu";
import { Dialog } from "@/components/Dialog";
import { EmptyState } from "@/components/EmptyState";
import { deleteBoardTag, renameBoardTag } from "@/lib/browser-client";
import type { BoardsSnapshot } from "@type/browser";
import { hasPreview, livePins, previewUrl, tagCounts } from "../folder-tree";
import type { BoardsScope } from "../scopes";

type TagsViewProps = {
  boards: BoardsSnapshot;
  query: string;
  onScope: (scope: BoardsScope) => void;
  onError: (reason: unknown) => void;
};

/** Every tag with its count and a few of its images. Renaming onto an existing tag merges them. */
export function TagsView({ boards, query, onScope, onError }: TagsViewProps) {
  const [menu, setMenu] = useState<{ tag: string; x: number; y: number } | null>(null);
  const [renaming, setRenaming] = useState<{ from: string; to: string } | null>(null);
  const [deleting, setDeleting] = useState<string | null>(null);
  const tags = useMemo(() => tagCounts(boards), [boards]);
  const previews = useMemo(() => {
    const found = new Map<string, string[]>();
    for (const pin of livePins(boards)) {
      if (!hasPreview(pin)) {
        continue;
      }
      for (const tag of pin.tags) {
        const list = found.get(tag) ?? [];
        if (list.length < 3) {
          list.push(previewUrl(pin));
          found.set(tag, list);
        }
      }
    }
    return found;
  }, [boards]);
  const needle = query.trim().toLowerCase();
  const shown = needle ? tags.filter((tag) => tag.name.toLowerCase().includes(needle)) : tags;
  const merging = renaming ? tags.some((tag) => tag.name.toLowerCase() === renaming.to.trim().toLowerCase() && tag.name !== renaming.from) : false;

  const submitRename = () => {
    if (!renaming || !renaming.to.trim() || renaming.to.trim() === renaming.from) {
      setRenaming(null);
      return;
    }
    void renameBoardTag(renaming.from, renaming.to.trim()).catch(onError);
    setRenaming(null);
  };

  if (tags.length === 0) {
    return <EmptyState id="boards-tags-empty" icon={Tags} title="No tags yet" hint="Select items and add tags in the inspector on the right." />;
  }

  return (
    <div id="boards-tags" className="min-h-0 flex-1 overflow-y-auto px-6 py-4">
      <ul id="boards-tags-list" className="grid gap-3" style={{ gridTemplateColumns: "repeat(auto-fill, minmax(180px, 1fr))" }}>
        {shown.map((tag, index) => {
          const images = previews.get(tag.name) ?? [];
          const idBase = `boards-tag-${index}`;
          return (
            <li id={idBase} key={tag.name}>
              <button
                id={`${idBase}-button`}
                type="button"
                className="group flex w-full flex-col gap-1.5 text-left"
                onClick={() => onScope({ kind: "tag", name: tag.name })}
                onContextMenu={(event) => {
                  event.preventDefault();
                  setMenu({ tag: tag.name, x: event.clientX, y: event.clientY });
                }}
              >
                <span id={`${idBase}-mosaic`} className="grid aspect-[16/9] w-full grid-cols-3 gap-0.5 overflow-hidden rounded-lg bg-zinc-900 ring-1 ring-zinc-800 group-hover:ring-zinc-600">
                  {images.length === 0 ? (
                    <span id={`${idBase}-empty`} className="col-span-3 flex items-center justify-center text-zinc-700">
                      <Hash id={`${idBase}-empty-icon`} className="size-8" aria-hidden />
                    </span>
                  ) : (
                    images.map((src, image) => (
                      <img
                        id={`${idBase}-image-${image}`}
                        key={src}
                        src={src}
                        alt=""
                        loading="lazy"
                        className={image === 0 && images.length === 1 ? "col-span-3 size-full object-cover" : image === 0 && images.length === 2 ? "col-span-2 size-full object-cover" : "size-full object-cover"}
                      />
                    ))
                  )}
                </span>
                <span id={`${idBase}-label`} className="flex items-center gap-1 px-0.5 text-sm">
                  <Hash id={`${idBase}-icon`} className="size-3.5 text-zinc-500" aria-hidden />
                  <span id={`${idBase}-name`} className="min-w-0 flex-1 truncate text-zinc-200">
                    {tag.name}
                  </span>
                  <span id={`${idBase}-count`} className="text-xs tabular-nums text-zinc-500">
                    {tag.count}
                  </span>
                </span>
              </button>
            </li>
          );
        })}
      </ul>
      {shown.length === 0 ? (
        <p id="boards-tags-no-match" className="py-8 text-center text-sm text-zinc-500">
          No tag matches “{query}”.
        </p>
      ) : null}

      {menu ? (
        <ContextMenu id="boards-tag-menu" label={`Tag ${menu.tag}`} x={menu.x} y={menu.y} onClose={() => setMenu(null)}>
          <ContextMenuItem
            id="boards-tag-menu-rename"
            icon={<Pencil />}
            label="Rename or merge…"
            onClick={() => {
              setRenaming({ from: menu.tag, to: menu.tag });
              setMenu(null);
            }}
          />
          <ContextMenuItem
            id="boards-tag-menu-delete"
            icon={<Trash2 />}
            label="Delete tag…"
            danger
            onClick={() => {
              setDeleting(menu.tag);
              setMenu(null);
            }}
          />
        </ContextMenu>
      ) : null}

      <Dialog id="boards-tag-rename" title="Rename tag" open={renaming !== null} onClose={() => setRenaming(null)} onEnter={submitRename} panelClassName="max-w-sm">
        <input
          id="boards-tag-rename-input"
          autoFocus
          value={renaming?.to ?? ""}
          className="h-8 w-full rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500"
          onChange={(event) => setRenaming((current) => (current ? { ...current, to: event.target.value } : current))}
        />
        {merging ? (
          <p id="boards-tag-rename-merge" className="mt-2 text-xs text-amber-300">
            That tag exists. Its items and these will share one tag.
          </p>
        ) : null}
        <div id="boards-tag-rename-actions" className="mt-4 flex justify-end gap-2">
          <Button id="boards-tag-rename-cancel" size="sm" variant="ghost" onClick={() => setRenaming(null)}>
            Cancel
          </Button>
          <Button id="boards-tag-rename-save" size="sm" variant="primary" onClick={submitRename}>
            {merging ? "Merge" : "Rename"}
          </Button>
        </div>
      </Dialog>

      <Dialog id="boards-tag-delete" title={`Delete “${deleting ?? ""}”?`} open={deleting !== null} onClose={() => setDeleting(null)} panelClassName="max-w-sm">
        <p id="boards-tag-delete-detail" className="text-sm text-zinc-300">
          The tag is taken off every item. The items stay.
        </p>
        <div id="boards-tag-delete-actions" className="mt-4 flex justify-end gap-2">
          <Button id="boards-tag-delete-cancel" size="sm" variant="ghost" onClick={() => setDeleting(null)}>
            Cancel
          </Button>
          <Button
            id="boards-tag-delete-confirm"
            size="sm"
            variant="destructive"
            onClick={() => {
              if (deleting) {
                void deleteBoardTag(deleting).catch(onError);
              }
              setDeleting(null);
            }}
          >
            Delete tag
          </Button>
        </div>
      </Dialog>
    </div>
  );
}
