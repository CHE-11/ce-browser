import type { AgentRun, AgentStep } from "@type/browser";

/** Safe file stem from a goal or message. */
export function chatFileName(title: string, fallback = "agent chat") {
  const base = title
    .split("\n")[0]
    .replace(/[^\p{L}\p{N} _-]+/gu, "")
    .trim()
    .slice(0, 60);
  return `${base || fallback}.md`;
}

function sectionTitle(kind: AgentStep["kind"]) {
  if (kind === "user") {
    return "You";
  }
  if (kind === "answer") {
    return "Answer";
  }
  if (kind === "error") {
    return "Error";
  }
  return "Agent";
}

/** Chat messages only — skips tool/action noise. */
export function chatMessages(steps: AgentStep[]) {
  return steps.filter(
    (step) =>
      (step.kind === "user" || step.kind === "think" || step.kind === "answer" || step.kind === "error") &&
      step.text.trim().length > 0,
  );
}

/** Full transcript as Markdown. */
export function formatWholeChat(run: AgentRun, steps: AgentStep[]) {
  const parts = [`# ${run.goal.split("\n")[0] || "Agent chat"}`, ""];
  for (const step of chatMessages(steps)) {
    parts.push(`## ${sectionTitle(step.kind)}`, "", step.text.trim(), "");
  }
  if (run.sources?.length) {
    parts.push("## Pages", "");
    run.sources.forEach((url, index) => {
      parts.push(`${index + 1}. ${url}`);
    });
    parts.push("");
  }
  return `${parts.join("\n").trim()}\n`;
}
