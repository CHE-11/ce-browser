import { useEffect, useState } from "react";
import { ShieldCheck } from "lucide-react";
import { Button } from "@/components/Button";
import { EmptyState } from "@/components/EmptyState";
import { SkeletonRows } from "@/components/Skeleton";
import { listPermissions, onPermissionsChanged, setPermission } from "@/lib/browser-client";
import { useAppearanceFromSettings } from "@/lib/use-browser";
import type { PermissionGrant } from "@type/browser";

const KIND_LABEL: Record<string, string> = {
  camera: "Camera",
  microphone: "Microphone",
  location: "Location",
  notifications: "Notifications",
};

function stateLabel(state: string) {
  if (state === "allow") {
    return "Allowed";
  }
  if (state === "session") {
    return "Allowed this session";
  }
  if (state === "deny") {
    return "Blocked";
  }
  return "Asked";
}

export function PermissionsPage() {
  useAppearanceFromSettings();
  const [grants, setGrants] = useState<PermissionGrant[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    document.title = "Permissions";
    void listPermissions().then(setGrants).catch((reason: unknown) => setError(String(reason)));
    const unlisten = onPermissionsChanged(setGrants);
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);

  return (
    <main id="permissions-page" className="h-full overflow-y-auto bg-zinc-950 text-zinc-100">
      <div id="permissions-body" className="mx-auto flex max-w-3xl flex-col gap-4 px-6 py-8">
        <header id="permissions-header">
          <h1 id="permissions-title" className="text-lg font-semibold">
            Site permissions
          </h1>
          <p id="permissions-hint" className="mt-1 text-sm text-zinc-500">
            Camera, microphone, location and notification requests are listed here. Revoke blocks the next request from that site.
          </p>
        </header>
        {error ? (
          <p id="permissions-error" className="text-sm text-red-300">
            {error}
          </p>
        ) : null}
        {grants === null ? (
          <SkeletonRows id="permissions-loading" rows={4} className="rounded-lg border border-zinc-800 p-4" />
        ) : grants.length === 0 ? (
          <EmptyState
            id="permissions-empty"
            icon={ShieldCheck}
            title="No sites have asked for a permission yet"
            hint="When a site asks for your camera, microphone, location or notifications, it shows up here."
            className="rounded-lg border border-dashed border-zinc-800"
          />
        ) : (
          <ul id="permissions-list" className="flex flex-col divide-y divide-zinc-800 rounded-lg border border-zinc-800">
            {grants.map((grant) => {
              const rowId = `permissions-${grant.kind}-${grant.origin.replace(/[^a-z0-9]+/gi, "-")}`;
              return (
                <li id={rowId} key={`${grant.origin}-${grant.kind}`} className="flex items-center gap-3 px-4 py-3">
                  <div id={`${rowId}-text`} className="min-w-0 flex-1">
                    <div id={`${rowId}-origin`} className="truncate text-sm">
                      {grant.origin}
                    </div>
                    <div id={`${rowId}-kind`} className="text-xs text-zinc-500">
                      {KIND_LABEL[grant.kind] ?? grant.kind} · {stateLabel(grant.state)}
                    </div>
                  </div>
                  <Button
                    id={`${rowId}-revoke`}
                    size="sm"
                    variant="outline"
                    disabled={grant.state === "deny"}
                    aria-label={`Revoke ${KIND_LABEL[grant.kind] ?? grant.kind} for ${grant.origin}`}
                    onClick={() => {
                      void setPermission(grant.origin, grant.kind, false).catch((reason: unknown) => setError(String(reason)));
                    }}
                  >
                    Revoke
                  </Button>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </main>
  );
}
