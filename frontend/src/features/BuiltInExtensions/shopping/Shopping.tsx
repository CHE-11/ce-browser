import { useState } from "react";
import { Check, Copy, ExternalLink, ShoppingCart } from "lucide-react";
import { Button } from "@/components/Button";
import { PopupFrame } from "@/components/PopupFrame";
import { createTab, findDeals } from "@/lib/browser-client";
import type { ShoppingDeals, TabInfo } from "@type/browser";
import { PopoverHeader } from "../PopoverParts";
import { readPopoverTab } from "../shared";
import { ToolButton } from "../ToolButton";
import { errorText } from "../use-page-tool";

export function ShoppingButton({ activeTab }: { activeTab: TabInfo | undefined }) {
  return (
    <ToolButton
      id="shopping"
      label="Shopping"
      kind="shopping"
      icon={<ShoppingCart id="address-shopping-icon" aria-hidden />}
      activeTab={activeTab}
      height={480}
    />
  );
}

function formatPrice(price: number | null | undefined, currency: string) {
  if (price === null || price === undefined) {
    return "";
  }
  try {
    return new Intl.NumberFormat(undefined, { style: "currency", currency: currency || "USD" }).format(price);
  } catch {
    return `${currency} ${price}`.trim();
  }
}

function hostOf(url: string) {
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return url;
  }
}

export function ShoppingPopover({ params }: { params: URLSearchParams }) {
  const tab = readPopoverTab(params);
  const [deals, setDeals] = useState<ShoppingDeals | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [copied, setCopied] = useState("");

  const find = () => {
    setBusy(true);
    setError("");
    void findDeals(tab.id)
      .then(setDeals)
      .catch((reason: unknown) => setError(errorText(reason)))
      .finally(() => setBusy(false));
  };

  const copy = (code: string) => {
    void navigator.clipboard.writeText(code).then(() => setCopied(code));
  };

  return (
    <PopupFrame id="shopping-popover" maxHeight={500}>
      <PopoverHeader id="shopping-header" icon={<ShoppingCart aria-hidden />} title="Shopping" subtitle={tab.title || tab.url} />
      <div id="shopping-body" className="flex flex-col gap-3 px-3 pb-3">
        <Button id="shopping-find" variant="primary" disabled={busy} onClick={find}>
          {busy ? "Searching for deals…" : deals ? "Search again" : "Find deals"}
        </Button>
        {error ? (
          <p id="shopping-error" className="text-xs text-amber-300">
            {error}
          </p>
        ) : null}
        {deals ? (
          <>
            <div id="shopping-product" className="rounded-md bg-zinc-900 px-2 py-1.5 text-sm">
              <p id="shopping-product-name" className="truncate">
                {deals.query}
              </p>
              <p id="shopping-product-price" className="text-xs text-zinc-400">
                {deals.product ? formatPrice(deals.product.price, deals.product.currency) : "No price on this page"} · {deals.store}
              </p>
            </div>
            {deals.summary ? (
              <p id="shopping-summary" className="whitespace-pre-wrap text-sm text-zinc-200">
                {deals.summary}
              </p>
            ) : null}
            <section id="shopping-coupons" className="flex flex-col gap-1">
              <h3 id="shopping-coupons-title" className="text-xs font-semibold uppercase tracking-wide text-zinc-500">
                Coupons
              </h3>
              {deals.coupons.length === 0 ? (
                <p id="shopping-coupons-empty" className="text-sm text-zinc-500">
                  No codes found in the search results.
                </p>
              ) : (
                <ul id="shopping-coupon-list" className="flex flex-col gap-1">
                  {deals.coupons.map((coupon, index) => (
                    <li
                      id={`shopping-coupon-${index}`}
                      key={coupon.code}
                      className="flex items-center gap-2 rounded-md bg-zinc-900 px-2 py-1.5"
                    >
                      <div id={`shopping-coupon-text-${index}`} className="min-w-0 flex-1">
                        <p id={`shopping-coupon-code-${index}`} className="truncate font-mono text-sm text-zinc-100">
                          {coupon.code}
                        </p>
                        <button
                          id={`shopping-coupon-source-${index}`}
                          type="button"
                          className="block max-w-full truncate text-left text-xs text-zinc-400 hover:text-sky-300"
                          title={coupon.source}
                          onClick={() => void createTab(coupon.source)}
                        >
                          {coupon.description || "See source"} · {hostOf(coupon.source)}
                        </button>
                      </div>
                      <Button
                        id={`shopping-coupon-copy-${index}`}
                        size="sm"
                        aria-label={`Copy ${coupon.code}`}
                        onClick={() => copy(coupon.code)}
                      >
                        {copied === coupon.code ? <Check aria-hidden className="size-3.5" /> : <Copy aria-hidden className="size-3.5" />}
                        {copied === coupon.code ? "Copied" : "Copy"}
                      </Button>
                    </li>
                  ))}
                </ul>
              )}
            </section>
            <section id="shopping-offers" className="flex flex-col gap-1">
              <h3 id="shopping-offers-title" className="text-xs font-semibold uppercase tracking-wide text-zinc-500">
                Better prices
              </h3>
              {deals.offers.length === 0 ? (
                <p id="shopping-offers-empty" className="text-sm text-zinc-500">
                  No lower price found in the search results.
                </p>
              ) : (
                <ul id="shopping-offer-list" className="flex flex-col gap-1">
                  {deals.offers.map((offer, index) => (
                    <li id={`shopping-offer-${index}`} key={offer.url}>
                      <button
                        id={`shopping-offer-open-${index}`}
                        type="button"
                        title={offer.url}
                        className="flex w-full items-center gap-2 rounded-md bg-zinc-900 px-2 py-1.5 text-left hover:bg-zinc-800"
                        onClick={() => void createTab(offer.url)}
                      >
                        <span id={`shopping-offer-text-${index}`} className="min-w-0 flex-1">
                          <span id={`shopping-offer-store-${index}`} className="block truncate text-sm">
                            {offer.store || hostOf(offer.url)}
                          </span>
                          {offer.note ? (
                            <span id={`shopping-offer-note-${index}`} className="block truncate text-xs text-zinc-400">
                              {offer.note}
                            </span>
                          ) : null}
                        </span>
                        <span id={`shopping-offer-price-${index}`} className="shrink-0 text-sm font-medium text-emerald-300">
                          {formatPrice(offer.price, offer.currency || deals.product?.currency || "")}
                        </span>
                        <ExternalLink id={`shopping-offer-icon-${index}`} aria-hidden className="size-3.5 shrink-0 text-zinc-500" />
                      </button>
                    </li>
                  ))}
                </ul>
              )}
            </section>
            <details id="shopping-sources" className="text-xs text-zinc-400">
              <summary id="shopping-sources-title" className="cursor-pointer select-none text-zinc-500 hover:text-zinc-300">
                Sources ({deals.sources.length})
              </summary>
              <ul id="shopping-source-list" className="mt-1 flex flex-col gap-1">
                {deals.sources.map((source, index) => (
                  <li id={`shopping-source-${index}`} key={source.url}>
                    <button
                      id={`shopping-source-open-${index}`}
                      type="button"
                      title={source.url}
                      className="block w-full truncate text-left hover:text-sky-300"
                      onClick={() => void createTab(source.url)}
                    >
                      {source.title} · {hostOf(source.url)}
                    </button>
                  </li>
                ))}
              </ul>
            </details>
            <p id="shopping-note" className="text-[11px] text-zinc-500">
              Codes and prices come from web search results and may have expired.
            </p>
          </>
        ) : null}
      </div>
    </PopupFrame>
  );
}
