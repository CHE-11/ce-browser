import { useEffect, useRef, useState, type MouseEvent as ReactMouseEvent } from "react";
import {
  AlertTriangle,
  Brain,
  CheckCircle2,
  ChevronRight,
  Copy,
  Download,
  Eye,
  EyeOff,
  MessagesSquare,
  MousePointerClick,
  Pin,
  RefreshCw,
  ShieldAlert,
  ShieldCheck,
  SkipForward,
  User,
  Workflow,
  Wrench,
} from "lucide-react";
import type { AgentRun, AgentStep, AgentStepKind } from "@type/browser";
import { Button } from "@/components/Button";
import { ContextMenu, ContextMenuItem, ContextMenuSeparator } from "@/components/ContextMenu";
import { Markdown } from "@/components/Markdown";
import { chooseSavePath, copyText, pinPage, writeTextFile } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { chatFileName, formatWholeChat } from "./agent-chat-export";
import { type AgentTimelineFilter } from "./agent-timeline-filter";
import { formatClock } from "./agent-styles";
import { LongMessage } from "./AgentError";

const ICONS: Record<AgentStepKind, typeof Brain> = {
  think: Brain,
  action: MousePointerClick,
  tool: Wrench,
  approved: ShieldCheck,
  blocked: ShieldAlert,
  error: AlertTriangle,
  user: User,
  answer: CheckCircle2,
};

const TONES: Record<AgentStepKind, string> = {
  think: "text-zinc-400",
  action: "text-sky-400",
  tool: "text-teal-400",
  approved: "text-fuchsia-300",
  blocked: "text-amber-400",
  error: "text-red-400",
  user: "text-violet-300",
  answer: "text-emerald-400",
};

/** Action lists open by default, counted from the newest. Messages stay shown until the user hides them. */
const OPEN_GROUPS = 4;

type Indexed = { step: AgentStep; index: number };
type Group = { head: Indexed | null; items: Indexed[] };
type MenuState = { x: number; y: number; index: number; text: string };

function groupSteps(steps: AgentStep[]) {
  const groups: Group[] = [];
  steps.forEach((step, index) => {
    const entry = { step, index };
    if (step.kind === "think" || step.kind === "user" || step.kind === "answer" || groups.length === 0) {
      const head = step.kind === "think" || step.kind === "user" || step.kind === "answer" ? entry : null;
      groups.push({ head, items: head ? [] : [entry] });
      return;
    }
    groups[groups.length - 1].items.push(entry);
  });
  return groups;
}

function matches(filter: AgentTimelineFilter, kind: AgentStepKind) {
  if (filter === "chat") {
    return kind === "user" || kind === "think" || kind === "answer";
  }
  if (filter === "actions") {
    return kind === "action" || kind === "tool" || kind === "approved";
  }
  if (filter === "problems") {
    return kind === "error" || kind === "blocked";
  }
  return true;
}

function errorOf(reason: unknown) {
  return typeof reason === "string" ? reason : reason instanceof Error ? reason.message : "Something went wrong.";
}

/** Short messages stay fully visible; only longer ones get a collapse control. */
function isLongMessage(text: string) {
  const value = text.trim();
  return value.length > 220 || value.split("\n").length > 4;
}

type FoldAll = { open: boolean; key: number };

type Props = {
  id: string;
  run: AgentRun;
  steps: AgentStep[];
  filter?: AgentTimelineFilter;
  /** Bumps when Expand all / Collapse all is pressed. */
  foldAll?: FoldAll | null;
  /** What the model is writing now, while the run works. */
  streaming?: { text: string; thinking: string } | null;
  /** Recovery actions for failed or blocked steps. */
  onCockpit?: (action: "retry" | "skip") => void;
  onSaveFlow?: () => void;
  onNotice?: (text: string) => void;
};

