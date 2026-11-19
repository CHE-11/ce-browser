import { useEffect, useMemo, useRef, useState } from "react";
import { KeyRound } from "lucide-react";
import { Button } from "@/components/Button";
import { PopupFrame } from "@/components/PopupFrame";
import { copyText, generatePassword, passwordSettings, savePasswordSettings } from "@/lib/browser-client";
import type { PasswordSettings, TabInfo } from "@type/browser";
import { PopoverHeader } from "../PopoverParts";
import { ToolButton } from "../ToolButton";

const COPIED_MS = 1600;
const SAVE_MS = 250;

const DEFAULT_PASSWORD: PasswordSettings = {
  length: 16,
  upper: true,
  lower: true,
  digits: true,
  symbols: true,
  minSymbols: 1,
  avoidAmbiguous: true,
  exclude: "",
};

function reasonText(reason: unknown, fallback: string) {
  if (typeof reason === "string" && reason.trim()) {
    return reason;
  }
  if (reason instanceof Error && reason.message) {
    return reason.message;
  }
  return fallback;
}

function sameSettings(left: PasswordSettings, right: PasswordSettings) {
  return (
    left.length === right.length &&
    left.upper === right.upper &&
    left.lower === right.lower &&
    left.digits === right.digits &&
    left.symbols === right.symbols &&
    left.minSymbols === right.minSymbols &&
    left.avoidAmbiguous === right.avoidAmbiguous &&
    left.exclude === right.exclude
  );
}

function requestPassword(options: PasswordSettings) {
  const minimum = options.symbols ? Math.min(options.minSymbols, options.length) : 0;
  return generatePassword(
    options.length,
    options.upper,
    options.lower,
    options.digits,
    options.symbols,
    minimum,
    options.avoidAmbiguous,
    options.exclude,
  );
}

export function PasswordButton({ activeTab }: { activeTab: TabInfo | undefined }) {
  return (
    <ToolButton
      id="password"
      label="Password"
      kind="password"
      icon={<KeyRound id="address-password-icon" aria-hidden />}
      activeTab={activeTab}
      needsPage={false}
    />
  );
}

