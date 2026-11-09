import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";

/** Keep in sync with the Debug HUD id in `frontend/src/features/BuiltInExtensions/builtin.ts`. */
const DEBUG_HUD_ID = "debug-hud";

function currentLabel() {
  try {
    return getCurrentWebview().label;
  } catch {
    return "";
  }
}

export const debugCaptureLabel = currentLabel();
/** The HUD does not record itself, or its own refreshes would fill the feed. */
export const debugCaptureEligible = debugCaptureLabel !== "" && !debugCaptureLabel.startsWith("debug-");

let on = import.meta.env.DEV && debugCaptureEligible;

/** IPC and console capture follow the Debug HUD extension, in dev and release builds. */
export function debugCaptureOn() {
  return on;
}

function apply(disabled: string[] | undefined) {
  if (!debugCaptureEligible) {
    return;
  }
  on = !(disabled ?? []).includes(DEBUG_HUD_ID);
}

if (debugCaptureEligible) {
  void invoke<{ disabledBuiltinTools?: string[] }>("get_settings")
    .then((settings) => apply(settings.disabledBuiltinTools))
    .catch(() => undefined);
  void listen<{ disabledBuiltinTools?: string[] }>("settings-changed", (event) => {
    apply(event.payload.disabledBuiltinTools);
  });
}
