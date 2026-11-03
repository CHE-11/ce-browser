import { useState } from "react";
import { Check, Copy, Crosshair, ExternalLink } from "lucide-react";
import { Button } from "@/components/Button";
import { IconButton } from "@/components/IconButton";
import { createTab, setCouponFeedback } from "@/lib/browser-client";
import type { ShoppingDeals } from "@type/browser";
import { formatPrice, hostOf } from "./format";

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
}: {
  id: string;
  deals: ShoppingDeals | null;
  busy: boolean;
  onSearch: (force: boolean) => void;
  onTrack?: (url: string) => void;
  onFeedback?: (deals: ShoppingDeals) => void;
}) {
  const [copied, setCopied] = useState("");
  const [focus, setFocus] = useState<"coupons" | "prices" | null>(null);
  const [hidden, setHidden] = useState(false);
  const minutes = deals?.checkedAt ? Math.max(0, Math.round((Date.now() - deals.checkedAt) / 60000)) : null;
  const showing = Boolean(deals && focus && !hidden);

  const choose = (next: "coupons" | "prices") => {
    const refresh = focus === next && Boolean(deals) && !hidden;
    setFocus(next);
    setHidden(false);
    if (!deals || refresh) {
      onSearch(Boolean(deals));
    }
  };

  return (
    <div id={id} className="flex flex-col gap-3">
      <div id={`${id}-actions`} className="flex flex-col gap-2">
        <div id={`${id}-action-buttons`} className="flex w-full gap-2">
          <Button id={`${id}-coupons-find`} className="flex-1" variant={focus === "coupons" ? "primary" : "secondary"} disabled={busy} onClick={() => choose("coupons")}>
            {busy && focus === "coupons" ? "Searching…" : "Find Coupons"}
          </Button>
          <Button id={`${id}-prices-find`} className="flex-1" variant={focus === "prices" ? "primary" : "secondary"} disabled={busy} onClick={() => choose("prices")}>
            {busy && focus === "prices" ? "Searching…" : "Compare Prices"}
          </Button>
        </div>
        {showing || minutes !== null ? (
          <div id={`${id}-action-meta`} className="flex items-center justify-between gap-2">
            {showing ? (
              <Button id={`${id}-clear`} variant="ghost" disabled={busy} onClick={() => setHidden(true)}>
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
      {showing && deals && !deals.aiAvailable ? (
        <p id={`${id}-ai-hint`} className="text-xs text-zinc-500">
          Add an AI provider in Settings for summaries.
        </p>
      ) : null}
      {showing && deals?.summary ? (
        <p id={`${id}-summary`} className="whitespace-pre-wrap text-sm text-zinc-200">
          {deals.summary}
        </p>
      ) : null}
      {showing && deals && focus === "coupons" ? (
        <>
          <section id={`${id}-coupons`} className="flex flex-col gap-1">
            <h3 id={`${id}-coupons-title`} className="text-xs font-semibold uppercase tracking-wide text-zinc-500">
              Coupons
            </h3>
            {deals.coupons.length === 0 ? (
              <p id={`${id}-coupons-empty`} className="text-sm text-zinc-500">
                No codes found in the search results.
              </p>
            ) : (
              <ul id={`${id}-coupon-list`} className="flex flex-col gap-1">
                {deals.coupons.map((coupon, index) => (
                  <li id={`${id}-coupon-${index}`} key={coupon.code} className="rounded-md bg-zinc-900 px-2 py-1.5">
                    <div id={`${id}-coupon-row-${index}`} className="flex items-center gap-2">
                      <div id={`${id}-coupon-text-${index}`} className="min-w-0 flex-1">
                        <p id={`${id}-coupon-code-${index}`} className="truncate font-mono text-sm text-zinc-100">
                          {coupon.code}
                          {coupon.worked ? " · worked" : coupon.failed ? " · failed" : coupon.triedAt ? " · tried" : ""}
                        </p>
                        <button
                          id={`${id}-coupon-source-${index}`}
                          type="button"
                          className="block max-w-full truncate text-left text-xs text-zinc-400 hover:text-sky-300"
                          title={coupon.source}
                          onClick={() => void createTab(coupon.source)}
                        >
                          {coupon.description || coupon.expiry || "See source"} · {hostOf(coupon.source)}
                        </button>
                      </div>
                      <button
                        id={`${id}-coupon-copy-${index}`}
                        type="button"
                        className="rounded p-1 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100"
                        aria-label={`Copy ${coupon.code}`}
                        onClick={() => void navigator.clipboard.writeText(coupon.code).then(() => setCopied(coupon.code))}
                      >
                        {copied === coupon.code ? <Check id={`${id}-coupon-copied-${index}`} className="size-3.5" /> : <Copy id={`${id}-coupon-copy-icon-${index}`} className="size-3.5" />}
                      </button>
                    </div>
                    <div id={`${id}-coupon-feedback-${index}`} className="mt-1 flex gap-1">
                      {(["tried", "worked", "failed"] as const).map((action) => (
                        <button
                          id={`${id}-coupon-${action}-${index}`}
                          key={action}
                          type="button"
                          className="rounded px-1.5 py-0.5 text-[11px] text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100"
                          onClick={() => {
                            if (!deals.key) {
                              return;
                            }
                            void setCouponFeedback(deals.key, coupon.code, action).then((next) => onFeedback?.(next));
                          }}
                        >
                          {action}
                        </button>
                      ))}
                    </div>
                  </li>
                ))}
              </ul>
            )}
          </section>
          <p id={`${id}-note`} className="text-[11px] text-zinc-500">
            Codes come from web search results and may have expired.
          </p>
        </>
      ) : null}
      {showing && deals && focus === "prices" ? (
        <>
          <section id={`${id}-offers`} className="flex flex-col gap-1">
            <h3 id={`${id}-offers-title`} className="text-xs font-semibold uppercase tracking-wide text-zinc-500">
              {deals.offers.some((offer) => offer.verified) ? "Verified prices" : "Unverified prices"}
            </h3>
            {deals.offers.length === 0 ? (
              <p id={`${id}-offers-empty`} className="text-sm text-zinc-500">
                No lower price found.
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
                            {offer.variantMismatch ? " · may be a different variant" : ""}
                          </span>
                          {offer.note || offer.shipping ? (
                            <span id={`${id}-offer-note-${index}`} className="block truncate text-xs text-zinc-400">
                              {[offer.note, offer.shipping].filter(Boolean).join(" · ")}
                            </span>
                          ) : null}
                        </span>
                        <span id={`${id}-offer-price-${index}`} className="shrink-0 text-sm font-medium text-emerald-300">
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
