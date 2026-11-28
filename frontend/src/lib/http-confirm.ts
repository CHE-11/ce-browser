import type { BrowserSettings } from "@type/browser";

function siteKey(host: string) {
  const lower = host.trim().replace(/\.$/, "").toLowerCase();
  return lower.startsWith("www.") && lower.length > 4 ? lower.slice(4) : lower;
}

function localHost(host: string) {
  return host === "localhost" || host === "127.0.0.1" || host === "::1" || host === "[::1]" || host.endsWith(".localhost");
}

/**
 * An explicit `http` address HTTPS-only would otherwise load as `https`.
 * Localhost and saved exceptions stay on `http` without a question.
 */
export function httpNeedsConfirm(raw: string, settings: Pick<BrowserSettings, "httpsOnly" | "httpsExceptions"> | null) {
  if (!settings?.httpsOnly) {
    return false;
  }
  let url: URL;
  try {
    url = new URL(raw.trim());
  } catch {
    return false;
  }
  if (url.protocol !== "http:") {
    return false;
  }
  const host = url.hostname.toLowerCase();
  if (localHost(host)) {
    return false;
  }
  const key = siteKey(host);
  return !settings.httpsExceptions.some((site) => {
    const exception = siteKey(site);
    return exception !== "" && (key === exception || key.endsWith(`.${exception}`));
  });
}
