import { useState } from "react";
import { createPortal } from "react-dom";
import { Check, ExternalLink, Pause, Play, StickyNote, Trash2 } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { cn } from "@/lib/cn";
import type { ProductPrice } from "@type/browser";
import { checkedLabel, formatPrice, priceChange } from "./format";
import { PriceSparkline } from "./PriceSparkline";

const PREVIEW_SIZE = 240;
const PREVIEW_GAP = 8;

function previewPosition(anchor: HTMLElement) {
  const rect = anchor.getBoundingClientRect();
  const left = rect.right + PREVIEW_GAP + PREVIEW_SIZE > window.innerWidth - PREVIEW_GAP
    ? Math.max(PREVIEW_GAP, rect.left - PREVIEW_GAP - PREVIEW_SIZE)
    : rect.right + PREVIEW_GAP;
  const top = Math.min(Math.max(PREVIEW_GAP, rect.top), Math.max(PREVIEW_GAP, window.innerHeight - PREVIEW_SIZE - PREVIEW_GAP));
  return { top, left };
}

function Thumbnail({ id, src }: { id: string; src: string }) {
  const [preview, setPreview] = useState<{ top: number; left: number } | null>(null);

  const show = (anchor: HTMLElement) => setPreview(previewPosition(anchor));

  return (
    <>
      <img
        id={id}
        src={src}
        alt=""
        className="h-10 w-10 rounded object-contain"
        onMouseEnter={(event) => show(event.currentTarget)}
        onMouseLeave={() => setPreview(null)}
      />
      {preview
        ? createPortal(
            <img
              id={`${id}-preview`}
              src={src}
              alt=""
              className="pointer-events-none fixed z-50 rounded-lg bg-zinc-900 object-contain p-2 shadow-xl ring-1 ring-zinc-700"
              style={{ top: preview.top, left: preview.left, width: PREVIEW_SIZE, height: PREVIEW_SIZE }}
            />,
            document.body,
          )
        : null}
    </>
  );
}

export function TrackedTable({
  items,
  picked,
  onToggle,
  onOpen,
  onCheck,
  onPause,
  onNote,
  onRemove,
}: {
  items: ProductPrice[];
  picked: Set<string>;
  onToggle: (id: string) => void;
  onOpen: (item: ProductPrice) => void;
  onCheck: (item: ProductPrice) => void;
  onPause: (item: ProductPrice) => void;
  onNote: (item: ProductPrice) => void;
  onRemove: (item: ProductPrice) => void;
}) {
  return (
    <table id="shopping-table" className="w-full border-collapse text-left text-sm">
      <thead id="shopping-table-head">
        <tr id="shopping-table-head-row" className="text-xs text-zinc-500">
          <th id="shopping-table-check" className="w-8 px-2 py-2" />
          <th id="shopping-table-thumbnail" className="w-14 px-2 py-2 font-medium">Thumbnail</th>
          <th id="shopping-table-product" className="px-2 py-2 font-medium">Product</th>
          <th id="shopping-table-price" className="px-2 py-2 font-medium">Price</th>
          <th id="shopping-table-change" className="px-2 py-2 font-medium">Change</th>
          <th id="shopping-table-history" className="px-2 py-2 font-medium">History</th>
          <th id="shopping-table-status" className="px-2 py-2 font-medium">Checked</th>
          <th id="shopping-table-actions" className="px-2 py-2 font-medium">Actions</th>
        </tr>
      </thead>
      <tbody id="shopping-table-body">
        {items.map((item) => {
          const change = priceChange(item);
          return (
            <tr id={`shopping-row-${item.id}`} key={item.id} className="border-t border-zinc-800">
              <td id={`shopping-row-check-${item.id}`} className="px-2 py-2">
                <input
                  id={`shopping-row-checkbox-${item.id}`}
                  type="checkbox"
                  checked={picked.has(item.id)}
                  onChange={() => onToggle(item.id)}
                />
              </td>
              <td id={`shopping-row-thumbnail-${item.id}`} className="px-2 py-2">
                {item.imageUrl ? (
                  <Thumbnail id={`shopping-row-image-${item.id}`} src={item.imageUrl} />
                ) : (
                  <div id={`shopping-row-image-${item.id}`} className="h-10 w-10 rounded bg-zinc-800" />
                )}
              </td>
              <td id={`shopping-row-title-${item.id}`} className="max-w-64 px-2 py-2">
                <p id={`shopping-row-name-${item.id}`} className="truncate">{item.title || item.url}</p>
                <p id={`shopping-row-store-${item.id}`} className="truncate text-xs text-zinc-500">{item.store}</p>
              </td>
              <td id={`shopping-row-price-${item.id}`} className="px-2 py-2">{formatPrice(item.price, item.currency)}</td>
              <td id={`shopping-row-change-${item.id}`} className={cn("px-2 py-2", change.amount < 0 ? "text-emerald-300" : "text-zinc-400")}>
                {change.percent.toFixed(0)}%
              </td>
              <td id={`shopping-row-spark-${item.id}`} className="px-2 py-2">
                <PriceSparkline id={`shopping-row-line-${item.id}`} history={item.history} width={96} height={28} />
              </td>
              <td id={`shopping-row-status-${item.id}`} className={cn("max-w-48 truncate px-2 py-2 text-xs", item.lastError ? "text-amber-300" : "text-zinc-500")}>
                {checkedLabel(item)}
              </td>
              <td id={`shopping-row-actions-${item.id}`} className="px-2 py-2">
                <div
                  id={`shopping-row-action-buttons-${item.id}`}
                  className="flex items-center gap-0.5"
                  onClick={(event) => event.stopPropagation()}
                >
                  <IconButton id={`shopping-row-open-${item.id}`} label="Open product" onClick={() => onOpen(item)}>
                    <ExternalLink id={`shopping-row-open-icon-${item.id}`} aria-hidden />
                  </IconButton>
                  <IconButton id={`shopping-row-check-now-${item.id}`} label="Check now" onClick={() => onCheck(item)}>
                    <Check id={`shopping-row-check-icon-${item.id}`} aria-hidden />
                  </IconButton>
                  <IconButton id={`shopping-row-pause-${item.id}`} label={item.paused ? "Resume" : "Pause"} onClick={() => onPause(item)}>
                    {item.paused ? (
                      <Play id={`shopping-row-pause-icon-${item.id}`} aria-hidden />
                    ) : (
                      <Pause id={`shopping-row-pause-icon-${item.id}`} aria-hidden />
                    )}
                  </IconButton>
                  <IconButton id={`shopping-row-note-${item.id}`} label="Note" onClick={() => onNote(item)}>
                    <StickyNote id={`shopping-row-note-icon-${item.id}`} aria-hidden />
                  </IconButton>
                  <IconButton id={`shopping-row-remove-${item.id}`} label="Remove" onClick={() => onRemove(item)}>
                    <Trash2 id={`shopping-row-remove-icon-${item.id}`} aria-hidden />
                  </IconButton>
                </div>
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}
