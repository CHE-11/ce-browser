import { useEffect, useRef, useState } from "react";
import { RotateCcw } from "lucide-react";
import type { BookmarksBarAlign, BrowserSettings } from "@type/browser";
import { Button } from "@/components/Button";
import { PageShell } from "@/components/PageShell";
import { bookmarksBarAlign } from "@/lib/bookmarks-bar-align";
import { CHROME, type ChromeKey } from "@/lib/chrome-metrics";
import { saveSettings } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import { useRoute } from "@/lib/route";
import { useSettings } from "@/lib/use-browser";
import { CopyStyleDialog } from "./CopyStyleDialog";
import { StylePreview } from "./StylePreview";
import { STYLE_SURFACES, styleSurface, type StyleChoice, type StyleControl } from "./surfaces";

const SAVE_DELAY_MS = 150;

function formatMetric(value: number, unit: StyleControl["unit"]) {
  if (unit === "px") {
    return String(Math.round(value));
  }
  return String(Math.round(value * 10000) / 10000);
}

function clampMetric(value: number, control: StyleControl) {
  const next = Math.min(control.max, Math.max(control.min, value));
  if (control.unit === "px") {
    return Math.round(next);
  }
  return Math.round(next * 10000) / 10000;
}

export function StylesPage() {
  const settings = useSettings();
  const route = useRoute();
  const requested = route.kind === "page" ? route.params.get("surface") : null;
  const surface = styleSurface(requested);
  const [draft, setDraft] = useState<BrowserSettings | null>(settings);
  const [copyOpen, setCopyOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const draftRef = useRef(draft);
  const revision = useRef(0);
  const acked = useRef(0);
  const timer = useRef(0);
  const chain = useRef(Promise.resolve());
  draftRef.current = draft;

  useEffect(() => {
    if (settings && acked.current === revision.current) {
      setDraft(settings);
    }
  }, [settings]);

  useEffect(() => {
    return () => window.clearTimeout(timer.current);
  }, []);

  const write = (mine: number) => {
    const saving = draftRef.current;
    if (!saving) {
      return;
    }
    chain.current = chain.current
      .catch(() => undefined)
      .then(() => saveSettings(saving))
      .then(() => {
        if (revision.current === mine) {
          acked.current = mine;
        }
      })
      .catch((reason: unknown) => {
        if (revision.current === mine) {
          setError(String(reason));
        }
      });
  };

  const commit = (next: BrowserSettings) => {
    revision.current += 1;
    const mine = revision.current;
    draftRef.current = next;
    setDraft(next);
    setError(null);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => write(mine), SAVE_DELAY_MS);
  };

  const change = (key: ChromeKey, value: number) => {
    if (!draft) {
      return;
    }
    commit({ ...draft, [key]: value });
  };

  const changeAlign = (value: BookmarksBarAlign) => {
    if (!draft) {
      return;
    }
    commit({ ...draft, bookmarksBarAlign: value });
  };

  const resetSurface = () => {
    if (!draft) {
      return;
    }
    const next = { ...draft };
    for (const choice of surface.choices ?? []) {
      next[choice.key] = choice.options[0].value;
    }
    for (const control of surface.controls) {
      next[control.key] = CHROME[control.key];
    }
    commit(next);
  };

  const selectSurface = (id: string) => {
    window.location.hash = `#/styles?surface=${id}`;
  };

  return (
    <PageShell
      id="styles-page"
      page="styles"
      title="Styles"
      flush
      actions={
        <Button id="styles-page-copy" onClick={() => setCopyOpen(true)}>
          Copy from profile
        </Button>
      }
    >
      <div id="styles-panes" className="flex min-h-0 min-w-0 flex-1 overflow-x-auto">
        <aside id="styles-surfaces" className="flex w-52 shrink-0 flex-col gap-1 overflow-y-auto border-r border-zinc-800 p-3">
          <div id="styles-surfaces-label" className="px-3 pb-2 text-xs font-medium text-zinc-500">
            Chrome
          </div>
          {STYLE_SURFACES.map((item) => (
            <button
              key={item.id}
              id={`styles-surface-${item.id}`}
              type="button"
              aria-current={item.id === surface.id ? "true" : undefined}
              className={cn(
                "flex h-9 items-center rounded-md px-3 text-left text-sm text-zinc-300 hover:bg-zinc-900 hover:text-zinc-100",
                focusRing,
                item.id === surface.id && "bg-sky-950 text-sky-300 hover:bg-sky-950",
              )}
              onClick={() => selectSurface(item.id)}
            >
              {item.label}
            </button>
          ))}
        </aside>
        <section id="styles-controls" className="flex w-96 shrink-0 flex-col border-r border-zinc-800">
          <div id="styles-controls-header" className="flex items-start justify-between gap-3 border-b border-zinc-800 px-5 py-4">
            <div id="styles-controls-heading">
              <h2 id="styles-controls-title" className="text-sm font-medium text-zinc-100">
                {surface.label}
              </h2>
              <p id="styles-controls-hint" className="mt-1 text-xs text-zinc-500">
                {surface.hint}
              </p>
            </div>
            <Button id="styles-controls-reset" size="sm" variant="ghost" disabled={!draft} onClick={resetSurface}>
              <RotateCcw id="styles-controls-reset-icon" className="size-3.5" aria-hidden />
              Reset
            </Button>
          </div>
          <div id="styles-controls-body" className="min-h-0 flex-1 overflow-y-auto px-5 py-4">
            {error ? (
              <p id="styles-controls-error" className="mb-3 text-sm text-red-300">
                {error}
              </p>
            ) : null}
            {draft ? (
              <div id="styles-controls-list" className="flex flex-col gap-5">
                {(surface.choices ?? []).map((choice) => (
                  <ChoiceControl
                    key={choice.key}
                    choice={choice}
                    value={bookmarksBarAlign(draft[choice.key])}
                    onChange={changeAlign}
                  />
                ))}
                {surface.controls.map((control) => (
                  <MetricControl
                    key={control.key}
                    control={control}
                    value={draft[control.key]}
                    onChange={(value) => change(control.key, value)}
                  />
                ))}
              </div>
            ) : (
              <p id="styles-controls-loading" className="text-sm text-zinc-500">
                Loading sizes
              </p>
            )}
          </div>
        </section>
        <section id="styles-preview" className="min-w-0 flex-1 bg-zinc-950">
          {draft ? <StylePreview surface={surface} settings={draft} /> : null}
        </section>
      </div>
      <CopyStyleDialog
        open={copyOpen}
        onClose={() => setCopyOpen(false)}
        onCopied={(next) => {
          window.clearTimeout(timer.current);
          revision.current += 1;
          const mine = revision.current;
          draftRef.current = next;
          setDraft(next);
          setError(null);
          write(mine);
        }}
      />
    </PageShell>
  );
}

