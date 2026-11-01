import { useEffect, useState } from "react";
import { Archive, BookOpen, KeyRound, Languages, Sparkles, Tags } from "lucide-react";
import { Button } from "@/components/Button";
import { PopupFrame } from "@/components/PopupFrame";
import { PopoverHeader } from "@features/BuiltInExtensions/PopoverParts";
import { readPopoverTab } from "@features/BuiltInExtensions/shared";
import {
  askPage,
  checkPrices,
  closePopup,
  createTab,
  generatePassword,
  listPrices,
  readingStats,
  restoreTranslation,
  trackPrice,
  translatePage,
  waybackUrl,
} from "@/lib/browser-client";
import { useSettings } from "@/lib/use-browser";
import type { ProductPrice, ReadingStats } from "@type/browser";

const fieldClass = "h-8 w-full rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm outline-none focus:border-sky-500";

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
    <PopupFrame id="translate-popover" maxHeight={280}>
      <PopoverHeader id="translate-header" icon={<Languages aria-hidden />} title="Translate" subtitle={tab.title || tab.url} />
      <div id="translate-body" className="flex flex-col gap-2 px-3 pb-3">
        <label id="translate-language-label" className="text-xs text-zinc-500">
          Language code
          <input id="translate-language" className={fieldClass} value={language} onChange={(event) => setLanguage(event.target.value)} />
        </label>
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

export function PasswordPopover() {
  const [length, setLength] = useState(16);
  const [upper, setUpper] = useState(true);
  const [lower, setLower] = useState(true);
  const [digits, setDigits] = useState(true);
  const [symbols, setSymbols] = useState(true);
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");

  const make = () => {
    setError("");
    void generatePassword(length, upper, lower, digits, symbols)
      .then(setPassword)
      .catch((reason: unknown) => setError(reason instanceof Error ? reason.message : "Couldn't generate a password."));
  };

  return (
    <PopupFrame id="password-popover" maxHeight={420}>
      <PopoverHeader id="password-header" icon={<KeyRound aria-hidden />} title="Password" subtitle="Generated on this computer" />
      <div id="password-body" className="flex flex-col gap-2 px-3 pb-3">
        <label id="password-length-label" className="text-xs text-zinc-500">
          Length {length}
          <input
            id="password-length"
            type="range"
            min={8}
            max={64}
            value={length}
            onChange={(event) => setLength(Number(event.target.value))}
          />
        </label>
        <div id="password-classes" className="grid grid-cols-2 gap-1 text-sm">
          <Class id="password-upper" label="Uppercase" checked={upper} onChange={setUpper} />
          <Class id="password-lower" label="Lowercase" checked={lower} onChange={setLower} />
          <Class id="password-digits" label="Digits" checked={digits} onChange={setDigits} />
          <Class id="password-symbols" label="Symbols" checked={symbols} onChange={setSymbols} />
        </div>
        <output id="password-value" className="break-all rounded-md bg-zinc-900 px-2 py-2 font-mono text-sm">
          {password || " "}
        </output>
        {error ? (
          <p id="password-error" className="text-xs text-amber-300">
            {error}
          </p>
        ) : null}
        <div id="password-actions" className="flex gap-2">
          <Button id="password-generate" variant="primary" className="flex-1" onClick={make}>
            Generate
          </Button>
          <Button
            id="password-copy"
            className="flex-1"
            disabled={!password}
            onClick={() => void navigator.clipboard.writeText(password)}
          >
            Copy
          </Button>
        </div>
      </div>
    </PopupFrame>
  );
}

function Class({ id, label, checked, onChange }: { id: string; label: string; checked: boolean; onChange: (value: boolean) => void }) {
  return (
    <label id={`${id}-label`} className="flex items-center gap-2">
      <input id={id} type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} />
      {label}
    </label>
  );
}

export function PricePopover({ params }: { params: URLSearchParams }) {
  const tab = readPopoverTab(params);
  const [items, setItems] = useState<ProductPrice[]>([]);
  const [error, setError] = useState("");

  const load = () => {
    void listPrices().then(setItems).catch(() => undefined);
  };

  useEffect(load, []);

  return (
    <PopupFrame id="price-popover" maxHeight={460}>
      <PopoverHeader id="price-header" icon={<Tags aria-hidden />} title="Prices" subtitle={tab.title || tab.url} />
      <div id="price-body" className="flex flex-col gap-2 px-3 pb-3">
        <div id="price-actions" className="flex gap-2">
          <Button
            id="price-track"
            variant="primary"
            className="flex-1"
            onClick={() =>
              void trackPrice(tab.id)
                .then(() => load())
                .catch((reason: unknown) => setError(reason instanceof Error ? reason.message : "No price on this page."))
            }
          >
            Track
          </Button>
          <Button
            id="price-check"
            className="flex-1"
            onClick={() =>
              void checkPrices()
                .then(setItems)
                .catch((reason: unknown) => setError(reason instanceof Error ? reason.message : "Couldn't check prices."))
            }
          >
            Check
          </Button>
        </div>
        {error ? (
          <p id="price-error" className="text-xs text-amber-300">
            {error}
          </p>
        ) : null}
        <ul id="price-list" className="flex flex-col gap-1">
          {items.length === 0 ? (
            <li id="price-empty" className="text-sm text-zinc-500">
              Nothing tracked yet.
            </li>
          ) : (
            items.map((item, index) => (
              <li id={`price-item-${index}`} key={item.url} className="rounded-md bg-zinc-900 px-2 py-1.5 text-sm">
                <p id={`price-title-${index}`} className="truncate">
                  {item.title || item.url}
                </p>
                <p id={`price-amount-${index}`} className="text-xs text-zinc-400">
                  {item.currency} {item.price}
                  {item.availability ? ` · ${item.availability}` : ""}
                  {item.dropped ? " · lower" : ""}
                  {item.stockChanged ? " · stock changed" : ""}
                </p>
              </li>
            ))
          )}
        </ul>
      </div>
    </PopupFrame>
  );
}

