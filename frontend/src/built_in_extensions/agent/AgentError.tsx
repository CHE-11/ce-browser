import { useState } from "react";
import { Button } from "@/components/Button";
import { cn } from "@/lib/cn";

const PREVIEW_CHARS = 180;

function isLongMessage(text: string) {
  return text.length > PREVIEW_CHARS || text.split("\n").length > 4;
}

/** Error text that stays wrapped, and opens the rest when the provider message is long. */
export function LongMessage({
  id,
  text,
  className,
  moreLabel = "Show full error",
}: {
  id: string;
  text: string;
  className?: string;
  moreLabel?: string;
}) {
  return <LongMessageBody key={text} id={id} text={text} className={className} moreLabel={moreLabel} />;
}

function LongMessageBody({
  id,
  text,
  className,
  moreLabel,
}: {
  id: string;
  text: string;
  className?: string;
  moreLabel: string;
}) {
  const long = isLongMessage(text);
  const [open, setOpen] = useState(false);

  return (
    <div id={id}>
      <div
        id={`${id}-text`}
        className={cn(
          "whitespace-pre-wrap break-words",
          className,
          long && !open && "line-clamp-3",
          long && open && "max-h-48 overflow-y-auto",
        )}
      >
        {text}
      </div>
      {long ? (
        <Button
          id={`${id}-toggle`}
          variant="link"
          size="xs"
          className="mt-1"
          aria-expanded={open}
          onClick={() => setOpen((value) => !value)}
        >
          {open ? "Show less" : moreLabel}
        </Button>
      ) : null}
    </div>
  );
}

export function AgentError({ id, text, alert = false }: { id: string; text: string; alert?: boolean }) {
  return (
    <div id={id} role={alert ? "alert" : undefined} className="rounded-lg bg-red-950/40 p-3 ring-1 ring-red-900/60">
      <LongMessage id={`${id}-body`} text={text} className="text-xs text-red-300" />
    </div>
  );
}
