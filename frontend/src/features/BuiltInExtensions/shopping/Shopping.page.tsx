import { useEffect, useMemo, useRef, useState } from "react";
import { Check, LayoutGrid, ShoppingCart, Table2, Trash } from "lucide-react";
import { Button } from "@/components/Button";
import { Dialog } from "@/components/Dialog";
import { EmptyState } from "@/components/EmptyState";
import { SkeletonRows } from "@/components/Skeleton";
import { cn } from "@/lib/cn";
import {
  addPriceUrl,
  checkPrice,
  checkPrices,
  createTab,
  findDealsForProduct,
  listPrices,
  onPriceCheckProgress,
  onPricesChanged,
  previewPriceUrl,
  untrackPrice,
  updateTrackedPrice,
} from "@/lib/browser-client";
import { useAppearanceFromSettings } from "@/lib/use-browser";
import type { PriceCheckProgress, ProductPrice, ShoppingDeals } from "@type/browser";
import { errorText } from "@features/BuiltInExtensions/use-page-tool";
import { formatPrice, matchesFilter, matchesQuery, sortItems, summaryOf, type ShoppingFilter, type ShoppingSort } from "./format";
import { DealList } from "./DealList";
import { ProductDetail } from "./ProductDetail";
import { TargetDialog } from "./TargetDialog";
import { TrackedGrid } from "./TrackedGrid";
import { TrackedTable } from "./TrackedTable";

function isTyping(target: EventTarget | null) {
  return target instanceof HTMLElement && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.tagName === "SELECT" || target.isContentEditable);
}

