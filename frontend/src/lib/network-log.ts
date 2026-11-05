import type { NetworkEntry, NetworkHeader } from "@type/browser";

const AUTH_HEADERS = new Set([
  "authorization",
  "proxy-authorization",
  "www-authenticate",
  "proxy-authenticate",
  "authentication-info",
  "cookie",
  "set-cookie",
  "x-api-key",
  "x-auth-token",
  "x-access-token",
  "x-csrf-token",
  "x-xsrf-token",
]);

export const networkColumnClass = {
  toggle: "w-6 shrink-0",
  status: "w-14 shrink-0",
  method: "w-16 shrink-0",
  when: "w-36 shrink-0",
  duration: "w-16 shrink-0 text-right",
  auth: "w-20 shrink-0",
  url: "min-w-0 flex-1",
} as const;

export type AuthFact = {
  key: string;
  label: string;
  detail: string;
  value: string;
};

export function isAuthHeader(name: string) {
  return AUTH_HEADERS.has(name.trim().toLowerCase());
}

export function formatWhen(time: number) {
  return new Date(time).toLocaleString(undefined, {
    month: "numeric",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
    second: "2-digit",
  });
}

export function formatClock(time: number) {
  return new Date(time).toLocaleTimeString(undefined, {
    hour: "numeric",
    minute: "2-digit",
    second: "2-digit",
  });
}

export function formatDuration(duration: number | null) {
  if (duration === null) {
    return "–";
  }
  if (duration < 1000) {
    return `${duration} ms`;
  }
  const seconds = duration / 1000;
  if (seconds < 10) {
    return `${seconds.toFixed(1)} s`;
  }
  if (seconds < 60) {
    return `${Math.round(seconds)} s`;
  }
  const minutes = seconds / 60;
  if (minutes < 10) {
    return `${minutes.toFixed(1)} m`;
  }
  return `${Math.round(minutes)} m`;
}

function authorization(entry: NetworkEntry) {
  return entry.requestHeaders.find((header) => header.name.toLowerCase() === "authorization");
}

/** Short label for the auth column: the scheme, a cookie, or a token URL. */
export function authBadge(entry: NetworkEntry) {
  const header = authorization(entry);
  if (header) {
    const scheme = header.value.trim().split(/\s+/)[0];
    return scheme || "Authorization";
  }
  const url = entry.url.toLowerCase();
  if (url.includes("refresh") && /token|oauth|auth/.test(url)) {
    return "Refresh";
  }
  if (entry.requestHeaders.some((item) => item.name.toLowerCase() === "cookie")) {
    return "Cookie";
  }
  if (entry.responseHeaders.some((item) => item.name.toLowerCase() === "set-cookie")) {
    return "Set-Cookie";
  }
  if (
    entry.requestHeaders.some((item) => isAuthHeader(item.name)) ||
    entry.responseHeaders.some((item) => isAuthHeader(item.name))
  ) {
    return "Auth";
  }
  if (/\/(oauth|authorize|login|signin|sign-in|session)\b/.test(url)) {
    return "Auth";
  }
  return null;
}

export function authFacts(entry: NetworkEntry): AuthFact[] {
  const facts: AuthFact[] = [];
  const push = (header: NetworkHeader, source: "request" | "response", index: number) => {
    if (!isAuthHeader(header.name)) {
      return;
    }
    const name = header.name.toLowerCase();
    let detail = source === "request" ? "Request" : "Response";
    if (name === "authorization" || name === "proxy-authorization") {
      const [scheme, ...rest] = header.value.trim().split(/\s+/);
      detail = rest.length > 0 ? `${scheme} credential` : "Credential";
    } else if (name === "www-authenticate" || name === "proxy-authenticate") {
      detail = "Challenge";
    }
    facts.push({
      key: `${source}-${index}-${name}`,
      label: header.name,
      detail,
      value: header.value,
    });
  };
  entry.requestHeaders.forEach((header, index) => push(header, "request", index));
  entry.responseHeaders.forEach((header, index) => push(header, "response", index));
  return facts;
}

export function entryMatches(entry: NetworkEntry, query: string, authOnly: boolean) {
  if (authOnly && !authBadge(entry)) {
    return false;
  }
  const needle = query.trim().toLowerCase();
  if (!needle) {
    return true;
  }
  if (entry.url.toLowerCase().includes(needle) || entry.method.toLowerCase().includes(needle)) {
    return true;
  }
  if (String(entry.status ?? "").includes(needle)) {
    return true;
  }
  const badge = authBadge(entry);
  if (badge?.toLowerCase().includes(needle)) {
    return true;
  }
  return [...entry.requestHeaders, ...entry.responseHeaders].some(
    (header) => header.name.toLowerCase().includes(needle) || header.value.toLowerCase().includes(needle),
  );
}
