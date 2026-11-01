import { cn } from "../lib/cn";

type LoadBarProps = {
  id: string;
  /** 0 to 100. */
  progress: number;
  loading: boolean;
};

/** A thin bar along the bottom edge that fills while the page loads, then fades. */
export function LoadBar({ id, progress, loading }: LoadBarProps) {
  const running = loading && progress > 0 && progress < 100;
  const starting = running && progress <= 10;
  return (
    <div
      id={id}
      role="progressbar"
      aria-label="Page loading"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={running ? progress : undefined}
      aria-hidden={!running}
      className="pointer-events-none absolute inset-x-0 bottom-0 h-0.5 overflow-hidden"
    >
      <div
        id={`${id}-fill`}
        className={cn(
          "h-full rounded-r-full bg-sky-500",
          starting ? "transition-none" : "transition-[width,opacity] duration-300 ease-out",
          running ? "opacity-100" : "opacity-0 delay-150",
        )}
        style={{ width: `${running ? progress : 100}%` }}
      />
    </div>
  );
}
