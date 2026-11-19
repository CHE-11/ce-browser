import { useEffect, useRef, useState } from "react";
import { ArrowUpRight, Copy, PenLine, RectangleHorizontal, Save, Sparkles, Type } from "lucide-react";
import { Button } from "@/components/Button";
import { chooseSavePath, copyEditedPng, defaultDownloadDir, lastCapturePng, saveEditedPng } from "@/lib/browser-client";
import { cn } from "@/lib/cn";

type Tool = "pen" | "arrow" | "rect" | "text" | "blur";

const TOOLS: { id: Tool; label: string; icon: typeof PenLine }[] = [
  { id: "pen", label: "Pen", icon: PenLine },
  { id: "arrow", label: "Arrow", icon: ArrowUpRight },
  { id: "rect", label: "Rectangle", icon: RectangleHorizontal },
  { id: "text", label: "Text", icon: Type },
  { id: "blur", label: "Blur", icon: Sparkles },
];

export function CaptureEditorPage() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const drag = useRef<{ x: number; y: number; image: ImageData | null } | null>(null);
  const [tool, setTool] = useState<Tool>("pen");
  const [text, setText] = useState("Note");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    document.title = "Screenshot editor";
  }, []);

  useEffect(() => {
    let gone = false;
    void lastCapturePng()
      .then((data) => {
        if (gone) {
          return;
        }
        const image = new Image();
        image.onload = () => {
          const canvas = canvasRef.current;
          if (!canvas) {
            return;
          }
          canvas.width = image.width;
          canvas.height = image.height;
          canvas.getContext("2d")?.drawImage(image, 0, 0);
        };
        image.src = `data:image/png;base64,${data}`;
      })
      .catch((reason: unknown) => {
        if (!gone) {
          setError(reason instanceof Error ? reason.message : "Capture a page first.");
        }
      });
    return () => {
      gone = true;
    };
  }, []);

  const point = (event: React.PointerEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas) {
      return { x: 0, y: 0 };
    }
    const rect = canvas.getBoundingClientRect();
    const scaleX = canvas.width / rect.width;
    const scaleY = canvas.height / rect.height;
    return { x: (event.clientX - rect.left) * scaleX, y: (event.clientY - rect.top) * scaleY };
  };

  const onDown = (event: React.PointerEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    const context = canvas?.getContext("2d");
    if (!canvas || !context) {
      return;
    }
    const at = point(event);
    if (tool === "text") {
      context.font = "28px sans-serif";
      context.fillStyle = "#f43f5e";
      context.fillText(text || "Note", at.x, at.y);
      return;
    }
    context.strokeStyle = "#38bdf8";
    context.fillStyle = "rgba(56,189,248,0.2)";
    context.lineWidth = 4;
    context.lineCap = "round";
    if (tool === "pen") {
      context.beginPath();
      context.moveTo(at.x, at.y);
    }
    drag.current = {
      x: at.x,
      y: at.y,
      image: tool === "pen" ? null : context.getImageData(0, 0, canvas.width, canvas.height),
    };
    canvas.setPointerCapture(event.pointerId);
  };

  const onMove = (event: React.PointerEvent<HTMLCanvasElement>) => {
    const start = drag.current;
    const canvas = canvasRef.current;
    const context = canvas?.getContext("2d");
    if (!start || !canvas || !context) {
      return;
    }
    const at = point(event);
    if (tool === "pen") {
      context.lineTo(at.x, at.y);
      context.stroke();
      return;
    }
    if (start.image) {
      context.putImageData(start.image, 0, 0);
    }
    if (tool === "rect" || tool === "blur") {
      const x = Math.min(start.x, at.x);
      const y = Math.min(start.y, at.y);
      const width = Math.abs(at.x - start.x);
      const height = Math.abs(at.y - start.y);
      context.strokeRect(x, y, width, height);
    } else if (tool === "arrow") {
      drawArrow(context, start.x, start.y, at.x, at.y);
    }
  };

  const onUp = (event: React.PointerEvent<HTMLCanvasElement>) => {
    const start = drag.current;
    const canvas = canvasRef.current;
    const context = canvas?.getContext("2d");
    drag.current = null;
    if (!start || !canvas || !context) {
      return;
    }
    const at = point(event);
    if (tool === "blur" && start.image) {
      context.putImageData(start.image, 0, 0);
      blurRect(context, canvas, start.x, start.y, at.x, at.y);
    }
  };

  const png = () => canvasRef.current?.toDataURL("image/png").split(",")[1] ?? "";

  const save = async () => {
    const data = png();
    if (!data) {
      return;
    }
    setBusy(true);
    try {
      const folder = await defaultDownloadDir();
      const path = await chooseSavePath(`${folder}\\screenshot.png`, "PNG image", ["png"]);
      if (path) {
        await saveEditedPng(data, path);
      }
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Couldn't save the image.");
    } finally {
      setBusy(false);
    }
  };

  const copy = async () => {
    const data = png();
    if (!data) {
      return;
    }
    setBusy(true);
    try {
      await copyEditedPng(data);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Couldn't copy the image.");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div id="capture-editor" className="flex h-full min-h-0 flex-col bg-zinc-950 text-zinc-100">
      <div id="capture-editor-bar" className="flex flex-wrap items-center gap-2 border-b border-zinc-800 px-4 py-3">
        <h1 id="capture-editor-title" className="mr-2 text-sm font-semibold">
          Screenshot
        </h1>
        {TOOLS.map((item) => {
          const Icon = item.icon;
          return (
            <button
              id={`capture-editor-tool-${item.id}`}
              key={item.id}
              type="button"
              className={cn(
                "inline-flex h-8 items-center gap-1.5 rounded-md px-2 text-xs",
                tool === item.id ? "bg-sky-600 text-white" : "bg-zinc-800 text-zinc-300 hover:bg-zinc-700",
              )}
              onClick={() => setTool(item.id)}
            >
              <Icon id={`capture-editor-tool-${item.id}-icon`} className="size-3.5" aria-hidden />
              {item.label}
            </button>
          );
        })}
        {tool === "text" ? (
          <input
            id="capture-editor-text"
            className="h-8 rounded-md border border-zinc-700 bg-zinc-900 px-2 text-xs"
            value={text}
            onChange={(event) => setText(event.target.value)}
          />
        ) : null}
        <div id="capture-editor-actions" className="ml-auto flex gap-2">
          <Button id="capture-editor-copy" disabled={busy} onClick={() => void copy()}>
            <Copy id="capture-editor-copy-icon" className="size-4" aria-hidden />
            Copy
          </Button>
          <Button id="capture-editor-save" variant="primary" disabled={busy} onClick={() => void save()}>
            <Save id="capture-editor-save-icon" className="size-4" aria-hidden />
            Save
          </Button>
        </div>
      </div>
      {error ? (
        <p id="capture-editor-error" className="px-4 py-2 text-sm text-amber-300">
          {error}
        </p>
      ) : null}
      <div id="capture-editor-stage" className="min-h-0 flex-1 overflow-auto p-4">
        <canvas
          id="capture-editor-canvas"
          ref={canvasRef}
          className="max-w-full cursor-crosshair bg-zinc-900"
          onPointerDown={onDown}
          onPointerMove={onMove}
          onPointerUp={onUp}
        />
      </div>
    </div>
  );
}

