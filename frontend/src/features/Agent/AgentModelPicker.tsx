import { useEffect, useState } from "react";
import { listAiModels } from "@/lib/browser-client";
import { cn } from "@/lib/cn";

let modelsRequest: Promise<string[]> | null = null;

function loadModels() {
  modelsRequest ??= listAiModels().catch(() => {
    modelsRequest = null;
    return [];
  });
  return modelsRequest;
}

type Props = {
  id: string;
  value: string;
  /** Model used when `value` is blank. */
  fallback: string;
  onChange: (model: string) => void;
  disabled?: boolean;
  className?: string;
};

/** Picks the model for the next task. Blank uses the agent model from Settings. */
export function AgentModelPicker({ id, value, fallback, onChange, disabled, className }: Props) {
  const [models, setModels] = useState<string[]>([]);

  useEffect(() => {
    let live = true;
    void loadModels().then((list) => live && setModels(list));
    return () => {
      live = false;
    };
  }, []);

  const choices = value && !models.includes(value) ? [value, ...models] : models;

  return (
    <select
      id={id}
      aria-label="Model"
      title="Model for the next task"
      disabled={disabled}
      className={cn("h-7 min-w-0 rounded-md border border-zinc-800 bg-zinc-900 px-1 text-[11px] text-zinc-300 outline-none focus:border-sky-500", className)}
      value={value}
      onChange={(event) => onChange(event.target.value)}
    >
      <option id={`${id}-default`} value="">
        {fallback ? `Default · ${fallback}` : "Default model"}
      </option>
      {choices.map((model) => (
        <option id={`${id}-${model}`} key={model} value={model}>
          {model}
        </option>
      ))}
    </select>
  );
}
