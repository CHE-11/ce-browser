export function formatBytes(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return "0 B";
  }
  const units = ["B", "KB", "MB", "GB", "TB"];
  const exponent = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
  const value = bytes / 1024 ** exponent;
  return `${value >= 100 || exponent === 0 ? Math.round(value) : value.toFixed(1)} ${units[exponent]}`;
}

export function hostOf(url: string) {
  try {
    return new URL(url).host || url;
  } catch {
    return url;
  }
}

/** Falls back to the site's /favicon.ico for bookmarks and suggestions that have no stored icon. */
export function faviconFor(url: string) {
  try {
    const parsed = new URL(url);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return null;
    }
    return `${parsed.origin}/favicon.ico`;
  } catch {
    return null;
  }
}

/** Key of the backend favicon cache: the lowercase hostname of a web address. */
export function faviconKey(url: string | null | undefined) {
  if (!url) {
    return null;
  }
  try {
    const parsed = new URL(url);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return null;
    }
    return parsed.hostname.toLowerCase() || null;
  } catch {
    return null;
  }
}

export function isInternal(url: string) {
  return url.startsWith("ce://");
}
