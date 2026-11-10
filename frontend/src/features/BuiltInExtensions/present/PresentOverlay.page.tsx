import { useEffect, useState } from "react";
import { onPresentChanged, onPresentPointer, presentState } from "@/lib/browser-client";
import type { PresentPointer, PresentState } from "@type/browser";
import { PresentStage } from "./PresentStage";

const IDLE: PresentState = {
  on: true,
  tool: "spotlight",
  cursor: "dot",
  color: "#f59e0b",
  placing: false,
  clear: 0,
  spotlight: null,
  zoom: null,
};

const AWAY: PresentPointer = { x: 0, y: 0, inside: false };

export function PresentOverlayPage() {
  const [state, setState] = useState<PresentState>(IDLE);
  const [pointer, setPointer] = useState<PresentPointer>(AWAY);

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
    const pointerListen = onPresentPointer((next) => {
      if (!stop) setPointer(next);
    });
    return () => {
      stop = true;
      delete document.documentElement.dataset.presentClear;
      void stateListen.then((unlisten) => unlisten());
      void pointerListen.then((unlisten) => unlisten());
    };
  }, []);

  return (
    <PresentStage
      tool={state.tool}
      cursor={state.cursor}
      color={state.color}
      placing={state.placing}
      clearToken={state.clear}
      spotlight={state.spotlight}
      zoom={state.zoom}
      pointer={pointer}
    />
  );
}
