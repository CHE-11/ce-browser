import {
  createContext,
  forwardRef,
  useContext,
  useLayoutEffect,
  type ButtonHTMLAttributes,
  type MouseEvent,
  type MouseEventHandler,
  type RefObject,
} from "react";
import { cn } from "../lib/cn";
import { focusRing } from "../lib/focus-ring";

/** Toolbar entries register the icon's click here and run it from the entry box. */
export const ToolbarEntryClickContext = createContext<RefObject<MouseEventHandler<HTMLElement> | null> | null>(null);

type IconButtonProps = Omit<ButtonHTMLAttributes<HTMLButtonElement>, "id"> & {
  id: string;
  label: string;
  active?: boolean;
};

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(function IconButton(
  { id, label, active, className, type = "button", onClick, ...props },
  ref,
) {
  const entryClick = useContext(ToolbarEntryClickContext);

  useLayoutEffect(() => {
    if (!entryClick) {
      return;
    }
    entryClick.current = onClick
      ? (event) => {
          onClick(event as unknown as MouseEvent<HTMLButtonElement>);
        }
      : null;
    return () => {
      entryClick.current = null;
    };
  }, [entryClick, onClick]);

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
      onClick={entryClick ? undefined : onClick}
    />
  );
});
