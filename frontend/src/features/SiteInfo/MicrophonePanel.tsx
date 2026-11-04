import { useEffect, useRef, useState } from "react";
import { ArrowLeft, Check, ExternalLink, Mic, Settings, X } from "lucide-react";
import { PopupFrame } from "@/components/PopupFrame";
import { Toggle } from "@/components/Toggle";
import {
  closePopup,
  createTab,
  listMicrophones,
  onMicrophoneLevel,
  setMicrophoneDevice,
  setPermission,
  unwatchMicrophone,
  watchMicrophone,
} from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import type { MicrophoneDevice, PermissionGrant } from "@type/browser";

type MicrophonePanelProps = {
  host: string;
  origin: string;
  grant: PermissionGrant | null;
  ready: boolean;
  onBack: () => void;
  onGrant: (grant: PermissionGrant) => void;
};

function statusText(state: string | undefined) {
  if (state === "allow" || state === "session") {
    return "Allowed";
  }
  if (state === "deny") {
    return "Blocked";
  }
  return "Ask";
}

/** Chrome-style microphone page opened from the address bar or site info. */
export function MicrophonePanel({ host, origin, grant, ready, onBack, onGrant }: MicrophonePanelProps) {
  const [devices, setDevices] = useState<MicrophoneDevice[] | null>(null);
  const [open, setOpen] = useState(false);
  const [level, setLevel] = useState(0);
  const watchEpoch = useRef(0);
  const allowed = grant?.state === "allow" || grant?.state === "session";
  const persist = grant?.state === "allow";
  const selected =
    devices?.find((device) => device.id === (grant?.deviceId || "")) ??
    (grant?.deviceId ? undefined : devices?.find((device) => device.isDefault));
  const deviceName = selected?.label || grant?.deviceLabel || "Default";
  const count = devices?.length ?? 0;

  useEffect(() => {
    let cancelled = false;
    listMicrophones()
      .then((next) => {
        if (!cancelled) {
          setDevices(next);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setDevices([]);
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!allowed) {
      setLevel(0);
      return;
    }
    const epoch = ++watchEpoch.current;
    void watchMicrophone(grant?.deviceId ?? "", epoch);
    const unlisten = onMicrophoneLevel((event) => {
      setLevel(Math.max(0, Math.min(1, event.level)));
    });
    return () => {
      void unwatchMicrophone(epoch);
      void unlisten.then((stop) => stop());
    };
  }, [allowed, grant?.deviceId]);

  const apply = (allow: boolean, everyVisit: boolean) => {
    if (!origin || !grant) {
      return;
    }
    const state = !allow ? "deny" : everyVisit ? "allow" : "session";
    onGrant({ ...grant, state });
    void setPermission(origin, "microphone", allow, everyVisit);
  };

  const pick = (device: MicrophoneDevice) => {
    if (!origin || !grant) {
      return;
    }
    onGrant({ ...grant, deviceId: device.id, deviceLabel: device.label });
    void setMicrophoneDevice(origin, device.id, device.label);
    setOpen(false);
  };

  const percent = Math.round(level * 100);

  return (
    <PopupFrame id="microphone-panel" width={360} ready={ready} role="dialog" labelledBy="microphone-title">
      <header id="microphone-header" className="flex items-start gap-1 px-2 pb-2 pt-2">
        <button
          id="microphone-back"
          type="button"
          aria-label="Back"
          className={cn("inline-flex size-8 shrink-0 items-center justify-center rounded-full text-zinc-300 hover:bg-zinc-800", focusRing)}
          onClick={onBack}
        >
          <ArrowLeft id="microphone-back-icon" className="size-4" aria-hidden />
        </button>
        <div id="microphone-heading" className="min-w-0 flex-1 pt-0.5">
          <h2 id="microphone-title" className="text-sm font-semibold text-zinc-100">
            Microphone
          </h2>
          <p id="microphone-host" className="truncate text-xs text-zinc-400" title={host}>
            {host || "This page"}
          </p>
        </div>
        <button
          id="microphone-close"
          type="button"
          aria-label="Close"
          className={cn("inline-flex size-8 shrink-0 items-center justify-center rounded-full text-zinc-300 hover:bg-zinc-800", focusRing)}
          onClick={() => void closePopup()}
        >
          <X id="microphone-close-icon" className="size-4" aria-hidden />
        </button>
      </header>

      <div id="microphone-permission" className="flex items-center gap-3 px-4 py-2">
        <Mic id="microphone-permission-icon" className="size-5 shrink-0 text-zinc-300" aria-hidden />
        <div id="microphone-permission-text" className="min-w-0 flex-1">
          <div id="microphone-permission-count" className="text-sm text-zinc-100">
            {devices === null ? "Microphones" : `Microphones (${count})`}
          </div>
          <div id="microphone-permission-state" className="text-xs text-zinc-400">
            {statusText(grant?.state)}
          </div>
        </div>
        <Toggle
          id="microphone-allow"
          checked={allowed}
          disabled={!grant || !origin}
          aria-label={allowed ? "Block microphone" : "Allow microphone"}
          onChange={(checked) => apply(checked, checked ? persist : false)}
        />
      </div>

      <label
        id="microphone-persist-label"
        className={cn("mx-4 mb-3 flex items-center gap-2 text-sm text-zinc-200", !allowed && "opacity-40")}
      >
        <input
          id="microphone-persist"
          type="checkbox"
          className="size-3.5 accent-sky-500"
          checked={persist}
          disabled={!allowed}
          onChange={(event) => apply(true, event.target.checked)}
        />
        Allow on every visit
      </label>

      {allowed ? (
        <div id="microphone-meter-block" className="px-4 pb-3">
          <div
            id="microphone-level"
            role="progressbar"
            aria-label="Microphone level"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={percent}
            className="relative h-6"
          >
            <div id="microphone-level-track" className="absolute inset-x-0 top-1/2 h-1 -translate-y-1/2 rounded-full bg-zinc-700" />
            <div
              id="microphone-level-fill"
              className="absolute top-1/2 left-0 h-1 -translate-y-1/2 rounded-full bg-sky-500"
              style={{ width: `${percent}%` }}
            />
            <div
              id="microphone-level-thumb"
              className="absolute top-1/2 size-3.5 -translate-y-1/2 rounded-full bg-sky-400"
              style={{ left: `clamp(0px, calc(${percent}% - 7px), calc(100% - 14px))` }}
            />
          </div>
          <button
            id="microphone-device"
            type="button"
            aria-haspopup="listbox"
            aria-expanded={open}
            className={cn("mt-1 flex w-full items-center rounded-md px-1 py-1.5 text-left text-sm text-zinc-100 hover:bg-zinc-800", focusRing)}
            onClick={() => setOpen((value) => !value)}
          >
            <span id="microphone-device-name" className="truncate">
              {deviceName}
            </span>
          </button>
          {open ? (
            <ul id="microphone-device-list" role="listbox" aria-label="Microphones" className="mb-1 flex flex-col">
              {devices === null ? (
                <li id="microphone-device-loading" className="px-2 py-1.5 text-xs text-zinc-500">
                  Looking for microphones…
                </li>
              ) : devices.length === 0 ? (
                <li id="microphone-device-empty" className="px-2 py-1.5 text-xs text-zinc-500">
                  No microphones found
                </li>
              ) : (
                devices.map((device, index) => {
                  const on = selected?.id === device.id;
                  const rowId = `microphone-device-${index}`;
                  return (
                    <li id={rowId} key={device.id}>
                      <button
                        id={`${rowId}-pick`}
                        type="button"
                        role="option"
                        aria-selected={on}
                        className={cn("flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-zinc-800", focusRing)}
                        onClick={() => pick(device)}
                      >
                        {on ? (
                          <Check id={`${rowId}-check`} className="size-4 shrink-0 text-sky-400" aria-hidden />
                        ) : (
                          <span id={`${rowId}-blank`} className="size-4 shrink-0" aria-hidden />
                        )}
                        <span id={`${rowId}-label`} className="truncate">
                          {device.label}
                        </span>
                      </button>
                    </li>
                  );
                })
              )}
            </ul>
          ) : null}
        </div>
      ) : null}

      <button
        id="microphone-manage"
        type="button"
        className={cn("flex h-11 w-full items-center gap-3 border-t border-zinc-800 px-4 text-left text-sm text-zinc-100 hover:bg-zinc-800", focusRing)}
        onClick={() => void createTab("ce://permissions").finally(() => closePopup())}
      >
        <Settings id="microphone-manage-icon" className="size-4 text-zinc-400" aria-hidden />
        <span id="microphone-manage-label" className="flex-1">
          Manage
        </span>
        <ExternalLink id="microphone-manage-external" className="size-3.5 text-zinc-500" aria-hidden />
      </button>
    </PopupFrame>
  );
}
