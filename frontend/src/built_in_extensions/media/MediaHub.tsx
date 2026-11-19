import { useEffect, useState, type ReactNode } from "react";
import { Clapperboard, Pause, Play, Volume2, VolumeX } from "lucide-react";
import { FavIcon } from "@/components/FavIcon";
import { PopupFrame } from "@/components/PopupFrame";
import { activateTab, closePopup, controlMedia, listMedia, setAllMediaMuted } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import { hostOf } from "@/lib/format";
import type { MediaElement } from "@type/browser";

export function MediaHub() {
  const [items, setItems] = useState<MediaElement[]>([]);
  const [error, setError] = useState("");

  useEffect(() => {
    let gone = false;
    const load = () => {
      void listMedia()
        .then((next) => {
          if (!gone) {
            setItems(next);
            setError("");
          }
        })
        .catch((reason: unknown) => {
          if (!gone) {
            setError(reason instanceof Error ? reason.message : "Couldn't list media.");
          }
        });
    };
    load();
    const timer = window.setInterval(load, 1000);
    return () => {
      gone = true;
      window.clearInterval(timer);
    };
  }, []);

  const playing = items.filter((item) => !item.paused && !item.ended).length;
  const allMuted = items.length > 0 && items.every((item) => item.muted);
  const allPaused = items.length > 0 && playing === 0;

  const muteAll = () => {
    const next = !allMuted;
    setItems((current) => current.map((item) => ({ ...item, muted: next })));
    void setAllMediaMuted(next).catch((reason: unknown) => {
      setError(reason instanceof Error ? reason.message : "Couldn't change the volume.");
    });
  };

  const pauseAll = () => {
    const nextPaused = !allPaused;
    setItems((current) => current.map((item) => ({ ...item, paused: nextPaused, ended: nextPaused ? item.ended : false })));
    void Promise.all(
      items
        .filter((item) => (nextPaused ? !item.paused && !item.ended : true))
        .map((item) => controlMedia(item.tabId, item.index, nextPaused ? "pause" : "play")),
    ).catch((reason: unknown) => {
      setError(reason instanceof Error ? reason.message : "Couldn't change playback.");
    });
  };

  return (
    <PopupFrame id="media-hub" width={480} maxHeight={560}>
      <div id="media-hub-header" className="flex items-center gap-3 px-4 pb-2 pt-3">
        <span
          id="media-hub-header-icon"
          className="inline-flex size-8 shrink-0 items-center justify-center rounded-lg bg-sky-500/15 text-sky-300"
        >
          <Clapperboard id="media-hub-header-icon-svg" className="size-4" aria-hidden />
        </span>
        <div id="media-hub-header-text" className="min-w-0 flex-1">
          <h2 id="media-hub-title" className="text-sm font-semibold">
            Media
          </h2>
          <p id="media-hub-subtitle" className="truncate text-xs text-zinc-500">
            {items.length === 0
              ? "Audio and video in open tabs"
              : `${items.length} ${items.length === 1 ? "item" : "items"}${playing > 0 ? `, ${playing} playing` : ""}`}
          </p>
        </div>
        <div id="media-hub-header-actions" className="flex shrink-0 items-center gap-1">
          <HeaderButton
            id="media-hub-pause-all"
            label={allPaused ? "Play all" : "Pause all"}
            disabled={items.length === 0}
            pressed={allPaused}
            onClick={pauseAll}
            icon={
              allPaused ? (
                <Play id="media-hub-pause-all-icon" className="size-3.5" aria-hidden />
              ) : (
                <Pause id="media-hub-pause-all-icon" className="size-3.5" aria-hidden />
              )
            }
          />
          <HeaderButton
            id="media-hub-mute-all"
            label={allMuted ? "Unmute all" : "Mute all"}
            disabled={items.length === 0}
            pressed={allMuted}
            onClick={muteAll}
            icon={
              allMuted ? (
                <VolumeX id="media-hub-mute-all-icon" className="size-3.5" aria-hidden />
              ) : (
                <Volume2 id="media-hub-mute-all-icon" className="size-3.5" aria-hidden />
              )
            }
          />
        </div>
      </div>
      {error ? (
        <p id="media-hub-error" className="px-4 pb-3 text-xs text-amber-300">
          {error}
        </p>
      ) : null}
      {items.length === 0 ? (
        <p id="media-hub-empty" className="px-4 pb-4 text-sm text-zinc-500">
          Nothing is playing.
        </p>
      ) : (
        <ul id="media-hub-list" className="flex flex-col gap-2 px-3 pb-3">
          {items.map((item) => (
            <MediaRow key={`${item.tabId}-${item.index}`} item={item} items={items} />
          ))}
        </ul>
      )}
    </PopupFrame>
  );
}

