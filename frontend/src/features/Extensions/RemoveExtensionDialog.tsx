import { ConfirmDialog } from "@/components/ConfirmDialog";

export function RemoveExtensionDialog({
  id,
  name,
  open,
  onCancel,
  onConfirm,
}: {
  id: string;
  name: string | null;
  open: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <ConfirmDialog
      id={id}
      title="Remove this extension?"
      open={open}
      confirm={{ label: "Remove", variant: "destructive", onClick: onConfirm }}
      onCancel={onCancel}
    >
      {name ? `${name} will be uninstalled.` : "This extension will be uninstalled."}
    </ConfirmDialog>
  );
}
