import { useEffect, useState } from "react";

/** How long the speaker control stays after a tab goes quiet, in case the sound returns. */
const AUDIO_HOLD_MS = 15_000;

type AudioTrack = {
  tabId: number | null;
  audible: boolean;
  /** When this tab last went quiet. Empty until it has played and then stopped. */
  quietAt: number | null;
};

/**
 * `audible` drops out during short pauses, which flashes the mute control.
 * Once a tab has made sound, keep the control visible for 15 seconds after it goes quiet.
 * A muted tab always shows it. Switching tabs follows the new tab at once.
 */
export function useStickyAudio(tabId: number | null, audible: boolean, muted: boolean) {
  const [track, setTrack] = useState<AudioTrack>({ tabId, audible, quietAt: null });

  let current = track;
  if (track.tabId !== tabId) {
    current = { tabId, audible, quietAt: null };
  } else if (track.audible !== audible) {
    current = { tabId, audible, quietAt: audible ? null : Date.now() };
  }
  if (current !== track) {
    setTrack(current);
  }

  const quietAt = current.tabId === tabId ? current.quietAt : null;

  useEffect(() => {
    if (quietAt === null) {
      return;
    }
    const remaining = AUDIO_HOLD_MS - (Date.now() - quietAt);
    if (remaining <= 0) {
      return;
    }
    const timer = window.setTimeout(() => {
      setTrack((latest) => {
        if (latest.tabId !== tabId || latest.quietAt !== quietAt) {
          return latest;
        }
        return { ...latest, quietAt: null };
      });
    }, remaining);
    return () => window.clearTimeout(timer);
  }, [tabId, quietAt]);

  const held = quietAt !== null && Date.now() - quietAt < AUDIO_HOLD_MS;
  return muted || audible || held;
}
