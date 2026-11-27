import { useEffect, useRef, useState } from "react";
import { Bot, Hand, Play, Square } from "lucide-react";
import type { AgentAttachment, AgentAutonomy, AgentRun, AgentStep, BrowserSettings } from "@type/browser";
import { Button } from "@/components/Button";
import { EmptyState } from "@/components/EmptyState";
import { withDefaults, type AccessToggle } from "@/lib/agent-access";
import { parseAgentInput, type ComposerMode } from "@/lib/agent-commands";
import { fontStack, normalizeAgentFont } from "@/lib/appearance";
import { modelLikelyAcceptsVision } from "@/lib/ai/model-label";
import { AI_INPUT_TOKENS, AI_OUTPUT_TOKENS } from "@/lib/ai/token-limits";
import { agentMemorySave, saveSettings } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import { useAgentStream } from "@/lib/use-agent-stream";
import { isActiveRun, useAgentRun } from "@/lib/use-agent-run";
import { useAppearanceState, useSettings, useTabs } from "@/lib/use-browser";
import { formatCost, runCost, useModelPrice } from "@/lib/use-model-prices";
import { AgentApproval } from "./AgentApproval";
import { AgentCockpit } from "./AgentCockpit";
import { AgentComposer } from "./AgentComposer";
import { AgentError } from "./AgentError";
import { AgentGoalCard } from "./AgentGoalCard";
import { AgentHeader, type AgentPanelView } from "./AgentHeader";
import { AgentLimitsMenu } from "./AgentLimitsMenu";
import { AgentModelPicker } from "./AgentModelPicker";
import { AgentLiveView } from "./AgentLiveView";
import { AgentPageStatus } from "./AgentPageStatus";
import { AgentMemoryView } from "./AgentMemoryView";
import { AgentChatPins } from "./AgentChatPins";
import { AgentResult } from "./AgentResult";
import { AgentRunList } from "./AgentRunList";
import { type AgentTimelineFilter } from "./agent-timeline-filter";
import { AgentTimeline } from "./AgentTimeline";
import { formatTokens } from "./agent-styles";

const SIDEBAR_WIDTH_KEY = "ce-agent-page-sidebar-width";
const SIDEBAR_DEFAULT = 360;
const SIDEBAR_MIN = 220;
const SIDEBAR_MAX = 560;

/** Older runs stored the goal only on the card; show it as the first chat message. */
function chatSteps(run: AgentRun): AgentStep[] {
  const goal = run.goal.trim();
  if (!goal) {
    return run.steps;
  }
  const already = run.steps.some((step) => step.kind === "user" && step.text.trim() === goal);
  if (already) {
    return run.steps;
  }
  return [
    {
      kind: "user",
      text: run.goal,
      detail: "",
      thumb: null,
      at: run.started,
      px: null,
      py: null,
      px2: null,
      py2: null,
    },
    ...run.steps,
  ];
}

function clampSidebarWidth(size: number) {
  return Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, Math.round(size)));
}

function readSidebarWidth() {
  try {
    const raw = localStorage.getItem(SIDEBAR_WIDTH_KEY);
    if (!raw) {
      return SIDEBAR_DEFAULT;
    }
    const value = Number(raw);
    return Number.isFinite(value) ? clampSidebarWidth(value) : SIDEBAR_DEFAULT;
  } catch {
    return SIDEBAR_DEFAULT;
  }
}

/** Host of a web page without `www.`, or blank for other pages. */
function siteOf(url: string) {
  try {
    const parsed = new URL(url);
    return /^https?:$/.test(parsed.protocol) ? parsed.hostname.replace(/^www\./, "") : "";
  } catch {
    return "";
  }
}

/** Whether `url` is one of `sites` or a subdomain of one. */
function siteBlocked(sites: string[] | undefined, url: string) {
  const host = siteOf(url);
  if (!host || !sites?.length) {
    return false;
  }
  return sites.some((site) => host === site || host.endsWith(`.${site}`));
}

