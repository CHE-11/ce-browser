import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { explain, studio } from "./format.js";
import { Icon, Mark } from "./ui.jsx";

function ToolButton({ id, label, onClick, children }) {
  return (
    <button
      id={id}
      type="button"
      title={label}
      aria-label={label}
      onClick={onClick}
      className="inline-flex size-7 shrink-0 items-center justify-center rounded-lg text-zinc-300 hover:bg-zinc-800 hover:text-white"
    >
      {children}
    </button>
  );
}

function Strip() {
  const [side, setSide] = useState("bottom");
  const [count, setCount] = useState(0);
  const [error, setError] = useState("");
  const vertical = side === "left" || side === "right";

  useEffect(() => {
    document.title = "Studio";
    const ce = studio();
    if (!ce) {
      setError("Studio runs inside the browser.");
      return;
    }
    let live = true;
    ce.ui.pageToolbar().then(
      (toolbar) => {
        if (live && toolbar && toolbar.side) setSide(toolbar.side);
      },
      (reason) => {
        if (live) setError(explain(reason));
      },
    );
    const paint = () => {
      ce.tabs.list().then(
        (tabs) => {
          if (live) setCount(Array.isArray(tabs) ? tabs.length : 0);
        },
        () => {},
      );
    };
    paint();
    const stopTabs = ce.tabs.onChanged(paint);
    const stopSide = ce.ui.onPageToolbar((next) => {
      if (next) setSide(next);
    });
    return () => {
      live = false;
      stopTabs();
      stopSide();
    };
  }, []);

  const run = (action) => {
    const ce = studio();
    if (!ce) {
      setError("Studio runs inside the browser.");
      return;
    }
    setError("");
    Promise.resolve()
      .then(() => action(ce))
      .catch((reason) => setError(explain(reason)));
  };

  return (
    <div
      id="studio-strip"
      data-side={side}
      title={error || undefined}
      className={
        vertical
          ? "flex h-full w-full flex-col items-center gap-1 overflow-hidden bg-zinc-950 px-1 py-1.5 text-zinc-100"
          : "flex h-full w-full items-center gap-2 overflow-hidden bg-zinc-950 px-2 text-zinc-100"
      }
    >
      <div id="studio-strip-brand" className={vertical ? "flex flex-col items-center gap-1" : "flex min-w-0 items-center gap-2"}>
        <Mark id="studio-strip-mark" small />
        {vertical ? null : (
          <span id="studio-strip-name" className="truncate text-xs font-semibold tracking-tight">
            Studio
          </span>
        )}
      </div>
      <p id="studio-strip-count" className={vertical ? "text-[10px] font-medium text-zinc-400 tabular-nums" : "text-[11px] text-zinc-400 tabular-nums"}>
        {vertical ? count : `${count} tabs`}
      </p>
      <div id="studio-strip-actions" className={vertical ? "flex flex-col items-center gap-0.5" : "flex items-center gap-0.5"}>
        <ToolButton id="studio-strip-panel" label="Open panel" onClick={() => run((ce) => ce.ui.togglePanel())}>
          <Icon id="studio-strip-panel-icon" name="panel" />
        </ToolButton>
        <ToolButton id="studio-strip-page" label="Open page" onClick={() => run((ce) => ce.ui.openPage("home"))}>
          <Icon id="studio-strip-page-icon" name="page" />
        </ToolButton>
        <ToolButton id="studio-strip-window" label="Open window" onClick={() => run((ce) => ce.ui.openPopout())}>
          <Icon id="studio-strip-window-icon" name="window" />
        </ToolButton>
      </div>
      <p id="studio-strip-error" className="hidden">
        {error}
      </p>
    </div>
  );
}

createRoot(document.getElementById("root")).render(<Strip />);
