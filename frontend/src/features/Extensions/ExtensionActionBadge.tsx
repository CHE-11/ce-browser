type ExtensionActionBadgeProps = {
  id: string;
  text: string;
  color: string;
  textColor: string;
};

/** Chrome-style badge on a toolbar icon. Empty text draws nothing. */
export function ExtensionActionBadge({ id, text, color, textColor }: ExtensionActionBadgeProps) {
  if (!text) {
    return null;
  }
  return (
    <span
      id={id}
      className="pointer-events-none absolute -bottom-1 -right-1.5 z-[1] max-w-7 truncate rounded-sm px-0.5 text-center text-[8px] font-bold leading-[10px]"
      style={{ backgroundColor: color, color: textColor }}
      aria-hidden
    >
      {text}
    </span>
  );
}
