import { useEffect, useState } from "react";
import { AppWindow, Monitor, Square, Video } from "lucide-react";
import { ActionTile, OptionRow, PopoverHeader, Segmented } from "../PopoverParts";
import { capturable, readPopoverTab } from "../shared";
import { PopupFrame } from "@/components/PopupFrame";
import {
  closePopup,
  currentWindowLabel,
  getSettings,
  listRecordingTargets,
  saveSettings,
  startScreenRecording,
  stopScreenRecording,
} from "@/lib/browser-client";
import type { RecordingResolution, RecordingSettings, RecordingSource, RecordingTargetInfo } from "@type/browser";

type RecordingPopoverProps = {
  params: URLSearchParams;
};

export function RecordingPopover({ params }: RecordingPopoverProps) {
  const tab = readPopoverTab(params);
  const pageOk = capturable(tab.url);
  const [source, setSource] = useState<RecordingSource>(pageOk ? "tab" : "browser");
  const [choices, setChoices] = useState<RecordingSettings>({
    microphone: params.get("microphone") !== "false",
    systemAudio: params.get("systemAudio") !== "false",
    camera: params.get("camera") === "true",
    countdown: params.get("countdown") !== "false",
    resolution: "1440",
    cameraCorner: "bottom-left",
    cursor: "system",
    focus: "off",
  });
  const [targets, setTargets] = useState<RecordingTargetInfo[]>([]);
  const [target, setTarget] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const recording = params.get("recording") === "true";

  useEffect(() => {
    void getSettings()
      .then((settings) => {
        if (settings.recording) setChoices((current) => ({ ...current, ...settings.recording }));
      })
      .catch(() => undefined);
    void listRecordingTargets(currentWindowLabel())
      .then((next) => {
        setTargets(next);
        const firstWindow = next.find((item) => item.kind === "window");
        const firstScreen = next.find((item) => item.kind === "display");
        setTarget((firstWindow ?? firstScreen)?.id ?? "");
      })
      .catch(() => undefined);
  }, []);

  const remember = (next: RecordingSettings) => {
    setChoices(next);
    void getSettings()
      .then((settings) => saveSettings({ ...settings, recording: next }))
      .catch(() => undefined);
  };

  const windows = targets.filter((item) => item.kind === "window");
  const screens = targets.filter((item) => item.kind === "display");
  const picked = source === "window" ? windows : screens;

  const start = async () => {
    setBusy(true);
    setError("");
    const chosen = source === "tab" || source === "browser" ? "" : target || picked[0]?.id || "";
    try {
      await startScreenRecording({
        source,
        target: chosen,
        window: currentWindowLabel(),
        tabId: tab.id || 0,
        title: tab.title || tab.url,
        microphone: choices.microphone,
        systemAudio: choices.systemAudio,
        camera: choices.camera,
        countdown: choices.countdown,
        resolution: choices.resolution,
        cameraCorner: choices.cameraCorner,
        cursor: choices.cursor,
        focus: choices.focus,
      });
      void closePopup();
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Couldn't start the recording.");
      setBusy(false);
    }
  };

  const stop = async () => {
    setBusy(true);
    try {
      await stopScreenRecording();
      void closePopup();
    } catch (caught) {
      const message = caught instanceof Error ? caught.message : "";
      if (message !== "cancelled") setError(message || "Couldn't stop the recording.");
      else void closePopup();
      setBusy(false);
    }
  };

  return (
    <PopupFrame id="recording-popover" width={340} maxHeight={560}>
      <PopoverHeader id="recording-header" icon={<Video aria-hidden />} title="Recording" subtitle={tab.title || "This window"} />
      {recording ? (
        <div id="recording-live" className="flex flex-col gap-2 px-3 pb-3">
          <p id="recording-live-note" className="px-1 text-sm text-zinc-300">
            A recording is in progress. The controls sit on top of the picture and stay out of the video.
          </p>
          <ActionTile id="recording-stop" primary icon={<Square aria-hidden />} label="Stop" hint="Save what has been recorded" onClick={() => void stop()} />
        </div>
      ) : (
        <div id="recording-form" className="flex flex-col gap-2 px-3 pb-3">
          <Segmented<RecordingSource>
            id="recording-source"
            label="What to record"
            value={source}
            options={[
              { value: "tab", label: "Tab" },
              { value: "browser", label: "Window" },
              { value: "window", label: "App" },
              { value: "display", label: "Screen" },
            ]}
            onChange={setSource}
          />
          {source === "tab" && !pageOk ? (
            <p id="recording-tab-empty" className="px-1 text-xs text-zinc-500">
              Open a web page to record the tab.
            </p>
          ) : null}
          {source === "window" || source === "display" ? (
            <label id="recording-target" className="flex flex-col gap-1 px-1">
              <span id="recording-target-label" className="text-xs text-zinc-500">
                {source === "window" ? "Window" : "Screen"}
              </span>
              <select
                id="recording-target-select"
                className="h-8 rounded-md bg-zinc-800 px-2 text-sm"
                value={picked.some((item) => item.id === target) ? target : (picked[0]?.id ?? "")}
                onChange={(event) => setTarget(event.target.value)}
              >
                {picked.length === 0 ? <option id="recording-target-none" value="">Nothing to record</option> : null}
                {picked.map((item) => (
                  <option id={`recording-target-${item.id}`} key={item.id} value={item.id}>
                    {item.label}
                  </option>
                ))}
              </select>
            </label>
          ) : null}
          <OptionRow id="recording-mic" label="Microphone" hint="Uses the default microphone" checked={choices.microphone} onChange={(microphone) => remember({ ...choices, microphone })} />
          <OptionRow id="recording-system" label="System audio" hint="Sound playing on this computer" checked={choices.systemAudio} onChange={(systemAudio) => remember({ ...choices, systemAudio })} />
          <OptionRow id="recording-camera" label="Camera bubble" hint="A round camera you can move later" checked={choices.camera} onChange={(camera) => remember({ ...choices, camera })} />
          <OptionRow id="recording-countdown" label="Countdown" hint="Three seconds before it starts" checked={choices.countdown} onChange={(countdown) => remember({ ...choices, countdown })} />
          <div id="recording-resolution" className="flex flex-col gap-1 px-1">
            <span id="recording-resolution-label" className="text-xs text-zinc-500">
              Resolution
            </span>
            <Segmented<RecordingResolution>
              id="recording-resolution-choice"
              label="Resolution"
              value={choices.resolution}
              options={[
                { value: "720", label: "720p" },
                { value: "1080", label: "1080p" },
                { value: "1440", label: "1440p" },
                { value: "native", label: "Native" },
              ]}
              onChange={(resolution) => remember({ ...choices, resolution })}
            />
          </div>
          {error ? (
            <p id="recording-error" className="rounded-lg bg-amber-500/10 px-3 py-2 text-sm text-amber-200">
              {error}
            </p>
          ) : null}
          <ActionTile
            id="recording-start"
            primary
            disabled={busy || (source === "tab" && !pageOk) || ((source === "window" || source === "display") && picked.length === 0)}
            icon={source === "display" ? <Monitor aria-hidden /> : source === "window" ? <AppWindow aria-hidden /> : <Video aria-hidden />}
            label={busy ? "Starting…" : "Start recording"}
            hint="Saved in this profile"
            onClick={() => void start()}
          />
        </div>
      )}
    </PopupFrame>
  );
}
