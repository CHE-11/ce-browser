import { useCallback, useEffect, useRef, useState } from "react";
import { AI_INPUT_TOKENS, AI_OUTPUT_TOKENS, clampTokenLimit } from "@/lib/ai/token-limits";
import { cn } from "@/lib/cn";
import { useDismiss } from "@/lib/use-dismiss";

type Props = {
  id: string;
  inputTokens: number;
  outputTokens: number;
  onInputTokens: (value: number) => void;
  onOutputTokens: (value: number) => void;
};

/** Token limits for the agent. Opens above the Limits label under the composer. */
export function AgentLimitsMenu({ id, inputTokens, outputTokens, onInputTokens, onOutputTokens }: Props) {
  const root = useRef<HTMLDivElement | null>(null);
  const [open, setOpen] = useState(false);
  const close = useCallback(() => setOpen(false), []);
  useDismiss(root, open, close);

  return (
    <div id={id} ref={root} className="shrink-0">
      <button
        id={`${id}-button`}
        type="button"
        aria-haspopup="dialog"
        aria-expanded={open}
        title="Token limits"
        className="flex flex-col items-start gap-0 text-left text-[10px] leading-tight text-zinc-500 hover:text-zinc-200"
        onClick={() => setOpen((value) => !value)}
      >
        <span id={`${id}-button-in`}>{`${inputTokens.toLocaleString()} in`}</span>
        <span id={`${id}-button-out`}>{`${outputTokens.toLocaleString()} out`}</span>
      </button>
      {open ? (
        <div
          id={`${id}-popover`}
          role="dialog"
          aria-label="Token limits"
          data-agent-popover
          className="absolute bottom-full left-0 z-40 mb-2 w-[min(20rem,100%)] rounded-lg bg-zinc-900 p-2 shadow-2xl ring-1 ring-zinc-700"
        >
          <div id={`${id}-title`} className="px-0.5 pb-2 text-xs font-semibold text-zinc-100">
            Token limits
          </div>
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
