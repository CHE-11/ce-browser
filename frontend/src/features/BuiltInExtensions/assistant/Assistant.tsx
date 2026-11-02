import { useState } from "react";
import { Sparkles } from "lucide-react";
import { Button } from "@/components/Button";
import { PopupFrame } from "@/components/PopupFrame";
import { askPage } from "@/lib/browser-client";
import type { TabInfo } from "@type/browser";
import { PopoverHeader } from "../PopoverParts";
import { readPopoverTab } from "../shared";
import { ToolButton } from "../ToolButton";

export function AssistantButton({ activeTab }: { activeTab: TabInfo | undefined }) {
  return (
    <ToolButton
      id="assistant"
      label="Assistant"
      kind="assistant"
      icon={<Sparkles id="address-assistant-icon" aria-hidden />}
      activeTab={activeTab}
    />
  );
}

export function AssistantPopover({ params }: { params: URLSearchParams }) {
  const tab = readPopoverTab(params);
  const [question, setQuestion] = useState(() => params.get("question") ?? "");
  const [answer, setAnswer] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const ask = (text: string) => {
    setBusy(true);
    setError("");
    void askPage(tab.id, text)
      .then(setAnswer)
      .catch((reason: unknown) => setError(reason instanceof Error ? reason.message : "Couldn't reach the assistant."))
      .finally(() => setBusy(false));
  };

  return (
    <PopupFrame id="assistant-popover" width={340} maxHeight={480}>
      <PopoverHeader id="assistant-header" icon={<Sparkles aria-hidden />} title="Assistant" subtitle={tab.title || tab.url} />
      <div id="assistant-body" className="flex flex-col gap-2 px-3 pb-3">
        <Button id="assistant-summarize" variant="primary" disabled={busy} onClick={() => ask("")}>
          Summarize
        </Button>
        <textarea
          id="assistant-question"
          value={question}
          rows={3}
          placeholder="Ask about this page"
          className="rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-sm outline-none focus:border-sky-500"
          onChange={(event) => setQuestion(event.target.value)}
        />
        <Button id="assistant-ask" disabled={busy || !question.trim()} onClick={() => ask(question)}>
          Ask
        </Button>
        {error ? (
          <p id="assistant-error" className="text-xs text-amber-300">
            {error}
          </p>
        ) : null}
        {answer ? (
          <p id="assistant-answer" className="whitespace-pre-wrap text-sm text-zinc-200">
            {answer}
          </p>
        ) : null}
      </div>
    </PopupFrame>
  );
}
