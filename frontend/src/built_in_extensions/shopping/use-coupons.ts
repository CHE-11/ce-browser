import { useCallback, useEffect, useRef, useState } from "react";
import {
  cancelCouponTry,
  detectCouponField,
  findCoupons,
  onCouponSearchProgress,
  onCouponTryProgress,
  setStoreCouponFeedback,
  tryCoupons,
} from "@/lib/browser-client";
import type { CouponField, CouponSearch, CouponSearchProgress, CouponTryProgress } from "@type/browser";
import { errorText } from "../use-page-tool";
import { siteDomain } from "./format";

const FIELD_RECHECK_MS = [1500, 4000];
const RECENT_FAIL_MS = 24 * 60 * 60 * 1000;
const SEARCH_LOG_LIMIT = 40;

/** Codes worth trying: everything except codes that failed on this store in the last day, unless all of them did. */
export function codesToTry(search: CouponSearch) {
  const now = Date.now();
  const fresh = search.coupons.filter((coupon) => !(coupon.failed && now - coupon.triedAt < RECENT_FAIL_MS));
  return (fresh.length > 0 ? fresh : search.coupons).map((coupon) => coupon.code);
}

export function tryRunning(progress: CouponTryProgress | null) {
  return progress !== null && (progress.status === "trying" || progress.status === "worked" || progress.status === "failed");
}

/** Store-wide coupon search and the Try all codes run for the tab `tabId`. */
export function useCoupons(tabId: number | undefined, url: string) {
  const store = siteDomain(url);
  const [search, setSearch] = useState<CouponSearch | null>(null);
  const [busy, setBusy] = useState(false);
  const [trying, setTrying] = useState(false);
  const [error, setError] = useState("");
  const [progress, setProgress] = useState<CouponTryProgress | null>(null);
  const [field, setField] = useState<CouponField | null>(null);
  const [searchProgress, setSearchProgress] = useState<CouponSearchProgress | null>(null);
  /** Finished search steps, oldest first. */
  const [searchLog, setSearchLog] = useState<CouponSearchProgress[]>([]);
  const tabRef = useRef(tabId);

  useEffect(() => {
    tabRef.current = tabId;
  }, [tabId]);

  useEffect(() => {
    setSearch(null);
    setError("");
    setSearchProgress(null);
    setSearchLog([]);
  }, [store]);

  useEffect(() => {
    const listen = onCouponSearchProgress((next) => {
      if (next.tabId !== tabRef.current) {
        return;
      }
      setSearchProgress(next);
      if (next.outcome) {
        setSearchLog((log) => [...log, next].slice(-SEARCH_LOG_LIMIT));
      }
    });
    return () => {
      void listen.then((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    setProgress(null);
  }, [tabId]);

  useEffect(() => {
    if (tabId === undefined || !store) {
      setField(null);
      return;
    }
    let live = true;
    const check = () => {
      void detectCouponField(tabId)
        .then((next) => live && setField(next))
        .catch(() => live && setField(null));
    };
    check();
    const timers = FIELD_RECHECK_MS.map((ms) => window.setTimeout(check, ms));
    return () => {
      live = false;
      timers.forEach((timer) => window.clearTimeout(timer));
    };
  }, [tabId, url, store]);

  useEffect(() => {
    const listen = onCouponTryProgress((next) => {
      if (next.tabId === tabRef.current) {
        setProgress(next);
      }
    });
    return () => {
      void listen.then((unlisten) => unlisten());
    };
  }, []);

  const find = useCallback(
    (force: boolean) => {
      if (tabId === undefined) {
        return;
      }
      setBusy(true);
      setError("");
      setSearchProgress(null);
      setSearchLog([]);
      void findCoupons(tabId, force)
        .then(setSearch)
        .catch((reason: unknown) => setError(errorText(reason)))
        .finally(() => setBusy(false));
    },
    [tabId],
  );

  const tryAll = useCallback(() => {
    if (tabId === undefined || !search || trying) {
      return;
    }
    const codes = codesToTry(search);
    if (codes.length === 0) {
      return;
    }
    setTrying(true);
    setError("");
    setProgress({
      tabId,
      index: 0,
      total: codes.length,
      code: codes[0],
      status: "trying",
      saving: null,
      bestCode: "",
      bestSaving: null,
      currency: field?.currency ?? "",
      message: "",
    });
    void tryCoupons(tabId, codes)
      .then(setSearch)
      .catch((reason: unknown) => setError(errorText(reason)))
      .finally(() => setTrying(false));
  }, [field?.currency, search, tabId, trying]);

  const cancel = useCallback(() => {
    void cancelCouponTry();
  }, []);

  const mark = useCallback(
    (code: string, action: "tried" | "worked" | "failed") => {
      if (!search) {
        return;
      }
      void setStoreCouponFeedback(search.store, code, action)
        .then(setSearch)
        .catch((reason: unknown) => setError(errorText(reason)));
    },
    [search],
  );

  return { store, search, busy, trying, error, progress, field, searchProgress, searchLog, find, tryAll, cancel, mark };
}

export type CouponsState = ReturnType<typeof useCoupons>;
