export function hostOf(url) {
  try {
    return new URL(url).hostname || "page";
  } catch {
    return "page";
  }
}
