import { useState } from "react";
import { Bell, Camera, Link2, MapPin, Mic, Shield, X, type LucideIcon } from "lucide-react";
import { Button } from "@/components/Button";
import { IconButton } from "@/components/IconButton";
import { PopupFrame } from "@/components/PopupFrame";
import { answerPermission, closeThisPopup } from "@/lib/browser-client";

type PermissionPromptProps = {
  params: URLSearchParams;
};

function iconFor(kind: string): LucideIcon {
  if (kind === "camera") {
    return Camera;
  }
  if (kind === "microphone") {
    return Mic;
  }
  if (kind === "location") {
    return MapPin;
  }
  if (kind === "notifications") {
    return Bell;
  }
  if (kind.startsWith("protocol:")) {
    return Link2;
  }
  return Shield;
}

/** Replaces the WebView2 permission bubble. Padding stays even, and the window hugs the content. */
export function PermissionPrompt({ params }: PermissionPromptProps) {
  const id = Number(params.get("id"));
  const host = params.get("host") || "This site";
  const label = params.get("label") || "Use a feature of this site";
  const kind = params.get("kind") || "";
  const [busy, setBusy] = useState(false);
  const Icon = iconFor(kind);

  const choose = (choice: "allow" | "block" | "dismiss") => {
    if (!Number.isFinite(id) || busy) {
      return;
    }
    setBusy(true);
    void answerPermission(id, choice).finally(() => closeThisPopup());
  };

  return (
    <PopupFrame id="permission-prompt" width={360} role="dialog" labelledBy="permission-prompt-title">
      <div id="permission-prompt-body" className="flex flex-col gap-3 p-4">
        <div id="permission-prompt-header" className="flex items-center gap-2">
          <h2 id="permission-prompt-title" className="min-w-0 flex-1 truncate text-sm font-medium text-zinc-100" title={`${host} wants to`}>
            <span id="permission-prompt-host">{host}</span> wants to
          </h2>
          <IconButton id="permission-prompt-close" label="Dismiss" disabled={busy} onClick={() => choose("dismiss")}>
            <X id="permission-prompt-close-icon" aria-hidden />
          </IconButton>
        </div>
        <div id="permission-prompt-request" className="flex items-center gap-2.5">
          <Icon id="permission-prompt-icon" className="size-4 shrink-0 text-zinc-300" aria-hidden />
          <p id="permission-prompt-label" className="text-sm text-zinc-100">
            {label}
          </p>
        </div>
        <div id="permission-prompt-actions" className="flex justify-end gap-2">
          <Button id="permission-prompt-block" variant="outline" className="px-3" disabled={busy} onClick={() => choose("block")}>
            Block
          </Button>
          <Button id="permission-prompt-allow" variant="primary" className="px-3" disabled={busy} autoFocus onClick={() => choose("allow")}>
            Allow
          </Button>
        </div>
      </div>
    </PopupFrame>
  );
}
