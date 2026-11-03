import { useEffect, useMemo, useRef, useState } from "react";
import { ChevronDown, RefreshCw } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import { modelPriceMarks } from "./model-label";
import { inputClass } from "./SettingsControls";

export function ModelPicker({
  id,
  value,
  models,
  loading,
  remote,
  blankLabel,
  onChange,
  onRefresh,
  onFocus,
}: {
  id: string;
  value: string;
  models: string[];
  loading: boolean;
  remote: boolean;
  blankLabel?: string;
  onChange: (value: string) => void;
  onRefresh: () => void;
  onFocus?: () => void;
}) {
  const choices = useMemo(() => {
    if (!value || models.includes(value)) {
      return models;
    }
    return [value, ...models];
  }, [models, value]);

  if (!remote) {
    return (
      <>
        <input
          id={`${id}-input`}
          className={inputClass}
          list={`${id}-list`}
          value={value}
          spellCheck={false}
          onChange={(event) => onChange(event.target.value)}
          onFocus={onFocus}
        />
        <datalist id={`${id}-list`}>
          {models.map((model, index) => (
            <option id={`${id}-option-${index}`} key={model} value={model} />
          ))}
        </datalist>
      </>
    );
  }

  const showLoading = loading && choices.length === 0 && !blankLabel;

  return (
    <div id={`${id}-picker`} className="flex min-w-0 flex-1 items-center gap-1">
      <ModelMenu
        id={id}
        value={value}
        choices={choices}
        blankLabel={blankLabel}
        loading={showLoading}
        onChange={onChange}
      />
      <IconButton id={`${id}-refresh`} label="Refresh models" disabled={loading} onClick={onRefresh}>
        <RefreshCw id={`${id}-refresh-icon`} className={loading ? "animate-spin" : undefined} aria-hidden />
      </IconButton>
    </div>
  );
}

function ModelMenu({
  id,
  value,
  choices,
  blankLabel,
  loading,
  onChange,
}: {
  id: string;
  value: string;
  choices: string[];
  blankLabel?: string;
  loading: boolean;
  onChange: (value: string) => void;
}) {
  const rootRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const options = useMemo(() => {
    const rows = choices.map((model) => ({ value: model, label: model, marks: modelPriceMarks(model) }));
    return blankLabel ? [{ value: "", label: blankLabel, marks: "" }, ...rows] : rows;
  }, [blankLabel, choices]);
  const selected = options.findIndex((option) => option.value === value);
  const [highlight, setHighlight] = useState(Math.max(selected, 0));

  useEffect(() => {
    if (!open) {
      return;
    }
    const onPointer = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    window.addEventListener("pointerdown", onPointer);
    return () => window.removeEventListener("pointerdown", onPointer);
  }, [open]);

  const choose = (next: string) => {
    onChange(next);
    setOpen(false);
  };

  const current = options.find((option) => option.value === value);

  return (
    <div
      id={`${id}-menu`}
      ref={rootRef}
      className="relative min-w-0 flex-1"
      onKeyDown={(event) => {
        if (loading) {
          return;
        }
        if (event.key === "Escape") {
          setOpen(false);
          return;
        }
        if (event.key === "ArrowDown" || event.key === "ArrowUp") {
          event.preventDefault();
          setOpen(true);
          setHighlight((index) => {
            const start = index < 0 ? Math.max(selected, 0) : index;
            const next = event.key === "ArrowDown" ? start + 1 : start - 1;
            return Math.max(0, Math.min(options.length - 1, next));
          });
          return;
        }
        if (event.key === "Enter" && open) {
          event.preventDefault();
          const option = options[highlight];
          if (option) {
            choose(option.value);
          }
        }
      }}
    >
      <button
        id={`${id}-input`}
        type="button"
        className={cn(inputClass, "flex items-center gap-2 text-left", focusRing)}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-busy={loading}
        disabled={loading}
        onClick={() => {
          setHighlight(Math.max(selected, 0));
          setOpen((currentOpen) => !currentOpen);
        }}
      >
        <span id={`${id}-current`} className="min-w-0 flex-1 truncate">
          {loading ? "Loading models…" : (current?.label ?? value)}
        </span>
        {current?.marks ? (
          <span id={`${id}-current-price`} className="w-[4ch] shrink-0 text-right font-mono text-zinc-400">
            {current.marks}
          </span>
        ) : null}
        <ChevronDown id={`${id}-chevron`} className="size-4 shrink-0 text-zinc-500" aria-hidden />
      </button>
      {open ? (
        <div
          id={`${id}-list`}
          role="listbox"
          aria-label="Models"
          className="absolute top-full right-0 left-0 z-30 mt-1 max-h-64 overflow-y-auto rounded-md border border-zinc-700 bg-zinc-950 py-1 shadow-lg"
        >
          {options.map((option, index) => (
            <button
              id={option.value ? `${id}-option-${index}` : `${id}-blank`}
              key={option.value || "blank"}
              type="button"
              role="option"
              aria-selected={option.value === value}
              className={cn(
                "flex w-full items-center gap-3 px-2 py-1.5 text-left text-sm text-zinc-100",
                (option.value === value || index === highlight) && "bg-zinc-800",
              )}
              onMouseEnter={() => setHighlight(index)}
              onClick={() => choose(option.value)}
            >
              <span id={`${id}-option-${index}-label`} className="min-w-0 flex-1 truncate">
                {option.label}
              </span>
              {option.marks ? (
                <span id={`${id}-option-${index}-price`} className="w-[4ch] shrink-0 text-right font-mono text-zinc-400">
                  {option.marks}
                </span>
              ) : null}
            </button>
          ))}
        </div>
      ) : null}
    </div>
  );
}
