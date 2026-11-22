import { useState } from "react";
import { ConsoleRow } from "./ConsoleRow";
import { ConsoleRun } from "./ConsoleRun";
import { collapseRuns, type Entry } from "./log";

type ConsoleLogProps = {
  entries: Entry[];
  tabId: number;
  timestamps: boolean;
  pageOrigin: string;
  closed: Set<string>;
  onToggleGroup: (key: string) => void;
  onStatus: (text: string | null) => void;
};

/** The scrolling list of console rows. Repeated messages fold into one row with a count. */
export function ConsoleLog({ entries, ...row }: ConsoleLogProps) {
  const [openRuns, setOpenRuns] = useState<Set<string>>(() => new Set());

  return (
    <div id="console-log" className="flex flex-col py-1">
      {collapseRuns(entries).map((run) => {
        const first = run.entries[0];
        const last = run.entries[run.entries.length - 1];
        if (run.entries.length === 1 || run.identical) {
          return <ConsoleRow key={first.key} entry={last} count={run.entries.length} {...row} />;
        }
        return (
          <ConsoleRun
            key={first.key}
            run={run}
            open={openRuns.has(first.key)}
            onToggle={() =>
              setOpenRuns((current) => {
                const next = new Set(current);
                if (next.has(first.key)) next.delete(first.key);
                else next.add(first.key);
                return next;
              })
            }
            {...row}
          />
        );
      })}
    </div>
  );
}
