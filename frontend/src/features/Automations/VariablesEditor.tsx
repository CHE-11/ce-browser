import { Plus, Trash2 } from "lucide-react";
import { Button } from "@/components/Button";
import { IconButton } from "@/components/IconButton";
import { cn } from "@/lib/cn";
import { inputClassName } from "@features/Agent/agent-styles";
import { BUILT_IN_VARIABLES } from "./flow-model";

export type VariableRow = { name: string; value: string };

export function rowsFromVariables(variables: Record<string, string>): VariableRow[] {
  return Object.entries(variables).map(([name, value]) => ({ name, value }));
}

/** Rows with a name, without secrets. A later row with the same name wins. */
export function variablesFromRows(rows: VariableRow[]) {
  const out: Record<string, string> = {};
  for (const row of rows) {
    const name = row.name.trim();
    if (name && !name.startsWith("secret:")) {
      out[name] = row.value;
    }
  }
  return out;
}

function rowIssue(row: VariableRow, rows: VariableRow[], index: number) {
  const name = row.name.trim();
  if (!name) {
    return row.value ? "Give it a name." : "";
  }
  if (name.startsWith("secret:")) {
    return "Save secrets in Settings, under Automation.";
  }
  if (/[{}]/.test(name)) {
    return "Leave out the braces.";
  }
  if (BUILT_IN_VARIABLES.includes(name)) {
    return "A run fills this one in itself and replaces your value.";
  }
  if (rows.findIndex((other) => other.name.trim() === name) !== index) {
    return "Another row has this name.";
  }
  return "";
}

type VariablesEditorProps = {
  id: string;
  rows: VariableRow[];
  onChange: (rows: VariableRow[]) => void;
};

export function VariablesEditor({ id, rows, onChange }: VariablesEditorProps) {
  const update = (index: number, change: Partial<VariableRow>) => onChange(rows.map((row, at) => (at === index ? { ...row, ...change } : row)));
  return (
    <section id={id} className="flex flex-col gap-2">
      <h2 id={`${id}-title`} className="text-sm font-semibold text-zinc-200">
        Variables
      </h2>
      <p id={`${id}-hint`} className="text-xs leading-relaxed text-zinc-500">
        Steps use them as {"{{name}}"}. {BUILT_IN_VARIABLES.map((name) => `{{${name}}}`).join(", ")} are filled in by runs and triggers. Keep passwords in Settings, under
        Automation, and type them with {"{{secret:name}}"}.
      </p>
      {rows.length ? (
        <ul id={`${id}-rows`} className="flex flex-col gap-1.5">
          {rows.map((row, index) => {
            const issue = rowIssue(row, rows, index);
            return (
              <li id={`${id}-row-${index}`} key={index} className="flex flex-col gap-0.5">
                <div id={`${id}-row-${index}-fields`} className="flex items-center gap-2">
                  <input
                    id={`${id}-row-${index}-name`}
                    className={cn(inputClassName, "w-48 font-mono text-xs", issue && "border-amber-700")}
                    value={row.name}
                    placeholder="name"
                    aria-label="Variable name"
                    onChange={(event) => update(index, { name: event.target.value })}
                  />
                  <input
                    id={`${id}-row-${index}-value`}
                    className={cn(inputClassName, "flex-1 font-mono text-xs")}
                    value={row.value}
                    placeholder="value"
                    aria-label={`Value of ${row.name || "the variable"}`}
                    onChange={(event) => update(index, { value: event.target.value })}
                  />
                  <IconButton id={`${id}-row-${index}-remove`} label="Remove variable" className="size-7" onClick={() => onChange(rows.filter((_, at) => at !== index))}>
                    <Trash2 id={`${id}-row-${index}-remove-icon`} aria-hidden />
                  </IconButton>
                </div>
                {issue ? (
                  <span id={`${id}-row-${index}-issue`} className="text-[11px] text-amber-300">
                    {issue}
                  </span>
                ) : null}
              </li>
            );
          })}
        </ul>
      ) : null}
      <div id={`${id}-actions`}>
        <Button id={`${id}-add`} size="xs" variant="outline" onClick={() => onChange([...rows, { name: "", value: "" }])}>
          <Plus id={`${id}-add-icon`} className="size-3.5" aria-hidden />
          Add variable
        </Button>
      </div>
    </section>
  );
}
