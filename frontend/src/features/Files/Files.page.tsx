import { useEffect, useMemo, useState } from "react";
import { ChevronLeft, File, Folder, FolderOpen, Search } from "lucide-react";
import { Button } from "@components/Button";
import { PageShell } from "@components/PageShell";
import { browseLocal, chooseFolder, openLocalFile } from "@lib/browser-client";
import { formatBytes } from "@lib/format";
import type { BrowseResult, LocalEntry } from "@type/browser";

type FilesPageProps = {
  params: URLSearchParams;
};

export function FilesPage({ params }: FilesPageProps) {
  const requested = params.get("path") ?? "";
  const [result, setResult] = useState<BrowseResult | null>(null);
  const [error, setError] = useState("");
  const [query, setQuery] = useState("");

  useEffect(() => {
    let gone = false;
    setError("");
    void browseLocal(requested || undefined)
      .then((next) => {
        if (!gone) {
          setResult(next);
        }
      })
      .catch((reason: unknown) => {
        if (!gone) {
          setError(reason instanceof Error ? reason.message : "Couldn't open that folder.");
        }
      });
    return () => {
      gone = true;
    };
  }, [requested]);

  const openPath = (path: string) => {
    window.location.hash = `#/files?path=${encodeURIComponent(path)}`;
  };

  return (
    <PageShell
      id="files-page"
      page="files"
      title="Files"
      actions={
        <label id="files-page-search" className="flex h-8 w-64 items-center gap-2 rounded-md border border-zinc-700 bg-zinc-900 px-2">
          <Search id="files-page-search-icon" className="size-4 text-zinc-500" aria-hidden />
          <input
            id="files-page-search-input"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={result?.type === "file" ? "Search in file" : "Search files"}
            className="min-w-0 flex-1 bg-transparent text-sm outline-none placeholder:text-zinc-500"
          />
        </label>
      }
    >
      <div id="files-page-body" className="mx-auto flex max-w-4xl flex-col gap-4">
        <div id="files-page-toolbar" className="flex items-center gap-2">
          {result?.parent ? (
            <Button id="files-page-up" onClick={() => openPath(result.parent ?? "")}>
              <ChevronLeft id="files-page-up-icon" className="size-4" aria-hidden />
              Up
            </Button>
          ) : null}
          <Button
            id="files-page-open-folder"
            onClick={() =>
              void chooseFolder("Open a folder", result?.path).then((folder) => {
                if (folder) {
                  openPath(folder);
                }
              })
            }
          >
            <FolderOpen id="files-page-open-folder-icon" className="size-4" aria-hidden />
            Open folder
          </Button>
          <p id="files-page-path" className="min-w-0 flex-1 truncate text-xs text-zinc-500">
            {result?.path}
          </p>
        </div>
        {error ? (
          <p id="files-page-error" className="text-sm text-amber-300">
            {error}
          </p>
        ) : null}
        {result?.type === "directory" ? <DirectoryList entries={result.entries} query={query} onOpen={openPath} /> : null}
        {result?.type === "file" ? <FileView result={result} query={query} /> : null}
      </div>
    </PageShell>
  );
}

function DirectoryList({
  entries,
  query,
  onOpen,
}: {
  entries: LocalEntry[];
  query: string;
  onOpen: (path: string) => void;
}) {
  const needle = query.trim().toLowerCase();
  const shown = needle ? entries.filter((entry) => entry.name.toLowerCase().includes(needle)) : entries;
  if (shown.length === 0) {
    return (
      <p id="files-page-empty" className="py-16 text-center text-sm text-zinc-500">
        {needle ? "No files match that search." : "This folder is empty."}
      </p>
    );
  }
  return (
    <ul id="files-page-list" className="flex flex-col gap-1">
      {shown.map((entry) => (
        <li key={entry.path} id={`files-row-${entry.name}`}>
          <button
            id={`files-open-${entry.name}`}
            type="button"
            className="flex w-full items-center gap-3 rounded-md px-3 py-2 text-left hover:bg-zinc-900"
            onClick={() => onOpen(entry.path)}
          >
            {entry.directory ? (
              <Folder id={`files-icon-${entry.name}`} className="size-4 shrink-0 text-sky-400" aria-hidden />
            ) : (
              <File id={`files-icon-${entry.name}`} className="size-4 shrink-0 text-zinc-400" aria-hidden />
            )}
            <span id={`files-name-${entry.name}`} className="min-w-0 flex-1 truncate text-sm">
              {entry.name}
            </span>
            <span id={`files-meta-${entry.name}`} className="shrink-0 text-xs text-zinc-500">
              {entry.directory ? "Folder" : formatBytes(entry.size)}
            </span>
          </button>
        </li>
      ))}
    </ul>
  );
}

