import type { Flow, FlowRun, FlowStep, FlowStepKind, FlowTrigger, FlowTriggerKind } from "@type/browser";

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

export function stepKindLabel(kind: string) {
  return STEP_KINDS.find((item) => item.kind === kind)?.label ?? `Unknown (${kind})`;
}

export function blankStep(kind: FlowStepKind): FlowStep {
  return {
    id: "",
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
    note: "",
    autonomy: "",
  };
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
    steps: (flow.steps ?? []).map((step) => ({ ...blankStep("click"), ...step })),
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

export function formatWhen(ms: number) {
  if (!ms) {
    return "Never";
  }
  return new Date(ms).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

export function runDuration(run: FlowRun) {
  const end = run.finished || Date.now();
  const seconds = Math.max(0, Math.round((end - run.started) / 1000));
  return seconds < 60 ? `${seconds}s` : `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
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
