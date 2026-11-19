import { Terminal } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { toggleConsolePanel } from "@/lib/browser-client";
import { useTabs } from "@/lib/use-browser";
import { useConsoleErrorCount } from "./use-console-errors";

function badgeText(count: number) {
  return count > 99 ? "99+" : String(count);
}

function consoleLabel(errors: number) {
  if (errors <= 0) {
    return "Console";
  }
  const shown = badgeText(errors);
  return `Console, ${shown} ${errors === 1 ? "error" : "errors"}`;
}

/** Opens or closes the console beside the pages. */
export function ConsoleButton() {
  const { consolePanel } = useTabs();
  const errors = useConsoleErrorCount();
  const label = consoleLabel(errors);
  return (
    <IconButton
      id="address-console"
      label={label}
      active={consolePanel}
      aria-pressed={consolePanel}
      className="relative"
      onClick={() => void toggleConsolePanel()}
    >
      <Terminal id="address-console-icon" aria-hidden />
      {errors > 0 ? (
        <span
          id="address-console-badge"
          className="pointer-events-none absolute -top-0.5 -right-0.5 min-w-4 rounded-full bg-red-600 px-1 text-center text-[9px] font-semibold leading-4 text-white"
          aria-hidden
        >
          {badgeText(errors)}
        </span>
      ) : null}
    </IconButton>
  );
}
