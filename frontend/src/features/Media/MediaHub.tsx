import { useEffect, useState } from "react";
import { Pause, Play } from "lucide-react";
import { PopupFrame } from "@/components/PopupFrame";
import { closePopup, controlMedia, listMedia } from "@/lib/browser-client";
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

  return (
    <PopupFrame id="media-hub" maxHeight={480}>
      <div id="media-hub-header" className="px-4 pb-2 pt-3">
        <h2 id="media-hub-title" className="text-sm font-semibold">
          Media
        </h2>
        <p id="media-hub-subtitle" className="text-xs text-zinc-500">
          Audio and video in open tabs
        </p>
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
            <li id={`media-hub-row-${item.tabId}-${item.index}`} key={`${item.tabId}-${item.index}`} className="rounded-lg bg-zinc-800 p-2">
              <p id={`media-hub-label-${item.tabId}-${item.index}`} className="truncate text-xs text-zinc-200">
                {item.kind} · {item.label || "Untitled"}
              </p>
              <div id={`media-hub-controls-${item.tabId}-${item.index}`} className="mt-2 flex items-center gap-2">
                <button
                  id={`media-hub-toggle-${item.tabId}-${item.index}`}
                  type="button"
                  className="inline-flex size-7 items-center justify-center rounded-md bg-zinc-700"
                  onClick={() => void controlMedia(item.tabId, item.index, item.paused ? "play" : "pause")}
                >
                  {item.paused ? (
                    <Play id={`media-hub-play-${item.tabId}-${item.index}`} className="size-3.5" aria-hidden />
                  ) : (
                    <Pause id={`media-hub-pause-${item.tabId}-${item.index}`} className="size-3.5" aria-hidden />
                  )}
                </button>
                <input
                  id={`media-hub-seek-${item.tabId}-${item.index}`}
                  type="range"
                  min={0}
                  max={item.duration || 0}
                  step={0.1}
                  value={Math.min(item.current, item.duration || item.current)}
                  className="min-w-0 flex-1"
                  onChange={(event) => void controlMedia(item.tabId, item.index, "seek", Number(event.target.value))}
                />
                <button id={`media-hub-close-${item.tabId}-${item.index}`} type="button" className="sr-only" onClick={() => void closePopup()}>
                  Close
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}
    </PopupFrame>
  );
}
