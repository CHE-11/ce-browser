import { useState } from "react";
import { ArrowDown, ArrowUp, Crosshair, Trash2 } from "lucide-react";
import type { ExtractSpec, FlowStep, FlowTarget, TabInfo } from "@type/browser";
import { Button } from "@/components/Button";
import { IconButton } from "@/components/IconButton";
import { Toggle } from "@/components/Toggle";
import { extractPick } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { inputClassName } from "@features/Agent/agent-styles";
import { STEP_KINDS, stepKindLabel } from "./flow-model";

type StepCardProps = {
  id: string;
  step: FlowStep;
  index: number;
  count: number;
  /** Web pages open in this window, for picking what an extract step reads. */
  pickTabs: TabInfo[];
  onChange: (step: FlowStep) => void;
  onMove: (delta: number) => void;
  onRemove: () => void;
};

const TARGET_KINDS = ["click", "type", "select", "hover", "upload"];

const EMPTY_TARGET: FlowTarget = { role: "", name: "", text: "", testId: "", css: "", xpath: "", tag: "", description: "" };

function Field({ id, label, children, className }: { id: string; label: string; children: React.ReactNode; className?: string }) {
  return (
    <label id={id} className={cn("flex min-w-0 flex-col gap-1 text-xs text-zinc-400", className)}>
      <span id={`${id}-label`}>{label}</span>
      {children}
    </label>
  );
}

function TargetEditor({ id, target, onChange }: { id: string; target: FlowTarget | null; onChange: (target: FlowTarget) => void }) {
  const value = target ?? EMPTY_TARGET;
  const recorded = [value.role, value.name ? `"${value.name}"` : "", value.testId].filter(Boolean).join(" ");
  return (
    <div id={id} className="grid grid-cols-2 gap-2">
      <Field id={`${id}-description`} label="Element, in words (used to find it again)">
        <input
          id={`${id}-description-input`}
          className={inputClassName}
          value={value.description}
          placeholder="the Add to cart button"
          onChange={(event) => onChange({ ...value, description: event.target.value })}
        />
      </Field>
      <Field id={`${id}-css`} label="CSS selector">
        <input
          id={`${id}-css-input`}
          className={cn(inputClassName, "font-mono text-xs")}
          value={value.css}
          placeholder="#search"
          onChange={(event) => onChange({ ...value, css: event.target.value })}
        />
      </Field>
      {recorded ? (
        <p id={`${id}-recorded`} className="col-span-2 truncate text-[11px] text-zinc-500">
          Recorded as {recorded}
        </p>
      ) : null}
    </div>
  );
}

