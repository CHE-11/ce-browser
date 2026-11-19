import { useEffect, useState } from "react";
import { Clock, Coins, DollarSign, MousePointerClick } from "lucide-react";
import type { AgentRun } from "@type/browser";
import { isActiveRun } from "@/lib/use-agent-run";
import { useSettings } from "@/lib/use-browser";
import { formatCost, runCost, useModelPrice } from "@/lib/use-model-prices";
import { formatElapsed, formatTokens } from "./agent-styles";

function useNow(ticking: boolean) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!ticking) {
      return;
    }
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [ticking]);
  return now;
}

/** Actions, tokens, cost and time a run used. */
export function AgentUsage({ id, run }: { id: string; run: AgentRun }) {
  const settings = useSettings();
  const active = isActiveRun(run);
  const now = useNow(active);
  const end = run.finished || now;
  const tokens = run.tokensIn + run.tokensOut;
  const model = run.model || settings?.automation.agentModel || settings?.aiProviders[settings.aiProvider]?.model || "";
  const price = useModelPrice(run.source === "agent" && tokens > 0 ? model : "");
  const cost = runCost(price, run.tokensIn, run.tokensOut);

  return (
    <div id={id} className="flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-zinc-500">
      <span id={`${id}-actions`} className="inline-flex items-center gap-1" title="Actions taken">
        <MousePointerClick id={`${id}-actions-icon`} className="size-3" aria-hidden />
        {run.actions}
      </span>
      {tokens > 0 ? (
        <span id={`${id}-tokens`} className="inline-flex items-center gap-1" title={`${run.tokensIn.toLocaleString()} tokens in, ${run.tokensOut.toLocaleString()} out`}>
          <Coins id={`${id}-tokens-icon`} className="size-3" aria-hidden />
          {formatTokens(tokens)}
          {run.budget > 0 ? ` / ${formatTokens(run.budget)}` : ""}
        </span>
      ) : null}
      {cost !== null ? (
        <span id={`${id}-cost`} className="inline-flex items-center gap-1 text-emerald-400/80" title={`About ${formatCost(cost)} at ${model}'s list price`}>
          <DollarSign id={`${id}-cost-icon`} className="size-3" aria-hidden />
          {formatCost(cost).replace(/^\$/, "")}
        </span>
      ) : null}
      <span id={`${id}-time`} className="inline-flex items-center gap-1" title="Time">
        <Clock id={`${id}-time-icon`} className="size-3" aria-hidden />
        {formatElapsed(end - run.started)}
      </span>
      {run.model ? (
        <span id={`${id}-model`} className="truncate" title="Model">
          {run.model}
        </span>
      ) : null}
    </div>
  );
}
