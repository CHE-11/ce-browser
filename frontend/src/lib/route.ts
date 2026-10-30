import { useEffect, useState } from "react";

export type Route =
  | { kind: "browser" }
  | { kind: "popup"; popup: string; params: URLSearchParams }
  | { kind: "page"; page: string; params: URLSearchParams };

/** No hash is the browser UI, `#/popup/<kind>?...` a popup, `#/<page>?...` a ce:// page. */
export function parseRoute(hash: string): Route {
  const path = hash.replace(/^#\/?/, "");
  if (!path) {
    return { kind: "browser" };
  }
  const [name, query = ""] = path.split("?");
  const params = new URLSearchParams(query);
  if (name.startsWith("popup/")) {
    return { kind: "popup", popup: name.slice("popup/".length), params };
  }
  return { kind: "page", page: name, params };
}

export function useRoute() {
  const [route, setRoute] = useState(() => parseRoute(window.location.hash));
  useEffect(() => {
    const onHash = () => setRoute(parseRoute(window.location.hash));
    window.addEventListener("hashchange", onHash);
    return () => window.removeEventListener("hashchange", onHash);
  }, []);
  return route;
}

/** Moves a ce:// page to another ce:// page inside the same tab. */
export function goToPage(page: string) {
  window.location.hash = `#/${page}`;
}
