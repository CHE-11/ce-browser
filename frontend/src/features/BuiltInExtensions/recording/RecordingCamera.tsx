import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { appendRecordingCamera, onScreenRecording, screenRecordingStatus } from "@/lib/browser-client";

let generation = 0;

function clearPage() {
  document.documentElement.dataset.recordingClear = "true";
  return () => {
    delete document.documentElement.dataset.recordingClear;
  };
}

export function RecordingCamera() {
  const videoRef = useRef<HTMLVideoElement>(null);
  const recorder = useRef<MediaRecorder | null>(null);
  const stream = useRef<MediaStream | null>(null);
  const [message, setMessage] = useState("");

  useEffect(() => {
    document.title = "Recording camera";
    const restore = clearPage();
    const mine = ++generation;
    let closed = false;

    const stopRecorder = () => {
      const current = recorder.current;
      recorder.current = null;
      if (current && current.state !== "inactive") {
        current.ondataavailable = null;
        current.stop();
      }
    };

    const stopAll = () => {
      stopRecorder();
      stream.current?.getTracks().forEach((track) => track.stop());
      stream.current = null;
      if (videoRef.current) videoRef.current.srcObject = null;
    };

    const preview = async () => {
      if (closed || stream.current) return;
      try {
        const media = await navigator.mediaDevices.getUserMedia({
          video: { width: { ideal: 640 }, height: { ideal: 640 }, facingMode: "user" },
          audio: false,
        });
        if (closed || mine !== generation) {
          media.getTracks().forEach((track) => track.stop());
          return;
        }
        stream.current = media;
        const video = videoRef.current;
        if (video) {
          video.srcObject = media;
          await video.play().catch(() => undefined);
        }
        setMessage("");
      } catch (caught) {
        const detail = caught instanceof Error ? caught.message : "";
        setMessage(detail ? `Camera: ${detail}` : "The camera could not start.");
      }
    };

    const record = () => {
      const media = stream.current;
      if (!media || recorder.current || closed) return;
      const mime = MediaRecorder.isTypeSupported("video/webm;codecs=vp9") ? "video/webm;codecs=vp9" : "video/webm";
      let recording: MediaRecorder;
      try {
        recording = new MediaRecorder(media, { mimeType: mime });
      } catch {
        return;
      }
      recording.ondataavailable = (event) => {
        if (event.data.size === 0) return;
        void event.data.arrayBuffer().then((buffer) => {
          void appendRecordingCamera(Array.from(new Uint8Array(buffer))).catch(() => undefined);
        });
      };
      recording.start(250);
      recorder.current = recording;
    };

    const apply = (phase: string, camera: boolean) => {
      const live = camera && phase !== "idle" && phase !== "error";
      if (!live) {
        stopAll();
        return;
      }
      void preview().then(() => {
        if (phase === "recording") record();
        else stopRecorder();
      });
    };

    void screenRecordingStatus().then((status) => apply(status.phase, status.camera)).catch(() => undefined);
    const listen = onScreenRecording((status) => apply(status.phase, status.camera));
    return () => {
      closed = true;
      if (mine === generation) stopAll();
      restore();
      void listen.then((unlisten) => unlisten());
    };
  }, []);

  return (
    <div id="recording-camera" className="h-full w-full bg-transparent" onPointerDown={() => void getCurrentWindow().startDragging()}>
      <video id="recording-camera-video" ref={videoRef} className="h-full w-full rounded-full bg-zinc-900 object-cover" autoPlay muted playsInline />
      {message ? (
        <p id="recording-camera-message" className="pointer-events-none absolute inset-2 flex items-center justify-center text-center text-[10px] leading-tight text-zinc-100">
          {message}
        </p>
      ) : null}
    </div>
  );
}
