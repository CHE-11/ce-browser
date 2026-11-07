import { useState } from "react";
import { Brain, Pin, Plus, X } from "lucide-react";
import { Button } from "@/components/Button";
import { EmptyState } from "@/components/EmptyState";
import { IconButton } from "@/components/IconButton";
import { cn } from "@/lib/cn";
import { useAgentMemory } from "@/lib/use-agent-memory";
import { AgentError } from "./AgentError";
import { AgentMemoryRow } from "./AgentMemoryRow";
import { inputClassName } from "./agent-styles";

/** Notes the agent keeps across runs: add, edit, pin and forget them. */
export function AgentMemoryView({ enabled, onClose }: { enabled: boolean; onClose: () => void }) {
  const memory = useAgentMemory();
  const [query, setQuery] = useState("");
  const [text, setText] = useState("");
  const [site, setSite] = useState("");
  const [pinned, setPinned] = useState(false);
  const needle = query.trim().toLowerCase();
  const items = memory.items.filter((item) => !needle || item.text.toLowerCase().includes(needle) || item.site.includes(needle));

  const add = () => {
    if (!text.trim()) {
      return;
    }
    void memory.add(text.trim(), site.trim(), pinned).then((saved) => {
      if (saved) {
        setText("");
        setSite("");
        setPinned(false);
      }
    });
  };

  return (
    <section id="agent-memory" aria-label="Memory" className="flex min-h-0 flex-1 flex-col">
      <div id="agent-memory-toolbar" className="flex shrink-0 items-center gap-2 px-3 py-2">
        <Brain id="agent-memory-icon" className="size-4 text-sky-400" aria-hidden />
        <h2 id="agent-memory-title" className="text-xs font-semibold">
          Memory
        </h2>
        <span id="agent-memory-count" className="text-[11px] text-zinc-500">
          {memory.items.length}
        </span>
        <div id="agent-memory-spacer" className="flex-1" />
        <IconButton id="agent-memory-close" label="Back to the task" className="size-7" onClick={onClose}>
          <X id="agent-memory-close-icon" aria-hidden />
        </IconButton>
      </div>
      {!enabled ? (
        <p id="agent-memory-off" className="mx-3 mb-2 rounded-md bg-amber-950/40 px-2 py-1.5 text-[11px] text-amber-200 ring-1 ring-amber-900/60">
          Memory is off in Settings, so runs don't read or add notes.
        </p>
      ) : null}
      <div id="agent-memory-add" className="mx-3 mb-2 flex flex-col gap-1.5 rounded-md bg-zinc-900/70 p-2 ring-1 ring-zinc-800">
        <textarea
          id="agent-memory-add-text"
          className={cn(inputClassName, "min-h-12 resize-none text-xs")}
          placeholder="Something the agent should know, like “I prefer window seats”"
          value={text}
          onChange={(event) => setText(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              add();
            }
          }}
        />
        <div id="agent-memory-add-row" className="flex items-center gap-1.5">
          <input id="agent-memory-add-site" className={cn(inputClassName, "h-7 py-1 text-xs")} placeholder="Site (optional)" value={site} onChange={(event) => setSite(event.target.value)} />
          <IconButton id="agent-memory-add-pin" label={pinned ? "Pinned: shown in every run" : "Pin: show in every run"} className="size-7" active={pinned} onClick={() => setPinned((value) => !value)}>
            <Pin id="agent-memory-add-pin-icon" aria-hidden />
          </IconButton>
          <Button id="agent-memory-add-save" size="sm" variant="primary" disabled={!text.trim()} onClick={add}>
            <Plus id="agent-memory-add-save-icon" className="size-3.5" aria-hidden />
            Add
          </Button>
        </div>
      </div>
      {memory.error ? (
        <div id="agent-memory-error-wrap" className="mx-3 mb-2">
          <AgentError id="agent-memory-error" text={memory.error} alert />
        </div>
      ) : null}
      {memory.items.length > 6 ? (
        <div id="agent-memory-search" className="mx-3 mb-2">
          <input id="agent-memory-search-input" className={cn(inputClassName, "h-7 py-1 text-xs")} placeholder="Search notes" value={query} onChange={(event) => setQuery(event.target.value)} />
        </div>
      ) : null}
      <div id="agent-memory-scroll" className="min-h-0 flex-1 overflow-y-auto px-3 pb-3">
        {memory.loaded && memory.items.length === 0 ? (
          <EmptyState id="agent-memory-empty" icon={Brain} title="Nothing remembered yet" hint="The agent keeps notes here when it learns something lasting, and reads them in later runs." />
        ) : (
          <ul id="agent-memory-list" className="flex flex-col gap-1.5">
            {items.map((item) => (
              <AgentMemoryRow key={item.id} item={item} onSave={memory.update} onPin={() => void memory.togglePin(item)} onDelete={() => void memory.remove(item.id)} />
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}
