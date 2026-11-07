import { useEffect, useRef } from "react";
import { Circle, ExternalLink, Square, X } from "lucide-react";
import type { FlowRecording, TabInfo } from "@type/browser";
import { Button } from "@/components/Button";
import { IconButton } from "@/components/IconButton";
import { stepKindLabel, stepSummary } from "./flow-model";

type RecordingViewProps = {
  recording: FlowRecording;
  targetName: string | null;
  tab: TabInfo | undefined;
  onShowTab: () => void;
  onRemove: (index: number) => void;
  onStop: (save: boolean) => void;
};

/** What the recorder has caught so far, step by step as it arrives. */
export function RecordingView({ recording, targetName, tab, onShowTab, onRemove, onStop }: RecordingViewProps) {
  const listRef = useRef<HTMLOListElement>(null);
  const count = recording.steps.length;

  useEffect(() => {
    const list = listRef.current;
    list?.lastElementChild?.scrollIntoView({ block: "nearest" });
  }, [count]);

  return (
    <section id="recording-view" className="flex min-w-0 flex-1 flex-col overflow-hidden">
      <header id="recording-view-header" className="flex items-center gap-3 border-b border-zinc-800 px-6 py-3">
        <Circle id="recording-view-dot" className="size-3 shrink-0 animate-pulse fill-red-500 text-red-500" aria-hidden />
        <div id="recording-view-heading" className="min-w-0 flex-1">
          <h1 id="recording-view-title" className="truncate text-lg font-semibold text-zinc-100">
            {targetName ? `Recording more steps for ${targetName}` : "Recording a new flow"}
          </h1>
          <p id="recording-view-tab" className="truncate text-xs text-zinc-500">
            {tab ? `Do the task in ${tab.title || tab.url}. Each click, field and key press shows up here.` : "The recorded tab is closed."}
          </p>
        </div>
        {tab ? (
          <Button id="recording-view-show" size="sm" variant="ghost" onClick={onShowTab}>
            <ExternalLink id="recording-view-show-icon" className="size-3.5" aria-hidden />
            Go to the page
          </Button>
        ) : null}
        <Button id="recording-view-discard" size="sm" variant="ghost" onClick={() => onStop(false)}>
          Discard
        </Button>
        <Button id="recording-view-save" size="sm" variant="destructive" onClick={() => onStop(true)}>
          <Square id="recording-view-save-icon" className="size-3" aria-hidden />
          {count ? `Stop and save ${count} ${count === 1 ? "step" : "steps"}` : "Stop"}
        </Button>
      </header>
      <div id="recording-view-body" className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
        {count === 0 ? (
          <p id="recording-view-empty" className="text-sm text-zinc-500">
            Nothing yet. Switch to the page and start clicking.
          </p>
        ) : (
          <ol id="recording-view-steps" ref={listRef} className="flex flex-col gap-1.5">
            {recording.steps.map((step, index) => (
              <li id={`recording-step-${index}`} key={`${index}-${step.kind}`} className="flex items-center gap-3 rounded-md border border-zinc-800 bg-zinc-900/60 px-3 py-2">
                <span id={`recording-step-${index}-number`} className="inline-flex size-6 shrink-0 items-center justify-center rounded-full bg-zinc-800 text-xs text-zinc-300">
                  {index + 1}
                </span>
                <span id={`recording-step-${index}-kind`} className="w-28 shrink-0 text-xs text-zinc-500">
                  {stepKindLabel(step.kind)}
                </span>
                <span id={`recording-step-${index}-summary`} className="min-w-0 flex-1 truncate text-sm text-zinc-200">
                  {stepSummary(step)}
                  {step.note ? (
                    <span id={`recording-step-${index}-note`} className="block truncate text-xs text-amber-300/90">
                      {step.note}
                    </span>
                  ) : null}
                </span>
                <IconButton id={`recording-step-${index}-remove`} label="Remove this step" className="size-7" onClick={() => onRemove(index)}>
                  <X id={`recording-step-${index}-remove-icon`} aria-hidden />
                </IconButton>
              </li>
            ))}
          </ol>
        )}
      </div>
    </section>
  );
}
