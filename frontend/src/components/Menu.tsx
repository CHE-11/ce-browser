import { useRef, type CSSProperties, type KeyboardEvent, type MouseEvent, type ReactNode } from "react";
import { menuIconClass, menuRowClass } from "../lib/chrome-metrics";
import { cn } from "../lib/cn";
import { focusRow } from "../lib/focus-ring";

const ITEM_SELECTOR = '[role="menuitem"]:not(:disabled), [role="menuitemcheckbox"]:not(:disabled)';
const TYPEAHEAD_RESET_MS = 700;

type Typeahead = { text: string; at: number };

/** Arrow keys, Home, End and typing the start of a label move focus between menu items. */
export function moveMenuFocus(event: KeyboardEvent<HTMLElement>, typeahead?: Typeahead) {
  const items = [...event.currentTarget.querySelectorAll<HTMLElement>(ITEM_SELECTOR)];
  if (items.length === 0) {
    return;
  }
  const index = items.findIndex((item) => item === document.activeElement);
  const focus = (next: number) => {
    event.preventDefault();
    items[next]?.focus();
  };
  switch (event.key) {
    case "ArrowDown":
      focus(index < 0 ? 0 : (index + 1) % items.length);
      return;
    case "ArrowUp":
      focus(index < 0 ? items.length - 1 : (index - 1 + items.length) % items.length);
      return;
    case "Home":
      focus(0);
      return;
    case "End":
      focus(items.length - 1);
      return;
  }
  if (!typeahead || event.key.length !== 1 || event.ctrlKey || event.altKey || event.metaKey || event.key === " ") {
    return;
  }
  const now = Date.now();
  typeahead.text = now - typeahead.at > TYPEAHEAD_RESET_MS ? event.key.toLowerCase() : typeahead.text + event.key.toLowerCase();
  typeahead.at = now;
  const label = (item: HTMLElement) => (item.dataset.label ?? item.textContent ?? "").trim().toLowerCase();
  const ordered = [...items.slice(index + 1), ...items.slice(0, index + 1)];
  const match =
    ordered.find((item) => label(item).startsWith(typeahead.text)) ??
    (typeahead.text.length > 1 ? undefined : ordered.find((item) => label(item).startsWith(event.key.toLowerCase())));
  if (match) {
    event.preventDefault();
    match.focus();
  }
}

type MenuProps = {
  id: string;
  label: string;
  children: ReactNode;
  className?: string;
  onKeyDown?: (event: KeyboardEvent<HTMLDivElement>) => void;
};

export function Menu({ id, label, children, className, onKeyDown }: MenuProps) {
  const typeahead = useRef<Typeahead>({ text: "", at: 0 });
  return (
    <div
      id={id}
      role="menu"
      aria-label={label}
      className={cn("flex flex-col gap-0.5 px-1.5 py-1.5", className)}
      onKeyDown={(event) => {
        onKeyDown?.(event);
        if (!event.defaultPrevented) {
          moveMenuFocus(event, typeahead.current);
        }
      }}
    >
      {children}
    </div>
  );
}

export type MenuItemProps = {
  id: string;
  icon?: ReactNode;
  label: string;
  shortcut?: string;
  disabled?: boolean;
  danger?: boolean;
  title?: string;
  style?: CSSProperties;
  onClick: () => void;
  onContextMenu?: (event: MouseEvent<HTMLButtonElement>) => void;
};

export function MenuItem({ id, icon, label, shortcut, disabled, danger, title, style, onClick, onContextMenu }: MenuItemProps) {
  const sized = style !== undefined;
  return (
    <button
      id={id}
      type="button"
      role="menuitem"
      data-label={label}
      title={title}
      disabled={disabled}
      style={style}
      className={cn(
        "group flex w-full items-center gap-3 rounded-lg text-left text-zinc-100 transition-colors hover:bg-zinc-800 disabled:opacity-40 disabled:hover:bg-transparent",
        sized ? undefined : menuRowClass,
        focusRow,
        danger && "text-red-300",
      )}
      onClick={onClick}
      onContextMenu={onContextMenu}
    >
      <span
        id={`${id}-icon`}
        aria-hidden
        className={cn(
          "inline-flex shrink-0 items-center justify-center text-zinc-400 group-hover:text-zinc-200",
          sized ? "size-[var(--item-icon)] [&_svg]:size-[var(--item-icon)]" : menuIconClass,
        )}
      >
        {icon}
      </span>
      <span id={`${id}-label`} className="min-w-0 flex-1 truncate">
        {label}
      </span>
      {shortcut ? (
        <kbd id={`${id}-shortcut`} className="shrink-0 font-sans text-[11px] text-zinc-500">
          {shortcut}
        </kbd>
      ) : null}
    </button>
  );
}

export function MenuSeparator({ id, className }: { id: string; className?: string }) {
  return <div id={id} role="separator" className={cn("mx-2 my-1 h-px bg-zinc-800", className)} />;
}
