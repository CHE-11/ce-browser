import { useState } from "react";
import { Check, Copy } from "lucide-react";
import { cn } from "@/lib/cn";
import { createTab } from "@/lib/browser-client";
import type { Coupon, CouponSearch, CouponTryProgress } from "@type/browser";
import { CouponTryBox } from "./CouponTryBox";
import { formatPrice, hostOf } from "./format";
import { codesToTry, tryRunning } from "./use-coupons";

function couponStatus(coupon: Coupon) {
  if (coupon.worked) {
    return "worked";
  }
  if (coupon.failed) {
    return "failed";
  }
  return coupon.triedAt ? "tried" : "";
}

function siteCount(search: CouponSearch) {
  return new Set(search.coupons.map((coupon) => hostOf(coupon.source)).filter(Boolean)).size;
}

export function CouponList({
  id,
  search,
  progress,
  trying,
  fieldFound,
  canTry = true,
  onTryAll,
  onCancel,
  onMark,
}: {
  id: string;
  search: CouponSearch;
  progress: CouponTryProgress | null;
  trying: boolean;
  /** The page has a coupon field, so codes can be tried on it. */
  fieldFound: boolean;
  /** False when the codes are not for an open tab, which hides Try all codes. */
  canTry?: boolean;
  onTryAll: () => void;
  onCancel: () => void;
  onMark: (code: string, action: "tried" | "worked" | "failed") => void;
}) {
  const [copied, setCopied] = useState("");
  const running = trying || tryRunning(progress);
  const count = codesToTry(search).length;
  const sites = siteCount(search);

  return (
    <div id={id} className="flex flex-col gap-2">
      {!search.aiAvailable ? (
        <p id={`${id}-ai-hint`} className="text-xs text-zinc-500">
          Add an AI provider in Settings for code descriptions.
        </p>
      ) : null}
      {search.summary ? (
        <p id={`${id}-summary`} className="whitespace-pre-wrap text-sm text-zinc-200">
          {search.summary}
        </p>
      ) : null}

      {canTry ? (
        <CouponTryBox
          id={`${id}-try`}
          count={count}
          progress={progress}
          running={running}
          fieldFound={fieldFound}
          onTryAll={onTryAll}
          onCancel={onCancel}
        />
      ) : null}

      <section id={`${id}-coupons`} className="flex flex-col gap-1">
        <h3 id={`${id}-coupons-title`} className="text-xs font-semibold uppercase tracking-wide text-zinc-500">
          {search.coupons.length === 0
            ? "Coupons"
            : `${search.coupons.length} ${search.coupons.length === 1 ? "code" : "codes"} from ${sites} ${sites === 1 ? "site" : "sites"}`}
        </h3>
        {search.coupons.length === 0 ? (
          <p id={`${id}-coupons-empty`} className="text-sm text-zinc-500">
            No codes found for {search.store}.
          </p>
        ) : (
          <ul id={`${id}-coupon-list`} className="flex flex-col gap-1">
            {search.coupons.map((coupon, index) => {
              const status = couponStatus(coupon);
              const active = running && progress?.code.toLowerCase() === coupon.code.toLowerCase();
              return (
                <li
                  id={`${id}-coupon-${index}`}
                  key={coupon.code}
                  className={cn("rounded-md bg-zinc-900 px-2 py-1.5", active && "ring-1 ring-sky-500", coupon.failed && !active && "opacity-60")}
                >
                  <div id={`${id}-coupon-row-${index}`} className="flex items-center gap-2">
                    <div id={`${id}-coupon-text-${index}`} className="min-w-0 flex-1">
                      <p id={`${id}-coupon-head-${index}`} className="flex min-w-0 items-center gap-1.5">
                        <span id={`${id}-coupon-code-${index}`} className="truncate font-mono text-sm text-zinc-100">
                          {coupon.code}
                        </span>
                        {coupon.saving ? (
                          <span id={`${id}-coupon-saving-${index}`} className="shrink-0 rounded bg-emerald-950 px-1 text-[11px] text-emerald-300">
                            saves {formatPrice(coupon.saving, coupon.currency)}
                          </span>
                        ) : null}
                        {status ? (
                          <span
                            id={`${id}-coupon-status-${index}`}
                            className={cn(
                              "shrink-0 text-[11px]",
                              status === "worked" && "text-emerald-400",
                              status === "failed" && "text-red-400",
                              status === "tried" && "text-zinc-500",
                            )}
                          >
                            {status}
                          </span>
                        ) : null}
                        {coupon.sources > 1 ? (
                          <span id={`${id}-coupon-sites-${index}`} className="shrink-0 text-[11px] text-zinc-500">
                            {coupon.sources} sites
                          </span>
                        ) : null}
                      </p>
                      {coupon.description ? (
                        <p id={`${id}-coupon-description-${index}`} className="line-clamp-2 text-xs text-zinc-300" title={coupon.description}>
                          {coupon.description}
                        </p>
                      ) : null}
                      {coupon.source ? (
                        <button
                          id={`${id}-coupon-source-${index}`}
                          type="button"
                          className="block max-w-full truncate text-left text-xs text-zinc-500 hover:text-sky-300"
                          title={coupon.source}
                          onClick={() => void createTab(coupon.source)}
                        >
                          {[coupon.expiry, hostOf(coupon.source)].filter(Boolean).join(" · ")}
                        </button>
                      ) : null}
                    </div>
                    <button
                      id={`${id}-coupon-copy-${index}`}
                      type="button"
                      className="rounded p-1 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100"
                      aria-label={`Copy ${coupon.code}`}
                      onClick={() => void navigator.clipboard.writeText(coupon.code).then(() => setCopied(coupon.code))}
                    >
                      {copied === coupon.code ? (
                        <Check id={`${id}-coupon-copied-${index}`} className="size-3.5" />
                      ) : (
                        <Copy id={`${id}-coupon-copy-icon-${index}`} className="size-3.5" />
                      )}
                    </button>
                  </div>
                  <div id={`${id}-coupon-feedback-${index}`} className="mt-1 flex gap-1">
                    {(["tried", "worked", "failed"] as const).map((action) => (
                      <button
                        id={`${id}-coupon-${action}-${index}`}
                        key={action}
                        type="button"
                        disabled={running}
                        className="rounded px-1.5 py-0.5 text-[11px] text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100 disabled:opacity-40"
                        onClick={() => onMark(coupon.code, action)}
                      >
                        {action}
                      </button>
                    ))}
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </section>
      <p id={`${id}-note`} className="text-[11px] text-zinc-500">
        {search.searchNote ? `${search.searchNote} ` : ""}Codes come from the web and may have expired.
      </p>
    </div>
  );
}
