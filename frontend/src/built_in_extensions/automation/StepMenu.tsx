import { Copy, EyeOff, Eye, Play, PlayCircle, Trash2 } from "lucide-react";
import { ContextMenu, ContextMenuItem, ContextMenuSeparator } from "@/components/ContextMenu";

export type StepAction = "runOnly" | "runFrom" | "duplicate" | "toggle" | "remove";

type StepMenuProps = {
  id: string;
  disabled: boolean;
  /** A run of this flow is going, so it can't start another. */
  busy: boolean;
  x: number;
  y: number;
  onAction: (action: StepAction) => void;
  onClose: () => void;
};

export function StepMenu({ id, disabled, busy, x, y, onAction, onClose }: StepMenuProps) {
  const act = (action: StepAction) => {
    onClose();
    onAction(action);
  };
  return (
    <ContextMenu id={id} label="Step actions" x={x} y={y} onClose={onClose} scrollParentId="flow-editor">
      <ContextMenuItem id={`${id}-run-only`} icon={<Play />} label="Run this step" disabled={busy || disabled} onClick={() => act("runOnly")} />
      <ContextMenuItem id={`${id}-run-from`} icon={<PlayCircle />} label="Run from here" disabled={busy} onClick={() => act("runFrom")} />
      <ContextMenuSeparator id={`${id}-separator-run`} />
      <ContextMenuItem id={`${id}-duplicate`} icon={<Copy />} label="Duplicate" onClick={() => act("duplicate")} />
      <ContextMenuItem id={`${id}-toggle`} icon={disabled ? <Eye /> : <EyeOff />} label={disabled ? "Turn on" : "Turn off"} onClick={() => act("toggle")} />
      <ContextMenuSeparator id={`${id}-separator-remove`} />
      <ContextMenuItem id={`${id}-remove`} icon={<Trash2 />} label="Delete" danger onClick={() => act("remove")} />
    </ContextMenu>
  );
}
