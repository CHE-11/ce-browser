import { useCallback, useEffect, useRef, useState, type KeyboardEvent } from "react";
import { Check } from "lucide-react";
import { modelAcceptsThinking } from "@/lib/ai/model-label";
import { cn } from "@/lib/cn";
import { useDismiss } from "@/lib/use-dismiss";
import { formatPrice, useModelCatalog, useModelPrice } from "@/lib/use-model-prices";

type Props = {
  id: string;
  value: string;
  /** Model used when `value` is blank. */
  fallback: string;
  provider: string;
  /** `low`, `medium`, `high`, or blank for the provider default. */
  thinking: string;
  onChange: (model: string) => void;
  onThinking: (thinking: string) => void;
  className?: string;
};

type Choice = { value: string; label: string; detail: string; price: string };

const THINKING_LEVELS = [
  { value: "", label: "Default" },
  { value: "low", label: "Low" },
  { value: "medium", label: "Medium" },
  { value: "high", label: "High" },
];

function listedModel(model: string) {
  return !model.toLowerCase().includes("healthcare");
}

/** Model name in the footer. The list opens above it, with a thinking level when the model accepts one. */
export function AgentModelPicker({ id, value, fallback, provider, thinking, onChange, onThinking, className }: Props) {
  const root = useRef<HTMLDivElement | null>(null);
  const input = useRef<HTMLInputElement | null>(null);
  const { models, prices, loading } = useModelCatalog();
  const selected = value || fallback;
  const fallbackPrice = useModelPrice(fallback);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [highlight, setHighlight] = useState(0);
  const close = useCallback(() => {
    setOpen(false);
    setQuery("");
  }, []);
  useDismiss(root, open, close);
  useEffect(() => {
    if (open) {
      input.current?.focus();
    }
  }, [open]);

  const needle = query.trim().toLowerCase();
  const catalog = models.filter(listedModel);
  const listed = value && listedModel(value) && !catalog.includes(value) ? [value, ...catalog] : catalog;
  const choices: Choice[] = [
    ...(needle ? [] : [{ value: "", label: "Default", detail: fallback || "Model from Settings", price: formatPrice(fallbackPrice) }]),
    ...listed
      .filter((model) => !needle || model.toLowerCase().includes(needle))
      .map((model) => ({ value: model, label: model, detail: "", price: formatPrice(prices[model]) })),
  ];
  const typed = query.trim();
  if (typed && listedModel(typed) && !listed.some((model) => model.toLowerCase() === typed.toLowerCase())) {
    choices.push({ value: typed, label: `Use “${typed}”`, detail: "A model id from your provider", price: "" });
  }
  const active = choices.length > 0 ? Math.min(highlight, choices.length - 1) : 0;
  const acceptsThinking = modelAcceptsThinking(provider, selected);
  const thinkingValue = THINKING_LEVELS.some((level) => level.value === thinking) ? thinking : "";

  const pick = (choice: Choice) => {
    onChange(choice.value);
    close();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      setHighlight((active + (event.key === "ArrowDown" ? 1 : choices.length - 1)) % Math.max(1, choices.length));
    } else if (event.key === "Enter") {
      event.preventDefault();
      if (choices[active]) {
        pick(choices[active]);
      }
    } else if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      close();
    }
  };

  return (
    <div id={id} ref={root} className={cn("min-w-0", className)}>
      <button
        id={`${id}-button`}
        type="button"
        aria-haspopup="dialog"
        aria-expanded={open}
        title={value ? "Model for this task" : "Model for the next task"}
        className="block w-full truncate text-center text-[10px] text-zinc-500 hover:text-zinc-200"
        onClick={() => {
          if (open) {
            close();
          } else {
            setQuery("");
            setHighlight(0);
            setOpen(true);
          }
        }}
      >
        {selected || "No model"}
      </button>
      {open ? (
        <div
          id={`${id}-popover`}
          role="dialog"
          aria-label="Model"
          data-agent-popover
          className="absolute bottom-full left-0 z-40 mb-2 w-[min(20rem,100%)] overflow-hidden rounded-lg bg-zinc-900 shadow-2xl ring-1 ring-zinc-700"
        >
          <div id={`${id}-heading`} className="flex justify-between border-b border-zinc-800 px-2.5 py-1 text-[10px] text-zinc-500">
            <span id={`${id}-heading-model`}>Model</span>
            <span id={`${id}-heading-price`}>$ per 1M tokens · in / out</span>
          </div>
          <input
            id={`${id}-input`}
            ref={input}
            role="combobox"
            aria-expanded={open}
            aria-controls={`${id}-list`}
            aria-autocomplete="list"
            aria-label="Search models"
            spellCheck={false}
            placeholder="Search models"
            className="h-7 w-full border-b border-zinc-800 bg-transparent px-2.5 text-xs text-zinc-100 outline-none placeholder:text-zinc-500"
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
              setHighlight(0);
            }}
            onKeyDown={onKeyDown}
          />
          <ul id={`${id}-list`} role="listbox" aria-label="Models" className="max-h-64 overflow-y-auto p-1">
            {choices.map((choice, index) => (
              <li id={`${id}-option-${index}`} key={`${choice.value}-${index}`} role="option" aria-selected={choice.value === value}>
                <button
                  id={`${id}-option-${index}-button`}
                  type="button"
                  className={cn("flex w-full items-center gap-2 rounded px-1.5 py-1 text-left", index === active ? "bg-zinc-800" : "hover:bg-zinc-800/60")}
                  onMouseDown={(event) => event.preventDefault()}
                  onMouseEnter={() => setHighlight(index)}
                  onClick={() => pick(choice)}
                >
                  <Check id={`${id}-option-${index}-check`} className={cn("size-3 shrink-0 text-sky-400", choice.value !== value && "invisible")} aria-hidden />
                  <span id={`${id}-option-${index}-body`} className="min-w-0 flex-1">
                    <span id={`${id}-option-${index}-label`} className="block truncate text-xs text-zinc-100">
                      {choice.label}
                    </span>
                    {choice.detail ? (
                      <span id={`${id}-option-${index}-detail`} className="block truncate text-[10px] text-zinc-500">
                        {choice.detail}
                      </span>
                    ) : null}
                  </span>
                  <span id={`${id}-option-${index}-price`} className={cn("shrink-0 text-[11px] tabular-nums", choice.price ? "text-emerald-400/90" : "text-zinc-600")}>
                    {choice.price || (choice.value && !loading ? "—" : "")}
                  </span>
                </button>
              </li>
            ))}
            {choices.length === 0 && !loading ? (
              <li id={`${id}-empty`} className="px-2 py-1.5 text-[11px] text-zinc-500">
                No models
              </li>
            ) : null}
            {loading ? (
              <li id={`${id}-loading`} className="px-2 py-1.5 text-[11px] text-zinc-500">
                Loading models…
              </li>
            ) : null}
          </ul>
          {acceptsThinking ? (
            <div id={`${id}-thinking`} className="border-t border-zinc-800 p-2">
              <div id={`${id}-thinking-label`} className="px-0.5 pb-1 text-[10px] text-zinc-500" title="How long the model thinks before it answers. Higher costs more.">
                Thinking
              </div>
              <div id={`${id}-thinking-levels`} role="radiogroup" aria-label="Thinking" className="flex rounded-md bg-zinc-950 p-0.5 ring-1 ring-zinc-800">
                {THINKING_LEVELS.map((level) => {
                  const selectedLevel = level.value === thinkingValue;
                  return (
                    <button
                      id={`${id}-thinking-${level.value || "default"}`}
                      key={level.label}
                      type="button"
                      role="radio"
                      aria-checked={selectedLevel}
                      className={cn(
                        "h-6 flex-1 rounded px-1 text-[11px] font-medium text-zinc-400 hover:text-zinc-100",
                        selectedLevel && "bg-sky-700 text-white",
                      )}
                      onClick={() => onThinking(level.value)}
                    >
                      {level.label}
                    </button>
                  );
                })}
              </div>
            </div>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