export function PasswordPopover() {
  const [length, setLength] = useState(DEFAULT_PASSWORD.length);
  const [upper, setUpper] = useState(DEFAULT_PASSWORD.upper);
  const [lower, setLower] = useState(DEFAULT_PASSWORD.lower);
  const [digits, setDigits] = useState(DEFAULT_PASSWORD.digits);
  const [symbols, setSymbols] = useState(DEFAULT_PASSWORD.symbols);
  const [minSymbols, setMinSymbols] = useState(DEFAULT_PASSWORD.minSymbols);
  const [avoidAmbiguous, setAvoidAmbiguous] = useState(DEFAULT_PASSWORD.avoidAmbiguous);
  const [exclude, setExclude] = useState(DEFAULT_PASSWORD.exclude);
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [copied, setCopied] = useState(false);
  const [ready, setReady] = useState(false);
  const copiedTimer = useRef<number | undefined>(undefined);
  const saved = useRef<PasswordSettings | null>(null);
  const pending = useRef<PasswordSettings | null>(null);
  const write = useRef(0);

  const settings = useMemo<PasswordSettings>(
    () => ({ length, upper, lower, digits, symbols, minSymbols, avoidAmbiguous, exclude }),
    [length, upper, lower, digits, symbols, minSymbols, avoidAmbiguous, exclude],
  );

  const markCopied = () => {
    setCopied(true);
    window.clearTimeout(copiedTimer.current);
    copiedTimer.current = window.setTimeout(() => setCopied(false), COPIED_MS);
  };

  const persist = (next: PasswordSettings) => {
    const token = ++write.current;
    void savePasswordSettings(next)
      .then((stored) => {
        if (write.current === token) {
          saved.current = stored;
        }
      })
      .catch(() => undefined);
  };

  const make = () => {
    if (!ready) {
      return;
    }
    setError("");
    void requestPassword(settings)
      .then(setPassword)
      .catch((reason: unknown) => setError(reasonText(reason, "Couldn't generate a password.")));
  };

  useEffect(() => {
    let active = true;
    setError("");
    void passwordSettings()
      .catch(() => DEFAULT_PASSWORD)
      .then(async (loaded) => {
        if (!active) {
          return;
        }
        saved.current = loaded;
        setLength(loaded.length);
        setUpper(loaded.upper);
        setLower(loaded.lower);
        setDigits(loaded.digits);
        setSymbols(loaded.symbols);
        setMinSymbols(loaded.minSymbols);
        setAvoidAmbiguous(loaded.avoidAmbiguous);
        setExclude(loaded.exclude);
        try {
          const value = await requestPassword(loaded);
          if (!active) {
            return;
          }
          setPassword(value);
          await copyText(value);
          if (active) {
            markCopied();
          }
        } catch (reason: unknown) {
          if (active) {
            setError(reasonText(reason, "Couldn't generate a password."));
          }
        }
        if (active) {
          setReady(true);
        }
      });
    return () => {
      active = false;
      window.clearTimeout(copiedTimer.current);
    };
  }, []);

  useEffect(() => {
    if (!ready || !saved.current) {
      return;
    }
    if (sameSettings(saved.current, settings)) {
      pending.current = null;
      return;
    }
    pending.current = settings;
    const handle = window.setTimeout(() => {
      const next = pending.current;
      if (!next) {
        return;
      }
      pending.current = null;
      persist(next);
    }, SAVE_MS);
    return () => window.clearTimeout(handle);
  }, [ready, settings]);

  useEffect(() => {
    return () => {
      const next = pending.current;
      if (!next) {
        return;
      }
      pending.current = null;
      void savePasswordSettings(next).catch(() => undefined);
    };
  }, []);

  const copy = () => {
    setError("");
    void copyText(password).catch((reason: unknown) => setError(reasonText(reason, "Couldn't copy that.")));
  };

  return (
    <PopupFrame id="password-popover" width={320} maxHeight={640} ready={ready}>
      <PopoverHeader id="password-header" icon={<KeyRound aria-hidden />} title="Password" subtitle="Generated on this computer" />
      <div id="password-body" className="flex flex-col gap-2 px-3 pb-3">
        <fieldset id="password-options" disabled={!ready} className="m-0 flex flex-col gap-2 border-0 p-0">
          <label id="password-length-label" className="flex items-center gap-2 text-xs text-zinc-400">
            <span id="password-length-name" className="w-14 shrink-0">
              Length
            </span>
            <input
              id="password-length"
              type="range"
              min={8}
              max={64}
              value={length}
              className="min-w-0 flex-1"
              onChange={(event) => {
                const next = Number(event.target.value);
                setLength(next);
                setMinSymbols((current) => Math.min(current, next));
              }}
            />
            <span id="password-length-value" className="w-6 text-right tabular-nums">
              {length}
            </span>
          </label>
          <div id="password-classes" className="grid grid-cols-2 gap-1 text-sm">
            <Class id="password-upper" label="Uppercase" checked={upper} onChange={setUpper} />
            <Class id="password-lower" label="Lowercase" checked={lower} onChange={setLower} />
            <Class id="password-digits" label="Digits" checked={digits} onChange={setDigits} />
            <Class id="password-symbols" label="Symbols" checked={symbols} onChange={setSymbols} />
          </div>
          <label
            id="password-min-symbols-label"
            className={`flex items-center justify-between gap-3 text-sm ${symbols ? "" : "opacity-40"}`}
          >
            <span id="password-min-symbols-text">Minimum special</span>
            <input
              id="password-min-symbols"
              type="number"
              min={0}
              max={length}
              disabled={!symbols}
              value={minSymbols}
              className="h-8 w-16 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-right text-sm disabled:opacity-40"
              onChange={(event) => {
                const next = Number(event.target.value);
                if (Number.isNaN(next)) {
                  return;
                }
                setMinSymbols(Math.min(length, Math.max(0, Math.round(next))));
              }}
            />
          </label>
          <Class id="password-ambiguous" label="Avoid ambiguous characters" checked={avoidAmbiguous} onChange={setAvoidAmbiguous} />
          <label id="password-exclude-label" className="flex flex-col gap-1 text-sm">
            <span id="password-exclude-name">Characters to not use</span>
            <input
              id="password-exclude"
              type="text"
              value={exclude}
              placeholder="a, e, i"
              spellCheck={false}
              autoComplete="off"
              className="h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2 font-mono text-sm"
              onChange={(event) => setExclude(event.target.value.slice(0, 240))}
            />
          </label>
        </fieldset>
        <output id="password-value" className="break-all rounded-md bg-zinc-900 px-2 py-2 font-mono text-sm">
          {password || " "}
        </output>
        {error ? (
          <p id="password-error" className="text-xs text-amber-300">
            {error}
          </p>
        ) : null}
        <div id="password-actions" className="flex gap-2">
          <Button id="password-generate" variant="primary" className="flex-1" disabled={!ready} onClick={make}>
            Generate
          </Button>
          <Button id="password-copy" className="flex-1" disabled={!password} onClick={copy}>
            {copied ? "Copied!" : "Copy"}
          </Button>
        </div>
      </div>
    </PopupFrame>
  );
}

function Class({ id, label, checked, onChange }: { id: string; label: string; checked: boolean; onChange: (value: boolean) => void }) {
  return (
    <label id={`${id}-label`} className="flex items-center gap-2 text-sm">
      <input id={id} type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} />
      {label}
    </label>
  );
}
