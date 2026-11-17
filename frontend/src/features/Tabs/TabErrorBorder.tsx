import { cn } from "@/lib/cn";

type TabErrorBorderProps = {
  id: string;
  title: string;
  className?: string;
};

/** Red line along the bottom of a tab that crashed, hung, logged errors, or is using a lot of memory. */
export function TabErrorBorder({ id, title, className }: TabErrorBorderProps) {
  return (
    <span
      id={id}
      aria-hidden
      title={title}
      className={cn("pointer-events-none absolute inset-x-0 bottom-0 z-10 h-0.5 bg-red-500", className)}
    />
  );
}
