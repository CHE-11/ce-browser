const KEY_NAMES: Record<string, string> = {
  ctrl: "Ctrl",
  shift: "Shift",
  alt: "Alt",
  plus: "+",
  minus: "-",
  slash: "/",
  comma: ",",
  period: ".",
  semicolon: ";",
  quote: "'",
  bracketleft: "[",
  bracketright: "]",
  backslash: "\\",
  backquote: "`",
  left: "Left",
  right: "Right",
  pageup: "Page Up",
  pagedown: "Page Down",
  tab: "Tab",
  esc: "Esc",
  space: "Space",
};

/** One stored token such as `ctrl+shift+a`, as people read it: `Ctrl+Shift+A`. */
export function formatKeyToken(token: string) {
  return token
    .split("+")
    .map((part) => KEY_NAMES[part] ?? (part.length === 1 ? part.toUpperCase() : part.toUpperCase()))
    .join("+");
}

/** A binding's keys, with a chord as `Ctrl+K then T`. */
export function formatKeys(keys: string[]) {
  return keys.map(formatKeyToken).join(" then ");
}
