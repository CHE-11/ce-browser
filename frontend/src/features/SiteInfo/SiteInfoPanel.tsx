import { useEffect, useState, type ReactNode } from "react";
import { Cookie, ExternalLink, Lock, ShieldAlert, ShieldBan, SlidersHorizontal, TriangleAlert } from "lucide-react";
import { Button } from "@/components/Button";
import { PopupFrame } from "@/components/PopupFrame";
import { Skeleton } from "@/components/Skeleton";
import { clearSiteCache, closePopup, createTab, openPopup, setPermission, siteInfo } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import { formatBytes } from "@/lib/format";
import { useTabs } from "@/lib/use-browser";
import { siteKey } from "@features/BuiltInExtensions/page-controls/site";
import type { PermissionGrant, SiteInfo } from "@type/browser";

type SiteInfoPanelProps = {
  tabId: number;
};

const PERMISSION_LABEL: Record<string, string> = {
  camera: "Camera",
  microphone: "Microphone",
  location: "Location",
  notifications: "Notifications",
};

function dateOf(seconds: number) {
  if (!seconds) {
    return "";
  }
  return new Date(seconds * 1000).toLocaleDateString(undefined, { dateStyle: "medium" });
}

function Section({ id, title, children }: { id: string; title: string; children: ReactNode }) {
  return (
    <section id={id} className="border-t border-zinc-800 px-4 py-3">
      <h3 id={`${id}-title`} className="mb-2 text-[11px] font-medium uppercase tracking-wide text-zinc-500">
        {title}
      </h3>
      {children}
    </section>
  );
}

function Fact({ id, label, value }: { id: string; label: string; value: string }) {
  if (!value) {
    return null;
  }
  return (
    <div id={id} className="flex gap-3 py-0.5 text-xs">
      <span id={`${id}-label`} className="w-20 shrink-0 text-zinc-500">
        {label}
      </span>
      <span id={`${id}-value`} className="min-w-0 break-words text-zinc-200">
        {value}
      </span>
    </div>
  );
}

