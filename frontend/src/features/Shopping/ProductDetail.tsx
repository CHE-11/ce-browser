import { Button } from "@/components/Button";
import { createTab } from "@/lib/browser-client";
import type { ProductPrice, ShoppingDeals } from "@type/browser";
import { DealList } from "./DealList";
import { formatPrice, priceChange } from "./format";
import { PriceSparkline } from "./PriceSparkline";

export function ProductDetail({
  item,
  deals,
  busy,
  onSearch,
  onFeedback,
  onCheck,
  onTarget,
  onPause,
  onNote,
  onRemove,
  onTrackOffer,
}: {
  item: ProductPrice;
  deals: ShoppingDeals | null;
  busy: boolean;
  onSearch: (force: boolean) => void;
  onFeedback: (deals: ShoppingDeals) => void;
  onCheck: () => void;
  onTarget: () => void;
  onPause: () => void;
  onNote: () => void;
  onRemove: () => void;
  onTrackOffer: (url: string) => void;
}) {
  const change = priceChange(item);
  return (
    <aside id="shopping-detail" className="flex w-full flex-col gap-4 rounded-lg border border-zinc-800 bg-zinc-900/50 p-4 lg:w-96">
      <div id="shopping-detail-heading">
        <h2 id="shopping-detail-title" className="text-base font-medium">
          {item.title || item.store}
        </h2>
        <p id="shopping-detail-price" className="mt-1 text-sm text-zinc-300">
          {formatPrice(item.price, item.currency)}
          {item.listPrice ? ` · list ${formatPrice(item.listPrice, item.currency)}` : ""}
          {change.amount < 0 ? ` · ${change.percent.toFixed(0)}% since first tracked` : ""}
        </p>
      </div>
      <PriceSparkline id="shopping-detail-chart" history={item.history} width={320} height={96} />
      <div id="shopping-detail-minmax" className="flex justify-between text-xs text-zinc-500">
        <span id="shopping-detail-min">{item.history.length ? formatPrice(Math.min(...item.history.map((point) => point.price)), item.currency) : ""}</span>
        <span id="shopping-detail-max">{item.history.length ? formatPrice(Math.max(...item.history.map((point) => point.price)), item.currency) : ""}</span>
      </div>
      <div id="shopping-detail-actions" className="flex flex-wrap gap-2">
        <Button id="shopping-detail-open" size="sm" onClick={() => void createTab(item.url)}>
          Open product
        </Button>
        <Button id="shopping-detail-check" size="sm" onClick={onCheck}>
          Check now
        </Button>
        <Button id="shopping-detail-target" size="sm" variant="secondary" onClick={onTarget}>
          {item.targetPrice ? `Target ${formatPrice(item.targetPrice, item.currency)}` : "Set target"}
        </Button>
        <Button id="shopping-detail-pause" size="sm" variant="secondary" onClick={onPause}>
          {item.paused ? "Resume" : "Pause"}
        </Button>
        <Button id="shopping-detail-note" size="sm" variant="ghost" onClick={onNote}>
          Note
        </Button>
        <Button id="shopping-detail-remove" size="sm" variant="destructive" onClick={onRemove}>
          Remove
        </Button>
      </div>
      {item.note ? (
        <p id="shopping-detail-note" className="text-sm text-zinc-400">
          {item.note}
        </p>
      ) : null}
      <DealList id="shopping-detail-deals" deals={deals} busy={busy} onSearch={onSearch} onFeedback={onFeedback} onTrack={onTrackOffer} />
    </aside>
  );
}
