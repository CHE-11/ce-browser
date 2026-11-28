import { useEffect, useState } from "react";
import { Check, Crosshair, ExternalLink } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import {
  addPriceUrl,
  checkPrice,
  createTab,
  findDeals,
  listPrices,
  onPricesChanged,
  trackPrice,
  untrackPrice,
} from "@/lib/browser-client";
import type { ProductPrice, ShoppingDeals } from "@type/browser";
import { useTabs } from "@/lib/use-browser";
import { DealList } from "@/built_in_extensions/shopping/DealList";
import { checkedLabel, formatPrice, priceChange, siteDomain } from "@/built_in_extensions/shopping/format";
import { PriceSparkline } from "@/built_in_extensions/shopping/PriceSparkline";
import { errorText } from "../use-page-tool";
import { TrackedSection } from "./TrackedSection";
import { useCoupons } from "./use-coupons";

function samePage(item: ProductPrice, url: string) {
  if (!url || !item.url) {
    return false;
  }
  try {
    const left = new URL(item.url);
    const right = new URL(url);
    left.hash = "";
    right.hash = "";
    return siteDomain(item.url) === siteDomain(url) && left.pathname.replace(/\/$/, "") === right.pathname.replace(/\/$/, "");
  } catch {
    return item.url === url;
  }
}

/** Price tracking and deals for the active tab, drawn in the docked shopping column. */
export function ShoppingPanel() {
  const { tabs, activeId } = useTabs();
  const tab = tabs.find((item) => item.id === activeId);
  const url = tab?.url ?? "";
  const productPage = /^https?:/i.test(url);
  const [items, setItems] = useState<ProductPrice[]>([]);
  const [listError, setListError] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [deals, setDeals] = useState<ShoppingDeals | null>(null);
  const coupons = useCoupons(tab?.id, url);

  useEffect(() => {
    void listPrices()
      .then(setItems)
      .catch((reason: unknown) => setListError(errorText(reason)));
    const listen = onPricesChanged(setItems);
    return () => {
      void listen.then((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    setDeals(null);
    setError("");
  }, [url]);

  const current = items.find((item) => samePage(item, url));
  const domain = siteDomain(url);
  const onSite = domain ? items.filter((item) => siteDomain(item.url) === domain) : [];
  const change = current ? priceChange(current) : null;

  const track = () => {
    setError("");
    if (!tab) {
      return;
    }
    if (current) {
      void untrackPrice(current.id)
        .then(setItems)
        .catch((reason: unknown) => setError(errorText(reason)));
      return;
    }
    void trackPrice(tab.id)
      .then(() => listPrices().then(setItems))
      .catch((reason: unknown) => setError(errorText(reason)));
  };

  const search = (force: boolean) => {
    if (!tab) {
      return;
    }
    setBusy(true);
    setError("");
    void findDeals(tab.id, force)
      .then(setDeals)
      .catch((reason: unknown) => setError(errorText(reason)))
      .finally(() => setBusy(false));
  };

  return (
    <main id="shopping-panel" className="flex h-full min-h-0 flex-col bg-zinc-950 text-zinc-100">
      <header id="shopping-panel-bar" className="flex shrink-0 items-center gap-2 border-b border-zinc-800 px-3 py-2">
        <p id="shopping-panel-page" className="min-w-0 flex-1 truncate text-xs text-zinc-400">
          {tab ? tab.title || tab.url : "No tab open"}
        </p>
        <div className="flex items-center gap-2">
          {current ? (
            <IconButton
              id="shopping-check"
              label="Check now"
              className="text-zinc-400"
              onClick={() => void checkPrice(current.id).then(setItems).catch((reason: unknown) => setError(errorText(reason)))}
            >
              <Check id="shopping-check-icon" aria-hidden />
            </IconButton>
          ) : null}
          <IconButton
            id="shopping-track"
            label={current ? "Untrack" : "Track"}
            className={current ? "text-sky-400 hover:text-sky-300" : "text-zinc-500"}
            onClick={track}
          >
            <Crosshair id="shopping-track-icon" aria-hidden />
          </IconButton>
          <IconButton
            id="shopping-open-page"
            label="Open Shopping page"
            onClick={() => {
              void createTab("ce://shopping");
            }}
          >
            <ExternalLink id="shopping-open-page-icon" aria-hidden />
          </IconButton>
        </div>
      </header>
      <div id="shopping-body" className="flex min-h-0 flex-1 flex-col gap-3 overflow-auto px-3 py-3">
        {productPage ? (
          <>
            <DealList
              id="shopping-deals"
              deals={deals}
              busy={busy}
              onSearch={search}
              coupons={coupons}
              onTrack={(offerUrl) => {
                void addPriceUrl(offerUrl)
                  .then(setItems)
                  .catch((reason: unknown) => setError(errorText(reason)));
              }}
            />
            {coupons.field?.found && !coupons.search && !coupons.busy ? (
              <p id="shopping-coupon-field-hint" className="text-xs text-emerald-300">
                This page has a coupon field. Find Coupons, then try them all here.
              </p>
            ) : null}
          </>
        ) : null}

        {(productPage && current && current.history.length > 1) ? (
          <>
            <div id="shopping-deals-divider" className="h-px w-full bg-zinc-800" />
            <section id="shopping-product" className="flex flex-col gap-2 rounded-md bg-zinc-900 px-2 py-2">
              <div id="shopping-product-row" className="flex items-start gap-2">
                <div id="shopping-product-text" className="min-w-0 flex-1">
                  <p id="shopping-product-title" className="truncate text-sm">
                    {current?.title || tab?.title || "This page"}
                  </p>
                  <p id="shopping-product-price" className="text-xs text-zinc-400">
                    {current ? formatPrice(current.price, current.currency) : null}
                    {change && change.amount < 0 ? ` · down ${formatPrice(Math.abs(change.amount), current?.currency || "")}` : ""}
                    {current?.targetPrice ? ` · target ${formatPrice(current.targetPrice, current.currency)}` : ""}
                  </p>
                  {current ? (
                    <p id="shopping-product-status" className="text-xs text-zinc-500">
                      {checkedLabel(current)}
                    </p>
                  ) : null}
                </div>
                {current && current.history.length > 1 ? <PriceSparkline id="shopping-product-spark" history={current.history} width={88} height={32} /> : null}
        
              </div>
            </section>
          </>
        ) : null}
        {domain ? (
          <TrackedSection id="shopping-tracked" items={onSite} error="" onCheck={(id) => void checkPrice(id).then(setItems).catch((reason: unknown) => setListError(errorText(reason)))} />
        ) : (
          <p id="shopping-tracked-empty" className="text-sm text-zinc-500">
            Open a site to see products tracked there.
          </p>
        )}
        {listError || error || coupons.error ? (
          <p id="shopping-error" className="text-xs text-amber-300">
            {error || coupons.error || listError}
          </p>
        ) : null}
      </div>
    </main>
  );
}
