import { Copy } from "lucide-react";
import { Button } from "@/components/Button";
import { Dialog } from "@/components/Dialog";

type PlaywrightPanelProps = {
  script: string | null;
  onCopy: (script: string) => void;
  onClose: () => void;
};

/** The flow as a Playwright test, to copy into a project. */
export function PlaywrightPanel({ script, onCopy, onClose }: PlaywrightPanelProps) {
  return (
    <Dialog id="flow-editor-script" title="Playwright test" open={script !== null} onClose={onClose} panelClassName="max-w-3xl">
      <p id="flow-editor-script-hint" className="mb-2 text-xs text-zinc-500">
        Extract, AI and notify steps only run in ce-browser, so they become comments. Secrets read from environment variables.
      </p>
      <pre id="flow-editor-script-code" className="max-h-[55vh] overflow-auto rounded-md border border-zinc-800 bg-zinc-950 p-3 font-mono text-xs text-zinc-300">
        {script}
      </pre>
      <div id="flow-editor-script-actions" className="mt-3 flex justify-end gap-2">
        <Button id="flow-editor-script-close" size="sm" variant="ghost" onClick={onClose}>
          Close
        </Button>
        <Button id="flow-editor-script-copy" size="sm" variant="primary" onClick={() => script !== null && onCopy(script)}>
          <Copy id="flow-editor-script-copy-icon" className="size-3.5" aria-hidden />
          Copy
        </Button>
      </div>
    </Dialog>
  );
}
