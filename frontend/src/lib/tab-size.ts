/** Defaults and limits. They match `BrowserSettings::normalize` in settings.rs. */
export const TAB_SIZE = {
  width: 220,
  widthLimits: { min: 88, max: 480 },
  menuText: 0.875,
  menuPadding: 0.625,
  text: { min: 0.5, max: 1.5 },
  padding: { min: 0, max: 1.5 },
  step: 0.025,
} as const;
