import { useEffect, useLayoutEffect, useRef, type KeyboardEvent, type ReactNode } from "react";
import { closePopup, resizePopup } from "../lib/browser-client";
import { cn } from "../lib/cn";

type PopupFrameProps = {
  id: string;
  children: ReactNode;
  className?: string;
  /** Grow or shrink the native popup to the content height. */
  fit?: boolean;
  maxHeight?: number;
};

export function PopupFrame({ id, children, className, fit = true, maxHeight = 640 }: PopupFrameProps) {
  const ref = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    const element = ref.current;
    if (!fit || !element) {
      return;
    }
    let last = 0;
    const report = () => {
      const height = Math.min(maxHeight, Math.ceil(element.getBoundingClientRect().height) + 2);
      if (Math.abs(height - last) >= 1) {
        last = height;
        void resizePopup(height).catch(() => undefined);
      }
    };
    report();
    const observer = new ResizeObserver(report);
    observer.observe(element);
    return () => observer.disconnect();
  }, [fit, maxHeight]);

  useEffect(() => {
    const onKey = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") {
        void closePopup();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div
      id={id}
      className={cn(
        "flex h-full flex-col overflow-hidden rounded-xl bg-zinc-900 text-zinc-100 ring-1 ring-inset ring-zinc-700",
        className,
      )}
      onContextMenu={(event) => event.preventDefault()}
    >
      <div id={`${id}-scroller`} className="min-h-0 flex-1 overflow-y-auto">
        <div id={`${id}-content`} ref={ref}>
          {children}
        </div>
      </div>
    </div>
  );
}

export function PopupMenu({ id, label, children }: { id: string; label: string; children: ReactNode }) {
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") {
      return;
    }
    event.preventDefault();
    const items = [...event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)')];
    const index = items.findIndex((item) => item === document.activeElement);
    const delta = event.key === "ArrowDown" ? 1 : -1;
    items[index < 0 ? 0 : (index + delta + items.length) % items.length]?.focus();
  };
  return (
    <div id={id} role="menu" aria-label={label} className="flex flex-col gap-0.5 px-1.5 py-1.5" onKeyDown={onKeyDown}>
      {children}
    </div>
  );
}

type PopupMenuItemProps = {
  id: string;
  icon?: ReactNode;
  label: string;
  shortcut?: string;
  disabled?: boolean;
  danger?: boolean;
  onClick: () => void;
};

export function PopupMenuItem({ id, icon, label, shortcut, disabled, danger, onClick }: PopupMenuItemProps) {
  return (
    <button
      id={id}
      type="button"
      role="menuitem"
      disabled={disabled}
      className={cn(
        "group flex h-9 w-full items-center gap-3 rounded-lg px-2.5 text-left text-sm text-zinc-100 transition-colors hover:bg-zinc-800 focus-visible:bg-zinc-800 focus-visible:outline-none disabled:opacity-40 disabled:hover:bg-transparent",
        danger && "text-red-300",
      )}
      onClick={onClick}
    >
      <span id={`${id}-icon`} className="inline-flex size-4 shrink-0 items-center justify-center text-zinc-400 group-hover:text-zinc-200 [&_svg]:size-4">
        {icon}
      </span>
      <span id={`${id}-label`} className="min-w-0 flex-1 truncate">
        {label}
      </span>
      {shortcut ? (
        <span id={`${id}-shortcut`} className="shrink-0 text-[11px] text-zinc-500">
          {shortcut}
        </span>
      ) : null}
    </button>
  );
}

export function PopupMenuSeparator({ id }: { id: string }) {
  return <div id={id} role="separator" className="mx-2 my-1 h-px bg-zinc-800" />;
}
