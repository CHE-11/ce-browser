import { useEffect, useMemo, useRef, useState } from "react";
import { Search, SearchX } from "lucide-react";
import { listShortcuts, onShortcutsChanged, resetShortcut, resetShortcuts, saveShortcuts } from "@/lib/browser-client";
import { Button } from "@/components/Button";
import { EmptyState } from "@/components/EmptyState";
import { SkeletonRows } from "@/components/Skeleton";
import { cn } from "@/lib/cn";
import { formatKeys, formatKeyToken } from "@/lib/shortcut-keys";
import { useAppearanceFromSettings, useSettings } from "@/lib/use-browser";
import type { ShortcutBinding } from "@type/browser";

/** Punctuation keys by physical position, so Shift and the keyboard layout don't change the name. */
const CODE_NAMES: Record<string, string> = {
  Equal: "plus",
  NumpadAdd: "plus",
  Minus: "minus",
  NumpadSubtract: "minus",
  Slash: "slash",
  Comma: "comma",
  Period: "period",
  Semicolon: "semicolon",
  Quote: "quote",
  BracketLeft: "bracketleft",
  BracketRight: "bracketright",
  Backslash: "backslash",
  Backquote: "backquote",
};

function tokenFrom(event: KeyboardEvent) {
  const name = event.key.toLowerCase();
  const key =
    CODE_NAMES[event.code] ??
    (name === "arrowleft"
      ? "left"
      : name === "arrowright"
        ? "right"
        : name === "escape"
          ? "esc"
          : name === " "
            ? "space"
            : /^Digit\d$/.test(event.code)
              ? event.code.slice(5)
              : /^Key[A-Z]$/.test(event.code)
                ? event.code.slice(3).toLowerCase()
                : name);
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

function matchesQuery(binding: ShortcutBinding, needle: string) {
  const haystack = [binding.label, binding.category ?? "", binding.action, binding.keys.join(" "), formatKeys(binding.keys)].join(" ").toLowerCase();
  return needle.split(/\s+/).every((word) => haystack.includes(word));
}

export function ShortcutsPage() {
  useAppearanceFromSettings();
  const settings = useSettings();
  const [bindings, setBindings] = useState<ShortcutBinding[] | null>(null);
  const [recording, setRecording] = useState<string | null>(null);
  const [first, setFirst] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
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
      const target = event.target;
      if (target instanceof HTMLElement && target.closest("input, textarea, [contenteditable='true']")) {
        return;
      }
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

  const visible = useMemo(() => {
    const rows = (bindings ?? []).filter((binding) => settings?.workspacesEnabled !== false || binding.category !== "Workspaces");
    const needle = query.trim().toLowerCase();
    return needle ? rows.filter((binding) => matchesQuery(binding, needle)) : rows;
  }, [bindings, query, settings?.workspacesEnabled]);

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
          <div id="shortcuts-header-actions" className="flex shrink-0 items-center gap-2">
            <label id="shortcuts-search" className="flex h-8 w-64 items-center gap-2 rounded-md border border-zinc-700 bg-zinc-900 px-2">
              <Search id="shortcuts-search-icon" className="size-4 text-zinc-500" aria-hidden />
              <input
                id="shortcuts-search-input"
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder="Search shortcuts"
                aria-label="Search shortcuts"
                className="min-w-0 flex-1 bg-transparent text-sm outline-none placeholder:text-zinc-500"
              />
            </label>
            <Button
              id="shortcuts-reset-all"
              size="sm"
              variant="outline"
              onClick={() => void resetShortcuts().then(setBindings).catch((reason: unknown) => setError(String(reason)))}
            >
              Reset all
            </Button>
          </div>
        </header>
        {error ? (
          <p id="shortcuts-error" className="text-sm text-red-300">
            {error}
          </p>
        ) : null}
        {bindings === null && !error ? <SkeletonRows id="shortcuts-loading" rows={8} className="rounded-lg border border-zinc-800 p-4" /> : null}
        {bindings !== null && query.trim() && visible.length === 0 ? (
          <EmptyState id="shortcuts-empty" icon={SearchX} title="No shortcuts match that search" hint="Search looks at names, categories, and keys." />
        ) : null}
        <ul
          id="shortcuts-list"
          className={cn("flex flex-col divide-y divide-zinc-800 rounded-lg border border-zinc-800", (bindings === null || visible.length === 0) && "hidden")}
        >
          {visible.map((binding) => {
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
                  {recording === binding.action
                    ? first
                      ? `${formatKeyToken(first)} then…`
                      : "Press a key"
                    : formatKeys(binding.keys) || "Unbound"}
                </span>
                <Button
                  id={`shortcut-${binding.action}-record`}
                  size="sm"
                  variant="outline"
                  aria-label={`Record a shortcut for ${binding.label}`}
                  onClick={() => {
                    setFirst(null);
                    setRecording(binding.action);
                  }}
                >
                  Record
                </Button>
                <Button
                  id={`shortcut-${binding.action}-reset`}
                  size="sm"
                  variant="ghost"
                  aria-label={`Reset the shortcut for ${binding.label}`}
                  onClick={() => void resetShortcut(binding.action).then(setBindings)}
                >
                  Reset
                </Button>
              </li>
            );
          })}
        </ul>
      </div>
    </main>
  );
}
