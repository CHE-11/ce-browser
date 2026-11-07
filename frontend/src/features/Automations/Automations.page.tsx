import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Plus, Workflow } from "lucide-react";
import type { Flow, FlowRun } from "@type/browser";
import { Button } from "@/components/Button";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EmptyState } from "@/components/EmptyState";
import {
  activateTab,
  deleteFlow,
  duplicateFlow,
  exportFlow,
  importFlow,
  removeRecordedStep,
  runFlow,
  setFlowPaused,
  startFlowRecording,
  stopFlowRecording,
  stopFlowRun,
} from "@/lib/browser-client";
import { NEW_FLOW, useAutomationsRoute, type AutomationsView } from "@/lib/use-automations-route";
import { useAppearanceFromSettings, useAutomations, useTabs } from "@/lib/use-browser";
import { AutomationsSidebar } from "./AutomationsSidebar";
import { DatasetsView } from "./DatasetsView";
import type { FlowAction } from "./FlowContextMenu";
import { FlowEditor, type FlowEditorAction } from "./FlowEditor";
import { blankFlow } from "./flow-model";
import { PageNotice, type Notice } from "./PageNotice";
import { RecordingView } from "./RecordingView";
import { RunHistory } from "./RunHistory";
import { RunShotDialog, type RunShot } from "./RunShotDialog";

type AutomationsPageProps = {
  params: URLSearchParams;
};

