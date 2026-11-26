import { useEffect, useRef, useState } from "react";
import { Button } from "@/components/Button";
import { cancelSpeedTest, lastSpeedTest, onSpeedTestProgress, runSpeedTest } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import type { SpeedTestPhase, SpeedTestProgress, SpeedTestResult } from "@type/browser";
import { formatRate } from "./format";

function phaseLabel(phase: SpeedTestPhase) {
  if (phase === "latency") {
    return "Checking latency";
  }
  if (phase === "download") {
    return "Downloading";
  }
  return "Uploading";
}

function testedAgo(at: number, now: number) {
  const seconds = Math.max(0, Math.round((now - at) / 1000));
  if (seconds < 10) {
    return "Tested just now";
  }
  if (seconds < 60) {
    return `Tested ${seconds}s ago`;
  }
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) {
    return `Tested ${minutes} min ago`;
  }
  const hours = Math.round(minutes / 60);
  return `Tested ${hours} hr ago`;
}

function failureMessage(reason: unknown) {
  if (typeof reason === "string") {
    return reason;
  }
  if (reason instanceof Error && reason.message) {
    return reason.message;
  }
  return "The speed test failed.";
}

function stopped(message: string) {
  return message.toLowerCase().includes("stopped");
}

export function SpeedTest({ online }: { online: boolean }) {
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<SpeedTestProgress | null>(null);
  const [result, setResult] = useState<SpeedTestResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const pending = useRef(false);

  useEffect(() => {
    let gone = false;
    void lastSpeedTest()
      .then((saved) => {
        if (!gone) {
          setResult(saved);
        }
      })
      .catch(() => undefined);
    return () => {
      gone = true;
    };
  }, []);

  useEffect(() => {
    const listen = onSpeedTestProgress((next) => setProgress(next));
    return () => {
      void listen.then((unlisten) => unlisten());
      void cancelSpeedTest();
    };
  }, []);

  useEffect(() => {
    if (!result) {
      return;
    }
    const timer = window.setInterval(() => setNow(Date.now()), 15000);
    return () => window.clearInterval(timer);
  }, [result]);

  function start() {
    if (pending.current) {
      return;
    }
    pending.current = true;
    setRunning(true);
    setError(null);
    setProgress(null);
    void runSpeedTest()
      .then((next) => {
        setResult(next);
        setNow(Date.now());
        setProgress(null);
      })
      .catch((reason: unknown) => {
        const message = failureMessage(reason);
        if (!stopped(message)) {
          setError(message);
        }
      })
      .finally(() => {
        pending.current = false;
        setRunning(false);
      });
  }

  const live = running ? progress : null;

  return (
    <div id="speed-test" className="mt-2 border-t border-zinc-800 pt-2">
      <div id="speed-test-heading" className="flex items-center justify-between gap-3 py-1">
        <span id="speed-test-title" className="text-xs font-medium text-zinc-200">
          Speed test
        </span>
        <Button
          id="speed-test-run"
          size="xs"
          variant={running ? "outline" : "primary"}
          disabled={!running && !online}
          title={online ? "Measure download and upload capacity" : "Connect to the internet to run a speed test."}
          onClick={running ? () => void cancelSpeedTest() : start}
        >
          {running ? "Stop" : "Run"}
        </Button>
      </div>
      {live ? (
        <p id="speed-test-live" className="py-1 text-xs text-zinc-300">
          {phaseLabel(live.phase)}
          {live.phase === "latency" ? "" : ` · ${formatRate(live.bps)}`}
        </p>
      ) : null}
      {error ? (
        <p id="speed-test-error" className="py-1 text-xs text-red-300">
          {error}
        </p>
      ) : null}
      {result && !running ? (
        <div id="speed-test-result" className="flex flex-col">
          <p id="speed-test-when" className="py-1 text-xs text-zinc-500">
            {testedAgo(result.at, now)}
          </p>
          <ResultRow
            id="speed-test-down"
            label="Download"
            value={formatRate(result.downBps)}
            hint="How fast a download from the internet can go. The rows above are traffic already moving, not this capacity."
          />
          <ResultRow
            id="speed-test-up"
            label="Upload"
            value={formatRate(result.upBps)}
            hint="How fast an upload to the internet can go. The rows above are traffic already moving, not this capacity."
          />
          <ResultRow
            id="speed-test-latency"
            label="Latency"
            value={result.latencyMs == null ? "—" : `${result.latencyMs} ms`}
            hint="Round trip to the speed test server, measured before the bulk transfer."
          />
        </div>
      ) : null}
      {!result && !running && !error ? (
        <p id="speed-test-hint" className="py-1 text-xs text-zinc-500">
          Measures how fast this connection can download and upload.
        </p>
      ) : null}
    </div>
  );
}

function ResultRow({ id, label, value, hint }: { id: string; label: string; value: string; hint: string }) {
  return (
    <div id={id} className="flex items-baseline justify-between gap-3 py-1">
      <span id={`${id}-label`} className="text-xs text-zinc-400">
        {label}
      </span>
      <span id={`${id}-value`} className={cn("truncate text-xs font-medium tabular-nums text-zinc-100", "cursor-help")} title={hint}>
        {value}
      </span>
    </div>
  );
}