function HeaderButton({
  id,
  label,
  disabled,
  pressed,
  onClick,
  icon,
}: {
  id: string;
  label: string;
  disabled: boolean;
  pressed: boolean;
  onClick: () => void;
  icon: ReactNode;
}) {
  return (
    <button
      id={id}
      type="button"
      disabled={disabled}
      aria-pressed={pressed}
      title={label}
      className={cn(
        "inline-flex h-8 shrink-0 items-center gap-1.5 rounded-md px-2 text-xs text-zinc-200 hover:bg-zinc-800 disabled:opacity-40",
        focusRing,
      )}
      onClick={onClick}
    >
      {icon}
      <span id={`${id}-label`}>{label}</span>
    </button>
  );
}

function MediaRow({ item, items }: { item: MediaElement; items: MediaElement[] }) {
  const id = `media-hub-row-${item.tabId}-${item.index}`;
  const site = item.site || hostOf(item.tabUrl);
  const source = sourceHost(item.label, site);
  const title = rowTitle(item, items);
  const meta = detailLine(item, site, source);

  return (
    <li id={id} className="flex gap-2 rounded-lg bg-zinc-800 p-2">
      <Poster id={id} item={item} />
      <div id={`${id}-body`} className="flex min-w-0 flex-1 flex-col gap-1.5">
        <button
          id={`${id}-open`}
          type="button"
          title={item.tabUrl}
          className={cn("flex min-w-0 items-center gap-1.5 text-left", focusRing)}
          onClick={() => {
            void activateTab(item.tabId).finally(() => closePopup());
          }}
        >
          <FavIcon id={`${id}-favicon`} url={item.tabUrl} src={item.favicon} />
          <span id={`${id}-title`} className="truncate text-sm text-zinc-100">
            {title}
          </span>
        </button>
        <p id={`${id}-meta`} className="truncate text-[11px] text-zinc-500" title={meta}>
          {meta}
        </p>
        <div id={`${id}-controls`} className="flex items-center gap-2">
          <button
            id={`${id}-toggle`}
            type="button"
            aria-label={item.paused || item.ended ? "Play" : "Pause"}
            className={cn("inline-flex size-7 shrink-0 items-center justify-center rounded-md bg-zinc-700 hover:bg-zinc-600", focusRing)}
            onClick={() => void controlMedia(item.tabId, item.index, item.paused || item.ended ? "play" : "pause")}
          >
            {item.paused || item.ended ? (
              <Play id={`${id}-play`} className="size-3.5" aria-hidden />
            ) : (
              <Pause id={`${id}-pause`} className="size-3.5" aria-hidden />
            )}
          </button>
          <input
            id={`${id}-seek`}
            type="range"
            min={0}
            max={item.duration || 0}
            step={0.1}
            aria-label="Position"
            value={Math.min(item.current, item.duration || item.current)}
            className="min-w-0 flex-1"
            onChange={(event) => void controlMedia(item.tabId, item.index, "seek", Number(event.target.value))}
          />
          <span id={`${id}-time`} className="shrink-0 text-[11px] tabular-nums text-zinc-400">
            {clock(item.current)}
            <span id={`${id}-time-split`} className="text-zinc-600">
              {" / "}
            </span>
            {item.duration > 0 ? clock(item.duration) : "--:--"}
          </span>
          <button
            id={`${id}-mute`}
            type="button"
            aria-pressed={item.muted}
            aria-label={item.muted ? "Unmute" : "Mute"}
            title={item.muted ? "Unmute" : `Mute, volume ${Math.round(item.volume * 100)}%`}
            className={cn("inline-flex size-7 shrink-0 items-center justify-center rounded-md text-zinc-300 hover:bg-zinc-700", focusRing)}
            onClick={() => void controlMedia(item.tabId, item.index, item.muted ? "unmute" : "mute")}
          >
            {item.muted ? <VolumeX id={`${id}-mute-icon`} className="size-3.5" aria-hidden /> : <Volume2 id={`${id}-mute-icon`} className="size-3.5" aria-hidden />}
          </button>
        </div>
      </div>
    </li>
  );
}

