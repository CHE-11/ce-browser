import { useEffect, useState, type CSSProperties } from "react";
import { ArrowLeft, Bug, Clock, Globe, RotateCw, Search, ShieldAlert, Unplug, WifiOff } from "lucide-react";
import type { TabFailure, TabInfo } from "@type/browser";
import { allowCertificate, goBack, navigate, reload } from "../lib/browser-client";
import { cn } from "../lib/cn";
import { hostOf } from "../lib/format";
import { Button } from "./Button";

type PageFailureProps = {
  tab: TabInfo;
  failure: TabFailure;
  className?: string;
  style?: CSSProperties;
};

function useOnline() {
  const [online, setOnline] = useState(() => navigator.onLine);
  useEffect(() => {
    const update = () => setOnline(navigator.onLine);
    window.addEventListener("online", update);
    window.addEventListener("offline", update);
    return () => {
      window.removeEventListener("online", update);
      window.removeEventListener("offline", update);
    };
  }, []);
  return online;
}

/** Words to search for when an address can't be reached: the host and the path. */
function searchWords(url: string) {
  try {
    const parsed = new URL(url);
    const path = decodeURIComponent(parsed.pathname).split(/[/\-_.]+/).filter(Boolean).join(" ");
    return `${parsed.hostname.replace(/^www\./, "")} ${path}`.trim();
  } catch {
    return url;
  }
}

function describe(failure: TabFailure, online: boolean) {
  const host = hostOf(failure.url);
  if (failure.kind === "crashed") {
    return {
      icon: Bug,
      title: "This tab crashed",
      body: failure.detail || "The page stopped unexpectedly. Reloading usually brings it back.",
    };
  }
  if (failure.kind === "certificate") {
    return {
      icon: ShieldAlert,
      title: "This connection isn't private",
      body: `${host} sent a certificate this browser can't trust. Someone could be reading or changing what you send.`,
    };
  }
  if (!online || failure.kind === "offline") {
    return {
      icon: WifiOff,
      title: online ? "The connection dropped" : "You're offline",
      body: online
        ? `The connection to ${host} was interrupted.`
        : "Check your network. This page reloads by itself when the connection comes back.",
    };
  }
  if (failure.kind === "dns") {
    return {
      icon: Globe,
      title: "This site can't be found",
      body: `No address was found for ${host}. Check the spelling, or search for it instead.`,
    };
  }
  if (failure.kind === "timeout") {
    return { icon: Clock, title: "The site took too long to answer", body: `${host} did not respond in time.` };
  }
  if (failure.kind === "refused") {
    return { icon: Unplug, title: "The site refused to connect", body: `${host} is up but did not accept the connection.` };
  }
  return { icon: Globe, title: "This page didn't load", body: `Something went wrong while loading ${host}.` };
}

/** Shown in the page area, behind the hidden page view, when a tab crashed or failed to load. */
export function PageFailure({ tab, failure, className, style }: PageFailureProps) {
  const online = useOnline();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const { icon: Icon, title, body } = describe(failure, online);
  const crashed = failure.kind === "crashed";
  const certificate = failure.kind === "certificate";
  const web = failure.url.startsWith("http://") || failure.url.startsWith("https://");

  const run = (action: () => Promise<unknown>) => {
    setBusy(true);
    setError(null);
    action()
      .catch((reason: unknown) => setError(String(reason)))
      .finally(() => setBusy(false));
  };

  const retry = () => run(() => (crashed ? reload(tab.id) : navigate(tab.id, failure.url)));

  useEffect(() => {
    if (failure.kind !== "offline" && failure.kind !== "dns") {
      return;
    }
    const onOnline = () => void navigate(tab.id, failure.url);
    window.addEventListener("online", onOnline);
    return () => window.removeEventListener("online", onOnline);
  }, [failure.kind, failure.url, tab.id]);

  return (
    <div
      id={`page-failure-${tab.id}`}
      role="alert"
      className={cn("flex items-center justify-center overflow-y-auto bg-zinc-950 p-8", className)}
      style={style}
    >
      <div id={`page-failure-${tab.id}-card`} className="flex w-full max-w-lg flex-col gap-4">
        <Icon id={`page-failure-${tab.id}-icon`} className={cn("size-10", certificate ? "text-red-400" : "text-zinc-500")} aria-hidden />
        <div id={`page-failure-${tab.id}-text`} className="flex flex-col gap-2">
          <h1 id={`page-failure-${tab.id}-title`} className="text-xl font-semibold text-zinc-100">
            {title}
          </h1>
          <p id={`page-failure-${tab.id}-body`} className="text-sm leading-relaxed text-zinc-400">
            {body}
          </p>
          {certificate && failure.detail ? (
            <p id={`page-failure-${tab.id}-detail`} className="text-xs leading-relaxed text-zinc-500">
              {failure.detail}
            </p>
          ) : null}
          <p id={`page-failure-${tab.id}-code`} className="break-all font-mono text-[11px] text-zinc-600">
            {failure.code}
            {web ? ` · ${failure.url}` : ""}
          </p>
        </div>
        <div id={`page-failure-${tab.id}-actions`} className="flex flex-wrap gap-2">
          {certificate ? (
            <>
              <Button
                id={`page-failure-${tab.id}-back`}
                variant="primary"
                disabled={busy}
                onClick={() => run(async () => (tab.canGoBack ? goBack(tab.id) : navigate(tab.id, "ce://newtab")))}
              >
                <ArrowLeft id={`page-failure-${tab.id}-back-icon`} className="size-4" aria-hidden />
                Go back to safety
              </Button>
              <Button id={`page-failure-${tab.id}-proceed`} variant="ghost" disabled={busy} onClick={() => run(() => allowCertificate(tab.id))}>
                Continue to {hostOf(failure.url)} anyway
              </Button>
            </>
          ) : (
            <>
              <Button id={`page-failure-${tab.id}-retry`} variant="primary" disabled={busy} onClick={retry}>
                <RotateCw id={`page-failure-${tab.id}-retry-icon`} className="size-4" aria-hidden />
                {crashed ? "Reload" : "Retry"}
              </Button>
              {!crashed && web ? (
                <>
                  <Button
                    id={`page-failure-${tab.id}-search`}
                    disabled={busy}
                    onClick={() => run(() => navigate(tab.id, `?${searchWords(failure.url)}`))}
                  >
                    <Search id={`page-failure-${tab.id}-search-icon`} className="size-4" aria-hidden />
                    Search instead
                  </Button>
                </>
              ) : null}
            </>
          )}
        </div>
        {error ? (
          <p id={`page-failure-${tab.id}-error`} className="text-xs text-red-300">
            {error}
          </p>
        ) : null}
      </div>
    </div>
  );
}
