import { useState } from "react";
import { Braces, Database, MousePointerClick, Radar, Sparkles, TableProperties } from "lucide-react";
import type { DatasetInfo, ExtractOptions, SniffedEndpoint } from "@type/browser";
import { Button } from "@/components/Button";
import { PopupFrame } from "@/components/PopupFrame";
import { closePopup, extractAi, extractEndpoint, extractPickSave, extractSniff, openAutomations } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { ActionTile, PopoverHeader } from "../PopoverParts";
import { readPopoverTab } from "../shared";

type ExtractPopoverProps = {
  params: URLSearchParams;
};

const fieldClass =
  "w-full rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-sm text-zinc-100 outline-none placeholder:text-zinc-600 focus:border-sky-500";

function endpointLabel(url: string) {
  try {
    const parsed = new URL(url);
    return `${parsed.host}${parsed.pathname}`;
  } catch {
    return url;
  }
}

export function ExtractPopover({ params }: ExtractPopoverProps) {
  const tab = readPopoverTab(params);
  const [name, setName] = useState("");
  const [pages, setPages] = useState(1);
  const [nextText, setNextText] = useState("");
  const [instruction, setInstruction] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [endpoints, setEndpoints] = useState<SniffedEndpoint[] | null>(null);

  const options: ExtractOptions = { name: name.trim(), pages: Math.max(1, Math.min(100, pages || 1)), nextText: nextText.trim() };

  const show = (info: DatasetInfo) => {
    void openAutomations(`dataset=${info.id}`);
    void closePopup();
  };

  const work = (label: string, task: () => Promise<DatasetInfo>) => {
    setBusy(label);
    setError(null);
    void task()
      .then(show)
      .catch((reason: unknown) => setError(String(reason)))
      .finally(() => setBusy(null));
  };

  const pick = () => {
    void extractPickSave(tab.id, options).catch(() => undefined);
    void closePopup();
  };

  const sniff = () => {
    setBusy("sniff");
    setError(null);
    void extractSniff(tab.id)
      .then(setEndpoints)
      .catch((reason: unknown) => setError(String(reason)))
      .finally(() => setBusy(null));
  };

  return (
    <PopupFrame id="extract-popover" width={340} maxHeight={600}>
      <PopoverHeader id="extract-header" icon={<TableProperties aria-hidden />} title="Extract data" subtitle={tab.title || tab.url} />
      <div id="extract-form" className="flex flex-col gap-2 px-3 pb-3">
        <div id="extract-options" className="grid grid-cols-[1fr_5rem] gap-2">
          <input id="extract-name" className={fieldClass} value={name} placeholder="Dataset name (optional)" onChange={(event) => setName(event.target.value)} aria-label="Dataset name" />
          <input
            id="extract-pages"
            type="number"
            min={1}
            max={100}
            className={fieldClass}
            value={pages}
            onChange={(event) => setPages(Number(event.target.value))}
            title="Pages to follow through the Next link"
            aria-label="Pages"
          />
        </div>
        {pages > 1 ? (
          <input
            id="extract-next"
            className={fieldClass}
            value={nextText}
            placeholder="Next link text (blank finds it)"
            onChange={(event) => setNextText(event.target.value)}
            aria-label="Next link text"
          />
        ) : null}

        <ActionTile
          id="extract-pick"
          primary
          icon={<MousePointerClick aria-hidden />}
          label="Pick a list on the page"
          hint="Click one item. Items like it become rows, and their text, links and images become columns."
          disabled={busy !== null}
          onClick={pick}
        />

        <div id="extract-ai" className="flex flex-col gap-1.5 rounded-lg bg-zinc-800/50 p-2">
          <label id="extract-ai-label" htmlFor="extract-ai-input" className="flex items-center gap-1.5 text-xs font-medium text-zinc-300">
            <Sparkles id="extract-ai-icon" className="size-3.5 text-sky-300" aria-hidden />
            Ask AI for a table
          </label>
          <textarea
            id="extract-ai-input"
            rows={2}
            className={cn(fieldClass, "resize-none")}
            value={instruction}
            placeholder="Every job with title, company, salary and link"
            onChange={(event) => setInstruction(event.target.value)}
          />
          <Button
            id="extract-ai-run"
            size="sm"
            disabled={busy !== null || !instruction.trim()}
            onClick={() => work("ai", () => extractAi(tab.id, instruction.trim(), options))}
          >
            {busy === "ai" ? "Reading the page…" : "Extract with AI"}
          </Button>
        </div>

        <div id="extract-api" className="flex flex-col gap-1.5 rounded-lg bg-zinc-800/50 p-2">
          <div id="extract-api-heading" className="flex items-center justify-between gap-2">
            <span id="extract-api-label" className="flex items-center gap-1.5 text-xs font-medium text-zinc-300">
              <Radar id="extract-api-icon" className="size-3.5 text-sky-300" aria-hidden />
              Read the site's own data
            </span>
            <Button id="extract-api-find" size="xs" variant="ghost" disabled={busy !== null} onClick={sniff}>
              {busy === "sniff" ? "Looking…" : endpoints ? "Look again" : "Find requests"}
            </Button>
          </div>
          {endpoints === null ? (
            <p id="extract-api-hint" className="text-xs text-zinc-500">
              Lists the JSON requests this page made. Pick one to save its records, signed in as you.
            </p>
          ) : endpoints.length === 0 ? (
            <p id="extract-api-none" className="text-xs text-zinc-500">
              No data requests yet. Scroll or search on the page, then look again.
            </p>
          ) : (
            <ul id="extract-api-list" className="flex max-h-44 flex-col gap-0.5 overflow-y-auto">
              {endpoints.map((endpoint, index) => (
                <li id={`extract-api-item-${index}`} key={endpoint.url}>
                  <button
                    id={`extract-api-item-${index}-button`}
                    type="button"
                    disabled={busy !== null}
                    title={endpoint.url}
                    className="flex w-full items-center gap-2 rounded px-1.5 py-1 text-left text-xs text-zinc-300 hover:bg-zinc-700 disabled:opacity-50"
                    onClick={() => work(`endpoint-${index}`, () => extractEndpoint(tab.id, endpoint.url, name.trim()))}
                  >
                    <Braces id={`extract-api-item-${index}-icon`} className="size-3.5 shrink-0 text-zinc-500" aria-hidden />
                    <span id={`extract-api-item-${index}-url`} className="min-w-0 flex-1 truncate">
                      {busy === `endpoint-${index}` ? "Reading…" : endpointLabel(endpoint.url)}
                    </span>
                    <span id={`extract-api-item-${index}-kind`} className="text-[10px] uppercase text-zinc-500">
                      {endpoint.kind === "xmlhttprequest" ? "xhr" : endpoint.kind}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>

        {error ? (
          <p id="extract-error" role="alert" className="rounded-md bg-amber-500/10 px-2 py-1.5 text-xs text-amber-200">
            {error}
          </p>
        ) : null}

        <Button
          id="extract-open-datasets"
          size="sm"
          variant="ghost"
          onClick={() => {
            void openAutomations("tab=datasets");
            void closePopup();
          }}
        >
          <Database id="extract-open-datasets-icon" className="size-3.5" aria-hidden />
          Datasets and crawls
        </Button>
      </div>
    </PopupFrame>
  );
}
