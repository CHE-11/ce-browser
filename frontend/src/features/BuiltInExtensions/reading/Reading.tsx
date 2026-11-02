import { useEffect, useState } from "react";
import { BookOpen } from "lucide-react";
import { PopupFrame } from "@/components/PopupFrame";
import { readingStats } from "@/lib/browser-client";
import type { ReadingStats, TabInfo } from "@type/browser";
import { PopoverHeader } from "../PopoverParts";
import { readPopoverTab } from "../shared";
import { ToolButton } from "../ToolButton";

export function ReadingButton({ activeTab }: { activeTab: TabInfo | undefined }) {
  return (
    <ToolButton
      id="reading"
      label="Reading"
      kind="reading"
      icon={<BookOpen id="address-reading-icon" aria-hidden />}
      activeTab={activeTab}
    />
  );
}

export function ReadingPopover({ params }: { params: URLSearchParams }) {
  const tab = readPopoverTab(params);
  const [stats, setStats] = useState<ReadingStats | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    void readingStats(tab.id)
      .then(setStats)
      .catch((reason: unknown) => setError(reason instanceof Error ? reason.message : "Couldn't read this page."));
  }, [tab.id]);

  return (
    <PopupFrame id="reading-popover" width={320} maxHeight={240}>
      <PopoverHeader id="reading-header" icon={<BookOpen aria-hidden />} title="Reading" subtitle={tab.title || tab.url} />
      <div id="reading-body" className="grid grid-cols-3 gap-2 px-3 pb-3 text-center">
        {error ? (
          <p id="reading-error" className="col-span-3 text-xs text-amber-300">
            {error}
          </p>
        ) : (
          <>
            <Stat id="reading-words" label="Words" value={stats ? String(stats.words) : "…"} />
            <Stat id="reading-minutes" label="Minutes" value={stats ? String(stats.minutes) : "…"} />
            <Stat id="reading-ease" label="Ease" value={stats ? String(stats.ease) : "…"} />
          </>
        )}
      </div>
    </PopupFrame>
  );
}

function Stat({ id, label, value }: { id: string; label: string; value: string }) {
  return (
    <div id={id} className="rounded-lg bg-zinc-900 px-2 py-3">
      <p id={`${id}-value`} className="text-lg font-semibold">
        {value}
      </p>
      <p id={`${id}-label`} className="text-xs text-zinc-500">
        {label}
      </p>
    </div>
  );
}