export function WaybackPopover({ params }: { params: URLSearchParams }) {
  const tab = readPopoverTab(params);
  const [error, setError] = useState("");

  const open = (save: boolean) => {
    void waybackUrl(save, tab.url)
      .then((url) => createTab(url))
      .then(() => closePopup())
      .catch((reason: unknown) => setError(reason instanceof Error ? reason.message : "That page can't be archived."));
  };

  return (
    <PopupFrame id="wayback-popover" maxHeight={240}>
      <PopoverHeader id="wayback-header" icon={<Archive aria-hidden />} title="Wayback" subtitle={tab.url} />
      <div id="wayback-body" className="flex flex-col gap-2 px-3 pb-3">
        {error ? (
          <p id="wayback-error" className="text-xs text-amber-300">
            {error}
          </p>
        ) : null}
        <Button id="wayback-open" variant="primary" onClick={() => open(false)}>
          Open in the archive
        </Button>
        <Button id="wayback-save" onClick={() => open(true)}>
          Save this page
        </Button>
      </div>
    </PopupFrame>
  );
}

export function ReadingPopover({ params }: { params: URLSearchParams }) {
  const tab = readPopoverTab(params);
  const [stats, setStats] = useState<ReadingStats | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    void readingStats(tab.id)
      .then(setStats)
      .catch((reason: unknown) => setError(reason instanceof Error ? reason.message : "Couldn't read this page."));
  }, [tab.id]);

  return (
    <PopupFrame id="reading-popover" maxHeight={240}>
      <PopoverHeader id="reading-header" icon={<BookOpen aria-hidden />} title="Reading" subtitle={tab.title || tab.url} />
      <div id="reading-body" className="grid grid-cols-3 gap-2 px-3 pb-3 text-center">
        {error ? (
          <p id="reading-error" className="col-span-3 text-xs text-amber-300">
            {error}
          </p>
        ) : (
          <>
            <Stat id="reading-words" label="Words" value={stats ? String(stats.words) : "…"} />
            <Stat id="reading-minutes" label="Minutes" value={stats ? String(stats.minutes) : "…"} />
            <Stat id="reading-ease" label="Ease" value={stats ? String(stats.ease) : "…"} />
          </>
        )}
      </div>
    </PopupFrame>
  );
}

function Stat({ id, label, value }: { id: string; label: string; value: string }) {
  return (
    <div id={id} className="rounded-lg bg-zinc-900 px-2 py-3">
      <p id={`${id}-value`} className="text-lg font-semibold">
        {value}
      </p>
      <p id={`${id}-label`} className="text-xs text-zinc-500">
        {label}
      </p>
    </div>
  );
}

export function AssistantPopover({ params }: { params: URLSearchParams }) {
  const tab = readPopoverTab(params);
  const [question, setQuestion] = useState("");
  const [answer, setAnswer] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const ask = (text: string) => {
    setBusy(true);
    setError("");
    void askPage(tab.id, text)
      .then(setAnswer)
      .catch((reason: unknown) => setError(reason instanceof Error ? reason.message : "Couldn't reach the assistant."))
      .finally(() => setBusy(false));
  };

  return (
    <PopupFrame id="assistant-popover" maxHeight={480}>
      <PopoverHeader id="assistant-header" icon={<Sparkles aria-hidden />} title="Assistant" subtitle={tab.title || tab.url} />
      <div id="assistant-body" className="flex flex-col gap-2 px-3 pb-3">
        <Button id="assistant-summarize" variant="primary" disabled={busy} onClick={() => ask("")}>
          Summarize
        </Button>
        <textarea
          id="assistant-question"
          value={question}
          rows={3}
          placeholder="Ask about this page"
          className="rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-sm outline-none focus:border-sky-500"
          onChange={(event) => setQuestion(event.target.value)}
        />
        <Button id="assistant-ask" disabled={busy || !question.trim()} onClick={() => ask(question)}>
          Ask
        </Button>
        {error ? (
          <p id="assistant-error" className="text-xs text-amber-300">
            {error}
          </p>
        ) : null}
        {answer ? (
          <p id="assistant-answer" className="whitespace-pre-wrap text-sm text-zinc-200">
            {answer}
          </p>
        ) : null}
      </div>
    </PopupFrame>
  );
}
