import { useEffect, useMemo, useRef, useState, type MutableRefObject } from "react";
import type { Flow, FlowRun, FlowRunOptions, TabInfo } from "@type/browser";
import { copyText, flowPlaywright, runFlow, saveFlow, setFlowPaused, stopFlowRun } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRow } from "@/lib/focus-ring";
import { useNow } from "@/lib/use-now";
import { inputClassName } from "@/features/BuiltInExtensions/agent/agent-styles";
import { FlowEditorHeader, type EditorMenuAction } from "./FlowEditorHeader";
import { FlowSettings } from "./FlowSettings";
import { validateFlow, withDefaults } from "./flow-model";
import { pickedTab } from "./PageTabSelect";
import { PlaywrightPanel } from "./PlaywrightPanel";
import { RunHistory } from "./RunHistory";
import type { RunShot } from "./RunShotDialog";
import type { StepRunState } from "./StepCard";
import { StepList, type RunStepMode } from "./StepList";
import { TriggerList } from "./TriggerList";
import { rowsFromVariables, variablesFromRows, type VariableRow } from "./VariablesEditor";

export type FlowEditorTab = "steps" | "triggers" | "settings" | "runs";

/** What the page does for the editor. Each one works on the saved flow. */
export type FlowEditorAction = "duplicate" | "export" | "delete";

type FlowEditorProps = {
  flow: Flow;
  flows: Flow[];
  runs: FlowRun[];
  nextRun: number | undefined;
  /** Why the scheduler isn't firing triggers now. Empty while it runs. */
  schedulerPaused: string;
  /** Web pages open in this window, most recently used first. */
  pickTabs: TabInfo[];
  /** Step to scroll to, from the address. */
  focusStep: number | null;
  /** Section shown when the editor opens. */
  initialTab?: FlowEditorTab;
  /** Filled in by the editor so the page can save before leaving. */
  saveRef: MutableRefObject<(() => Promise<boolean>) | null>;
  onDirtyChange: (dirty: boolean) => void;
  onSaved: (flow: Flow) => void;
  onAction: (flow: Flow, action: FlowEditorAction) => void;
  onRecordMore: (flowId: string, tabId: number) => Promise<void>;
  onOpenFlow: (id: string, step?: number) => void;
  onOpenDataset: (id: string) => void;
  onShowShot: (shot: RunShot) => void;
};

function sameFlow(a: Flow, b: Flow) {
  return JSON.stringify(a) === JSON.stringify(b);
}

