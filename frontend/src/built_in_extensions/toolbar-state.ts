import { ensureBuiltinToolbar } from "@/lib/browser-client";
import { BUILTIN_TOOLS } from "./builtin";

/** Stores each built-in tool's pin location, pin status, and toolbar order on the active profile. */
export function saveBuiltinToolbarState() {
  return ensureBuiltinToolbar(BUILTIN_TOOLS.map((tool) => ({ id: tool.id, address: tool.address ?? false })));
}
