import { useEffect, useRef, useState } from "react";
import { Square, Video } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { createTab, currentWindowLabel, onScreenRecording, openPopup, screenRecordingStatus } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import type { BrowserSettings, ScreenRecordingStatus, TabInfo } from "@type/browser";
import { tabParams } from "../shared";

type RecordingButtonProps = {
  activeTab: TabInfo | undefined;
  settings: BrowserSettings | null;
};

export function RecordingButton({ activeTab, settings }: RecordingButtonProps) {
  const buttonRef = useRef<HTMLButtonElement>(null);
  const opened = useRef("");
  const [live, setLive] = useState<ScreenRecordingStatus | null>(null);
  const recording = live && live.phase !== "idle" && live.phase !== "error";

  useEffect(() => {
    const owner = currentWindowLabel();
    void screenRecordingStatus()
      .then((status) => {
        if (status.ownerWindow === owner && status.phase !== "idle") setLive(status);
      })
      .catch(() => undefined);
    const listen = onScreenRecording((status) => {
      if (status.ownerWindow && status.ownerWindow !== owner) return;
      setLive(status.phase === "idle" || status.phase === "error" ? status : status);
      if (status.phase === "idle" && status.id && status.path && status.ownerWindow === owner && opened.current !== status.id) {
        opened.current = status.id;
        void createTab(`ce://recordings?id=${encodeURIComponent(status.id)}`);
      }
    });
    return () => {
      void listen.then((stop) => stop());
    };
  }, []);

  const open = () => {
    const anchor = buttonRef.current;
    if (!anchor) return;
    const params = new URLSearchParams(activeTab ? tabParams(activeTab) : "");
    const choices = settings?.recording;
    if (choices) {
      params.set("microphone", String(choices.microphone));
      params.set("systemAudio", String(choices.systemAudio));
      params.set("camera", String(choices.camera));
      params.set("countdown", String(choices.countdown));
    }
    if (recording) params.set("recording", "true");
    void openPopup("recording", anchor, { params: params.toString() });
  };

  const label = live?.phase === "error" ? live.error || "Recording failed" : recording ? "Recording" : "Record";

  return (
    <IconButton
      id="address-recording"
      ref={buttonRef}
      label={label}
      className={cn(recording && "text-rose-300", live?.phase === "error" && "text-amber-300")}
      onClick={open}
    >
      {recording ? <Square id="address-recording-icon" aria-hidden /> : <Video id="address-recording-icon" aria-hidden />}
    </IconButton>
  );
}
