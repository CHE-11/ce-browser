import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import { cn } from "../lib/cn";

export const contextMenuPanelClass = "min-w-48 rounded-md border border-zinc-700 bg-zinc-900 py-0.5 shadow-2xl";

const itemClass =
  "flex h-6 w-full items-center justify-between gap-2 whitespace-nowrap px-2 text-left text-xs leading-4 text-zinc-100 hover:bg-zinc-800 focus-visible:bg-zinc-800 focus-visible:outline-none disabled:opacity-40 disabled:hover:bg-transparent";

type ContextMenuProps = {
  id: string;
  label: string;
  x: number;
  y: number;
  onClose: () => void;
  scrollParentId?: string;
  className?: string;
  children: ReactNode;
  onKeyDown?: (event: ReactKeyboardEvent<HTMLDivElement>) => void;
  onEscape?: () => boolean;
};

export function ContextMenu({
  id,
  label,
  x,
  y,
  onClose,
  scrollParentId,
  className,
  children,
  onKeyDown,
  onEscape,
}: ContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);
  const [point, setPoint] = useState(() => clampMenuToViewport(x, y, 240, 200));

  useLayoutEffect(() => {
    const menu = menuRef.current;
    if (!menu) {
      return;
    }
    const rect = menu.getBoundingClientRect();
    setPoint(clampMenuToViewport(x, y, rect.width, rect.height));
    menu.querySelector<HTMLButtonElement>('[role="menuitem"]:not(:disabled)')?.focus({ preventScroll: true });
  }, [x, y]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }
      event.preventDefault();
      if (onEscape?.()) {
        return;
      }
      onClose();
    };
    const onPointer = (event: PointerEvent) => {
      const target = event.target;
      if (target instanceof Node && menuRef.current?.contains(target)) {
        return;
      }
      onClose();
    };
    const scroller = scrollParentId ? document.getElementById(scrollParentId) : null;
    window.addEventListener("keydown", onKey);
    window.addEventListener("pointerdown", onPointer, true);
    window.addEventListener("resize", onClose);
    scroller?.addEventListener("scroll", onClose);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("pointerdown", onPointer, true);
      window.removeEventListener("resize", onClose);
      scroller?.removeEventListener("scroll", onClose);
    };
  }, [onClose, onEscape, scrollParentId]);

  return createPortal(
    <div
      id={id}
      ref={menuRef}
      role="menu"
      aria-label={label}
      tabIndex={-1}
      className={cn(contextMenuPanelClass, "fixed z-50", className)}
      style={{ left: point.left, top: point.top }}
      onContextMenu={(event) => event.preventDefault()}
      onKeyDown={(event) => {
        onKeyDown?.(event);
        if (!event.defaultPrevented) {
          moveMenuFocus(event);
        }
      }}
    >
      {children}
    </div>,
    document.body,
  );
}

export function ContextMenuSeparator({ id }: { id: string }) {
  return <div id={id} role="separator" className="my-0.5 h-px bg-zinc-800" />;
}

export function ContextMenuItem({
  id,
  icon,
  label,
  title,
  disabled,
  danger,
  hasPopup,
  expanded,
  scope,
  labelClassName,
  onClick,
  trailing,
}: {
  id: string;
  icon: ReactNode;
  label: string;
  title?: string;
  disabled?: boolean;
  danger?: boolean;
  hasPopup?: boolean;
  expanded?: boolean;
  scope?: string;
  labelClassName?: string;
  onClick: () => void;
  trailing?: ReactNode;
}) {
  return (
    <button
      id={id}
      type="button"
      role="menuitem"
      data-scope={scope}
      title={title}
      aria-haspopup={hasPopup ? "menu" : undefined}
      aria-expanded={hasPopup ? Boolean(expanded) : undefined}
      className={cn(itemClass, danger && "text-red-300 hover:bg-red-950/70 focus-visible:bg-red-950/70")}
      disabled={disabled}
      onClick={onClick}
    >
      <span id={`${id}-content`} className="flex min-w-0 items-center gap-1.5">
        <span
          id={`${id}-icon`}
          className={cn(
            "inline-flex size-3.5 shrink-0 items-center justify-center [&_svg]:size-3.5",
            danger ? "text-current" : "text-zinc-400",
          )}
        >
          {icon}
        </span>
        <span id={`${id}-label`} className={cn("truncate", labelClassName)}>
          {label}
        </span>
      </span>
      {trailing ?? null}
    </button>
  );
}

export function clampMenuToViewport(x: number, y: number, width: number, height: number) {
  const margin = 8;
  const maxLeft = Math.max(margin, window.innerWidth - width - margin);
  const maxTop = Math.max(margin, window.innerHeight - height - margin);
  return {
    left: Math.min(Math.max(margin, x), maxLeft),
    top: Math.min(Math.max(margin, y), maxTop),
  };
}

function moveMenuFocus(event: ReactKeyboardEvent<HTMLElement>) {
  if (event.key !== "ArrowDown" && event.key !== "ArrowUp" && event.key !== "Home" && event.key !== "End") {
    return;
  }
  const items = event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)');
  if (items.length === 0) {
    return;
  }
  event.preventDefault();
  const list = [...items];
  const index = list.findIndex((item) => item === document.activeElement);
  if (event.key === "Home") {
    list[0]?.focus();
    return;
  }
  if (event.key === "End") {
    list[list.length - 1]?.focus();
    return;
  }
  const delta = event.key === "ArrowDown" ? 1 : -1;
  const next = index < 0 ? 0 : (index + delta + list.length) % list.length;
  list[next]?.focus();
}
