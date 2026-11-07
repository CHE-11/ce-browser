import { useEffect, useState } from "react";
import type { AgentDelta } from "@type/browser";
import { onAgentDelta } from "./browser-client";

const EMPTY = { text: "", thinking: "" };

/** What the model is writing for `runId` right now. Empty between turns. */
export function useAgentStream(runId: number | null) {
  const [delta, setDelta] = useState<{ runId: number | null; text: string; thinking: string }>({ runId: null, ...EMPTY });

  useEffect(() => {
    if (runId === null) {
      return;
    }
    const unlisten = onAgentDelta((next: AgentDelta) => {
      if (next.runId === runId) {
        setDelta({ runId, text: next.text, thinking: next.thinking });
      }
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [runId]);

  return delta.runId === runId ? { text: delta.text, thinking: delta.thinking } : EMPTY;
}
