type Begin = (force: boolean) => void;
type Cancel = () => void;

type Handlers = {
  begin: Begin;
  cancel: Cancel;
};

let handlers: Handlers | null = null;

/** The address bar registers this. The new-tab button calls it in the click, before the page view exists. */
export function registerNewTabTyping(next: Handlers) {
  handlers = next;
  return () => {
    if (handlers === next) {
      handlers = null;
    }
  };
}

/**
 * Clears and focuses the address bar in this turn.
 * `force` starts a new gesture. A repeat from the backend, while one is already open, only refocuses.
 */
export function beginNewTabTyping(force = true) {
  handlers?.begin(force);
}

/** The new tab was not created. Puts the address bar back when the user had not typed yet. */
export function cancelNewTabTyping() {
  handlers?.cancel();
}
