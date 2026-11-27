import type { AgentRun } from "@type/browser";
import { AgentError } from "./AgentError";

type Props = {
  run: AgentRun;
};

/** Whether the transcript already shows this result as a chat message. */
function resultInChat(run: AgentRun) {
  const result = run.result.trim();
  if (!result || result === "Stopped." || result === "Finished.") {
    return true;
  }
  return run.steps.some((step) => {
    if (step.kind !== "think" && step.kind !== "answer" && step.kind !== "error") {
      return false;
    }
    const text = step.text.trim();
    return text === result || (text.length >= 80 && result.startsWith(text));
  });
}

/** Fallback when a finished result is not already in the transcript. */
export function AgentResult({ run }: Props) {
  if (!run.result) {
    return null;
  }
  if (run.status === "failed") {
    return resultInChat(run) ? null : <AgentError id="agent-panel-result" text={run.result} />;
  }
  if (run.result === "Stopped." || run.result === "Finished." || resultInChat(run)) {
    return null;
  }

  return (
    <div id="agent-panel-result" className="rounded-lg rounded-bl-sm bg-zinc-900/80 px-2.5 py-1.5 text-xs text-zinc-200 ring-1 ring-zinc-800">
      <p id="agent-panel-result-text" className="whitespace-pre-wrap break-words">
        {run.result}
      </p>
    </div>
  );
}
