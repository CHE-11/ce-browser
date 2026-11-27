import { useState } from "react";
import { HelpCircle, ShieldQuestion } from "lucide-react";
import type { AgentApproval as Approval, AgentAutonomy } from "@type/browser";
import { Button } from "@/components/Button";
import { autonomyLabel } from "@/lib/agent-autonomy";
import { inputClassName } from "./agent-styles";

const RISK_TEXT: Record<Approval["risk"], string> = {
  "": "",
  open: "New site",
  send: "Sends or posts",
  destructive: "Deletes or closes",
  payment: "Spends money",
  upload: "Uploads files",
  script: "Runs a script",
  budget: "Limit reached",
  desktop: "Uses the desktop",
};

const RISK_HINT: Record<Approval["risk"], string> = {
  "": "Review what the agent wants to do before it continues.",
  open: "The agent wants to leave the current site or open a page that is not on your allow list.",
  send: "This can post, publish, or send content. Check the page before allowing it.",
  destructive: "This may delete, cancel, or close something permanently.",
  payment: "This looks like spending money or confirming a purchase. It always asks once.",
  upload: "Local files would be put on the page. It always asks once.",
  script: "Custom JavaScript would run in the page.",
  budget: "The token budget for this run was reached.",
  desktop: "The agent wants to control the Windows desktop outside the browser.",
};

/** The lowest autonomy that would let this kind of action go ahead on its own. */
const COVERED_BY: Partial<Record<Approval["risk"], AgentAutonomy>> = {
  open: "auto",
  send: "auto",
  destructive: "full",
  script: "full",
};

const RUN_SCOPE_RISKS = new Set(["open", "send", "destructive", "script", "desktop"]);

type Props = {
  approval: Approval;
  autonomy: AgentAutonomy;
  onAnswer: (allow: boolean) => void;
  onReply: (text: string) => void;
  /** Allows this action and raises the run's autonomy so similar ones don't ask. */
  onRaise?: (autonomy: AgentAutonomy) => void;
  /** Allows this risk kind for the rest of the run without raising autonomy. */
  onAllowRisk?: (risk: string) => void;
};

/** What the agent is waiting on: approval for a sensitive action, or an answer to its question. */
export function AgentApproval({ approval, autonomy, onAnswer, onReply, onRaise, onAllowRisk }: Props) {
  const [text, setText] = useState("");
  const question = approval.kind === "question";
  const Icon = question ? HelpCircle : ShieldQuestion;
  const raise = COVERED_BY[approval.risk];
  const canRaise = !!onRaise && !!raise && raise !== autonomy && !(autonomy === "full" && raise === "auto");
  const canScope = !!onAllowRisk && RUN_SCOPE_RISKS.has(approval.risk);

  return (
    <div id="agent-approval" role="alertdialog" aria-labelledby="agent-approval-prompt" className="rounded-lg border border-amber-700/60 bg-amber-950/40 p-3">
      <div id="agent-approval-header" className="mb-2 flex items-start gap-2">
        <Icon id="agent-approval-icon" className="mt-0.5 size-4 shrink-0 text-amber-400" aria-hidden />
        <div id="agent-approval-body" className="min-w-0">
          {approval.risk ? (
            <span id="agent-approval-risk" className="mb-0.5 inline-block rounded bg-amber-900/60 px-1 text-[10px] font-medium uppercase tracking-wide text-amber-200">
              {RISK_TEXT[approval.risk]}
            </span>
          ) : null}
          <p id="agent-approval-prompt" className="whitespace-pre-wrap break-words text-sm text-zinc-100">
            {approval.prompt}
          </p>
          {!question ? (
            <p id="agent-approval-hint" className="mt-1 text-[11px] text-amber-100/70">
              {RISK_HINT[approval.risk]}
            </p>
          ) : null}
        </div>
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
        <div id="agent-approval-actions" className="flex flex-wrap justify-end gap-2">
          <Button id="agent-approval-deny" size="sm" variant="outline" onClick={() => onAnswer(false)}>
            Deny
          </Button>
          {canScope ? (
            <Button
              id="agent-approval-scope"
              size="sm"
              variant="secondary"
              title={`Allow this ${RISK_TEXT[approval.risk].toLowerCase() || "action"} for the rest of this run`}
              onClick={() => onAllowRisk?.(approval.risk)}
            >
              Allow for this run
            </Button>
          ) : null}
          {canRaise ? (
            <Button id="agent-approval-raise" size="sm" variant="secondary" title={`Allow, and let similar actions go ahead in this run`} onClick={() => onRaise?.(raise!)}>
              Allow, switch to {autonomyLabel(raise!)}
            </Button>
          ) : null}
          <Button id="agent-approval-allow" size="sm" variant="primary" autoFocus onClick={() => onAnswer(true)}>
            {approval.risk === "budget" ? "Keep going" : "Allow once"}
          </Button>
        </div>
      )}
    </div>
  );
}
