import { forwardRef, type ButtonHTMLAttributes } from "react";
import { cn } from "../lib/cn";
import { focusRing } from "../lib/focus-ring";

type IconButtonProps = Omit<ButtonHTMLAttributes<HTMLButtonElement>, "id"> & {
  id: string;
  label: string;
  active?: boolean;
};

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(function IconButton(
  { id, label, active, className, type = "button", ...props },
  ref,
) {
  return (
    <button
      id={id}
      ref={ref}
      type={type}
      title={label}
      aria-label={label}
      data-active={active ? "true" : undefined}
      className={cn(
        "chrome-toolbar-button inline-flex size-8 shrink-0 items-center justify-center rounded-full text-zinc-300 transition-colors hover:bg-zinc-800 hover:text-zinc-100 disabled:pointer-events-none disabled:opacity-35 [&_svg]:size-4",
        focusRing,
        active && "bg-zinc-800 text-zinc-100",
        className,
      )}
      {...props}
    />
  );
});
