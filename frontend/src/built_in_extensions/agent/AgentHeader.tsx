import { Brain, ExternalLink, History, Plus, Settings } from "lucide-react";
import type { AgentAccess, AgentAutonomy, TabInfo } from "@type/browser";
import { IconButton } from "@/components/IconButton";
import type { AccessToggle } from "@/lib/agent-access";
import { activateTab, createTab, navigate } from "@/lib/browser-client";
import { AgentAutonomyPicker } from "./AgentAutonomyPicker";
import { AgentPermissionsMenu } from "./AgentPermissionsMenu";

const AI_SETTINGS_URL = "ce://settings?focus=settings-ai";
const AGENT_PAGE_URL = "ce://agent";

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

/** Opens the full agent chat page on the current chat, reusing an agent tab when possible. */
function openAgentPage(tabs: TabInfo[], runId: number | null) {
  const url = runId != null ? `${AGENT_PAGE_URL}?run=${runId}` : AGENT_PAGE_URL;
  const open = tabs.find((tab) => tab.url === AGENT_PAGE_URL || tab.url.startsWith("ce://agent?") || tab.url.startsWith(`${AGENT_PAGE_URL}?`));
  if (!open) {
    void createTab(url);
    return;
  }
  void activateTab(open.id).then(() => navigate(open.id, url));
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
  access: AgentAccess;
  onAccess: (access: AgentAccess) => void;
  currentSite: string;
  accessBlocked: Partial<Record<AccessToggle, string>>;
  /** Hide the history button when the page already shows a task list. */
  hideHistory?: boolean;
  /** Hide the open-page button when already on `ce://agent`. */
  hideOpenPage?: boolean;
  /** Opens `ce://agent` on this chat. Null opens a blank composer. */
  runId?: number | null;
};

/** Options bar: permissions, centered autonomy, and view/settings icons. */
export function AgentHeader({
  tabs,
  view,
  runCount,
  onView,
  onNewTask,
  autonomy,
  autonomyFor,
  onAutonomy,
  access,
  onAccess,
  currentSite,
  accessBlocked,
  hideHistory = false,
  hideOpenPage = false,
  runId = null,
}: Props) {
  return (
    <div id="agent-header" className="shrink-0 border-b border-zinc-800 pt-5">
      <div id="agent-panel-options" className="grid grid-cols-[1fr_auto_1fr] items-center gap-2 px-3 py-2">
        <div id="agent-panel-options-left" className="min-w-0 justify-self-start">
          <AgentPermissionsMenu
            id="agent-panel-chat-settings"
            access={access}
            onAccess={onAccess}
            scope={autonomyFor}
            currentSite={currentSite}
            blockedBySettings={accessBlocked}
          />
        </div>
        <AgentAutonomyPicker id="agent-panel-autonomy" value={autonomy} onChange={onAutonomy} />
        <div id="agent-panel-options-right" className="flex min-w-[110px] items-center justify-end gap-0.5">
          <IconButton id="agent-panel-memory" label="What the agent remembers" className="size-7" active={view === "memory"} onClick={() => onView(view === "memory" ? "run" : "memory")}>
            <Brain id="agent-panel-memory-icon" aria-hidden />
          </IconButton>
          {hideHistory ? null : (
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
          )}
          <IconButton id="agent-panel-new" label="New task" className="size-7" onClick={onNewTask}>
            <Plus id="agent-panel-new-icon" aria-hidden />
          </IconButton>
          <IconButton id="agent-panel-settings" label="AI/Agent settings" className="size-7" onClick={() => openAiSettings(tabs)}>
            <Settings id="agent-panel-settings-icon" aria-hidden />
          </IconButton>
          {hideOpenPage ? null : (
            <IconButton
              id="agent-panel-open-page"
              label={runId != null ? "Open this chat full page" : "Open agent page"}
              className="size-7"
              onClick={() => openAgentPage(tabs, runId)}
            >
              <ExternalLink id="agent-panel-open-page-icon" aria-hidden />
            </IconButton>
          )}
        </div>
      </div>
    </div>
  );
}
