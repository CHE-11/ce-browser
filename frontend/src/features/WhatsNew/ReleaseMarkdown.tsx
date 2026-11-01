import type { ReactNode } from "react";
import { createTab } from "@/lib/browser-client";

type Block =
  | { kind: "heading"; level: number; text: string }
  | { kind: "list"; items: string[] }
  | { kind: "paragraph"; text: string };

/** Headings, bullet lists and paragraphs. Release notes use little else. */
function blocks(markdown: string): Block[] {
  const out: Block[] = [];
  let paragraph: string[] = [];
  let list: string[] | null = null;
  const flush = () => {
    if (paragraph.length > 0) {
      out.push({ kind: "paragraph", text: paragraph.join(" ") });
      paragraph = [];
    }
    if (list) {
      out.push({ kind: "list", items: list });
      list = null;
    }
  };
  for (const raw of markdown.replace(/\r\n/g, "\n").split("\n")) {
    const line = raw.trim();
    const heading = /^(#{1,4})\s+(.*)$/.exec(line);
    const item = /^[-*+]\s+(.*)$/.exec(line);
    if (!line) {
      flush();
    } else if (heading) {
      flush();
      out.push({ kind: "heading", level: heading[1].length, text: heading[2] });
    } else if (item) {
      if (paragraph.length > 0) {
        out.push({ kind: "paragraph", text: paragraph.join(" ") });
        paragraph = [];
      }
      list = [...(list ?? []), item[1]];
    } else if (list) {
      list[list.length - 1] += ` ${line}`;
    } else {
      paragraph.push(line);
    }
  }
  flush();
  return out;
}

const INLINE = /(\*\*[^*]+\*\*|`[^`]+`|\[[^\]]+\]\([^)\s]+\)|https?:\/\/[^\s)]+)/g;

function inline(text: string, id: string): ReactNode[] {
  return text.split(INLINE).map((part, index) => {
    const key = `${id}-${index}`;
    if (part.startsWith("**") && part.endsWith("**") && part.length > 4) {
      return (
        <strong id={key} key={key} className="font-semibold text-zinc-100">
          {part.slice(2, -2)}
        </strong>
      );
    }
    if (part.startsWith("`") && part.endsWith("`") && part.length > 2) {
      return (
        <code id={key} key={key} className="rounded bg-zinc-800 px-1 py-0.5 font-mono text-[0.85em] text-zinc-200">
          {part.slice(1, -1)}
        </code>
      );
    }
    const link = /^\[([^\]]+)\]\(([^)\s]+)\)$/.exec(part);
    const url = link ? link[2] : /^https?:\/\//.test(part) ? part : null;
    if (url) {
      return (
        <a
          id={key}
          key={key}
          href={url}
          className="text-sky-400 underline-offset-2 hover:underline"
          onClick={(event) => {
            event.preventDefault();
            void createTab(url);
          }}
        >
          {link ? link[1] : part.replace(/^https?:\/\/(www\.)?github\.com\//, "")}
        </a>
      );
    }
    return part;
  });
}

export function ReleaseMarkdown({ id, markdown }: { id: string; markdown: string }) {
  return (
    <div id={id} className="flex flex-col gap-3 text-sm leading-relaxed text-zinc-300">
      {blocks(markdown).map((block, index) => {
        const key = `${id}-${index}`;
        if (block.kind === "heading") {
          return (
            <h2 id={key} key={key} className={block.level <= 2 ? "pt-2 text-base font-semibold text-zinc-100" : "pt-1 text-sm font-semibold text-zinc-200"}>
              {inline(block.text, key)}
            </h2>
          );
        }
        if (block.kind === "list") {
          return (
            <ul id={key} key={key} className="flex list-disc flex-col gap-1 pl-5 marker:text-zinc-600">
              {block.items.map((item, itemIndex) => (
                <li id={`${key}-${itemIndex}`} key={`${key}-${itemIndex}`}>
                  {inline(item, `${key}-${itemIndex}`)}
                </li>
              ))}
            </ul>
          );
        }
        return (
          <p id={key} key={key}>
            {inline(block.text, key)}
          </p>
        );
      })}
    </div>
  );
}
