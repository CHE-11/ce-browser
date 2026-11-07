import type { Flow } from "@type/browser";
import { Toggle } from "@/components/Toggle";
import { inputClassName } from "@features/Agent/agent-styles";
import { VariablesEditor, type VariableRow } from "./VariablesEditor";

type FlowSettingsProps = {
  flow: Flow;
  variables: VariableRow[];
  onChange: (change: Partial<Flow>) => void;
  onVariablesChange: (rows: VariableRow[]) => void;
};

export function FlowSettings({ flow, variables, onChange, onVariablesChange }: FlowSettingsProps) {
  return (
    <div id="flow-settings" className="flex flex-col gap-6">
      <section id="flow-editor-basics" className="grid grid-cols-2 gap-3">
        <label id="flow-editor-start" className="flex flex-col gap-1 text-xs text-zinc-400">
          Start address
          <input
            id="flow-editor-start-input"
            className={inputClassName}
            value={flow.startUrl}
            placeholder="Blank runs on the current page"
            onChange={(event) => onChange({ startUrl: event.target.value })}
          />
        </label>
        <label id="flow-editor-description" className="flex flex-col gap-1 text-xs text-zinc-400">
          Description
          <input id="flow-editor-description-input" className={inputClassName} value={flow.description} onChange={(event) => onChange({ description: event.target.value })} />
        </label>
        <label id="flow-editor-background" className="flex items-center gap-2 text-sm text-zinc-300">
          <Toggle id="flow-editor-background-toggle" checked={flow.background} onChange={(background) => onChange({ background })} aria-label="Run in a hidden tab" />
          Run in a hidden tab
        </label>
        <label id="flow-editor-notify" className="flex items-center gap-2 text-sm text-zinc-300">
          <Toggle id="flow-editor-notify-toggle" checked={flow.notify} onChange={(notify) => onChange({ notify })} aria-label="Notify when it finishes" />
          Notify me when it finishes
        </label>
        <label id="flow-editor-paused" className="col-span-2 flex items-center gap-2 text-sm text-zinc-300">
          <Toggle id="flow-editor-paused-toggle" checked={flow.paused} onChange={(paused) => onChange({ paused })} aria-label="Pause triggers" />
          Pause triggers
          <span id="flow-editor-paused-hint" className="text-xs text-zinc-500">
            Schedules, monitors, visits, downloads and shortcuts wait. You can still run it by hand.
          </span>
        </label>
      </section>
      <VariablesEditor id="flow-editor-variables" rows={variables} onChange={onVariablesChange} />
    </div>
  );
}
