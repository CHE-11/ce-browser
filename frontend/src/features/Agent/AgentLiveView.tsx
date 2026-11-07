import { MonitorPlay } from "lucide-react";
import type { AgentRun } from "@type/browser";

/** The newest picture of the page the agent is working on. Opens the tab when clicked. */
export function AgentLiveView({ run, onShowTab }: { run: AgentRun; onShowTab: () => void }) {
  const thumb = [...run.steps].reverse().find((step) => step.thumb)?.thumb;
  if (!thumb) {
    return null;
  }

  return (
    <button
      id="agent-live"
      type="button"
      title="Show this tab"
      className="group relative block w-full overflow-hidden rounded-lg ring-1 ring-zinc-800 hover:ring-sky-700"
      onClick={onShowTab}
    >
      <img id="agent-live-image" src={thumb} alt="What the agent sees now" className="block w-full" />
      <span id="agent-live-badge" className="absolute left-2 top-2 inline-flex items-center gap-1 rounded bg-zinc-950/80 px-1.5 py-0.5 text-[10px] font-medium text-sky-300 ring-1 ring-zinc-800">
        <MonitorPlay id="agent-live-badge-icon" className="size-3" aria-hidden />
        Live
      </span>
    </button>
  );
}
