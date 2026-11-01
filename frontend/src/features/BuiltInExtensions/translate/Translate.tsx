import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent } from "react";
import { ChevronDown, Languages } from "lucide-react";
import { Button } from "@/components/Button";
import { PopupFrame } from "@/components/PopupFrame";
import { cn } from "@/lib/cn";
import { closePopup, restoreTranslation, translatePage } from "@/lib/browser-client";
import { useSettings } from "@/lib/use-browser";
import type { TabInfo } from "@type/browser";
import { PopoverHeader } from "../PopoverParts";
import { readPopoverTab } from "../shared";
import { ToolButton } from "../ToolButton";
import { filterTranslateLanguages, translateLanguageName } from "./languages";

const fieldClass = "h-8 w-full rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm outline-none focus:border-sky-500";

export function TranslateButton({ activeTab }: { activeTab: TabInfo | undefined }) {
  return (
    <ToolButton
      id="translate"
      label="Translate page"
      kind="translate"
      icon={<Languages id="address-translate-icon" aria-hidden />}
      activeTab={activeTab}
      restore
    />
  );
}

export function TranslatePopover({ params }: { params: URLSearchParams }) {
  const tab = readPopoverTab(params);
  const settings = useSettings();
  const [language, setLanguage] = useState(settings?.translateLanguage || "en");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    if (settings?.translateLanguage) {
      setLanguage(settings.translateLanguage);
    }
  }, [settings?.translateLanguage]);

  const run = (restore: boolean) => {
    setBusy(true);
    setError("");
    const job = restore ? restoreTranslation(tab.id) : translatePage(tab.id, language);
    void job
      .then(() => closePopup())
      .catch((reason: unknown) => setError(reason instanceof Error ? reason.message : "Couldn't translate this page."))
      .finally(() => setBusy(false));
  };

  return (
    <PopupFrame id="translate-popover" maxHeight={480}>
      <PopoverHeader id="translate-header" icon={<Languages aria-hidden />} title="Translate" subtitle={tab.title || tab.url} />
      <div id="translate-body" className="flex flex-col gap-2 px-3 pb-3">
        <LanguageCombobox id="translate-language" value={language} onChange={setLanguage} />
        {error ? (
          <p id="translate-error" className="text-xs text-amber-300">
            {error}
          </p>
        ) : null}
        <Button id="translate-run" variant="primary" disabled={busy} onClick={() => run(false)}>
          Translate page
        </Button>
        <Button id="translate-restore" disabled={busy} onClick={() => run(true)}>
          Show original
        </Button>
      </div>
    </PopupFrame>
  );
}

function LanguageCombobox({ id, value, onChange }: { id: string; value: string; onChange: (code: string) => void }) {
  const rootRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const matches = filterTranslateLanguages(query);

  const closeList = () => {
    setOpen(false);
    setQuery("");
  };

  const choose = (code: string) => {
    onChange(code);
    closeList();
  };

  const highlight = (items: ReturnType<typeof filterTranslateLanguages>) => {
    const index = items.findIndex((language) => language.code.toLowerCase() === value.toLowerCase());
    setActive(index < 0 ? 0 : index);
  };

  useLayoutEffect(() => {
    if (!open) {
      return;
    }
    const current = filterTranslateLanguages(query)[active];
    if (!current) {
      return;
    }
    document.getElementById(`${id}-option-${current.code}`)?.scrollIntoView({ block: "nearest" });
  }, [active, id, open, query]);

  useEffect(() => {
    if (!open) {
      return;
    }
    const onPointer = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) {
        closeList();
      }
    };
    window.addEventListener("pointerdown", onPointer);
    return () => window.removeEventListener("pointerdown", onPointer);
  }, [open]);

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      if (!open) {
        setQuery("");
        highlight(filterTranslateLanguages(""));
        setOpen(true);
        return;
      }
      const delta = event.key === "ArrowDown" ? 1 : -1;
      setActive((index) => (matches.length === 0 ? 0 : (index + delta + matches.length) % matches.length));
      return;
    }
    if (event.key === "Enter" && open) {
      event.preventDefault();
      const match = matches[active];
      if (match) {
        choose(match.code);
      }
      return;
    }
    if (event.key === "Escape" && open) {
      event.preventDefault();
      event.stopPropagation();
      closeList();
    }
  };

  return (
    <div id={id} ref={rootRef}>
      <label id={`${id}-label`} className="text-xs text-zinc-500">
        Translate to
        <span id={`${id}-field`} className="relative mt-1 block">
          <input
            id={`${id}-input`}
            role="combobox"
            aria-expanded={open}
            aria-controls={`${id}-list`}
            aria-autocomplete="list"
            aria-activedescendant={open && matches[active] ? `${id}-option-${matches[active].code}` : undefined}
            className={cn(fieldClass, "pr-8 text-zinc-100 placeholder:text-zinc-500")}
            placeholder="Search languages"
            autoComplete="off"
            spellCheck={false}
            value={open ? query : translateLanguageName(value)}
            onBlur={(event) => {
              if (!rootRef.current?.contains(event.relatedTarget)) {
                closeList();
              }
            }}
            onFocus={() => {
              setQuery("");
              highlight(filterTranslateLanguages(""));
              setOpen(true);
            }}
            onChange={(event) => {
              const next = event.target.value;
              setQuery(next);
              setActive(next.trim() ? 0 : highlightIndex(next, value));
              setOpen(true);
            }}
            onKeyDown={onKeyDown}
          />
          <ChevronDown
            id={`${id}-chevron`}
            aria-hidden
            className={cn("pointer-events-none absolute top-2 right-2 size-4 text-zinc-500", open && "rotate-180")}
          />
        </span>
      </label>
      {open ? (
        <div id={`${id}-list`} role="listbox" aria-label="Languages" className="mt-1 max-h-52 overflow-y-auto rounded-md border border-zinc-700 bg-zinc-950 py-1">
          {matches.length === 0 ? (
            <p id={`${id}-empty`} className="px-2 py-2 text-xs text-zinc-500">
              No languages match.
            </p>
          ) : (
            matches.map((language, index) => (
              <button
                id={`${id}-option-${language.code}`}
                key={language.code}
                type="button"
                role="option"
                aria-selected={index === active}
                className={cn(
                  "flex h-8 w-full items-center gap-2 px-2 text-left text-sm",
                  index === active ? "bg-zinc-800 text-zinc-100" : "text-zinc-300 hover:bg-zinc-800/70",
                )}
                onMouseDown={(event) => event.preventDefault()}
                onMouseEnter={() => setActive(index)}
                onClick={() => choose(language.code)}
              >
                <span id={`${id}-option-${language.code}-name`} className="min-w-0 flex-1 truncate">
                  {language.name}
                </span>
                <span id={`${id}-option-${language.code}-code`} className="shrink-0 text-[11px] text-zinc-500">
                  {language.code}
                </span>
              </button>
            ))
          )}
        </div>
      ) : null}
    </div>
  );
}

function highlightIndex(query: string, value: string) {
  const items = filterTranslateLanguages(query);
  const index = items.findIndex((language) => language.code.toLowerCase() === value.toLowerCase());
  return index < 0 ? 0 : index;
}
