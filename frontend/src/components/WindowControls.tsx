import { useEffect, useState } from "react";
import { Copy, Minus, Square, X } from "lucide-react";
import { closeWindow, isWindowMaximized, minimizeWindow, onWindowResized, toggleMaximizeWindow } from "../lib/browser-client";

const buttonClass =
  "inline-flex h-full w-11 items-center justify-center text-zinc-300 transition-colors hover:bg-zinc-800 hover:text-zinc-100 [&_svg]:size-3.5";

export function WindowControls() {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    const refresh = () => void isWindowMaximized().then(setMaximized);
    refresh();
    const unlisten = onWindowResized(refresh);
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);

  return (
    <div id="window-controls" className="flex h-full shrink-0 items-stretch">
      <button id="window-minimize" type="button" title="Minimize" className={buttonClass} onClick={() => void minimizeWindow()}>
        <Minus aria-hidden />
      </button>
      <button
        id="window-maximize"
        type="button"
        title={maximized ? "Restore" : "Maximize"}
        className={buttonClass}
        onClick={() => void toggleMaximizeWindow()}
      >
        {maximized ? <Copy aria-hidden className="-scale-x-100" /> : <Square aria-hidden />}
      </button>
      <button
        id="window-close"
        type="button"
        title="Close"
        className={`${buttonClass} hover:bg-red-600 hover:text-white`}
        onClick={() => void closeWindow()}
      >
        <X aria-hidden />
      </button>
    </div>
  );
}
