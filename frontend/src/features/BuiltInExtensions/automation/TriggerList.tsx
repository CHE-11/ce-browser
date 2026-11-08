import { Plus, Trash2 } from "lucide-react";
import type { FlowTrigger, FlowTriggerKind } from "@type/browser";
import { IconButton } from "@/components/IconButton";
import { Toggle } from "@/components/Toggle";
import { cn } from "@/lib/cn";
import { inputClassName } from "@/features/BuiltInExtensions/agent/agent-styles";
import { TRIGGER_KINDS, WEEKDAYS, blankTrigger } from "./flow-model";

type TriggerListProps = {
  id: string;
  triggers: FlowTrigger[];
  /** Problems by trigger index. */
  issues?: Record<number, string[]>;
  onChange: (triggers: FlowTrigger[]) => void;
};

type TriggerRowProps = {
  id: string;
  trigger: FlowTrigger;
  issues: string[];
  onChange: (trigger: FlowTrigger) => void;
  onRemove: () => void;
};

function TriggerRow({ id, trigger, issues, onChange, onRemove }: TriggerRowProps) {
  const set = (change: Partial<FlowTrigger>) => onChange({ ...trigger, ...change });
  const known = TRIGGER_KINDS.some((item) => item.kind === trigger.kind);
  return (
    <li id={id} className={cn("flex flex-wrap items-center gap-2 rounded-md border bg-zinc-900/60 px-3 py-2", issues.length ? "border-amber-900/70" : "border-zinc-800", !trigger.enabled && "opacity-60")}>
      <Toggle id={`${id}-enabled`} size="sm" checked={trigger.enabled} onChange={(enabled) => set({ enabled })} aria-label="Trigger on" />
      {known ? (
        <select id={`${id}-kind`} className={cn(inputClassName, "w-52")} value={trigger.kind} onChange={(event) => onChange({ ...blankTrigger(event.target.value as FlowTriggerKind), enabled: trigger.enabled })}>
          {TRIGGER_KINDS.map((item) => (
            <option id={`${id}-kind-${item.kind}`} key={item.kind} value={item.kind}>
              {item.label}
            </option>
          ))}
        </select>
      ) : (
        <span id={`${id}-unknown`} className="text-sm text-amber-300">
          Made by a newer version ({trigger.kind})
        </span>
      )}
      {trigger.kind === "daily" || trigger.kind === "weekly" ? (
        <input id={`${id}-time`} type="time" className={cn(inputClassName, "w-32")} value={trigger.time} onChange={(event) => set({ time: event.target.value })} aria-label="Time" />
      ) : null}
      {trigger.kind === "weekly" ? (
        <div id={`${id}-days`} className="flex gap-1">
          {WEEKDAYS.map((day, index) => {
            const on = trigger.days.includes(index);
            return (
              <button
                id={`${id}-day-${index}`}
                key={day}
                type="button"
                aria-pressed={on}
                className={cn("h-7 w-9 rounded text-xs", on ? "bg-sky-700 text-white" : "bg-zinc-800 text-zinc-400 hover:bg-zinc-700")}
                onClick={() => set({ days: on ? trigger.days.filter((item) => item !== index) : [...trigger.days, index].sort() })}
              >
                {day}
              </button>
            );
          })}
        </div>
      ) : null}
      {trigger.kind === "interval" || trigger.kind === "pageChange" ? (
        <label id={`${id}-minutes`} className="flex items-center gap-2 text-xs text-zinc-400">
          every
          <input
            id={`${id}-minutes-input`}
            type="number"
            min={1}
            className={cn(inputClassName, "w-20")}
            value={trigger.minutes}
            onChange={(event) => set({ minutes: Number(event.target.value) })}
          />
          minutes
        </label>
      ) : null}
      {trigger.kind === "visit" || trigger.kind === "download" ? (
        <input
          id={`${id}-pattern`}
          className={cn(inputClassName, "min-w-48 flex-1")}
          value={trigger.pattern}
          placeholder={trigger.kind === "visit" ? "*.example.com/orders*" : "*.pdf"}
          onChange={(event) => set({ pattern: event.target.value })}
          aria-label={trigger.kind === "visit" ? "Address pattern" : "File name pattern"}
        />
      ) : null}
      {trigger.kind === "hotkey" ? (
        <label id={`${id}-slot`} className="flex items-center gap-2 text-xs text-zinc-400">
          slot
          <select id={`${id}-slot-input`} className={cn(inputClassName, "w-16")} value={trigger.slot} onChange={(event) => set({ slot: Number(event.target.value) })}>
            {[1, 2, 3, 4, 5, 6, 7, 8, 9].map((slot) => (
              <option id={`${id}-slot-${slot}`} key={slot} value={slot}>
                {slot}
              </option>
            ))}
          </select>
          <span id={`${id}-slot-hint`} className="text-zinc-500">
            Bind "Run hotkey flow {trigger.slot}" in ce://shortcuts.
          </span>
        </label>
      ) : null}
      <span id={`${id}-spacer`} className="flex-1" />
      <IconButton id={`${id}-remove`} label="Remove trigger" onClick={onRemove}>
        <Trash2 id={`${id}-remove-icon`} aria-hidden />
      </IconButton>
      {issues.length ? (
        <p id={`${id}-issues`} className="w-full text-xs text-amber-300">
          {issues.join(" ")}
        </p>
      ) : null}
    </li>
  );
}

export function TriggerList({ id, triggers, issues = {}, onChange }: TriggerListProps) {
  return (
    <div id={id} className="flex flex-col gap-2">
      {triggers.length === 0 ? (
        <p id={`${id}-empty`} className="text-xs text-zinc-500">
          Runs only when you start it. Add a trigger to run it on a schedule, on a page change, or from a shortcut.
        </p>
      ) : (
        <ul id={`${id}-items`} className="flex flex-col gap-2">
          {triggers.map((trigger, index) => (
            <TriggerRow
              id={`${id}-${index}`}
              key={index}
              trigger={trigger}
              issues={issues[index] ?? []}
              onChange={(next) => onChange(triggers.map((item, at) => (at === index ? next : item)))}
              onRemove={() => onChange(triggers.filter((_, at) => at !== index))}
            />
          ))}
        </ul>
      )}
      <div id={`${id}-add`} className="flex flex-wrap gap-1.5">
        {TRIGGER_KINDS.map((item) => (
          <button
            id={`${id}-add-${item.kind}`}
            key={item.kind}
            type="button"
            className="inline-flex h-7 items-center gap-1 rounded-md border border-zinc-800 px-2 text-xs text-zinc-300 hover:bg-zinc-800"
            onClick={() => onChange([...triggers, blankTrigger(item.kind)])}
          >
            <Plus id={`${id}-add-${item.kind}-icon`} className="size-3" aria-hidden />
            {item.label}
          </button>
        ))}
      </div>
    </div>
  );
}
