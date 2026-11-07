import { useEffect, useMemo, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { Circle, ExternalLink, Pause, Play, Square, Workflow } from "lucide-react";
import type { Flow, FlowRun, FlowStep, TabInfo } from "@type/browser";
import { Button } from "@/components/Button";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EmptyState } from "@/components/EmptyState";
import { IconButton } from "@/components/IconButton";
import {
  activateTab,
  deleteFlow,
  duplicateFlow,
  exportFlow,
  openAutomations,
  removeRecordedStep,
  resizeAgentPanel,
  runFlow,
  setAgentPanelWidth,
  setFlowPaused,
  startFlowRecording,
  stopFlowRecording,
  stopFlowRun,
} from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRow } from "@/lib/focus-ring";
import { useAppearanceState, useAutomations, useSettings, useTabs } from "@/lib/use-browser";
import { AutomationsPanelHeader, type AutomationsPanelView } from "./AutomationsPanelHeader";
import { FlowContextMenu, type FlowAction } from "./FlowContextMenu";
import { statusDot, stepKindLabel, stepSummary } from "./flow-model";

const MIN_PANEL_WIDTH = 280;
const MAX_PANEL_WIDTH = 720;
const DEFAULT_PANEL_WIDTH = 380;

function clampPanelWidth(width: number) {
  return Math.min(MAX_PANEL_WIDTH, Math.max(MIN_PANEL_WIDTH, Math.round(width)));
}

function pageTab(tab: TabInfo | undefined) {
  return tab && /^(https?|file):/i.test(tab.url) && !tab.automated ? tab : undefined;
}

