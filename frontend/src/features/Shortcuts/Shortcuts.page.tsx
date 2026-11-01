import { useEffect, useRef, useState } from "react";
import { listShortcuts, onShortcutsChanged, resetShortcut, resetShortcuts, saveShortcuts } from "@/lib/browser-client";
import { useAppearanceFromSettings } from "@/lib/use-browser";
import type { ShortcutBinding } from "@type/browser";

function tokenFrom(event: KeyboardEvent) {
  const name = event.key.length === 1 ? event.key.toLowerCase() : event.key.toLowerCase();
  const key =
    name === "+" || name === "="
      ? "plus"
      : name === "-"
        ? "minus"
        : name === "arrowleft"
          ? "left"
          : name === "arrowright"
            ? "right"
            : name === "escape"
              ? "esc"
              : name === " "
                ? "space"
                : name;
  if (["control", "shift", "alt", "meta"].includes(key)) {
    return null;
  }
  const parts = [];
  if (event.ctrlKey || event.metaKey) {
    parts.push("ctrl");
  }
  if (event.shiftKey) {
    parts.push("shift");
  }
  if (event.altKey) {
    parts.push("alt");
  }
  parts.push(key);
  return parts.join("+");
}

export function ShortcutsPage() {
  useAppearanceFromSettings();
  const [bindings, setBindings] = useState<ShortcutBinding[] | null>(null);
  const [recording, setRecording] = useState<string | null>(null);
  const [first, setFirst] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const bindingsRef = useRef(bindings);
  bindingsRef.current = bindings;

  useEffect(() => {
    document.title = "Shortcuts";
    void listShortcuts().then(setBindings).catch((reason: unknown) => setError(String(reason)));
    const unlisten = onShortcutsChanged(setBindings);
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);

  useEffect(() => {
    if (!recording) {
      return;
    }
    let timer = 0;
    let chord: string | null = null;
    const onKey = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();
      const token = tokenFrom(event);
      if (!token) {
        return;
      }
      const current = bindingsRef.current;
      if (!current) {
        return;
      }
      const write = (keys: string[]) => {
        setRecording(null);
        setFirst(null);
        const next = current.map((binding) => (binding.action === recording ? { ...binding, keys } : binding));
        void saveShortcuts(next).then(setBindings).catch((reason: unknown) => setError(String(reason)));
      };
      if (!chord) {
        chord = token;
        setFirst(token);
        timer = window.setTimeout(() => write([token]), 1000);
        return;
      }
      window.clearTimeout(timer);
      write([chord, token]);
    };
    window.addEventListener("keydown", onKey, true);
    return () => {
      window.clearTimeout(timer);
      window.removeEventListener("keydown", onKey, true);
    };
  }, [recording]);

  const conflict = (binding: ShortcutBinding) =>
    bindings?.find((other) => other.action !== binding.action && other.keys.join(" ") === binding.keys.join(" ") && binding.keys.length > 0);

  return (
    <main id="shortcuts-page" className="h-full overflow-y-auto bg-zinc-950 text-zinc-100">
      <div id="shortcuts-body" className="mx-auto flex max-w-3xl flex-col gap-4 px-6 py-8">
        <header id="shortcuts-header" className="flex items-end justify-between gap-3">
          <div id="shortcuts-heading">
            <h1 id="shortcuts-title" className="text-lg font-semibold">
              Keyboard shortcuts
            </h1>
            <p id="shortcuts-hint" className="mt-1 text-sm text-zinc-500">
              Record keeps the first key, then a second key within one second makes a chord. Esc still closes menus.
            </p>
          </div>
          <button
            id="shortcuts-reset-all"
            type="button"
            className="h-8 rounded-md border border-zinc-700 px-3 text-xs hover:bg-zinc-800"
            onClick={() => void resetShortcuts().then(setBindings).catch((reason: unknown) => setError(String(reason)))}
          >
            Reset all
          </button>
        </header>
        {error ? (
          <p id="shortcuts-error" className="text-sm text-red-300">
            {error}
          </p>
        ) : null}
        <ul id="shortcuts-list" className="flex flex-col divide-y divide-zinc-800 rounded-lg border border-zinc-800">
          {(bindings ?? []).map((binding) => {
            const other = conflict(binding);
            return (
              <li id={`shortcut-${binding.action}`} key={binding.action} className="flex items-center gap-3 px-4 py-3">
                <div id={`shortcut-${binding.action}-text`} className="min-w-0 flex-1">
                  <div id={`shortcut-${binding.action}-label`} className="text-sm">
                    {binding.label}
                  </div>
                  {other ? (
                    <div id={`shortcut-${binding.action}-conflict`} className="text-xs text-amber-300">
                      Also used by {other.label}
                    </div>
                  ) : null}
                </div>
                <span id={`shortcut-${binding.action}-keys`} className="text-xs text-zinc-400">
                  {recording === binding.action ? (first ? `${first} then…` : "Press a key") : binding.keys.join(" then ") || "Unbound"}
                </span>
                <button
                  id={`shortcut-${binding.action}-record`}
                  type="button"
                  className="h-8 rounded-md border border-zinc-700 px-3 text-xs hover:bg-zinc-800"
                  onClick={() => {
                    setFirst(null);
                    setRecording(binding.action);
                  }}
                >
                  Record
                </button>
                <button
                  id={`shortcut-${binding.action}-reset`}
                  type="button"
                  className="h-8 rounded-md px-2 text-xs text-zinc-400 hover:bg-zinc-800"
                  onClick={() => void resetShortcut(binding.action).then(setBindings)}
                >
                  Reset
                </button>
              </li>
            );
          })}
        </ul>
      </div>
    </main>
  );
}
