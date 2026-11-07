import { useState } from "react";
import { HelpCircle, ShieldQuestion } from "lucide-react";
import type { AgentApproval as Approval } from "@type/browser";
import { Button } from "@/components/Button";
import { inputClassName } from "./agent-styles";

type Props = {
  approval: Approval;
  onAnswer: (allow: boolean) => void;
  onReply: (text: string) => void;
};

/** What the agent is waiting on: approval for a sensitive action, or an answer to its question. */
export function AgentApproval({ approval, onAnswer, onReply }: Props) {
  const [text, setText] = useState("");
  const question = approval.kind === "question";
  const Icon = question ? HelpCircle : ShieldQuestion;

  return (
    <div id="agent-approval" role="alertdialog" aria-labelledby="agent-approval-prompt" className="rounded-lg border border-amber-700/60 bg-amber-950/40 p-3">
      <div id="agent-approval-header" className="mb-2 flex items-start gap-2">
        <Icon id="agent-approval-icon" className="mt-0.5 size-4 shrink-0 text-amber-400" aria-hidden />
        <p id="agent-approval-prompt" className="whitespace-pre-wrap text-sm text-zinc-100">
          {approval.prompt}
        </p>
      </div>
      {question ? (
        <form
          id="agent-approval-form"
          className="flex flex-col gap-2"
          onSubmit={(event) => {
            event.preventDefault();
            if (text.trim()) {
              onReply(text.trim());
              setText("");
            }
          }}
        >
          <input
            id="agent-approval-input"
            className={inputClassName}
            value={text}
            autoFocus
            placeholder="Your answer"
            onChange={(event) => setText(event.target.value)}
          />
          <div id="agent-approval-actions" className="flex justify-end gap-2">
            <Button id="agent-approval-skip" size="sm" variant="ghost" onClick={() => onAnswer(false)}>
              Skip
            </Button>
            <Button id="agent-approval-send" size="sm" variant="primary" type="submit" disabled={!text.trim()}>
              Answer
            </Button>
          </div>
        </form>
      ) : (
        <div id="agent-approval-actions" className="flex justify-end gap-2">
          <Button id="agent-approval-deny" size="sm" variant="outline" onClick={() => onAnswer(false)}>
            Deny
          </Button>
          <Button id="agent-approval-allow" size="sm" variant="primary" autoFocus onClick={() => onAnswer(true)}>
            Allow
          </Button>
        </div>
      )}
    </div>
  );
}