function when(ms: number) {
  if (!ms) {
    return "";
  }
  return new Date(ms).toLocaleString(undefined, { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" });
}

/** The docked column beside the pages: flows, a live recording, and recent runs. */
export function AutomationsPanelPage() {
  useAppearanceState();
  const settings = useSettings();
  const tabs = useTabs();
  const { snapshot } = useAutomations();
  const width = settings?.agentPanelWidth ?? DEFAULT_PANEL_WIDTH;
  const resizing = useRef(false);
  const [resizingPanel, setResizingPanel] = useState(false);
  const [view, setView] = useState<AutomationsPanelView>("flows");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [notice, setNotice] = useState<string | null>(null);
  const [deleting, setDeleting] = useState<Flow | null>(null);
  const seenRecording = useRef(0);

  const flows = useMemo(() => (snapshot?.flows ?? []).slice().sort((a, b) => b.updated - a.updated), [snapshot?.flows]);
  const runs = snapshot?.runs ?? [];
  const recording = snapshot?.recording ?? null;
  const activeTab = tabs.windowTabs.find((tab) => tab.id === tabs.activeId);
  const recordOn = pageTab(activeTab);
  const recordingTab = recording ? tabs.windowTabs.find((tab) => tab.id === recording.tabId) : undefined;
  const recordingTarget = recording?.flowId ? flows.find((flow) => flow.id === recording.flowId) : undefined;
  const selected = selectedId ? flows.find((flow) => flow.id === selectedId) ?? null : null;

  useEffect(() => {
    document.title = recording ? `Automations · ${recording.steps.length} steps` : "Automations";
  }, [recording]);

  useEffect(() => {
    const started = recording?.started ?? 0;
    if (started && started !== seenRecording.current) {
      setView("recording");
    }
    seenRecording.current = started;
  }, [recording?.started]);

  const onResizePointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || resizing.current) {
      return;
    }
    event.preventDefault();
    resizing.current = true;
    setResizingPanel(true);
    void resizeAgentPanel().finally(() => {
      resizing.current = false;
      setResizingPanel(false);
    });
  };

  const onResizeKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const step = event.shiftKey ? 64 : 16;
    if (event.key === "ArrowLeft") {
      event.preventDefault();
      void setAgentPanelWidth(clampPanelWidth(width + step));
    } else if (event.key === "ArrowRight") {
      event.preventDefault();
      void setAgentPanelWidth(clampPanelWidth(width - step));
    } else if (event.key === "Home") {
      event.preventDefault();
      void setAgentPanelWidth(DEFAULT_PANEL_WIDTH);
    }
  };

  const fail = (reason: unknown) => setNotice(reason instanceof Error ? reason.message : String(reason));

  const flowAction = (flow: Flow, action: FlowAction) => {
    const name = flow.name || "the flow";
    setNotice(null);
    switch (action) {
      case "run":
        void runFlow(flow.id, recordOn?.id)
          .then(() => setNotice(`Running ${name}.`))
          .catch(fail);
        break;
      case "stop": {
        const active = runs.find((run) => run.flowId === flow.id && run.status === "running");
        if (active) {
          void stopFlowRun(active.id).catch(fail);
        }
        break;
      }
      case "duplicate":
        void duplicateFlow(flow.id)
          .then((copy) => {
            setSelectedId(copy.id);
            setView("flows");
            setNotice(`Made ${copy.name || "a copy"}.`);
          })
          .catch(fail);
        break;
      case "export":
        void exportFlow(flow.id, flow.name)
          .then((saved) => saved && setNotice(`Exported ${name}.`))
          .catch(fail);
        break;
      case "pause":
      case "resume":
        void setFlowPaused(flow.id, action === "pause")
          .then(() => setNotice(action === "pause" ? `Paused the triggers of ${name}.` : `Resumed the triggers of ${name}.`))
          .catch(fail);
        break;
      case "delete":
        setDeleting(flow);
        break;
    }
  };

  const confirmDelete = () => {
    const flow = deleting;
    setDeleting(null);
    if (!flow) {
      return;
    }
    void deleteFlow(flow.id)
      .then(() => {
        if (selectedId === flow.id) {
          setSelectedId(null);
        }
        setNotice(`Deleted ${flow.name || "the flow"}.`);
      })
      .catch(fail);
  };

  const record = (flowId?: string) => {
    if (!recordOn) {
      setNotice("Open a web page to record on.");
      return;
    }
    setNotice(null);
    void startFlowRecording(recordOn.id, flowId).catch(fail);
  };

  const stop = (save: boolean) => {
    setNotice(null);
    void stopFlowRecording(save)
      .then((id) => {
        if (save && id) {
          setSelectedId(id);
          setView("flows");
          setNotice("Saved the recorded steps.");
        } else if (save) {
          setNotice("Nothing was recorded.");
        } else {
          setView("flows");
        }
      })
      .catch(fail);
  };

  return (
    <div
      id="automations-panel"
      className={cn("relative flex h-screen flex-col border-l border-zinc-800 bg-zinc-950 text-zinc-100", resizingPanel && "cursor-col-resize select-none")}
      onContextMenu={(event) => event.preventDefault()}
    >
      <div
        id="automations-panel-resize"
        role="separator"
        aria-orientation="vertical"
        aria-label="Resize the automations panel"
        aria-valuemin={MIN_PANEL_WIDTH}
        aria-valuemax={MAX_PANEL_WIDTH}
        aria-valuenow={width}
        tabIndex={0}
        title="Drag to resize. Left and right arrows change the width."
        className={cn(
          "absolute inset-y-0 left-0 z-20 w-1.5 cursor-col-resize touch-none hover:bg-rose-500/40 focus-visible:bg-rose-500/40 focus-visible:outline-none",
          resizingPanel && "bg-rose-500/50",
        )}
        onPointerDown={onResizePointerDown}
        onKeyDown={onResizeKeyDown}
      />
      <AutomationsPanelHeader
        view={view}
        flowCount={flows.length}
        runCount={runs.length}
        recordedSteps={recording ? recording.steps.length : null}
        onView={(next) => {
          setNotice(null);
          if (next === "flows") {
            setSelectedId(null);
          }
          setView(next);
        }}
      />

      {view === "recording" ? (
        <RecordingBody
          steps={recording?.steps ?? []}
          recording={recording !== null}
          targetName={recordingTarget ? recordingTarget.name || "Untitled flow" : null}
          tab={recordingTab}
          onRemove={(index) => void removeRecordedStep(index).catch(fail)}
          onShowTab={() => {
            if (recording) {
              void activateTab(recording.tabId).catch(fail);
            }
          }}
        />
      ) : view === "runs" ? (
        <RunsBody
          runs={runs}
          onOpen={(flowId) => {
            setSelectedId(flowId);
            setView("flows");
          }}
          onStop={(id) => void stopFlowRun(id).catch(fail)}
        />
      ) : selected ? (
        <FlowBody
          flow={selected}
          running={runs.find((run) => run.flowId === selected.id && run.status === "running")}
          canRecord={recordOn !== undefined && recording === null}
          onBack={() => setSelectedId(null)}
          onRun={() => {
            setNotice(null);
            void runFlow(selected.id, recordOn?.id)
              .then(() => setNotice(`Running ${selected.name || "the flow"}.`))
              .catch(fail);
          }}
          onRecord={() => record(selected.id)}
          onPause={() => void setFlowPaused(selected.id, !selected.paused).catch(fail)}
          onOpen={() => void openAutomations(`flow=${selected.id}`)}
          onStop={(id) => void stopFlowRun(id).catch(fail)}
        />
      ) : (
        <FlowIndex flows={flows} runs={runs} query={query} onQuery={setQuery} onOpen={setSelectedId} onAction={flowAction} />
      )}

      <footer id="automations-panel-footer" className="flex shrink-0 flex-col gap-2 border-t border-zinc-800 p-3">
        {notice ? (
          <p id="automations-panel-notice" role="status" className="text-xs text-zinc-400">
            {notice}
          </p>
        ) : null}
        {recording ? (
          <div id="automations-panel-recording-actions" className="flex gap-2">
            <Button
              id="automations-panel-undo"
              size="sm"
              variant="outline"
              disabled={recording.steps.length === 0}
              onClick={() => void removeRecordedStep(recording.steps.length - 1).catch(fail)}
            >
              Undo
            </Button>
            <Button id="automations-panel-discard" size="sm" variant="ghost" onClick={() => stop(false)}>
              Discard
            </Button>
            <Button id="automations-panel-save" size="sm" variant="destructive" className="flex-1" onClick={() => stop(true)}>
              <Square id="automations-panel-save-icon" className="size-3" aria-hidden />
              {recording.steps.length ? `Stop and save ${recording.steps.length}` : "Stop"}
            </Button>
          </div>
        ) : (
          <Button id="automations-panel-record" size="sm" variant="outline" disabled={!recordOn} onClick={() => record()}>
            <Circle id="automations-panel-record-icon" className="size-3 fill-red-500 text-red-500" aria-hidden />
            {recordOn ? `Record on ${recordOn.title || "this page"}` : "Open a web page to record"}
          </Button>
        )}
      </footer>
      <ConfirmDialog
        id="automations-panel-delete"
        title="Delete this flow?"
        open={deleting !== null}
        onCancel={() => setDeleting(null)}
        confirm={{ label: "Delete", variant: "destructive", onClick: confirmDelete }}
      >
        {deleting ? `${deleting.name || "This flow"} and its steps will be removed.` : ""}
      </ConfirmDialog>
    </div>
  );
}