export function FlowEditor({
  flow,
  flows,
  runs,
  nextRun,
  schedulerPaused,
  pickTabs,
  focusStep,
  initialTab = "steps",
  saveRef,
  onDirtyChange,
  onSaved,
  onAction,
  onRecordMore,
  onOpenFlow,
  onOpenDataset,
  onShowShot,
}: FlowEditorProps) {
  const now = useNow(30_000);
  const [draft, setDraft] = useState<Flow>(() => withDefaults(flow));
  const [variables, setVariables] = useState<VariableRow[]>(() => rowsFromVariables(flow.variables ?? {}));
  const [dirty, setDirty] = useState(!flow.id);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [script, setScript] = useState<string | null>(null);
  const [runTab, setRunTab] = useState<number | "own">("own");
  const [chosenPickTab, setChosenPickTab] = useState<number | null>(null);
  const [tab, setTab] = useState<FlowEditorTab>(initialTab);

  const dirtyRef = useRef(dirty);
  dirtyRef.current = dirty;

  useEffect(() => {
    if (!dirtyRef.current) {
      setDraft((current) => {
        const next = withDefaults(flow);
        return sameFlow(current, next) ? current : next;
      });
      setVariables(rowsFromVariables(flow.variables ?? {}));
    }
  }, [flow]);

  useEffect(() => {
    onDirtyChange(dirty);
  }, [dirty, onDirtyChange]);

  useEffect(() => () => onDirtyChange(false), [onDirtyChange]);

  useEffect(() => {
    if (focusStep !== null) {
      setTab("steps");
    }
  }, [focusStep]);

  useEffect(() => {
    if (!notice) {
      return;
    }
    const timer = window.setTimeout(() => setNotice(null), 4000);
    return () => window.clearTimeout(timer);
  }, [notice]);

  const stepTabs = useMemo(() => pickTabs.filter((item) => /^(https?|file):/i.test(item.url)), [pickTabs]);
  const runTabStillOpen = runTab === "own" || stepTabs.some((item) => item.id === runTab);
  const chosenRunTab = runTabStillOpen ? runTab : "own";
  const pickTab = pickedTab(stepTabs, typeof chosenRunTab === "number" && chosenPickTab === null ? chosenRunTab : chosenPickTab);

  const ownRuns = useMemo(() => (draft.id ? runs.filter((run) => run.flowId === draft.id) : []), [runs, draft.id]);
  const activeRun = ownRuns.find((run) => run.status === "running");
  const latestRun = useMemo(() => ownRuns.slice().sort((a, b) => b.started - a.started)[0], [ownRuns]);
  const shownRun = activeRun ?? (dirty ? undefined : latestRun);

  const runStates = useMemo(() => {
    const out: Record<number, StepRunState> = {};
    for (const step of shownRun?.steps ?? []) {
      out[step.index] = {
        status: step.status,
        detail: step.detail,
        shot: step.shot && shownRun ? { runId: shownRun.id, index: step.index } : null,
      };
    }
    return out;
  }, [shownRun]);

  const issues = useMemo(() => validateFlow(draft, Object.keys(variablesFromRows(variables))), [draft, variables]);

  const change = (next: Partial<Flow>) => {
    setDraft((current) => ({ ...current, ...next }));
    setDirty(true);
  };

  const fail = (reason: unknown) => {
    setNotice(null);
    setError(String(reason));
  };

  const persist = async () => {
    setError(null);
    const saved = await saveFlow({ ...draft, variables: variablesFromRows(variables) });
    setDraft(withDefaults(saved));
    setVariables(rowsFromVariables(saved.variables));
    setDirty(false);
    dirtyRef.current = false;
    onSaved(saved);
    return saved;
  };

  const savedFlow = async () => (dirtyRef.current || !draft.id ? persist() : draft);

  saveRef.current = async () => {
    try {
      await persist();
      return true;
    } catch (reason) {
      fail(reason);
      return false;
    }
  };

  const save = () => {
    void persist()
      .then(() => setNotice("Saved."))
      .catch(fail);
  };

  const start = (options?: FlowRunOptions, tabId?: number, message = "Running. Each step's result shows up as it goes.") => {
    setNotice(null);
    void (async () => {
      const saved = await savedFlow();
      await runFlow(saved.id, tabId, options);
      setNotice(message);
    })().catch(fail);
  };

  const run = () => start(undefined, chosenRunTab === "own" ? undefined : chosenRunTab);

  const runStep = (index: number, mode: RunStepMode) => {
    const tabId = typeof chosenRunTab === "number" ? chosenRunTab : (pickTab ?? undefined);
    const where = stepTabs.find((item) => item.id === tabId);
    const options: FlowRunOptions = { fromStep: index, toStep: mode === "only" ? index : null, here: tabId !== undefined };
    const what = mode === "only" ? `step ${index + 1}` : `from step ${index + 1}`;
    start(options, tabId, where ? `Running ${what} in ${where.title || where.url}.` : `Running ${what}.`);
  };

  const stop = () => {
    if (activeRun) {
      void stopFlowRun(activeRun.id).catch(fail);
    }
  };

  const togglePause = () => {
    if (draft.id && !dirty) {
      void setFlowPaused(draft.id, !draft.paused)
        .then(() => setNotice(draft.paused ? "Triggers resumed." : "Triggers paused. You can still run it by hand."))
        .catch(fail);
    } else {
      change({ paused: !draft.paused });
    }
  };

  const recordMore = () => {
    const tabId = typeof chosenRunTab === "number" ? chosenRunTab : pickTab;
    if (tabId === null) {
      fail("Open the page to record on in a tab first.");
      return;
    }
    void (async () => {
      const saved = await savedFlow();
      await onRecordMore(saved.id, tabId);
    })().catch(fail);
  };

  const menu = (action: EditorMenuAction) => {
    if (action === "playwright") {
      void (async () => {
        const saved = await savedFlow();
        setScript(await flowPlaywright(saved.id));
      })().catch(fail);
      return;
    }
    if (action === "delete") {
      onAction(draft, "delete");
      return;
    }
    void savedFlow()
      .then((saved) => onAction(saved, action))
      .catch(fail);
  };

  const shortcuts = useRef({ save, run });
  shortcuts.current = { save, run };
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (!event.ctrlKey || event.altKey || event.metaKey || document.querySelector('[role="dialog"]')) {
        return;
      }
      if (event.key === "s" || event.key === "S") {
        event.preventDefault();
        if (dirtyRef.current) {
          shortcuts.current.save();
        }
      } else if (event.key === "Enter") {
        event.preventDefault();
        shortcuts.current.run();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const enabledTriggers = draft.triggers.filter((trigger) => trigger.enabled).length;
  const tabs: { value: FlowEditorTab; label: string; count?: number; warn?: boolean }[] = [
    { value: "steps", label: "Steps", count: draft.steps.length, warn: Object.keys(issues.steps).length > 0 },
    { value: "triggers", label: "Triggers", count: enabledTriggers, warn: Object.keys(issues.triggers).length > 0 },
    { value: "settings", label: "Settings" },
    { value: "runs", label: "Runs", count: ownRuns.length },
  ];
  const watchesPage = draft.triggers.some((trigger) => trigger.kind === "pageChange");

  return (
    <section id="flow-editor" className="flex min-h-0 min-w-0 flex-1 flex-col overflow-y-auto">
      <FlowEditorHeader
        flow={draft}
        dirty={dirty}
        issueCount={issues.count}
        activeRun={activeRun}
        nextRun={nextRun}
        schedulerPaused={schedulerPaused}
        now={now}
        runTab={chosenRunTab}
        tabs={stepTabs}
        onRunTabChange={setRunTab}
        onNameChange={(name) => change({ name })}
        onRun={run}
        onStop={stop}
        onSave={save}
        onTogglePause={togglePause}
        onRecordMore={recordMore}
        onShowIssues={() => setTab(Object.keys(issues.steps).length ? "steps" : "triggers")}
        onMenu={menu}
      />

      <nav id="flow-editor-tabs" role="tablist" aria-label="Flow sections" className="flex flex-wrap gap-1 border-b border-zinc-800 px-3">
        {tabs.map((item) => (
          <button
            id={`flow-editor-tab-${item.value}`}
            key={item.value}
            type="button"
            role="tab"
            aria-selected={tab === item.value}
            aria-controls={`flow-editor-panel-${item.value}`}
            className={cn(
              "-mb-px inline-flex h-9 items-center gap-1.5 border-b-2 px-3 text-sm",
              focusRow,
              tab === item.value ? "border-sky-500 text-zinc-100" : "border-transparent text-zinc-400 hover:text-zinc-200",
            )}
            onClick={() => setTab(item.value)}
          >
            {item.label}
            {item.count ? (
              <span id={`flow-editor-tab-${item.value}-count`} className="rounded-full bg-zinc-800 px-1.5 text-[11px] text-zinc-400">
                {item.count}
              </span>
            ) : null}
            {item.warn ? <span id={`flow-editor-tab-${item.value}-warn`} aria-label="Has problems" className="size-1.5 rounded-full bg-amber-400" /> : null}
          </button>
        ))}
      </nav>

      <div id="flow-editor-body" className="flex flex-col gap-4 px-3 py-5">
        {error ? (
          <p id="flow-editor-error" role="alert" className="flex items-start gap-2 rounded-md bg-red-950/60 px-3 py-2 text-sm text-red-300">
            <span id="flow-editor-error-text" className="min-w-0 flex-1">
              {error}
            </span>
            <button id="flow-editor-error-close" type="button" className="text-xs text-red-200 hover:underline" onClick={() => setError(null)}>
              Dismiss
            </button>
          </p>
        ) : null}
        {notice && !error ? (
          <p id="flow-editor-notice" role="status" className="rounded-md bg-sky-950/60 px-3 py-2 text-sm text-sky-200">
            {notice}
          </p>
        ) : null}

        {tab === "steps" ? (
          <div id="flow-editor-panel-steps" role="tabpanel" aria-labelledby="flow-editor-tab-steps">
            <StepList
              steps={draft.steps}
              issues={issues.steps}
              tabs={stepTabs}
              tabId={pickTab}
              onTabChange={setChosenPickTab}
              runStates={runStates}
              currentIndex={activeRun?.current ?? null}
              busy={Boolean(activeRun)}
              focusStep={focusStep}
              onChange={(steps) => change({ steps })}
              onRunStep={runStep}
              onShowShot={(shot) => onShowShot({ ...shot, label: shownRun?.steps.find((step) => step.index === shot.index)?.label })}
            />
          </div>
        ) : null}

        {tab === "triggers" ? (
          <section id="flow-editor-panel-triggers" role="tabpanel" aria-labelledby="flow-editor-tab-triggers" className="flex flex-col gap-3">
            <p id="flow-editor-triggers-hint" className="text-xs text-zinc-500">
              {draft.paused ? "Triggers are paused. Resume them with the pause button above." : "Each trigger that is on starts the flow by itself."}
            </p>
            <TriggerList id="flow-editor-trigger-list" triggers={draft.triggers} issues={issues.triggers} onChange={(triggers) => change({ triggers })} />
            {watchesPage ? (
              <label id="flow-editor-monitor" className="flex flex-col gap-1 text-xs text-zinc-400">
                Notify only when this is true about the change (AI). Blank notifies on any change of the start address.
                <input
                  id="flow-editor-monitor-input"
                  className={inputClassName}
                  value={draft.monitorPrompt}
                  placeholder="The price drops below $500, or the item is back in stock"
                  onChange={(event) => change({ monitorPrompt: event.target.value })}
                />
              </label>
            ) : null}
          </section>
        ) : null}

        {tab === "settings" ? (
          <div id="flow-editor-panel-settings" role="tabpanel" aria-labelledby="flow-editor-tab-settings">
            <FlowSettings
              flow={draft}
              variables={variables}
              onChange={change}
              onVariablesChange={(rows) => {
                setVariables(rows);
                setDirty(true);
              }}
            />
          </div>
        ) : null}

        {tab === "runs" ? (
          <div id="flow-editor-panel-runs" role="tabpanel" aria-labelledby="flow-editor-tab-runs">
            <RunHistory
              runs={runs}
              flows={flows}
              flowId={draft.id}
              onOpenFlow={(id, step) => {
                if (id === draft.id && step !== undefined) {
                  setTab("steps");
                }
                onOpenFlow(id, step);
              }}
              onOpenDataset={onOpenDataset}
              onRerun={(item) => start({ fromStep: item.fromStep, toStep: item.toStep }, chosenRunTab === "own" ? undefined : chosenRunTab)}
              onShowShot={onShowShot}
            />
          </div>
        ) : null}
      </div>

      <PlaywrightPanel script={script} onCopy={(text) => void copyText(text).then(() => setNotice("Copied the test."))} onClose={() => setScript(null)} />
    </section>
  );
}