function drawArrow(context: CanvasRenderingContext2D, x1: number, y1: number, x2: number, y2: number) {
  const angle = Math.atan2(y2 - y1, x2 - x1);
  context.beginPath();
  context.moveTo(x1, y1);
  context.lineTo(x2, y2);
  context.stroke();
  context.beginPath();
  context.moveTo(x2, y2);
  context.lineTo(x2 - 16 * Math.cos(angle - 0.4), y2 - 16 * Math.sin(angle - 0.4));
  context.lineTo(x2 - 16 * Math.cos(angle + 0.4), y2 - 16 * Math.sin(angle + 0.4));
  context.closePath();
  context.fillStyle = "#38bdf8";
  context.fill();
}

function blurRect(context: CanvasRenderingContext2D, canvas: HTMLCanvasElement, x1: number, y1: number, x2: number, y2: number) {
  const x = Math.max(0, Math.min(x1, x2));
  const y = Math.max(0, Math.min(y1, y2));
  const width = Math.min(canvas.width - x, Math.abs(x2 - x1));
  const height = Math.min(canvas.height - y, Math.abs(y2 - y1));
  if (width < 4 || height < 4) {
    return;
  }
  const small = document.createElement("canvas");
  small.width = Math.max(1, Math.round(width / 8));
  small.height = Math.max(1, Math.round(height / 8));
  const scaled = small.getContext("2d");
  if (!scaled) {
    return;
  }
  scaled.drawImage(canvas, x, y, width, height, 0, 0, small.width, small.height);
  context.imageSmoothingEnabled = true;
  context.drawImage(small, 0, 0, small.width, small.height, x, y, width, height);
}
