import { useEffect, useRef, useState, type MouseEvent } from "react";

/** Right click copies `text`, or the selected text when there is a selection. */
export function useCopyOnRightClick(onStatus: (text: string | null) => void) {
  const [copied, setCopied] = useState(false);
  const timer = useRef(0);

  useEffect(() => () => window.clearTimeout(timer.current), []);

  const copy = (event: MouseEvent, text: string) => {
    event.preventDefault();
    event.stopPropagation();
    const selected = window.getSelection()?.toString() ?? "";
    void navigator.clipboard.writeText(selected || text).then(
      () => {
        onStatus(null);
        setCopied(true);
        window.clearTimeout(timer.current);
        timer.current = window.setTimeout(() => setCopied(false), 1200);
      },
      (reason: unknown) => onStatus(String(reason)),
    );
  };

  return { copied, copy };
}
