import type { CSSProperties } from "react";
import { cn } from "../lib/cn";

type SkeletonProps = {
  id: string;
  className?: string;
  style?: CSSProperties;
};

/** A placeholder block that pulses while content loads. */
export function Skeleton({ id, className, style }: SkeletonProps) {
  return <div id={id} aria-hidden className={cn("animate-pulse rounded-md bg-zinc-800", className)} style={style} />;
}

type SkeletonRowsProps = {
  id: string;
  rows?: number;
  /** Show an icon square at the start of each row. */
  icon?: boolean;
  className?: string;
};

/** Rows of placeholder lines for a list that is still loading. */
export function SkeletonRows({ id, rows = 5, icon = false, className }: SkeletonRowsProps) {
  return (
    <div id={id} role="status" aria-label="Loading" className={cn("flex flex-col gap-3", className)}>
      {Array.from({ length: rows }, (_, index) => (
        <div id={`${id}-${index}`} key={index} className="flex items-center gap-3">
          {icon ? <Skeleton id={`${id}-${index}-icon`} className="size-8 shrink-0" /> : null}
          <div id={`${id}-${index}-lines`} className="flex min-w-0 flex-1 flex-col gap-1.5">
            <Skeleton id={`${id}-${index}-line`} className="h-3" style={{ width: `${70 - ((index * 13) % 35)}%` }} />
            <Skeleton id={`${id}-${index}-sub`} className="h-2.5 w-1/3 opacity-60" />
          </div>
        </div>
      ))}
    </div>
  );
}
