import { useEffect, useRef, useState } from "react";
import type { AgentAccess, AgentAttachment, AgentAutonomy, AgentPlanItem, AgentRun } from "@type/browser";
import { DEFAULT_ACCESS, withDefaults } from "./agent-access";
import {
  agentAllowRisk,
  agentAnswer,
  agentAnswerDialog,
  agentClear,
  agentCockpit,
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
  agentUpdatePlan,
  createTab,
  currentWindowLabel,
} from "./browser-client";
import { useAgentRuns, useSettings, useTabs } from "./use-browser";

const OPTIONS_KEY = "ce-agent-panel-options";
const TAB_RUNS_KEY = "ce-agent-panel-tab-runs";

/** How the next task starts. */
export type AgentDraftOptions = {
  autonomy: AgentAutonomy;
  /** Model override for the next task. Empty means use Settings. */
  model: string;
  access: AgentAccess;
};

export type AgentStartRequest = {
  goal: string;
  /** Other tabs whose text the agent reads first. */
  contextTabs?: number[];
  /** Work in a new background tab instead of the selected one. */
  background?: boolean;
  /** Tab the agent should control. Defaults to the active tab, or a web tab / new tab when that is a `ce://` page. */
  tabId?: number;
  /** Overrides the autonomy picked in the panel for this task only. */
  autonomy?: AgentAutonomy;
  /** Guidance for the model that isn't shown as part of the goal. */
  instructions?: string;
  /** Images and text files sent with the opening message. */
  attachments?: AgentAttachment[];
};

/** Pages the agent can click and type on. */
function isWorkableUrl(url: string) {
  return /^(https?:|file:)/i.test(url);
}

export function isActiveRun(run: AgentRun | null | undefined) {
  return !!run && (run.status === "running" || run.status === "waiting" || run.status === "paused");
}

