import { useEffect } from "react";
import { createRoot } from "react-dom/client";
import { studio } from "./format.js";
import { Mark } from "./ui.jsx";

function Bar() {
  useEffect(() => {
    document.title = "Studio";
  }, []);
  return (
    <div id="studio-bar" className="flex h-full items-center gap-2 overflow-hidden bg-zinc-950 px-2 text-zinc-100">
      <Mark id="studio-bar-mark" small />
      <span id="studio-bar-label" className="truncate text-[11px] font-semibold tracking-tight">
        Studio
      </span>
      <button
        id="studio-bar-open"
        type="button"
        onClick={() => {
          const ce = studio();
          if (ce) ce.ui.openPage("home");
        }}
        className="shrink-0 rounded-md bg-indigo-500 px-2 py-0.5 text-[11px] leading-none font-medium text-white hover:bg-indigo-400"
      >
        Open page
      </button>
    </div>
  );
}

createRoot(document.getElementById("root")).render(<Bar />);
