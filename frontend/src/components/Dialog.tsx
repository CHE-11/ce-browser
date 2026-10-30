import { useEffect, useRef, type ReactNode } from "react";
import { cn } from "../lib/cn";

type DialogProps = {
  id: string;
  title: string;
  open: boolean;
  onClose: () => void;
  onEnter?: () => void;
  children: ReactNode;
  panelClassName?: string;
  bodyClassName?: string;
};

function isTypingTarget(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  const tag = target.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || target.isContentEditable;
}

export function Dialog({ id, title, open, onClose, onEnter, children, panelClassName, bodyClassName }: DialogProps) {
  const ignoreBackdropUntil = useRef(0);

  useEffect(() => {
    if (!open) {
      return;
    }
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
      }
    };
    const onEnterKey = (event: KeyboardEvent) => {
      if (!onEnter || event.key !== "Enter" || event.repeat || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey || event.isComposing) {
        return;
      }
      if (isTypingTarget(event.target)) {
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      onEnter();
    };
    const onFocus = () => {
      ignoreBackdropUntil.current = Date.now() + 500;
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("keydown", onEnterKey, true);
    window.addEventListener("focus", onFocus);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("keydown", onEnterKey, true);
      window.removeEventListener("focus", onFocus);
    };
  }, [open, onClose, onEnter]);

  if (!open) {
    return null;
  }

  return (
    <div
      id={`${id}-backdrop`}
      className="fixed inset-0 z-40 flex items-center justify-center bg-black/60 p-6"
      onClick={() => {
        if (Date.now() < ignoreBackdropUntil.current) {
          return;
        }
        onClose();
      }}
    >
      <div
        id={id}
        role="dialog"
        aria-modal="true"
        aria-labelledby={`${id}-title`}
        className={cn(
          "flex max-h-[80vh] w-full max-w-2xl flex-col overflow-hidden rounded-lg border border-zinc-700 bg-zinc-900 shadow-2xl",
          panelClassName,
        )}
        onClick={(event) => event.stopPropagation()}
      >
        <div id={`${id}-header`} className="border-b border-zinc-800 px-4 py-3">
          <h2 id={`${id}-title`} className="text-base font-semibold text-zinc-100">
            {title}
          </h2>
        </div>
        <div id={`${id}-body`} className={cn("min-h-0 flex-1 overflow-auto px-4 py-3", bodyClassName)}>
          {children}
        </div>
      </div>
    </div>
  );
}