/** The run's chat: user and agent messages stay visible, and each one can be hidden or shown. Actions under an agent message fold separately. */
export function AgentTimeline({ id, run, steps, filter = "all", foldAll = null, streaming, onCockpit, onSaveFlow, onNotice }: Props) {
  const end = useRef<HTMLDivElement | null>(null);
  const [toggled, setToggled] = useState<Record<number, boolean>>({});
  const [messagesOpen, setMessagesOpen] = useState<Record<number, boolean>>({});
  const [zoomed, setZoomed] = useState<number | null>(null);
  const [menu, setMenu] = useState<MenuState | null>(null);
  const groups = groupSteps(steps);
  const live = streaming && (streaming.text || streaming.thinking) ? streaming : null;

  useEffect(() => {
    const scroller = end.current?.closest("#agent-panel-body, #agent-page-body");
    if (!scroller) {
      return;
    }
    const nearBottom = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 200;
    if (nearBottom) {
      end.current?.scrollIntoView({ block: "end" });
    }
  }, [steps.length, live?.text, live?.thinking]);

  useEffect(() => {
    if (!foldAll) {
      return;
    }
    const nextMessages: Record<number, boolean> = {};
    const nextActions: Record<number, boolean> = {};
    for (const group of groups) {
      const key = group.head?.index ?? group.items[0]?.index;
      if (key === undefined) {
        continue;
      }
      if (group.head) {
        nextMessages[group.head.index] = foldAll.open;
      }
      nextActions[key] = foldAll.open;
    }
    setMessagesOpen(nextMessages);
    setToggled(nextActions);
  }, [foldAll?.key, foldAll?.open]);

  const openMenu = (event: ReactMouseEvent, index: number, text: string) => {
    if (!text.trim()) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    setMenu({ x: event.clientX, y: event.clientY, index, text });
  };

  const closeMenu = () => setMenu(null);

  const runAction = (label: string, action: () => Promise<unknown>) => {
    closeMenu();
    void action()
      .then((result) => {
        if (result !== false) {
          onNotice?.(label);
        }
      })
      .catch((reason: unknown) => onNotice?.(errorOf(reason)));
  };

  const saveText = async (title: string, body: string) => {
    const path = await chooseSavePath(chatFileName(title), "Markdown", ["md"]);
    if (!path) {
      return false;
    }
    await writeTextFile(path, body.endsWith("\n") ? body : `${body}\n`);
    return true;
  };

  const row = ({ step, index }: Indexed) => {
    const Icon = ICONS[step.kind] ?? Wrench;
    const errorText = step.kind === "error" && step.detail ? `${step.text}\n${step.detail}` : step.text;
    return (
      <li
        id={`${id}-${index}`}
        key={`${step.at}-${index}`}
        className="flex gap-2 rounded-md bg-zinc-900/70 p-2 ring-1 ring-zinc-800"
        onContextMenu={(event) => openMenu(event, index, errorText)}
      >
        <Icon id={`${id}-${index}-icon`} className={cn("mt-0.5 size-3.5 shrink-0", TONES[step.kind])} aria-hidden />
        <div id={`${id}-${index}-body`} className="min-w-0 flex-1">
          {step.kind === "error" ? (
            <LongMessage id={`${id}-${index}-text`} text={errorText} className="text-xs text-zinc-100" />
          ) : (
            <>
              <div id={`${id}-${index}-text`} className="whitespace-pre-wrap break-words text-xs text-zinc-100">
                {step.text}
              </div>
              {step.detail ? (
                <div id={`${id}-${index}-detail`} className="mt-0.5 line-clamp-3 whitespace-pre-wrap break-words text-[11px] text-zinc-500">
                  {step.detail}
                </div>
              ) : null}
            </>
          )}
          {step.thumb ? (
            <button
              id={`${id}-${index}-thumb`}
              type="button"
              className="mt-1.5 block overflow-hidden rounded ring-1 ring-zinc-700"
              title={zoomed === index ? "Shrink" : "Enlarge"}
              onClick={() => setZoomed(zoomed === index ? null : index)}
            >
              <img id={`${id}-${index}-thumb-image`} src={step.thumb} alt="Page after this step" className={cn("block", zoomed === index ? "w-full" : "w-36")} />
            </button>
          ) : null}
          {onCockpit && (step.kind === "error" || step.kind === "blocked") ? (
            <div id={`${id}-${index}-cockpit`} className="mt-1.5 flex gap-1">
              <Button id={`${id}-${index}-retry`} size="xs" variant="outline" onClick={() => onCockpit("retry")}>
                <RefreshCw id={`${id}-${index}-retry-icon`} className="size-3" aria-hidden />
                Retry
              </Button>
              <Button id={`${id}-${index}-skip`} size="xs" variant="ghost" onClick={() => onCockpit("skip")}>
                <SkipForward id={`${id}-${index}-skip-icon`} className="size-3" aria-hidden />
                Skip
              </Button>
            </div>
          ) : null}
        </div>
        <time id={`${id}-${index}-time`} className="shrink-0 text-[10px] text-zinc-600">
          {formatClock(step.at)}
        </time>
      </li>
    );
  };

  const shown = (index: number) => messagesOpen[index] ?? true;
  const toggleMessage = (index: number) => setMessagesOpen((previous) => ({ ...previous, [index]: !(previous[index] ?? true) }));

  const messageToggle = (index: number, open: boolean) => (
    <button
      id={`${id}-${index}-message`}
      type="button"
      className="mt-0.5 flex size-4 shrink-0 items-center justify-center rounded text-zinc-500 hover:text-zinc-200"
      aria-expanded={open}
      aria-label={open ? "Hide message" : "Show message"}
      title={open ? "Hide message" : "Show message"}
      onClick={() => toggleMessage(index)}
    >
      <ChevronRight id={`${id}-${index}-message-icon`} className={cn("size-3.5 transition-transform", open && "rotate-90")} aria-hidden />
    </button>
  );

  const head = ({ step, index }: Indexed, actionsOpen: boolean, count: number, onToggleActions: () => void) => {
    const long = isLongMessage(step.text);
    const open = long ? shown(index) : true;
    const prior = steps[index - 1];
    const repeatsPrior =
      !!step.text.trim() && !!prior && (prior.kind === "think" || prior.kind === "answer") && prior.text.trim() === step.text.trim();
    if (step.kind === "user") {
      return (
        <div id={`${id}-${index}`} className="flex items-start justify-end gap-1">
          {long ? messageToggle(index, open) : null}
          <div
            id={`${id}-${index}-bubble`}
            className="min-w-0 max-w-[85%] rounded-lg rounded-br-sm bg-violet-950/50 px-2.5 py-1.5 text-xs text-violet-100 ring-1 ring-violet-900/60"
            onContextMenu={(event) => openMenu(event, index, step.text)}
          >
            <div id={`${id}-${index}-text`} className={cn("whitespace-pre-wrap break-words", long && !open && "line-clamp-1")}>
              {step.text}
            </div>
            {open && step.detail ? (
              <div id={`${id}-${index}-detail`} className="mt-1 whitespace-pre-wrap break-words text-[11px] text-violet-300/70">
                {step.detail}
              </div>
            ) : null}
            {open && step.thumb ? (
              <button
                id={`${id}-${index}-thumb`}
                type="button"
                className="mt-1.5 block overflow-hidden rounded ring-1 ring-violet-800/80"
                title={zoomed === index ? "Shrink" : "Enlarge"}
                onClick={() => setZoomed(zoomed === index ? null : index)}
              >
                <img
                  id={`${id}-${index}-thumb-image`}
                  src={step.thumb}
                  alt="Attached image"
                  className={cn("block", zoomed === index ? "w-full" : "w-28")}
                />
              </button>
            ) : null}
          </div>
        </div>
      );
    }
    if (step.kind === "answer" && (!step.text.trim() || repeatsPrior)) {
      return (
        <div id={`${id}-${index}`} className="flex items-center gap-1.5 text-[11px] text-emerald-400">
          <CheckCircle2 id={`${id}-${index}-icon`} className="size-3.5" aria-hidden />
          Finished
          <time id={`${id}-${index}-time`} className="ml-auto text-[10px] text-zinc-600">
            {formatClock(step.at)}
          </time>
        </div>
      );
    }
    return (
      <div id={`${id}-${index}`} className="flex gap-1.5">
        {long ? messageToggle(index, open) : null}
        <div id={`${id}-${index}-body`} className="min-w-0 flex-1">
          <div
            id={`${id}-${index}-bubble`}
            className="rounded-lg rounded-bl-sm bg-zinc-900/80 px-2.5 py-1.5 ring-1 ring-zinc-800"
            onContextMenu={(event) => openMenu(event, index, step.text)}
          >
            {open ? (
              <Markdown id={`${id}-${index}-text`} text={step.text} className="text-xs text-zinc-200" />
            ) : (
              <p id={`${id}-${index}-text`} className="line-clamp-1 whitespace-pre-wrap break-words text-xs text-zinc-400">
                {step.text}
              </p>
            )}
          </div>
          {step.kind === "answer" ? (
            <div id={`${id}-${index}-status`} className="mt-1 flex items-center gap-1.5 text-[11px] text-emerald-400">
              <CheckCircle2 id={`${id}-${index}-icon`} className="size-3.5" aria-hidden />
              Finished
              <time id={`${id}-${index}-time`} className="ml-auto text-[10px] text-zinc-600">
                {formatClock(step.at)}
              </time>
            </div>
          ) : null}
          {open && count > 0 ? (
            <button
              id={`${id}-${index}-toggle`}
              type="button"
              className="mt-1 flex items-center gap-1 text-[10px] text-zinc-500 hover:text-zinc-300"
              aria-expanded={actionsOpen}
              aria-label={actionsOpen ? "Hide actions" : `Show ${count} actions`}
              onClick={onToggleActions}
            >
              <ChevronRight id={`${id}-${index}-toggle-icon`} className={cn("size-3 transition-transform", actionsOpen && "rotate-90")} aria-hidden />
              <span id={`${id}-${index}-folded`}>{actionsOpen ? "Hide actions" : `${count} ${count === 1 ? "action" : "actions"}`}</span>
            </button>
          ) : null}
          {!open && count > 0 ? (
            <div id={`${id}-${index}-folded`} className="mt-0.5 text-[10px] text-zinc-500">
              {count} {count === 1 ? "action" : "actions"}
            </div>
          ) : null}
        </div>
      </div>
    );
  };

  const menuOpen = menu ? shown(menu.index) : true;
  const menuLong = menu ? isLongMessage(menu.text) : false;
  const scrollParentId = id.startsWith("agent-page") ? "agent-page-body" : "agent-panel-body";

  return (
    <div id={id} className="flex flex-col gap-2">
      {filter === "all" || filter === "chat" ? (
        <ol id={`${id}-groups`} className="flex flex-col gap-3">
          {groups.map((group, groupIndex) => {
            if (filter === "chat" && (!group.head || !matches("chat", group.head.step.kind))) {
              return null;
            }
            const key = group.head?.index ?? group.items[0]?.index ?? groupIndex;
            const fallbackOpen = groupIndex >= groups.length - OPEN_GROUPS;
            const actionsOpen = filter === "chat" ? false : (toggled[key] ?? fallbackOpen);
            const messageOpen = group.head ? shown(group.head.index) : true;
            const actionCount = filter === "chat" ? 0 : group.items.length;
            return (
              <li id={`${id}-group-${key}`} key={key} className="flex flex-col gap-1.5">
                {group.head
                  ? head(group.head, actionsOpen, actionCount, () => setToggled((previous) => ({ ...previous, [key]: !actionsOpen })))
                  : null}
                {filter !== "chat" && group.items.length > 0 && messageOpen && (actionsOpen || !group.head) ? (
                  <ol id={`${id}-group-${key}-items`} className={cn("flex flex-col gap-1.5", group.head?.step.kind === "think" && "ml-5")}>
                    {group.items.map(row)}
                  </ol>
                ) : null}
              </li>
            );
          })}
        </ol>
      ) : (
        <ol id={`${id}-filtered`} className="flex flex-col gap-1.5">
          {steps
            .map((step, index) => ({ step, index }))
            .filter(({ step }) => matches(filter, step.kind))
            .map(row)}
        </ol>
      )}
      {live ? (
        <div id={`${id}-live`} aria-live="polite" className="flex gap-1.5 rounded-md border border-dashed border-zinc-800 p-2">
          <Brain id={`${id}-live-icon`} className="mt-0.5 size-3.5 shrink-0 animate-pulse text-sky-400" aria-hidden />
          <div id={`${id}-live-body`} className="min-w-0 flex-1">
            {live.thinking ? (
              <p id={`${id}-live-thinking`} className="line-clamp-4 whitespace-pre-wrap break-words text-[11px] italic text-zinc-500">
                {live.thinking.slice(-600)}
              </p>
            ) : null}
            {live.text ? <Markdown id={`${id}-live-text`} text={live.text} className="text-xs text-zinc-200" /> : null}
          </div>
        </div>
      ) : null}
      <div id={`${id}-end`} ref={end} aria-hidden className="h-px" />

      {menu ? (
        <ContextMenu id={`${id}-menu`} label="Message" x={menu.x} y={menu.y} onClose={closeMenu} scrollParentId={scrollParentId}>
          <ContextMenuItem
            id={`${id}-menu-copy`}
            icon={<Copy />}
            label="Copy"
            onClick={() => runAction("Copied", () => copyText(menu.text.trim()))}
          />
          <ContextMenuItem
            id={`${id}-menu-save`}
            icon={<Download />}
            label="Save…"
            onClick={() => runAction("Saved", () => saveText(menu.text.slice(0, 40) || run.goal, `${menu.text.trim()}\n`))}
          />
          <ContextMenuItem
            id={`${id}-menu-copy-chat`}
            icon={<MessagesSquare />}
            label="Copy whole chat"
            onClick={() => runAction("Copied chat", () => copyText(formatWholeChat(run, steps)))}
          />
          <ContextMenuItem
            id={`${id}-menu-save-chat`}
            icon={<Download />}
            label="Save whole chat…"
            onClick={() => runAction("Saved chat", () => saveText(run.goal, formatWholeChat(run, steps)))}
          />
          <ContextMenuSeparator id={`${id}-menu-sep-run`} />
          {run.source === "agent" ? (
            <ContextMenuItem
              id={`${id}-menu-pin`}
              icon={<Pin />}
              label="Pin page"
              onClick={() => runAction("Pinned", () => pinPage(run.tabId, "", false))}
            />
          ) : null}
          {run.flowSteps > 0 && onSaveFlow ? (
            <ContextMenuItem
              id={`${id}-menu-flow`}
              icon={<Workflow />}
              label={`Save as flow (${run.flowSteps})`}
              onClick={() => {
                closeMenu();
                onSaveFlow();
                onNotice?.("Saved as flow");
              }}
            />
          ) : null}
          {menuLong ? (
            <>
              <ContextMenuSeparator id={`${id}-menu-sep-view`} />
              <ContextMenuItem
                id={`${id}-menu-toggle`}
                icon={menuOpen ? <EyeOff /> : <Eye />}
                label={menuOpen ? "Hide message" : "Show message"}
                onClick={() => {
                  toggleMessage(menu.index);
                  closeMenu();
                }}
              />
            </>
          ) : null}
        </ContextMenu>
      ) : null}
    </div>
  );
}
