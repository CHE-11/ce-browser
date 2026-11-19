import { useEffect, useState } from "react";
import { Download } from "lucide-react";
import { Button } from "@/components/Button";
import { PopupFrame } from "@/components/PopupFrame";
import { closePopup, downloadPageMedia, pageMedia } from "@/lib/browser-client";
import type { PageMedia } from "@type/browser";

type MediaGridProps = {
  tabId: number;
};

export function MediaGrid({ tabId }: MediaGridProps) {
  const [items, setItems] = useState<PageMedia[]>([]);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let gone = false;
    void pageMedia(tabId)
      .then((next) => {
        if (!gone) {
          setItems(next);
        }
      })
      .catch((reason: unknown) => {
        if (!gone) {
          setError(reason instanceof Error ? reason.message : "Couldn't read this page.");
        }
      });
    return () => {
      gone = true;
    };
  }, [tabId]);

  const save = async (urls: string[]) => {
    setBusy(true);
    try {
      await downloadPageMedia(tabId, urls.slice(0, 40));
      void closePopup();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Couldn't start the download.");
    } finally {
      setBusy(false);
    }
  };

  return (
    <PopupFrame id="media-grid" width={420} maxHeight={520}>
      <div id="media-grid-header" className="flex items-center gap-2 px-4 pb-2 pt-3">
        <h2 id="media-grid-title" className="flex-1 text-sm font-semibold">
          Media on this page
        </h2>
        <Button id="media-grid-save-all" variant="primary" disabled={busy || items.length === 0} onClick={() => void save(items.map((item) => item.url))}>
          <Download id="media-grid-save-all-icon" className="size-4" aria-hidden />
          Save all
        </Button>
      </div>
      {error ? (
        <p id="media-grid-error" className="px-4 pb-3 text-xs text-amber-300">
          {error}
        </p>
      ) : null}
      {items.length === 0 && !error ? (
        <p id="media-grid-empty" className="px-4 pb-4 text-sm text-zinc-500">
          No images or media on this page.
        </p>
      ) : (
        <div id="media-grid-items" className="grid grid-cols-3 gap-2 px-3 pb-3">
          {items.map((item, index) => (
            <button
              id={`media-grid-item-${index}`}
              key={`${item.url}-${index}`}
              type="button"
              title={item.url}
              disabled={busy}
              className="overflow-hidden rounded-md bg-zinc-800"
              onClick={() => void save([item.url])}
            >
              {item.kind === "image" ? (
                <img id={`media-grid-thumb-${index}`} src={item.url} alt="" className="aspect-square w-full object-cover" />
              ) : (
                <span id={`media-grid-kind-${index}`} className="flex aspect-square items-center justify-center text-xs text-zinc-400">
                  {item.kind}
                </span>
              )}
            </button>
          ))}
        </div>
      )}
    </PopupFrame>
  );
}
