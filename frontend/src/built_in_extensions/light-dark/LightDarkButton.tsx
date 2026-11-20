import { useEffect, useId, useRef, useState } from "react";
import { IconButton } from "@/components/IconButton";
import { resolveMode, type ResolvedMode } from "@/lib/appearance";
import { getWindowsColorScheme, onWindowsColorScheme } from "@/lib/browser-client";
import { useSettingsState } from "@/lib/use-browser";
import { toggleSystemColorScheme } from "./toggle";

/** Four rays, held off the disc so the mark stays a sun. */
const SUN_RAYS = Array.from({ length: 4 }, (_, index) => {
  const angle = (Math.PI / 2) * index - Math.PI / 2;
  const cos = Math.cos(angle);
  const sin = Math.sin(angle);
  return {
    x1: 8 + cos * 4.7,
    y1: 8 + sin * 4.7,
    x2: 8 + cos * 6.85,
    y2: 8 + sin * 6.85,
  };
});

export function LightDarkButton() {
  const { settings } = useSettingsState();
  const [windows, setWindows] = useState<ResolvedMode | null>(null);
  const [override, setOverride] = useState<ResolvedMode | null>(null);
  const busy = useRef(false);

  useEffect(() => {
    let alive = true;
    void getWindowsColorScheme()
      .then((scheme) => {
        if (alive) {
          setWindows((current) => current ?? scheme);
        }
      })
      .catch(() => undefined);
    const unlisten = onWindowsColorScheme((scheme) => {
      alive = false;
      setWindows(scheme);
    });
    return () => {
      alive = false;
      void unlisten.then((stop) => stop());
    };
  }, []);

  const resolved: ResolvedMode =
    settings?.colorMode === "light" || settings?.colorMode === "dark"
      ? settings.colorMode
      : (windows ?? resolveMode(settings?.colorMode ?? "system"));
  const mode = override ?? resolved;
  const dark = mode === "dark";

  useEffect(() => {
    setOverride(null);
  }, [resolved]);

  const onToggle = () => {
    if (busy.current) {
      return;
    }
    busy.current = true;
    setOverride(mode === "dark" ? "light" : "dark");
    void toggleSystemColorScheme(mode)
      .then((applied) => setOverride(applied))
      .catch(() => setOverride(null))
      .finally(() => {
        busy.current = false;
      });
  };

  return (
    <IconButton
      id="address-light-dark"
      label={dark ? "Switch to light mode" : "Switch to dark mode"}
      className="relative"
      style={{
        backgroundColor: dark ? "color-mix(in oklch, var(--app-violet-500) 22%, transparent)" : "transparent",
      }}
      onClick={onToggle}
    >
      <LightDarkGlyph dark={dark} />
    </IconButton>
  );
}

function LightDarkGlyph({ dark }: { dark: boolean }) {
  const maskId = `light-dark-${useId().replace(/:/g, "")}`;

  return (
    <svg
      id="address-light-dark-icon"
      viewBox="0 0 16 16"
      aria-hidden
      className="absolute"
      style={{
        left: "50%",
        top: "50%",
        width: 22,
        height: 22,
        transform: "translate(-50%, -50%)",
        color: "var(--app-violet-200)",
      }}
    >
      <defs>
        <mask id={maskId}>
          <rect id="address-light-dark-mask" width="16" height="16" fill="white" />
          <circle
            id="address-light-dark-bite"
            cx="8"
            cy="8"
            r="3.15"
            fill="black"
            style={{
              transform: dark ? "translate(2.15px, -1.55px)" : "translate(8px, -6px)",
              transition: "transform 520ms cubic-bezier(.2, .8, .2, 1)",
            }}
          />
        </mask>
      </defs>
      <g id="address-light-dark-moon" style={{ opacity: dark ? 1 : 0, transition: "opacity 320ms ease" }}>
        <circle id="address-light-dark-sky" cx="8" cy="8" r="7.2" fill="var(--app-violet-800)" opacity={0.85} />
        <circle id="address-light-dark-disc" cx="8" cy="8" r="3.15" fill="currentColor" mask={`url(#${maskId})`} />
        <g id="address-light-dark-stars">
          <circle id="address-light-dark-star-a" cx="12.4" cy="3.2" r="0.7" fill="currentColor">
            <animate id="address-light-dark-star-a-twinkle" attributeName="opacity" values="0.35;1;0.35" dur="2.6s" repeatCount="indefinite" />
          </circle>
          <circle id="address-light-dark-star-b" cx="14.1" cy="6.1" r="0.45" fill="currentColor">
            <animate id="address-light-dark-star-b-twinkle" attributeName="opacity" values="1;0.25;1" dur="3.1s" repeatCount="indefinite" />
          </circle>
          <circle id="address-light-dark-star-c" cx="3.1" cy="4.2" r="0.4" fill="currentColor">
            <animate id="address-light-dark-star-c-twinkle" attributeName="opacity" values="0.4;1;0.4" dur="2.2s" begin="0.4s" repeatCount="indefinite" />
          </circle>
        </g>
      </g>
      <g id="address-light-dark-sun" style={{ opacity: dark ? 0 : 1, transition: "opacity 320ms ease" }}>
        {SUN_RAYS.map((ray, index) => (
          <line
            key={index}
            id={`address-light-dark-ray-${index}`}
            x1={ray.x1}
            y1={ray.y1}
            x2={ray.x2}
            y2={ray.y2}
            stroke="#f0a000"
            strokeWidth="1.35"
            strokeLinecap="round"
          />
        ))}
        <circle id="address-light-dark-sun-disc" cx="8" cy="8" r="2.85" fill="#ffc53d" />
        <circle id="address-light-dark-sun-glint" cx="7.15" cy="7.05" r="1.05" fill="#fff4c8" />
      </g>
    </svg>
  );
}
