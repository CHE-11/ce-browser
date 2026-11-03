import type { ProductPrice } from "@type/browser";

export function formatPrice(price: number | null | undefined, currency: string) {
  if (price === null || price === undefined || Number.isNaN(price)) {
    return "";
  }
  try {
    return new Intl.NumberFormat(undefined, { style: "currency", currency: currency || "USD" }).format(price);
  } catch {
    return `${currency} ${price.toFixed(2)}`.trim();
  }
}

export function hostOf(url: string) {
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return url;
  }
}

/** Hostname of an http(s) page, without a leading www. Empty for other pages. */
export function siteDomain(url: string) {
  try {
    const parsed = new URL(url);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return "";
    }
    return parsed.hostname.replace(/^www\./i, "").toLowerCase();
  } catch {
    return "";
  }
}

export function firstPrice(item: ProductPrice) {
  return item.history[0]?.price ?? item.previousPrice ?? item.price;
}

export function priceChange(item: ProductPrice) {
  const first = firstPrice(item);
  const amount = item.price - first;
  const percent = first > 0 ? (amount / first) * 100 : 0;
  return { amount, percent };
}

export function inStock(item: ProductPrice) {
  const text = item.availability.toLowerCase().replace(/[\s_-]/g, "");
  if (!text) {
    return false;
  }
  if (text.includes("outofstock") || text.includes("soldout") || text.includes("discontinued")) {
    return false;
  }
  return text.includes("instock") || text.includes("limitedavailability") || text.includes("preorder") || text.includes("onlineonly");
}

export function belowTarget(item: ProductPrice) {
  return item.targetPrice !== null && item.price <= item.targetPrice;
}

export function checkedLabel(item: ProductPrice) {
  if (item.lastError) {
    return item.lastError;
  }
  if (!item.lastCheckedAt) {
    return "Not checked yet";
  }
  const minutes = Math.max(0, Math.round((Date.now() - item.lastCheckedAt) / 60000));
  if (minutes < 1) {
    return "Checked just now";
  }
  if (minutes < 60) {
    return `Checked ${minutes}m ago`;
  }
  const hours = Math.round(minutes / 60);
  return `Checked ${hours}h ago`;
}

export type ShoppingFilter = "all" | "dropped" | "target" | "stock" | "error" | "paused";
export type ShoppingSort = "drop" | "percent" | "price" | "checked";

export function matchesFilter(item: ProductPrice, filter: ShoppingFilter) {
  if (filter === "dropped") {
    return item.dropped;
  }
  if (filter === "target") {
    return belowTarget(item);
  }
  if (filter === "stock") {
    return inStock(item);
  }
  if (filter === "error") {
    return item.lastError.length > 0;
  }
  if (filter === "paused") {
    return item.paused;
  }
  return true;
}

export function matchesQuery(item: ProductPrice, query: string) {
  const needle = query.trim().toLowerCase();
  if (!needle) {
    return true;
  }
  return [item.title, item.store, item.note, item.tags.join(" ")].join(" ").toLowerCase().includes(needle);
}

export function sortItems(items: ProductPrice[], sort: ShoppingSort) {
  const copy = [...items];
  copy.sort((left, right) => {
    if (sort === "price") {
      return left.price - right.price;
    }
    if (sort === "checked") {
      return right.lastCheckedAt - left.lastCheckedAt;
    }
    if (sort === "percent") {
      return priceChange(left).percent - priceChange(right).percent;
    }
    return priceChange(left).amount - priceChange(right).amount;
  });
  return copy;
}

export function summaryOf(items: ProductPrice[]) {
  const dropped = items.filter((item) => item.dropped).length;
  const targets = items.filter(belowTarget).length;
  const saving = items.reduce((sum, item) => {
    const change = priceChange(item).amount;
    return change < 0 ? sum + Math.abs(change) : sum;
  }, 0);
  return { count: items.length, dropped, targets, saving };
}
