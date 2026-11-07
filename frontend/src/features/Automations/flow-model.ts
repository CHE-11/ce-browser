import type { Flow, FlowRun, FlowStep, FlowStepKind, FlowTarget, FlowTrigger, FlowTriggerKind } from "@type/browser";

export const STEP_KINDS: { kind: FlowStepKind; label: string; hint: string }[] = [
  { kind: "navigate", label: "Go to address", hint: "Load an address in the tab." },
  { kind: "click", label: "Click", hint: "Click an element." },
  { kind: "type", label: "Type", hint: "Fill a field. {{secret:name}} types a saved secret." },
  { kind: "press", label: "Press key", hint: "Press a key such as Enter or Control+A." },
  { kind: "select", label: "Choose option", hint: "Pick options in a list box." },
  { kind: "scroll", label: "Scroll", hint: "Scroll the page." },
  { kind: "hover", label: "Hover", hint: "Move the mouse over an element." },
  { kind: "wait", label: "Wait", hint: "Pause for some seconds." },
  { kind: "waitFor", label: "Wait for text", hint: "Wait until text appears or goes away." },
  { kind: "back", label: "Go back", hint: "Go back one page." },
  { kind: "extract", label: "Extract data", hint: "Read a list or table into the run's dataset." },
  { kind: "ai", label: "AI task", hint: "Let the agent do this part in plain words." },
  { kind: "assert", label: "Check text", hint: "Fail the run unless the page shows this text." },
  { kind: "notify", label: "Notify", hint: "Show a Windows notification." },
  { kind: "upload", label: "Upload files", hint: "Put files into a file input." },
];

export const TRIGGER_KINDS: { kind: FlowTriggerKind; label: string }[] = [
  { kind: "daily", label: "Every day" },
  { kind: "weekly", label: "Every week" },
  { kind: "interval", label: "Every few minutes" },
  { kind: "pageChange", label: "When the page changes" },
  { kind: "visit", label: "When I visit a site" },
  { kind: "download", label: "When a download finishes" },
  { kind: "startup", label: "When the browser starts" },
  { kind: "hotkey", label: "Keyboard shortcut" },
];

export const WEEKDAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/** Step kinds that act on one element of the page. */
export const TARGET_KINDS = ["click", "type", "select", "hover", "upload"];

/** `{{name}}` values a run fills in by itself. */
export const BUILT_IN_VARIABLES = ["date", "url", "file", "path", "output", "rows"];

export const EMPTY_TARGET: FlowTarget = { role: "", name: "", text: "", testId: "", css: "", xpath: "", tag: "", description: "" };

export function stepKindLabel(kind: string) {
  return STEP_KINDS.find((item) => item.kind === kind)?.label ?? `Unknown (${kind})`;
}

/** A step id that stays put while steps are reordered before the flow is saved. */
export function newStepId() {
  return `${Date.now().toString(16)}${Math.floor(Math.random() * 0x10000)
    .toString(16)
    .padStart(4, "0")}`;
}

export function blankStep(kind: FlowStepKind): FlowStep {
  return {
    id: newStepId(),
    kind,
    target: null,
    url: "",
    text: "",
    keys: kind === "press" ? "Enter" : "",
    values: [],
    seconds: kind === "wait" ? 2 : 0,
    prompt: "",
    spec: null,
    pages: 1,
    optional: false,
    disabled: false,
    note: "",
    autonomy: "",
  };
}

export function copyStep(step: FlowStep): FlowStep {
  return { ...structuredClone(step), id: newStepId() };
}

export function blankTrigger(kind: FlowTriggerKind): FlowTrigger {
  return {
    kind,
    enabled: true,
    minutes: kind === "pageChange" ? 30 : 60,
    time: "09:00",
    days: kind === "weekly" ? [1] : [],
    pattern: "",
    slot: 1,
  };
}

export function blankFlow(startUrl = ""): Flow {
  return {
    id: "",
    name: "",
    description: "",
    startUrl,
    steps: [],
    triggers: [],
    variables: {},
    background: false,
    notify: true,
    monitorPrompt: "",
    paused: false,
    lastRun: 0,
    lastStatus: "",
    created: 0,
    updated: 0,
  };
}

/** Steps written by an older build may lack fields this UI reads. */
export function withDefaults(flow: Flow): Flow {
  return {
    ...blankFlow(),
    ...flow,
    steps: (flow.steps ?? []).map((step) => ({ ...blankStep("click"), ...step, id: step.id || newStepId() })),
    triggers: (flow.triggers ?? []).map((trigger) => ({ ...blankTrigger("daily"), ...trigger })),
    variables: flow.variables ?? {},
  };
}

