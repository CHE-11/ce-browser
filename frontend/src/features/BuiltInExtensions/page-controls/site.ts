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
    site.referrer !== "default"
  );
}
