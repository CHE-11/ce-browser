import { useState } from "react";
import { Check, Copy, Download, Pin, Workflow } from "lucide-react";
import type { AgentRun } from "@type/browser";
import { Button } from "@/components/Button";
import { Markdown } from "@/components/Markdown";
import { chooseSavePath, copyText, createBoard, listBoards, pinPage, writeTextFile } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { AgentError } from "./AgentError";

type Props = {
  run: AgentRun;
  onSaveFlow: () => void;
  onError: (text: string) => void;
};

function errorOf(reason: unknown) {
  return typeof reason === "string" ? reason : reason instanceof Error ? reason.message : "Something went wrong.";
}

function fileName(goal: string) {
  const base = goal
    .split("\n")[0]
    .replace(/[^\p{L}\p{N} _-]+/gu, "")
    .trim()
    .slice(0, 60);
  return `${base || "agent result"}.md`;
}

/** What the run found, in Markdown, with ways to keep it. */
export function AgentResult({ run, onSaveFlow, onError }: Props) {
  const [done, setDone] = useState("");
  if (!run.result) {
    return null;
  }
  if (run.status === "failed") {
    return <AgentError id="agent-panel-result" text={run.result} />;
  }

  const flash = (what: string) => {
    setDone(what);
    window.setTimeout(() => setDone(""), 1500);
  };
  const act = (what: string, action: () => Promise<unknown>) => {
    void action()
      .then((result) => {
        if (result !== false) {
          flash(what);
        }
      })
      .catch((reason: unknown) => onError(errorOf(reason)));
  };
  const pin = async () => {
    const boards = await listBoards();
    const board = boards.boards.find((item) => item.id === boards.lastBoardId) ?? boards.boards[0] ?? (await createBoard("Agent", null));
    await pinPage(run.tabId, board.id, false);
  };
  const save = async () => {
    const path = await chooseSavePath(fileName(run.goal), "Markdown", ["md"]);
    if (!path) {
      return false;
    }
    await writeTextFile(path, `# ${run.goal.split("\n")[0]}\n\n${run.result}\n`);
    return true;
  };

  const action = (id: string, label: string, Icon: typeof Copy, onClick: () => void) => (
    <Button id={`agent-panel-result-${id}`} size="xs" variant="ghost" onClick={onClick}>
      {done === id ? <Check id={`agent-panel-result-${id}-done`} className="size-3 text-emerald-400" aria-hidden /> : <Icon id={`agent-panel-result-${id}-icon`} className="size-3" aria-hidden />}
      {done === id ? "Done" : label}
    </Button>
  );

  return (
    <section
      id="agent-panel-result"
      className={cn("rounded-lg p-3 ring-1", run.status === "done" ? "bg-emerald-950/30 ring-emerald-800/60" : "bg-zinc-900 ring-zinc-800")}
    >
      <Markdown id="agent-panel-result-text" text={run.result} className="text-zinc-100" />
      <div id="agent-panel-result-actions" className="-mb-1 mt-2 flex flex-wrap gap-1 border-t border-zinc-800/80 pt-2">
        {action("copy", "Copy", Copy, () => act("copy", () => copyText(run.result)))}
        {action("save", "Save", Download, () => act("save", save))}
        {run.source === "agent" ? action("pin", "Pin page", Pin, () => act("pin", pin)) : null}
        {run.flowSteps > 0 ? action("flow", `Save as flow (${run.flowSteps})`, Workflow, onSaveFlow) : null}
      </div>
    </section>
  );
}
