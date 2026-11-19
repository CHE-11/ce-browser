import { useEffect, useRef, useState } from "react";
import { Bot, Hand, Play, Square } from "lucide-react";
import type { AgentAutonomy, BrowserSettings } from "@type/browser";
import { Button } from "@/components/Button";
import { EmptyState } from "@/components/EmptyState";
import { withDefaults, type AccessToggle } from "@/lib/agent-access";
import { parseAgentInput, type ComposerMode } from "@/lib/agent-commands";
import { AI_INPUT_TOKENS, AI_OUTPUT_TOKENS } from "@/lib/ai/token-limits";
import { agentMemorySave, saveSettings } from "@/lib/browser-client";
import { useAgentStream } from "@/lib/use-agent-stream";
import { isActiveRun, useAgentRun } from "@/lib/use-agent-run";
import { useAppearanceState, useSettings, useTabs } from "@/lib/use-browser";
import { AgentApproval } from "./AgentApproval";
import { AgentComposer } from "./AgentComposer";
import { AgentError } from "./AgentError";
import { AgentGoalCard } from "./AgentGoalCard";
import { AgentHeader, type AgentPanelView } from "./AgentHeader";
import { AgentLiveView } from "./AgentLiveView";
import { AgentPageStatus } from "./AgentPageStatus";
import { AgentMemoryView } from "./AgentMemoryView";
import { AgentPlan } from "./AgentPlan";
import { AgentResult } from "./AgentResult";
import { AgentRunList } from "./AgentRunList";
import { AgentTimeline } from "./AgentTimeline";
import { formatTokens } from "./agent-styles";

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