export function triggerSummary(trigger: FlowTrigger) {
  switch (trigger.kind) {
    case "daily":
      return `Every day at ${trigger.time}`;
    case "weekly":
      return `${trigger.days.map((day) => WEEKDAYS[day]).join(", ") || "Every day"} at ${trigger.time}`;
    case "interval":
      return `Every ${trigger.minutes} min`;
    case "pageChange":
      return `Check every ${trigger.minutes} min`;
    case "visit":
      return `On ${trigger.pattern || "any site"}`;
    case "download":
      return `After ${trigger.pattern || "any download"}`;
    case "startup":
      return "At startup";
    case "hotkey":
      return `Shortcut slot ${trigger.slot}`;
    default:
      return trigger.kind;
  }
}

function clip(text: string, max: number) {
  return text.length > max ? `${text.slice(0, max)}…` : text;
}

function maskSecrets(text: string) {
  return text.replace(/\{\{\s*secret:[^}]*\}\}/g, "•••");
}

/** Plain words for the element a step acts on. Mirrors `FlowTarget::describe` in Rust. */
export function describeTarget(target: FlowTarget | null) {
  if (!target) {
    return "the focused element";
  }
  if (target.description.trim()) {
    return target.description.trim();
  }
  if (target.name.trim()) {
    return `the "${clip(target.name.trim(), 60)}" ${target.role || target.tag}`;
  }
  if (target.text.trim()) {
    return `the element with text "${clip(target.text.trim(), 60)}"`;
  }
  return target.css ? `the element at ${clip(target.css, 80)}` : "an element you haven't picked yet";
}

/** What a step does, in one line. Mirrors `step_label` in Rust. */
export function stepSummary(step: FlowStep) {
  const target = describeTarget(step.target);
  switch (step.kind) {
    case "navigate":
      return step.url ? `Go to ${clip(step.url, 100)}` : "Go to an address";
    case "click":
      return `Click ${target}`;
    case "type":
      return `Type "${clip(maskSecrets(step.text), 60)}" into ${target}`;
    case "press":
      return `Press ${step.keys || "a key"}`;
    case "select":
      return `Choose ${step.values.join(", ") || "an option"} in ${target}`;
    case "scroll":
      return `Scroll ${step.text || "down"}`;
    case "hover":
      return `Hover over ${target}`;
    case "wait":
      return `Wait ${Math.max(step.seconds, 0.1)}s`;
    case "waitFor":
      return `Wait for "${clip(step.text, 60)}"${step.keys === "gone" ? " to go away" : ""}`;
    case "back":
      return "Go back";
    case "extract":
      return step.spec ? `Extract ${step.spec.fields.map((field) => field.name).join(", ")}` : step.prompt ? `Extract: ${clip(step.prompt, 70)}` : "Extract data";
    case "ai":
      return `AI: ${clip(step.prompt, 80) || "a task"}`;
    case "assert":
      return `Check the page shows "${clip(step.text, 60)}"`;
    case "notify":
      return `Notify: ${clip(step.text, 60)}`;
    case "upload":
      return `Upload ${step.values.join(", ") || "files"}`;
    default:
      return `Unknown step "${step.kind}"`;
  }
}

/** `{{name}}` placeholders in some text, without secrets. */
export function usedVariables(text: string) {
  const names: string[] = [];
  for (const match of text.matchAll(/\{\{\s*([^}]+?)\s*\}\}/g)) {
    const name = match[1];
    if (!name.startsWith("secret:") && !names.includes(name)) {
      names.push(name);
    }
  }
  return names;
}

function hasTarget(target: FlowTarget | null) {
  return Boolean(target && (target.css || target.xpath || target.testId || target.text || target.description || (target.role && target.name)));
}

export function stepIssues(step: FlowStep, known: Set<string>) {
  const issues: string[] = [];
  if (step.disabled) {
    return issues;
  }
  if (TARGET_KINDS.includes(step.kind) && !hasTarget(step.target)) {
    issues.push("Pick the element on the page, or describe it in words.");
  }
  switch (step.kind) {
    case "navigate":
      if (!step.url.trim()) {
        issues.push("Enter the address to load.");
      }
      break;
    case "press":
      if (!step.keys.trim()) {
        issues.push("Enter the key to press.");
      }
      break;
    case "waitFor":
    case "assert":
      if (!step.text.trim()) {
        issues.push("Enter the text to look for.");
      }
      break;
    case "extract":
      if (!step.spec && !step.prompt.trim()) {
        issues.push("Pick the list on a page, or describe the data for AI.");
      }
      break;
    case "ai":
      if (!step.prompt.trim()) {
        issues.push("Describe what the agent should do.");
      }
      break;
    case "upload":
      if (step.values.length === 0) {
        issues.push("Add the full path of each file.");
      }
      break;
  }
  const text = [step.url, step.text, step.prompt, step.keys, ...step.values].join("\n");
  const missing = usedVariables(text).filter((name) => !known.has(name));
  if (missing.length) {
    issues.push(`${missing.map((name) => `{{${name}}}`).join(", ")} ${missing.length === 1 ? "isn't" : "aren't"} defined under Settings, in Variables.`);
  }
  return issues;
}

