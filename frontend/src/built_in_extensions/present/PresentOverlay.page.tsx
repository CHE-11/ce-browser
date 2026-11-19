import { useEffect, useState } from "react";
import { onPresentChanged, presentState } from "@/lib/browser-client";
import type { PresentState } from "@type/browser";
import { PresentStage } from "./PresentStage";

const IDLE: PresentState = {
  on: true,
  tool: "spotlight",
  cursor: "dot",
  color: "#f59e0b",
  width: 1,
  dim: 0.62,
  radius: 12,
  spread: 240,
  circle: false,
  placing: false,
  clear: 0,
  undo: 0,
  spotlight: null,
  zoom: null,
};

export function PresentOverlayPage() {
  const [state, setState] = useState<PresentState>(IDLE);

  useEffect(() => {
    document.title = "Present";
    document.documentElement.dataset.presentClear = "true";
    let stop = false;
    void presentState()
      .then((next) => {
        if (!stop) setState(next);
      })
      .catch(() => undefined);
    const stateListen = onPresentChanged((next) => {
      if (!stop) setState(next);
    });
    return () => {
      stop = true;
      delete document.documentElement.dataset.presentClear;
      void stateListen.then((unlisten) => unlisten());
    };
  }, []);

  return (
    <PresentStage
      tool={state.tool}
      cursor={state.cursor}
      color={state.color}
      width={state.width}
      dim={state.dim}
      radius={state.radius}
      spread={state.spread}
      circle={state.circle}
      placing={state.placing}
      clearToken={state.clear}
      undoToken={state.undo}
      spotlight={state.spotlight}
    />
  );
}
