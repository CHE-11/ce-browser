import { useEffect, useRef, useState } from "react";
import { Download, FolderOpen, Trash2 } from "lucide-react";
import { Button } from "@/components/Button";
import {
  appendRecordingExport,
  beginRecordingExport,
  chooseSavePath,
  deleteRecording,
  exportRecording,
  finishRecordingExport,
  listRecordings,
  onScreenRecording,
  recordingMediaUrl,
  showSavedFile,
  updateRecording,
} from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import type { CameraLayout, ScreenRecording } from "@type/browser";
import { clock } from "./time";
import { TrimBar } from "./TrimBar";

type RecordingsPageProps = {
  params: URLSearchParams;
};

export function RecordingsPage({ params }: RecordingsPageProps) {
  const [items, setItems] = useState<ScreenRecording[]>([]);
  const [selected, setSelected] = useState(params.get("id") ?? "");
  const [error, setError] = useState("");
  const [exporting, setExporting] = useState("");
  const item = items.find((entry) => entry.id === selected) ?? items[0];

  const load = () => {
    void listRecordings()
      .then((next) => {
        setItems(next);
        setSelected((current) => (next.some((entry) => entry.id === current) ? current : next[0]?.id ?? ""));
      })
      .catch((caught: unknown) => setError(caught instanceof Error ? caught.message : "Couldn't load recordings."));
  };

  useEffect(() => {
    document.title = "Recordings";
    load();
    const listen = onScreenRecording((status) => {
      if (status.phase === "idle" && status.id) load();
    });
    return () => {
      void listen.then((stop) => stop());
    };
  }, []);

  return (
    <main id="recordings-page" className="flex h-full min-h-0 bg-zinc-950 text-zinc-100">
      <aside id="recordings-list" className="flex w-72 shrink-0 flex-col border-r border-zinc-800">
        <h1 id="recordings-heading" className="px-4 py-3 text-sm font-semibold">
          Recordings
        </h1>
        {items.length === 0 ? (
          <p id="recordings-empty" className="px-4 text-sm text-zinc-500">
            Recordings you make show up here.
          </p>
        ) : (
          <div id="recordings-items" className="min-h-0 flex-1 overflow-auto">
            {items.map((entry) => (
              <button
                id={`recordings-item-${entry.id}`}
                key={entry.id}
                type="button"
                className={cn("flex w-full flex-col items-start px-4 py-2 text-left hover:bg-zinc-900", item?.id === entry.id && "bg-zinc-900")}
                onClick={() => setSelected(entry.id)}
              >
                <span id={`recordings-item-${entry.id}-title`} className="truncate text-sm">
                  {entry.title || "Recording"}
                </span>
                <span id={`recordings-item-${entry.id}-meta`} className="text-xs text-zinc-500">
                  {clock(entry.durationMs)} · {entry.sourceLabel}
                </span>
              </button>
            ))}
          </div>
        )}
      </aside>
      <section id="recordings-player" className="flex min-w-0 flex-1 flex-col">
        {item ? (
          <Player
            item={item}
            exporting={exporting}
            error={error}
            onError={setError}
            onExporting={setExporting}
            onChange={(next) => setItems((current) => current.map((entry) => (entry.id === next.id ? next : entry)))}
            onDelete={() => {
              void deleteRecording(item.id).then(load).catch((caught: unknown) => setError(caught instanceof Error ? caught.message : "Couldn't delete it."));
            }}
          />
        ) : (
          <p id="recordings-none" className="m-auto text-sm text-zinc-500">
            Nothing to play yet.
          </p>
        )}
      </section>
    </main>
  );
}

