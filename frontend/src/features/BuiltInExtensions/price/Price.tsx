import { useEffect, useState } from "react";
import { Tags } from "lucide-react";
import { Button } from "@/components/Button";
import { PopupFrame } from "@/components/PopupFrame";
import { checkPrices, listPrices, trackPrice } from "@/lib/browser-client";
import type { ProductPrice, TabInfo } from "@type/browser";
import { PopoverHeader } from "../PopoverParts";
import { readPopoverTab } from "../shared";
import { ToolButton } from "../ToolButton";

export function PriceButton({ activeTab }: { activeTab: TabInfo | undefined }) {
  return (
    <ToolButton
      id="price"
      label="Prices"
      kind="price"
      icon={<Tags id="address-price-icon" aria-hidden />}
      activeTab={activeTab}
    />
  );
}

export function PricePopover({ params }: { params: URLSearchParams }) {
  const tab = readPopoverTab(params);
  const [items, setItems] = useState<ProductPrice[]>([]);
  const [error, setError] = useState("");

  const load = () => {
    void listPrices().then(setItems).catch(() => undefined);
  };

  useEffect(load, []);

  return (
    <PopupFrame id="price-popover" width={320} maxHeight={460}>
      <PopoverHeader id="price-header" icon={<Tags aria-hidden />} title="Prices" subtitle={tab.title || tab.url} />
      <div id="price-body" className="flex flex-col gap-2 px-3 pb-3">
        <div id="price-actions" className="flex gap-2">
          <Button
            id="price-track"
            variant="primary"
            className="flex-1"
            onClick={() =>
              void trackPrice(tab.id)
                .then(() => load())
                .catch((reason: unknown) => setError(reason instanceof Error ? reason.message : "No price on this page."))
            }
          >
            Track
          </Button>
          <Button
            id="price-check"
            className="flex-1"
            onClick={() =>
              void checkPrices()
                .then(setItems)
                .catch((reason: unknown) => setError(reason instanceof Error ? reason.message : "Couldn't check prices."))
            }
          >
            Check
          </Button>
        </div>
        {error ? (
          <p id="price-error" className="text-xs text-amber-300">
            {error}
          </p>
        ) : null}
        <ul id="price-list" className="flex flex-col gap-1">
          {items.length === 0 ? (
            <li id="price-empty" className="text-sm text-zinc-500">
              Nothing tracked yet.
            </li>
          ) : (
            items.map((item, index) => (
              <li id={`price-item-${index}`} key={item.url} className="rounded-md bg-zinc-900 px-2 py-1.5 text-sm">
                <p id={`price-title-${index}`} className="truncate">
                  {item.title || item.url}
                </p>
                <p id={`price-amount-${index}`} className="text-xs text-zinc-400">
                  {item.currency} {item.price}
                  {item.availability ? ` · ${item.availability}` : ""}
                  {item.dropped ? " · lower" : ""}
                  {item.stockChanged ? " · stock changed" : ""}
                </p>
              </li>
            ))
          )}
        </ul>
      </div>
    </PopupFrame>
  );
}
