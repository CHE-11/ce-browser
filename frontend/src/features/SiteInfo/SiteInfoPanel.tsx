import { useEffect, useState, type ReactNode } from "react";
import { Cookie, ChevronRight, ExternalLink, Lock, ShieldAlert, ShieldBan, SlidersHorizontal, TriangleAlert } from "lucide-react";
import { Button } from "@/components/Button";
import { PopupFrame } from "@/components/PopupFrame";
import { Skeleton } from "@/components/Skeleton";
import { clearSiteCache, closePopup, createTab, openPopup, setPermission, setTabsMuted, siteInfo } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import { formatBytes } from "@/lib/format";
import { useTabs } from "@/lib/use-browser";
import { siteKey } from "@features/BuiltInExtensions/page-controls/site";
import type { PermissionGrant, SiteInfo } from "@type/browser";
import { MicrophonePanel } from "./MicrophonePanel";

type SiteInfoPanelProps = {
  tabId: number;
  initialView?: string;
};

const PERMISSION_LABEL: Record<string, string> = {
  camera: "Camera",
  microphone: "Microphone",
  location: "Location",
  notifications: "Notifications",
};

const PERMISSION_ORDER = ["microphone", "camera", "location", "notifications"];

const insetFocus = "focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-sky-500";

function permissionRank(kind: string) {
  const index = PERMISSION_ORDER.indexOf(kind);
  return index === -1 ? PERMISSION_ORDER.length : index;
}

function permissionAllowed(state: string) {
  return state === "allow" || state === "session";
}

function permissionStatus(state: string) {
  if (permissionAllowed(state)) {
    return "Allowed";
  }
  if (state === "deny") {
    return "Blocked";
  }
  return "Ask";
}

function PermissionLine({ id, label, children }: { id: string; label: string; children: ReactNode }) {
  return (
    <li id={id} className="grid h-8 grid-cols-[minmax(0,1fr)_9rem] items-center gap-3">
      <span id={`${id}-label`} className="truncate text-sm text-zinc-100">
        {label}
      </span>
      {children}
    </li>
  );
}

function PermissionChoice({
  id,
  label,
  allowed,
  denied,
  blockLabel = "Block",
  onAllow,
  onBlock,
}: {
  id: string;
  label: string;
  allowed: boolean;
  denied: boolean;
  blockLabel?: string;
  onAllow: () => void;
  onBlock: () => void;
}) {
  const choices = [
    { key: "allow", text: "Allow", on: allowed, pick: onAllow },
    { key: "block", text: blockLabel, on: denied, pick: onBlock },
  ];
  return (
    <span id={id} role="radiogroup" aria-label={label} className="grid h-7 w-full grid-cols-2 gap-0.5 rounded-md bg-zinc-800 p-0.5">
      {choices.map((choice) => (
        <button
          key={choice.key}
          id={`${id}-${choice.key}`}
          type="button"
          role="radio"
          aria-checked={choice.on}
          className={cn(insetFocus, "rounded text-xs font-medium", choice.on ? "bg-zinc-600 text-zinc-50" : "text-zinc-400 hover:text-zinc-100")}
          onClick={choice.pick}
        >
          {choice.text}
        </button>
      ))}
    </span>
  );
}

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
      <span id={`${id}-value`} className="min-w-0 wrap-break-word text-zinc-200">
        {value}
      </span>
    </div>
  );
}

/** Opened from the lock in the address bar: connection, permissions, stored data and blocked requests. */
export function SiteInfoPanel({ tabId, initialView = "" }: SiteInfoPanelProps) {
  const snapshot = useTabs();
  const tab = snapshot.windowTabs.find((item) => item.id === tabId);
  const [info, setInfo] = useState<SiteInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [cleared, setCleared] = useState(false);
  const [soundMuted, setSoundMuted] = useState(false);
  const [view, setView] = useState(initialView === "microphone" ? "microphone" : "");

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

  useEffect(() => {
    setSoundMuted(!!tab?.muted);
  }, [tab?.id, tab?.muted]);

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

  const setSound = (muted: boolean) => {
    setSoundMuted(muted);
    void setTabsMuted([tabId], muted);
  };

  const permissions = [...(info?.permissions ?? [])].sort(
    (left, right) => permissionRank(left.kind) - permissionRank(right.kind) || left.kind.localeCompare(right.kind),
  );

  const openPageControls = () => {
    if (!tab) {
      return;
    }
    const params = new URLSearchParams({ tab: String(tab.id), title: tab.title || tab.url, url: tab.url, host: siteKey(tab.url) });
    void openPopup("page-controls", { x: 0, y: 0 }, { params: params.toString() });
  };

  if (view === "microphone" && !internal) {
    const grant = info?.permissions.find((item) => item.kind === "microphone") ?? null;
    return (
      <MicrophonePanel
        host={host}
        origin={info?.origin ?? ""}
        grant={grant}
        ready={snapshot.loaded && (!!info || !!error)}
        onBack={() => setView("")}
        onGrant={(next) => {
          setInfo((current) => {
            if (!current) {
              return current;
            }
            const exists = current.permissions.some((item) => item.kind === "microphone");
            const permissions = exists
              ? current.permissions.map((item) => (item.kind === "microphone" ? next : item))
              : [...current.permissions, next];
            return { ...current, permissions };
          });
        }}
      />
    );
  }

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
            <Skeleton id="site-info-permissions-loading" className="h-8 w-full" />
          ) : (
            <ul id="site-info-permissions-list" className="flex flex-col gap-1">
              {tab ? (
                <PermissionLine id="site-info-permission-speakers" label="Speakers">
                  <PermissionChoice
                    id="site-info-permission-speakers-choice"
                    label="Speakers"
                    blockLabel="Mute"
                    allowed={!soundMuted}
                    denied={soundMuted}
                    onAllow={() => setSound(false)}
                    onBlock={() => setSound(true)}
                  />
                </PermissionLine>
              ) : null}
              {permissions.map((grant) => {
                const label = PERMISSION_LABEL[grant.kind] ?? grant.kind;
                if (grant.kind === "microphone") {
                  return (
                    <PermissionLine id="site-info-permission-microphone" key="microphone" label="Microphone">
                      <button
                        id="site-info-permission-microphone-open"
                        type="button"
                        className={cn(
                          "flex h-7 w-full items-center justify-between rounded-md bg-zinc-800 px-2 text-xs text-zinc-300 hover:bg-zinc-700 hover:text-zinc-100",
                          insetFocus,
                        )}
                        onClick={() => setView("microphone")}
                      >
                        <span id="site-info-permission-microphone-state">{permissionStatus(grant.state)}</span>
                        <ChevronRight id="site-info-permission-microphone-chevron" className="size-3.5 shrink-0 text-zinc-500" aria-hidden />
                      </button>
                    </PermissionLine>
                  );
                }
                return (
                  <PermissionLine id={`site-info-permission-${grant.kind}`} key={grant.kind} label={label}>
                    <PermissionChoice
                      id={`site-info-permission-${grant.kind}-choice`}
                      label={label}
                      allowed={permissionAllowed(grant.state)}
                      denied={grant.state === "deny"}
                      onAllow={() => updatePermission(grant, true)}
                      onBlock={() => updatePermission(grant, false)}
                    />
                  </PermissionLine>
                );
              })}
              {permissions.length === 0 ? (
                <li id="site-info-permissions-empty">
                  <p id="site-info-permissions-empty-text" className="text-xs text-zinc-500">
                    This site hasn't asked for camera, microphone, location or notifications.
                  </p>
                </li>
              ) : null}
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