function Player({
  item,
  exporting,
  error,
  onError,
  onExporting,
  onChange,
  onDelete,
}: {
  item: ScreenRecording;
  exporting: string;
  error: string;
  onError: (message: string) => void;
  onExporting: (message: string) => void;
  onChange: (item: ScreenRecording) => void;
  onDelete: () => void;
}) {
  const screenRef = useRef<HTMLVideoElement>(null);
  const cameraRef = useRef<HTMLVideoElement>(null);
  const stageRef = useRef<HTMLDivElement>(null);
  const [videoUrl, setVideoUrl] = useState("");
  const [cameraUrl, setCameraUrl] = useState("");
  const [title, setTitle] = useState(item.title);
  const [camera, setCamera] = useState<CameraLayout>(item.camera);
  const [currentMs, setCurrentMs] = useState(0);
  const dragging = useRef<{ x: number; y: number; left: number; top: number } | null>(null);

  useEffect(() => {
    setTitle(item.title);
    setCamera(item.camera);
    let cancel = false;
    void recordingMediaUrl(item.id, "video")
      .then((url) => {
        if (!cancel) setVideoUrl(url);
      })
      .catch(() => undefined);
    if (item.cameraPath) {
      void recordingMediaUrl(item.id, "camera")
        .then((url) => {
          if (!cancel) setCameraUrl(url);
        })
        .catch(() => undefined);
    } else {
      setCameraUrl("");
    }
    return () => {
      cancel = true;
    };
  }, [item.id, item.cameraPath, item.title, item.camera]);

  const trimEnd = item.trimEndMs || item.durationMs;

  const seekMedia = (video: HTMLVideoElement, wallMs: number) => {
    const scale = mediaScale(video, item.durationMs);
    video.playbackRate = scale;
    const at = (wallMs / 1000) * scale;
    if (Math.abs(video.currentTime - at) > 0.05) video.currentTime = at;
  };

  const seekStart = (video: HTMLVideoElement | null) => {
    if (!video) return;
    seekMedia(video, item.trimStartMs);
  };

  const saveVideo = async () => {
    const path = await chooseSavePath(`${item.title || "recording"}.mp4`, "Video", ["mp4"]);
    if (!path) return;
    onError("");
    try {
      await exportRecording(item.id, path);
      onExporting("Saved.");
    } catch (caught) {
      onError(caught instanceof Error ? caught.message : "Couldn't export the recording.");
    }
  };

  const saveWithCamera = async () => {
    if (!videoUrl) return;
    const path = await chooseSavePath(`${item.title || "recording"}.webm`, "Video", ["webm"]);
    if (!path) return;
    onError("");
    onExporting("This plays the video through once.");
    try {
      await burnCamera(item, videoUrl, cameraUrl, camera, path, (note) => onExporting(note));
      onExporting("Saved.");
    } catch (caught) {
      onError(caught instanceof Error ? caught.message : "Couldn't export the recording.");
      onExporting("");
    }
  };

  return (
    <div id="recordings-stage-wrap" className="flex min-h-0 flex-1 flex-col">
      <div id="recordings-toolbar" className="flex flex-wrap items-center gap-2 border-b border-zinc-800 px-4 py-3">
        <input
          id="recordings-title"
          className="h-8 min-w-0 flex-1 rounded-md bg-zinc-900 px-2 text-sm"
          value={title}
          aria-label="Title"
          onChange={(event) => setTitle(event.target.value)}
          onBlur={() => {
            if (title.trim() && title !== item.title) {
              void updateRecording({ id: item.id, title: title.trim() }).then(onChange).catch(() => undefined);
            }
          }}
        />
        <Button id="recordings-show" onClick={() => void showSavedFile(item.videoPath)}>
          <FolderOpen id="recordings-show-icon" className="size-4" aria-hidden />
          Show
        </Button>
        <Button id="recordings-export" onClick={() => void saveVideo()}>
          <Download id="recordings-export-icon" className="size-4" aria-hidden />
          Save video
        </Button>
        {item.cameraPath ? (
          <Button id="recordings-export-camera" onClick={() => void saveWithCamera()} disabled={exporting.startsWith("Exporting")}>
            Save with camera
          </Button>
        ) : null}
        <Button id="recordings-delete" variant="destructive" onClick={onDelete}>
          <Trash2 id="recordings-delete-icon" className="size-4" aria-hidden />
          Delete
        </Button>
      </div>
      {error ? (
        <p id="recordings-error" className="px-4 py-2 text-sm text-amber-300">
          {error}
        </p>
      ) : null}
      {exporting ? (
        <p id="recordings-export-note" className="px-4 py-2 text-sm text-zinc-400">
          {exporting}
        </p>
      ) : null}
      <div id="recordings-stage" ref={stageRef} className="relative m-4 min-h-0 flex-1 overflow-hidden rounded-lg bg-black">
        <video
          id="recordings-video"
          ref={screenRef}
          className="h-full w-full object-contain"
          src={videoUrl}
          controls
          onLoadedMetadata={(event) => seekStart(event.currentTarget)}
          onPlay={() => {
            const cameraVideo = cameraRef.current;
            const screen = screenRef.current;
            if (!screen) return;
            screen.playbackRate = mediaScale(screen, item.durationMs);
            if (cameraVideo) {
              cameraVideo.currentTime = wallSeconds(screen, item.durationMs);
              void cameraVideo.play().catch(() => undefined);
            }
          }}
          onPause={() => cameraRef.current?.pause()}
          onTimeUpdate={(event) => {
            const video = event.currentTarget;
            const at = wallSeconds(video, item.durationMs) * 1000;
            setCurrentMs(at);
            if (at >= trimEnd) {
              video.pause();
              seekMedia(video, trimEnd);
            }
            const cameraVideo = cameraRef.current;
            const wall = at / 1000;
            if (cameraVideo && Math.abs(cameraVideo.currentTime - wall) > 0.3) {
              cameraVideo.currentTime = wall;
            }
          }}
        />
        {cameraUrl ? (
          <video
            id="recordings-camera"
            ref={cameraRef}
            className="absolute rounded-full object-cover shadow-lg"
            style={{ left: `${camera.x * 100}%`, top: `${camera.y * 100}%`, width: `${camera.size * 100}%`, aspectRatio: "1" }}
            src={cameraUrl}
            muted
            playsInline
            onPointerDown={(event) => {
              const stage = stageRef.current?.getBoundingClientRect();
              if (!stage) return;
              dragging.current = { x: event.clientX, y: event.clientY, left: camera.x, top: camera.y };
              event.currentTarget.setPointerCapture(event.pointerId);
            }}
            onPointerMove={(event) => {
              const drag = dragging.current;
              const stage = stageRef.current?.getBoundingClientRect();
              if (!drag || !stage) return;
              setCamera({
                ...camera,
                x: Math.min(0.9, Math.max(-0.1, drag.left + (event.clientX - drag.x) / stage.width)),
                y: Math.min(0.9, Math.max(-0.1, drag.top + (event.clientY - drag.y) / stage.height)),
              });
            }}
            onPointerUp={() => {
              dragging.current = null;
              void updateRecording({ id: item.id, camera }).then(onChange).catch(() => undefined);
            }}
          />
        ) : null}
        <span id="recordings-clock" className="pointer-events-none absolute top-3 right-3 rounded-full bg-black/70 px-2 py-1 text-xs font-semibold text-zinc-100 tabular-nums">
          {clock(currentMs)} / {clock(trimEnd)}
        </span>
      </div>
      <TrimBar
        durationMs={item.durationMs}
        startMs={item.trimStartMs}
        endMs={trimEnd}
        currentMs={currentMs}
        onChange={(trimStartMs, trimEndMs, handle) => {
          onChange({ ...item, trimStartMs, trimEndMs });
          const video = screenRef.current;
          if (video) seekMedia(video, handle === "start" ? trimStartMs : trimEndMs);
        }}
        onCommit={(trimStartMs, trimEndMs) => {
          void updateRecording({ id: item.id, trimStartMs, trimEndMs }).then(onChange).catch(() => undefined);
        }}
      />
    </div>
  );
}