function SpecPicker({ id, spec, pickTabs, onPick }: { id: string; spec: ExtractSpec | null; pickTabs: TabInfo[]; onPick: (spec: ExtractSpec) => void }) {
  const [tabId, setTabId] = useState<number | null>(pickTabs[0]?.id ?? null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const chosen = tabId ?? pickTabs[0]?.id ?? null;

  const pick = () => {
    if (chosen === null) {
      return;
    }
    setBusy(true);
    setError(null);
    void extractPick(chosen)
      .then((picked) => {
        if (picked) {
          onPick(picked.spec);
        }
      })
      .catch((reason: unknown) => setError(String(reason)))
      .finally(() => setBusy(false));
  };

  return (
    <div id={id} className="flex flex-col gap-2">
      {spec ? (
        <p id={`${id}-summary`} className="text-xs text-zinc-300">
          Reads <span className="font-mono text-sky-300">{spec.item}</span> with {spec.fields.map((field) => field.name).join(", ")}.
        </p>
      ) : (
        <p id={`${id}-none`} className="text-xs text-zinc-500">
          Nothing picked yet.
        </p>
      )}
      <div id={`${id}-controls`} className="flex items-center gap-2">
        <select
          id={`${id}-tab`}
          className={cn(inputClassName, "min-w-0 flex-1")}
          value={chosen ?? ""}
          disabled={pickTabs.length === 0}
          onChange={(event) => setTabId(Number(event.target.value))}
        >
          {pickTabs.length === 0 ? <option id={`${id}-tab-none`}>Open the page in a tab first</option> : null}
          {pickTabs.map((tab) => (
            <option id={`${id}-tab-${tab.id}`} key={tab.id} value={tab.id}>
              {tab.title || tab.url}
            </option>
          ))}
        </select>
        <Button id={`${id}-pick`} size="sm" disabled={chosen === null || busy} onClick={pick}>
          <Crosshair id={`${id}-pick-icon`} className="size-3.5" aria-hidden />
          {busy ? "Pick on the page…" : "Pick on page"}
        </Button>
      </div>
      {error ? (
        <p id={`${id}-error`} className="text-xs text-red-400">
          {error}
        </p>
      ) : null}
    </div>
  );
}

export function StepCard({ id, step, index, count, pickTabs, onChange, onMove, onRemove }: StepCardProps) {
  const set = (change: Partial<FlowStep>) => onChange({ ...step, ...change });
  const known = STEP_KINDS.some((item) => item.kind === step.kind);
  const hint = STEP_KINDS.find((item) => item.kind === step.kind)?.hint ?? "This step was made by a newer version of ce-browser and is skipped.";

  return (
    <li id={id} className="flex flex-col gap-3 rounded-lg border border-zinc-800 bg-zinc-900/60 p-3">
      <div id={`${id}-header`} className="flex items-center gap-2">
        <span id={`${id}-number`} className="inline-flex size-6 shrink-0 items-center justify-center rounded-full bg-zinc-800 text-xs text-zinc-300">
          {index + 1}
        </span>
        {known ? (
          <select
            id={`${id}-kind`}
            className={cn(inputClassName, "w-44")}
            value={step.kind}
            onChange={(event) => set({ kind: event.target.value })}
          >
            {STEP_KINDS.map((item) => (
              <option id={`${id}-kind-${item.kind}`} key={item.kind} value={item.kind}>
                {item.label}
              </option>
            ))}
          </select>
        ) : (
          <span id={`${id}-kind-unknown`} className="text-sm text-amber-300">
            {stepKindLabel(step.kind)}
          </span>
        )}
        <span id={`${id}-hint`} className="min-w-0 flex-1 truncate text-xs text-zinc-500">
          {hint}
        </span>
        <label id={`${id}-optional`} className="flex items-center gap-1.5 text-xs text-zinc-400">
          <Toggle id={`${id}-optional-toggle`} size="sm" checked={step.optional} onChange={(optional) => set({ optional })} aria-label="Keep going if this fails" />
          Optional
        </label>
        <IconButton id={`${id}-up`} label="Move up" disabled={index === 0} onClick={() => onMove(-1)}>
          <ArrowUp id={`${id}-up-icon`} aria-hidden />
        </IconButton>
        <IconButton id={`${id}-down`} label="Move down" disabled={index === count - 1} onClick={() => onMove(1)}>
          <ArrowDown id={`${id}-down-icon`} aria-hidden />
        </IconButton>
        <IconButton id={`${id}-remove`} label="Remove step" onClick={onRemove}>
          <Trash2 id={`${id}-remove-icon`} aria-hidden />
        </IconButton>
      </div>

      {TARGET_KINDS.includes(step.kind) ? <TargetEditor id={`${id}-target`} target={step.target} onChange={(target) => set({ target })} /> : null}

      {step.kind === "navigate" ? (
        <Field id={`${id}-url`} label="Address">
          <input id={`${id}-url-input`} className={inputClassName} value={step.url} placeholder="https://example.com/{{page}}" onChange={(event) => set({ url: event.target.value })} />
        </Field>
      ) : null}

      {step.kind === "click" ? (
        <Field id={`${id}-button`} label="Button" className="w-40">
          <select id={`${id}-button-input`} className={inputClassName} value={step.keys || "left"} onChange={(event) => set({ keys: event.target.value === "left" ? "" : event.target.value })}>
            <option id={`${id}-button-left`} value="left">Left</option>
            <option id={`${id}-button-right`} value="right">Right</option>
            <option id={`${id}-button-middle`} value="middle">Middle</option>
          </select>
        </Field>
      ) : null}

      {step.kind === "type" ? (
        <Field id={`${id}-text`} label="Text">
          <textarea id={`${id}-text-input`} rows={2} className={inputClassName} value={step.text} onChange={(event) => set({ text: event.target.value })} />
        </Field>
      ) : null}

      {step.kind === "press" ? (
        <Field id={`${id}-keys`} label="Keys" className="w-64">
          <input id={`${id}-keys-input`} className={inputClassName} value={step.keys} placeholder="Enter, Escape, Control+A" onChange={(event) => set({ keys: event.target.value })} />
        </Field>
      ) : null}

      {step.kind === "select" ? (
        <Field id={`${id}-values`} label="Options, separated by commas">
          <input
            id={`${id}-values-input`}
            className={inputClassName}
            value={step.values.join(", ")}
            onChange={(event) => set({ values: event.target.value.split(",").map((value) => value.trim()).filter(Boolean) })}
          />
        </Field>
      ) : null}

      {step.kind === "upload" ? (
        <Field id={`${id}-files`} label="Files, one full path per line">
          <textarea
            id={`${id}-files-input`}
            rows={2}
            className={cn(inputClassName, "font-mono text-xs")}
            value={step.values.join("\n")}
            onChange={(event) => set({ values: event.target.value.split("\n").map((value) => value.trim()).filter(Boolean) })}
          />
        </Field>
      ) : null}

      {step.kind === "scroll" ? (
        <div id={`${id}-scroll`} className="flex gap-2">
          <Field id={`${id}-direction`} label="Direction" className="w-40">
            <select id={`${id}-direction-input`} className={inputClassName} value={step.text || "down"} onChange={(event) => set({ text: event.target.value })}>
              <option id={`${id}-direction-down`} value="down">Down</option>
              <option id={`${id}-direction-up`} value="up">Up</option>
              <option id={`${id}-direction-left`} value="left">Left</option>
              <option id={`${id}-direction-right`} value="right">Right</option>
            </select>
          </Field>
          <Field id={`${id}-screens`} label="Screens" className="w-28">
            <input id={`${id}-screens-input`} type="number" min={0.1} step={0.5} className={inputClassName} value={step.seconds || 1} onChange={(event) => set({ seconds: Number(event.target.value) })} />
          </Field>
        </div>
      ) : null}

      {step.kind === "wait" ? (
        <Field id={`${id}-seconds`} label="Seconds" className="w-28">
          <input id={`${id}-seconds-input`} type="number" min={0.1} step={0.5} className={inputClassName} value={step.seconds} onChange={(event) => set({ seconds: Number(event.target.value) })} />
        </Field>
      ) : null}

      {step.kind === "waitFor" || step.kind === "assert" ? (
        <div id={`${id}-wait`} className="flex gap-2">
          <Field id={`${id}-wait-text`} label="Text on the page" className="flex-1">
            <input id={`${id}-wait-text-input`} className={inputClassName} value={step.text} onChange={(event) => set({ text: event.target.value })} />
          </Field>
          {step.kind === "waitFor" ? (
            <Field id={`${id}-wait-mode`} label="Until it" className="w-36">
              <select id={`${id}-wait-mode-input`} className={inputClassName} value={step.keys === "gone" ? "gone" : "appears"} onChange={(event) => set({ keys: event.target.value === "gone" ? "gone" : "" })}>
                <option id={`${id}-wait-mode-appears`} value="appears">Appears</option>
                <option id={`${id}-wait-mode-gone`} value="gone">Goes away</option>
              </select>
            </Field>
          ) : null}
          <Field id={`${id}-wait-timeout`} label="Give up after (s)" className="w-32">
            <input id={`${id}-wait-timeout-input`} type="number" min={0} className={inputClassName} value={step.seconds || ""} placeholder={step.kind === "assert" ? "5" : "15"} onChange={(event) => set({ seconds: Number(event.target.value) })} />
          </Field>
        </div>
      ) : null}

      {step.kind === "extract" ? (
        <div id={`${id}-extract`} className="flex flex-col gap-3">
          <SpecPicker id={`${id}-spec`} spec={step.spec} pickTabs={pickTabs} onPick={(spec) => set({ spec })} />
          <div id={`${id}-extract-paging`} className="flex gap-2">
            <Field id={`${id}-pages`} label="Pages to follow" className="w-32">
              <input id={`${id}-pages-input`} type="number" min={1} max={100} className={inputClassName} value={step.pages || 1} onChange={(event) => set({ pages: Number(event.target.value) })} />
            </Field>
            <Field id={`${id}-next`} label="Next link text (blank finds it)" className="flex-1">
              <input id={`${id}-next-input`} className={inputClassName} value={step.text} placeholder="Next" onChange={(event) => set({ text: event.target.value })} />
            </Field>
          </div>
          <Field id={`${id}-extract-prompt`} label="Or describe the data for AI extraction">
            <input
              id={`${id}-extract-prompt-input`}
              className={inputClassName}
              value={step.prompt}
              placeholder="Every product with its name, price and rating"
              onChange={(event) => set({ prompt: event.target.value })}
            />
          </Field>
        </div>
      ) : null}

      {step.kind === "ai" ? (
        <Field id={`${id}-prompt`} label="What the agent should do here">
          <textarea
            id={`${id}-prompt-input`}
            rows={3}
            className={inputClassName}
            value={step.prompt}
            placeholder="Find the cheapest shipping option and choose it"
            onChange={(event) => set({ prompt: event.target.value })}
          />
        </Field>
      ) : null}

      {step.kind === "notify" ? (
        <Field id={`${id}-message`} label="Message. {{output}} is the last AI answer, {{rows}} the rows extracted.">
          <input id={`${id}-message-input`} className={inputClassName} value={step.text} onChange={(event) => set({ text: event.target.value })} />
        </Field>
      ) : null}

      {step.note ? (
        <p id={`${id}-note`} className="text-xs text-amber-300/90">
          {step.note}
        </p>
      ) : null}
    </li>
  );
}
