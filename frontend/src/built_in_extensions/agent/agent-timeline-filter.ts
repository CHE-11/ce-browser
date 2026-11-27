export type AgentTimelineFilter = "all" | "chat" | "actions" | "problems";

export const AGENT_TIMELINE_FILTERS: { value: AgentTimelineFilter; label: string }[] = [
  { value: "all", label: "All" },
  { value: "chat", label: "Chat" },
  { value: "actions", label: "Actions" },
  { value: "problems", label: "Problems" },
];
