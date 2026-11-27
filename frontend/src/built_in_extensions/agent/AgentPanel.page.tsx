import { AgentWorkspace } from "./AgentWorkspace";

/** The docked column beside the pages where agent runs start, report each step, and ask for approval. */
export function AgentPanelPage() {
  return <AgentWorkspace layout="panel" />;
}
