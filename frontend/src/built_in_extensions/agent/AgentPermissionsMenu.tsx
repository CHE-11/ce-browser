import { useCallback, useRef, useState } from "react";
import { ChevronDown, SlidersHorizontal } from "lucide-react";
import type { AgentAccess } from "@type/browser";
import type { AccessToggle } from "@/lib/agent-access";
import { useDismiss } from "@/lib/use-dismiss";
import { AgentAccessOptions } from "./AgentAccessMenu";

type Props = {
  id: string;
  access: AgentAccess;
  onAccess: (access: AgentAccess) => void;
  scope: "run" | "next";
  currentSite: string;
  blockedBySettings: Partial<Record<AccessToggle, string>>;
};

/** What the agent may reach. The model and token limits are the labels under the composer. */
export function AgentPermissionsMenu({ id, access, onAccess, scope, currentSite, blockedBySettings }: Props) {
  const root = useRef<HTMLDivElement | null>(null);
  const [open, setOpen] = useState(false);
  const close = useCallback(() => setOpen(false), []);
  useDismiss(root, open, close);

  return (
    <div id={id} ref={root} className="relative min-w-[150px]">
      <button
        id={`${id}-button`}
        type="button"
        aria-haspopup="dialog"
        aria-expanded={open}
        title="Permissions"
        className="flex h-7 w-full min-w-0 items-center gap-1 rounded-md bg-zinc-900 px-1.5 text-[11px] text-zinc-200 ring-1 ring-zinc-800 hover:bg-zinc-800"
        onClick={() => setOpen((value) => !value)}
      >
        <SlidersHorizontal id={`${id}-button-icon`} className="size-3 shrink-0 text-zinc-400" aria-hidden />
        <span id={`${id}-button-label`} className="min-w-0 flex-1 truncate text-left">
          Permissions
        </span>
        <ChevronDown id={`${id}-button-chevron`} className="size-3 shrink-0 opacity-60" aria-hidden />
      </button>
      {open ? (
        <div
          id={`${id}-popover`}
          role="dialog"
          aria-label="Permissions"
          data-agent-popover
          className="absolute left-0 top-full z-40 mt-1 flex max-h-[min(50rem,90vh)] w-[min(20rem,calc(100vw-1.5rem))] flex-col gap-2 overflow-y-auto rounded-lg bg-zinc-900 p-2 shadow-2xl ring-1 ring-zinc-700"
        >
          <div id={`${id}-title`} className="px-1 text-xs font-semibold text-zinc-100">
            Permissions
          </div>
          <p id={`${id}-note`} className="px-1 text-[10px] leading-4 text-zinc-500">
            Model, thinking, and token limits are the labels under the composer.
          </p>
          <div id={`${id}-access`} className="border-t border-zinc-800 pt-2">
            <AgentAccessOptions id={`${id}-access`} access={access} onChange={onAccess} scope={scope} currentSite={currentSite} blockedBySettings={blockedBySettings} />
          </div>
        </div>
      ) : null}
    </div>
  );
}
