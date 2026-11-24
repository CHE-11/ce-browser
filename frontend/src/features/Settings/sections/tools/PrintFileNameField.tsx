import { useEffect, useLayoutEffect, useRef, useState } from "react";
import {
  FILE_NAME_GROUPS,
  FILE_NAME_TEMPLATE_MAX,
  FILE_NAME_TOKENS,
  previewFileName,
  suggestedFileNameTemplate,
} from "@/built_in_extensions/print-to-pdf/file-name";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import { inputClass } from "../../SettingsControls";
import { PrintFileNameTry } from "./PrintFileNameTry";

const FILE_NAME_HINT =
  "Suggested when you save with this preset. Click a placeholder to insert it. A published date or author the page does not name is skipped, and the dash beside it is dropped. Leave this blank to use the page title.";

/** Settings echo the template after a save, and that round trip puts the caret at the end. The draft is what is on screen. */
function cleanTemplate(raw: string) {
  return raw.replace(/[\u0000-\u001F\u007F-\u009F]/g, "").slice(0, FILE_NAME_TEMPLATE_MAX);
}

function spliceToken(current: string, token: string, start: number, end: number) {
  const piece = `{${token}}`;
  const before = current.slice(0, start);
  const after = current.slice(end);
  if (before.length + piece.length > FILE_NAME_TEMPLATE_MAX) {
    return null;
  }
  const next = `${before}${piece}${after}`.slice(0, FILE_NAME_TEMPLATE_MAX);
  return { next, cursor: before.length + piece.length };
}

export function PrintFileNameField({
  id,
  presetId,
  presetName,
  value,
  onChange,
}: {
  id: string;
  presetId: string;
  presetName: string;
  value: string;
  onChange: (template: string) => void;
}) {
  const inputRef = useRef<HTMLInputElement>(null);
  const cursor = useRef<number | null>(null);
  const committed = useRef(value);
  const [draft, setDraft] = useState<string | null>(null);
  const shown = draft ?? value;
  const suggested = suggestedFileNameTemplate(presetId);
  const preview = previewFileName(shown, presetName);

  useEffect(() => {
    if (value !== committed.current) {
      committed.current = value;
      setDraft(null);
    }
  }, [value]);

  useLayoutEffect(() => {
    if (cursor.current === null) {
      return;
    }
    const input = inputRef.current;
    const place = cursor.current;
    cursor.current = null;
    input?.focus();
    input?.setSelectionRange(place, place);
  }, [shown]);

  const publish = (next: string) => {
    const visible = cleanTemplate(next);
    setDraft(visible);
    const stored = visible.trim();
    if (stored !== value) {
      committed.current = stored;
      onChange(stored);
    }
  };

  const insert = (token: string) => {
    const input = inputRef.current;
    const start = input?.selectionStart ?? shown.length;
    const end = input?.selectionEnd ?? shown.length;
    const result = spliceToken(shown, token, start, end);
    if (!result) {
      return;
    }
    cursor.current = result.cursor;
    publish(result.next);
  };

  return (
    <div id={id} className="flex flex-col gap-2">
      <div id={`${id}-caption-row`} className="flex items-center justify-between gap-3">
        <span id={`${id}-legend`} className="text-[11px] font-semibold uppercase tracking-wider text-zinc-500">
          File name
        </span>
        {shown.trim() !== suggested ? (
          <button
            id={`${id}-suggested`}
            type="button"
            className={cn("text-xs text-sky-400 hover:text-sky-300", focusRing)}
            onClick={() => {
              committed.current = suggested;
              setDraft(null);
              if (suggested !== value) {
                onChange(suggested);
              }
            }}
          >
            Use suggested
          </button>
        ) : null}
      </div>
      <input
        id={`${id}-input`}
        ref={inputRef}
        className={cn(inputClass, "font-mono text-[13px]")}
        maxLength={FILE_NAME_TEMPLATE_MAX}
        spellCheck={false}
        placeholder="{title}"
        value={shown}
        onChange={(event) => publish(event.target.value)}
        onBlur={() => {
          if (draft !== null && draft.trim() === value) {
            setDraft(null);
          }
        }}
      />
      <p id={`${id}-hint`} className="text-xs text-zinc-500">
        {FILE_NAME_HINT}
      </p>
      <p id={`${id}-preview`} className="text-xs text-zinc-400">
        <span id={`${id}-preview-label`}>Example: </span>
        <span id={`${id}-preview-value`} className="font-mono text-zinc-300">
          {preview}
        </span>
      </p>
      <PrintFileNameTry id={`${id}-try`} template={shown} presetName={presetName} />
      <div id={`${id}-tokens`} className="flex flex-col gap-1.5">
        {FILE_NAME_GROUPS.map((group) => (
          <div id={`${id}-tokens-${group.id}`} key={group.id} className="flex flex-wrap items-center gap-1">
            <span id={`${id}-tokens-${group.id}-label`} className="w-14 shrink-0 text-[11px] text-zinc-500">
              {group.label}
            </span>
            {FILE_NAME_TOKENS.filter((token) => token.group === group.id).map((token) => (
              <button
                id={`${id}-token-${token.token}`}
                key={token.token}
                type="button"
                title={token.hint}
                aria-label={`Insert {${token.token}}`}
                className={cn(
                  "rounded-md bg-zinc-800 px-1.5 py-0.5 font-mono text-[11px] text-zinc-300 hover:bg-zinc-700 hover:text-zinc-100",
                  focusRing,
                )}
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => insert(token.token)}
              >
                {`{${token.token}}`}
              </button>
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}
