import { useEffect, useState } from "react";
import { Copy, Languages } from "lucide-react";
import { Button } from "@/components/Button";
import { PopupFrame } from "@/components/PopupFrame";
import { Skeleton } from "@/components/Skeleton";
import { translateSelection } from "@/lib/browser-client";
import { useSettings } from "@/lib/use-browser";
import { PopoverHeader } from "../PopoverParts";
import { translateLanguageName } from "./languages";

/** Opened from the page's right-click menu: the selected text, translated. */
export function SelectionTranslatePopover({ params }: { params: URLSearchParams }) {
  const text = params.get("text") ?? "";
  const settings = useSettings();
  const [result, setResult] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const language = settings ? translateLanguageName(settings.translateLanguage) : "";

  useEffect(() => {
    let cancelled = false;
    translateSelection(text)
      .then((translated) => {
        if (!cancelled) {
          setResult(translated);
        }
      })
      .catch((reason: unknown) => {
        if (!cancelled) {
          setError(String(reason));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [text]);

  return (
    <PopupFrame id="selection-translate" width={360} maxHeight={420}>
      <PopoverHeader id="selection-translate-header" icon={<Languages aria-hidden />} title="Translation" subtitle={language ? `Into ${language}` : "Selected text"} />
      <div id="selection-translate-body" className="flex flex-col gap-2 px-3 pb-3">
        <p id="selection-translate-source" className="line-clamp-3 text-xs text-zinc-500">
          {text}
        </p>
        {error ? (
          <p id="selection-translate-error" className="text-xs text-amber-300">
            {error}
          </p>
        ) : result === null ? (
          <div id="selection-translate-loading" role="status" aria-label="Translating" className="flex flex-col gap-1.5">
            <Skeleton id="selection-translate-loading-1" className="h-3.5 w-full" />
            <Skeleton id="selection-translate-loading-2" className="h-3.5 w-2/3" />
          </div>
        ) : (
          <p id="selection-translate-result" className="whitespace-pre-wrap text-sm text-zinc-100">
            {result}
          </p>
        )}
        {result ? (
          <Button
            id="selection-translate-copy"
            size="sm"
            className="self-end"
            onClick={() => void navigator.clipboard.writeText(result).then(() => setCopied(true))}
          >
            <Copy id="selection-translate-copy-icon" className="size-3.5" aria-hidden />
            {copied ? "Copied" : "Copy"}
          </Button>
        ) : null}
      </div>
    </PopupFrame>
  );
}
