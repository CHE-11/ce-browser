import { useCallback, useRef, useState, type KeyboardEvent } from "react";
import { Check, ChevronDown } from "lucide-react";
import { cn } from "@/lib/cn";
import { useDismiss } from "@/lib/use-dismiss";
import { formatPrice, useModelCatalog, useModelPrice } from "@/lib/use-model-prices";

type Props = {
  id: string;
  value: string;
  /** Model used when `value` is blank. */
  fallback: string;
  onChange: (model: string) => void;
  className?: string;
};

type Choice = { value: string; label: string; detail: string; price: string };

/** Combo box for the next task's model, with its price per million tokens. Blank uses the agent model from Settings. */
export function AgentModelPicker({ id, value, fallback, onChange, className }: Props) {
  const root = useRef<HTMLDivElement | null>(null);
  const input = useRef<HTMLInputElement | null>(null);
  const { models, prices, loading } = useModelCatalog();
  const fallbackPrice = useModelPrice(fallback);
  const selectedPrice = useModelPrice(value || fallback);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState<string | null>(null);
  const [highlight, setHighlight] = useState(0);
  const close = useCallback(() => {
    setOpen(false);
    setQuery(null);
  }, []);
  useDismiss(root, open, close);

  const needle = (query ?? "").trim().toLowerCase();
  const listed = value && !models.includes(value) ? [value, ...models] : models;
  const choices: Choice[] = [
    ...(needle
      ? []
      : [{ value: "", label: "Default", detail: fallback || "Model from Settings", price: formatPrice(fallbackPrice) }]),
    ...listed
      .filter((model) => !needle || model.toLowerCase().includes(needle))
      .map((model) => ({ value: model, label: model, detail: "", price: formatPrice(prices[model]) })),
  ];
  const typed = query?.trim() ?? "";
  if (typed && !listed.some((model) => model.toLowerCase() === typed.toLowerCase())) {
    choices.push({ value: typed, label: `Use “${typed}”`, detail: "A model id from your provider", price: "" });
  }
  const active = choices.length > 0 ? Math.min(highlight, choices.length - 1) : 0;

  const pick = (choice: Choice) => {
    onChange(choice.value);
    close();
    input.current?.blur();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      if (!open) {
        setOpen(true);
        return;
      }
      setHighlight((active + (event.key === "ArrowDown" ? 1 : choices.length - 1)) % Math.max(1, choices.length));
    } else if (event.key === "Enter") {
      event.preventDefault();
      if (open && choices[active]) {
        pick(choices[active]);
      }
    } else if (event.key === "Escape" && open) {
      event.preventDefault();
      close();
    }
  };

  const shownPrice = formatPrice(selectedPrice);

  return (
    <div id={id} ref={root} className={cn("relative min-w-0", className)}>
      <div id={`${id}-field`} className="flex h-7 items-center rounded-md border border-zinc-800 bg-zinc-900 focus-within:border-sky-500">
        <input
          id={`${id}-input`}
          ref={input}
          role="combobox"
          aria-expanded={open}
          aria-controls={`${id}-list`}
          aria-autocomplete="list"
          aria-label="Model"
          title="Model for the next task"
          spellCheck={false}
          className="h-full min-w-0 flex-1 bg-transparent px-1.5 text-[11px] text-zinc-200 outline-none placeholder:text-zinc-400"
          placeholder={fallback ? `Default · ${fallback}` : "Default model"}
          value={query ?? value}
          onFocus={(event) => {
            setOpen(true);
            event.currentTarget.select();
          }}
          onChange={(event) => {
            setQuery(event.target.value);
            setOpen(true);
            setHighlight(0);
          }}
          onKeyDown={onKeyDown}
        />
        {shownPrice && query === null ? (
          <span id={`${id}-price`} className="shrink-0 pr-0.5 text-[10px] tabular-nums text-emerald-400/90" title="US dollars per million tokens, in / out">
            {shownPrice}
          </span>
        ) : null}
        <button
          id={`${id}-toggle`}
          type="button"
          tabIndex={-1}
          aria-label="Show models"
          className="flex h-full w-5 shrink-0 items-center justify-center text-zinc-500 hover:text-zinc-200"
          onClick={() => {
            if (open) {
              close();
            } else {
              input.current?.focus();
            }
          }}
        >
          <ChevronDown id={`${id}-toggle-icon`} className="size-3.5" aria-hidden />
        </button>
      </div>
      {open ? (
        <div
          id={`${id}-popover`}
          data-agent-popover
          className="absolute right-0 top-full z-40 mt-1 w-[min(20rem,calc(100vw-1.5rem))] overflow-hidden rounded-lg bg-zinc-900 shadow-2xl ring-1 ring-zinc-700"
        >
          <div id={`${id}-heading`} className="flex justify-between border-b border-zinc-800 px-2.5 py-1 text-[10px] text-zinc-500">
            <span id={`${id}-heading-model`}>Model</span>
            <span id={`${id}-heading-price`}>$ per 1M tokens · in / out</span>
          </div>
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
            {loading ? (
              <li id={`${id}-loading`} className="px-2 py-1.5 text-[11px] text-zinc-500">
                Loading models…
              </li>
            ) : null}
          </ul>
        </div>
      ) : null}
    </div>
  );
}
