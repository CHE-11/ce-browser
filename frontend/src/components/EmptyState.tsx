import type { ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import { cn } from "../lib/cn";

type EmptyStateProps = {
  id: string;
  icon: LucideIcon;
  title: string;
  hint?: string;
  /** A button or link that fills the empty space, such as Add or Import. */
  action?: ReactNode;
  className?: string;
};

/** What a ce:// page shows when a list has nothing in it yet, or a search matched nothing. */
export function EmptyState({ id, icon: Icon, title, hint, action, className }: EmptyStateProps) {
  return (
    <div id={id} role="status" className={cn("flex flex-col items-center justify-center gap-2 px-6 py-14 text-center", className)}>
      <span id={`${id}-icon`} className="mb-1 inline-flex size-11 items-center justify-center rounded-full bg-zinc-900 text-zinc-500 ring-1 ring-zinc-800">
        <Icon className="size-5" aria-hidden />
      </span>
      <p id={`${id}-title`} className="text-sm font-medium text-zinc-300">
        {title}
      </p>
      {hint ? (
        <p id={`${id}-hint`} className="max-w-sm text-xs leading-relaxed text-zinc-500">
          {hint}
        </p>
      ) : null}
      {action ? (
        <div id={`${id}-action`} className="mt-2">
          {action}
        </div>
      ) : null}
    </div>
  );
}
