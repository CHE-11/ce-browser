import { useEffect, useState } from "react";
import { onDebugOutline } from "@/lib/browser-client";

/** The outline window's region shows only a 3px border and a 280x22 tab at the top-left
 * (`src-tauri/src/debug/outline.rs`), so the page paints everything in the outline color. */
export function DebugOutlinePage({ params }: { params: URLSearchParams }) {
  const [label, setLabel] = useState(() => params.get("label") ?? "");

  useEffect(() => {
    document.title = "Debug outline";
    const root = document.documentElement;
    root.style.background = "#d946ef";
    document.body.style.background = "#d946ef";
    const unlisten = onDebugOutline(setLabel);
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);

  return (
    <main id="debug-outline" className="h-screen w-screen overflow-hidden bg-fuchsia-500">
      <span id="debug-outline-label" className="block h-[22px] w-[280px] truncate px-2 font-mono text-[11px] font-semibold leading-[22px] text-white">
        {label}
      </span>
    </main>
  );
}
