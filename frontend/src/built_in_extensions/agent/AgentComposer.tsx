import { useRef, useState, type KeyboardEvent } from "react";
import { AtSign, ChevronRight, SendHorizontal, Slash } from "lucide-react";
import type { TabInfo } from "@type/browser";
import { FavIcon } from "@/components/FavIcon";
import { IconButton } from "@/components/IconButton";
import { commandMatches, type AgentCommand, type ComposerMode } from "@/lib/agent-commands";
import { cn } from "@/lib/cn";
import { inputClassName } from "./agent-styles";

/** Unsent text per run, kept while the panel is open. */
const drafts = new Map<string, string>();

const PLACEHOLDERS: Record<ComposerMode, string> = {
  start: "What should the agent do? @ adds a tab, / shows commands",
  reply: "Tell the agent something…",
  followup: "Ask a follow-up…",
};

type Props = {
  mode: ComposerMode;
  draftKey: string;
  disabled?: boolean;
  suggestions?: string[];
  /** Tabs that `@` can point the agent at. */
  tabs?: TabInfo[];
  /** Earlier tasks, newest first, for the up arrow. */
  history?: string[];
  onSubmit: (text: string, contextTabs: number[]) => void;
};

type Mention = { id: number; token: string };

type Menu =
  | { kind: "command"; items: AgentCommand[] }
  | { kind: "mention"; items: TabInfo[]; at: number; caret: number };

function mentionToken(tab: TabInfo) {
  const title = (tab.title || tab.url).replace(/\s+/g, " ").trim().slice(0, 40);
  return `@${title}`;
}

function findMenu(text: string, caret: number, mode: ComposerMode, tabs: TabInfo[]): Menu | null {
  const commands = commandMatches(text, mode);
  if (commands.length > 0) {
    return { kind: "command", items: commands };
  }
  if (tabs.length === 0) {
    return null;
  }
  const before = text.slice(0, caret);
  const at = before.lastIndexOf("@");
  if (at < 0 || (at > 0 && !/\s/.test(before[at - 1]))) {
    return null;
  }
  const query = before.slice(at + 1);
  if (/\s{2}|\n/.test(query) || query.length > 40) {
    return null;
  }
  const needle = query.toLowerCase();
  const items = tabs.filter((tab) => !needle || tab.title.toLowerCase().includes(needle) || tab.url.toLowerCase().includes(needle)).slice(0, 6);
  return items.length > 0 ? { kind: "mention", items, at, caret } : null;
}

