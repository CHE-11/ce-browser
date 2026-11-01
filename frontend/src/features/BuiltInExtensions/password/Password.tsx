import { useState } from "react";
import { KeyRound } from "lucide-react";
import { Button } from "@/components/Button";
import { PopupFrame } from "@/components/PopupFrame";
import { generatePassword } from "@/lib/browser-client";
import type { TabInfo } from "@type/browser";
import { PopoverHeader } from "../PopoverParts";
import { ToolButton } from "../ToolButton";

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
