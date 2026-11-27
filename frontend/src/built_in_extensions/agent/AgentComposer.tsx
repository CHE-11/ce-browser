import { useRef, useState, type DragEvent, type KeyboardEvent, type ClipboardEvent } from "react";
import { AtSign, FileText, Paperclip, SendHorizontal, Slash, X } from "lucide-react";
import type { AgentAttachment, TabInfo } from "@type/browser";
import { FavIcon } from "@/components/FavIcon";
import { IconButton } from "@/components/IconButton";
import { commandMatches, type AgentCommand, type ComposerMode } from "@/lib/agent-commands";
import { pickAgentFiles } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { inputClassName } from "./agent-styles";

/** Unsent text per run, kept while the panel is open. */
const drafts = new Map<string, string>();

const PLACEHOLDERS: Record<ComposerMode, string> = {
  start: "What should the agent do? @ adds a tab, / shows commands",
  reply: "Tell the agent something…",
  followup: "Ask a follow-up…",
};

const MAX_ATTACHMENTS = 6;
const IMAGE_EXT = /\.(png|jpe?g|webp|gif)$/i;
const TEXT_EXT = /\.(txt|md|json|csv|log|html?|css|jsx?|tsx?)$/i;

type Props = {
  mode: ComposerMode;
  draftKey: string;
  disabled?: boolean;
  /** When false, image paste/attach is blocked (text files still ok). */
  allowImages?: boolean;
  /** Tabs that `@` can point the agent at. */
  tabs?: TabInfo[];
  /** Earlier tasks, newest first, for the up arrow. */
  history?: string[];
  onSubmit: (text: string, contextTabs: number[], attachments: AgentAttachment[]) => void;
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

function stamp() {
  return `att-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

function stripDataUrl(dataUrl: string) {
  const comma = dataUrl.indexOf(",");
  return comma >= 0 ? dataUrl.slice(comma + 1) : dataUrl;
}

function isImageFile(file: File) {
  return file.type.startsWith("image/") || IMAGE_EXT.test(file.name);
}

function isTextFile(file: File) {
  return file.type.startsWith("text/") || file.type === "application/json" || TEXT_EXT.test(file.name);
}

async function fileToAttachment(file: File): Promise<AgentAttachment | null> {
  if (isImageFile(file)) {
    const dataUrl = await new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result ?? ""));
      reader.onerror = () => reject(new Error("Could not read that image."));
      reader.readAsDataURL(file);
    });
    return {
      id: stamp(),
      kind: "image",
      name: file.name || "image.png",
      mediaType: file.type || "image/png",
      data: stripDataUrl(dataUrl),
    };
  }
  if (isTextFile(file)) {
    const text = await file.text();
    return {
      id: stamp(),
      kind: "text",
      name: file.name || "file.txt",
      mediaType: file.type || "text/plain",
      data: text,
    };
  }
  return null;
}

function previewSrc(attachment: AgentAttachment) {
  if (attachment.kind !== "image") {
    return null;
  }
  if (attachment.data.startsWith("data:")) {
    return attachment.data;
  }
  return `data:${attachment.mediaType || "image/png"};base64,${attachment.data}`;
}

export function AgentComposer({ mode, draftKey, disabled, allowImages = true, tabs = [], history = [], onSubmit }: Props) {
  const input = useRef<HTMLTextAreaElement | null>(null);
  const fileInput = useRef<HTMLInputElement | null>(null);
  const [text, setTextState] = useState(() => drafts.get(draftKey) ?? "");
  const [attachments, setAttachments] = useState<AgentAttachment[]>([]);
  const [attachNote, setAttachNote] = useState<string | null>(null);
  const [dragging, setDragging] = useState(false);
  const [caret, setCaret] = useState(0);
  const [mentions, setMentions] = useState<Mention[]>([]);
  const [highlight, setHighlight] = useState(0);
  const [menuClosed, setMenuClosed] = useState(false);
  const [recall, setRecall] = useState(-1);
  const menu = menuClosed || disabled ? null : findMenu(text, caret, mode, tabs);
  const menuSize = menu?.items.length ?? 0;
  const active = menuSize > 0 ? Math.min(highlight, menuSize - 1) : 0;
  const hasImages = attachments.some((item) => item.kind === "image");

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

  const addAttachments = async (files: FileList | File[]) => {
    const list = Array.from(files);
    if (list.length === 0) {
      return;
    }
    setAttachNote(null);
    const next = [...attachments];
    for (const file of list) {
      if (next.length >= MAX_ATTACHMENTS) {
        setAttachNote(`Attach at most ${MAX_ATTACHMENTS} files.`);
        break;
      }
      if (isImageFile(file) && !allowImages) {
        setAttachNote("This model may not accept images. Pick a vision model, or attach a text file.");
        continue;
      }
      try {
        const item = await fileToAttachment(file);
        if (!item) {
          setAttachNote(`Unsupported file: ${file.name || "attachment"}`);
          continue;
        }
        next.push(item);
      } catch (reason: unknown) {
        setAttachNote(String(reason));
      }
    }
    setAttachments(next);
  };

  const pickFiles = async () => {
    if (disabled) {
      return;
    }
    setAttachNote(null);
    try {
      const picked = await pickAgentFiles();
      if (picked.length === 0) {
        return;
      }
      const next = [...attachments];
      for (const item of picked) {
        if (next.length >= MAX_ATTACHMENTS) {
          setAttachNote(`Attach at most ${MAX_ATTACHMENTS} files.`);
          break;
        }
        if (item.kind === "image" && !allowImages) {
          setAttachNote("This model may not accept images. Pick a vision model, or attach a text file.");
          continue;
        }
        next.push(item);
      }
      setAttachments(next);
    } catch (reason: unknown) {
      setAttachNote(String(reason));
    }
  };

  const send = () => {
    const value = text.trim();
    if ((!value && attachments.length === 0) || disabled) {
      return;
    }
    if (hasImages && !allowImages) {
      setAttachNote("Remove images or pick a vision model before sending.");
      return;
    }
    const contextTabs = mentions.filter((mention) => value.includes(mention.token)).map((mention) => mention.id);
    onSubmit(value, [...new Set(contextTabs)], attachments);
    setText("");
    setAttachments([]);
    setAttachNote(null);
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

  const onPaste = (event: ClipboardEvent<HTMLTextAreaElement>) => {
    const files = event.clipboardData?.files;
    const items = event.clipboardData?.items;
    const imageFiles: File[] = [];
    if (files && files.length > 0) {
      for (const file of Array.from(files)) {
        if (isImageFile(file) || isTextFile(file)) {
          imageFiles.push(file);
        }
      }
    } else if (items) {
      for (const item of Array.from(items)) {
        if (item.kind === "file") {
          const file = item.getAsFile();
          if (file && (isImageFile(file) || isTextFile(file))) {
            imageFiles.push(file);
          }
        }
      }
    }
    if (imageFiles.length === 0) {
      return;
    }
    event.preventDefault();
    void addAttachments(imageFiles);
  };

  const onDrop = (event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    setDragging(false);
    if (disabled) {
      return;
    }
    void addAttachments(event.dataTransfer.files);
  };

  return (
    <div id="agent-composer" className="flex flex-col gap-2">
      {attachments.length > 0 ? (
        <div id="agent-composer-chips" className="flex flex-wrap gap-1.5">
          {attachments.map((item) => {
            const src = previewSrc(item);
            return (
              <div
                id={`agent-composer-chip-${item.id}`}
                key={item.id}
                className="flex max-w-full items-center gap-1.5 rounded-md bg-zinc-900 px-1.5 py-1 ring-1 ring-zinc-700"
              >
                {src ? (
                  <img id={`agent-composer-chip-${item.id}-thumb`} src={src} alt="" className="size-8 rounded object-cover" />
                ) : (
                  <FileText id={`agent-composer-chip-${item.id}-icon`} className="size-3.5 shrink-0 text-zinc-400" aria-hidden />
                )}
                <span id={`agent-composer-chip-${item.id}-name`} className="max-w-36 truncate text-[11px] text-zinc-300">
                  {item.name}
                </span>
                <button
                  id={`agent-composer-chip-${item.id}-remove`}
                  type="button"
                  className="flex size-5 items-center justify-center rounded text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200"
                  aria-label={`Remove ${item.name}`}
                  onClick={() => setAttachments((previous) => previous.filter((entry) => entry.id !== item.id))}
                >
                  <X id={`agent-composer-chip-${item.id}-remove-icon`} className="size-3" aria-hidden />
                </button>
              </div>
            );
          })}
        </div>
      ) : null}
      {hasImages && !allowImages ? (
        <p id="agent-composer-vision-block" role="status" className="rounded-md bg-amber-950/40 px-2 py-1.5 text-[11px] text-amber-200 ring-1 ring-amber-900/60">
          Attached images need a vision model. Remove them or change the model.
        </p>
      ) : null}
      {attachNote ? (
        <p id="agent-composer-attach-note" role="status" className="text-[11px] text-amber-200">
          {attachNote}
        </p>
      ) : null}
      <div
        id="agent-composer-box"
        className={cn("relative", dragging && "rounded-md ring-1 ring-sky-600")}
        onDragEnter={(event) => {
          event.preventDefault();
          if (!disabled) {
            setDragging(true);
          }
        }}
        onDragOver={(event) => event.preventDefault()}
        onDragLeave={() => setDragging(false)}
        onDrop={onDrop}
      >
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
          className={cn(inputClassName, "min-h-16 resize-none pr-16")}
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
          onPaste={onPaste}
        />
        <input
          id="agent-composer-file-input"
          ref={fileInput}
          type="file"
          className="hidden"
          multiple
          accept="image/png,image/jpeg,image/webp,image/gif,.txt,.md,.json,.csv,.log,.html,.htm,.css,.js,.ts,.tsx,.jsx"
          onChange={(event) => {
            if (event.target.files) {
              void addAttachments(event.target.files);
            }
            event.target.value = "";
          }}
        />
        <IconButton
          id="agent-composer-attach"
          label="Attach files"
          className="absolute bottom-1.5 right-9 size-7"
          disabled={disabled || attachments.length >= MAX_ATTACHMENTS}
          onClick={() => void pickFiles()}
        >
          <Paperclip id="agent-composer-attach-icon" aria-hidden />
        </IconButton>
        <IconButton
          id="agent-composer-send"
          label={mode === "start" ? "Start (Enter)" : "Send (Enter)"}
          className="absolute bottom-1.5 right-1.5 size-7"
          disabled={disabled || (!text.trim() && attachments.length === 0) || (hasImages && !allowImages)}
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
