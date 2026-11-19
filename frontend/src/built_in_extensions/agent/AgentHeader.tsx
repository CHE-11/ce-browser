import { Bot, Brain, History, Plus, Settings } from "lucide-react";
import type { AgentAccess, AgentAutonomy, TabInfo } from "@type/browser";
import { IconButton } from "@/components/IconButton";
import type { AccessToggle } from "@/lib/agent-access";
import { activateTab, createTab, navigate } from "@/lib/browser-client";
import { AgentAutonomyPicker } from "./AgentAutonomyPicker";
import { AgentChatSettingsMenu } from "./AgentChatSettingsMenu";

const AI_SETTINGS_URL = "ce://settings?focus=settings-ai";

export type AgentPanelView = "run" | "runs" | "memory";

/** Opens the AI/Agent settings section, reusing a settings tab in this window. */
function openAiSettings(tabs: TabInfo[]) {
  const open = tabs.find((tab) => tab.url.startsWith("ce://settings"));
  if (!open) {
    void createTab(AI_SETTINGS_URL);
    return;
  }
  void activateTab(open.id).then(() => navigate(open.id, AI_SETTINGS_URL));
}

type Props = {
  tabs: TabInfo[];
  view: AgentPanelView;
  runCount: number;
  onView: (view: AgentPanelView) => void;
  onNewTask: () => void;
  autonomy: AgentAutonomy;
  /** Says whether the autonomy applies to the run on show or to the next task. */
  autonomyFor: "run" | "next";
  onAutonomy: (autonomy: AgentAutonomy) => void;
  /** Saved agent model. Blank uses the provider model. */
  model: string;
  modelFallback: string;
  onModel: (model: string) => void;
  inputTokens: number;
  outputTokens: number;
  onInputTokens: (value: number) => void;
  onOutputTokens: (value: number) => void;
  access: AgentAccess;
  onAccess: (access: AgentAccess) => void;
  currentSite: string;
  accessBlocked: Partial<Record<AccessToggle, string>>;
};

/** Title bar with views and the options for the next task or the running one. */
export function AgentHeader({
  tabs,
  view,
  runCount,
  onView,
  onNewTask,
  autonomy,
  autonomyFor,
  onAutonomy,
  model,
  modelFallback,
  onModel,
  inputTokens,
  outputTokens,
  onInputTokens,
  onOutputTokens,
  access,
  onAccess,
  currentSite,
  accessBlocked,
}: Props) {
  return (
    <div id="agent-header" className="shrink-0 border-b border-zinc-800">
      <header id="agent-panel-header" className="flex h-[var(--toolbar-height)] items-center gap-1 pl-3 pr-1.5">
        <Bot id="agent-panel-header-icon" className="size-4 text-sky-400" aria-hidden />
        <h1 id="agent-panel-title" className="ml-1 text-sm font-semibold">
          Agent
        </h1>
        <div id="agent-panel-header-spacer" className="min-w-0 flex-1" />
        <IconButton id="agent-panel-memory" label="What the agent remembers" className="size-7" active={view === "memory"} onClick={() => onView(view === "memory" ? "run" : "memory")}>
          <Brain id="agent-panel-memory-icon" aria-hidden />
        </IconButton>
        <IconButton
          id="agent-panel-history"
          label={`Tasks (${runCount})`}
          className="size-7"
          active={view === "runs"}
          disabled={runCount === 0}
          onClick={() => onView(view === "runs" ? "run" : "runs")}
        >
          <History id="agent-panel-history-icon" aria-hidden />
        </IconButton>
        <IconButton id="agent-panel-new" label="New task" className="size-7" onClick={onNewTask}>
          <Plus id="agent-panel-new-icon" aria-hidden />
        </IconButton>
        <IconButton id="agent-panel-settings" label="AI/Agent settings" className="size-7" onClick={() => openAiSettings(tabs)}>
          <Settings id="agent-panel-settings-icon" aria-hidden />
        </IconButton>
      </header>
      <div id="agent-panel-options" className="flex items-center justify-between gap-2 px-3 pb-2">
        <AgentChatSettingsMenu
          id="agent-panel-chat-settings"
          model={model}
          modelFallback={modelFallback}
          onModel={onModel}
          inputTokens={inputTokens}
          outputTokens={outputTokens}
          onInputTokens={onInputTokens}
          onOutputTokens={onOutputTokens}
          access={access}
          onAccess={onAccess}
          scope={autonomyFor}
          currentSite={currentSite}
          blockedBySettings={accessBlocked}
        />
        <AgentAutonomyPicker id="agent-panel-autonomy" value={autonomy} onChange={onAutonomy} />

      </div>
    </div>
  );
}
