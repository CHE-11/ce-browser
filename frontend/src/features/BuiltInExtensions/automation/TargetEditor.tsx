import { useState } from "react";
import { Crosshair, ScanSearch } from "lucide-react";
import type { FlowTarget } from "@type/browser";
import { Button } from "@/components/Button";
import { flowPickTarget, flowTestTarget } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { inputClassName } from "@/features/BuiltInExtensions/agent/agent-styles";
import { EMPTY_TARGET } from "./flow-model";
import { Field } from "./StepField";

const MATCHED_BY: Record<string, string> = {
  testId: "its test id",
  role: "its role and name",
  css: "its CSS selector",
  text: "its text. The next run updates the step with sturdier locators",
  xpath: "its position on the page. The next run updates the step with sturdier locators",
};

type TargetEditorProps = {
  id: string;
  target: FlowTarget | null;
  /** Tab to pick and test in. */
  tabId: number | null;
  onChange: (target: FlowTarget) => void;
};

export function TargetEditor({ id, target, tabId, onChange }: TargetEditorProps) {
  const value = target ?? EMPTY_TARGET;
  const [busy, setBusy] = useState<"pick" | "test" | null>(null);
  const [result, setResult] = useState<{ ok: boolean; text: string } | null>(null);
  const recorded = [value.role, value.name ? `"${value.name}"` : "", value.testId].filter(Boolean).join(" ");

  const pick = () => {
    if (tabId === null) {
      return;
    }
    setBusy("pick");
    setResult(null);
    void flowPickTarget(tabId)
      .then((picked) => {
        if (picked) {
          onChange({ ...picked, description: picked.description || value.description });
          setResult({ ok: true, text: "Picked. Test it to see that it's found again." });
        }
      })
      .catch((reason: unknown) => setResult({ ok: false, text: String(reason) }))
      .finally(() => setBusy(null));
  };

  const test = () => {
    if (tabId === null) {
      return;
    }
    setBusy("test");
    setResult(null);
    void flowTestTarget(tabId, value)
      .then((found) =>
        setResult(
          found.found
            ? { ok: true, text: `Found by ${MATCHED_BY[found.by] ?? found.by}.` }
            : { ok: false, text: "Not found on that page. A run would ask AI to find it by its description." },
        ),
      )
      .catch((reason: unknown) => setResult({ ok: false, text: String(reason) }))
      .finally(() => setBusy(null));
  };

  return (
    <div id={id} className="grid grid-cols-2 gap-2">
      <Field id={`${id}-description`} label="Element, in words (used to find it again)">
        <input
          id={`${id}-description-input`}
          className={inputClassName}
          value={value.description}
          placeholder="the Add to cart button"
          onChange={(event) => onChange({ ...value, description: event.target.value })}
        />
      </Field>
      <Field id={`${id}-css`} label="CSS selector">
        <input
          id={`${id}-css-input`}
          className={cn(inputClassName, "font-mono text-xs")}
          value={value.css}
          placeholder="#search"
          onChange={(event) => onChange({ ...value, css: event.target.value })}
        />
      </Field>
      <div id={`${id}-tools`} className="col-span-2 flex flex-wrap items-center gap-2">
        <Button id={`${id}-pick`} size="xs" variant="outline" disabled={tabId === null || busy !== null} onClick={pick} title={tabId === null ? "Open the page in a tab first" : undefined}>
          <Crosshair id={`${id}-pick-icon`} className="size-3.5" aria-hidden />
          {busy === "pick" ? "Click it on the page…" : "Pick on page"}
        </Button>
        <Button id={`${id}-test`} size="xs" variant="ghost" disabled={tabId === null || busy !== null || !target} onClick={test}>
          <ScanSearch id={`${id}-test-icon`} className="size-3.5" aria-hidden />
          {busy === "test" ? "Looking…" : "Test"}
        </Button>
        {result ? (
          <span id={`${id}-result`} role="status" className={cn("min-w-0 flex-1 text-[11px]", result.ok ? "text-emerald-400" : "text-amber-300")}>
            {result.text}
          </span>
        ) : recorded ? (
          <span id={`${id}-recorded`} className="min-w-0 flex-1 truncate text-[11px] text-zinc-500">
            Recorded as {recorded}
          </span>
        ) : null}
      </div>
    </div>
  );
}
