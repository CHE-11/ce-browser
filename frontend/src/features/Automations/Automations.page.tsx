import { useEffect, useMemo, useState } from "react";
import { Circle, Database, History, Plus, Square, Workflow } from "lucide-react";
import type { Flow } from "@type/browser";
import { Button } from "@/components/Button";
import { EmptyState } from "@/components/EmptyState";
import { SkeletonRows } from "@/components/Skeleton";
import { activateTab, startFlowRecording, stopFlowRecording } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRow } from "@/lib/focus-ring";
import { useAppearanceFromSettings, useAutomations, useTabs } from "@/lib/use-browser";
import { inputClassName } from "@features/Agent/agent-styles";
import { DatasetsView } from "./DatasetsView";
import { FlowEditor } from "./FlowEditor";
import { RunHistory } from "./RunHistory";
import { blankFlow, statusTone, triggerSummary } from "./flow-model";

type AutomationsPageProps = {
  params: URLSearchParams;
};

type View = "flows" | "history" | "datasets";

const NEW_FLOW = "new";

function viewFromParams(params: URLSearchParams): View {
  if (params.get("dataset") || params.get("tab") === "datasets") {
    return "datasets";
  }
  if (params.get("tab") === "history") {
    return "history";
  }
  return "flows";
}

export function AutomationsPage({ params }: AutomationsPageProps) {
  useAppearanceFromSettings();
  const { snapshot, loaded } = useAutomations();
  const { windowTabs } = useTabs();
  const [view, setView] = useState<View>(() => viewFromParams(params));
  const [flowId, setFlowId] = useState<string | null>(() => params.get("flow"));
  const [datasetId, setDatasetId] = useState<string | null>(() => params.get("dataset"));
  const [draft, setDraft] = useState<Flow | null>(null);
  const [lastSaved, setLastSaved] = useState<Flow | null>(null);
  const [recordTab, setRecordTab] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const query = params.toString();

  useEffect(() => {
    document.title = "Automations";
  }, []);

  useEffect(() => {
    const next = new URLSearchParams(query);
    setView(viewFromParams(next));
    if (next.get("flow")) {
      setFlowId(next.get("flow"));
      setDraft(null);
    }
    if (next.get("dataset")) {
      setDatasetId(next.get("dataset"));
    }
  }, [query]);

  const webTabs = useMemo(
    () => windowTabs.filter((tab) => /^(https?|file):/i.test(tab.url) && !tab.automated).sort((a, b) => b.lastActive - a.lastActive),
    [windowTabs],
  );
  const chosenRecordTab = webTabs.some((tab) => tab.id === recordTab) ? recordTab : (webTabs[0]?.id ?? null);

  const flows = useMemo(() => (snapshot?.flows ?? []).slice().sort((a, b) => b.updated - a.updated), [snapshot?.flows]);
  const runs = snapshot?.runs ?? [];
  const running = runs.filter((run) => run.status === "running").length;
  const recording = snapshot?.recording ?? null;
  const recordingTab = recording ? windowTabs.find((tab) => tab.id === recording.tabId) : undefined;

  const selectedFlow =
    flowId === NEW_FLOW
      ? draft
      : (flows.find((flow) => flow.id === flowId) ?? (lastSaved && lastSaved.id === flowId ? lastSaved : null));

  const newFlow = () => {
    setDraft(blankFlow(webTabs[0]?.url ?? ""));
    setFlowId(NEW_FLOW);
    setView("flows");
  };

  const openFlow = (id: string) => {
    setFlowId(id);
    setDraft(null);
    setView("flows");
  };

  const openDataset = (id: string | null) => {
    setDatasetId(id);
    setView("datasets");
  };

  const record = () => {
    if (chosenRecordTab === null) {
      return;
    }
    setError(null);
    void startFlowRecording(chosenRecordTab)
      .then(() => activateTab(chosenRecordTab))
      .catch((reason: unknown) => setError(String(reason)));
  };

  const stopRecording = (save: boolean) => {
    setError(null);
    void stopFlowRecording(save)
      .then((id) => {
        if (id) {
          openFlow(id);
        }
      })
      .catch((reason: unknown) => setError(String(reason)));
  };

  return (
    <main id="automations-page" className="flex h-full bg-zinc-950 text-zinc-100">
      <aside id="automations-nav" className="flex w-64 shrink-0 flex-col gap-1 border-r border-zinc-800 p-3">
        <div id="automations-nav-title" className="px-2 pb-2 text-sm font-semibold text-zinc-300">
          Automations
        </div>
        <button
          id="automations-nav-history"
          type="button"
          aria-current={view === "history" ? "page" : undefined}
          className={cn("flex h-8 items-center gap-2 rounded-md px-2 text-left text-sm hover:bg-zinc-800", focusRow, view === "history" && "bg-zinc-800")}
          onClick={() => setView("history")}
        >
          <History id="automations-nav-history-icon" className="size-4 text-zinc-400" aria-hidden />
          <span id="automations-nav-history-label" className="flex-1">
            History
          </span>
          {running ? (
            <span id="automations-nav-running" className="text-xs text-sky-400">
              {running} running
            </span>
          ) : null}
        </button>
        <button
          id="automations-nav-datasets"
          type="button"
          aria-current={view === "datasets" ? "page" : undefined}
          className={cn("flex h-8 items-center gap-2 rounded-md px-2 text-left text-sm hover:bg-zinc-800", focusRow, view === "datasets" && "bg-zinc-800")}
          onClick={() => setView("datasets")}
        >
          <Database id="automations-nav-datasets-icon" className="size-4 text-zinc-400" aria-hidden />
          <span id="automations-nav-datasets-label" className="flex-1">
            Datasets
          </span>
          <span id="automations-nav-datasets-count" className="text-xs text-zinc-500">
            {snapshot?.datasets.length ?? 0}
          </span>
        </button>

        <div id="automations-nav-flows-title" className="mt-3 flex items-center justify-between px-2 pb-1 text-xs font-semibold uppercase tracking-wide text-zinc-500">
          Flows
          <button id="automations-new" type="button" className={cn("inline-flex items-center gap-1 rounded px-1 text-sky-300 hover:bg-zinc-800", focusRow)} onClick={newFlow}>
            <Plus id="automations-new-icon" className="size-3.5" aria-hidden />
            New
          </button>
        </div>
        <div id="automations-flow-list" className="flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto">
          {!loaded ? <SkeletonRows id="automations-loading" rows={3} className="px-2 py-1" /> : null}
          {loaded && flows.length === 0 ? (
            <p id="automations-flows-empty" className="px-2 py-2 text-xs leading-relaxed text-zinc-500">
              No flows yet. Record one on a page, or build it step by step.
            </p>
          ) : null}
          {flowId === NEW_FLOW && draft ? (
            <div id="automations-flow-draft" className="flex h-9 items-center rounded-md bg-zinc-800 px-2 text-sm text-zinc-300 italic">
              {draft.name || "New flow"}
            </div>
          ) : null}
          {flows.map((flow) => {
            const current = view === "flows" && flow.id === flowId;
            const trigger = flow.triggers.find((item) => item.enabled);
            return (
              <button
                id={`automations-flow-${flow.id}`}
                key={flow.id}
                type="button"
                aria-current={current ? "true" : undefined}
                className={cn("flex flex-col rounded-md px-2 py-1.5 text-left hover:bg-zinc-800", focusRow, current && "bg-zinc-800")}
                onClick={() => openFlow(flow.id)}
              >
                <span id={`automations-flow-${flow.id}-name`} className="truncate text-sm text-zinc-200">
                  {flow.name || "Untitled flow"}
                </span>
                <span id={`automations-flow-${flow.id}-meta`} className="flex gap-2 truncate text-xs text-zinc-500">
                  <span id={`automations-flow-${flow.id}-steps`}>{flow.steps.length} steps</span>
                  {trigger ? <span id={`automations-flow-${flow.id}-trigger`}>{triggerSummary(trigger)}</span> : null}
                  {flow.lastStatus ? (
                    <span id={`automations-flow-${flow.id}-status`} className={statusTone(flow.lastStatus)}>
                      {flow.lastStatus}
                    </span>
                  ) : null}
                </span>
              </button>
            );
          })}
        </div>

        <div id="automations-record" className="flex flex-col gap-2 border-t border-zinc-800 pt-3">
          {recording ? (
            <>
              <p id="automations-record-status" className="flex items-center gap-2 px-1 text-xs text-red-300">
                <Circle id="automations-record-dot" className="size-2.5 animate-pulse fill-red-500 text-red-500" aria-hidden />
                Recording {recording.steps.length} steps{recordingTab ? ` on ${recordingTab.title || recordingTab.url}` : ""}
              </p>
              <div id="automations-record-actions" className="flex gap-2">
                <Button id="automations-record-save" size="sm" variant="destructive" className="flex-1" onClick={() => stopRecording(true)}>
                  <Square id="automations-record-save-icon" className="size-3" aria-hidden />
                  Stop and save
                </Button>
                <Button id="automations-record-discard" size="sm" variant="ghost" onClick={() => stopRecording(false)}>
                  Discard
                </Button>
              </div>
            </>
          ) : (
            <>
              <select
                id="automations-record-tab"
                className={inputClassName}
                value={chosenRecordTab ?? ""}
                disabled={webTabs.length === 0}
                onChange={(event) => setRecordTab(Number(event.target.value))}
                aria-label="Page to record on"
              >
                {webTabs.length === 0 ? (
                  <option id="automations-record-tab-none" value="">
                    Open a web page to record on
                  </option>
                ) : null}
                {webTabs.map((tab) => (
                  <option id={`automations-record-tab-${tab.id}`} key={tab.id} value={tab.id}>
                    {tab.title || tab.url}
                  </option>
                ))}
              </select>
              <Button id="automations-record-start" size="sm" variant="outline" disabled={chosenRecordTab === null} onClick={record}>
                <Circle id="automations-record-start-icon" className="size-3 fill-red-500 text-red-500" aria-hidden />
                Record a flow
              </Button>
            </>
          )}
          {error ? (
            <p id="automations-record-error" role="alert" className="px-1 text-xs text-red-300">
              {error}
            </p>
          ) : null}
        </div>
      </aside>

      {view === "history" ? <RunHistory runs={runs} onOpenFlow={openFlow} onOpenDataset={(id) => openDataset(id)} /> : null}
      {view === "datasets" ? <DatasetsView datasets={snapshot?.datasets ?? []} selectedId={datasetId} pickTabs={webTabs} onSelect={openDataset} /> : null}
      {view === "flows" ? (
        selectedFlow ? (
          <FlowEditor
            key={selectedFlow.id || NEW_FLOW}
            flow={selectedFlow}
            pickTabs={webTabs}
            onSaved={(saved) => {
              setLastSaved(saved);
              setFlowId(saved.id);
              setDraft(null);
            }}
            onDeleted={() => {
              setFlowId(null);
              setDraft(null);
            }}
          />
        ) : (
          <section id="automations-welcome" className="flex min-w-0 flex-1 items-center justify-center">
            <EmptyState
              id="automations-welcome-empty"
              icon={Workflow}
              title={flows.length ? "Choose a flow" : "Automate the web"}
              hint="Flows repeat what you do on a page: click, type, read lists, ask AI. They heal themselves when a site changes, and run on a schedule, when a page changes, when you visit a site, or after a download."
              action={
                <Button id="automations-welcome-new" size="sm" variant="primary" onClick={newFlow}>
                  <Plus id="automations-welcome-new-icon" className="size-3.5" aria-hidden />
                  New flow
                </Button>
              }
            />
          </section>
        )
      ) : null}
    </main>
  );
}
