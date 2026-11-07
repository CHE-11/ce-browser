import { memo, useId } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import { createTab } from "@/lib/browser-client";
import { cn } from "@/lib/cn";

type MarkdownProps = {
  id: string;
  text: string;
  className?: string;
};

function components(prefix: string): Components {
  let count = 0;
  const next = (kind: string) => `${prefix}-${kind}-${count++}`;
  return {
    p: ({ children }) => (
      <p id={next("p")} className="my-1.5 first:mt-0 last:mb-0">
        {children}
      </p>
    ),
    a: ({ href, children }) => (
      <a
        id={next("link")}
        href={href}
        className="text-sky-400 underline-offset-2 hover:underline"
        onClick={(event) => {
          event.preventDefault();
          if (href && /^https?:\/\//i.test(href)) {
            void createTab(href);
          }
        }}
      >
        {children}
      </a>
    ),
    ul: ({ children }) => (
      <ul id={next("ul")} className="my-1.5 list-disc space-y-0.5 pl-5">
        {children}
      </ul>
    ),
    ol: ({ children }) => (
      <ol id={next("ol")} className="my-1.5 list-decimal space-y-0.5 pl-5">
        {children}
      </ol>
    ),
    li: ({ children }) => <li id={next("li")}>{children}</li>,
    h1: ({ children }) => (
      <h3 id={next("h")} className="mb-1 mt-2 text-sm font-semibold first:mt-0">
        {children}
      </h3>
    ),
    h2: ({ children }) => (
      <h3 id={next("h")} className="mb-1 mt-2 text-sm font-semibold first:mt-0">
        {children}
      </h3>
    ),
    h3: ({ children }) => (
      <h4 id={next("h")} className="mb-1 mt-2 text-[13px] font-semibold first:mt-0">
        {children}
      </h4>
    ),
    h4: ({ children }) => (
      <h4 id={next("h")} className="mb-1 mt-2 text-xs font-semibold uppercase tracking-wide text-zinc-400 first:mt-0">
        {children}
      </h4>
    ),
    blockquote: ({ children }) => (
      <blockquote id={next("quote")} className="my-1.5 border-l-2 border-zinc-700 pl-2 text-zinc-400">
        {children}
      </blockquote>
    ),
    code: ({ className, children }) => (
      <code id={next("code")} className={cn("rounded bg-zinc-800 px-1 py-px font-mono text-[0.9em] text-zinc-200", className)}>
        {children}
      </code>
    ),
    pre: ({ children }) => (
      <pre id={next("pre")} className="my-1.5 overflow-x-auto rounded-md bg-zinc-950 p-2 text-xs ring-1 ring-zinc-800 [&_code]:bg-transparent [&_code]:p-0">
        {children}
      </pre>
    ),
    table: ({ children }) => (
      <div id={next("table-wrap")} className="my-1.5 overflow-x-auto">
        <table id={next("table")} className="w-full border-collapse text-xs">
          {children}
        </table>
      </div>
    ),
    thead: ({ children }) => <thead id={next("thead")}>{children}</thead>,
    tbody: ({ children }) => <tbody id={next("tbody")}>{children}</tbody>,
    tr: ({ children }) => (
      <tr id={next("tr")} className="border-b border-zinc-800 last:border-0">
        {children}
      </tr>
    ),
    th: ({ children }) => (
      <th id={next("th")} className="px-1.5 py-1 text-left font-semibold text-zinc-300">
        {children}
      </th>
    ),
    td: ({ children }) => (
      <td id={next("td")} className="px-1.5 py-1 align-top">
        {children}
      </td>
    ),
    hr: () => <hr id={next("hr")} className="my-2 border-zinc-800" />,
    img: ({ alt }) => (
      <span id={next("img")} className="text-zinc-500">
        {alt ? `[image: ${alt}]` : "[image]"}
      </span>
    ),
  };
}

/** Markdown from the model, without raw HTML. Links open in a new tab. */
export const Markdown = memo(function Markdown({ id, text, className }: MarkdownProps) {
  const prefix = `${id}-${useId().replace(/:/g, "")}`;
  return (
    <div id={id} className={cn("break-words text-sm leading-relaxed [overflow-wrap:anywhere]", className)}>
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={components(prefix)}>
        {text}
      </ReactMarkdown>
    </div>
  );
});
