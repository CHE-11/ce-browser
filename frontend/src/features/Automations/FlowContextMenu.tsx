import { Copy, Download, Pause, Play, Square, Trash2 } from "lucide-react";
import type { Flow } from "@type/browser";
import { ContextMenu, ContextMenuItem, ContextMenuSeparator } from "@/components/ContextMenu";

export type FlowAction = "run" | "stop" | "duplicate" | "export" | "pause" | "resume" | "delete";

type FlowContextMenuProps = {
  flow: Flow;
  running: boolean;
  x: number;
  y: number;
  onAction: (action: FlowAction) => void;
  onClose: () => void;
  /** List that should dismiss the menu when it scrolls. */
  scrollParentId?: string;
};

export function FlowContextMenu({ flow, running, x, y, onAction, onClose, scrollParentId = "automations-flow-list" }: FlowContextMenuProps) {
  const act = (action: FlowAction) => {
    onClose();
    onAction(action);
  };
  return (
    <ContextMenu id="flow-context-menu" label={`Actions for ${flow.name || "this flow"}`} x={x} y={y} onClose={onClose} scrollParentId={scrollParentId}>
      {running ? (
        <ContextMenuItem id="flow-context-menu-stop" icon={<Square />} label="Stop" onClick={() => act("stop")} />
      ) : (
        <ContextMenuItem id="flow-context-menu-run" icon={<Play />} label="Run" onClick={() => act("run")} />
      )}
      <ContextMenuItem id="flow-context-menu-duplicate" icon={<Copy />} label="Duplicate" onClick={() => act("duplicate")} />
      <ContextMenuItem id="flow-context-menu-export" icon={<Download />} label="Export…" onClick={() => act("export")} />
      {flow.paused ? (
        <ContextMenuItem id="flow-context-menu-resume" icon={<Play />} label="Resume triggers" onClick={() => act("resume")} />
      ) : (
        <ContextMenuItem id="flow-context-menu-pause" icon={<Pause />} label="Pause triggers" onClick={() => act("pause")} />
      )}
      <ContextMenuSeparator id="flow-context-menu-separator" />
      <ContextMenuItem id="flow-context-menu-delete" icon={<Trash2 />} label="Delete…" danger onClick={() => act("delete")} />
    </ContextMenu>
  );
}
