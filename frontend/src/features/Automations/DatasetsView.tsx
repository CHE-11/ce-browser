import { useEffect, useState } from "react";
import { Copy, Database, Download, LayoutGrid, Network, Trash2 } from "lucide-react";
import type { Dataset, DatasetFormat, DatasetInfo, ExtractSpec, TabInfo } from "@type/browser";
import { Button } from "@/components/Button";
import { EmptyState } from "@/components/EmptyState";
import { copyDataset, datasetToBoard, deleteDataset, exportDataset, extractCrawl, extractPick, getDataset, renameDataset } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRow } from "@/lib/focus-ring";
import { inputClassName } from "@features/Agent/agent-styles";
import { formatWhen } from "./flow-model";

const PREVIEW_ROWS = 200;
const FORMATS: DatasetFormat[] = ["csv", "tsv", "json"];

type DatasetsViewProps = {
  datasets: DatasetInfo[];
  selectedId: string | null;
  pickTabs: TabInfo[];
  onSelect: (id: string | null) => void;
};

function DatasetTable({ dataset }: { dataset: Dataset }) {
  const columns = dataset.columns.length ? dataset.columns : Object.keys(dataset.data[0] ?? {});
  const rows = dataset.data.slice(0, PREVIEW_ROWS);
  return (
    <div id="dataset-table-wrap" className="min-h-0 flex-1 overflow-auto rounded-md border border-zinc-800">
      <table id="dataset-table" className="w-full border-collapse text-left text-xs">
        <thead id="dataset-table-head" className="sticky top-0 bg-zinc-900">
          <tr id="dataset-table-head-row">
            <th id="dataset-table-head-index" className="w-10 border-b border-zinc-800 px-2 py-1.5 text-zinc-600">
              #
            </th>
            {columns.map((column) => (
              <th id={`dataset-table-head-${column}`} key={column} className="border-b border-zinc-800 px-2 py-1.5 font-medium text-zinc-300">
                {column}
              </th>
            ))}
          </tr>
        </thead>
        <tbody id="dataset-table-body">
          {rows.map((row, index) => (
            <tr id={`dataset-row-${index}`} key={index} className="odd:bg-zinc-900/40">
              <td id={`dataset-row-${index}-index`} className="px-2 py-1 align-top text-zinc-600">
                {index + 1}
              </td>
              {columns.map((column) => (
                <td id={`dataset-row-${index}-${column}`} key={column} className="max-w-80 truncate px-2 py-1 align-top text-zinc-300" title={row[column] ?? ""}>
                  {row[column] ?? ""}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      {dataset.data.length > PREVIEW_ROWS ? (
        <p id="dataset-table-more" className="px-3 py-2 text-xs text-zinc-500">
          Showing the first {PREVIEW_ROWS} of {dataset.data.length} rows. Export to see them all.
        </p>
      ) : null}
    </div>
  );
}

function DatasetDetail({ info, onDeleted }: { info: DatasetInfo; onDeleted: () => void }) {
  const [dataset, setDataset] = useState<Dataset | null>(null);
  const [name, setName] = useState(info.name);
  const [format, setFormat] = useState<DatasetFormat>("csv");
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setName(info.name);
  }, [info.name]);

  useEffect(() => {
    let live = true;
    void getDataset(info.id)
      .then((next) => live && setDataset(next))
      .catch((reason: unknown) => live && setError(String(reason)));
    return () => {
      live = false;
    };
  }, [info.id, info.updated]);

  const act = (work: Promise<unknown>, done: string) => {
    setError(null);
    setMessage(null);
    void work.then(() => setMessage(done)).catch((reason: unknown) => setError(String(reason)));
  };

  return (
    <div id="dataset-detail" className="flex min-h-0 flex-1 flex-col gap-3">
      <div id="dataset-detail-header" className="flex flex-wrap items-center gap-2">
        <input
          id="dataset-detail-name"
          className="min-w-0 flex-1 bg-transparent text-lg font-semibold text-zinc-100 outline-none"
          value={name}
          onChange={(event) => setName(event.target.value)}
          onBlur={() => {
            if (name.trim() && name !== info.name) {
              act(renameDataset(info.id, name.trim()), "Renamed.");
            }
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.currentTarget.blur();
            }
          }}
          aria-label="Dataset name"
        />
        <select id="dataset-detail-format" className={cn(inputClassName, "w-24")} value={format} onChange={(event) => setFormat(event.target.value as DatasetFormat)} aria-label="Format">
          {FORMATS.map((item) => (
            <option id={`dataset-detail-format-${item}`} key={item} value={item}>
              {item.toUpperCase()}
            </option>
          ))}
        </select>
        <Button id="dataset-detail-copy" size="sm" onClick={() => act(copyDataset(info.id, format), `Copied as ${format.toUpperCase()}.`)}>
          <Copy id="dataset-detail-copy-icon" className="size-3.5" aria-hidden />
          Copy
        </Button>
        <Button
          id="dataset-detail-export"
          size="sm"
          onClick={() => {
            setError(null);
            void exportDataset(info.id, format, info.name)
              .then((saved) => saved && setMessage("Exported."))
              .catch((reason: unknown) => setError(String(reason)));
          }}
        >
          <Download id="dataset-detail-export-icon" className="size-3.5" aria-hidden />
          Export
        </Button>
        <Button id="dataset-detail-board" size="sm" variant="ghost" onClick={() => act(datasetToBoard(info.id), "Added to Boards as a CSV pin.")}>
          <LayoutGrid id="dataset-detail-board-icon" className="size-3.5" aria-hidden />
          To board
        </Button>
        <Button
          id="dataset-detail-delete"
          size="sm"
          variant="ghost"
          onClick={() => {
            void deleteDataset(info.id)
              .then(onDeleted)
              .catch((reason: unknown) => setError(String(reason)));
          }}
        >
          <Trash2 id="dataset-detail-delete-icon" className="size-3.5" aria-hidden />
          Delete
        </Button>
      </div>
      <p id="dataset-detail-meta" className="truncate text-xs text-zinc-500">
        {info.rows} rows · {info.columns.length} columns · updated {formatWhen(info.updated)}
        {info.source ? ` · from ${info.source}` : ""}
      </p>
      {error ? (
        <p id="dataset-detail-error" role="alert" className="rounded-md bg-red-950/60 px-3 py-2 text-sm text-red-300">
          {error}
        </p>
      ) : null}
      {message && !error ? (
        <p id="dataset-detail-message" role="status" className="text-xs text-emerald-400">
          {message}
        </p>
      ) : null}
      {dataset ? (
        <DatasetTable dataset={dataset} />
      ) : (
        <p id="dataset-detail-loading" className="text-xs text-zinc-500">
          Loading rows…
        </p>
      )}
    </div>
  );
}

function CrawlForm({ pickTabs, onDone }: { pickTabs: TabInfo[]; onDone: (info: DatasetInfo) => void }) {
  const [urls, setUrls] = useState("");
  const [instruction, setInstruction] = useState("");
  const [name, setName] = useState("");
  const [spec, setSpec] = useState<ExtractSpec | null>(null);
  const [pickTab, setPickTab] = useState<number | null>(pickTabs[0]?.id ?? null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const list = urls
    .split(/\s+/)
    .map((url) => url.trim())
    .filter((url) => /^https?:\/\//i.test(url));

  const pick = () => {
    if (pickTab === null) {
      return;
    }
    setError(null);
    void extractPick(pickTab)
      .then((result) => result && setSpec(result.spec))
      .catch((reason: unknown) => setError(String(reason)));
  };

  const crawl = () => {
    setBusy(true);
    setError(null);
    void extractCrawl(list, spec, instruction.trim(), name.trim())
      .then(onDone)
      .catch((reason: unknown) => setError(String(reason)))
      .finally(() => setBusy(false));
  };

  return (
    <section id="crawl-form" className="flex flex-col gap-2 rounded-md border border-zinc-800 bg-zinc-900/50 p-4">
      <h2 id="crawl-form-title" className="flex items-center gap-2 text-sm font-semibold text-zinc-200">
        <Network id="crawl-form-icon" className="size-4 text-zinc-400" aria-hidden />
        Crawl many pages
      </h2>
      <p id="crawl-form-hint" className="text-xs text-zinc-500">
        Opens up to three hidden tabs, reads every address with the same fields, and adds a url column. Signed-in pages work because the tabs share your profile.
      </p>
      <textarea
        id="crawl-form-urls"
        rows={4}
        className={cn(inputClassName, "font-mono text-xs")}
        value={urls}
        placeholder={"https://example.com/item/1\nhttps://example.com/item/2"}
        onChange={(event) => setUrls(event.target.value)}
        aria-label="Addresses, one per line"
      />
      <div id="crawl-form-spec" className="flex flex-wrap items-center gap-2 text-xs text-zinc-400">
        {spec ? (
          <span id="crawl-form-spec-picked" className="text-emerald-400">
            Fields picked: {spec.fields.map((field) => field.name).join(", ")} on {spec.item}
          </span>
        ) : (
          <span id="crawl-form-spec-none">Pick fields on an open page with the same layout, or describe them for AI.</span>
        )}
        <select
          id="crawl-form-pick-tab"
          className={cn(inputClassName, "w-56")}
          value={pickTab ?? ""}
          onChange={(event) => setPickTab(event.target.value ? Number(event.target.value) : null)}
          aria-label="Page to pick fields on"
        >
          {pickTabs.length === 0 ? (
            <option id="crawl-form-pick-none" value="">
              No open web pages
            </option>
          ) : null}
          {pickTabs.map((tab) => (
            <option id={`crawl-form-pick-${tab.id}`} key={tab.id} value={tab.id}>
              {tab.title || tab.url}
            </option>
          ))}
        </select>
        <Button id="crawl-form-pick" size="xs" variant="outline" disabled={pickTab === null} onClick={pick}>
          Pick fields
        </Button>
        {spec ? (
          <Button id="crawl-form-spec-clear" size="xs" variant="ghost" onClick={() => setSpec(null)}>
            Clear
          </Button>
        ) : null}
      </div>
      {!spec ? (
        <input
          id="crawl-form-instruction"
          className={inputClassName}
          value={instruction}
          placeholder="Product name, price and rating"
          onChange={(event) => setInstruction(event.target.value)}
          aria-label="What to read from each page"
        />
      ) : null}
      <div id="crawl-form-footer" className="flex items-center gap-2">
        <input id="crawl-form-name" className={cn(inputClassName, "flex-1")} value={name} placeholder="Dataset name" onChange={(event) => setName(event.target.value)} aria-label="Dataset name" />
        <Button id="crawl-form-start" size="sm" variant="primary" disabled={busy || list.length === 0 || (!spec && !instruction.trim())} onClick={crawl}>
          {busy ? "Crawling…" : `Crawl ${list.length || ""} page${list.length === 1 ? "" : "s"}`}
        </Button>
      </div>
      {error ? (
        <p id="crawl-form-error" role="alert" className="text-sm text-red-300">
          {error}
        </p>
      ) : null}
    </section>
  );
}

export function DatasetsView({ datasets, selectedId, pickTabs, onSelect }: DatasetsViewProps) {
  const sorted = datasets.slice().sort((a, b) => b.updated - a.updated);
  const selected = sorted.find((item) => item.id === selectedId) ?? null;
  return (
    <section id="datasets-view" className="flex min-w-0 flex-1">
      <div id="datasets-list" className="flex w-64 shrink-0 flex-col gap-1 overflow-y-auto border-r border-zinc-800 p-3">
        <button
          id="datasets-crawl"
          type="button"
          aria-current={selected === null ? "true" : undefined}
          className={cn("h-8 rounded-md px-2 text-left text-sm text-sky-300 hover:bg-zinc-800", focusRow, selected === null && "bg-zinc-800")}
          onClick={() => onSelect(null)}
        >
          New crawl
        </button>
        {sorted.map((item) => (
          <button
            id={`datasets-item-${item.id}`}
            key={item.id}
            type="button"
            aria-current={item.id === selectedId ? "true" : undefined}
            className={cn("flex flex-col rounded-md px-2 py-1.5 text-left hover:bg-zinc-800", focusRow, item.id === selectedId && "bg-zinc-800")}
            onClick={() => onSelect(item.id)}
          >
            <span id={`datasets-item-${item.id}-name`} className="truncate text-sm text-zinc-200">
              {item.name || "Dataset"}
            </span>
            <span id={`datasets-item-${item.id}-meta`} className="truncate text-xs text-zinc-500">
              {item.rows} rows · {formatWhen(item.updated)}
            </span>
          </button>
        ))}
      </div>
      <div id="datasets-main" className="flex min-w-0 flex-1 flex-col gap-4 overflow-y-auto px-6 py-5">
        {selected ? (
          <DatasetDetail key={selected.id} info={selected} onDeleted={() => onSelect(null)} />
        ) : (
          <>
            {sorted.length === 0 ? (
              <EmptyState
                id="datasets-empty"
                icon={Database}
                title="No datasets yet"
                hint="Use the extract button in the address bar to pick a list on any page, ask AI for a table, or pull a site's own API. Flows with extract steps save here too."
              />
            ) : null}
            <CrawlForm pickTabs={pickTabs} onDone={(info) => onSelect(info.id)} />
          </>
        )}
      </div>
    </section>
  );
}
