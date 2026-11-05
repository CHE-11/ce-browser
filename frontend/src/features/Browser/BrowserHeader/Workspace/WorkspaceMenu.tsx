import { useState } from "react";
import { Check, Pencil, Plus, Trash2 } from "lucide-react";
import type { TabGroupColor, Workspace } from "../../../../../../types/browser";
import { closePopup, createWorkspace, deleteWorkspace, renameWorkspace, switchWorkspace } from "../../../../lib/browser-client";
import { menuRowClass } from "../../../../lib/chrome-metrics";
import { cn } from "../../../../lib/cn";
import { GROUP_COLORS, groupColorClasses } from "../../../../lib/tab-rows";
import { useTabs } from "../../../../lib/use-browser";
import { Button } from "../../../../components/Button";
import { IconButton } from "../../../../components/IconButton";
import { PopupFrame } from "../../../../components/PopupFrame";

const inputClass =
  "h-8 min-w-0 flex-1 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500";

function ColorPicker({ id, value, onChange }: { id: string; value: TabGroupColor; onChange: (color: TabGroupColor) => void }) {
  return (
    <div id={id} role="radiogroup" aria-label="Workspace color" className="flex flex-wrap gap-1.5">
      {GROUP_COLORS.map((color) => (
        <button
          key={color}
          id={`${id}-${color}`}
          type="button"
          role="radio"
          aria-checked={value === color}
          title={color}
          className={cn(
            "inline-flex size-5 items-center justify-center rounded-full ring-offset-2 ring-offset-zinc-900",
            groupColorClasses(color).swatch,
            value === color && "ring-2 ring-zinc-200",
          )}
          onClick={() => onChange(color)}
        >
          {value === color ? <Check id={`${id}-${color}-check`} className="size-3 text-zinc-950" aria-hidden /> : null}
        </button>
      ))}
    </div>
  );
}

