/** Keep in step with `AI_INPUT_TOKENS_*` and `AI_OUTPUT_TOKENS_*` in settings.rs. */
export const AI_INPUT_TOKENS = { min: 256, max: 1_000_000, fallback: 32_768 };
export const AI_OUTPUT_TOKENS = { min: 64, max: 128_000, fallback: 4_096 };

export function clampTokenLimit(value: number, bounds: { min: number; max: number; fallback: number }) {
  const parsed = Math.round(value);
  if (!Number.isFinite(parsed) || parsed === 0) {
    return bounds.fallback;
  }
  return Math.min(bounds.max, Math.max(bounds.min, parsed));
}
