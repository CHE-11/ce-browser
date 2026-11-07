import { useState } from "react";
import type { AgentRun } from "@type/browser";
import {
  agentAnswer,
  agentClear,
  agentReply,
  agentResume,
  agentShowTab,
  agentStart,
  agentStop,
  agentTakeOver,
  currentWindowLabel,
} from "./browser-client";
import { useAgentRuns, useTabs } from "./use-browser";

/** Selection that shows the composer for a new task instead of a run. */
const NEW_TASK = -1;

export function isActiveRun(run: AgentRun | null | undefined) {
  return !!run && (run.status === "running" || run.status === "waiting" || run.status === "paused");
}

function errorOf(reason: unknown) {
  if (typeof reason === "string") {
    return reason;
  }
  if (reason instanceof Error) {
    return reason.message;
  }
  return "Something went wrong.";
}

function windowLabel() {
  try {
    return currentWindowLabel();
  } catch {
    return "main";
  }
}

/** Agent runs for this window, the run on show, and the actions the panel offers. */
export function useAgentRun() {
  const { runs } = useAgentRuns();
  const tabs = useTabs();
  const label = windowLabel();
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const windowRuns = runs.filter((run) => run.window === label);
  const activeTab = tabs.windowTabs.find((tab) => tab.id === tabs.activeId) ?? null;
  const run =
    selectedId === NEW_TASK
      ? null
      : windowRuns.find((item) => item.id === selectedId) ??
    windowRuns.find((item) => item.tabId === tabs.activeId && isActiveRun(item)) ??
    windowRuns.find((item) => isActiveRun(item)) ??
    windowRuns[0] ??
    null;

  const guard = (action: Promise<unknown>) => {
    setError(null);
    void action.catch((reason: unknown) => setError(errorOf(reason)));
  };

  return {
    runs: windowRuns,
    run,
    activeTab,
    error,
    clearError: () => setError(null),
    select: (id: number | null) => setSelectedId(id),
    start: (goal: string) => {
      if (!activeTab) {
        setError("Open a tab for the agent to work in.");
        return;
      }
      setError(null);
      void agentStart(activeTab.id, goal)
        .then((id) => setSelectedId(id))
        .catch((reason: unknown) => setError(errorOf(reason)));
    },
    newTask: () => setSelectedId(NEW_TASK),
    stop: (id: number) => guard(agentStop(id)),
    answer: (id: number, approval: number, allow: boolean) => guard(agentAnswer(id, approval, allow)),
    reply: (id: number, text: string) => guard(agentReply(id, text)),
    takeOver: (id: number) => guard(agentTakeOver(id)),
    resume: (id: number) => guard(agentResume(id)),
    showTab: (tabId: number) => guard(agentShowTab(tabId)),
    clear: () => {
      setSelectedId(null);
      guard(agentClear());
    },
  };
}
