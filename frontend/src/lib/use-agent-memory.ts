import { useState } from "react";
import type { AgentMemoryItem } from "@type/browser";
import { agentMemoryDelete, agentMemorySave } from "./browser-client";
import { useAgentMemoryItems } from "./use-browser";

function errorOf(reason: unknown) {
  return typeof reason === "string" ? reason : reason instanceof Error ? reason.message : "Something went wrong.";
}

/** The agent's notes and the edits the memory view offers. */
export function useAgentMemory() {
  const { items, loaded } = useAgentMemoryItems();
  const [error, setError] = useState<string | null>(null);

  const guard = (action: Promise<unknown>) => {
    setError(null);
    return action.then(
      () => true,
      (reason: unknown) => {
        setError(errorOf(reason));
        return false;
      },
    );
  };

  return {
    items,
    loaded,
    error,
    add: (text: string, site: string, pinned: boolean, category: AgentMemoryItem["category"] = "") =>
      guard(
        agentMemorySave({
          id: "",
          text,
          site,
          pinned,
          created: 0,
          used: 0,
          category,
          sourceUrl: "",
          sourceRun: 0,
          expires: 0,
        }),
      ),
    update: (item: AgentMemoryItem) =>
      guard(
        agentMemorySave({
          ...item,
          category: item.category ?? "",
          sourceUrl: item.sourceUrl ?? "",
          sourceRun: item.sourceRun ?? 0,
          expires: item.expires ?? 0,
        }),
      ),
    togglePin: (item: AgentMemoryItem) =>
      guard(
        agentMemorySave({
          ...item,
          category: item.category ?? "",
          sourceUrl: item.sourceUrl ?? "",
          sourceRun: item.sourceRun ?? 0,
          expires: item.expires ?? 0,
          pinned: !item.pinned,
        }),
      ),
    remove: (id: string) => guard(agentMemoryDelete(id)),
  };
}
