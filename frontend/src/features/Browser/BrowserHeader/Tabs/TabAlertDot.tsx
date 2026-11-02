import { cn } from "@/lib/cn";

type TabAlertDotProps = {
  id: string;
  title: string;
  className?: string;
};

/** Red mark on a tab that crashed, hung, logged errors, or is using a lot of memory. */
export function TabAlertDot({ id, title, className }: TabAlertDotProps) {
  return (
    <span
      id={id}
      aria-hidden
      title={title}
      className={cn("pointer-events-none absolute z-10 size-2 rounded-full bg-red-500 ring-2 ring-zinc-950", className)}
    />
  );
}