export function WorkspaceMenu() {
  const snapshot = useTabs();
  const [editing, setEditing] = useState<Workspace | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const [newName, setNewName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const used = new Set(snapshot.workspaces.map((workspace) => workspace.color));
  const [newColor, setNewColor] = useState<TabGroupColor>(() => GROUP_COLORS.find((color) => !used.has(color)) ?? "green");

  const count = (id: string) => snapshot.windowTabs.filter((tab) => tab.workspace === id).length;

  const report = <T,>(work: Promise<T>) => {
    setError(null);
    return work.catch((reason: unknown): never => {
      setError(String(reason));
      throw reason;
    });
  };

  const switchTo = (id: string) => {
    void report(switchWorkspace(id)).then(() => closePopup(), () => undefined);
  };

  const create = () => {
    void report(createWorkspace(newName, newColor))
      .then((id) => switchWorkspace(id))
      .then(() => closePopup(), () => undefined);
  };

  const saveEdit = () => {
    if (!editing) {
      return;
    }
    void report(renameWorkspace(editing.id, editing.name, editing.color)).then(() => setEditing(null), () => undefined);
  };

  return (
    <PopupFrame id="workspace-menu" width={300} ready={snapshot.loaded}>
      <div id="workspace-menu-body" className="flex flex-col gap-0.5 p-1.5">
        <div id="workspace-menu-title" className="px-2 pb-1 pt-1 text-xs font-medium text-zinc-400">
          Workspaces
        </div>
        {snapshot.workspaces.map((workspace, index) => {
          const current = workspace.id === snapshot.workspace;
          const domId = `workspace-menu-${workspace.id}`;
          if (editing?.id === workspace.id) {
            return (
              <div key={workspace.id} id={`${domId}-edit`} className="flex flex-col gap-2 rounded-lg bg-zinc-800/60 p-2">
                <div id={`${domId}-edit-row`} className="flex items-center gap-1.5">
                  <input
                    id={`${domId}-edit-name`}
                    autoFocus
                    value={editing.name}
                    maxLength={40}
                    className={inputClass}
                    onChange={(event) => setEditing({ ...editing, name: event.target.value })}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") {
                        event.preventDefault();
                        saveEdit();
                      }
                    }}
                  />
                  <Button id={`${domId}-edit-save`} size="sm" onClick={saveEdit}>
                    Save
                  </Button>
                </div>
                <ColorPicker
                  id={`${domId}-edit-color`}
                  value={editing.color}
                  onChange={(color) => setEditing({ ...editing, color })}
                />
              </div>
            );
          }
          if (confirmDelete === workspace.id) {
            const tabs = count(workspace.id);
            return (
              <div key={workspace.id} id={`${domId}-confirm`} className="flex flex-col gap-2 rounded-lg bg-red-950/40 p-2">
                <p id={`${domId}-confirm-text`} className="text-xs text-zinc-200">
                  Delete {workspace.name}? {tabs === 0 ? "It has no tabs in this window." : `Its ${tabs} ${tabs === 1 ? "tab closes" : "tabs close"}.`}
                </p>
                <div id={`${domId}-confirm-actions`} className="flex justify-end gap-1.5">
                  <Button id={`${domId}-confirm-cancel`} size="sm" variant="ghost" onClick={() => setConfirmDelete(null)}>
                    Cancel
                  </Button>
                  <Button
                    id={`${domId}-confirm-delete`}
                    size="sm"
                    variant="destructive"
                    onClick={() => {
                      void report(deleteWorkspace(workspace.id)).then(() => setConfirmDelete(null), () => undefined);
                    }}
                  >
                    Delete
                  </Button>
                </div>
              </div>
            );
          }
          return (
            <div
              key={workspace.id}
              id={domId}
              className={cn("group flex items-center gap-1 rounded-lg hover:bg-zinc-800", menuRowClass, current && "bg-zinc-800")}
            >
              <button
                id={`${domId}-switch`}
                type="button"
                className="flex h-full min-w-0 flex-1 items-center gap-2.5 text-left text-[length:var(--menu-text)] text-zinc-100"
                onClick={() => switchTo(workspace.id)}
              >
                <span id={`${domId}-dot`} className={cn("size-2.5 shrink-0 rounded-full", groupColorClasses(workspace.color).swatch)} />
                <span id={`${domId}-name`} className="min-w-0 flex-1 truncate">
                  {workspace.name}
                </span>
                <span id={`${domId}-count`} className="shrink-0 text-[11px] text-zinc-500">
                  {count(workspace.id)}
                </span>
                {index < 9 ? (
                  <span id={`${domId}-shortcut`} className="shrink-0 text-[11px] text-zinc-500">
                    Ctrl+Alt+{index + 1}
                  </span>
                ) : null}
              </button>
              <IconButton
                id={`${domId}-rename`}
                label={`Rename ${workspace.name}`}
                className="size-7 opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
                onClick={() => setEditing(workspace)}
              >
                <Pencil id={`${domId}-rename-icon`} aria-hidden />
              </IconButton>
              <IconButton
                id={`${domId}-delete`}
                label={`Delete ${workspace.name}`}
                disabled={snapshot.workspaces.length <= 1}
                className="size-7 opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
                onClick={() => setConfirmDelete(workspace.id)}
              >
                <Trash2 id={`${domId}-delete-icon`} aria-hidden />
              </IconButton>
            </div>
          );
        })}
        <div id="workspace-menu-new" className="mt-1 flex flex-col gap-2 border-t border-zinc-800 px-1 pb-1 pt-2">
          <div id="workspace-menu-new-row" className="flex items-center gap-1.5">
            <input
              id="workspace-menu-new-name"
              value={newName}
              maxLength={40}
              placeholder="New workspace"
              className={inputClass}
              onChange={(event) => setNewName(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault();
                  create();
                }
              }}
            />
            <Button id="workspace-menu-new-create" size="sm" onClick={create}>
              <Plus id="workspace-menu-new-create-icon" className="size-3.5" aria-hidden />
              Add
            </Button>
          </div>
          <ColorPicker id="workspace-menu-new-color" value={newColor} onChange={setNewColor} />
        </div>
        {error ? (
          <div id="workspace-menu-error" className="px-2 py-1 text-xs text-red-300">
            {error}
          </div>
        ) : null}
      </div>
    </PopupFrame>
  );
}
