import type { ReactNode } from "react";
import { cn } from "@/lib/cn";

export function Field({ id, label, children, className }: { id: string; label: string; children: ReactNode; className?: string }) {
  return (
    <label id={id} className={cn("flex min-w-0 flex-col gap-1 text-xs text-zinc-400", className)}>
      <span id={`${id}-label`}>{label}</span>
      {children}
    </label>
  );
}