function ChoiceControl({
  choice,
  value,
  onChange,
}: {
  choice: StyleChoice;
  value: BookmarksBarAlign;
  onChange: (value: BookmarksBarAlign) => void;
}) {
  const id = `styles-choice-${choice.key}`;
  return (
    <div id={id}>
      <div id={`${id}-heading`} className="mb-1">
        <span id={`${id}-label`} className="text-sm text-zinc-100">
          {choice.label}
        </span>
      </div>
      <p id={`${id}-hint`} className="mb-2 text-xs text-zinc-500">
        {choice.hint}
      </p>
      <div id={`${id}-options`} role="radiogroup" aria-label={choice.label} className="grid grid-cols-3 gap-0.5 rounded-md border border-zinc-700 p-0.5">
        {choice.options.map((option) => (
          <button
            key={option.value}
            id={`${id}-${option.value}`}
            type="button"
            role="radio"
            aria-checked={value === option.value}
            className={cn(
              "h-8 rounded text-sm text-zinc-400 hover:text-zinc-100",
              focusRing,
              value === option.value && "bg-zinc-700 text-zinc-100",
            )}
            onClick={() => onChange(option.value)}
          >
            {option.label}
          </button>
        ))}
      </div>
    </div>
  );
}

function MetricControl({ control, value, onChange }: { control: StyleControl; value: number; onChange: (value: number) => void }) {
  const [text, setText] = useState(formatMetric(value, control.unit));

  useEffect(() => {
    setText(formatMetric(value, control.unit));
  }, [value, control.unit]);

  const id = `styles-metric-${control.key}`;
  const commitText = () => {
    const parsed = Number(text);
    const next = Number.isFinite(parsed) && text.trim() !== "" ? clampMetric(parsed, control) : value;
    setText(formatMetric(next, control.unit));
    if (next !== value) {
      onChange(next);
    }
  };

  return (
    <div id={id}>
      <div id={`${id}-heading`} className="mb-1 flex items-baseline justify-between gap-3">
        <label id={`${id}-label`} htmlFor={`${id}-range`} className="text-sm text-zinc-100">
          {control.label}
        </label>
        <span id={`${id}-unit`} className="text-xs text-zinc-500">
          {control.unit}
        </span>
      </div>
      <p id={`${id}-hint`} className="mb-2 text-xs text-zinc-500">
        {control.hint}
      </p>
      <div id={`${id}-inputs`} className="flex items-center gap-3">
        <input
          id={`${id}-range`}
          type="range"
          min={control.min}
          max={control.max}
          step={control.step}
          value={Number.isFinite(value) ? value : control.min}
          aria-valuemin={control.min}
          aria-valuemax={control.max}
          aria-valuenow={value}
          className="h-8 min-w-0 flex-1 accent-sky-500"
          onChange={(event) => onChange(clampMetric(Number(event.target.value), control))}
        />
        <input
          id={`${id}-number`}
          type="number"
          min={control.min}
          max={control.max}
          step={control.step}
          value={text}
          aria-label={control.label}
          className="h-8 w-24 rounded-md border border-zinc-700 bg-zinc-900 px-2 text-right text-sm text-zinc-100 outline-none focus:border-sky-500"
          onChange={(event) => {
            const raw = event.target.value;
            setText(raw);
            const parsed = Number(raw);
            if (raw.trim() !== "" && Number.isFinite(parsed)) {
              onChange(clampMetric(parsed, control));
            }
          }}
          onBlur={commitText}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.currentTarget.blur();
            }
          }}
        />
      </div>
    </div>
  );
}
