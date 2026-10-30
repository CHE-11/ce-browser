const STYLE_ID = "user-styles";

/** Injects this profile's styles.css. Later rules override the built-in theme. */
export function applyUserStyles(css: string) {
  let style = document.getElementById(STYLE_ID);
  if (!(style instanceof HTMLStyleElement)) {
    style = document.createElement("style");
    style.id = STYLE_ID;
    document.head.appendChild(style);
  }
  style.textContent = css;
}