/** The docked column beside the pages where agent runs start, report each step, and ask for approval. */
export function AgentPanelPage() {
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

  useEffect(() => {
    document.title = run ? `Agent · ${run.goal.slice(0, 40)}` : "Agent";
  }, [run]);

  useEffect(() => {
    if (composerFocus === 0) {
      return;
    }
    const frame = window.requestAnimationFrame(() => {
      const input = document.getElementById("agent-composer-input");
      if (input instanceof HTMLTextAreaElement) {
        input.focus();
      }
    });
    return () => window.cancelAnimationFrame(frame);
  }, [composerFocus]);

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
  const providerModel = settings?.aiProviders[settings.aiProvider]?.model ?? "";
  const savedModel = settings?.automation.agentModel ?? "";
  const modelName = savedModel || providerModel;
  const inputTokens = settings?.aiMaxInputTokens ?? AI_INPUT_TOKENS.fallback;
  const outputTokens = settings?.aiMaxOutputTokens ?? AI_OUTPUT_TOKENS.fallback;
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

  const submit = (text: string, contextTabs: number[]) => {
    setNotice(null);
    const parsed = parseAgentInput(text, mode);
    if (parsed.kind === "memory") {
      setView("memory");
      return;
    }
    if (parsed.kind === "remember") {
      void agentMemorySave({ id: "", text: parsed.text, site: "", pinned: false, created: 0, used: 0 })
        .then(() => setNotice("Saved to memory."))
        .catch((reason: unknown) => setNotice(String(reason)));
      return;
    }
    if (parsed.kind === "message") {
      if (!run) {
        return;
      }
      if (active) {
        agent.reply(run.id, parsed.text);
      } else {
        agent.followUp(run.id, parsed.text);
      }
      return;
    }
    setView("run");
    void agent.start({ goal: parsed.goal, instructions: parsed.instructions, contextTabs, background: parsed.background, autonomy: parsed.autonomy });
  };

  return (
    <div
      id="agent-panel"
      className="relative flex h-screen flex-col border-l border-zinc-800 bg-zinc-950 text-zinc-100"
      onContextMenu={(event) => event.preventDefault()}
    >
      <AgentHeader
        tabs={tabs.windowTabs}
        view={view}
        runCount={runs.length}
        onView={setView}
        onNewTask={() => {
          setView("run");
          agent.newTask();
          setComposerFocus((token) => token + 1);
        }}
        autonomy={autonomy}
        autonomyFor={showRunAutonomy ? "run" : "next"}
        onAutonomy={(level) => (showRunAutonomy ? agent.setAutonomy(run.id, level) : agent.setDraft({ autonomy: level }))}
        model={modelName}
        modelFallback={providerModel}
        onModel={(model) => {
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
          agent.setDraft({ model: "" });
        }}
        inputTokens={inputTokens}
        outputTokens={outputTokens}
        onInputTokens={(aiMaxInputTokens) => saveChatSettings({ aiMaxInputTokens })}
        onOutputTokens={(aiMaxOutputTokens) => saveChatSettings({ aiMaxOutputTokens })}
        access={access}
        onAccess={(next) => (showRunAutonomy ? agent.setAccess(run.id, next) : agent.setDraft({ access: next }))}
        currentSite={currentSite}
        accessBlocked={accessBlocked}
      />

      {view === "runs" ? (
        <AgentRunList
          runs={runs}
          selectedId={run?.id ?? null}
          onSelect={(id) => {
            agent.select(id);
            setView("run");
          }}
          onDelete={agent.deleteRun}
          onClear={agent.clear}
          onClose={() => setView("run")}
        />
      ) : view === "memory" ? (
        <AgentMemoryView enabled={settings?.automation.agentMemory ?? true} onClose={() => setView("run")} />
      ) : (
        <main id="agent-panel-body" className="min-h-0 flex-1 overflow-y-auto px-3 py-3">
          {run ? (
            <div id="agent-panel-run" className="flex flex-col gap-3">
              <AgentGoalCard run={run} helpers={agent.childrenOf(run.id)} onShowTab={() => agent.showTab(run.tabId)} onSelect={agent.select} />
              <AgentPlan id="agent-panel-plan" plan={run.plan} />
              {active ? (
                <AgentLiveView run={run} onShowTab={() => agent.showTab(run.tabId)} screen={!!settings?.automation.desktopControl && access.desktop} />
              ) : null}
              {run.steps.length < run.stepCount && run.steps.length === 0 ? (
                <p id="agent-panel-steps-loading" className="text-xs text-zinc-500">
                  Loading steps…
                </p>
              ) : null}
              <AgentTimeline id="agent-panel-steps" steps={run.steps} streaming={active ? stream : null} />
              {!active ? <AgentResult run={run} onSaveFlow={() => agent.saveFlow(run.id)} onError={setNotice} /> : null}
            </div>
          ) : aiBlocked ? (
            <EmptyState id="agent-panel-empty" icon={Bot} title="Not allowed on this site." hint="The agent won't take questions or screenshots of this page." />
          ) : (
            <EmptyState
              id="agent-panel-empty"
              icon={Bot}
              title="Give the agent a task"
              hint={`It works in the selected tab${activeTab ? ` (${activeTab.title || activeTab.url})` : ""} with real clicks and typing. It can open tabs, run your flows, pull data, remember things, and start helpers. Type / for commands or @ to add a tab.`}
            />
          )}
        </main>
      )}

      <footer id="agent-panel-footer" className="flex shrink-0 flex-col gap-2 border-t border-zinc-800 p-3">
        {agent.error ? <AgentError id="agent-panel-error" text={agent.error} alert /> : null}
        {notice ? (
          <p id="agent-panel-notice" role="status" className="text-xs text-zinc-400">
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
          />
        ) : null}
        {run && active ? (
          <div id="agent-panel-controls" className="flex gap-2">
            {run.status === "paused" ? (
              <Button id="agent-panel-resume" size="sm" variant="primary" className="flex-1" onClick={() => agent.resume(run.id)}>
                <Play id="agent-panel-resume-icon" className="size-3.5" aria-hidden />
                Resume
              </Button>
            ) : run.source === "agent" ? (
              <Button id="agent-panel-take-over" size="sm" variant="outline" className="flex-1" onClick={() => agent.takeOver(run.id)}>
                <Hand id="agent-panel-take-over-icon" className="size-3.5" aria-hidden />
                Take over
              </Button>
            ) : null}
            <Button
              id="agent-panel-stop"
              size="sm"
              variant="destructiveOutline"
              className="flex-1"
              aria-keyshortcuts="Escape"
              title="Stop (Esc)"
              onClick={() => agent.stop(run.id)}
            >
              <Square id="agent-panel-stop-icon" className="size-3.5" aria-hidden />
              Stop
              <kbd id="agent-panel-stop-key" className="rounded border border-red-800 bg-red-950/70 px-1 font-sans text-[10px] leading-4 text-red-200">
                Esc
              </kbd>
            </Button>
          </div>
        ) : null}
        {aiBlocked ? (
          <p id="agent-panel-site-blocked" role="status" className="rounded-md bg-zinc-900 px-3 py-2 text-sm text-zinc-200">
            Not allowed on this site.
          </p>
        ) : run && active && run.source === "mcp" ? null : (
          <AgentComposer
            key={`${mode}-${run?.id ?? "new"}`}
            mode={mode}
            draftKey={`${mode}-${run?.id ?? "new"}`}
            disabled={mode === "start" && !activeTab}
            suggestions={settings?.agentSuggestions ?? ["Summarize this page and list the key facts"]}
            tabs={mentionTabs}
            history={mode === "start" ? history : []}
            onSubmit={submit}
          />
        )}
        <div id="agent-panel-chat-meta" className="flex flex-row items-center justify-between gap-1">
          <p id="agent-panel-chat-meta-limits" className="text-[10px] text-zinc-500">
            {`${inputTokens.toLocaleString()} in · ${outputTokens.toLocaleString()} out`}
          </p>
          <p id="agent-panel-chat-meta-model" className="truncate text-[10px] text-zinc-500">
            {modelName || "No model"}
          </p>
          {run && run.tokensIn + run.tokensOut > 0 ? (
            <p id="agent-panel-chat-meta-used" className="text-[10px] text-zinc-500">
              Used {formatTokens(run?.tokensIn ?? 0)} in / {formatTokens(run?.tokensOut ?? 0)} out
            </p>
          ) : (
            <p id="agent-panel-chat-meta-used" className="text-[10px] text-zinc-500">
              Used...
            </p>
          )}
        </div>

      </footer>
    </div>
  );
}
