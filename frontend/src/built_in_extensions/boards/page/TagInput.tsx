import { useMemo, useState } from "react";
import { X } from "lucide-react";
import { cn } from "@/lib/cn";

type TagInputProps = {
  id: string;
  tags: string[];
  /** Every tag in the library, for suggestions. */
  known: string[];
  onAdd: (tags: string[]) => void;
  onRemove: (tag: string) => void;
  /** Clicking a chip shows every pin with that tag. */
  onOpenTag?: (tag: string) => void;
  placeholder?: string;
};

const SUGGESTIONS = 8;

/** Tag chips with an input that adds on Enter or comma and suggests existing tags. */
export function TagInput({ id, tags, known, onAdd, onRemove, onOpenTag, placeholder = "Add tags" }: TagInputProps) {
  const [value, setValue] = useState("");
  const [focused, setFocused] = useState(false);
  const [active, setActive] = useState(0);
  const own = useMemo(() => new Set(tags.map((tag) => tag.toLowerCase())), [tags]);
  const needle = value.trim().toLowerCase();
  const suggestions = useMemo(
    () =>
      known
        .filter((tag) => !own.has(tag.toLowerCase()) && (!needle || tag.toLowerCase().includes(needle)))
        .sort((left, right) => Number(!left.toLowerCase().startsWith(needle)) - Number(!right.toLowerCase().startsWith(needle)))
        .slice(0, SUGGESTIONS),
    [known, own, needle],
  );

  const commit = (raw: string) => {
    const next = raw
      .split(",")
      .map((tag) => tag.trim())
      .filter((tag) => tag && !own.has(tag.toLowerCase()));
    if (next.length > 0) {
      onAdd(next);
    }
    setValue("");
    setActive(0);
  };

  return (
    <div id={id} className="relative">
      <div
        id={`${id}-box`}
        className={cn("flex min-h-8 flex-wrap items-center gap-1 rounded-md border border-zinc-700 bg-zinc-950 p-1", focused && "border-sky-500")}
      >
        {tags.map((tag) => (
          <span id={`${id}-tag-${tag}`} key={tag} className="flex max-w-full items-center gap-0.5 rounded bg-zinc-800 py-0.5 pl-1.5 pr-0.5 text-xs text-zinc-200">
            <button
              id={`${id}-tag-${tag}-open`}
              type="button"
              className="truncate hover:text-sky-300"
              title={onOpenTag ? `Show everything tagged ${tag}` : tag}
              onClick={() => onOpenTag?.(tag)}
            >
              {tag}
            </button>
            <button
              id={`${id}-tag-${tag}-remove`}
              type="button"
              aria-label={`Remove ${tag}`}
              className="inline-flex size-4 shrink-0 items-center justify-center rounded text-zinc-500 hover:bg-zinc-700 hover:text-zinc-100"
              onClick={() => onRemove(tag)}
            >
              <X id={`${id}-tag-${tag}-remove-icon`} className="size-3" aria-hidden />
            </button>
          </span>
        ))}
        <input
          id={`${id}-input`}
          value={value}
          placeholder={tags.length === 0 ? placeholder : ""}
          className="h-6 min-w-16 flex-1 bg-transparent px-1 text-xs text-zinc-100 outline-none placeholder:text-zinc-500"
          onFocus={() => setFocused(true)}
          onBlur={() => {
            setFocused(false);
            if (value.trim()) {
              commit(value);
            }
          }}
          onChange={(event) => {
            const next = event.target.value;
            if (next.includes(",")) {
              commit(next);
            } else {
              setValue(next);
              setActive(0);
            }
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              commit(suggestions[active] && needle ? suggestions[active] : value);
            } else if (event.key === "Tab" && needle && suggestions[active]) {
              event.preventDefault();
              commit(suggestions[active]);
            } else if (event.key === "ArrowDown" && suggestions.length > 0) {
              event.preventDefault();
              setActive((current) => (current + 1) % suggestions.length);
            } else if (event.key === "ArrowUp" && suggestions.length > 0) {
              event.preventDefault();
              setActive((current) => (current - 1 + suggestions.length) % suggestions.length);
            } else if (event.key === "Backspace" && !value && tags.length > 0) {
              onRemove(tags[tags.length - 1]);
            } else if (event.key === "Escape") {
              setValue("");
              event.currentTarget.blur();
            }
          }}
        />
      </div>
      {focused && needle && suggestions.length > 0 ? (
        <ul id={`${id}-suggestions`} role="listbox" className="absolute inset-x-0 top-full z-20 mt-1 rounded-md border border-zinc-700 bg-zinc-900 py-0.5 shadow-xl">
          {suggestions.map((tag, index) => (
            <li id={`${id}-suggestion-${index}`} key={tag} role="option" aria-selected={index === active}>
              <button
                id={`${id}-suggestion-${index}-button`}
                type="button"
                className={cn("flex h-6 w-full items-center px-2 text-left text-xs text-zinc-200 hover:bg-zinc-800", index === active && "bg-zinc-800")}
                onMouseDown={(event) => {
                  event.preventDefault();
                  commit(tag);
                }}
              >
                {tag}
              </button>
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}
