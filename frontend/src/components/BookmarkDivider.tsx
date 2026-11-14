import type { HTMLAttributes } from "react";
import {
  BOOKMARK_DIVIDER_COLOR,
  bookmarkDividerFrameStyle,
  bookmarkDividerMarkStyle,
  type BookmarkDividerMetrics,
} from "@/lib/bookmark-divider";
import { cn } from "@/lib/cn";

type BookmarkDividerProps = {
  id: string;
  orientation: "vertical" | "horizontal";
  metrics: BookmarkDividerMetrics;
  /** Words centered in a horizontal line. A vertical divider ignores this. */
  label?: string;
  color?: string;
  className?: string;
  title?: string;
} & Omit<HTMLAttributes<HTMLDivElement>, "id" | "role" | "color" | "title">;

/** A full-width piece of a horizontal rule. Length is applied to the whole divider, not each piece. */
function horizontalPiece(id: string, metrics: BookmarkDividerMetrics, color: string) {
  const style = bookmarkDividerMarkStyle("horizontal", { ...metrics, length: 100 }, color);
  style.width = "100%";
  return <span id={id} style={style} />;
}

/** A bookmark separator. Vertical lines sit on the bar. Horizontal lines sit in folders and lists. */
export function BookmarkDivider({
  id,
  orientation,
  metrics,
  label,
  color = BOOKMARK_DIVIDER_COLOR,
  className,
  title,
  ...rest
}: BookmarkDividerProps) {
  const vertical = orientation === "vertical";
  const text = vertical ? "" : (label?.trim() ?? "");
  const labeled = text.length > 0;
  return (
    <div
      id={id}
      title={title ?? (text || "Divider")}
      className={cn("flex min-w-0 items-center justify-center", vertical && "shrink-0 self-stretch", className)}
      style={bookmarkDividerFrameStyle(orientation, metrics, labeled)}
      {...rest}
      role="separator"
      aria-orientation={orientation}
    >
      {labeled ? (
        <div id={`${id}-rule`} className="flex min-w-0 items-center" style={{ width: `${metrics.length}%` }}>
          <span id={`${id}-start`} className="flex min-w-0 flex-1 items-center">
            {horizontalPiece(`${id}-line-start`, metrics, color)}
          </span>
          <span id={`${id}-label`} className="shrink-0 px-2 text-xs text-zinc-400">
            {text}
          </span>
          <span id={`${id}-end`} className="flex min-w-0 flex-1 items-center">
            {horizontalPiece(`${id}-line-end`, metrics, color)}
          </span>
        </div>
      ) : (
        <div id={`${id}-line`} style={bookmarkDividerMarkStyle(orientation, metrics, color)} />
      )}
    </div>
  );
}
