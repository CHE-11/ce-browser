import { useEffect } from "react";
import { X } from "lucide-react";
import { cn } from "@/lib/cn";

export type Notice = { tone: "info" | "error"; text: string };

type PageNoticeProps = {
  notice: Notice | null;
  onClose: () => void;
};

/** A short message in the corner of the page. Information goes away by itself, errors stay. */
export function PageNotice({ notice, onClose }: PageNoticeProps) {
  useEffect(() => {
    if (!notice || notice.tone === "error") {
      return;
    }
    const timer = window.setTimeout(onClose, 4500);
    return () => window.clearTimeout(timer);
  }, [notice, onClose]);

  if (!notice) {
    return null;
  }
  return (
    <div
      id="automations-notice"
      role={notice.tone === "error" ? "alert" : "status"}
      className={cn(
        "fixed right-5 bottom-5 z-30 flex max-w-md items-start gap-2 rounded-md border px-3 py-2 text-sm shadow-xl",
        notice.tone === "error" ? "border-red-900 bg-red-950 text-red-200" : "border-zinc-700 bg-zinc-900 text-zinc-200",
      )}
    >
      <span id="automations-notice-text" className="min-w-0 flex-1">
        {notice.text}
      </span>
      <button id="automations-notice-close" type="button" aria-label="Dismiss" className="text-zinc-400 hover:text-zinc-100" onClick={onClose}>
        <X id="automations-notice-close-icon" className="size-4" aria-hidden />
      </button>
    </div>
  );
}
