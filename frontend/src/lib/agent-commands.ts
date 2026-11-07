import type { AgentAutonomy } from "@type/browser";

/** `start` begins a task, `reply` talks to a running one, `followup` continues a finished one. */
export type ComposerMode = "start" | "reply" | "followup";

export type AgentCommand = {
  name: string;
  usage: string;
  hint: string;
  modes: ComposerMode[];
};

const ALL: ComposerMode[] = ["start", "reply", "followup"];

export const AGENT_COMMANDS: AgentCommand[] = [
  { name: "plan", usage: "/plan <task>", hint: "Plan the steps first, then work through them.", modes: ["start", "followup"] },
  { name: "background", usage: "/background <task>", hint: "Work in a new background tab and notify when done.", modes: ["start"] },
  { name: "full", usage: "/full <task>", hint: "Run this task on full auto.", modes: ["start"] },
  { name: "extract", usage: "/extract <what>", hint: "Pull rows from this page into a dataset.", modes: ["start", "followup"] },
  { name: "flow", usage: "/flow <name>", hint: "Run one of your saved flows.", modes: ["start", "followup"] },
  { name: "summarize", usage: "/summarize", hint: "Summarize this page and list the key facts.", modes: ["start", "followup"] },
  { name: "remember", usage: "/remember <note>", hint: "Save a note the agent reads in later runs.", modes: ALL },
  { name: "memory", usage: "/memory", hint: "Open what the agent remembers.", modes: ALL },
];

export type ParsedInput =
  | { kind: "task"; goal: string; instructions?: string; background?: boolean; autonomy?: AgentAutonomy }
  | { kind: "remember"; text: string }
  | { kind: "memory" }
  | { kind: "message"; text: string };

/** The commands that match what is typed after `/`, for the composer's list. */
export function commandMatches(text: string, mode: ComposerMode) {
  const match = /^\/(\w*)$/.exec(text);
  if (!match) {
    return [];
  }
  return AGENT_COMMANDS.filter((command) => command.modes.includes(mode) && command.name.startsWith(match[1].toLowerCase()));
}

/** What a submitted line asks for. Plain text, and unknown commands, pass through unchanged. */
export function parseAgentInput(text: string, mode: ComposerMode): ParsedInput {
  const plain: ParsedInput = mode === "start" ? { kind: "task", goal: text } : { kind: "message", text };
  const match = /^\/(\w+)\s*([\s\S]*)$/.exec(text.trim());
  if (!match) {
    return plain;
  }
  const name = match[1].toLowerCase();
  const rest = match[2].trim();
  const command = AGENT_COMMANDS.find((item) => item.name === name);
  if (!command || !command.modes.includes(mode)) {
    return plain;
  }
  const work = (goal: string, instructions: string): ParsedInput =>
    mode === "start" ? { kind: "task", goal, instructions } : { kind: "message", text: `${goal}. ${instructions}` };
  switch (name) {
    case "memory":
      return { kind: "memory" };
    case "remember":
      return rest ? { kind: "remember", text: rest } : { kind: "memory" };
    case "plan":
      return rest ? work(rest, "Start by calling update_plan with the steps, and keep it current as you work.") : plain;
    case "background":
      return rest ? { kind: "task", goal: rest, background: true } : plain;
    case "full":
      return rest ? { kind: "task", goal: rest, autonomy: "full" } : plain;
    case "extract":
      return work(
        `Extract ${rest || "the main list or table"} from this page`,
        "Use the extract tool to save the rows as a dataset. Scroll or page through the list if there is more. Then summarize what you saved.",
      );
    case "flow":
      return rest
        ? work(`Run my "${rest}" flow`, "Call list_flows, run the saved flow that best matches with run_flow, and report the result.")
        : work("Show my saved flows", "Call list_flows and list them.");
    case "summarize":
      return work(`Summarize this page${rest ? ` with a focus on ${rest}` : ""}`, "List the key facts.");
    default:
      return plain;
  }
}
