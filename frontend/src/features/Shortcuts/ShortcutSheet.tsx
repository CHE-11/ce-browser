import { useEffect, useMemo, useState } from "react";
import { Keyboard, Pencil } from "lucide-react";
import { Button } from "@/components/Button";
import { SkeletonRows } from "@/components/Skeleton";
import { closePopup, createTab, listShortcuts, onShortcutsChanged } from "@/lib/browser-client";
import { useSettings } from "@/lib/use-browser";
import { usePopupPainted } from "@/lib/popup-ready";
import { formatKeyToken } from "@/lib/shortcut-keys";
import type { ShortcutBinding } from "@type/browser";

const CATEGORY_ORDER = ["Tabs", "Address bar and bookmarks", "Page", "Workspaces", "Browser"];

type Row = { label: string; keys: string[][] };

function slug(text: string) {
  return text.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}

/** Ctrl+/ overlay: every bound shortcut, grouped. Several actions with one label share a row. */
export function ShortcutSheet() {
  const settings = useSettings();
  const [bindings, setBindings] = useState<ShortcutBinding[] | null>(null);

  usePopupPainted(bindings !== null);

  useEffect(() => {
    void listShortcuts()
      .then(setBindings)
      .catch(() => setBindings([]));
    const unlisten = onShortcutsChanged(setBindings);
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        void closePopup();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const groups = useMemo(() => {
    const byCategory = new Map<string, Row[]>();
    for (const binding of bindings ?? []) {
      if (binding.keys.length === 0) {
        continue;
      }
      const category = binding.category || "Browser";
      if (category === "Workspaces" && settings?.workspacesEnabled === false) {
        continue;
      }
      const rows = byCategory.get(category) ?? [];
      const existing = rows.find((row) => row.label === binding.label);
      if (existing) {
        existing.keys.push(binding.keys);
      } else {
        rows.push({ label: binding.label, keys: [binding.keys] });
      }
      byCategory.set(category, rows);
    }
    const rank = (category: string) => {
      const index = CATEGORY_ORDER.indexOf(category);
      return index < 0 ? CATEGORY_ORDER.length : index;
    };
    return [...byCategory.entries()].sort(([left], [right]) => rank(left) - rank(right));
  }, [bindings, settings?.workspacesEnabled]);

  return (
    <div
      id="shortcut-sheet"
      role="dialog"
      aria-labelledby="shortcut-sheet-title"
      className="flex h-full flex-col overflow-hidden rounded-xl bg-zinc-900 text-zinc-100 ring-1 ring-inset ring-zinc-700"
    >
      <header id="shortcut-sheet-header" className="flex shrink-0 items-center gap-3 border-b border-zinc-800 px-5 py-3">
        <Keyboard id="shortcut-sheet-icon" className="size-5 text-sky-300" aria-hidden />
        <h2 id="shortcut-sheet-title" className="flex-1 text-sm font-semibold">
          Keyboard shortcuts
        </h2>
        <Button id="shortcut-sheet-edit" size="sm" variant="ghost" onClick={() => void createTab("ce://shortcuts").finally(() => closePopup())}>
          <Pencil id="shortcut-sheet-edit-icon" className="size-3.5" aria-hidden />
          Change shortcuts
        </Button>
      </header>
      <div id="shortcut-sheet-body" className="min-h-0 flex-1 overflow-y-auto px-5 py-4">
        {bindings === null ? (
          <SkeletonRows id="shortcut-sheet-loading" rows={8} />
        ) : (
          <div id="shortcut-sheet-columns" className="columns-2 gap-8">
            {groups.map(([category, rows]) => {
              const groupId = `shortcut-sheet-${slug(category)}`;
              return (
                <section id={groupId} key={category} className="mb-5 break-inside-avoid">
                  <h3 id={`${groupId}-title`} className="mb-2 text-[11px] font-medium uppercase tracking-wide text-zinc-500">
                    {category}
                  </h3>
                  <dl id={`${groupId}-list`} className="flex flex-col gap-1.5">
                    {rows.map((row) => {
                      const rowId = `${groupId}-${slug(row.label)}`;
                      return (
                        <div id={rowId} key={row.label} className="flex items-baseline justify-between gap-3 text-sm">
                          <dt id={`${rowId}-label`} className="min-w-0 text-zinc-300">
                            {row.label}
                          </dt>
                          <dd id={`${rowId}-keys`} className="flex shrink-0 flex-wrap justify-end gap-1">
                            {row.keys.map((keys, bindingIndex) => (
                              <span id={`${rowId}-binding-${bindingIndex}`} key={keys.join(" ")} className="flex items-center gap-1">
                                {keys.map((token, index) => (
                                  <span id={`${rowId}-binding-${bindingIndex}-${index}`} key={token} className="flex items-center gap-1">
                                    {index > 0 ? (
                                      <span id={`${rowId}-binding-${bindingIndex}-${index}-then`} className="text-[11px] text-zinc-500">
                                        then
                                      </span>
                                    ) : null}
                                    <kbd
                                      id={`${rowId}-binding-${bindingIndex}-${index}-key`}
                                      className="rounded border border-zinc-700 bg-zinc-800 px-1.5 py-0.5 font-sans text-[11px] text-zinc-200"
                                    >
                                      {formatKeyToken(token)}
                                    </kbd>
                                  </span>
                                ))}
                              </span>
                            ))}
                          </dd>
                        </div>
                      );
                    })}
                  </dl>
                </section>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
