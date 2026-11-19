import { useEffect } from "react";
import { NetworkLog } from "./NetworkLog";

type NetworkPageProps = {
  tabId: number;
};

export function NetworkPage({ tabId }: NetworkPageProps) {
  useEffect(() => {
    document.title = "Network";
  }, []);

  return (
    <main id="network-page" className="flex h-full min-h-0 flex-col bg-zinc-900 text-zinc-100">
      <NetworkLog tabId={tabId} variant="window" />
    </main>
  );
}
