import { useEffect, useRef, useState } from "react";
import { ArrowUpRight, Camera, CameraOff, ChevronDown, ChevronUp, Eraser, Highlighter, Mic, MicOff, Pause, Pencil, Play, Settings, Square, Trash2, Undo2 } from "lucide-react";
import {
  getSettings,
  onScreenRecording,
  pauseScreenRecording,
  resumeScreenRecording,
  saveSettings,
  screenRecordingStatus,
  clearRecordingStrokes,
  setRecordingCamera,
  setRecordingCameraCorner,
  setRecordingControlsMode,
  setRecordingCursor,
  setRecordingDrawing,
  setRecordingFocus,
  setRecordingMicMuted,
  setRecordingStyle,
  stopScreenRecording,
  undoRecordingStroke,
} from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import type { RecordingCameraCorner, RecordingCursor, RecordingFocus, RecordingSettings, ScreenRecordingStatus } from "@type/browser";
import { clock } from "./time";

const COLORS = ["#ef4444", "#22c55e", "#f97316", "#171717", "#facc15", "#f5f5f5", "#a1a1aa"];
const TOOLS = [
  { id: "pen", label: "Pen", icon: Pencil },
  { id: "arrow", label: "Arrow", icon: ArrowUpRight },
  { id: "rectangle", label: "Shape", icon: Square },
  { id: "highlighter", label: "Highlighter", icon: Highlighter },
  { id: "eraser", label: "Eraser", icon: Eraser },
] as const;
const CORNERS: { id: RecordingCameraCorner; label: string }[] = [
  { id: "top-left", label: "Top left" },
  { id: "top-right", label: "Top right" },
  { id: "bottom-left", label: "Bottom left" },
  { id: "bottom-right", label: "Bottom right" },
];
const CURSORS: { id: RecordingCursor; label: string }[] = [
  { id: "system", label: "System" },
  { id: "hidden", label: "Hidden" },
  { id: "circle", label: "Circle" },
];
const FOCUSES: { id: RecordingFocus; label: string }[] = [
  { id: "off", label: "Off" },
  { id: "1.5", label: "1.5×" },
  { id: "2", label: "2×" },
];
const EMPTY: ScreenRecordingStatus = {
  phase: "countdown",
  elapsedMs: 0,
  countdown: 3,
  id: "",
  error: null,
  path: null,
  camera: false,
  microphone: true,
  micMuted: false,
  micLevel: 0,
  drawing: false,
  strokes: 0,
  tool: "pen",
  color: "#ef4444",
  ownerWindow: "",
};

const DEFAULT_PREFS: RecordingSettings = {
  microphone: true,
  systemAudio: true,
  camera: false,
  countdown: true,
  resolution: "1440",
  cameraCorner: "bottom-left",
  cursor: "system",
  focus: "off",
};

const METER = [0.12, 0.28, 0.46, 0.66, 0.86];
const DRAW_CHOICE = "ce-recording-draw";
const DRAW_UNDO = "ce-recording-undo";
const DRAW_CLEAR = "ce-recording-clear";

function rememberDraw(tool: string, color: string, width: number) {
  localStorage.setItem(DRAW_CHOICE, JSON.stringify({ tool, color, width, at: Date.now() }));
}

function requestUndo() {
  const next = Number(localStorage.getItem(DRAW_UNDO) || 0) + 1;
  localStorage.setItem(DRAW_UNDO, String(next));
  void undoRecordingStroke();
}

function requestClear() {
  localStorage.setItem(DRAW_CLEAR, String(Date.now()));
  void clearRecordingStrokes();
}

