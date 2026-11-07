import { useCallback, useEffect, useState } from "react";
import type { DebugFeed, DebugFeedTarget, DebugIpcRecord, DebugLogLine } from "@type/browser";
import { debugClear, debugSnapshot, onDebugFeed } from "./browser-client";
import { appendCapped } from "./debug-hud";

/** The HUD's IPC records and log lines: what the backend held when it opened, then the live feed. */
export function useDebugFeed() {
  const [ipc, setIpc] = useState<DebugIpcRecord[]>([]);
  const [logs, setLogs] = useState<DebugLogLine[]>([]);
  const [error, setError] = useState("");

  useEffect(() => {
    let alive = true;
    let loaded = false;
    const early: DebugFeed[] = [];
    // Ids rise across both lists, so anything at or below the snapshot's last id is a repeat.
    let lastIpc = 0;
    let lastLog = 0;

    const apply = (feed: DebugFeed) => {
      if (feed.kind === "ipc") {
        const fresh = feed.records.filter((record) => record.id > lastIpc);
        lastIpc = fresh.at(-1)?.id ?? lastIpc;
        setIpc((current) => appendCapped(current, fresh));
      } else if (feed.kind === "log") {
        const fresh = feed.lines.filter((line) => line.id > lastLog);
        lastLog = fresh.at(-1)?.id ?? lastLog;
        setLogs((current) => appendCapped(current, fresh));
      } else if (feed.target === "ipc") {
        setIpc([]);
      } else {
        setLogs([]);
      }
    };

    const unlisten = onDebugFeed((feed) => {
      if (loaded) {
        apply(feed);
      } else {
        early.push(feed);
      }
    });
    void debugSnapshot()
      .then((snapshot) => {
        if (!alive) {
          return;
        }
        lastIpc = snapshot.ipc.at(-1)?.id ?? 0;
        lastLog = snapshot.logs.at(-1)?.id ?? 0;
        setIpc(snapshot.ipc);
        setLogs(snapshot.logs);
        loaded = true;
        for (const feed of early.splice(0)) {
          apply(feed);
        }
      })
      .catch((reason: unknown) => {
        if (alive) {
          setError(typeof reason === "string" ? reason : "The debug feed could not be read.");
        }
      });
    return () => {
      alive = false;
      void unlisten.then((stop) => stop());
    };
  }, []);

  const clear = useCallback((target: DebugFeedTarget) => {
    void debugClear(target).catch((reason: unknown) => setError(typeof reason === "string" ? reason : "The feed could not be cleared."));
  }, []);

  return { ipc, logs, error, clear };
}