export function triggerIssues(trigger: FlowTrigger, flow: Flow) {
  const issues: string[] = [];
  if (!trigger.enabled) {
    return issues;
  }
  if (trigger.kind === "visit" && !trigger.pattern.trim()) {
    issues.push("Enter an address pattern, such as *.example.com/*.");
  }
  if (trigger.kind === "pageChange" && !flow.startUrl.trim()) {
    issues.push("Set a start address under Settings. That's the page this trigger watches.");
  }
  return issues;
}

export type FlowIssues = {
  steps: Record<number, string[]>;
  triggers: Record<number, string[]>;
  count: number;
};

/** Problems that would make a run fail or a trigger never fire. Saving is still allowed. */
export function validateFlow(flow: Flow, variableNames: string[]): FlowIssues {
  const known = new Set([...BUILT_IN_VARIABLES, ...variableNames]);
  const steps: Record<number, string[]> = {};
  const triggers: Record<number, string[]> = {};
  let count = 0;
  flow.steps.forEach((step, index) => {
    const found = stepIssues(step, known);
    if (found.length) {
      steps[index] = found;
      count += found.length;
    }
  });
  flow.triggers.forEach((trigger, index) => {
    const found = triggerIssues(trigger, flow);
    if (found.length) {
      triggers[index] = found;
      count += found.length;
    }
  });
  return { steps, triggers, count };
}

export function formatWhen(ms: number) {
  if (!ms) {
    return "Never";
  }
  return new Date(ms).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

export function formatTime(ms: number) {
  return new Date(ms).toLocaleTimeString(undefined, { timeStyle: "short" });
}

/** "in 5 min", "in 3h", "tomorrow 09:00" for a time ahead of `now`. */
export function untilText(at: number, now: number) {
  const minutes = Math.round((at - now) / 60_000);
  if (minutes <= 0) {
    return "now";
  }
  if (minutes < 60) {
    return `in ${minutes} min`;
  }
  const hours = Math.round(minutes / 60);
  if (hours < 24) {
    return `in ${hours}h`;
  }
  const day = new Date(at);
  return `${day.toLocaleDateString(undefined, { weekday: "short" })} ${formatTime(at)}`;
}

/** "Today", "Yesterday" or the date, for grouping runs. */
export function dayLabel(ms: number, now: number) {
  const day = new Date(ms);
  const today = new Date(now);
  const start = (date: Date) => new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
  const days = Math.round((start(today) - start(day)) / 86_400_000);
  if (days === 0) {
    return "Today";
  }
  if (days === 1) {
    return "Yesterday";
  }
  return day.toLocaleDateString(undefined, { weekday: "long", month: "short", day: "numeric", year: day.getFullYear() === today.getFullYear() ? undefined : "numeric" });
}

export function runDuration(run: FlowRun) {
  const end = run.finished || Date.now();
  const seconds = Math.max(0, Math.round((end - run.started) / 1000));
  return seconds < 60 ? `${seconds}s` : `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
}

/** "steps 3–5" or "from step 3" for a run that covered part of its flow. Blank for whole runs. */
export function runRange(run: FlowRun) {
  if (run.toStep !== null && run.toStep !== undefined) {
    return run.toStep === run.fromStep ? `step ${run.fromStep + 1} only` : `steps ${run.fromStep + 1}–${run.toStep + 1}`;
  }
  return run.fromStep ? `from step ${run.fromStep + 1}` : "";
}

export function statusTone(status: string) {
  switch (status) {
    case "done":
    case "ok":
      return "text-emerald-400";
    case "healed":
      return "text-amber-300";
    case "failed":
      return "text-red-400";
    case "running":
      return "text-sky-400";
    default:
      return "text-zinc-400";
  }
}

export function statusDot(status: string) {
  switch (status) {
    case "done":
    case "ok":
      return "bg-emerald-400";
    case "healed":
      return "bg-amber-300";
    case "failed":
      return "bg-red-400";
    case "running":
      return "bg-sky-400";
    default:
      return "bg-zinc-600";
  }
}
