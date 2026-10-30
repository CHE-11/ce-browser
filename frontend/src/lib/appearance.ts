export const THEME_KEY = "ce-browser.theme";
export const COLOR_MODE_KEY = "ce-browser.color-mode";

export const THEME_IDS = [
  "zinc",
  "nord",
  "catppuccin",
  "solarized",
  "dracula",
  "gruvbox",
  "tokyo",
  "rosepine",
  "github",
  "onedark",
] as const;
export const COLOR_MODES = ["dark", "light", "system"] as const;

export type ThemeId = (typeof THEME_IDS)[number];
export type ColorMode = (typeof COLOR_MODES)[number];
export type ResolvedMode = "dark" | "light";

export const DEFAULT_THEME: ThemeId = "zinc";
export const DEFAULT_COLOR_MODE: ColorMode = "dark";

export type ThemeSwatch = {
  background: string;
  surface: string;
  accent: string;
  text: string;
};

export const THEMES: { id: ThemeId; label: string; swatch: Record<ResolvedMode, ThemeSwatch> }[] = [
  {
    id: "zinc",
    label: "Zinc",
    swatch: {
      dark: { background: "#09090b", surface: "#27272a", accent: "#0284c7", text: "#f4f4f5" },
      light: { background: "#fafafa", surface: "#f4f4f5", accent: "#0284c7", text: "#18181b" },
    },
  },
  {
    id: "nord",
    label: "Nord",
    swatch: {
      dark: { background: "#2e3440", surface: "#3b4252", accent: "#88c0d0", text: "#eceff4" },
      light: { background: "#eceff4", surface: "#e5e9f0", accent: "#5e81ac", text: "#2e3440" },
    },
  },
  {
    id: "catppuccin",
    label: "Catppuccin",
    swatch: {
      dark: { background: "#1e1e2e", surface: "#313244", accent: "#89b4fa", text: "#cdd6f4" },
      light: { background: "#eff1f5", surface: "#e6e9ef", accent: "#1e66f5", text: "#4c4f69" },
    },
  },
  {
    id: "solarized",
    label: "Solarized",
    swatch: {
      dark: { background: "#002b36", surface: "#073642", accent: "#268bd2", text: "#eee8d5" },
      light: { background: "#fdf6e3", surface: "#eee8d5", accent: "#268bd2", text: "#002b36" },
    },
  },
  {
    id: "dracula",
    label: "Dracula",
    swatch: {
      dark: { background: "#282a36", surface: "#44475a", accent: "#bd93f9", text: "#f8f8f2" },
      light: { background: "#f8f8f2", surface: "#e6e6e0", accent: "#6d4eb2", text: "#282a36" },
    },
  },
  {
    id: "gruvbox",
    label: "Gruvbox",
    swatch: {
      dark: { background: "#282828", surface: "#3c3836", accent: "#83a598", text: "#ebdbb2" },
      light: { background: "#fbf1c7", surface: "#ebdbb2", accent: "#076678", text: "#3c3836" },
    },
  },
  {
    id: "tokyo",
    label: "Tokyo Night",
    swatch: {
      dark: { background: "#1a1b26", surface: "#292e42", accent: "#7aa2f7", text: "#c0caf5" },
      light: { background: "#e1e2e7", surface: "#d0d5e3", accent: "#2e7de9", text: "#3760bf" },
    },
  },
  {
    id: "rosepine",
    label: "Rosé Pine",
    swatch: {
      dark: { background: "#191724", surface: "#26233a", accent: "#c4a7e7", text: "#e0def4" },
      light: { background: "#faf4ed", surface: "#f2e9e1", accent: "#907aa9", text: "#575279" },
    },
  },
  {
    id: "github",
    label: "GitHub",
    swatch: {
      dark: { background: "#0d1117", surface: "#161b22", accent: "#58a6ff", text: "#e6edf3" },
      light: { background: "#ffffff", surface: "#f6f8fa", accent: "#0969da", text: "#1f2328" },
    },
  },
  {
    id: "onedark",
    label: "One Dark",
    swatch: {
      dark: { background: "#282c34", surface: "#3e4451", accent: "#61afef", text: "#abb2bf" },
      light: { background: "#fafafa", surface: "#f0f0f0", accent: "#4078f2", text: "#383a42" },
    },
  },
];

export const COLOR_MODE_OPTIONS: { id: ColorMode; label: string }[] = [
  { id: "dark", label: "Dark" },
  { id: "light", label: "Light" },
  { id: "system", label: "Match system" },
];

type Appearance = {
  theme: ThemeId;
  colorMode: ColorMode;
};

let current: Appearance = { theme: DEFAULT_THEME, colorMode: DEFAULT_COLOR_MODE };
let listening = false;

export function normalizeTheme(value: string | null | undefined): ThemeId {
  return THEME_IDS.find((theme) => theme === value) ?? DEFAULT_THEME;
}

export function normalizeColorMode(value: string | null | undefined): ColorMode {
  return COLOR_MODES.find((mode) => mode === value) ?? DEFAULT_COLOR_MODE;
}

export function resolveMode(colorMode: ColorMode): ResolvedMode {
  if (colorMode === "light" || colorMode === "dark") {
    return colorMode;
  }
  if (typeof window === "undefined") {
    return "dark";
  }
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

function readStored(key: string) {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

export function readAppearance(): Appearance {
  return {
    theme: normalizeTheme(readStored(THEME_KEY)),
    colorMode: normalizeColorMode(readStored(COLOR_MODE_KEY)),
  };
}

function writeAttributes(theme: ThemeId, colorMode: ColorMode) {
  const root = document.documentElement;
  root.dataset.theme = theme;
  root.dataset.mode = resolveMode(colorMode);
}

function ensureSystemListener() {
  if (listening || typeof window === "undefined") {
    return;
  }
  listening = true;
  window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
    if (current.colorMode === "system") {
      writeAttributes(current.theme, current.colorMode);
    }
  });
}

export function applyAppearance(theme: ThemeId, colorMode: ColorMode) {
  current = { theme, colorMode };
  writeAttributes(theme, colorMode);
  ensureSystemListener();
}

function rememberAppearance(theme: ThemeId, colorMode: ColorMode) {
  try {
    localStorage.setItem(THEME_KEY, theme);
    localStorage.setItem(COLOR_MODE_KEY, colorMode);
  } catch {
    // Ignore quota and private-mode failures; the look still applies for this session.
  }
}

export function publishAppearance(theme: string, colorMode: string) {
  const next = {
    theme: normalizeTheme(theme),
    colorMode: normalizeColorMode(colorMode),
  };
  applyAppearance(next.theme, next.colorMode);
  rememberAppearance(next.theme, next.colorMode);
  return next;
}
