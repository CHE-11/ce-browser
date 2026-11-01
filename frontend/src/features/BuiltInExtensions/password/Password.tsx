import { useEffect, useState } from "react";
import { KeyRound } from "lucide-react";
import { Button } from "@/components/Button";
import { PopupFrame } from "@/components/PopupFrame";
import { copyText, generatePassword, showNotice } from "@/lib/browser-client";
import type { TabInfo } from "@type/browser";
import { PopoverHeader } from "../PopoverParts";
import { ToolButton } from "../ToolButton";

const COPIED = "Password copied";

function reasonText(reason: unknown, fallback: string) {
  if (typeof reason === "string" && reason.trim()) {
    return reason;
  }
  if (reason instanceof Error && reason.message) {
    return reason.message;
  }
  return fallback;
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
      height={480}
    />
  );
}

export function PasswordPopover() {
  const [length, setLength] = useState(16);
  const [upper, setUpper] = useState(true);
  const [lower, setLower] = useState(true);
  const [digits, setDigits] = useState(true);
  const [symbols, setSymbols] = useState(true);
  const [minSymbols, setMinSymbols] = useState(1);
  const [avoidAmbiguous, setAvoidAmbiguous] = useState(true);
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");

  const minimum = symbols ? Math.min(minSymbols, length) : 0;

  const make = () => {
    setError("");
    void generatePassword(length, upper, lower, digits, symbols, minimum, avoidAmbiguous)
      .then(setPassword)
      .catch((reason: unknown) => setError(reasonText(reason, "Couldn't generate a password.")));
  };

  useEffect(() => {
    // Runs once, from the options above. Editing them waits for Generate.
    let active = true;
    setError("");
    void generatePassword(length, upper, lower, digits, symbols, minimum, avoidAmbiguous)
      .then((value) => {
        if (!active) {
          return;
        }
        setPassword(value);
        return copyText(value).then(() => {
          if (active) {
            return showNotice(COPIED);
          }
        });
      })
      .catch((reason: unknown) => {
        if (active) {
          setError(reasonText(reason, "Couldn't generate a password."));
        }
      });
    return () => {
      active = false;
    };
  }, []);

  const copy = () => {
    setError("");
    void copyText(password)
      .then(() => showNotice(COPIED))
      .catch((reason: unknown) => setError(reasonText(reason, "Couldn't copy that.")));
  };

  return (
    <PopupFrame id="password-popover" maxHeight={520}>
      <PopoverHeader id="password-header" icon={<KeyRound aria-hidden />} title="Password" subtitle="Generated on this computer" />
      <div id="password-body" className="flex flex-col gap-2 px-3 pb-3">
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
          <Button id="password-copy" className="flex-1" disabled={!password} onClick={copy}>
            Copy
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