function Poster({ id, item }: { id: string; item: MediaElement }) {
  const [failed, setFailed] = useState(false);
  const show = item.poster.startsWith("http://") || item.poster.startsWith("https://");
  if (!show || failed) {
    return (
      <span
        id={`${id}-poster`}
        className="flex h-14 w-16 shrink-0 items-center justify-center rounded-md bg-zinc-900 text-zinc-500"
      >
        <Clapperboard id={`${id}-poster-icon`} className="size-4" aria-hidden />
      </span>
    );
  }
  return (
    <img
      id={`${id}-poster`}
      src={item.poster}
      alt=""
      className="h-14 w-16 shrink-0 rounded-md bg-zinc-900 object-cover"
      onError={() => setFailed(true)}
    />
  );
}

function rowTitle(item: MediaElement, items: MediaElement[]) {
  const name = item.tabTitle || item.title || item.site || "Untitled";
  const same = items.filter((other) => other.tabId === item.tabId && (other.tabTitle || other.title) === (item.tabTitle || item.title));
  if (same.length < 2) {
    return name;
  }
  const kind = item.kind === "audio" ? "Audio" : "Video";
  return `${name} · ${kind} ${item.index + 1}`;
}

function detailLine(item: MediaElement, site: string, source: string) {
  const parts = [site, item.kind === "audio" ? "Audio" : "Video"];
  if (item.width > 0 && item.height > 0) {
    parts.push(`${item.width}×${item.height}`);
  }
  if (item.ended) {
    parts.push("Ended");
  } else if (item.paused) {
    parts.push("Paused");
  }
  if (item.muted) {
    parts.push("Muted");
  } else if (item.volume < 0.99) {
    parts.push(`Volume ${Math.round(item.volume * 100)}%`);
  }
  if (item.playbackRate > 0 && Math.abs(item.playbackRate - 1) > 0.01) {
    parts.push(`${item.playbackRate}×`);
  }
  if (source) {
    parts.push(source);
  }
  return parts.filter(Boolean).join(" · ");
}

function sourceHost(label: string, site: string) {
  if (!label.startsWith("http://") && !label.startsWith("https://")) {
    return "";
  }
  const host = hostOf(label).replace(/^www\./, "");
  if (!host || host === site || host.endsWith(`.${site}`) || site.endsWith(`.${host}`)) {
    return "";
  }
  return host;
}

function clock(seconds: number) {
  if (!Number.isFinite(seconds) || seconds < 0) {
    return "0:00";
  }
  const total = Math.floor(seconds);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const rest = total % 60;
  const pad = (value: number) => String(value).padStart(2, "0");
  if (hours > 0) {
    return `${hours}:${pad(minutes)}:${pad(rest)}`;
  }
  return `${minutes}:${pad(rest)}`;
}
