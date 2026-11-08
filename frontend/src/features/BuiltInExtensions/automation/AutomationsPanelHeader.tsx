import { Circle, SquareArrowOutUpRight, Workflow, X } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { openAutomations, toggleAutomationPanel } from "@/lib/browser-client";
import { cn } from "@/lib/cn";

export type AutomationsPanelView = "flows" | "recording" | "runs";

type Props = {
  view: AutomationsPanelView;
  flowCount: number;
  runCount: number;
  /** Steps caught so far. Null while nothing is being recorded. */
  recordedSteps: number | null;
  onView: (view: AutomationsPanelView) => void;
};

/** Title bar for the docked automations panel. */
export function AutomationsPanelHeader({ view, flowCount, runCount, recordedSteps, onView }: Props) {
  return (
    <div id="automations-panel-header" className="shrink-0 border-b border-zinc-800">
      <header id="automations-panel-titlebar" className="flex h-[var(--toolbar-height)] items-center gap-1 pl-3 pr-1.5">
        <Workflow id="automations-panel-header-icon" className="size-4 text-rose-400" aria-hidden />
        <h1 id="automations-panel-title" className="ml-1 min-w-0 flex-1 truncate text-sm font-semibold">
          Automations
        </h1>
        <IconButton id="automations-panel-open" label="Open the Automations page" className="size-7" onClick={() => void openAutomations()}>
          <SquareArrowOutUpRight id="automations-panel-open-icon" aria-hidden />
        </IconButton>
        <IconButton id="automations-panel-close" label="Close the automations panel" className="size-7" onClick={() => void toggleAutomationPanel(false)}>
          <X id="automations-panel-close-icon" aria-hidden />
        </IconButton>
      </header>
      <div id="automations-panel-views" className="flex gap-1 px-3 pb-2">
        <ViewButton id="automations-panel-flows" label="Flows" count={flowCount} current={view === "flows"} onClick={() => onView("flows")} />
        <ViewButton
          id="automations-panel-recording"
          label="Recording"
          count={recordedSteps}
          current={view === "recording"}
          live={recordedSteps !== null}
          onClick={() => onView("recording")}
        />
        <ViewButton id="automations-panel-runs" label="Runs" count={runCount} current={view === "runs"} onClick={() => onView("runs")} />
      </div>
    </div>
  );
}

function ViewButton({
  id,
  label,
  count,
  current,
  live,
  onClick,
}: {
  id: string;
  label: string;
  count: number | null;
  current: boolean;
  live?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      id={id}
      type="button"
      aria-current={current ? "page" : undefined}
      className={cn(
        "inline-flex h-7 min-w-0 flex-1 items-center justify-center gap-1 rounded-md px-2 text-xs text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100",
        current && "bg-zinc-800 text-zinc-100",
      )}
      onClick={onClick}
    >
      {live ? <Circle id={`${id}-live`} className="size-2 shrink-0 fill-red-500 text-red-500" aria-hidden /> : null}
      <span id={`${id}-label`} className="truncate">
        {label}
      </span>
      {count !== null && (live || count > 0) ? (
        <span id={`${id}-count`} className="shrink-0 text-[10px] text-zinc-500">
          {count}
        </span>
      ) : null}
    </button>
  );
}
