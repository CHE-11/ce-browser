import { useCallback, useEffect, useRef, useState } from "react";

export type AutomationsView = "flows" | "history" | "datasets";

/** Where ce://automations is: a view, the open flow or dataset, and a step to scroll to. */
export type AutomationsLocation = {
  view: AutomationsView;
  /** A flow id, `new` for an unsaved flow, or null for none. */
  flowId: string | null;
  datasetId: string | null;
  step: number | null;
};

export const NEW_FLOW = "new";

export function parseAutomationsLocation(params: URLSearchParams): AutomationsLocation {
  const tab = params.get("tab");
  const datasetId = params.get("dataset");
  const step = Number(params.get("step"));
  const view: AutomationsView = datasetId || tab === "datasets" ? "datasets" : tab === "history" ? "history" : "flows";
  return {
    view,
    flowId: view === "flows" ? params.get("flow") : null,
    datasetId,
    step: params.has("step") && Number.isInteger(step) && step >= 0 ? step : null,
  };
}

export function automationsHash(location: AutomationsLocation) {
  const params = new URLSearchParams();
  if (location.view === "history") {
    params.set("tab", "history");
  } else if (location.view === "datasets") {
    params.set("tab", "datasets");
    if (location.datasetId) {
      params.set("dataset", location.datasetId);
    }
  } else if (location.flowId) {
    params.set("flow", location.flowId);
    if (location.step !== null) {
      params.set("step", String(location.step));
    }
  }
  const query = params.toString();
  return `#/automations${query ? `?${query}` : ""}`;
}

/** True when moving from `from` to `to` closes the flow that is open. */
export function leavesFlow(from: AutomationsLocation, to: AutomationsLocation) {
  return from.view === "flows" && from.flowId !== null && (to.view !== "flows" || to.flowId !== from.flowId);
}

function sameLocation(a: AutomationsLocation, b: AutomationsLocation) {
  return a.view === b.view && a.flowId === b.flowId && a.datasetId === b.datasetId && a.step === b.step;
}

/**
 * Follows the address of ce://automations, so Back, Forward and reloading keep the page where it was.
 * `canLeave` runs before the open flow closes. When it returns false the move waits in `pending`
 * until `confirmLeave` or `cancelLeave`.
 */
export function useAutomationsRoute(params: URLSearchParams, canLeave: () => boolean) {
  const query = params.toString();
  const [shown, setShown] = useState(() => parseAutomationsLocation(params));
  const [pending, setPending] = useState<AutomationsLocation | null>(null);
  const shownRef = useRef(shown);
  shownRef.current = shown;
  const pendingRef = useRef(pending);
  pendingRef.current = pending;
  const canLeaveRef = useRef(canLeave);
  canLeaveRef.current = canLeave;

  const apply = useCallback((next: AutomationsLocation) => {
    const current = shownRef.current;
    if (sameLocation(next, current)) {
      setPending(null);
      return;
    }
    if (leavesFlow(current, next) && !canLeaveRef.current()) {
      setPending(next);
      return;
    }
    setPending(null);
    setShown(next);
  }, []);

  useEffect(() => {
    apply(parseAutomationsLocation(new URLSearchParams(query)));
  }, [query, apply]);

  /** Moves the page. `replace` changes the address without adding a Back entry. */
  const go = useCallback(
    (next: Partial<AutomationsLocation> & { view: AutomationsView }, replace = false) => {
      const location: AutomationsLocation = { flowId: null, datasetId: null, step: null, ...next };
      const hash = automationsHash(location);
      if (window.location.hash !== hash) {
        if (replace) {
          window.location.replace(hash);
        } else {
          window.location.hash = hash;
        }
      }
      apply(location);
    },
    [apply],
  );

  const confirmLeave = useCallback(() => {
    const next = pendingRef.current;
    setPending(null);
    if (next) {
      setShown(next);
    }
  }, []);

  const cancelLeave = useCallback(() => {
    setPending(null);
    window.history.replaceState(window.history.state, "", automationsHash(shownRef.current));
  }, []);

  return { location: shown, pending, go, confirmLeave, cancelLeave };
}
