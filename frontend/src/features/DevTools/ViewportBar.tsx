import { RotateCw } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { clearViewport, openDevtools } from "@/lib/browser-client";
import { DEVICE_GROUPS, DEVICES, deviceFor, pushViewport, viewportEnvironment } from "./devices";

type ViewportBarProps = {
  width: number;
  height: number;
  cutout: boolean;
  homeBar: boolean;
  /** Standalone installed web app, instead of a page in a mobile browser. */
  installed: boolean;
  /** Active tab, used to close the docked tools. Null hides the button. */
  devtoolsTabId: number | null;
};

function presetFor(width: number, height: number) {
  if (width <= 0 || height <= 0) {
    return "full";
  }
  return deviceFor(width, height)?.id ?? "custom";
}

/**
 * Sits in the page column, above the real page view. The native webview is placed below this bar
 * at the chosen CSS size, so device preview is the page itself.
 */
/** Matches the device bar height the page view is placed under. */
const BAR_HEIGHT = 36;

export function ViewportBar({ width, height, cutout, homeBar, installed, devtoolsTabId }: ViewportBarProps) {
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

  const commit = (
    nextWidth: number,
    nextHeight: number,
    nextCutout = cutout,
    nextHome = homeBar,
    nextInstalled = installed,
  ) => {
    if (!Number.isFinite(nextWidth) || !Number.isFinite(nextHeight) || nextWidth <= 0 || nextHeight <= 0) {
      void clearViewport();
      return;
    }
    void pushViewport(nextWidth, nextHeight, nextCutout, nextHome, nextInstalled);
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
  const environment = viewportEnvironment(width, height, installed, cutout, homeBar);
  const safe = environment.insets;
  const installedTitle = installed
    ? `Installed web app. Safe area ${safe.top} ${safe.right} ${safe.bottom} ${safe.left}. Standalone display mode, with the status bar and cutout over the page.`
    : "Mobile web app installed. Full screen, with a status bar, cutout, and safe-area insets instead of a browser page.";

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
        className="h-7 max-w-48 rounded-md border border-zinc-700 bg-zinc-900 px-2 text-xs text-zinc-100"
        value={preset}
        onChange={(event) => {
          if (event.target.value === "full") {
            void clearViewport();
            return;
          }
          const next = DEVICES.find((item) => item.id === event.target.value);
          if (next) {
            commit(next.width, next.height, next.cutout, next.home);
          }
        }}
      >
        <option id="viewport-device-full" value="full">
          Full size
        </option>
        {DEVICE_GROUPS.map((group) => (
          <optgroup id={`viewport-devices-${group.toLowerCase()}`} key={group} label={group}>
            {DEVICES.filter((item) => item.group === group).map((item) => (
              <option id={`viewport-device-${item.id}`} key={item.id} value={item.id}>
                {item.label}
              </option>
            ))}
          </optgroup>
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
      <button
        id="viewport-installed"
        type="button"
        aria-pressed={installed}
        disabled={!sized}
        title={installedTitle}
        className="h-7 rounded-md px-2 text-xs text-zinc-200 hover:bg-zinc-800 disabled:opacity-40 aria-pressed:bg-zinc-700"
        onClick={() => commit(width, height, cutout, homeBar, !installed)}
      >
        Installed
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
