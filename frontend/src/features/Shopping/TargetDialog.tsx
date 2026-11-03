import { useState } from "react";
import { Button } from "@/components/Button";
import { Dialog } from "@/components/Dialog";
import type { ProductPrice } from "@type/browser";

export function TargetDialog({
  item,
  onClose,
  onSave,
}: {
  item: ProductPrice | null;
  onClose: () => void;
  onSave: (item: ProductPrice, target: number | null, alerts: { alertOnDrop: boolean; alertBelowTarget: boolean; alertBackInStock: boolean }) => void;
}) {
  const [target, setTarget] = useState(item?.targetPrice?.toString() ?? "");
  const [alertOnDrop, setAlertOnDrop] = useState(item?.alertOnDrop ?? true);
  const [alertBelowTarget, setAlertBelowTarget] = useState(item?.alertBelowTarget ?? true);
  const [alertBackInStock, setAlertBackInStock] = useState(item?.alertBackInStock ?? true);

  return (
    <Dialog id="shopping-target-dialog" title="Target price" open={item !== null} onClose={onClose}>
      {item ? (
        <form
          id="shopping-target-form"
          className="flex flex-col gap-3"
          onSubmit={(event) => {
            event.preventDefault();
            const value = target.trim() ? Number(target) : null;
            onSave(item, value !== null && Number.isFinite(value) ? value : null, { alertOnDrop, alertBelowTarget, alertBackInStock });
          }}
        >
          <label id="shopping-target-label" className="flex flex-col gap-1 text-sm" htmlFor="shopping-target-input">
            Alert at or below
            <input
              id="shopping-target-input"
              className="h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2"
              inputMode="decimal"
              value={target}
              onChange={(event) => setTarget(event.target.value)}
            />
          </label>
          <label id="shopping-alert-drop" className="flex items-center gap-2 text-sm">
            <input id="shopping-alert-drop-input" type="checkbox" checked={alertOnDrop} onChange={(event) => setAlertOnDrop(event.target.checked)} />
            Price drops
          </label>
          <label id="shopping-alert-target" className="flex items-center gap-2 text-sm">
            <input id="shopping-alert-target-input" type="checkbox" checked={alertBelowTarget} onChange={(event) => setAlertBelowTarget(event.target.checked)} />
            At or below the target
          </label>
          <label id="shopping-alert-stock" className="flex items-center gap-2 text-sm">
            <input id="shopping-alert-stock-input" type="checkbox" checked={alertBackInStock} onChange={(event) => setAlertBackInStock(event.target.checked)} />
            Back in stock
          </label>
          <div id="shopping-target-actions" className="flex justify-end gap-2">
            <Button id="shopping-target-cancel" variant="ghost" onClick={onClose}>
              Cancel
            </Button>
            <Button id="shopping-target-save" type="submit" variant="primary">
              Save
            </Button>
          </div>
        </form>
      ) : null}
    </Dialog>
  );
}