function FlowIndex({
  flows,
  runs,
  query,
  onQuery,
  onOpen,
  onAction,
}: {
  flows: Flow[];
  runs: FlowRun[];
  query: string;
  onQuery: (query: string) => void;
  onOpen: (id: string) => void;
  onAction: (flow: Flow, action: FlowAction) => void;
}) {
  const [menu, setMenu] = useState<{ flow: Flow; x: number; y: number } | null>(null);
  const words = query.trim().toLowerCase();
  const shown = words ? flows.filter((flow) => `${flow.name} ${flow.description} ${flow.startUrl}`.toLowerCase().includes(words)) : flows;
  const menuRunning = menu ? runs.some((run) => run.flowId === menu.flow.id && run.status === "running") : false;
  return (
    <div id="automations-panel-flows-view" className="flex min-h-0 flex-1 flex-col">
      <div id="automations-panel-search-row" className="px-3 pt-3">
        <input
          id="automations-panel-search"
          value={query}
          placeholder="Search flows"
          aria-label="Search flows"
          className="h-8 w-full rounded-md bg-zinc-900 px-2.5 text-sm text-zinc-100 ring-1 ring-zinc-800 outline-none placeholder:text-zinc-600 focus:ring-rose-500"
          onChange={(event) => onQuery(event.target.value)}
        />
      </div>
      <div id="automations-panel-flow-list" className="min-h-0 flex-1 overflow-y-auto px-2 py-2">
        {shown.length === 0 ? (
          <EmptyState
            id="automations-panel-flows-empty"
            icon={Workflow}
            title={flows.length === 0 ? "No flows yet" : "No matching flows"}
            hint={flows.length === 0 ? "Record what you do on a page. The steps show up here, then you can run them again." : "Try a different name or address."}
          />
        ) : (
          shown.map((flow) => {
            const running = runs.find((run) => run.flowId === flow.id && run.status === "running");
            return (
              <button
                id={`automations-panel-flow-${flow.id}`}
                key={flow.id}
                type="button"
                className={cn("flex w-full items-start gap-2 rounded-md px-2 py-1.5 text-left hover:bg-zinc-900", focusRow)}
                onClick={() => onOpen(flow.id)}
                onContextMenu={(event) => {
                  event.preventDefault();
                  setMenu({ flow, x: event.clientX, y: event.clientY });
                }}
              >
                <span
                  id={`automations-panel-flow-${flow.id}-dot`}
                  aria-hidden
                  className={cn("mt-1.5 size-2 shrink-0 rounded-full", statusDot(running ? "running" : flow.lastStatus), running && "animate-pulse")}
                />
                <span id={`automations-panel-flow-${flow.id}-text`} className="min-w-0 flex-1">
                  <span id={`automations-panel-flow-${flow.id}-name`} className="block truncate text-sm text-zinc-200">
                    {flow.name || "Untitled flow"}
                  </span>
                  <span id={`automations-panel-flow-${flow.id}-meta`} className="block truncate text-xs text-zinc-500">
                    {running ? "Running" : flow.paused ? "Paused" : flow.lastStatus ? `Last run ${flow.lastStatus}` : "Never run"}
                    {" · "}
                    {flow.steps.length} {flow.steps.length === 1 ? "step" : "steps"}
                  </span>
                </span>
              </button>
            );
          })
        )}
      </div>
      {menu ? (
        <FlowContextMenu
          flow={menu.flow}
          running={menuRunning}
          x={menu.x}
          y={menu.y}
          scrollParentId="automations-panel-flow-list"
          onAction={(action) => onAction(menu.flow, action)}
          onClose={() => setMenu(null)}
        />
      ) : null}
    </div>
  );
}

