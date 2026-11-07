import type { ReactNode } from "react";
import { Button } from "./Button";
import { Dialog } from "./Dialog";

type ConfirmAction = {
  label: string;
  variant?: "primary" | "secondary" | "destructive";
  onClick: () => void;
};

type ConfirmDialogProps = {
  id: string;
  title: string;
  open: boolean;
  children: ReactNode;
  /** The main action, focused first. Enter also runs it. */
  confirm: ConfirmAction;
  /** A second choice shown before the main action, such as Discard next to Save. */
  alternative?: ConfirmAction;
  cancelLabel?: string;
  onCancel: () => void;
};

/** A small question with a main action, an optional second action, and Cancel. */
export function ConfirmDialog({ id, title, open, children, confirm, alternative, cancelLabel = "Cancel", onCancel }: ConfirmDialogProps) {
  return (
    <Dialog id={id} title={title} open={open} onClose={onCancel} onEnter={confirm.onClick} panelClassName="max-w-md">
      <div id={`${id}-message`} className="text-sm leading-relaxed text-zinc-300">
        {children}
      </div>
      <div id={`${id}-actions`} className="mt-4 flex justify-end gap-2">
        <Button id={`${id}-cancel`} size="sm" variant="ghost" onClick={onCancel}>
          {cancelLabel}
        </Button>
        {alternative ? (
          <Button id={`${id}-alternative`} size="sm" variant={alternative.variant ?? "secondary"} onClick={alternative.onClick}>
            {alternative.label}
          </Button>
        ) : null}
        <Button id={`${id}-confirm`} size="sm" variant={confirm.variant ?? "primary"} onClick={confirm.onClick} autoFocus>
          {confirm.label}
        </Button>
      </div>
    </Dialog>
  );
}