function FileView({ result, query }: { result: Extract<BrowseResult, { type: "file" }>; query: string }) {
  if (result.kind === "other") {
    return (
      <div id="files-preview-other" className="flex flex-col items-start gap-3">
        <p id="files-preview-other-text" className="text-sm text-zinc-400">
          {result.name} opens in its usual app.
        </p>
        <Button id="files-preview-open" variant="primary" onClick={() => void openLocalFile(result.path)}>
          Open
        </Button>
      </div>
    );
  }
  if (result.tooBig) {
    return (
      <div id="files-preview-large" className="flex flex-col items-start gap-3">
        <p id="files-preview-large-text" className="text-sm text-zinc-400">
          {result.name} is {formatBytes(result.size)}. Previews stop at 2 MB.
        </p>
        <Button id="files-preview-large-open" variant="primary" onClick={() => void openLocalFile(result.path)}>
          Open
        </Button>
      </div>
    );
  }
  return (
    <div id="files-preview" className="flex flex-col gap-3">
      <div id="files-preview-actions" className="flex justify-end">
        <Button id="files-preview-open" onClick={() => void openLocalFile(result.path)}>
          Open
        </Button>
      </div>
      {result.kind === "json" ? <JsonView text={result.text} query={query} /> : null}
      {result.kind === "csv" ? <CsvView text={result.text} query={query} /> : null}
      {result.kind === "markdown" ? <MarkdownView text={result.text} query={query} /> : null}
      {result.kind === "log" ? <LogView text={result.text} query={query} /> : null}
    </div>
  );
}

function JsonView({ text, query }: { text: string; query: string }) {
  const pretty = useMemo(() => {
    try {
      return JSON.stringify(JSON.parse(text), null, 2);
    } catch {
      return text;
    }
  }, [text]);
  const lines = pretty.split("\n").filter((line) => matches(line, query) || !query.trim());
  return (
    <pre id="files-json" className="overflow-auto rounded-lg bg-zinc-900 p-4 text-xs leading-5 text-zinc-200">
      {lines.map((line, index) => (
        <div id={`files-json-line-${index}`} key={index}>
          {highlight(line, query)}
        </div>
      ))}
    </pre>
  );
}

