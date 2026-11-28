import { useState } from "react";
import { Crosshair, ExternalLink } from "lucide-react";
import { Button } from "@/components/Button";
import { IconButton } from "@/components/IconButton";
import { createTab, setCouponFeedback } from "@/lib/browser-client";
import type { CouponSearch, ShoppingDeals } from "@type/browser";
import { CouponList } from "./CouponList";
import { CouponSearchStatus } from "./CouponSearchStatus";
import { formatPrice, hostOf } from "./format";
import type { CouponsState } from "./use-coupons";

/** Coupons from a product deal search, shown in the same list as a store-wide search. */
function dealCoupons(deals: ShoppingDeals | null): CouponSearch | null {
  if (!deals) {
    return null;
  }
  return {
    store: deals.store,
    coupons: deals.coupons,
    sources: deals.sources,
    summary: deals.summary,
    aiAvailable: deals.aiAvailable,
    checkedAt: deals.checkedAt,
    cached: deals.cached,
    searchNote: "",
    pagesRead: 0,
  };
}

function offerImage(image: string, page: string) {
  try {
    return new URL(image, page).href;
  } catch {
    return image;
  }
}

export function DealList({
  id,
  deals,
  busy,
  onSearch,
  onTrack,
  onFeedback,
  coupons,
  pricesOnly = false,
}: {
  id: string;
  deals: ShoppingDeals | null;
  busy: boolean;
  onSearch: (force: boolean) => void;
  onTrack?: (url: string) => void;
  /** Receives deals after a code is marked, when coupons come from `deals`. */
  onFeedback?: (deals: ShoppingDeals) => void;
  /** Store-wide coupon search and Try all codes for an open tab. Without it, coupons come from `deals`. */
  coupons?: CouponsState;
  /** Skips coupons and opens on lower prices at other stores. */
  pricesOnly?: boolean;
}) {
  const [focus, setFocus] = useState<"coupons" | "prices" | null>(pricesOnly ? "prices" : null);
  const [hidden, setHidden] = useState(false);
  const couponSearch = coupons ? coupons.search : dealCoupons(deals);
  const couponBusy = coupons ? coupons.busy : busy;
  const checkedAt = focus === "coupons" ? couponSearch?.checkedAt : deals?.checkedAt;
  const minutes = checkedAt ? Math.max(0, Math.round((Date.now() - checkedAt) / 60000)) : null;
  const showingCoupons = Boolean(focus === "coupons" && couponSearch && !hidden);
  const showingPrices = Boolean(focus === "prices" && deals && !hidden);
  const showing = showingCoupons || showingPrices;
  const searching = (focus === "coupons" && couponBusy) || (focus === "prices" && busy);

  const choose = (next: "coupons" | "prices") => {
    const current = next === "coupons" ? couponSearch : deals;
    const refresh = focus === next && Boolean(current) && !hidden;
    setFocus(next);
    setHidden(false);
    if (current && !refresh) {
      return;
    }
    if (next === "coupons" && coupons) {
      coupons.find(Boolean(current));
    } else {
      onSearch(Boolean(current));
    }
  };

  const markDealCoupon = (code: string, action: "tried" | "worked" | "failed") => {
    if (!deals?.key) {
      return;
    }
    void setCouponFeedback(deals.key, code, action).then((next) => onFeedback?.(next));
  };

  return (
    <div id={id} className="flex flex-col gap-3">
      <div id={`${id}-actions`} className="flex flex-col gap-2">
        <div id={`${id}-action-buttons`} className="flex w-full gap-2">
          {pricesOnly ? null : (
            <Button
              id={`${id}-coupons-find`}
              className="flex-1"
              variant={focus === "coupons" ? "primary" : "secondary"}
              disabled={couponBusy || Boolean(coupons?.trying)}
              onClick={() => choose("coupons")}
            >
              {couponBusy && focus === "coupons" ? "Searching…" : showingCoupons ? "Search again" : "Find Coupons"}
            </Button>
          )}
          <Button id={`${id}-prices-find`} className="flex-1" variant={focus === "prices" ? "primary" : "secondary"} disabled={busy} onClick={() => choose("prices")}>
            {busy && focus === "prices" ? "Searching…" : pricesOnly && deals ? "Search again" : "Compare Prices"}
          </Button>
        </div>
        {showing || minutes !== null ? (
          <div id={`${id}-action-meta`} className="flex items-center justify-between gap-2">
            {showing ? (
              <Button id={`${id}-clear`} variant="ghost" disabled={Boolean(searching) || Boolean(coupons?.trying)} onClick={() => setHidden(true)}>
                Clear results
              </Button>
            ) : (
              <span id={`${id}-clear-spacer`} />
            )}
            {minutes !== null ? (
              <p id={`${id}-checked`} className="text-xs text-zinc-500">
                {minutes < 1 ? "Checked just now" : `Checked ${minutes}m ago`}
              </p>
            ) : null}
          </div>
        ) : null}
      </div>
      {focus === "coupons" && coupons && (coupons.busy || !hidden) ? (
        <CouponSearchStatus id={`${id}-coupon-search`} live={coupons.busy} progress={coupons.searchProgress} log={coupons.searchLog} />
      ) : null}
      {showingCoupons && couponSearch ? (
        coupons ? (
          <CouponList
            id={`${id}-coupon-results`}
            search={couponSearch}
            progress={coupons.progress}
            trying={coupons.trying}
            fieldFound={Boolean(coupons.field?.found)}
            onTryAll={coupons.tryAll}
            onCancel={coupons.cancel}
            onMark={coupons.mark}
          />
        ) : (
          <CouponList
            id={`${id}-coupon-results`}
            search={couponSearch}
            progress={null}
            trying={false}
            fieldFound={false}
            canTry={false}
            onTryAll={() => undefined}
            onCancel={() => undefined}
            onMark={markDealCoupon}
          />
        )
      ) : null}
      {showingPrices && deals && !deals.aiAvailable ? (
        <p id={`${id}-ai-hint`} className="text-xs text-zinc-500">
          Add an AI provider in Settings for summaries.
        </p>
      ) : null}
      {showingPrices && deals?.summary ? (
        <p id={`${id}-summary`} className="whitespace-pre-wrap text-sm text-zinc-200">
          {deals.summary}
        </p>
      ) : null}
      {showingPrices && deals ? (
        <>
          <section id={`${id}-offers`} className="flex flex-col gap-1">
            <h3 id={`${id}-offers-title`} className="text-xs font-semibold uppercase tracking-wide text-zinc-500">
              {deals.offers.some((offer) => offer.verified) ? "Verified prices" : "Unverified prices"}
            </h3>
            {deals.searchNote ? (
              <p id={`${id}-search-note`} className="text-xs text-zinc-500">
                {deals.searchNote}
              </p>
            ) : null}
            {deals.offers.length === 0 ? (
              <p id={`${id}-offers-empty`} className="text-sm text-zinc-500">
                {pricesOnly ? "No price could be confirmed." : "No lower price found."}
              </p>
            ) : (
              <ul id={`${id}-offer-list`} className="flex flex-col gap-1">
                {deals.offers.map((offer, index) => (
                  <li id={`${id}-offer-${index}`} key={offer.url} className="rounded-md bg-zinc-900 px-2 py-1.5">
                    <div id={`${id}-offer-row-${index}`} className="flex items-center gap-1">
                      {!offer.variantMismatch && offer.imageUrl ? (
                        <img
                          id={`${id}-offer-image-${index}`}
                          src={offerImage(offer.imageUrl, offer.url)}
                          alt=""
                          className="size-12 shrink-0 rounded bg-zinc-800 object-contain"
                        />
                      ) : null}
                      <button
                        id={`${id}-offer-open-${index}`}
                        type="button"
                        className="flex min-w-0 flex-1 items-center gap-2 text-left hover:text-sky-300"
                        onClick={() => void createTab(offer.url)}
                      >
                        <span id={`${id}-offer-text-${index}`} className="min-w-0 flex-1">
                          <span id={`${id}-offer-store-${index}`} className="block truncate text-sm">
                            {offer.store || hostOf(offer.url)}
                            {offer.verified ? "" : " · unverified"}
                            {offer.lower ? " · lower" : ""}
                            {offer.variantMismatch ? " · may be a different variant" : ""}
                          </span>
                          {offer.note || offer.shipping ? (
                            <span id={`${id}-offer-note-${index}`} className="block truncate text-xs text-zinc-400">
                              {[offer.note, offer.shipping].filter(Boolean).join(" · ")}
                            </span>
                          ) : null}
                        </span>
                        <span id={`${id}-offer-price-${index}`} className={offer.lower ? "shrink-0 text-sm font-medium text-emerald-300" : "shrink-0 text-sm font-medium"}>
                          {formatPrice(offer.price, offer.currency || deals.product?.currency || "")}
                        </span>
                      </button>
                      <button
                        id={`${id}-offer-link-${index}`}
                        type="button"
                        className="inline-flex size-6 shrink-0 items-center justify-center rounded text-zinc-500 hover:bg-zinc-800 hover:text-zinc-100"
                        aria-label="Open offer"
                        title="Open offer"
                        onClick={() => void createTab(offer.url)}
                      >
                        <ExternalLink id={`${id}-offer-icon-${index}`} aria-hidden className="size-3.5" />
                      </button>
                      {onTrack ? (
                        <IconButton id={`${id}-offer-track-${index}`} label="Track" className="size-6 text-zinc-500" onClick={() => onTrack(offer.url)}>
                          <Crosshair id={`${id}-offer-track-icon-${index}`} aria-hidden />
                        </IconButton>
                      ) : null}
                    </div>
                  </li>
                ))}
              </ul>
            )}
          </section>
          <p id={`${id}-prices-note`} className="text-[11px] text-zinc-500">
            Prices come from web search results and may have expired.
          </p>
        </>
      ) : null}
    </div>
  );
}
