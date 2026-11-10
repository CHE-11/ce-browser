import { useEffect, useState } from "react";
import { Presentation } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { onPresentChanged, presentState, togglePresent } from "@/lib/browser-client";

/** Turns the window overlay on or off. */
export function PresentButton() {
  const [on, setOn] = useState(false);

  useEffect(() => {
    let stop = false;
    void presentState()
      .then((state) => {
        if (!stop) setOn(state.on);
      })
      .catch(() => undefined);
    const listen = onPresentChanged((state) => {
      if (!stop) setOn(state.on);
    });
    return () => {
      stop = true;
      void listen.then((unlisten) => unlisten());
    };
  }, []);

  return (
    <IconButton id="address-present" label={on ? "Stop presenting" : "Present"} active={on} onClick={() => void togglePresent().then((state) => setOn(state.on))}>
      <Presentation id="address-present-icon" aria-hidden />
    </IconButton>
  );
}