function CsvView({ text, query }: { text: string; query: string }) {
  const rows = useMemo(() => parseCsv(text), [text]);
  const needle = query.trim().toLowerCase();
  const body = needle ? rows.slice(1).filter((row) => row.some((cell) => cell.toLowerCase().includes(needle))) : rows.slice(1);
  const header = rows[0] ?? [];
  return (
    <div id="files-csv" className="overflow-auto rounded-lg border border-zinc-800">
      <table id="files-csv-table" className="w-full border-collapse text-left text-sm">
        <thead id="files-csv-head">
          <tr id="files-csv-header-row">
            {header.map((cell, index) => (
              <th id={`files-csv-header-${index}`} key={index} className="border-b border-zinc-800 px-3 py-2 font-medium text-zinc-400">
                {highlight(cell, query)}
              </th>
            ))}
          </tr>
        </thead>
        <tbody id="files-csv-body">
          {body.map((row, rowIndex) => (
            <tr id={`files-csv-row-${rowIndex}`} key={rowIndex} className="odd:bg-zinc-900/60">
              {row.map((cell, cellIndex) => (
                <td id={`files-csv-cell-${rowIndex}-${cellIndex}`} key={cellIndex} className="px-3 py-1.5 text-zinc-200">
                  {highlight(cell, query)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function MarkdownView({ text, query }: { text: string; query: string }) {
  const blocks = useMemo(() => markdownBlocks(text), [text]);
  const needle = query.trim().toLowerCase();
  const shown = needle ? blocks.filter((block) => block.text.toLowerCase().includes(needle)) : blocks;
  return (
    <div id="files-markdown" className="flex flex-col gap-3 text-sm leading-6 text-zinc-200">
      {shown.map((block, index) => {
        if (block.kind === "h1") {
          return (
            <h2 id={`files-md-${index}`} key={index} className="text-xl font-semibold">
              {highlight(block.text, query)}
            </h2>
          );
        }
        if (block.kind === "h2") {
          return (
            <h3 id={`files-md-${index}`} key={index} className="text-lg font-semibold">
              {highlight(block.text, query)}
            </h3>
          );
        }
        if (block.kind === "h3") {
          return (
            <h4 id={`files-md-${index}`} key={index} className="text-base font-semibold">
              {highlight(block.text, query)}
            </h4>
          );
        }
        if (block.kind === "code") {
          return (
            <pre id={`files-md-${index}`} key={index} className="overflow-auto rounded-lg bg-zinc-900 p-3 text-xs">
              {highlight(block.text, query)}
            </pre>
          );
        }
        if (block.kind === "item") {
          return (
            <p id={`files-md-${index}`} key={index} className="pl-4">
              • {highlight(block.text, query)}
            </p>
          );
        }
        return (
          <p id={`files-md-${index}`} key={index}>
            {linkify(block.text, query, index)}
          </p>
        );
      })}
    </div>
  );
}

function LogView({ text, query }: { text: string; query: string }) {
  const lines = text.split(/\r?\n/).filter((line) => matches(line, query) || !query.trim());
  return (
    <pre id="files-log" className="overflow-auto rounded-lg bg-zinc-900 p-4 font-mono text-xs leading-5 text-zinc-200">
      {lines.map((line, index) => (
        <div id={`files-log-line-${index}`} key={index}>
          {highlight(line, query)}
        </div>
      ))}
    </pre>
  );
}

function matches(text: string, query: string) {
  const needle = query.trim().toLowerCase();
  return !needle || text.toLowerCase().includes(needle);
}

function highlight(text: string, query: string) {
  const needle = query.trim();
  if (!needle) {
    return text;
  }
  const pattern = new RegExp(`(${needle.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")})`, "ig");
  const parts = text.split(pattern);
  return parts.map((part, index) =>
    part.toLowerCase() === needle.toLowerCase() ? (
      <mark id={`files-hit-${index}-${part.slice(0, 8)}`} key={index} className="rounded bg-amber-400/40 text-zinc-50">
        {part}
      </mark>
    ) : (
      <span id={`files-text-${index}`} key={index}>
        {part}
      </span>
    ),
  );
}

function linkify(text: string, query: string, block: number) {
  const pattern = /(!?)\[([^\]]+)\]\(([^)]+)\)/g;
  const nodes = [];
  let last = 0;
  let match = pattern.exec(text);
  let index = 0;
  while (match) {
    if (match.index > last) {
      nodes.push(<span key={`t-${index}`}>{highlight(text.slice(last, match.index), query)}</span>);
    }
    const label = match[1] === "!" ? match[2] : match[2];
    nodes.push(
      <a id={`files-md-link-${block}-${index}`} key={`a-${index}`} href={match[3]} className="text-sky-400 underline" target="_blank" rel="noreferrer">
        {highlight(label, query)}
      </a>,
    );
    last = match.index + match[0].length;
    index += 1;
    match = pattern.exec(text);
  }
  if (last < text.length) {
    nodes.push(<span key="tail">{highlight(text.slice(last), query)}</span>);
  }
  return nodes;
}

function markdownBlocks(text: string) {
  const blocks: { kind: "h1" | "h2" | "h3" | "item" | "code" | "p"; text: string }[] = [];
  const lines = text.split(/\r?\n/);
  let code = false;
  let buffer: string[] = [];
  const flush = () => {
    if (buffer.length > 0) {
      blocks.push({ kind: "code", text: buffer.join("\n") });
      buffer = [];
    }
  };
  for (const line of lines) {
    if (line.startsWith("```")) {
      if (code) {
        flush();
      }
      code = !code;
      continue;
    }
    if (code) {
      buffer.push(line);
      continue;
    }
    if (line.startsWith("### ")) {
      blocks.push({ kind: "h3", text: line.slice(4) });
    } else if (line.startsWith("## ")) {
      blocks.push({ kind: "h2", text: line.slice(3) });
    } else if (line.startsWith("# ")) {
      blocks.push({ kind: "h1", text: line.slice(2) });
    } else if (line.startsWith("- ") || line.startsWith("* ")) {
      blocks.push({ kind: "item", text: line.slice(2) });
    } else if (line.trim()) {
      blocks.push({ kind: "p", text: line });
    }
  }
  if (code) {
    flush();
  }
  return blocks;
}

function parseCsv(text: string) {
  const rows: string[][] = [];
  let row: string[] = [];
  let cell = "";
  let quoted = false;
  for (let index = 0; index < text.length; index += 1) {
    const character = text[index];
    if (quoted) {
      if (character === '"' && text[index + 1] === '"') {
        cell += '"';
        index += 1;
      } else if (character === '"') {
        quoted = false;
      } else {
        cell += character;
      }
      continue;
    }
    if (character === '"') {
      quoted = true;
    } else if (character === ",") {
      row.push(cell);
      cell = "";
    } else if (character === "\n") {
      row.push(cell);
      rows.push(row);
      row = [];
      cell = "";
    } else if (character !== "\r") {
      cell += character;
    }
  }
  if (cell.length > 0 || row.length > 0) {
    row.push(cell);
    rows.push(row);
  }
  return rows;
}
