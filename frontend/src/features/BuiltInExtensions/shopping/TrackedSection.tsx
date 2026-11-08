import { Button } from "@/components/Button";
import type { ProductPrice } from "@type/browser";
import { PriceSparkline } from "@/features/BuiltInExtensions/shopping/PriceSparkline";
import { belowTarget, checkedLabel, formatPrice, priceChange } from "@/features/BuiltInExtensions/shopping/format";
import { Check } from "lucide-react";

export function TrackedSection({
  id,
  items,
  error,
  onCheck,
}: {
  id: string;
  items: ProductPrice[];
  error: string;
  onCheck: (id: string) => void;
}) {
  const ranked = [...items].sort((left, right) => priceChange(left).amount - priceChange(right).amount);
  const hits = items.filter(belowTarget).length;
  const errors = items.filter((item) => item.lastError).length;

  return (
    <section id={id} className="flex flex-col gap-2">
      {/* Divider between the header and the content */}
      <div id='divider' className="h-px w-full bg-zinc-800"></div>
      <h1 id={`${id}-title`} className="text-sm font-medium text-zinc-100 mt-2">Tracked Items</h1>
      {hits > 0 || errors > 0 ? (
        <div id={`${id}-summary`} className="flex items-center justify-between gap-2 text-xs text-zinc-400">
          <p id={`${id}-counts`}>
            {hits ? ` · ${hits} at target` : ""}
            {errors ? ` · ${errors} with errors` : ""}
          </p>
        </div>
      ) : null}

      {error ? (
        <p id={`${id}-error`} className="text-xs text-amber-300">
          {error}
        </p>
      ) : null}
      {ranked.length === 0 ? (
        <p id={`${id}-empty`} className="text-sm text-zinc-500">
          Nothing tracked on this site.
        </p>
      ) : (
        <ul id={`${id}-list`} className="flex flex-col gap-1">
          {ranked.map((item) => {
            const change = priceChange(item);
            return (
              <li id={`${id}-item-${item.id}`} key={item.id} className="rounded-md bg-zinc-900 px-2 py-1.5">
                <div id={`${id}-item-row-${item.id}`} className="flex items-center gap-2">
                  <div id={`${id}-item-text-${item.id}`} className="min-w-0 flex-1">
                    <p id={`${id}-item-title-${item.id}`} className="truncate text-sm">
                      {item.title || item.store}
                      {belowTarget(item) ? " · target" : ""}
                      {item.lastError ? " · error" : ""}
                    </p>
                    <p id={`${id}-item-price-${item.id}`} className="text-xs text-zinc-400">
                      {formatPrice(item.price, item.currency)}
                      {change.amount < 0 ? ` · down ${formatPrice(Math.abs(change.amount), item.currency)}` : ""}
                      {` · ${checkedLabel(item)}`}
                    </p>
                  </div>
                  {item.history.length > 1 ? <PriceSparkline id={`${id}-spark-${item.id}`} history={item.history} width={72} height={28} /> : null}
                  <Button id={`${id}-check-${item.id}`} size="xs" variant="ghost" onClick={() => onCheck(item.id)}>
                    <Check id={`${id}-check-${item.id}-icon`} aria-hidden className="text-zinc-400" />
                  </Button>
                </div>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