async function burnCamera(
  item: ScreenRecording,
  videoUrl: string,
  cameraUrl: string,
  layout: CameraLayout,
  path: string,
  onProgress: (note: string) => void,
) {
  const screen = document.createElement("video");
  screen.src = videoUrl;
  screen.crossOrigin = "anonymous";
  const camera = cameraUrl ? document.createElement("video") : null;
  if (camera) {
    camera.src = cameraUrl;
    camera.muted = true;
    camera.crossOrigin = "anonymous";
  }
  await new Promise<void>((resolve, reject) => {
    screen.onloadeddata = () => resolve();
    screen.onerror = () => reject(new Error("Couldn't export the recording."));
  });
  const width = item.width || screen.videoWidth || 1280;
  const height = item.height || screen.videoHeight || 720;
  const canvas = document.createElement("canvas");
  canvas.id = "recordings-export-canvas";
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext("2d");
  if (!context) throw new Error("Couldn't export the recording.");
  const audio = new AudioContext();
  const source = audio.createMediaElementSource(screen);
  const dest = audio.createMediaStreamDestination();
  source.connect(dest);
  source.connect(audio.destination);
  const stream = canvas.captureStream(30);
  const track = dest.stream.getAudioTracks()[0];
  if (track) stream.addTrack(track);
  const mime = MediaRecorder.isTypeSupported("video/webm;codecs=vp9") ? "video/webm;codecs=vp9,opus" : "video/webm";
  const recorder = new MediaRecorder(stream, { mimeType: mime });
  await beginRecordingExport(path);
  const done = new Promise<void>((resolve) => {
    recorder.ondataavailable = (event) => {
      if (event.data.size === 0) return;
      void event.data.arrayBuffer().then((buffer) => {
        void appendRecordingExport(Array.from(new Uint8Array(buffer)));
      });
    };
    recorder.onstop = () => resolve();
  });
  const start = item.trimStartMs / 1000;
  const end = (item.trimEndMs || item.durationMs) / 1000;
  const scale = mediaScale(screen, item.durationMs);
  screen.playbackRate = scale;
  screen.currentTime = start * scale;
  if (camera) camera.currentTime = start;
  recorder.start(200);
  await screen.play();
  if (camera) void camera.play().catch(() => undefined);
  await new Promise<void>((resolve) => {
    const draw = () => {
      context.drawImage(screen, 0, 0, width, height);
      if (camera && camera.readyState >= 2) {
        const size = layout.size * width;
        context.save();
        context.beginPath();
        context.arc(layout.x * width + size / 2, layout.y * height + size / 2, size / 2, 0, Math.PI * 2);
        context.clip();
        context.drawImage(camera, layout.x * width, layout.y * height, size, size);
        context.restore();
      }
      const wall = wallSeconds(screen, item.durationMs);
      if (camera && Math.abs(camera.currentTime - wall) > 0.3) camera.currentTime = wall;
      onProgress(`Exporting ${clock(wall * 1000)} of ${clock(end * 1000)}. This plays the video through once.`);
      if (wall >= end || screen.ended) {
        screen.pause();
        camera?.pause();
        resolve();
        return;
      }
      requestAnimationFrame(draw);
    };
    requestAnimationFrame(draw);
  });
  recorder.stop();
  await done;
  await finishRecordingExport();
  await audio.close();
}

/** How many seconds of the file correspond to one second of the take. Older files were packed too short. */
function mediaScale(video: HTMLVideoElement, durationMs: number) {
  const mediaMs = video.duration * 1000;
  if (!Number.isFinite(mediaMs) || mediaMs <= 0 || durationMs <= 0) return 1;
  return mediaMs / durationMs;
}

function wallSeconds(video: HTMLVideoElement, durationMs: number) {
  const scale = mediaScale(video, durationMs);
  return scale > 0 ? video.currentTime / scale : video.currentTime;
}