export function AgentComposer({ mode, draftKey, disabled, suggestions = [], tabs = [], history = [], onSubmit }: Props) {
  const input = useRef<HTMLTextAreaElement | null>(null);
  const [text, setTextState] = useState(() => drafts.get(draftKey) ?? "");
  const [caret, setCaret] = useState(0);
  const [mentions, setMentions] = useState<Mention[]>([]);
  const [highlight, setHighlight] = useState(0);
  const [menuClosed, setMenuClosed] = useState(false);
  const [recall, setRecall] = useState(-1);
  const [suggestionsOpen, setSuggestionsOpen] = useState(false);
  const menu = menuClosed || disabled ? null : findMenu(text, caret, mode, tabs);
  const menuSize = menu?.items.length ?? 0;
  const active = menuSize > 0 ? Math.min(highlight, menuSize - 1) : 0;

  const setText = (value: string) => {
    setTextState(value);
    drafts.set(draftKey, value);
  };

  const focusAt = (position: number) => {
    window.requestAnimationFrame(() => {
      input.current?.focus();
      input.current?.setSelectionRange(position, position);
      setCaret(position);
    });
  };

  const send = () => {
    const value = text.trim();
    if (!value || disabled) {
      return;
    }
    const contextTabs = mentions.filter((mention) => value.includes(mention.token)).map((mention) => mention.id);
    onSubmit(value, [...new Set(contextTabs)]);
    setText("");
    setMentions([]);
    setRecall(-1);
  };

  const pick = (index: number) => {
    if (!menu) {
      return;
    }
    if (menu.kind === "command") {
      const command = menu.items[index];
      const next = `/${command.name} `;
      setText(next);
      focusAt(next.length);
      return;
    }
    const tab = menu.items[index];
    const token = mentionToken(tab);
    const next = `${text.slice(0, menu.at)}${token} ${text.slice(menu.caret)}`;
    setText(next);
    setMentions((previous) => [...previous.filter((mention) => mention.id !== tab.id), { id: tab.id, token }]);
    focusAt(menu.at + token.length + 1);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (menu && menuSize > 0) {
      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault();
        setHighlight((active + (event.key === "ArrowDown" ? 1 : menuSize - 1)) % menuSize);
        return;
      }
      if (event.key === "Enter" || event.key === "Tab") {
        event.preventDefault();
        pick(active);
        return;
      }
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        setMenuClosed(true);
        return;
      }
    }
    if ((event.key === "ArrowUp" || event.key === "ArrowDown") && history.length > 0 && (!text || (recall >= 0 && text === history[recall]))) {
      event.preventDefault();
      const next = event.key === "ArrowUp" ? Math.min(history.length - 1, recall + 1) : recall - 1;
      setRecall(next);
      setText(next >= 0 ? history[next] : "");
      return;
    }
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      send();
    }
  };

  return (
    <div id="agent-composer" className="flex flex-col gap-2">
      {mode === "start" && !text && suggestions.length > 0 ? (
        <div id="agent-composer-examples">
          <button
            id="agent-composer-examples-toggle"
            type="button"
            className="flex items-center gap-1 rounded-md px-1 py-0.5 text-xs text-zinc-400 hover:text-zinc-100"
            aria-expanded={suggestionsOpen}
            aria-controls="agent-composer-examples-list"
            onClick={() => setSuggestionsOpen((open) => !open)}
          >
            <ChevronRight id="agent-composer-examples-toggle-icon" className={cn("size-3.5 transition-transform", suggestionsOpen && "rotate-90")} aria-hidden />
            Suggestions
          </button>
          {suggestionsOpen ? (
            <ul id="agent-composer-examples-list" className="mt-1 flex flex-col gap-1">
              {suggestions.map((example, index) => (
                <li id={`agent-composer-example-${index}`} key={example}>
                  <button
                    id={`agent-composer-example-${index}-button`}
                    type="button"
                    className="w-full rounded-md px-2 py-1 text-left text-xs text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100"
                    onClick={() => setText(example)}
                  >
                    {example}
                  </button>
                </li>
              ))}
            </ul>
          ) : null}
        </div>
      ) : null}
      <div id="agent-composer-box" className="relative">
        {menu && menuSize > 0 ? (
          <ul
            id="agent-composer-menu"
            role="listbox"
            aria-label={menu.kind === "command" ? "Commands" : "Tabs"}
            className="absolute inset-x-0 bottom-full z-30 mb-1 max-h-56 overflow-y-auto rounded-md bg-zinc-900 p-1 shadow-xl ring-1 ring-zinc-700"
          >
            {menu.kind === "command"
              ? menu.items.map((command, index) => (
                  <li id={`agent-composer-menu-${command.name}`} key={command.name} role="option" aria-selected={index === active}>
                    <button
                      id={`agent-composer-menu-${command.name}-button`}
                      type="button"
                      className={cn("flex w-full items-start gap-2 rounded px-2 py-1 text-left", index === active ? "bg-zinc-800" : "hover:bg-zinc-800/60")}
                      onMouseDown={(event) => event.preventDefault()}
                      onMouseEnter={() => setHighlight(index)}
                      onClick={() => pick(index)}
                    >
                      <Slash id={`agent-composer-menu-${command.name}-icon`} className="mt-0.5 size-3 shrink-0 text-sky-400" aria-hidden />
                      <span id={`agent-composer-menu-${command.name}-body`} className="min-w-0">
                        <span id={`agent-composer-menu-${command.name}-usage`} className="block font-mono text-[11px] text-zinc-100">
                          {command.usage}
                        </span>
                        <span id={`agent-composer-menu-${command.name}-hint`} className="block text-[11px] text-zinc-500">
                          {command.hint}
                        </span>
                      </span>
                    </button>
                  </li>
                ))
              : menu.items.map((tab, index) => (
                  <li id={`agent-composer-menu-tab-${tab.id}`} key={tab.id} role="option" aria-selected={index === active}>
                    <button
                      id={`agent-composer-menu-tab-${tab.id}-button`}
                      type="button"
                      className={cn("flex w-full items-center gap-2 rounded px-2 py-1 text-left", index === active ? "bg-zinc-800" : "hover:bg-zinc-800/60")}
                      onMouseDown={(event) => event.preventDefault()}
                      onMouseEnter={() => setHighlight(index)}
                      onClick={() => pick(index)}
                    >
                      <FavIcon id={`agent-composer-menu-tab-${tab.id}-icon`} url={tab.url} src={tab.favicon} className="size-3.5" />
                      <span id={`agent-composer-menu-tab-${tab.id}-title`} className="min-w-0 flex-1 truncate text-xs text-zinc-100">
                        {tab.title || tab.url}
                      </span>
                    </button>
                  </li>
                ))}
          </ul>
        ) : null}
        <textarea
          id="agent-composer-input"
          ref={input}
          className={cn(inputClassName, "min-h-16 resize-none pr-10")}
          rows={3}
          value={text}
          disabled={disabled}
          placeholder={PLACEHOLDERS[mode]}
          onChange={(event) => {
            setText(event.target.value);
            setCaret(event.target.selectionStart);
            setMenuClosed(false);
            setHighlight(0);
          }}
          onSelect={(event) => setCaret(event.currentTarget.selectionStart)}
          onKeyDown={onKeyDown}
        />
        <IconButton
          id="agent-composer-send"
          label={mode === "start" ? "Start (Enter)" : "Send (Enter)"}
          className="absolute bottom-1.5 right-1.5 size-7"
          disabled={disabled || !text.trim()}
          onClick={send}
        >
          <SendHorizontal id="agent-composer-send-icon" aria-hidden />
        </IconButton>
      </div>
      {mentions.some((mention) => text.includes(mention.token)) ? (
        <div id="agent-composer-mentions" className="flex flex-wrap items-center gap-1 text-[10px] text-zinc-500">
          <AtSign id="agent-composer-mentions-icon" className="size-3" aria-hidden />
          Reads first:
          {mentions
            .filter((mention) => text.includes(mention.token))
            .map((mention) => (
              <span id={`agent-composer-mention-${mention.id}`} key={mention.id} className="rounded bg-zinc-800 px-1 text-zinc-300">
                {mention.token.slice(1)}
              </span>
            ))}
        </div>
      ) : null}
    </div>
  );
}
