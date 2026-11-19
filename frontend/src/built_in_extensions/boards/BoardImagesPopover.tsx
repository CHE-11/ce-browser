import { useEffect, useMemo, useState } from "react";
import { Check, Images, LoaderCircle } from "lucide-react";
import { Button } from "@/components/Button";
import { PopupFrame } from "@/components/PopupFrame";
import { closePopup, pageMedia, pinImages, showNotice } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import type { PageMedia } from "@type/browser";
import { PopoverHeader } from "../PopoverParts";
import { readPopoverTab } from "../shared";
import { errorText } from "../use-page-tool";
import { BoardPicker } from "./BoardPicker";
import { useBoards } from "./use-boards";
import { boardLabel } from "./board-tree";

/** Images smaller than this on both sides are usually icons and spacers. */
const MIN_SIDE = 48;

export function BoardImagesPopover({ params }: { params: URLSearchParams }) {
  const tab = readPopoverTab(params);
  const { boards, loaded } = useBoards();
  const [boardId, setBoardId] = useState(params.get("board") ?? "");
  const [items, setItems] = useState<PageMedia[] | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let gone = false;
    void pageMedia(tab.id)
      .then((next) => {
        if (gone) {
          return;
        }
        const seen = new Set<string>();
        setItems(
          next.filter((item) => {
            const keep =
              item.kind === "image" && !seen.has(item.url) && (item.width === 0 || item.height === 0 || item.width >= MIN_SIDE || item.height >= MIN_SIDE);
            seen.add(item.url);
            return keep;
          }),
        );
      })
      .catch((reason: unknown) => {
        if (!gone) {
          setItems([]);
          setError(errorText(reason));
        }
      });
    return () => {
      gone = true;
    };
  }, [tab.id]);

  const all = useMemo(() => items ?? [], [items]);
  const allSelected = all.length > 0 && selected.size === all.length;

  const toggle = (url: string) => {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(url)) {
        next.delete(url);
      } else {
        next.add(url);
      }
      return next;
    });
  };

  const save = async () => {
    setBusy(true);
    setError("");
    try {
      const pins = await pinImages(tab.id, boardId, [...selected]);
      const board = boards.boards.find((item) => item.id === pins[0]?.boardId);
      const where = board ? boardLabel(boards, board) : "the board";
      await showNotice(pins.length === 1 ? `Saved 1 image to ${where}` : `Saved ${pins.length} images to ${where}`).catch(() => undefined);
      void closePopup();
    } catch (reason) {
      setError(errorText(reason));
    } finally {
      setBusy(false);
    }
  };

  return (
    <PopupFrame id="board-images" width={440} maxHeight={600} ready={loaded && items !== null}>
      <PopoverHeader id="board-images-header" icon={<Images aria-hidden />} title="Save images to board" subtitle={tab.title || tab.url} />
      <div id="board-images-body" className="flex flex-col gap-2 px-3 pb-3">
        <BoardPicker id="board-images-picker" boards={boards} value={boardId} onChange={setBoardId} />
        {error ? (
          <p id="board-images-error" className="text-xs text-amber-300">
            {error}
          </p>
        ) : null}
        {all.length === 0 ? (
          <p id="board-images-empty" className="py-4 text-center text-sm text-zinc-500">
            No images on this page.
          </p>
        ) : (
          <>
            <div id="board-images-toolbar" className="flex items-center justify-between text-xs text-zinc-400">
              <span id="board-images-count">
                {selected.size} of {all.length} selected
              </span>
              <Button
                id="board-images-select-all"
                variant="ghost"
                size="xs"
                onClick={() => setSelected(allSelected ? new Set() : new Set(all.map((item) => item.url)))}
              >
                {allSelected ? "Select none" : "Select all"}
              </Button>
            </div>
            <div id="board-images-grid" className="grid grid-cols-3 gap-2">
              {all.map((item, index) => {
                const checked = selected.has(item.url);
                return (
                  <button
                    id={`board-images-item-${index}`}
                    key={item.url}
                    type="button"
                    title={item.url}
                    aria-pressed={checked}
                    disabled={busy}
                    className={cn(
                      "relative overflow-hidden rounded-md bg-zinc-800 ring-2 ring-transparent transition",
                      checked && "ring-sky-500",
                    )}
                    onClick={() => toggle(item.url)}
                  >
                    <img id={`board-images-thumb-${index}`} src={item.url} alt="" loading="lazy" className="aspect-square w-full object-cover" />
                    {checked ? (
                      <span
                        id={`board-images-check-${index}`}
                        className="absolute right-1 top-1 inline-flex size-5 items-center justify-center rounded-full bg-sky-500 text-white"
                      >
                        <Check id={`board-images-check-icon-${index}`} className="size-3.5" aria-hidden />
                      </span>
                    ) : null}
                  </button>
                );
              })}
            </div>
          </>
        )}
        <Button id="board-images-save" variant="primary" disabled={busy || selected.size === 0} onClick={() => void save()}>
          {busy ? <LoaderCircle id="board-images-save-icon" className="size-4 animate-spin" aria-hidden /> : null}
          {busy ? "Saving…" : selected.size > 1 ? `Save ${selected.size} images` : "Save image"}
        </Button>
      </div>
    </PopupFrame>
  );
}