export type AgentWorkspaceLayout = "panel" | "page";

type Props = {
  /** `panel` is the docked sidebar. `page` is the full `ce://agent` chat. */
  layout?: AgentWorkspaceLayout;
  /** From `ce://agent?run=…` — select that chat when the page opens. */
  params?: URLSearchParams;
};

/** Shared agent chat: docked panel or full-page `ce://agent`. */
export function AgentWorkspace({ layout = "panel", params }: Props) {
  const page = layout === "page";
  const rootId = page ? "agent-page" : "agent-panel";
  useAppearanceState();
  const settings = useSettings();
  const tabs = useTabs();
  const agent = useAgentRun();
  const { run, runs, activeTab } = agent;
  const active = isActiveRun(run);
  const stream = useAgentStream(active && run ? run.id : null);
  const settingsRef = useRef(settings);
  const settingsWrites = useRef(0);
  if (settings && settingsWrites.current === 0) {
    settingsRef.current = settings;
  }
  const [view, setView] = useState<AgentPanelView>("run");
  const [notice, setNotice] = useState<string | null>(null);
  const [composerFocus, setComposerFocus] = useState(0);
  const [sidebarWidth, setSidebarWidth] = useState(readSidebarWidth);
  const [timelineFilter, setTimelineFilter] = useState<AgentTimelineFilter>("all");
  const [foldAll, setFoldAll] = useState<{ open: boolean; key: number } | null>(null);
  const deepLinkRun = params?.get("run");
  const appliedDeepLink = useRef<string | null>(null);
  /** After picking a chat, scroll the transcript to the latest message once content is ready. */
  const scrollToRun = useRef<number | null>(null);

  const openRun = (id: number) => {
    agent.select(id);
    setView("run");
    scrollToRun.current = id;
  };

  useEffect(() => {
    if (!page) {
      return;
    }
    localStorage.setItem(SIDEBAR_WIDTH_KEY, String(sidebarWidth));
  }, [page, sidebarWidth]);

  useEffect(() => {
    document.title = run ? `Agent · ${run.goal.slice(0, 40)}` : "Agent";
  }, [run]);

  useEffect(() => {
    setTimelineFilter("all");
    setFoldAll(null);
  }, [run?.id]);

  const selectRun = agent.select;
  useEffect(() => {
    if (!page || deepLinkRun == null || appliedDeepLink.current === deepLinkRun) {
      return;
    }
    const id = Number(deepLinkRun);
    if (!Number.isFinite(id)) {
      return;
    }
    if (!runs.some((item) => item.id === id)) {
      return;
    }
    appliedDeepLink.current = deepLinkRun;
    selectRun(id);
    setView("run");
    scrollToRun.current = id;
  }, [page, deepLinkRun, runs, selectRun]);

  useEffect(() => {
    if (composerFocus === 0) {
      return;
    }
    const frame = window.requestAnimationFrame(() => {
      const input = document.getElementById(`${rootId}-composer-input`) ?? document.getElementById("agent-composer-input");
      if (input instanceof HTMLTextAreaElement) {
        input.focus();
      }
    });
    return () => window.cancelAnimationFrame(frame);
  }, [composerFocus, rootId]);

  useEffect(() => {
    const target = scrollToRun.current;
    if (target == null || view !== "run" || run?.id !== target) {
      return;
    }
    const body = document.getElementById(`${rootId}-body`);
    if (!(body instanceof HTMLElement)) {
      return;
    }
    const scroll = () => {
      body.scrollTop = body.scrollHeight;
    };
    scroll();
    const frame = window.requestAnimationFrame(() => {
      scroll();
      if (run.steps.length > 0 || run.stepCount === 0) {
        scrollToRun.current = null;
      }
    });
    return () => window.cancelAnimationFrame(frame);
  }, [run?.id, run?.steps.length, run?.stepCount, view, rootId]);

  const stopRef = useRef(agent.stop);
  stopRef.current = agent.stop;
  const escapeRun = active ? run?.id : undefined;
  useEffect(() => {
    if (escapeRun === undefined) {
      return;
    }
    const onKey = (event: globalThis.KeyboardEvent) => {
      if (event.key !== "Escape" || event.ctrlKey || event.altKey || event.metaKey || event.defaultPrevented) {
        return;
      }
      if (document.querySelector("[data-agent-popover]")) {
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      if (!event.repeat) {
        stopRef.current(escapeRun);
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [escapeRun]);

  const mode: ComposerMode = !run ? "start" : active ? "reply" : run.source === "agent" ? "followup" : "start";
  const showRunAutonomy = !!run && active && run.source === "agent";
  const autonomy: AgentAutonomy = showRunAutonomy ? run.autonomy : agent.draft.autonomy;
  const provider = settings?.aiProvider ?? "openai";
  const providerModel = settings?.aiProviders[provider]?.model ?? "";
  const savedModel = settings?.automation.agentModel ?? "";
  const selectedModel = (run?.model || agent.draft.model || savedModel || providerModel).trim();
  const thinking = settings?.aiProviders[provider]?.thinking ?? "";
  const inputTokens = settings?.aiMaxInputTokens ?? AI_INPUT_TOKENS.fallback;
  const outputTokens = settings?.aiMaxOutputTokens ?? AI_OUTPUT_TOKENS.fallback;
  const modelPrice = useModelPrice(selectedModel);
  const spend = run ? runCost(modelPrice, run.tokensIn, run.tokensOut) : null;
  const visionOk = modelLikelyAcceptsVision(provider, selectedModel);
  const screenshotsOn = settings?.automation.screenshotsToModel !== false;
  const agentFontFamily = fontStack(normalizeAgentFont(settings?.agentFont));
  const saveChatSettings = (patch: Partial<BrowserSettings>) => {
    const current = settingsRef.current;
    if (!current) {
      return;
    }
    const next = { ...current, ...patch };
    settingsRef.current = next;
    settingsWrites.current += 1;
    void saveSettings(next).finally(() => {
      settingsWrites.current -= 1;
    });
  };
  const access = showRunAutonomy ? withDefaults(run.access) : agent.draft.access;
  const currentSite = siteOf(activeTab?.url ?? "");
  const accessBlocked: Partial<Record<AccessToggle, string>> = {
    ...(settings?.automation.agentAllowScript ? {} : { scripts: "Off in Settings, under Automation." }),
    ...(settings?.automation.desktopControl ? {} : { desktop: "Off in Settings, under Automation." }),
    ...(settings?.automation.agentMemory === false ? { memory: "Off in Settings, under Automation." } : {}),
    ...(settings?.automation.maxParallelAgents === 0 ? { helpers: "Off in Settings, under Automation." } : {}),
  };
  const history = [...new Set(runs.filter((item) => item.source === "agent" && item.parent === null).map((item) => item.goal))].slice(0, 30);
  const mentionTabs = tabs.windowTabs.filter((tab) => tab.id !== activeTab?.id && /^https?:/i.test(tab.url));
  const workingUrl = mode === "start" ? (activeTab?.url ?? "") : (tabs.windowTabs.find((tab) => tab.id === run?.tabId)?.url ?? activeTab?.url ?? "");
  const aiBlocked = siteBlocked(settings?.aiBlockedSites, workingUrl);

  const submit = (text: string, contextTabs: number[], attachments: AgentAttachment[] = []) => {
    setNotice(null);
    const parsed = parseAgentInput(text, mode);
    if (parsed.kind === "memory") {
      setView("memory");
      return;
    }
    if (parsed.kind === "remember") {
      void agentMemorySave({
        id: "",
        text: parsed.text,
        site: "",
        pinned: false,
        created: 0,
        used: 0,
        category: "fact",
        sourceUrl: activeTab?.url ?? "",
        sourceRun: 0,
        expires: 0,
      })
        .then(() => setNotice("Saved to memory."))
        .catch((reason: unknown) => setNotice(String(reason)));
      return;
    }
    if (parsed.kind === "message") {
      if (!run) {
        return;
      }
      if (active) {
        agent.reply(run.id, parsed.text, attachments);
      } else {
        agent.followUp(run.id, parsed.text, attachments);
      }
      return;
    }
    setView("run");
    void agent.start({
      goal: parsed.goal,
      instructions: parsed.instructions,
      contextTabs,
      background: parsed.background,
      autonomy: parsed.autonomy,
      attachments,
    });
  };


  const chatBody = run ? (
      <div id={`${rootId}-run`} className={page ? "mx-auto flex w-full max-w-[968px] flex-col gap-3" : "flex flex-col gap-3"}>
        <AgentChatPins
          id={`${rootId}-pins`}
          run={run}
          page={page}
          filter={timelineFilter}
          onFilter={setTimelineFilter}
          onExpandAll={() => setFoldAll({ open: true, key: Date.now() })}
          onCollapseAll={() => setFoldAll({ open: false, key: Date.now() })}
          onPlanChange={run.source === "agent" ? (plan) => agent.updatePlan(run.id, plan) : undefined}
        />
        <AgentGoalCard run={run} helpers={agent.childrenOf(run.id)} onSelect={agent.select} />
        {active || run.status === "failed" || run.status === "stopped" ? (
          <AgentCockpit id={`${rootId}-cockpit`} onAction={(action) => agent.cockpit(run.id, action)} />
        ) : null}
        {active ? (
          <AgentLiveView run={run} onShowTab={() => agent.showTab(run.tabId)} screen={!!settings?.automation.desktopControl && access.desktop} />
        ) : null}
        {run.steps.length < run.stepCount && run.steps.length === 0 ? (
          <p id={`${rootId}-steps-loading`} className="text-xs text-zinc-500">
            Loading steps…
          </p>
        ) : null}
        <AgentTimeline
          id={`${rootId}-steps`}
          run={run}
          steps={chatSteps(run)}
          filter={timelineFilter}
          foldAll={foldAll}
          streaming={active ? stream : null}
          onCockpit={(action) => agent.cockpit(run.id, action)}
          onSaveFlow={() => agent.saveFlow(run.id)}
          onNotice={setNotice}
        />
        {!active ? <AgentResult run={run} /> : null}
      </div>
    ) : aiBlocked ? (
      <EmptyState id={`${rootId}-empty`} icon={Bot} title="Not allowed on this site." hint="The agent won't take questions or screenshots of this page." />
    ) : (
      <EmptyState
        id={`${rootId}-empty`}
        icon={Bot}
        title={page ? "What should the agent do?" : "Give the agent a task"}
        hint={
          page
            ? `It can open tabs, run flows, pull data, remember things, and start helpers. Type / for commands or @ to add a tab.`
            : `It works in the selected tab${activeTab ? ` (${activeTab.title || activeTab.url})` : ""} with real clicks and typing. It can open tabs, run your flows, pull data, remember things, and start helpers. Type / for commands or @ to add a tab.`
        }
      />
    );

  const footer = (
    <footer id={`${rootId}-footer`} className={page ? "mx-auto flex w-full max-w-[968px] shrink-0 flex-col gap-2 px-4 pb-4 pt-2" : "flex shrink-0 flex-col gap-2 border-t border-zinc-800 p-3"}>
      {agent.error ? <AgentError id={`${rootId}-error`} text={agent.error} alert /> : null}
      {notice ? (
        <p id={`${rootId}-notice`} role="status" className="text-xs text-zinc-400">
          {notice}
        </p>
      ) : null}
      {run && active && (run.dialog || run.download || run.openedTab) ? (
        <AgentPageStatus run={run} onDialog={(accept, text) => agent.answerDialog(run.id, accept, text)} onShowTab={(tabId) => agent.showTab(tabId)} />
      ) : null}
      {run?.approval ? (
        <AgentApproval
          approval={run.approval}
          autonomy={run.autonomy}
          onAnswer={(allow) => agent.answer(run.id, run.approval!.id, allow)}
          onReply={(text) => agent.reply(run.id, text)}
          onRaise={run.source === "agent" ? (level) => agent.setAutonomy(run.id, level) : undefined}
          onAllowRisk={run.source === "agent" ? (risk) => agent.allowRisk(run.id, risk) : undefined}
        />
      ) : null}
      {screenshotsOn && selectedModel && !visionOk ? (
        <p id={`${rootId}-vision-warning`} role="status" className="rounded-md bg-amber-950/40 px-2 py-1.5 text-[11px] text-amber-200 ring-1 ring-amber-900/60">
          {selectedModel} may not accept screenshots. Turn off “Screenshots to model” in Settings, or pick a vision model.
        </p>
      ) : null}
      {run && active ? (
        <div id={`${rootId}-controls`} className="flex gap-2">
          {run.status === "paused" ? (
            <Button id={`${rootId}-resume`} size="sm" variant="primary" className="flex-1" onClick={() => agent.resume(run.id)}>
              <Play id={`${rootId}-resume-icon`} className="size-3.5" aria-hidden />
              Resume
            </Button>
          ) : run.source === "agent" ? (
            <Button id={`${rootId}-take-over`} size="sm" variant="outline" className="flex-1" onClick={() => agent.takeOver(run.id)}>
              <Hand id={`${rootId}-take-over-icon`} className="size-3.5" aria-hidden />
              Take over
            </Button>
          ) : null}
          <Button
            id={`${rootId}-stop`}
            size="sm"
            variant="destructiveOutline"
            className="flex-1"
            aria-keyshortcuts="Escape"
            title="Stop (Esc)"
            onClick={() => agent.stop(run.id)}
          >
            <Square id={`${rootId}-stop-icon`} className="size-3.5" aria-hidden />
            Stop
            <kbd id={`${rootId}-stop-key`} className="rounded border border-red-800 bg-red-950/70 px-1 font-sans text-[10px] leading-4 text-red-200">
              Esc
            </kbd>
          </Button>
        </div>
      ) : null}
      {aiBlocked ? (
        <p id={`${rootId}-site-blocked`} role="status" className="rounded-md bg-zinc-900 px-3 py-2 text-sm text-zinc-200">
          Not allowed on this site.
        </p>
      ) : run && active && run.source === "mcp" ? null : (
        <AgentComposer
          key={`${mode}-${run?.id ?? "new"}`}
          mode={mode}
          draftKey={`${mode}-${run?.id ?? "new"}`}
          disabled={mode === "start" && !page && !activeTab}
          allowImages={visionOk}
          tabs={mentionTabs}
          history={mode === "start" ? history : []}
          onSubmit={submit}
        />
      )}
      <div id={`${rootId}-chat-meta`} className="relative grid grid-cols-3 items-center gap-1">
        <div id={`${rootId}-chat-meta-limits-slot`} className="justify-self-start">
          <AgentLimitsMenu
            id={`${rootId}-chat-meta-limits`}
            inputTokens={inputTokens}
            outputTokens={outputTokens}
            onInputTokens={(aiMaxInputTokens) => saveChatSettings({ aiMaxInputTokens })}
            onOutputTokens={(aiMaxOutputTokens) => saveChatSettings({ aiMaxOutputTokens })}
          />
        </div>
        <AgentModelPicker
          id={`${rootId}-chat-meta-model`}
          className="min-w-0 w-full justify-self-center text-center"
          value={run?.model || agent.draft.model || savedModel}
          fallback={providerModel}
          provider={provider}
          thinking={thinking}
          onChange={(model) => {
            const current = settingsRef.current;
            if (!current) {
              return;
            }
            const provider = current.aiProvider;
            const slot = current.aiProviders[provider];
            saveChatSettings({
              automation: { ...current.automation, agentModel: model },
              aiProviders: model ? { ...current.aiProviders, [provider]: { ...slot, model } } : current.aiProviders,
            });
            agent.setDraft({ model });
          }}
          onThinking={(thinking) => {
            const current = settingsRef.current;
            if (!current) {
              return;
            }
            const provider = current.aiProvider;
            const slot = current.aiProviders[provider];
            saveChatSettings({
              aiProviders: { ...current.aiProviders, [provider]: { ...slot, thinking } },
            });
          }}
        />
        <div id={`${rootId}-chat-meta-used`} className="flex flex-col items-end justify-self-end text-[10px] leading-tight text-zinc-500">
          {run && run.tokensIn + run.tokensOut > 0 ? (
            <>
              <span id={`${rootId}-chat-meta-used-in`}>Used {formatTokens(run.tokensIn)} in</span>
              <span id={`${rootId}-chat-meta-used-out`}>{formatTokens(run.tokensOut)} out</span>
              {spend !== null ? (
                <span id={`${rootId}-chat-meta-used-cost`} className="text-emerald-400/80" title="Estimated spend at list price">
                  {formatCost(spend)}
                  {run.budget > 0 ? ` · ${Math.min(100, Math.round(((run.tokensIn + run.tokensOut) / run.budget) * 100))}% budget` : ""}
                </span>
              ) : null}
            </>
          ) : (
            <>
              <span id={`${rootId}-chat-meta-used-in`}>Used…</span>
              <span id={`${rootId}-chat-meta-used-out`}>—</span>
            </>
          )}
        </div>
      </div>
    </footer>
  );

  if (page) {
    return (
      <div
        id={rootId}
        className="flex h-screen bg-zinc-950 text-zinc-100"
        style={{ fontFamily: agentFontFamily }}
        onContextMenu={(event) => event.preventDefault()}
      >
        <aside id={`${rootId}-sidebar`} style={{ width: sidebarWidth }} className="flex max-w-[50%] shrink-0 flex-col border-r border-zinc-800 bg-zinc-950">
          <div id={`${rootId}-sidebar-header`} className="flex items-center gap-2 border-b border-zinc-800 px-3 py-2">
            <Bot id={`${rootId}-sidebar-icon`} className="size-4 text-sky-400" aria-hidden />
            <h1 id={`${rootId}-sidebar-title`} className="text-sm font-semibold">
              Agent
            </h1>
          </div>
          <AgentRunList
            runs={runs}
            tabs={tabs.windowTabs}
            selectedId={run?.id ?? null}
            onSelect={openRun}
            onDelete={agent.deleteRun}
            onClear={agent.clear}
            onClose={() => setView("run")}
            onShowTab={agent.showTab}
          />
        </aside>
        <div
          id={`${rootId}-sidebar-resize`}
          role="separator"
          aria-orientation="vertical"
          aria-label="Resize the task list"
          aria-valuemin={SIDEBAR_MIN}
          aria-valuemax={SIDEBAR_MAX}
          aria-valuenow={Math.round(sidebarWidth)}
          tabIndex={0}
          title="Drag to resize. Left and right arrows change the width."
          className={cn("w-1.5 shrink-0 cursor-col-resize touch-none hover:bg-sky-500/40 focus-visible:bg-sky-500/40 focus-visible:outline-none", focusRing)}
          onPointerDown={(event) => {
            if (event.button !== 0) {
              return;
            }
            event.preventDefault();
            const origin = sidebarWidth;
            const start = event.clientX;
            const node = event.currentTarget;
            node.setPointerCapture(event.pointerId);
            const move = (ev: globalThis.PointerEvent) => {
              setSidebarWidth(clampSidebarWidth(origin + ev.clientX - start));
            };
            const end = () => {
              node.removeEventListener("pointermove", move);
              node.removeEventListener("pointerup", end);
              node.removeEventListener("pointercancel", end);
            };
            node.addEventListener("pointermove", move);
            node.addEventListener("pointerup", end);
            node.addEventListener("pointercancel", end);
          }}
          onKeyDown={(event) => {
            const step = event.shiftKey ? 32 : 16;
            if (event.key === "ArrowRight") {
              event.preventDefault();
              setSidebarWidth((width) => clampSidebarWidth(width + step));
            } else if (event.key === "ArrowLeft") {
              event.preventDefault();
              setSidebarWidth((width) => clampSidebarWidth(width - step));
            }
          }}
        />
        <div id={`${rootId}-main`} className="flex min-w-0 flex-1 flex-col">
          <AgentHeader
            tabs={tabs.windowTabs}
            view={view}
            runCount={runs.length}
            onView={setView}
            hideHistory
            hideOpenPage
            page
            onNewTask={() => {
              setView("run");
              agent.newTask();
              setComposerFocus((token) => token + 1);
            }}
            autonomy={autonomy}
            autonomyFor={showRunAutonomy ? "run" : "next"}
            onAutonomy={(level) => (showRunAutonomy ? agent.setAutonomy(run.id, level) : agent.setDraft({ autonomy: level }))}
            access={access}
            onAccess={(next) => (showRunAutonomy ? agent.setAccess(run.id, next) : agent.setDraft({ access: next }))}
            currentSite={currentSite}
            accessBlocked={accessBlocked}
          />
          <main id={`${rootId}-body`} className="min-h-0 flex-1 overflow-y-auto px-4 pb-4 pt-0">
            {view === "memory" ? (
              <div id={`${rootId}-memory-wrap`} className="mx-auto h-full max-w-[968px]">
                <AgentMemoryView enabled={settings?.automation.agentMemory ?? true} onClose={() => setView("run")} />
              </div>
            ) : (
              chatBody
            )}
          </main>
          {view === "memory" ? null : footer}
        </div>
      </div>
    );
  }

  return (
    <div
      id={rootId}
      className="relative flex h-screen flex-col border-l border-zinc-800 bg-zinc-950 text-zinc-100"
      style={{ fontFamily: agentFontFamily }}
      onContextMenu={(event) => event.preventDefault()}
    >
      <AgentHeader
        tabs={tabs.windowTabs}
        view={view}
        runCount={runs.length}
        runId={run?.id ?? null}
        onView={setView}
        onNewTask={() => {
          setView("run");
          agent.newTask();
          setComposerFocus((token) => token + 1);
        }}
        autonomy={autonomy}
        autonomyFor={showRunAutonomy ? "run" : "next"}
        onAutonomy={(level) => (showRunAutonomy ? agent.setAutonomy(run.id, level) : agent.setDraft({ autonomy: level }))}
        access={access}
        onAccess={(next) => (showRunAutonomy ? agent.setAccess(run.id, next) : agent.setDraft({ access: next }))}
        currentSite={currentSite}
        accessBlocked={accessBlocked}
      />

      {view === "runs" ? (
        <AgentRunList
          runs={runs}
          tabs={tabs.windowTabs}
          selectedId={run?.id ?? null}
          onSelect={openRun}
          onDelete={agent.deleteRun}
          onClear={agent.clear}
          onClose={() => setView("run")}
          onShowTab={agent.showTab}
        />
      ) : view === "memory" ? (
        <AgentMemoryView enabled={settings?.automation.agentMemory ?? true} onClose={() => setView("run")} />
      ) : (
        <main id={`${rootId}-body`} className="min-h-0 flex-1 overflow-y-auto px-3 pb-3 pt-0">
          {chatBody}
        </main>
      )}

      {view === "run" ? footer : null}
    </div>
  );
}
