import { useEffect, useState } from "react";
import { Check, Crosshair, ExternalLink, ShoppingCart } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { PopupFrame } from "@/components/PopupFrame";
import {
  addPriceUrl,
  checkPrice,
  closePopup,
  createTab,
  findDeals,
  listPrices,
  onPricesChanged,
  trackPrice,
  untrackPrice,
} from "@/lib/browser-client";
import type { ProductPrice, ShoppingDeals } from "@type/browser";
import { DealList } from "@/features/BuiltInExtensions/shopping/DealList";
import { checkedLabel, formatPrice, priceChange, siteDomain } from "@/features/BuiltInExtensions/shopping/format";
import { PriceSparkline } from "@/features/BuiltInExtensions/shopping/PriceSparkline";
import { PopoverHeader } from "../PopoverParts";
import { readPopoverTab } from "../shared";
import { errorText } from "../use-page-tool";
import { TrackedSection } from "./TrackedSection";


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

export function ShoppingPopover({ params }: { params: URLSearchParams }) {
  const tab = readPopoverTab(params);
  const productPage = /^https?:/i.test(tab.url);
  const [items, setItems] = useState<ProductPrice[]>([]);
  const [listError, setListError] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [deals, setDeals] = useState<ShoppingDeals | null>(null);

  useEffect(() => {
    void listPrices()
      .then(setItems)
      .catch((reason: unknown) => setListError(errorText(reason)));
    const listen = onPricesChanged(setItems);
    return () => {
      void listen.then((unlisten) => unlisten());
    };
  }, []);

  const current = items.find((item) => samePage(item, tab.url));
  const domain = siteDomain(tab.url);
  const onSite = domain ? items.filter((item) => siteDomain(item.url) === domain) : [];
  const change = current ? priceChange(current) : null;

  const track = () => {
    setError("");
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
    setBusy(true);
    setError("");
    void findDeals(tab.id, force)
      .then(setDeals)
      .catch((reason: unknown) => setError(errorText(reason)))
      .finally(() => setBusy(false));
  };

  return (
    <PopupFrame id="shopping-popover" width={480} maxHeight={760} className="min-h-[36rem]">
      
      <PopoverHeader
        id="shopping-header"
        icon={<ShoppingCart aria-hidden />}
        title="Shopping"
        subtitle={tab.title || tab.url}
        action={
          <IconButton
            id="shopping-open-page-icon"
            label="Open Shopping page"
            onClick={() => {
              void createTab("ce://shopping");
              void closePopup();
            }}
          >
            <ExternalLink id="shopping-open-page-icon-glyph" aria-hidden />
          </IconButton>
        }
      />

      <div id="shopping-body" className="flex flex-col gap-3 px-3 pb-3">
        {/* Divider between the header and the content */}
        <div id='divider' className="h-px w-full bg-zinc-800"></div>
        {productPage ? (
          <>
            <DealList
              id="shopping-deals"
              deals={deals}
              busy={busy}
              onSearch={search}
              onFeedback={setDeals}
              onTrack={(url) => {
                void addPriceUrl(url)
                  .then(setItems)
                  .catch((reason: unknown) => setError(errorText(reason)));
              }}
            />
            <div id='divider' className="h-px w-full bg-zinc-800"></div>
          </>
        ) : null}

        {productPage ? (
          <section id="shopping-product" className="flex flex-col gap-2 rounded-md bg-zinc-900 px-2 py-2">
            <div id="shopping-product-row" className="flex items-start gap-2">
              <div id="shopping-product-text" className="min-w-0 flex-1">
                <p id="shopping-product-title" className="truncate text-sm">
                  {current?.title || tab.title || "This page"}
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
              <div id="shopping-product-icons" className="flex shrink-0 items-center">
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
              </div>
            </div>
          </section>
        ) : null}
        {domain ? (
          <TrackedSection id="shopping-tracked" items={onSite} error="" onCheck={(id) => void checkPrice(id).then(setItems).catch((reason: unknown) => setListError(errorText(reason)))} />
        ) : (
          <p id="shopping-tracked-empty" className="text-sm text-zinc-500">
            Open a site to see products tracked there.
          </p>
        )}
        {listError || error ? (
          <p id="shopping-error" className="text-xs text-amber-300">
            {error || listError}
          </p>
        ) : null}

      </div>
    </PopupFrame>
  );
}
