import { useState } from "react";
import { Button } from "@/components/Button";
import { Dialog } from "@/components/Dialog";
import { clearHistory } from "@/lib/browser-client";
import { HistoryItem } from "../../../../types/browser";

type ClearChoice = "everything" | "hour" | "today" | "week" | "range";

const CHOICES: { id: ClearChoice; label: string; hint: string }[] = [
  { id: "everything", label: "Everything", hint: "Every page in history" },
  { id: "hour", label: "Last hour", hint: "The past 60 minutes" },
  { id: "today", label: "Today", hint: "Since midnight" },
  { id: "week", label: "This week", hint: "Since Monday" },
  { id: "range", label: "Date range", hint: "From the start of the first day through the end of the last" },
];

function dateInputValue(date: Date) {
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${date.getFullYear()}-${month}-${day}`;
}

function startOfDay(date: Date) {
  const start = new Date(date);
  start.setHours(0, 0, 0, 0);
  return start.getTime();
}

function startOfWeek(date: Date) {
  const start = new Date(date);
  start.setHours(0, 0, 0, 0);
  const weekday = start.getDay();
  start.setDate(start.getDate() - (weekday === 0 ? 6 : weekday - 1));
  return start.getTime();
}

function dayStamp(value: string, end: boolean) {
  const [year, month, day] = value.split("-").map(Number);
  if (!year || !month || !day) {
    return null;
  }
  const date = new Date(year, month - 1, day, end ? 23 : 0, end ? 59 : 0, end ? 59 : 0, end ? 999 : 0);
  const stamp = date.getTime();
  return Number.isNaN(stamp) ? null : stamp;
}

/** Milliseconds of the last visits to remove. `null` means the date range is not usable yet. */
function windowFor(choice: ClearChoice, from: string, to: string, now: number): { since?: number; until?: number } | null {
  if (choice === "everything") {
    return {};
  }
  if (choice === "hour") {
    return { since: now - 60 * 60 * 1000, until: now };
  }
  if (choice === "today") {
    return { since: startOfDay(new Date(now)), until: now };
  }
  if (choice === "week") {
    return { since: startOfWeek(new Date(now)), until: now };
  }
  const since = dayStamp(from, false);
  const until = dayStamp(to, true);
  if (since === null || until === null || since > until) {
    return null;
  }
  return { since, until };
}

function matches(item: HistoryItem, range: { since?: number; until?: number }) {
  if (range.since !== undefined && item.lastVisited < range.since) {
    return false;
  }
  if (range.until !== undefined && item.lastVisited > range.until) {
    return false;
  }
  return true;
}

export function ClearHistoryDialog({ history, onClose }: { history: HistoryItem[]; onClose: () => void }) {
  const today = dateInputValue(new Date());
  const [choice, setChoice] = useState<ClearChoice>("everything");
  const [from, setFrom] = useState(today);
  const [to, setTo] = useState(today);
  const [busy, setBusy] = useState(false);
  const range = windowFor(choice, from, to, Date.now());
  const count = range ? history.filter((item) => matches(item, range)).length : 0;
  const rangeInvalid = choice === "range" && range === null;

  const clear = () => {
    if (!range || count === 0 || busy) {
      return;
    }
    setBusy(true);
    void clearHistory(range)
      .then(() => onClose())
      .catch(() => setBusy(false));
  };

  return (
    <Dialog id="history-clear-dialog" title="Clear history" open onClose={onClose} panelClassName="max-w-md">
      <form
        id="history-clear-form"
        className="flex flex-col gap-3"
        onSubmit={(event) => {
          event.preventDefault();
          clear();
        }}
      >
        <fieldset id="history-clear-choices" className="flex flex-col gap-2">
          <legend id="history-clear-choices-legend" className="mb-1 text-sm text-zinc-300">
            What to clear
          </legend>
          {CHOICES.map((option) => (
            <label
              key={option.id}
              id={`history-clear-${option.id}-label`}
              className="flex items-start gap-2 text-sm text-zinc-200"
            >
              <input
                id={`history-clear-${option.id}`}
                type="radio"
                name="history-clear-choice"
                className="mt-1"
                autoFocus={option.id === "everything"}
                checked={choice === option.id}
                onChange={() => setChoice(option.id)}
              />
              <span id={`history-clear-${option.id}-text`} className="flex flex-col">
                <span id={`history-clear-${option.id}-title`}>{option.label}</span>
                <span id={`history-clear-${option.id}-hint`} className="text-xs text-zinc-500">
                  {option.hint}
                </span>
              </span>
            </label>
          ))}
        </fieldset>
        {choice === "range" ? (
          <div id="history-clear-dates" className="grid grid-cols-2 gap-2">
            <label id="history-clear-from-label" className="flex flex-col gap-1 text-xs text-zinc-400">
              From
              <input
                id="history-clear-from"
                type="date"
                required
                value={from}
                max={to || undefined}
                onChange={(event) => setFrom(event.target.value)}
                className="scheme-dark h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500"
              />
            </label>
            <label id="history-clear-to-label" className="flex flex-col gap-1 text-xs text-zinc-400">
              To
              <input
                id="history-clear-to"
                type="date"
                required
                value={to}
                min={from || undefined}
                onChange={(event) => setTo(event.target.value)}
                className="scheme-dark h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500"
              />
            </label>
          </div>
        ) : null}
        <p id="history-clear-summary" className="text-sm text-zinc-400">
          {rangeInvalid
            ? "The start date has to be on or before the end date."
            : count === 0
              ? "Nothing in history falls in that range."
              : count === 1
                ? "This removes 1 entry."
                : `This removes ${count} entries.`}
        </p>
        <div id="history-clear-actions" className="flex justify-end gap-2">
          <Button id="history-clear-cancel" variant="ghost" disabled={busy} onClick={onClose}>
            Cancel
          </Button>
          <Button id="history-clear-confirm" type="submit" variant="destructive" disabled={busy || !range || count === 0}>
            Clear history
          </Button>
        </div>
      </form>
    </Dialog>
  );
}
