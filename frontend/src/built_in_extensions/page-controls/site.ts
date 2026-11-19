import type { PageRules, SiteControl } from "@type/browser";

export const EMPTY_SITE: SiteControl = {
  forceInteraction: false,
  cleanPage: false,
  dark: { enabled: false, brightness: 100, contrast: 100 },
  userAgent: "",
  videoSpeed: 1,
  blockScript: false,
  blockImages: false,
  blockFonts: false,
  hidden: [],
  referrer: "default",
  zoom: 0,
  pageZoomDomain: false,
  pageZoomUrls: [],
};

/** Host of an http(s) page, without a leading `www.`. */
export function siteKey(url: string | undefined) {
  if (!url) {
    return "";
  }
  try {
    const parsed = new URL(url);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return "";
    }
    const host = parsed.hostname.replace(/\.$/, "").toLowerCase();
    return host.startsWith("www.") ? host.slice(4) : host;
  } catch {
    return "";
  }
}

/** The most specific stored rule for `host`, or the defaults. */
export function controlFor(rules: PageRules | null, host: string): SiteControl {
  if (!rules || !host) {
    return EMPTY_SITE;
  }
  let best: { key: string; site: SiteControl } | null = null;
  for (const [key, site] of Object.entries(rules.sites)) {
    if (host === key || host.endsWith(`.${key}`)) {
      if (!best || key.length > best.key.length) {
        best = { key, site };
      }
    }
  }
  return best?.site ?? EMPTY_SITE;
}

export function siteActive(site: SiteControl) {
  return (
    site.forceInteraction ||
    site.cleanPage ||
    site.dark.enabled ||
    site.userAgent.trim() !== "" ||
    site.videoSpeed !== 1 ||
    site.blockScript ||
    site.blockImages ||
    site.blockFonts ||
    site.hidden.length > 0 ||
    site.referrer !== "default" ||
    (site.zoom ?? 0) !== 0 ||
    site.pageZoomDomain === true ||
    (site.pageZoomUrls ?? []).length > 0
  );
}

/** Address without a hash or a trailing slash. Empty when `url` is not http(s). */
export function pageAddress(url: string | undefined) {
  if (!url) {
    return "";
  }
  try {
    const parsed = new URL(url);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return "";
    }
    parsed.hash = "";
    const text = parsed.toString();
    return text.endsWith("/") ? text.slice(0, -1) : text;
  } catch {
    return "";
  }
}

/** Ctrl+scroll over the page does not zoom when the domain or this address is locked. */
export function pageZoomDisabled(rules: PageRules | null, url: string | undefined) {
  const site = controlFor(rules, siteKey(url));
  if (site.pageZoomDomain) {
    return true;
  }
  const address = pageAddress(url);
  return address !== "" && (site.pageZoomUrls ?? []).includes(address);
}
