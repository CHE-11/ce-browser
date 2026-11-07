import { useEffect, useState } from "react";
import type { AgentAccess, AgentAutonomy, AgentRun } from "@type/browser";
import { DEFAULT_ACCESS, withDefaults } from "./agent-access";
import {
  agentAnswer,
  agentClear,
  agentContinue,
  agentDeleteRun,
  agentReply,
  agentResume,
  agentRun,
  agentSaveFlow,
  agentSetAccess,
  agentSetAutonomy,
  agentShowTab,
  agentStart,
  agentStop,
  agentTakeOver,
  createTab,
  currentWindowLabel,
} from "./browser-client";
import { useAgentRuns, useSettings, useTabs } from "./use-browser";

/** Selection that shows the composer for a new task instead of a run. */
const NEW_TASK = -1;
const OPTIONS_KEY = "ce-agent-panel-options";

/** How the next task starts. */
export type AgentDraftOptions = {
  autonomy: AgentAutonomy;
  /** Blank uses the agent model from Settings. */
  model: string;
  access: AgentAccess;
};

export type AgentStartRequest = {
  goal: string;
  /** Other tabs whose text the agent reads first. */
  contextTabs?: number[];
  /** Work in a new background tab instead of the selected one. */
  background?: boolean;
  /** Overrides the autonomy picked in the panel for this task only. */
  autonomy?: AgentAutonomy;
  /** Guidance for the model that isn't shown as part of the goal. */
  instructions?: string;
};

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

function loadOptions(): Partial<AgentDraftOptions> {
  try {
    const saved = JSON.parse(localStorage.getItem(OPTIONS_KEY) ?? "{}") as Partial<AgentDraftOptions>;
    return {
      autonomy: saved.autonomy === "ask" || saved.autonomy === "auto" || saved.autonomy === "full" ? saved.autonomy : undefined,
      model: typeof saved.model === "string" ? saved.model : undefined,
      access: saved.access && typeof saved.access === "object" ? withDefaults({ ...saved.access, site: "" }) : undefined,
    };
  } catch {
    return {};
  }
}

/** Agent runs for this window, the run on show, and the actions the panel offers. */
export function useAgentRun() {
  const { runs } = useAgentRuns();
  const tabs = useTabs();
  const settings = useSettings();
  const label = windowLabel();
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [details, setDetails] = useState<Record<number, AgentRun>>({});
  const [options, setOptions] = useState<Partial<AgentDraftOptions>>(loadOptions);
  const windowRuns = runs.filter((run) => run.window === label);
  const activeTab = tabs.windowTabs.find((tab) => tab.id === tabs.activeId) ?? null;
  const listed =
    selectedId === NEW_TASK
      ? null
      : windowRuns.find((item) => item.id === selectedId) ??
        windowRuns.find((item) => item.tabId === tabs.activeId && isActiveRun(item)) ??
        windowRuns.find((item) => item.parent === null && isActiveRun(item)) ??
        windowRuns.find((item) => item.parent === null) ??
        null;
  const missingSteps = !!listed && listed.steps.length < listed.stepCount;
  const cached = listed ? details[listed.id] : undefined;
  const run = listed && missingSteps && cached ? { ...listed, steps: cached.steps } : listed;
  const draft: AgentDraftOptions = {
    autonomy: options.autonomy ?? settings?.automation.defaultAutonomy ?? "ask",
    model: options.model ?? "",
    access: options.access ?? DEFAULT_ACCESS,
  };

  const listedId = listed?.id;
  const listedCount = listed?.stepCount;
  useEffect(() => {
    if (listedId === undefined || !missingSteps || cached?.stepCount === listedCount) {
      return;
    }
    let live = true;
    void agentRun(listedId).then((detail) => {
      if (live && detail) {
        setDetails((previous) => ({ ...previous, [detail.id]: detail }));
      }
    });
    return () => {
      live = false;
    };
  }, [listedId, listedCount, missingSteps, cached?.stepCount]);

  const guard = (action: Promise<unknown>) => {
    setError(null);
    void action.catch((reason: unknown) => setError(errorOf(reason)));
  };

  const start = async ({ goal, contextTabs = [], background = false, autonomy, instructions }: AgentStartRequest) => {
    setError(null);
    try {
      let tabId = activeTab?.id;
      if (background) {
        tabId = await createTab(undefined, true);
      }
      if (tabId === undefined) {
        setError("Open a tab for the agent to work in.");
        return;
      }
      const id = await agentStart(tabId, goal, {
        autonomy: autonomy ?? draft.autonomy,
        model: draft.model,
        background,
        contextTabs: contextTabs.filter((id) => id !== tabId),
        instructions,
        access: { ...draft.access, site: "" },
      });
      setSelectedId(id);
    } catch (reason: unknown) {
      setError(errorOf(reason));
    }
  };

  return {
    runs: windowRuns,
    run,
    activeTab,
    error,
    draft,
    clearError: () => setError(null),
    setDraft: (patch: Partial<AgentDraftOptions>) => {
      setOptions((previous) => {
        const next = { ...previous, ...patch };
        localStorage.setItem(OPTIONS_KEY, JSON.stringify(next));
        return next;
      });
    },
    select: (id: number | null) => setSelectedId(id),
    start,
    newTask: () => setSelectedId(NEW_TASK),
    childrenOf: (id: number) => windowRuns.filter((item) => item.parent === id),
    stop: (id: number) => guard(agentStop(id)),
    answer: (id: number, approval: number, allow: boolean) => guard(agentAnswer(id, approval, allow)),
    reply: (id: number, text: string) => guard(agentReply(id, text)),
    followUp: (id: number, text: string) => guard(agentContinue(id, text, activeTab?.id)),
    setAutonomy: (id: number, autonomy: AgentAutonomy) => guard(agentSetAutonomy(id, autonomy)),
    setAccess: (id: number, access: AgentAccess) => guard(agentSetAccess(id, access)),
    takeOver: (id: number) => guard(agentTakeOver(id)),
    resume: (id: number) => guard(agentResume(id)),
    showTab: (tabId: number) => guard(agentShowTab(tabId)),
    saveFlow: (id: number) => guard(agentSaveFlow(id)),
    deleteRun: (id: number) => {
      if (selectedId === id) {
        setSelectedId(null);
      }
      guard(agentDeleteRun(id));
    },
    clear: () => {
      setSelectedId(null);
      guard(agentClear());
    },
  };
}

export type AgentController = ReturnType<typeof useAgentRun>;