function FlowBody({
  flow,
  running,
  canRecord,
  onBack,
  onRun,
  onRecord,
  onPause,
  onOpen,
  onStop,
}: {
  flow: Flow;
  running: FlowRun | undefined;
  canRecord: boolean;
  onBack: () => void;
  onRun: () => void;
  onRecord: () => void;
  onPause: () => void;
  onOpen: () => void;
  onStop: (id: number) => void;
}) {
  return (
    <div id="automations-panel-flow" className="flex min-h-0 flex-1 flex-col">
      <div id="automations-panel-flow-bar" className="flex items-center gap-2 px-3 pt-3">
        <Button id="automations-panel-flow-back" size="sm" variant="ghost" onClick={onBack}>
          Flows
        </Button>
        <h2 id="automations-panel-flow-name" className="min-w-0 flex-1 truncate text-sm font-semibold">
          {flow.name || "Untitled flow"}
        </h2>
      </div>
      <p id="automations-panel-flow-meta" className="truncate px-3 pt-1 text-xs text-zinc-500">
        {flow.steps.length} {flow.steps.length === 1 ? "step" : "steps"}
        {flow.startUrl ? ` · ${flow.startUrl}` : ""}
      </p>
      <div id="automations-panel-flow-actions" className="flex flex-wrap gap-2 px-3 py-3">
        {running ? (
          <Button id="automations-panel-flow-stop" size="sm" variant="destructiveOutline" onClick={() => onStop(running.id)}>
            <Square id="automations-panel-flow-stop-icon" className="size-3" aria-hidden />
            Stop
          </Button>
        ) : (
          <Button id="automations-panel-flow-run" size="sm" variant="primary" onClick={onRun}>
            <Play id="automations-panel-flow-run-icon" className="size-3.5" aria-hidden />
            Run
          </Button>
        )}
        <Button id="automations-panel-flow-record" size="sm" variant="outline" disabled={!canRecord} onClick={onRecord}>
          <Circle id="automations-panel-flow-record-icon" className="size-3 fill-red-500 text-red-500" aria-hidden />
          Record more
        </Button>
        <Button id="automations-panel-flow-pause" size="sm" variant="ghost" onClick={onPause}>
          <Pause id="automations-panel-flow-pause-icon" className="size-3.5" aria-hidden />
          {flow.paused ? "Resume schedule" : "Pause schedule"}
        </Button>
        <Button id="automations-panel-flow-edit" size="sm" variant="ghost" onClick={onOpen}>
          <ExternalLink id="automations-panel-flow-edit-icon" className="size-3.5" aria-hidden />
          Edit
        </Button>
      </div>
      {running ? (
        <p id="automations-panel-flow-running" className="px-3 pb-2 text-xs text-sky-400">
          {running.currentLabel || "Starting…"}
        </p>
      ) : null}
      <ol id="automations-panel-flow-steps" className="min-h-0 flex-1 overflow-y-auto px-3 pb-3">
        {flow.steps.length === 0 ? (
          <li id="automations-panel-flow-steps-empty" className="text-sm text-zinc-500">
            This flow has no steps yet. Record more to add some.
          </li>
        ) : (
          flow.steps.map((step, index) => <StepRow key={step.id || index} id={`automations-panel-flow-step-${index}`} index={index} step={step} />)
        )}
      </ol>
    </div>
  );
}

