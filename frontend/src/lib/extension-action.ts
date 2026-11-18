import { useMemo } from "react";
import type { ExtensionActionState } from "@type/browser";
import { useExtensionActions } from "./use-browser";

export type ExtensionActionFace = {
  icon: string | null;
  badge: string;
  badgeColor: string;
  badgeTextColor: string;
  title: string;
};

const EMPTY_ACTION: ExtensionActionFace = {
  icon: null,
  badge: "",
  badgeColor: "",
  badgeTextColor: "",
  title: "",
};

/** Page address the badge was stored under. Fragments and non-web URLs do not count. */
export function extensionPageKey(url: string) {
  const bare = url.split("#")[0]?.trim() ?? "";
  if (!bare.startsWith("http://") && !bare.startsWith("https://")) {
    return "";
  }
  return bare.length > 2000 ? bare.slice(0, 2000) : bare;
}

function samePage(stored: string, current: string) {
  if (stored === current) {
    return true;
  }
  const trim = (value: string) => (value.length > "https://".length && value.endsWith("/") ? value.slice(0, -1) : value);
  return trim(stored) === trim(current);
}

/** The icon and badge to draw for this extension on the open page. */
export function resolveExtensionAction(actions: ExtensionActionState[], id: string, pageUrl?: string): ExtensionActionFace {
  if (!id) {
    return EMPTY_ACTION;
  }
  const state = actions.find((item) => item.id === id);
  if (!state) {
    return EMPTY_ACTION;
  }
  const key = extensionPageKey(pageUrl ?? "");
  const tab = key ? state.tabs.find((item) => samePage(item.url, key)) : undefined;
  const badge = tab?.badge != null ? tab.badge : state.badge;
  const icon = tab?.icon != null ? tab.icon || null : state.icon;
  return {
    icon,
    badge,
    badgeColor: tab?.badgeColor || state.badgeColor || (badge ? "#d93025" : ""),
    badgeTextColor: tab?.badgeTextColor || state.badgeTextColor || (badge ? "#ffffff" : ""),
    title: tab?.title || state.title,
  };
}

export function useExtensionAction(id: string, pageUrl?: string) {
  const actions = useExtensionActions();
  return useMemo(() => resolveExtensionAction(actions, id, pageUrl), [actions, id, pageUrl]);
}
