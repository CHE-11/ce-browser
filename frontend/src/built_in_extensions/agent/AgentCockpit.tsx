import { ListRestart, RefreshCw, SkipForward, ShieldCheck } from "lucide-react";
import { Button } from "@/components/Button";

type Action = "retry" | "skip" | "replan" | "verify";

type Props = {
  id: string;
  disabled?: boolean;
  onAction: (action: Action) => void;
};

const ACTIONS: { value: Action; label: string; Icon: typeof RefreshCw; title: string }[] = [
  { value: "retry", label: "Retry", Icon: RefreshCw, title: "Retry the last failed or blocked action" },
  { value: "skip", label: "Skip", Icon: SkipForward, title: "Skip the current step and continue" },
  { value: "replan", label: "Replan", Icon: ListRestart, title: "Rewrite the plan from the current page" },
  { value: "verify", label: "Verify", Icon: ShieldCheck, title: "Check that the last action succeeded" },
];

/** Recovery controls for a run that hit trouble or needs a checkpoint. */
export function AgentCockpit({ id, disabled, onAction }: Props) {
  return (
    <div id={id} className="flex flex-wrap gap-1.5 rounded-lg bg-zinc-900/70 p-2 ring-1 ring-zinc-800">
      <span id={`${id}-label`} className="w-full px-0.5 text-[10px] font-medium uppercase tracking-wide text-zinc-500">
        Cockpit
      </span>
      {ACTIONS.map(({ value, label, Icon, title }) => (
        <Button id={`${id}-${value}`} key={value} size="xs" variant="outline" disabled={disabled} title={title} onClick={() => onAction(value)}>
          <Icon id={`${id}-${value}-icon`} className="size-3" aria-hidden />
          {label}
        </Button>
      ))}
    </div>
  );
}
