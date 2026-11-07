import { useEffect, useMemo, useRef, useState } from "react";
import { Code2, Copy, Play, Plus, Save, Trash2 } from "lucide-react";
import type { Flow, FlowStepKind, TabInfo } from "@type/browser";
import { Button } from "@/components/Button";
import { Toggle } from "@/components/Toggle";
import { copyText, deleteFlow, flowPlaywright, runFlow, saveFlow } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { inputClassName } from "@features/Agent/agent-styles";
import { STEP_KINDS, blankStep, formatWhen, statusTone, withDefaults } from "./flow-model";
import { StepCard } from "./StepCard";
import { TriggerList } from "./TriggerList";

type FlowEditorProps = {
  flow: Flow;
  /** Web pages open in this window. */
  pickTabs: TabInfo[];
  onSaved: (flow: Flow) => void;
  onDeleted: () => void;
};

function variablesText(variables: Record<string, string>) {
  return Object.entries(variables)
    .map(([key, value]) => `${key}=${value}`)
    .join("\n");
}

function parseVariables(text: string) {
  const out: Record<string, string> = {};
  for (const line of text.split("\n")) {
    const at = line.indexOf("=");
    if (at <= 0) {
      continue;
    }
    const key = line.slice(0, at).trim();
    if (key && !key.startsWith("secret:")) {
      out[key] = line.slice(at + 1).trim();
    }
  }
  return out;
}