export function ShoppingPage({ params }: { params: URLSearchParams }) {
  useAppearanceFromSettings();
  const [items, setItems] = useState<ProductPrice[] | null>(null);
  const [error, setError] = useState("");
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<ShoppingFilter>("all");
  const [sort, setSort] = useState<ShoppingSort>("drop");
  const [view, setView] = useState<"cards" | "table">("table");
  const [selectedId, setSelectedId] = useState<string | null>(params.get("product"));
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [targetItem, setTargetItem] = useState<ProductPrice | null>(null);
  const [noteItem, setNoteItem] = useState<ProductPrice | null>(null);
  const [note, setNote] = useState("");
  const [tags, setTags] = useState("");
  const [confirmIds, setConfirmIds] = useState<string[] | null>(null);
  const [url, setUrl] = useState("");
  const [preview, setPreview] = useState<ProductPrice | null>(null);
  const [progress, setProgress] = useState<PriceCheckProgress | null>(null);
  const [deals, setDeals] = useState<ShoppingDeals | null>(null);
  const [dealsBusy, setDealsBusy] = useState(false);
  const [compareItem, setCompareItem] = useState<ProductPrice | null>(null);
  const [compareDeals, setCompareDeals] = useState<ShoppingDeals | null>(null);
  const [compareBusy, setCompareBusy] = useState(false);
  const [compareError, setCompareError] = useState("");
  const compareRequest = useRef("");

  useEffect(() => {
    document.title = "Shopping";
  }, []);

  useEffect(() => {
    void listPrices()
      .then(setItems)
      .catch((reason: unknown) => {
        setItems([]);
        setError(errorText(reason));
      });
    const prices = onPricesChanged((next) => {
      setItems(next);
      setError("");
    });
    const checks = onPriceCheckProgress(setProgress);
    return () => {
      void prices.then((unlisten) => unlisten());
      void checks.then((unlisten) => unlisten());
    };
  }, []);

  const visible = useMemo(() => {
    const list = items ?? [];
    return sortItems(list.filter((item) => matchesFilter(item, filter) && matchesQuery(item, query)), sort);
  }, [items, filter, query, sort]);

  const selected = items?.find((item) => item.id === selectedId) ?? null;
  const summary = summaryOf(items ?? []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (isTyping(event.target)) {
        return;
      }
      if (event.key === "Escape") {
        setPicked(new Set());
        setConfirmIds(null);
      }
      if (event.key === "Delete" && picked.size > 0) {
        setConfirmIds([...picked]);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [picked]);

  const run = (work: Promise<ProductPrice[]>) => {
    void work.then(setItems).catch((reason: unknown) => setError(errorText(reason)));
  };

  const findCheaper = (item: ProductPrice, force = false) => {
    compareRequest.current = item.id;
    setCompareItem(item);
    setCompareError("");
    if (!force) {
      setCompareDeals(null);
    }
    setCompareBusy(true);
    void findDealsForProduct(item.url, item.title, force)
      .then((next) => {
        if (compareRequest.current !== item.id) {
          return;
        }
        setCompareDeals(next);
      })
      .catch((reason: unknown) => {
        if (compareRequest.current === item.id) {
          setCompareError(errorText(reason));
        }
      })
      .finally(() => {
        if (compareRequest.current === item.id) {
          setCompareBusy(false);
        }
      });
  };

  const searchDeals = (force: boolean) => {
    if (!selected) {
      return;
    }
    setDealsBusy(true);
    void findDealsForProduct(selected.url, selected.title, force)
      .then(setDeals)
      .catch((reason: unknown) => setError(errorText(reason)))
      .finally(() => setDealsBusy(false));
  };

  return (
    <main id="shopping-page" className="flex h-full min-h-0 flex-col bg-zinc-950 text-zinc-100">
      <header id="shopping-header" className="flex flex-wrap items-end justify-between gap-3 border-b border-zinc-800 px-6 py-4">
        <div id="shopping-heading">
          <h1 id="shopping-title" className="text-lg font-semibold">Shopping</h1>
          <p id="shopping-summary" className="text-sm text-zinc-400">
            {summary.count} tracked · {summary.dropped} dropped · {summary.targets} at target · {formatPrice(summary.saving, items?.[0]?.currency || "USD")} saved vs first price
          </p>
        </div>
        <div id="shopping-header-actions" className="flex flex-wrap items-center gap-2">
          <Button id="shopping-check-all" size="sm" onClick={() => run(checkPrices())}>
            {progress && !progress.done ? `Checking ${progress.checked} of ${progress.total}` : "Check all"}
          </Button>
          <div id="shopping-views" className="flex items-center rounded-md border border-zinc-800 p-0.5">
            {(
              [
                ["table", Table2, "Table"],
                ["cards", LayoutGrid, "Cards"],
              ] as const
            ).map(([id, Icon, label]) => (
              <button
                key={id}
                id={`shopping-view-${id}`}
                type="button"
                title={label}
                aria-label={label}
                aria-pressed={view === id}
                className={cn(
                  "inline-flex size-6 items-center justify-center rounded text-zinc-400 hover:text-zinc-100",
                  view === id && "bg-zinc-800 text-zinc-100",
                )}
                onClick={() => setView(id)}
              >
                <Icon id={`shopping-view-${id}-icon`} className="size-3.5" aria-hidden />
              </button>
            ))}
          </div>
        </div>
      </header>
      {error ? (
        <p id="shopping-error" className="border-b border-amber-900 bg-amber-950/40 px-6 py-2 text-sm text-amber-200">
          {error}
        </p>
      ) : null}
      <div id="shopping-toolbar" className="flex flex-wrap items-center justify-between gap-2 border-b border-zinc-800 px-6 py-3">
        <div id="shopping-toolbar-left" className="flex items-center gap-2">
          <input
            id="shopping-search"
            className="h-8 w-56 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm"
            placeholder="Search title, store, note, tag"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
          />
          <select id="shopping-filter" className="h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm" value={filter} onChange={(event) => setFilter(event.target.value as ShoppingFilter)}>
            <option id="shopping-filter-all" value="all">All</option>
            <option id="shopping-filter-dropped" value="dropped">Dropped</option>
            <option id="shopping-filter-target" value="target">Below target</option>
            <option id="shopping-filter-stock" value="stock">In stock</option>
            <option id="shopping-filter-error" value="error">Has error</option>
            <option id="shopping-filter-paused" value="paused">Paused</option>
          </select>
          <select id="shopping-sort" className="h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm" value={sort} onChange={(event) => setSort(event.target.value as ShoppingSort)}>
            <option id="shopping-sort-drop" value="drop">Biggest drop</option>
            <option id="shopping-sort-percent" value="percent">Biggest percent</option>
            <option id="shopping-sort-price" value="price">Lowest price</option>
            <option id="shopping-sort-checked" value="checked">Recently checked</option>
          </select>
          <form
            id="shopping-add-form"
            className="flex gap-2"
            onSubmit={(event) => {
              event.preventDefault();
              void previewPriceUrl(url)
                .then(setPreview)
                .catch((reason: unknown) => setError(errorText(reason)));
            }}
          >
            <input
              id="shopping-add-url"
              className="h-8 w-64 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm"
              placeholder="Add by product URL"
              value={url}
              onChange={(event) => setUrl(event.target.value)}
            />
            <Button id="shopping-add-preview" size="sm" type="submit">
              Preview
            </Button>
          </form>
        </div>
        <div id="shopping-toolbar-right" className="flex items-center gap-2">
          {view === "table" && picked.size > 0 ? (
            <>
            <Button id="shopping-check-selected" size="sm" variant="primary" onClick={() => run(checkPrices([...picked]))}>
              <Check id="shopping-check-selected-icon" aria-hidden className="size-3.5" />
              Check {picked.size}
            </Button>
            <Button id="shopping-remove-selected" size="sm" variant="destructive" onClick={() => setConfirmIds([...picked])}>
              <Trash id="shopping-remove-selected-icon" aria-hidden className="size-3.5" />
              Remove {picked.size}
            </Button>
            </>
          ) : null}
        </div>

      </div>
      <div id="shopping-content" className="flex min-h-0 flex-1 gap-4 overflow-auto px-6 py-4">
        <div id="shopping-list" className="min-w-0 flex-1">
          {items === null ? (
            <SkeletonRows id="shopping-loading" rows={6} />
          ) : visible.length === 0 ? (
            <EmptyState
              id="shopping-empty"
              icon={ShoppingCart}
              title={items.length === 0 ? "No tracked products" : "Nothing matches"}
              hint={items.length === 0 ? "Track a product from its page, or paste a product URL above." : "Try another search or filter."}
            />
          ) : view === "cards" ? (
            <TrackedGrid items={visible} selected={selectedId} onSelect={setSelectedId} />
          ) : (
            <TrackedTable
              items={visible}
              picked={picked}
              onToggle={(id) => {
                const next = new Set(picked);
                if (next.has(id)) {
                  next.delete(id);
                } else {
                  next.add(id);
                }
                setPicked(next);
              }}
              onToggleAll={(select) => {
                const next = new Set(picked);
                for (const item of visible) {
                  if (select) next.add(item.id);
                  else next.delete(item.id);
                }
                setPicked(next);
              }}
              onOpen={(item) => void createTab(item.url)}
              onCheck={(item) => run(checkPrice(item.id))}
              onFindCheaper={(item) => findCheaper(item)}
              onPause={(item) => run(updateTrackedPrice({ id: item.id, paused: !item.paused }))}
              onNote={(item) => {
                setNoteItem(item);
                setNote(item.note);
                setTags(item.tags.join(", "));
              }}
              onRemove={(item) => setConfirmIds([item.id])}
            />
          )}
        </div>
        {view === "cards" && selected ? (
          <ProductDetail
            item={selected}
            deals={deals && deals.key ? deals : null}
            busy={dealsBusy}
            onSearch={searchDeals}
            onFeedback={setDeals}
            onCheck={() => run(checkPrice(selected.id))}
            onTarget={() => setTargetItem(selected)}
            onPause={() => run(updateTrackedPrice({ id: selected.id, paused: !selected.paused }))}
            onNote={() => {
              setNoteItem(selected);
              setNote(selected.note);
              setTags(selected.tags.join(", "));
            }}
            onRemove={() => setConfirmIds([selected.id])}
            onTrackOffer={(offerUrl) => run(addPriceUrl(offerUrl))}
          />
        ) : null}
      </div>
      <Dialog
        id="shopping-compare-dialog"
        title="Cheaper at other stores"
        open={compareItem !== null}
        onClose={() => {
          compareRequest.current = "";
          setCompareItem(null);
        }}
      >
        {compareItem ? (
          <div id="shopping-compare" className="flex flex-col gap-3">
            <div id="shopping-compare-heading">
              <p id="shopping-compare-product" className="text-sm">{compareItem.title || compareItem.url}</p>
              <p id="shopping-compare-price" className="text-sm text-zinc-400">
                {formatPrice(compareItem.price, compareItem.currency)} at {compareItem.store || "this store"}
              </p>
            </div>
            {compareError ? (
              <p id="shopping-compare-error" className="text-sm text-amber-300">{compareError}</p>
            ) : null}
            <DealList
              key={compareItem.id}
              id="shopping-compare-deals"
              deals={compareDeals}
              busy={compareBusy}
              pricesOnly
              onSearch={(force) => findCheaper(compareItem, force)}
              onFeedback={setCompareDeals}
              onTrack={(offerUrl) => run(addPriceUrl(offerUrl))}
            />
          </div>
        ) : null}
      </Dialog>
      <TargetDialog
        key={targetItem?.id ?? "shopping-target-closed"}
        item={targetItem}
        onClose={() => setTargetItem(null)}
        onSave={(item, target, alerts) => {
          run(updateTrackedPrice({
            id: item.id,
            clearTarget: target === null,
            targetPrice: target ?? undefined,
            ...alerts,
          }));
          setTargetItem(null);
        }}
      />
      <Dialog id="shopping-note-dialog" title="Note and tags" open={noteItem !== null} onClose={() => setNoteItem(null)}>
        {noteItem ? (
          <form
            id="shopping-note-form"
            className="flex flex-col gap-3"
            onSubmit={(event) => {
              event.preventDefault();
              run(updateTrackedPrice({
                id: noteItem.id,
                note,
                tags: tags.split(",").map((tag) => tag.trim()).filter(Boolean),
              }));
              setNoteItem(null);
            }}
          >
            <textarea id="shopping-note-input" className="min-h-20 rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1 text-sm" value={note} onChange={(event) => setNote(event.target.value)} />
            <input id="shopping-tags-input" className="h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm" value={tags} placeholder="tags, separated by commas" onChange={(event) => setTags(event.target.value)} />
            <div id="shopping-note-actions" className="flex justify-end gap-2">
              <Button id="shopping-note-cancel" variant="ghost" onClick={() => setNoteItem(null)}>Cancel</Button>
              <Button id="shopping-note-save" type="submit" variant="primary">Save</Button>
            </div>
          </form>
        ) : null}
      </Dialog>
      <Dialog id="shopping-preview-dialog" title="Track this product?" open={preview !== null} onClose={() => setPreview(null)}>
        {preview ? (
          <div id="shopping-preview-body" className="flex flex-col gap-3">
            <p id="shopping-preview-title" className="text-sm">{preview.title || preview.url}</p>
            <p id="shopping-preview-price" className="text-sm text-zinc-300">{formatPrice(preview.price, preview.currency)} · {preview.store}</p>
            <div id="shopping-preview-actions" className="flex justify-end gap-2">
              <Button id="shopping-preview-cancel" variant="ghost" onClick={() => setPreview(null)}>Cancel</Button>
              <Button
                id="shopping-preview-track"
                variant="primary"
                onClick={() => {
                  run(addPriceUrl(preview.url || url));
                  setPreview(null);
                  setUrl("");
                }}
              >
                Track
              </Button>
            </div>
          </div>
        ) : null}
      </Dialog>
      <Dialog id="shopping-remove-dialog" title="Remove tracked products?" open={confirmIds !== null} onClose={() => setConfirmIds(null)}>
        <div id="shopping-remove-body" className="flex flex-col gap-3">
          <p id="shopping-remove-text" className="text-sm text-zinc-300">
            {confirmIds?.length === 1 ? "This product will stop being checked." : `${confirmIds?.length ?? 0} products will stop being checked.`}
          </p>
          <div id="shopping-remove-actions" className="flex justify-end gap-2">
            <Button id="shopping-remove-cancel" variant="ghost" onClick={() => setConfirmIds(null)}>Cancel</Button>
            <Button
              id="shopping-remove-confirm"
              variant="destructive"
              onClick={() => {
                const ids = confirmIds ?? [];
                setConfirmIds(null);
                setPicked(new Set());
                void (async () => {
                  let latest = items ?? [];
                  for (const id of ids) {
                    latest = await untrackPrice(id);
                  }
                  setItems(latest);
                  if (selectedId && ids.includes(selectedId)) {
                    setSelectedId(null);
                  }
                })().catch((reason: unknown) => setError(errorText(reason)));
              }}
            >
              Remove
            </Button>
          </div>
        </div>
      </Dialog>
    </main>
  );
}
