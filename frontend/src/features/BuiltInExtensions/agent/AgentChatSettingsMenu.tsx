import { useCallback, useEffect, useRef, useState } from "react";
import { ChevronDown, SlidersHorizontal } from "lucide-react";
import type { AgentAccess } from "@type/browser";
import type { AccessToggle } from "@/lib/agent-access";
import { AI_INPUT_TOKENS, AI_OUTPUT_TOKENS, clampTokenLimit } from "@/lib/ai/token-limits";
import { cn } from "@/lib/cn";
import { useDismiss } from "@/lib/use-dismiss";
import { AgentAccessOptions } from "./AgentAccessMenu";
import { AgentModelPicker } from "./AgentModelPicker";

type Props = {
  id: string;
  /** Saved agent model. Blank uses the provider model. */
  model: string;
  modelFallback: string;
  onModel: (model: string) => void;
  inputTokens: number;
  outputTokens: number;
  onInputTokens: (value: number) => void;
  onOutputTokens: (value: number) => void;
  access: AgentAccess;
  onAccess: (access: AgentAccess) => void;
  scope: "run" | "next";
  currentSite: string;
  blockedBySettings: Partial<Record<AccessToggle, string>>;
};

/** Model, token limits, and access for the agent. Model and token changes are saved in Settings. */
export function AgentChatSettingsMenu({
  id,
  model,
  modelFallback,
  onModel,
  inputTokens,
  outputTokens,
  onInputTokens,
  onOutputTokens,
  access,
  onAccess,
  scope,
  currentSite,
  blockedBySettings,
}: Props) {
  const root = useRef<HTMLDivElement | null>(null);
  const [open, setOpen] = useState(false);
  const close = useCallback(() => setOpen(false), []);
  useDismiss(root, open, close);

  return (
    <div id={id} ref={root} className="relative min-w-[150px]">
      <button
        id={`${id}-button`}
        type="button"
        aria-haspopup="dialog"
        aria-expanded={open}
        title="Chat settings"
        className="flex h-7 w-full min-w-0 items-center gap-1 rounded-md bg-zinc-900 px-1.5 text-[11px] text-zinc-200 ring-1 ring-zinc-800 hover:bg-zinc-800"
        onClick={() => setOpen((value) => !value)}
      >
        <SlidersHorizontal id={`${id}-button-icon`} className="size-3 shrink-0 text-zinc-400" aria-hidden />
        <span id={`${id}-button-label`} className="min-w-0 flex-1 truncate text-left">
          Chat Settings
        </span>
        <ChevronDown id={`${id}-button-chevron`} className="size-3 shrink-0 opacity-60" aria-hidden />
      </button>
      {open ? (
        <div
          id={`${id}-popover`}
          role="dialog"
          aria-label="Chat settings"
          data-agent-popover
          className="absolute left-0 top-full z-40 mt-1 flex max-h-[min(50rem,90vh)] w-[min(20rem,calc(100vw-1.5rem))] flex-col gap-2 overflow-y-auto rounded-lg bg-zinc-900 p-2 shadow-2xl ring-1 ring-zinc-700"
        >
          <div id={`${id}-title`} className="px-1 text-xs font-semibold text-zinc-100">
            Chat settings
          </div>
          <AgentModelPicker id={`${id}-model`} value={model} fallback={modelFallback} listPlacement="inline" onChange={onModel} />
          <div id={`${id}-tokens`} className="grid grid-cols-2 gap-2">
            <TokenField
              id={`${id}-input-tokens`}
              label="Input tokens"
              hint="Prompt size. Older text is shortened to fit."
              value={inputTokens}
              bounds={AI_INPUT_TOKENS}
              onChange={onInputTokens}
            />
            <TokenField
              id={`${id}-output-tokens`}
              label="Output tokens"
              hint="Reply length. On Claude, thinking raises a number under 8192."
              value={outputTokens}
              bounds={AI_OUTPUT_TOKENS}
              onChange={onOutputTokens}
            />
          </div>
          <div id={`${id}-access`} className="border-t border-zinc-800 pt-2">
            <AgentAccessOptions id={`${id}-access`} access={access} onChange={onAccess} scope={scope} currentSite={currentSite} blockedBySettings={blockedBySettings} />
          </div>
        </div>
      ) : null}
    </div>
  );
}

function TokenField({
  id,
  label,
  hint,
  value,
  bounds,
  onChange,
}: {
  id: string;
  label: string;
  hint: string;
  value: number;
  bounds: { min: number; max: number; fallback: number };
  onChange: (value: number) => void;
}) {
  const [draft, setDraft] = useState(String(value));
  useEffect(() => {
    setDraft(String(value));
  }, [value]);
  const commit = () => {
    const next = clampTokenLimit(Number(draft), bounds);
    setDraft(String(next));
    if (next !== value) {
      onChange(next);
    }
  };
  return (
    <label id={id} className="flex flex-col gap-1" title={hint}>
      <span id={`${id}-label`} className="px-0.5 text-[10px] text-zinc-500">
        {label}
      </span>
      <input
        id={`${id}-input`}
        className={cn("h-7 w-full rounded-md border border-zinc-800 bg-zinc-950 px-1.5 text-right text-[11px] text-zinc-100 outline-none focus:border-sky-500")}
        type="number"
        inputMode="numeric"
        min={bounds.min}
        max={bounds.max}
        step={1}
        aria-label={label}
        value={draft}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            event.currentTarget.blur();
          }
        }}
      />
    </label>
  );
}
