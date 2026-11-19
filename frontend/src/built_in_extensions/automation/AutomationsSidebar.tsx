import type { ReactNode } from "react";
import { Database, FileUp, History, Plus } from "lucide-react";
import type { AutomationsSnapshot, Flow, TabInfo } from "@type/browser";
import { IconButton } from "@/components/IconButton";
import { cn } from "@/lib/cn";
import { focusRow } from "@/lib/focus-ring";
import type { AutomationsView } from "@/lib/use-automations-route";
import type { FlowAction } from "./FlowContextMenu";
import { FlowList } from "./FlowList";
import { RecordControl } from "./RecordControl";

type AutomationsSidebarProps = {
  snapshot: AutomationsSnapshot | null;
  loaded: boolean;
  flows: Flow[];
  view: AutomationsView;
  currentFlowId: string | null;
  draftName: string | null;
  webTabs: TabInfo[];
  recordingTab: TabInfo | undefined;
  recordingTargetName: string | null;
  onView: (view: AutomationsView) => void;
  onOpenFlow: (id: string) => void;
  onNewFlow: () => void;
  onImport: () => void;
  onFlowAction: (flow: Flow, action: FlowAction) => void;
  onRecord: (tabId: number) => void;
  onStopRecording: (save: boolean) => void;
  onShowRecording: () => void;
};

function NavButton({ id, icon, label, current, trailing, onClick }: { id: string; icon: ReactNode; label: string; current: boolean; trailing?: ReactNode; onClick: () => void }) {
  return (
    <button
      id={id}
      type="button"
      aria-current={current ? "page" : undefined}
      className={cn("flex h-8 items-center gap-2 rounded-md px-2 text-left text-sm hover:bg-zinc-800", focusRow, current && "bg-zinc-800")}
      onClick={onClick}
    >
      <span id={`${id}-icon`} aria-hidden className="inline-flex text-zinc-400 [&_svg]:size-4">
        {icon}
      </span>
      <span id={`${id}-label`} className="flex-1">
        {label}
      </span>
      {trailing}
    </button>
  );
}

export function AutomationsSidebar({
  snapshot,
  loaded,
  flows,
  view,
  currentFlowId,
  draftName,
  webTabs,
  recordingTab,
  recordingTargetName,
  onView,
  onOpenFlow,
  onNewFlow,
  onImport,
  onFlowAction,
  onRecord,
  onStopRecording,
  onShowRecording,
}: AutomationsSidebarProps) {
  const runs = snapshot?.runs ?? [];
  const running = runs.filter((run) => run.status === "running").length;
  const failedToday = runs.filter((run) => run.status === "failed" && Date.now() - run.started < 86_400_000).length;

  return (
    <aside id="automations-nav" className="flex w-72 shrink-0 flex-col gap-1 border-r border-zinc-800 p-3">
      <div id="automations-nav-title" className="px-2 pb-2 text-sm font-semibold text-zinc-300">
        Automations
      </div>
      <NavButton
        id="automations-nav-history"
        icon={<History />}
        label="History"
        current={view === "history"}
        onClick={() => onView("history")}
        trailing={
          running ? (
            <span id="automations-nav-running" className="text-xs text-sky-400">
              {running} running
            </span>
          ) : failedToday ? (
            <span id="automations-nav-failed" className="text-xs text-red-400">
              {failedToday} failed today
            </span>
          ) : null
        }
      />
      <NavButton
        id="automations-nav-datasets"
        icon={<Database />}
        label="Datasets"
        current={view === "datasets"}
        onClick={() => onView("datasets")}
        trailing={
          <span id="automations-nav-datasets-count" className="text-xs text-zinc-500">
            {snapshot?.datasets.length ?? 0}
          </span>
        }
      />

      <div id="automations-nav-flows-title" className="mt-3 flex items-center justify-between px-2 pb-1 text-xs font-semibold uppercase tracking-wide text-zinc-500">
        Flows
        <span id="automations-nav-flows-actions" className="flex items-center gap-0.5 normal-case">
          <IconButton id="automations-import" label="Import a flow" className="size-6 [&_svg]:size-3.5" onClick={onImport}>
            <FileUp id="automations-import-icon" aria-hidden />
          </IconButton>
          <button id="automations-new" type="button" className={cn("inline-flex h-6 items-center gap-1 rounded px-1.5 font-medium tracking-normal text-sky-300 hover:bg-zinc-800", focusRow)} onClick={onNewFlow}>
            <Plus id="automations-new-icon" className="size-3.5" aria-hidden />
            New
          </button>
        </span>
      </div>
      {snapshot?.schedulerPaused && flows.some((flow) => !flow.paused && flow.triggers.some((trigger) => trigger.enabled)) ? (
        <p id="automations-scheduler-paused" role="status" className="mx-1 mb-1 rounded-md bg-amber-950/40 px-2 py-1.5 text-xs leading-relaxed text-amber-200">
          {snapshot.schedulerPaused}
        </p>
      ) : null}
      <FlowList
        flows={flows}
        runs={runs}
        nextRuns={snapshot?.nextRuns ?? {}}
        loaded={loaded}
        currentId={view === "flows" ? currentFlowId : null}
        draftName={draftName}
        onOpen={onOpenFlow}
        onAction={onFlowAction}
      />

      <RecordControl
        recording={snapshot?.recording ?? null}
        targetName={recordingTargetName}
        tabs={webTabs}
        recordingTab={recordingTab}
        onRecord={onRecord}
        onStop={onStopRecording}
        onShow={onShowRecording}
      />
    </aside>
  );
}