export function RecordingControls() {
  const barRef = useRef<HTMLDivElement>(null);
  const stamp = useRef({ elapsed: 0, at: performance.now() });
  const [status, setStatus] = useState<ScreenRecordingStatus>(EMPTY);
  const [tick, setTick] = useState(0);
  const [hidden, setHidden] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [prefs, setPrefs] = useState<RecordingSettings>(DEFAULT_PREFS);
  const [drawWidth, setDrawWidth] = useState(2);

  const apply = (next: ScreenRecordingStatus) => {
    stamp.current = { elapsed: next.elapsedMs, at: performance.now() };
    setStatus(next);
  };

  useEffect(() => {
    document.title = "Recording";
    document.documentElement.dataset.recordingClear = "true";
    let stop = false;
    const pull = () => {
      void screenRecordingStatus()
        .then((next) => {
          if (!stop) apply(next);
        })
        .catch(() => undefined);
    };
    pull();
    const poll = window.setInterval(pull, 200);
    const clockTick = window.setInterval(() => setTick(performance.now()), 200);
    const listen = onScreenRecording((next) => {
      if (!stop) apply(next);
    });
    void getSettings()
      .then((settings) => {
        if (settings.recording) setPrefs({ ...DEFAULT_PREFS, ...settings.recording });
      })
      .catch(() => undefined);
    return () => {
      stop = true;
      delete document.documentElement.dataset.recordingClear;
      window.clearInterval(poll);
      window.clearInterval(clockTick);
      void listen.then((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    const mode = hidden ? "hidden" : settingsOpen ? "settings" : status.drawing ? "tools" : "pill";
    void setRecordingControlsMode(mode).catch(() => undefined);
  }, [hidden, settingsOpen, status.drawing]);

  useEffect(() => {
    if (!status.drawing) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.ctrlKey || event.altKey || event.metaKey) return;
      event.preventDefault();
      void setRecordingDrawing(false).then(apply);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [status.drawing]);

  const counting = status.phase === "countdown";
  const paused = status.phase === "paused";
  const running = status.phase === "recording";
  const shownMs = running ? stamp.current.elapsed + Math.max(0, tick - stamp.current.at) : status.elapsedMs;
  const tools = status.drawing && !hidden && !settingsOpen;

  const savePrefs = (next: RecordingSettings) => {
    setPrefs(next);
    void getSettings()
      .then((settings) => saveSettings({ ...settings, recording: next }))
      .catch(() => undefined);
    if (next.cameraCorner !== prefs.cameraCorner) void setRecordingCameraCorner(next.cameraCorner).catch(() => undefined);
    if (next.cursor !== prefs.cursor) void setRecordingCursor(next.cursor).catch(() => undefined);
    if (next.focus !== prefs.focus) void setRecordingFocus(next.focus).catch(() => undefined);
  };

  return (
    <div id="recording-controls" className="flex h-full items-end justify-center overflow-visible bg-transparent p-1">
      <div id="recording-controls-bar" ref={barRef} className="relative flex w-max flex-col items-end justify-end">
        {settingsOpen && !hidden ? (
          <div id="recording-controls-settings" className="absolute right-0 bottom-full z-10 mb-1 w-max rounded-lg bg-zinc-950/95 px-1 py-1 text-zinc-100 shadow-lg ring-1 ring-white/10">
            <p id="recording-controls-corner-label" className="px-0.5 pb-0.5 text-[9px] leading-none tracking-wide text-zinc-500 uppercase">
              Camera
            </p>
            <div id="recording-controls-corners" className="grid w-max grid-cols-[max-content_max-content] gap-0.5">
              {CORNERS.map((corner) => (
                <button
                  id={`recording-controls-corner-${corner.id}`}
                  key={corner.id}
                  type="button"
                  aria-pressed={prefs.cameraCorner === corner.id}
                  className={cn(
                    "h-5 rounded px-1.5 text-[9px] leading-none whitespace-nowrap hover:bg-zinc-800",
                    prefs.cameraCorner === corner.id && "bg-zinc-700 text-white",
                  )}
                  onClick={() => savePrefs({ ...prefs, cameraCorner: corner.id })}
                >
                  {corner.label}
                </button>
              ))}
            </div>
            <p id="recording-controls-cursor-label" className="px-0.5 pt-1 pb-0.5 text-[9px] leading-none tracking-wide text-zinc-500 uppercase">
              Cursor
            </p>
            <div id="recording-controls-cursors" className="grid w-max grid-cols-3 gap-0.5">
              {CURSORS.map((cursor) => (
                <button
                  id={`recording-controls-cursor-${cursor.id}`}
                  key={cursor.id}
                  type="button"
                  aria-pressed={prefs.cursor === cursor.id}
                  className={cn(
                    "h-5 rounded px-1.5 text-[9px] leading-none whitespace-nowrap hover:bg-zinc-800",
                    prefs.cursor === cursor.id && "bg-zinc-700 text-white",
                  )}
                  onClick={() => savePrefs({ ...prefs, cursor: cursor.id })}
                >
                  {cursor.label}
                </button>
              ))}
            </div>
            <p id="recording-controls-focus-label" className="px-0.5 pt-1 pb-0.5 text-[9px] leading-none tracking-wide text-zinc-500 uppercase">
              Focus
            </p>
            <div id="recording-controls-focuses" className="grid w-max grid-cols-3 gap-0.5">
              {FOCUSES.map((focus) => (
                <button
                  id={`recording-controls-focus-${focus.id === "1.5" ? "15" : focus.id}`}
                  key={focus.id}
                  type="button"
                  aria-pressed={prefs.focus === focus.id}
                  className={cn(
                    "h-5 rounded px-1.5 text-[9px] leading-none whitespace-nowrap hover:bg-zinc-800",
                    prefs.focus === focus.id && "bg-zinc-700 text-white",
                  )}
                  onClick={() => savePrefs({ ...prefs, focus: focus.id })}
                >
                  {focus.label}
                </button>
              ))}
            </div>
          </div>
        ) : null}
        {tools ? (
          <div id="recording-controls-tools" className="absolute bottom-full left-1/2 z-10 mb-1 flex w-max -translate-x-1/2 items-center gap-0.5 rounded-full bg-zinc-950/95 px-1.5 py-1 text-zinc-100 shadow-lg ring-1 ring-white/10">
            {TOOLS.map((tool) => {
              const Icon = tool.icon;
              return (
                <button
                  id={`recording-controls-tool-${tool.id}`}
                  key={tool.id}
                  type="button"
                  aria-label={tool.label}
                  aria-pressed={status.tool === tool.id}
                  className={cn("inline-flex size-6 items-center justify-center rounded-full", status.tool === tool.id ? "bg-zinc-700" : "hover:bg-zinc-800")}
                  onClick={() => {
                    rememberDraw(tool.id, status.color, drawWidth);
                    void setRecordingStyle(tool.id, status.color).then(apply);
                  }}
                >
                  <Icon id={`recording-controls-tool-${tool.id}-icon`} className="size-3" aria-hidden />
                </button>
              );
            })}
            <span id="recording-controls-tool-divider" className="mx-0.5 h-4 w-px bg-white/15" />
            {COLORS.map((color) => (
              <button
                id={`recording-controls-color-${color.slice(1)}`}
                key={color}
                type="button"
                aria-label={color}
                className={cn(
                  "size-3.5 rounded-full ring-1 ring-white/25 ring-offset-1 ring-offset-zinc-950",
                  status.color === color && "ring-2 ring-sky-300",
                )}
                style={{ background: color }}
                onClick={() => {
                  rememberDraw(status.tool, color, drawWidth);
                  void setRecordingStyle(status.tool, color).then(apply);
                }}
              />
            ))}
            <span id="recording-controls-width-divider" className="mx-0.5 h-4 w-px bg-white/15" />
            {[0, 1, 2, 3].map((size) => (
              <button
                id={`recording-controls-width-${size}`}
                key={size}
                type="button"
                aria-label={["Hairline", "Thin stroke", "Medium stroke", "Thick stroke"][size]}
                aria-pressed={drawWidth === size}
                className={cn("inline-flex size-6 items-center justify-center rounded-full", drawWidth === size && "bg-zinc-700")}
                onClick={() => {
                  setDrawWidth(size);
                  rememberDraw(status.tool, status.color, size);
                }}
              >
                <span
                  id={`recording-controls-width-${size}-mark`}
                  className="rounded-full bg-zinc-100"
                  style={{ width: [3, 5, 7, 10][size], height: [3, 5, 7, 10][size] }}
                />
              </button>
            ))}
            <button
              id="recording-controls-clear"
              type="button"
              aria-label="Clear drawing"
              className="inline-flex size-6 items-center justify-center rounded-full hover:bg-zinc-800"
              onClick={requestClear}
            >
              <Trash2 id="recording-controls-clear-icon" className="size-3" aria-hidden />
            </button>
            <button
              id="recording-controls-undo"
              type="button"
              aria-label="Undo"
              className="inline-flex size-6 items-center justify-center rounded-full hover:bg-zinc-800"
              onClick={requestUndo}
            >
              <Undo2 id="recording-controls-undo-icon" className="size-3" aria-hidden />
            </button>
          </div>
        ) : null}
        <div id="recording-controls-row" className="flex w-max items-center gap-1.5 rounded-full bg-zinc-950/95 px-2 py-1 text-zinc-100 shadow-lg ring-1 ring-white/10">
          <span id="recording-controls-time" className="min-w-16 shrink-0 text-center text-xs font-semibold tabular-nums text-rose-300">
            {counting ? status.countdown || 3 : clock(shownMs)}
          </span>
          <button
            id="recording-controls-stop"
            type="button"
            className="inline-flex size-7 shrink-0 items-center justify-center rounded-full bg-rose-600"
            aria-label="Stop"
            onClick={() => void stopScreenRecording().catch(() => undefined)}
          >
            <Square id="recording-controls-stop-icon" className="size-3 fill-current" aria-hidden />
          </button>
          {hidden ? null : (
            <>
              <button
                id="recording-controls-pause"
                type="button"
                className="inline-flex size-7 shrink-0 items-center justify-center rounded-full hover:bg-zinc-800"
                aria-label={paused ? "Resume" : "Pause"}
                disabled={counting}
                onClick={() => void (paused ? resumeScreenRecording() : pauseScreenRecording()).then(apply)}
              >
                {paused ? <Play id="recording-controls-pause-icon" className="size-3.5" aria-hidden /> : <Pause id="recording-controls-pause-icon" className="size-3.5" aria-hidden />}
              </button>
              <button
                id="recording-controls-mic"
                type="button"
                className={cn(
                  "inline-flex h-7 shrink-0 items-center gap-1 rounded-full px-1.5 hover:bg-zinc-800",
                  status.micMuted && "text-rose-300",
                )}
                aria-label={status.micMuted ? "Unmute microphone" : "Mute microphone"}
                aria-pressed={status.micMuted}
                disabled={!status.microphone}
                onClick={() => void setRecordingMicMuted(!status.micMuted).then(apply)}
              >
                <span id="recording-controls-mic-meter" className="flex h-4 items-end gap-px" aria-hidden>
                  {METER.map((step, index) => (
                    <span
                      id={`recording-controls-mic-bar-${index}`}
                      key={step}
                      className={cn(
                        "w-0.5 rounded-full",
                        status.micLevel >= step ? (status.micMuted ? "bg-rose-400" : "bg-emerald-400") : "bg-zinc-700",
                      )}
                      style={{ height: `${6 + index * 2}px` }}
                    />
                  ))}
                </span>
                {status.micMuted ? (
                  <MicOff id="recording-controls-mic-icon" className="size-3.5" aria-hidden />
                ) : (
                  <Mic id="recording-controls-mic-icon" className="size-3.5" aria-hidden />
                )}
              </button>
              <button
                id="recording-controls-camera"
                type="button"
                className={cn("inline-flex size-7 shrink-0 items-center justify-center rounded-full hover:bg-zinc-800", status.camera && "text-sky-300")}
                aria-label={status.camera ? "Hide camera" : "Show camera"}
                aria-pressed={status.camera}
                onClick={() => void setRecordingCamera(!status.camera).then(apply)}
              >
                {status.camera ? <Camera id="recording-controls-camera-icon" className="size-3.5" aria-hidden /> : <CameraOff id="recording-controls-camera-icon" className="size-3.5" aria-hidden />}
              </button>
              <button
                id="recording-controls-draw"
                type="button"
                className={cn("inline-flex size-7 shrink-0 items-center justify-center rounded-full hover:bg-zinc-800", status.drawing && "text-sky-300")}
                aria-label={status.drawing ? "Stop drawing" : "Draw"}
                aria-pressed={status.drawing}
                onClick={() => void setRecordingDrawing(!status.drawing).then(apply)}
              >
                <Pencil id="recording-controls-draw-icon" className="size-3.5" aria-hidden />
              </button>
              <button
                id="recording-controls-settings-toggle"
                type="button"
                className={cn("inline-flex size-7 shrink-0 items-center justify-center rounded-full hover:bg-zinc-800", settingsOpen && "text-sky-300")}
                aria-label="Recording settings"
                aria-pressed={settingsOpen}
                onClick={() => setSettingsOpen((open) => !open)}
              >
                <Settings id="recording-controls-settings-icon" className="size-3.5" aria-hidden />
              </button>
            </>
          )}
          <button
            id="recording-controls-hide"
            type="button"
            className="inline-flex size-7 shrink-0 items-center justify-center rounded-full hover:bg-zinc-800"
            aria-label={hidden ? "Show controls" : "Hide controls"}
            aria-pressed={hidden}
            onClick={() => {
              setHidden((current) => !current);
              setSettingsOpen(false);
            }}
          >
            {hidden ? <ChevronUp id="recording-controls-hide-icon" className="size-3.5" aria-hidden /> : <ChevronDown id="recording-controls-hide-icon" className="size-3.5" aria-hidden />}
          </button>
        </div>
      </div>
    </div>
  );
}