function RecordingBody({
  steps,
  recording,
  targetName,
  tab,
  onRemove,
  onShowTab,
}: {
  steps: FlowStep[];
  recording: boolean;
  targetName: string | null;
  tab: TabInfo | undefined;
  onRemove: (index: number) => void;
  onShowTab: () => void;
}) {
  const listRef = useRef<HTMLOListElement>(null);
  useEffect(() => {
    listRef.current?.lastElementChild?.scrollIntoView({ block: "nearest" });
  }, [steps.length]);

  return (
    <div id="automations-panel-recording-view" className="flex min-h-0 flex-1 flex-col">
      <div id="automations-panel-recording-status" className="flex items-start gap-2 px-3 pt-3">
        <Circle
          id="automations-panel-recording-dot"
          className={cn("mt-1 size-2.5 shrink-0", recording ? "animate-pulse fill-red-500 text-red-500" : "text-zinc-600")}
          aria-hidden
        />
        <div id="automations-panel-recording-heading" className="min-w-0 flex-1">
          <p id="automations-panel-recording-title" className="text-sm font-medium text-zinc-100">
            {recording ? (targetName ? `Recording into ${targetName}` : "Recording a new flow") : "Not recording"}
          </p>
          <p id="automations-panel-recording-tab" className="truncate text-xs text-zinc-500">
            {recording
              ? tab
                ? `On ${tab.title || tab.url}. Clicks, typing, and Enter show up here.`
                : "The recorded tab is closed."
              : "Start from the button below, or from the record icon on the toolbar."}
          </p>
        </div>
        {tab ? (
          <IconButton id="automations-panel-recording-show" label="Go to the recorded page" className="size-7" onClick={onShowTab}>
            <ExternalLink id="automations-panel-recording-show-icon" aria-hidden />
          </IconButton>
        ) : null}
      </div>
      <div id="automations-panel-recording-body" className="min-h-0 flex-1 overflow-y-auto px-3 py-3">
        {steps.length === 0 ? (
          <p id="automations-panel-recording-empty" className="text-sm text-zinc-500">
            {recording ? "Nothing yet. Click, type, or press Enter on the page." : "Steps you record will be listed here as you go."}
          </p>
        ) : (
          <ol id="automations-panel-recording-steps" ref={listRef} className="flex flex-col gap-1.5">
            {steps.map((step, index) => (
              <StepRow key={`${index}-${step.kind}`} id={`automations-panel-recorded-${index}`} index={index} step={step} onRemove={() => onRemove(index)} />
            ))}
          </ol>
        )}
      </div>
    </div>
  );
}

