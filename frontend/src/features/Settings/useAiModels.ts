import { useCallback, useEffect, useSyncExternalStore } from "react";
import type { AiProviderId } from "@type/browser";
import { listAiModels } from "@/lib/browser-client";
import { errorText } from "./SettingsControls";

type ModelSnapshot = {
  provider: string;
  models: string[];
  loading: boolean;
  error: string;
  loaded: boolean;
};

const EMPTY_MODELS: string[] = [];

let snapshot: ModelSnapshot = {
  provider: "",
  models: EMPTY_MODELS,
  loading: false,
  error: "",
  loaded: false,
};

const subscribers = new Set<() => void>();
let generation = 0;
let running: { provider: string; promise: Promise<void> } | null = null;

function publish(next: ModelSnapshot) {
  snapshot = next;
  for (const subscriber of subscribers) {
    subscriber();
  }
}

function subscribe(subscriber: () => void) {
  subscribers.add(subscriber);
  return () => {
    subscribers.delete(subscriber);
  };
}

function getSnapshot() {
  return snapshot;
}

/** Loads the active provider's models. OpenAI and Claude results are text chat models. */
export function loadAiModels(provider: string, force = false) {
  if (!force && running?.provider === provider) {
    return running.promise;
  }
  if (!force && snapshot.provider === provider && snapshot.loaded && !snapshot.loading) {
    return Promise.resolve();
  }
  const ticket = ++generation;
  const keep = snapshot.provider === provider ? snapshot.models : EMPTY_MODELS;
  publish({ provider, models: keep, loading: true, error: "", loaded: false });
  const promise = listAiModels()
    .then((models) => {
      if (ticket !== generation) {
        return;
      }
      publish({ provider, models, loading: false, error: "", loaded: true });
    })
    .catch((reason: unknown) => {
      if (ticket !== generation) {
        return;
      }
      publish({ provider, models: EMPTY_MODELS, loading: false, error: errorText(reason), loaded: true });
    })
    .finally(() => {
      if (running?.promise === promise) {
        running = null;
      }
    });
  running = { provider, promise };
  return promise;
}

export function useAiModels(provider: AiProviderId, remote: boolean) {
  const current = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
  useEffect(() => {
    if (!remote) {
      return;
    }
    void loadAiModels(provider);
  }, [provider, remote]);
  const refresh = useCallback(() => {
    if (remote) {
      void loadAiModels(provider, true);
    }
  }, [provider, remote]);
  if (!remote) {
    return { models: EMPTY_MODELS, loading: false, error: "", refresh };
  }
  if (current.provider !== provider) {
    return { models: EMPTY_MODELS, loading: true, error: "", refresh };
  }
  return { models: current.models, loading: current.loading, error: current.error, refresh };
}
