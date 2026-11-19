import { cn } from "@/lib/cn";
import type { ProductPrice } from "@type/browser";
import { checkedLabel, formatPrice, priceChange } from "./format";
import { PriceSparkline } from "./PriceSparkline";

export function TrackedGrid({
  items,
  selected,
  onSelect,
}: {
  items: ProductPrice[];
  selected: string | null;
  onSelect: (id: string) => void;
}) {
  return (
    <ul id="shopping-grid" className="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-3">
      {items.map((item) => {
        const change = priceChange(item);
        return (
          <li id={`shopping-card-${item.id}`} key={item.id}>
            <button
              id={`shopping-card-open-${item.id}`}
              type="button"
              className={cn("flex h-full w-full flex-col gap-2 rounded-lg border bg-zinc-900/70 p-3 text-left", selected === item.id ? "border-sky-500" : "border-zinc-800")}
              onClick={() => onSelect(item.id)}
            >
              {item.imageUrl ? (
                <img id={`shopping-card-image-${item.id}`} src={item.imageUrl} alt="" className="h-24 w-full rounded object-contain" />
              ) : (
                <div id={`shopping-card-image-${item.id}`} className="h-24 rounded bg-zinc-800" />
              )}
              <p id={`shopping-card-title-${item.id}`} className="line-clamp-2 text-sm">
                {item.title || item.url}
              </p>
              <p id={`shopping-card-store-${item.id}`} className="text-xs text-zinc-500">
                {item.store}
              </p>
              <div id={`shopping-card-prices-${item.id}`} className="flex items-end justify-between gap-2">
                <div id={`shopping-card-amounts-${item.id}`}>
                  <p id={`shopping-card-current-${item.id}`} className="text-sm font-medium">
                    {formatPrice(item.price, item.currency)}
                  </p>
                  <p id={`shopping-card-change-${item.id}`} className={cn("text-xs", change.amount < 0 ? "text-emerald-300" : "text-zinc-500")}>
                    {formatPrice(item.history[0]?.price, item.currency)} · {change.percent.toFixed(0)}%
                  </p>
                </div>
                <PriceSparkline id={`shopping-card-spark-${item.id}`} history={item.history} />
              </div>
              <p id={`shopping-card-status-${item.id}`} className={cn("truncate text-xs", item.lastError ? "text-amber-300" : "text-zinc-500")}>
                {checkedLabel(item)}
                {item.paused ? " · paused" : ""}
              </p>
            </button>
          </li>
        );
      })}
    </ul>
  );
}