/** Root agent tasks that belong to a browser tab (not helpers or MCP). */
function isTabChat(run: AgentRun) {
  return run.parent === null && run.source === "agent";
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

type TabRunMap = Record<string, number | "new">;

function loadTabRuns(): TabRunMap {
  try {
    const saved = JSON.parse(localStorage.getItem(TAB_RUNS_KEY) ?? "{}") as TabRunMap;
    if (!saved || typeof saved !== "object") {
      return {};
    }
    const next: TabRunMap = {};
    for (const [key, value] of Object.entries(saved)) {
      if (value === "new" || (typeof value === "number" && Number.isFinite(value))) {
        next[key] = value;
      }
    }
    return next;
  } catch {
    return {};
  }
}

function saveTabRuns(map: TabRunMap) {
  localStorage.setItem(TAB_RUNS_KEY, JSON.stringify(map));
}

/** Newest matching chat for a tab, or null when that tab has none. */
function runForTab(windowRuns: AgentRun[], tabId: number | null | undefined): AgentRun | null {
  if (tabId == null) {
    return null;
  }
  return (
    windowRuns.find((item) => isTabChat(item) && item.tabId === tabId && isActiveRun(item)) ??
    windowRuns.find((item) => isTabChat(item) && item.tabId === tabId) ??
    null
  );
}

/** Resolve the run to show for the active tab, honoring an explicit per-tab pick. */
function resolveListed(windowRuns: AgentRun[], tabId: number | null | undefined, binding: number | "new" | undefined): AgentRun | null {
  if (binding === "new") {
    return null;
  }
  if (typeof binding === "number") {
    const picked = windowRuns.find((item) => item.id === binding);
    if (picked) {
      return picked;
    }
  }
  return runForTab(windowRuns, tabId);
}

/** Agent runs for this window, the run on show, and the actions the panel offers. */
export function useAgentRun() {
  const { runs } = useAgentRuns();
  const tabs = useTabs();
  const settings = useSettings();
  const label = windowLabel();
  const [tabRuns, setTabRuns] = useState<TabRunMap>(loadTabRuns);
  const [error, setError] = useState<string | null>(null);
  const [details, setDetails] = useState<Record<number, AgentRun>>({});
  const [options, setOptions] = useState<Partial<AgentDraftOptions>>(loadOptions);
  const windowRuns = runs.filter((run) => run.window === label);
  const activeTab = tabs.windowTabs.find((tab) => tab.id === tabs.activeId) ?? null;
  const activeTabId = activeTab?.id ?? null;
  const tabKey = activeTabId == null ? null : String(activeTabId);
  const binding = tabKey == null ? undefined : tabRuns[tabKey];
  const listed = resolveListed(windowRuns, activeTabId, binding);
  const missingSteps = !!listed && listed.steps.length < listed.stepCount;
  const cached = listed ? details[listed.id] : undefined;
  const run = listed && missingSteps && cached ? { ...listed, steps: cached.steps } : listed;
  const draft: AgentDraftOptions = {
    autonomy: options.autonomy ?? settings?.automation.defaultAutonomy ?? "ask",
    model: options.model ?? "",
    access: options.access ?? DEFAULT_ACCESS,
  };

  const bindTab = (tabId: number | null | undefined, value: number | "new") => {
    if (tabId == null) {
      return;
    }
    setTabRuns((previous) => {
      const next = { ...previous, [String(tabId)]: value };
      saveTabRuns(next);
      return next;
    });
  };

  // Drop bindings for tabs that no longer exist, and clear stale run ids.
  const runIds = useRef(new Set(windowRuns.map((item) => item.id)));
  runIds.current = new Set(windowRuns.map((item) => item.id));
  useEffect(() => {
    const openTabs = new Set(tabs.windowTabs.map((tab) => String(tab.id)));
    setTabRuns((previous) => {
      let changed = false;
      const next: TabRunMap = {};
      for (const [key, value] of Object.entries(previous)) {
        if (!openTabs.has(key)) {
          changed = true;
          continue;
        }
        if (value !== "new" && !runIds.current.has(value)) {
          changed = true;
          continue;
        }
        next[key] = value;
      }
      if (!changed && Object.keys(next).length === Object.keys(previous).length) {
        return previous;
      }
      saveTabRuns(next);
      return next;
    });
  }, [tabs.windowTabs, runs]);

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

  const start = async ({
    goal,
    contextTabs = [],
    background = false,
    tabId: preferredTab,
    autonomy,
    instructions,
    attachments,
  }: AgentStartRequest) => {
    setError(null);
    try {
      let tabId = preferredTab;
      let workInBackground = background;
      if (tabId === undefined && background) {
        tabId = await createTab(undefined, true);
      } else if (tabId === undefined) {
        if (activeTab && isWorkableUrl(activeTab.url)) {
          tabId = activeTab.id;
        } else {
          const other = tabs.windowTabs
            .filter((tab) => isWorkableUrl(tab.url) && !tab.automated)
            .sort((a, b) => b.lastActive - a.lastActive)[0];
          if (other) {
            tabId = other.id;
          } else {
            tabId = await createTab(undefined, true);
            workInBackground = true;
          }
        }
      }
      if (tabId === undefined) {
        setError("Open a tab for the agent to work in.");
        return;
      }
      const id = await agentStart(tabId, goal, {
        autonomy: autonomy ?? draft.autonomy,
        model: draft.model,
        background: workInBackground,
        contextTabs: contextTabs.filter((id) => id !== tabId),
        instructions,
        access: { ...draft.access, site: "" },
        attachments,
      });
      // Keep the chat on the tab the user is viewing (e.g. ce://agent), even when work happens elsewhere.
      bindTab(activeTabId ?? tabId, id);
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
    select: (id: number | null) => {
      if (id == null) {
        bindTab(activeTabId, "new");
        return;
      }
      bindTab(activeTabId, id);
    },
    start,
    newTask: () => bindTab(activeTabId, "new"),
    childrenOf: (id: number) => windowRuns.filter((item) => item.parent === id),
    stop: (id: number) => guard(agentStop(id)),
    answer: (id: number, approval: number, allow: boolean) => guard(agentAnswer(id, approval, allow)),
    answerDialog: (id: number, accept: boolean, text = "") => guard(agentAnswerDialog(id, accept, text)),
    reply: (id: number, text: string, attachments?: AgentAttachment[]) => guard(agentReply(id, text, attachments)),
    followUp: (id: number, text: string, attachments?: AgentAttachment[]) =>
      guard(agentContinue(id, text, activeTab?.id, attachments)),
    setAutonomy: (id: number, autonomy: AgentAutonomy) => guard(agentSetAutonomy(id, autonomy)),
    setAccess: (id: number, access: AgentAccess) => guard(agentSetAccess(id, access)),
    updatePlan: (id: number, plan: AgentPlanItem[]) => guard(agentUpdatePlan(id, plan)),
    allowRisk: (id: number, risk: string) => guard(agentAllowRisk(id, risk)),
    cockpit: (id: number, action: "retry" | "skip" | "replan" | "verify") => guard(agentCockpit(id, action)),
    takeOver: (id: number) => guard(agentTakeOver(id)),
    resume: (id: number) => guard(agentResume(id)),
    showTab: (tabId: number) => guard(agentShowTab(tabId)),
    saveFlow: (id: number) => guard(agentSaveFlow(id)),
    deleteRun: (id: number) => {
      setTabRuns((previous) => {
        let changed = false;
        const next: TabRunMap = {};
        for (const [key, value] of Object.entries(previous)) {
          if (value === id) {
            changed = true;
            next[key] = "new";
            continue;
          }
          next[key] = value;
        }
        if (changed) {
          saveTabRuns(next);
          return next;
        }
        return previous;
      });
      guard(agentDeleteRun(id));
    },
    clear: () => {
      setTabRuns(() => {
        const next: TabRunMap = {};
        saveTabRuns(next);
        return next;
      });
      guard(agentClear());
    },
  };
}

export type AgentController = ReturnType<typeof useAgentRun>;