/** Opened from the lock in the address bar: connection, permissions, stored data and blocked requests. */
export function SiteInfoPanel({ tabId }: SiteInfoPanelProps) {
  const snapshot = useTabs();
  const tab = snapshot.windowTabs.find((item) => item.id === tabId);
  const [info, setInfo] = useState<SiteInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [cleared, setCleared] = useState(false);

  useEffect(() => {
    let cancelled = false;
    siteInfo(tabId)
      .then((next) => {
        if (!cancelled) {
          setInfo(next);
        }
      })
      .catch((reason: unknown) => {
        if (!cancelled) {
          setError(String(reason));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [tabId]);

  const host = info?.host || (tab ? siteKey(tab.url) : "");
  const internal = !!tab && tab.url.startsWith("ce://");
  const secure = info?.security === "secure" && !info.certificateException;
  const status = !info
    ? null
    : internal
      ? { icon: Lock, tone: "text-emerald-400", text: "This is a ce-browser page." }
      : info.certificateException
        ? { icon: ShieldAlert, tone: "text-red-400", text: "You chose to continue past a certificate error on this site." }
        : secure
          ? { icon: Lock, tone: "text-emerald-400", text: "Connection is secure. What you send to this site stays private." }
          : info.url.startsWith("http://")
            ? { icon: TriangleAlert, tone: "text-amber-400", text: "Connection is not secure. Don't enter passwords or payment details." }
            : { icon: TriangleAlert, tone: "text-zinc-400", text: "The connection details couldn't be read." };

  const updatePermission = (grant: PermissionGrant, allow: boolean) => {
    if (!info) {
      return;
    }
    const state = allow ? "allow" : "deny";
    setInfo({ ...info, permissions: info.permissions.map((item) => (item === grant ? { ...item, state } : item)) });
    void setPermission(grant.origin, grant.kind, allow);
  };

  const openPageControls = () => {
    if (!tab) {
      return;
    }
    const params = new URLSearchParams({ tab: String(tab.id), title: tab.title || tab.url, url: tab.url, host: siteKey(tab.url) });
    void openPopup("page-controls", { x: 0, y: 0 }, { params: params.toString() });
  };

  return (
    <PopupFrame id="site-info" width={360} ready={snapshot.loaded}>
      <header id="site-info-header" className="flex flex-col gap-1.5 px-4 pb-3 pt-4">
        <h2 id="site-info-host" className="truncate text-sm font-semibold text-zinc-100" title={info?.url}>
          {host || tab?.title || "This page"}
        </h2>
        {status ? (
          <p id="site-info-status" className={cn("flex items-start gap-2 text-xs leading-relaxed", status.tone)}>
            <status.icon id="site-info-status-icon" className="mt-0.5 size-3.5 shrink-0" aria-hidden />
            <span id="site-info-status-text">{status.text}</span>
          </p>
        ) : error ? (
          <p id="site-info-error" className="text-xs text-red-300">
            {error}
          </p>
        ) : (
          <Skeleton id="site-info-status-loading" className="h-3.5 w-3/4" />
        )}
      </header>

      {info?.certificate ? (
        <Section id="site-info-certificate" title="Certificate">
          <Fact id="site-info-cert-subject" label="Issued to" value={info.certificate.subject} />
          <Fact id="site-info-cert-issuer" label="Issued by" value={info.certificate.issuer} />
          <Fact
            id="site-info-cert-valid"
            label="Valid"
            value={[dateOf(info.certificate.validFrom), dateOf(info.certificate.validTo)].filter(Boolean).join(" to ")}
          />
          <Fact
            id="site-info-cert-protocol"
            label="Connection"
            value={[info.certificate.protocol, info.certificate.keyExchange, info.certificate.cipher].filter(Boolean).join(", ")}
          />
        </Section>
      ) : null}

      {!internal ? (
        <Section id="site-info-permissions" title="Permissions">
          {!info ? (
            <Skeleton id="site-info-permissions-loading" className="h-6 w-full" />
          ) : info.permissions.length === 0 ? (
            <p id="site-info-permissions-empty" className="text-xs text-zinc-500">
              This site hasn't asked for camera, microphone, location or notifications.
            </p>
          ) : (
            <ul id="site-info-permissions-list" className="flex flex-col gap-1">
              {info.permissions.map((grant) => {
                const allowed = grant.state === "allow";
                const denied = grant.state === "deny";
                return (
                  <li id={`site-info-permission-${grant.kind}`} key={grant.kind} className="flex items-center justify-between gap-2 text-sm">
                    <span id={`site-info-permission-${grant.kind}-label`}>{PERMISSION_LABEL[grant.kind] ?? grant.kind}</span>
                    <span id={`site-info-permission-${grant.kind}-choice`} role="radiogroup" aria-label={PERMISSION_LABEL[grant.kind] ?? grant.kind} className="flex rounded-md border border-zinc-700 p-0.5">
                      {[
                        { allow: true, label: "Allow", on: allowed },
                        { allow: false, label: "Block", on: denied },
                      ].map((choice) => (
                        <button
                          key={choice.label}
                          id={`site-info-permission-${grant.kind}-${choice.label.toLowerCase()}`}
                          type="button"
                          role="radio"
                          aria-checked={choice.on}
                          className={cn("rounded px-2 py-0.5 text-xs text-zinc-400 hover:text-zinc-100", focusRing, choice.on && "bg-zinc-700 text-zinc-100")}
                          onClick={() => updatePermission(grant, choice.allow)}
                        >
                          {choice.label}
                        </button>
                      ))}
                    </span>
                  </li>
                );
              })}
            </ul>
          )}
        </Section>
      ) : null}

      {!internal ? (
        <Section id="site-info-data" title="Cookies and site data">
          {!info ? (
            <Skeleton id="site-info-data-loading" className="h-4 w-2/3" />
          ) : (
            <div id="site-info-data-body" className="flex flex-col gap-2">
              <p id="site-info-data-summary" className="flex items-center gap-2 text-sm text-zinc-200">
                <Cookie id="site-info-data-icon" className="size-4 shrink-0 text-zinc-400" aria-hidden />
                {info.cookies === null
                  ? "Load the page to read its cookies."
                  : `${info.cookies} ${info.cookies === 1 ? "cookie" : "cookies"}${info.storageBytes !== null ? `, ${formatBytes(info.storageBytes)} stored` : ""}`}
              </p>
              <div id="site-info-data-actions" className="flex flex-wrap gap-2">
                <Button
                  id="site-info-data-manage"
                  size="sm"
                  onClick={() => void createTab(`ce://storage?tab=${tabId}`).finally(() => closePopup())}
                >
                  Manage
                </Button>
                <Button
                  id="site-info-data-clear-cache"
                  size="sm"
                  variant="ghost"
                  disabled={cleared}
                  onClick={() => void clearSiteCache(tabId).then(() => setCleared(true))}
                >
                  {cleared ? "Cache cleared" : "Clear cached files"}
                </Button>
              </div>
            </div>
          )}
        </Section>
      ) : null}

      {info && !internal ? (
        <Section id="site-info-ads" title="Ad blocker">
          <p id="site-info-ads-count" className="flex items-center gap-2 text-sm text-zinc-200">
            <ShieldBan id="site-info-ads-icon" className="size-4 shrink-0 text-zinc-400" aria-hidden />
            {info.adsBlocked === 0
              ? "Nothing blocked on this page."
              : `${info.adsBlocked} ${info.adsBlocked === 1 ? "request" : "requests"} blocked on this page`}
          </p>
        </Section>
      ) : null}

      {!internal && tab ? (
        <div id="site-info-links" className="flex flex-col gap-0.5 border-t border-zinc-800 p-1.5">
          <button
            id="site-info-page-controls"
            type="button"
            className={cn("flex h-9 items-center gap-3 rounded-lg px-2.5 text-left text-sm hover:bg-zinc-800", focusRing)}
            onClick={openPageControls}
          >
            <SlidersHorizontal id="site-info-page-controls-icon" className="size-4 text-zinc-400" aria-hidden />
            Page controls for this site
          </button>
          <button
            id="site-info-all-permissions"
            type="button"
            className={cn("flex h-9 items-center gap-3 rounded-lg px-2.5 text-left text-sm hover:bg-zinc-800", focusRing)}
            onClick={() => void createTab("ce://permissions").finally(() => closePopup())}
          >
            <ExternalLink id="site-info-all-permissions-icon" className="size-4 text-zinc-400" aria-hidden />
            All site permissions
          </button>
        </div>
      ) : null}
    </PopupFrame>
  );
}
