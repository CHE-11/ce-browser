import { useCallback, useRef } from "react";
import { setPopupShape } from "./browser-client";

/** Marks an element whose box is drawn. Everything else in the popup shows the page below. */
export const POPUP_SHAPE_ATTR = "data-popup-shape";

/**
 * Clips the popup to the elements marked with `POPUP_SHAPE_ATTR`. Call `report` after any of
 * them moves, resizes, mounts or unmounts. `settle` resolves when the last shape is applied.
 */
export function usePopupShape() {
  const last = useRef("");
  const queued = useRef(false);
  const applied = useRef<Promise<unknown>>(Promise.resolve());

  const report = useCallback(() => {
    if (queued.current) {
      return;
    }
    queued.current = true;
    queueMicrotask(() => {
      queued.current = false;
      const shape = [...document.querySelectorAll<HTMLElement>(`[${POPUP_SHAPE_ATTR}]`)].map((element) => {
        const rect = element.getBoundingClientRect();
        const x = Math.floor(rect.left);
        const y = Math.floor(rect.top);
        return { x, y, width: Math.ceil(rect.right) - x, height: Math.ceil(rect.bottom) - y };
      });
      const key = JSON.stringify(shape);
      if (shape.length === 0 || key === last.current) {
        return;
      }
      last.current = key;
      applied.current = setPopupShape(shape).catch(() => undefined);
    });
  }, []);

  const settle = useCallback(() => applied.current, []);

  return { report, settle };
}
