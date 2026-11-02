import { ChevronDown } from "lucide-react";
import type { Workspace } from "../../../../../types/browser";
import { openPopupBelow } from "../../../lib/browser-client";
import { cn } from "../../../lib/cn";
import { focusRing } from "../../../lib/focus-ring";
import { groupColorClasses } from "../../../lib/tab-rows";

type WorkspaceSwitcherProps = {
  workspaces: Workspace[];
  current: string;
  /** Collapsed vertical sidebar: show only the colored dot. */
  compact?: boolean;
  className?: string;
};

export function WorkspaceSwitcher({ workspaces, current, compact = false, className }: WorkspaceSwitcherProps) {
  const workspace = workspaces.find((item) => item.id === current) ?? workspaces[0];
  if (!workspace) {
    return null;
  }
  const colors = groupColorClasses(workspace.color);
  return (
    <button
      id="workspace-switcher"
      type="button"
      title={`Workspace: ${workspace.name}\nCtrl+Alt+1 to 9 switch workspaces`}
      aria-label={`Workspace ${workspace.name}`}
      aria-haspopup="menu"
      className={cn(
        "inline-flex h-7 shrink-0 items-center gap-1.5 rounded-md text-xs text-zinc-300 transition-colors hover:bg-zinc-800 hover:text-zinc-100",
        focusRing,
        compact ? "w-9 justify-center" : "max-w-36 px-2",
        className,
      )}
      onClick={(event) =>
        void openPopupBelow(event.currentTarget, { kind: "workspaces", width: 300, height: 360 }, "start")
      }
    >
      <span id="workspace-switcher-dot" className={cn("size-2.5 shrink-0 rounded-full", colors.swatch)} />
      {compact ? null : (
        <>
          <span id="workspace-switcher-name" className="min-w-0 truncate">
            {workspace.name}
          </span>
          <ChevronDown id="workspace-switcher-chevron" className="size-3 shrink-0 text-zinc-500" aria-hidden />
        </>
      )}
    </button>
  );
}
