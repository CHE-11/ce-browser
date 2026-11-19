import { useCallback, useEffect, useRef, useState } from "react";
import { Clapperboard, Volume2, VolumeX } from "lucide-react";
import { listMedia, setAllMediaMuted } from "@/lib/browser-client";
import { focusRing } from "@/lib/focus-ring";
import { cn } from "@/lib/cn";
import type { MediaElement } from "@type/browser";
import { ToolButton } from "../ToolButton";

function useMediaItems() {
  const [items, setItems] = useState<MediaElement[]>([]);
  const ticket = useRef(0);

  const refresh = useCallback(() => {
    const id = ++ticket.current;
    return listMedia().then((next) => {
      if (id === ticket.current) {
        setItems(next);
      }
      return next;
    });
  }, []);

  useEffect(() => {
    const load = () => {
      if (!document.hidden) {
        void refresh().catch(() => undefined);
      }
    };
    load();
    const timer = window.setInterval(load, 2000);
    document.addEventListener("visibilitychange", load);
    return () => {
      ticket.current += 1;
      window.clearInterval(timer);
      document.removeEventListener("visibilitychange", load);
    };
  }, [refresh]);

  return { items, refresh };
}

export function MediaHubButton() {
  const { items, refresh } = useMediaItems();
  const [forced, setForced] = useState<boolean | null>(null);
  const count = items.length;
  const allMuted = forced ?? (count > 0 && items.every((item) => item.muted));

  const toggleMute = () => {
    const next = !allMuted;
    setForced(next);
    void setAllMediaMuted(next)
      .then(() => refresh())
      .finally(() => setForced(null));
  };

  return (
    <ToolButton
      id="media-hub"
      label="Media"
      kind="media-hub"
      icon={<Clapperboard id="address-media-hub-icon" aria-hidden />}
      activeTab={undefined}
      needsPage={false}
      corner={
        count > 0 ? (
          <button
            id="address-media-hub-mute"
            type="button"
            title={allMuted ? "Unmute all" : "Mute all"}
            aria-label={allMuted ? "Unmute all" : "Mute all"}
            aria-pressed={allMuted}
            className={cn(
              "inline-flex size-4 items-center justify-center rounded-full bg-zinc-950 text-zinc-100 ring-1 ring-zinc-600 hover:bg-zinc-800 [&_svg]:size-3",
              focusRing,
            )}
            onPointerDown={(event) => event.stopPropagation()}
            onClick={(event) => {
              event.preventDefault();
              event.stopPropagation();
              toggleMute();
            }}
          >
            {allMuted ? (
              <VolumeX id="address-media-hub-mute-icon" aria-hidden />
            ) : (
              <Volume2 id="address-media-hub-mute-icon" aria-hidden />
            )}
          </button>
        ) : 
        null
      }
    />
  );
}
