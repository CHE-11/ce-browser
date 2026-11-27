import { useState } from "react";
import { Check, Pencil, Pin, PinOff, Trash2, X } from "lucide-react";
import type { AgentMemoryCategory, AgentMemoryItem } from "@type/browser";
import { IconButton } from "@/components/IconButton";
import { cn } from "@/lib/cn";
import { inputClassName } from "./agent-styles";

const CATEGORIES: { value: AgentMemoryCategory; label: string }[] = [
  { value: "", label: "General" },
  { value: "preference", label: "Preference" },
  { value: "procedure", label: "Procedure" },
  { value: "fact", label: "Fact" },
];

type Props = {
  item: AgentMemoryItem;
  onSave: (item: AgentMemoryItem) => Promise<boolean>;
  onPin: () => void;
  onDelete: () => void;
};

function categoryLabel(category: string) {
  return CATEGORIES.find((item) => item.value === category)?.label ?? "General";
}

/** One note, with pin, edit and forget. */
export function AgentMemoryRow({ item, onSave, onPin, onDelete }: Props) {
  const [editing, setEditing] = useState(false);
  const [text, setText] = useState(item.text);
  const [site, setSite] = useState(item.site);
  const [category, setCategory] = useState<AgentMemoryCategory>(item.category ?? "");
  const id = `agent-memory-item-${item.id}`;
  const expired = item.expires > 0 && item.expires <= Date.now();

  if (editing) {
    return (
      <li id={id} className="flex flex-col gap-1.5 rounded-md bg-zinc-900 p-2 ring-1 ring-sky-800">
        <textarea id={`${id}-text`} className={cn(inputClassName, "min-h-14 resize-none text-xs")} value={text} autoFocus onChange={(event) => setText(event.target.value)} />
        <div id={`${id}-edit-row`} className="flex flex-wrap gap-1.5">
          <input id={`${id}-site`} className={cn(inputClassName, "h-7 min-w-[8rem] flex-1 py-1 text-xs")} placeholder="Site, or blank for everywhere" value={site} onChange={(event) => setSite(event.target.value)} />
          <select
            id={`${id}-category`}
            className={cn(inputClassName, "h-7 w-28 py-1 text-xs")}
            value={category}
            onChange={(event) => setCategory(event.target.value as AgentMemoryCategory)}
          >
            {CATEGORIES.map((choice) => (
              <option id={`${id}-category-${choice.value || "general"}`} key={choice.value || "general"} value={choice.value}>
                {choice.label}
              </option>
            ))}
          </select>
          <IconButton id={`${id}-cancel`} label="Cancel" className="size-7" onClick={() => setEditing(false)}>
            <X id={`${id}-cancel-icon`} aria-hidden />
          </IconButton>
          <IconButton
            id={`${id}-save`}
            label="Save"
            className="size-7"
            disabled={!text.trim()}
            onClick={() => void onSave({ ...item, text, site, category }).then((saved) => saved && setEditing(false))}
          >
            <Check id={`${id}-save-icon`} aria-hidden />
          </IconButton>
        </div>
      </li>
    );
  }

  return (
    <li id={id} className={cn("group flex items-start gap-1.5 rounded-md bg-zinc-900/70 p-2 ring-1 ring-zinc-800", expired && "opacity-60")}>
      <div id={`${id}-body`} className="min-w-0 flex-1">
        <p id={`${id}-text`} className="whitespace-pre-wrap break-words text-xs text-zinc-100">
          {item.text}
        </p>
        <p id={`${id}-meta`} className="mt-0.5 text-[10px] text-zinc-500">
          {[
            categoryLabel(item.category ?? ""),
            item.site || "Everywhere",
            item.pinned ? "Pinned" : null,
            item.sourceUrl ? "Has source" : null,
            expired ? "Expired" : item.expires > 0 ? "Expires" : null,
            new Date(item.created).toLocaleDateString(),
          ]
            .filter(Boolean)
            .join(" · ")}
        </p>
        {item.sourceUrl ? (
          <p id={`${id}-source`} className="mt-0.5 truncate text-[10px] text-zinc-600" title={item.sourceUrl}>
            From {item.sourceUrl}
          </p>
        ) : null}
      </div>
      <div id={`${id}-actions`} className="flex shrink-0 opacity-60 group-hover:opacity-100">
        <IconButton id={`${id}-pin`} label={item.pinned ? "Unpin" : "Pin: show in every run"} className="size-6" active={item.pinned} onClick={onPin}>
          {item.pinned ? <PinOff id={`${id}-pin-icon`} aria-hidden /> : <Pin id={`${id}-pin-icon`} aria-hidden />}
        </IconButton>
        <IconButton
          id={`${id}-edit`}
          label="Edit"
          className="size-6"
          onClick={() => {
            setText(item.text);
            setSite(item.site);
            setCategory(item.category ?? "");
            setEditing(true);
          }}
        >
          <Pencil id={`${id}-edit-icon`} aria-hidden />
        </IconButton>
        <IconButton id={`${id}-delete`} label="Forget" className="size-6" onClick={onDelete}>
          <Trash2 id={`${id}-delete-icon`} aria-hidden />
        </IconButton>
      </div>
    </li>
  );
}
