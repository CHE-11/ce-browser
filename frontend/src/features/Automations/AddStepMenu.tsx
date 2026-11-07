import type { ReactNode } from "react";
import {
  ArrowDownUp,
  Bell,
  CheckCheck,
  Globe,
  Hourglass,
  Keyboard,
  ListChecks,
  MousePointer,
  MousePointerClick,
  Sparkles,
  Table,
  TextCursorInput,
  Timer,
  Undo2,
  Upload,
} from "lucide-react";
import type { FlowStepKind } from "@type/browser";
import { ContextMenu, ContextMenuItem } from "@/components/ContextMenu";
import { STEP_KINDS } from "./flow-model";

const STEP_ICONS: Record<FlowStepKind, ReactNode> = {
  navigate: <Globe />,
  click: <MousePointerClick />,
  type: <TextCursorInput />,
  press: <Keyboard />,
  select: <ListChecks />,
  scroll: <ArrowDownUp />,
  hover: <MousePointer />,
  wait: <Timer />,
  waitFor: <Hourglass />,
  back: <Undo2 />,
  extract: <Table />,
  ai: <Sparkles />,
  assert: <CheckCheck />,
  notify: <Bell />,
  upload: <Upload />,
};

type AddStepMenuProps = {
  id: string;
  x: number;
  y: number;
  onPick: (kind: FlowStepKind) => void;
  onClose: () => void;
};

export function AddStepMenu({ id, x, y, onPick, onClose }: AddStepMenuProps) {
  return (
    <ContextMenu id={id} label="Add a step" x={x} y={y} onClose={onClose} scrollParentId="flow-editor">
      {STEP_KINDS.map((item) => (
        <ContextMenuItem
          id={`${id}-${item.kind}`}
          key={item.kind}
          icon={STEP_ICONS[item.kind]}
          label={item.label}
          title={item.hint}
          onClick={() => {
            onClose();
            onPick(item.kind);
          }}
        />
      ))}
    </ContextMenu>
  );
}
