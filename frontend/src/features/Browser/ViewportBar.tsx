import { RotateCw } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { clearViewport, openDevtools, setViewport } from "@/lib/browser-client";

type ViewportBarProps = {
  width: number;
  height: number;
  cutout: boolean;
  homeBar: boolean;
  /** Active tab, used to close the docked tools. Null hides the button. */
  devtoolsTabId: number | null;
};

const PRESETS = [
  { id: "full", label: "Full size", width: 0, height: 0, cutout: false, home: false },
  { id: "iphone-16", label: "iPhone 16", width: 393, height: 852, cutout: true, home: true },
  { id: "iphone-se", label: "iPhone SE", width: 375, height: 667, cutout: false, home: false },
  { id: "pixel-9", label: "Pixel 9", width: 412, height: 892, cutout: true, home: true },
  { id: "ipad", label: "iPad", width: 820, height: 1180, cutout: false, home: true },
  { id: "laptop", label: "Laptop", width: 1280, height: 800, cutout: false, home: false },
] as const;

function presetFor(width: number, height: number) {
  if (width <= 0 || height <= 0) {
    return "full";
  }
  return (
    PRESETS.find(
      (preset) =>
        preset.width > 0 &&
        ((preset.width === width && preset.height === height) ||
          (preset.width === height && preset.height === width)),
    )?.id ?? "custom"
  );
}

/**
 * Sits in the page column, above the real page view. The native webview is placed below this bar
 * at the chosen CSS size, so device preview is the page itself.
 */
/** Matches the device bar height the page view is placed under. */
const BAR_HEIGHT = 36;

export function ViewportBar({ width, height, cutout, homeBar, devtoolsTabId }: ViewportBarProps) {
  const barRef = useRef<HTMLDivElement>(null);
  const widthFocused = useRef(false);
  const heightFocused = useRef(false);
  const [frame, setFrame] = useState({ width: 0, height: 0 });
  const [draftWidth, setDraftWidth] = useState("");
  const [draftHeight, setDraftHeight] = useState("");
  const preset = presetFor(width, height);
  const full = width <= 0 || height <= 0;

  useEffect(() => {
    const pages = barRef.current?.parentElement;
    if (!pages) {
      return;
    }
    const measure = () => {
      const rect = pages.getBoundingClientRect();
      setFrame({
        width: Math.max(0, Math.round(rect.width)),
        height: Math.max(0, Math.round(rect.height - BAR_HEIGHT)),
      });
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(pages);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const shownWidth = full ? frame.width : width;
    const shownHeight = full ? frame.height : height;
    if (!widthFocused.current && shownWidth > 0) {
      setDraftWidth(String(shownWidth));
    }
    if (!heightFocused.current && shownHeight > 0) {
      setDraftHeight(String(shownHeight));
    }
  }, [full, width, height, frame]);

  const commit = (nextWidth: number, nextHeight: number, nextCutout = cutout, nextHome = homeBar) => {
    if (!Number.isFinite(nextWidth) || !Number.isFinite(nextHeight) || nextWidth <= 0 || nextHeight <= 0) {
      void clearViewport();
      return;
    }
    void setViewport(Math.round(nextWidth), Math.round(nextHeight), nextCutout, nextHome);
  };

  const commitDraft = () => {
    const nextWidth = Number(draftWidth);
    const nextHeight = Number(draftHeight);
    if (full && nextWidth === frame.width && nextHeight === frame.height) {
      return;
    }
    commit(nextWidth, nextHeight);
  };

  const sized = width > 0 && height > 0;

  return (
    <div
      id="viewport-bar"
      ref={barRef}
      className="absolute inset-x-0 top-0 z-20 flex h-9 items-center justify-center border-b border-zinc-800 bg-zinc-950 px-2"
    >
      <div id="viewport-controls" className="flex items-center gap-2">
      <label id="viewport-device-label" className="sr-only" htmlFor="viewport-device">
        Device
      </label>
      <select
        id="viewport-device"
        aria-label="Device"
        className="h-7 max-w-40 rounded-md border border-zinc-700 bg-zinc-900 px-2 text-xs text-zinc-100"
        value={preset}
        onChange={(event) => {
          const next = PRESETS.find((item) => item.id === event.target.value);
          if (next) {
            commit(next.width, next.height, next.cutout, next.home);
          }
        }}
      >
        {PRESETS.map((item) => (
          <option id={`viewport-device-${item.id}`} key={item.id} value={item.id}>
            {item.label}
          </option>
        ))}
        <option id="viewport-device-custom" value="custom">
          Custom
        </option>
      </select>
      <label id="viewport-width-label" className="sr-only" htmlFor="viewport-width">
        Width
      </label>
      <input
        id="viewport-width"
        aria-label="Width"
        inputMode="numeric"
        className="h-7 w-16 rounded-md border border-zinc-700 bg-zinc-900 px-2 text-xs text-zinc-100"
        value={draftWidth}
        onChange={(event) => setDraftWidth(event.target.value)}
        onFocus={() => {
          widthFocused.current = true;
        }}
        onBlur={() => {
          widthFocused.current = false;
          commitDraft();
        }}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            commitDraft();
          }
        }}
      />
      <span id="viewport-size-times" className="text-xs text-zinc-500">
        ×
      </span>
      <label id="viewport-height-label" className="sr-only" htmlFor="viewport-height">
        Height
      </label>
      <input
        id="viewport-height"
        aria-label="Height"
        inputMode="numeric"
        className="h-7 w-16 rounded-md border border-zinc-700 bg-zinc-900 px-2 text-xs text-zinc-100"
        value={draftHeight}
        onChange={(event) => setDraftHeight(event.target.value)}
        onFocus={() => {
          heightFocused.current = true;
        }}
        onBlur={() => {
          heightFocused.current = false;
          commitDraft();
        }}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            commitDraft();
          }
        }}
      />
      <button
        id="viewport-rotate"
        type="button"
        aria-label="Rotate"
        title="Rotate"
        className="flex size-7 items-center justify-center rounded-md text-zinc-300 hover:bg-zinc-800"
        onClick={() => commit(height, width)}
      >
        <RotateCw id="viewport-rotate-icon" className="size-3.5" aria-hidden />
      </button>
      <button
        id="viewport-cutout"
        type="button"
        aria-pressed={cutout}
        disabled={!sized}
        title="Cutout"
        className="h-7 rounded-md px-2 text-xs text-zinc-200 hover:bg-zinc-800 disabled:opacity-40 aria-pressed:bg-zinc-700"
        onClick={() => commit(width, height, !cutout, homeBar)}
      >
        Cutout
      </button>
      <button
        id="viewport-home"
        type="button"
        aria-pressed={homeBar}
        disabled={!sized}
        title="Home bar"
        className="h-7 rounded-md px-2 text-xs text-zinc-200 hover:bg-zinc-800 disabled:opacity-40 aria-pressed:bg-zinc-700"
        onClick={() => commit(width, height, cutout, !homeBar)}
      >
        Home bar
      </button>
      </div>
      {devtoolsTabId !== null ? (
        <button
          id="viewport-close-devtools"
          type="button"
          className="absolute right-2 h-7 rounded-md px-2 text-xs text-zinc-200 hover:bg-zinc-800"
          onClick={() => void openDevtools(devtoolsTabId)}
        >
          Close DevTools
        </button>
      ) : null}
    </div>
  );
}