export function FlowEditor({ flow, pickTabs, onSaved, onDeleted }: FlowEditorProps) {
  const [draft, setDraft] = useState<Flow>(() => withDefaults(flow));
  const [variables, setVariables] = useState(() => variablesText(flow.variables ?? {}));
  const [dirty, setDirty] = useState(!flow.id);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [script, setScript] = useState<string | null>(null);
  const [runTab, setRunTab] = useState<number | "own">("own");
  const [addKind, setAddKind] = useState<FlowStepKind>("click");

  const dirtyRef = useRef(dirty);
  dirtyRef.current = dirty;

  useEffect(() => {
    if (!dirtyRef.current) {
      setDraft(withDefaults(flow));
      setVariables(variablesText(flow.variables ?? {}));
    }
  }, [flow]);

  const watchesPage = draft.triggers.some((trigger) => trigger.kind === "pageChange");
  const change = (next: Partial<Flow>) => {
    setDraft((current) => ({ ...current, ...next }));
    setDirty(true);
  };

  const steps = draft.steps;
  const moveStep = (index: number, delta: number) => {
    const target = index + delta;
    if (target < 0 || target >= steps.length) {
      return;
    }
    const next = [...steps];
    [next[index], next[target]] = [next[target], next[index]];
    change({ steps: next });
  };

  const persist = async () => {
    setError(null);
    const saved = await saveFlow({ ...draft, variables: parseVariables(variables) });
    setDraft(withDefaults(saved));
    setDirty(false);
    onSaved(saved);
    return saved;
  };

  const save = () => {
    void persist()
      .then(() => setNotice("Saved."))
      .catch((reason: unknown) => setError(String(reason)));
  };

  const run = () => {
    setNotice(null);
    void (async () => {
      const saved = dirty || !draft.id ? await persist() : draft;
      await runFlow(saved.id, runTab === "own" ? undefined : runTab);
      setNotice("Running. Follow it under History.");
    })().catch((reason: unknown) => setError(String(reason)));
  };

  const exportScript = () => {
    void (async () => {
      const saved = dirty || !draft.id ? await persist() : draft;
      setScript(await flowPlaywright(saved.id));
    })().catch((reason: unknown) => setError(String(reason)));
  };

  const remove = () => {
    if (!draft.id) {
      onDeleted();
      return;
    }
    void deleteFlow(draft.id)
      .then(onDeleted)
      .catch((reason: unknown) => setError(String(reason)));
  };

  const stepTabs = useMemo(() => pickTabs.filter((tab) => /^(https?|file):/i.test(tab.url)), [pickTabs]);

  return (
    <section id="flow-editor" className="flex min-w-0 flex-1 flex-col overflow-y-auto">
      <header id="flow-editor-header" className="sticky top-0 z-10 flex items-center gap-2 border-b border-zinc-800 bg-zinc-950/95 px-6 py-3 backdrop-blur">
        <input
          id="flow-editor-name"
          className="min-w-0 flex-1 bg-transparent text-lg font-semibold text-zinc-100 outline-none placeholder:text-zinc-600"
          value={draft.name}
          placeholder="Untitled flow"
          onChange={(event) => change({ name: event.target.value })}
          aria-label="Flow name"
        />
        {draft.lastRun ? (
          <span id="flow-editor-last" className={cn("text-xs", statusTone(draft.lastStatus))}>
            Last run {formatWhen(draft.lastRun)} · {draft.lastStatus}
          </span>
        ) : null}
        <select
          id="flow-editor-run-tab"
          className={cn(inputClassName, "w-48")}
          value={runTab}
          onChange={(event) => setRunTab(event.target.value === "own" ? "own" : Number(event.target.value))}
          aria-label="Where to run"
        >
          <option id="flow-editor-run-tab-own" value="own">
            {draft.background ? "In a hidden tab" : "In a new tab"}
          </option>
          {stepTabs.map((tab) => (
            <option id={`flow-editor-run-tab-${tab.id}`} key={tab.id} value={tab.id}>
              In {tab.title || tab.url}
            </option>
          ))}
        </select>
        <Button id="flow-editor-run" size="sm" variant="primary" onClick={run}>
          <Play id="flow-editor-run-icon" className="size-3.5" aria-hidden />
          Run
        </Button>
        <Button id="flow-editor-save" size="sm" disabled={!dirty} onClick={save}>
          <Save id="flow-editor-save-icon" className="size-3.5" aria-hidden />
          Save
        </Button>
        <Button id="flow-editor-export" size="sm" variant="ghost" onClick={exportScript}>
          <Code2 id="flow-editor-export-icon" className="size-3.5" aria-hidden />
          Playwright
        </Button>
        <Button id="flow-editor-delete" size="sm" variant="ghost" onClick={remove}>
          <Trash2 id="flow-editor-delete-icon" className="size-3.5" aria-hidden />
          {draft.id ? "Delete" : "Discard"}
        </Button>
      </header>

      <div id="flow-editor-body" className="flex flex-col gap-6 px-6 py-5">
        {error ? (
          <p id="flow-editor-error" role="alert" className="rounded-md bg-red-950/60 px-3 py-2 text-sm text-red-300">
            {error}
          </p>
        ) : null}
        {notice && !error ? (
          <p id="flow-editor-notice" role="status" className="rounded-md bg-sky-950/60 px-3 py-2 text-sm text-sky-200">
            {notice}
          </p>
        ) : null}

        {script !== null ? (
          <section id="flow-editor-script" className="flex flex-col gap-2">
            <div id="flow-editor-script-header" className="flex items-center justify-between">
              <h2 id="flow-editor-script-title" className="text-sm font-semibold text-zinc-200">
                Playwright test
              </h2>
              <div id="flow-editor-script-actions" className="flex gap-2">
                <Button id="flow-editor-script-copy" size="sm" onClick={() => void copyText(script).then(() => setNotice("Copied the test."))}>
                  <Copy id="flow-editor-script-copy-icon" className="size-3.5" aria-hidden />
                  Copy
                </Button>
                <Button id="flow-editor-script-close" size="sm" variant="ghost" onClick={() => setScript(null)}>
                  Close
                </Button>
              </div>
            </div>
            <pre id="flow-editor-script-code" className="max-h-80 overflow-auto rounded-md border border-zinc-800 bg-zinc-900 p-3 font-mono text-xs text-zinc-300">
              {script}
            </pre>
          </section>
        ) : null}

        <section id="flow-editor-basics" className="grid grid-cols-2 gap-3">
          <label id="flow-editor-start" className="flex flex-col gap-1 text-xs text-zinc-400">
            Start address
            <input
              id="flow-editor-start-input"
              className={inputClassName}
              value={draft.startUrl}
              placeholder="Blank runs on the current page"
              onChange={(event) => change({ startUrl: event.target.value })}
            />
          </label>
          <label id="flow-editor-description" className="flex flex-col gap-1 text-xs text-zinc-400">
            Description
            <input id="flow-editor-description-input" className={inputClassName} value={draft.description} onChange={(event) => change({ description: event.target.value })} />
          </label>
          <label id="flow-editor-background" className="flex items-center gap-2 text-sm text-zinc-300">
            <Toggle id="flow-editor-background-toggle" checked={draft.background} onChange={(background) => change({ background })} aria-label="Run in a hidden tab" />
            Run in a hidden tab
          </label>
          <label id="flow-editor-notify" className="flex items-center gap-2 text-sm text-zinc-300">
            <Toggle id="flow-editor-notify-toggle" checked={draft.notify} onChange={(notify) => change({ notify })} aria-label="Notify when it finishes" />
            Notify me when it finishes
          </label>
        </section>

        <section id="flow-editor-triggers" className="flex flex-col gap-2">
          <h2 id="flow-editor-triggers-title" className="text-sm font-semibold text-zinc-200">
            Triggers
          </h2>
          <TriggerList id="flow-editor-trigger-list" triggers={draft.triggers} onChange={(triggers) => change({ triggers })} />
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

        <section id="flow-editor-steps" className="flex flex-col gap-2">
          <h2 id="flow-editor-steps-title" className="text-sm font-semibold text-zinc-200">
            Steps
          </h2>
          {steps.length === 0 ? (
            <p id="flow-editor-steps-empty" className="text-xs text-zinc-500">
              No steps yet. Record them with the record button in the address bar, or add them here.
            </p>
          ) : (
            <ol id="flow-editor-step-list" className="flex flex-col gap-2">
              {steps.map((step, index) => (
                <StepCard
                  id={`flow-step-${index}`}
                  key={step.id || index}
                  step={step}
                  index={index}
                  count={steps.length}
                  pickTabs={stepTabs}
                  onChange={(next) => change({ steps: steps.map((item, at) => (at === index ? next : item)) })}
                  onMove={(delta) => moveStep(index, delta)}
                  onRemove={() => change({ steps: steps.filter((_, at) => at !== index) })}
                />
              ))}
            </ol>
          )}
          <div id="flow-editor-add-step" className="flex items-center gap-2">
            <select id="flow-editor-add-kind" className={cn(inputClassName, "w-52")} value={addKind} onChange={(event) => setAddKind(event.target.value as FlowStepKind)}>
              {STEP_KINDS.map((item) => (
                <option id={`flow-editor-add-kind-${item.kind}`} key={item.kind} value={item.kind}>
                  {item.label}
                </option>
              ))}
            </select>
            <Button id="flow-editor-add" size="sm" onClick={() => change({ steps: [...steps, blankStep(addKind)] })}>
              <Plus id="flow-editor-add-icon" className="size-3.5" aria-hidden />
              Add step
            </Button>
          </div>
        </section>

        <section id="flow-editor-variables" className="flex flex-col gap-1">
          <h2 id="flow-editor-variables-title" className="text-sm font-semibold text-zinc-200">
            Variables
          </h2>
          <p id="flow-editor-variables-hint" className="text-xs text-zinc-500">
            One name=value per line. Steps use them as {"{{name}}"}. {"{{date}}"}, {"{{url}}"}, {"{{file}}"} and {"{{path}}"} are filled in by triggers. Keep
            passwords in Settings, under Automation, and type them with {"{{secret:name}}"}.
          </p>
          <textarea
            id="flow-editor-variables-input"
            rows={3}
            className={cn(inputClassName, "font-mono text-xs")}
            value={variables}
            onChange={(event) => {
              setVariables(event.target.value);
              setDirty(true);
            }}
          />
        </section>
      </div>
    </section>
  );
}