function RunsBody({ runs, onOpen, onStop }: { runs: FlowRun[]; onOpen: (flowId: string) => void; onStop: (id: number) => void }) {
  const recent = runs.slice(0, 40);
  return (
    <div id="automations-panel-runs" className="min-h-0 flex-1 overflow-y-auto px-2 py-2">
      {recent.length === 0 ? (
        <EmptyState id="automations-panel-runs-empty" icon={Workflow} title="No runs yet" hint="Run a flow from the list. What happened shows up here." />
      ) : (
        recent.map((run) => (
          <div id={`automations-panel-run-${run.id}`} key={run.id} className="flex items-center gap-1 rounded-md px-1 hover:bg-zinc-900">
            <button
              id={`automations-panel-run-${run.id}-open`}
              type="button"
              className={cn("flex min-w-0 flex-1 items-start gap-2 px-1 py-1.5 text-left", focusRow)}
              onClick={() => onOpen(run.flowId)}
            >
              <span id={`automations-panel-run-${run.id}-dot`} aria-hidden className={cn("mt-1.5 size-2 shrink-0 rounded-full", statusDot(run.status), run.status === "running" && "animate-pulse")} />
              <span id={`automations-panel-run-${run.id}-text`} className="min-w-0 flex-1">
                <span id={`automations-panel-run-${run.id}-name`} className="block truncate text-sm text-zinc-200">
                  {run.flowName || "Untitled flow"}
                </span>
                <span id={`automations-panel-run-${run.id}-meta`} className="block truncate text-xs text-zinc-500">
                  {run.status}
                  {run.currentLabel ? ` · ${run.currentLabel}` : ""}
                  {run.started ? ` · ${when(run.started)}` : ""}
                </span>
              </span>
            </button>
            {run.status === "running" ? (
              <IconButton id={`automations-panel-run-${run.id}-stop`} label="Stop this run" className="size-7" onClick={() => onStop(run.id)}>
                <Square id={`automations-panel-run-${run.id}-stop-icon`} aria-hidden />
              </IconButton>
            ) : null}
          </div>
        ))
      )}
    </div>
  );
}

function StepRow({ id, index, step, onRemove }: { id: string; index: number; step: FlowStep; onRemove?: () => void }) {
  return (
    <li id={id} className="flex items-start gap-2 rounded-md border border-zinc-800 bg-zinc-900/60 px-2 py-1.5">
      <span id={`${id}-number`} className="inline-flex size-5 shrink-0 items-center justify-center rounded-full bg-zinc-800 text-[10px] text-zinc-300">
        {index + 1}
      </span>
      <span id={`${id}-text`} className="min-w-0 flex-1">
        <span id={`${id}-kind`} className="block text-[10px] uppercase tracking-wide text-zinc-500">
          {stepKindLabel(step.kind)}
        </span>
        <span id={`${id}-summary`} className="block truncate text-sm text-zinc-200">
          {stepSummary(step)}
        </span>
        {step.note ? (
          <span id={`${id}-note`} className="block truncate text-xs text-amber-300/90">
            {step.note}
          </span>
        ) : null}
      </span>
      {onRemove ? (
        <button id={`${id}-remove`} type="button" className="shrink-0 text-xs text-zinc-500 hover:text-zinc-200" onClick={onRemove}>
          Remove
        </button>
      ) : null}
    </li>
  );
}