export function AutomationsPage({ params }: AutomationsPageProps) {
  useAppearanceFromSettings();
  const { snapshot, loaded } = useAutomations();
  const { windowTabs } = useTabs();
  const dirtyRef = useRef(false);
  const saveRef = useRef<(() => Promise<boolean>) | null>(null);
  const { location, pending, go, confirmLeave, cancelLeave } = useAutomationsRoute(params, () => !dirtyRef.current);
  const [draft, setDraft] = useState<Flow | null>(null);
  const [draftSerial, setDraftSerial] = useState(0);
  const [createdId, setCreatedId] = useState<string | null>(null);
  const [lastSaved, setLastSaved] = useState<Flow | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [deleting, setDeleting] = useState<Flow | null>(null);
  const [shot, setShot] = useState<RunShot | null>(null);

  useEffect(() => {
    document.title = "Automations";
  }, []);

  const webTabs = useMemo(
    () => windowTabs.filter((tab) => /^(https?|file):/i.test(tab.url) && !tab.automated).sort((a, b) => b.lastActive - a.lastActive),
    [windowTabs],
  );
  const flows = useMemo(() => (snapshot?.flows ?? []).slice().sort((a, b) => b.updated - a.updated), [snapshot?.flows]);
  const runs = snapshot?.runs ?? [];
  const recording = snapshot?.recording ?? null;
  const recordingTab = recording ? windowTabs.find((tab) => tab.id === recording.tabId) : undefined;
  const recordingTarget = recording?.flowId ? flows.find((flow) => flow.id === recording.flowId) : undefined;

  const onNew = location.view === "flows" && location.flowId === NEW_FLOW;
  const firstTabUrl = webTabs[0]?.url ?? "";
  useEffect(() => {
    if (onNew && !draft) {
      setDraft(blankFlow(firstTabUrl));
      setDraftSerial((serial) => serial + 1);
    } else if (!onNew && draft) {
      setDraft(null);
    }
  }, [onNew, draft, firstTabUrl]);

  const selectedFlow =
    location.view !== "flows" || !location.flowId
      ? null
      : location.flowId === NEW_FLOW
        ? draft
        : (flows.find((flow) => flow.id === location.flowId) ?? (lastSaved?.id === location.flowId ? lastSaved : null));
  const editorKey = onNew || (createdId !== null && location.flowId === createdId) ? `draft-${draftSerial}` : (location.flowId ?? "");

  const clearNotice = useCallback(() => setNotice(null), []);
  const say = useCallback((text: string) => setNotice({ tone: "info", text }), []);
  const fail = useCallback((reason: unknown) => setNotice({ tone: "error", text: String(reason) }), []);
  const onDirtyChange = useCallback((dirty: boolean) => {
    dirtyRef.current = dirty;
  }, []);

  const showView = (view: AutomationsView) => go({ view });
  const openFlow = (id: string, step?: number) => go({ view: "flows", flowId: id, step: step ?? null });
  const openDataset = (id: string | null) => go({ view: "datasets", datasetId: id });
  const newFlow = () => go({ view: "flows", flowId: NEW_FLOW });

  const runningRun = (flowId: string) => runs.find((run) => run.flowId === flowId && run.status === "running");

  const flowAction = (flow: Flow, action: FlowAction | FlowEditorAction) => {
    const name = flow.name || "the flow";
    switch (action) {
      case "run":
        void runFlow(flow.id)
          .then(() => say(`Running ${name}.`))
          .catch(fail);
        break;
      case "stop": {
        const active = runningRun(flow.id);
        if (active) {
          void stopFlowRun(active.id).catch(fail);
        }
        break;
      }
      case "duplicate":
        void duplicateFlow(flow.id)
          .then((copy) => {
            openFlow(copy.id);
            say(copy.paused ? `Made ${copy.name}. Its triggers are paused so it doesn't run twice.` : `Made ${copy.name}.`);
          })
          .catch(fail);
        break;
      case "export":
        void exportFlow(flow.id, flow.name)
          .then((saved) => saved && say(`Exported ${name}.`))
          .catch(fail);
        break;
      case "pause":
      case "resume":
        void setFlowPaused(flow.id, action === "pause")
          .then(() => say(action === "pause" ? `Paused the triggers of ${name}.` : `Resumed the triggers of ${name}.`))
          .catch(fail);
        break;
      case "delete":
        if (!flow.id) {
          dirtyRef.current = false;
          go({ view: "flows" }, true);
        } else {
          setDeleting(flow);
        }
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
        say(`Deleted ${flow.name || "the flow"}.`);
        if (location.flowId === flow.id) {
          dirtyRef.current = false;
          go({ view: "flows" }, true);
        }
      })
      .catch(fail);
  };

  const importOne = () => {
    void importFlow()
      .then((flow) => {
        if (flow) {
          openFlow(flow.id);
          say(flow.paused ? `Imported ${flow.name}. Its triggers are paused until you resume them.` : `Imported ${flow.name}.`);
        }
      })
      .catch(fail);
  };

  const record = (tabId: number, flowId?: string) =>
    startFlowRecording(tabId, flowId)
      .then(() => activateTab(tabId))
      .catch((reason: unknown) => {
        fail(reason);
        throw reason;
      });

  const stopRecording = (save: boolean) => {
    void stopFlowRecording(save)
      .then((id) => {
        if (id) {
          openFlow(id);
          say("Saved the recorded steps.");
        }
      })
      .catch(fail);
  };

  const rerun = (run: FlowRun) => {
    void runFlow(run.flowId, undefined, { fromStep: run.fromStep, toStep: run.toStep })
      .then(() => say(`Running ${run.flowName || "the flow"} again.`))
      .catch(fail);
  };

  const showRecording = () => {
    if (recording?.flowId) {
      openFlow(recording.flowId);
    } else {
      go({ view: "flows" });
    }
  };

  const leaveWithSave = () => {
    void saveRef.current?.().then((saved) => {
      if (saved) {
        confirmLeave();
      } else {
        cancelLeave();
      }
    });
  };

  const showRecordingView = recording !== null && location.view === "flows" && (!selectedFlow || (recording.flowId !== "" && recording.flowId === selectedFlow.id));

  let main;
  if (location.view === "history") {
    main = <RunHistory runs={runs} flows={flows} onOpenFlow={openFlow} onOpenDataset={openDataset} onRerun={rerun} onShowShot={setShot} />;
  } else if (location.view === "datasets") {
    main = <DatasetsView datasets={snapshot?.datasets ?? []} selectedId={location.datasetId} pickTabs={webTabs} onSelect={openDataset} />;
  } else if (showRecordingView && recording) {
    main = (
      <RecordingView
        recording={recording}
        targetName={recordingTarget ? recordingTarget.name || "Untitled flow" : null}
        tab={recordingTab}
        onShowTab={() => void activateTab(recording.tabId).catch(fail)}
        onRemove={(index) => void removeRecordedStep(index).catch(fail)}
        onStop={stopRecording}
      />
    );
  } else if (selectedFlow) {
    main = (
      <FlowEditor
        key={editorKey}
        flow={selectedFlow}
        flows={flows}
        runs={runs}
        nextRun={snapshot?.nextRuns[selectedFlow.id]}
        schedulerPaused={snapshot?.schedulerPaused ?? ""}
        pickTabs={webTabs}
        focusStep={location.step}
        saveRef={saveRef}
        onDirtyChange={onDirtyChange}
        onSaved={(saved) => {
          setLastSaved(saved);
          dirtyRef.current = false;
          if (onNew) {
            setCreatedId(saved.id);
            go({ view: "flows", flowId: saved.id }, true);
          }
        }}
        onAction={flowAction}
        onRecordMore={(flowId, tabId) => record(tabId, flowId)}
        onOpenFlow={openFlow}
        onOpenDataset={openDataset}
        onShowShot={setShot}
      />
    );
  } else {
    const missing = loaded && location.flowId !== null && location.flowId !== NEW_FLOW;
    main = (
      <section id="automations-welcome" className="flex min-w-0 flex-1 items-center justify-center">
        <EmptyState
          id="automations-welcome-empty"
          icon={Workflow}
          title={missing ? "That flow no longer exists" : flows.length ? "Choose a flow" : "Automate the web"}
          hint="Flows repeat what you do on a page: click, type, read lists, ask AI. They heal themselves when a site changes, and run on a schedule, when a page changes, when you visit a site, or after a download."
          action={
            <Button id="automations-welcome-new" size="sm" variant="primary" onClick={newFlow}>
              <Plus id="automations-welcome-new-icon" className="size-3.5" aria-hidden />
              New flow
            </Button>
          }
        />
      </section>
    );
  }

  const pendingName = selectedFlow?.name || "this flow";

  return (
    <main id="automations-page" className="flex h-full bg-zinc-950 text-zinc-100">
      <AutomationsSidebar
        snapshot={snapshot}
        loaded={loaded}
        flows={flows}
        view={location.view}
        currentFlowId={location.flowId}
        draftName={onNew && draft ? draft.name : null}
        webTabs={webTabs}
        recordingTab={recordingTab}
        recordingTargetName={recordingTarget ? recordingTarget.name || "Untitled flow" : null}
        onView={showView}
        onOpenFlow={(id) => openFlow(id)}
        onNewFlow={newFlow}
        onImport={importOne}
        onFlowAction={flowAction}
        onRecord={(tabId) => void record(tabId).catch(() => undefined)}
        onStopRecording={stopRecording}
        onShowRecording={showRecording}
      />

      {main}

      <ConfirmDialog
        id="automations-leave"
        title="Save your changes?"
        open={pending !== null}
        confirm={{ label: "Save", onClick: leaveWithSave }}
        alternative={{
          label: "Discard",
          variant: "destructive",
          onClick: () => {
            dirtyRef.current = false;
            confirmLeave();
          },
        }}
        onCancel={cancelLeave}
      >
        {selectedFlow?.id ? `${pendingName} has changes that aren't saved.` : "This new flow isn't saved yet."}
      </ConfirmDialog>

      <ConfirmDialog
        id="automations-delete"
        title="Delete this flow?"
        open={deleting !== null}
        confirm={{ label: "Delete", variant: "destructive", onClick: confirmDelete }}
        onCancel={() => setDeleting(null)}
      >
        {deleting ? `${deleting.name || "The flow"} and its triggers are removed. Its past runs stay in History.` : null}
      </ConfirmDialog>

      <RunShotDialog shot={shot} onClose={() => setShot(null)} />
      <PageNotice notice={notice} onClose={clearNotice} />
    </main>
  );
}
