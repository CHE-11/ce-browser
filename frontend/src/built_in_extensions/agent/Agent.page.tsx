import { AgentWorkspace } from "./AgentWorkspace";

type Props = {
  params: URLSearchParams;
};

/** Full-page agent chat at `ce://agent`. */
export function AgentPage({ params }: Props) {
  return <AgentWorkspace layout="page" params={params} />;
}
