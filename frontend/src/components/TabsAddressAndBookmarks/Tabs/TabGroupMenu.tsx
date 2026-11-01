import { useEffect, useState } from "react";
import { Check, FolderMinus, Plus, X } from "lucide-react";
import { closeGroup, closePopup, createContainer, listContainers, newTabInGroup, onContainersChanged, setGroupContainer, ungroup, updateGroup } from "../../../lib/browser-client";
import { cn } from "../../../lib/cn";
import { GROUP_COLORS, groupColorClasses } from "../../../lib/tab-rows";
import { useTabs } from "../../../lib/use-browser";
import type { Container } from "@type/browser";
import { PopupFrame, PopupMenu, PopupMenuItem, PopupMenuSeparator } from "../../PopupFrame";

type TabGroupMenuProps = {
  groupId: number;
};

export function TabGroupMenu({ groupId }: TabGroupMenuProps) {
  const snapshot = useTabs();
  const group = snapshot.groups.find((item) => item.id === groupId);
  const count = snapshot.tabs.filter((tab) => tab.groupId === groupId).length;
  const [name, setName] = useState("");
  const [containers, setContainers] = useState<Container[]>([]);
  const [containerName, setContainerName] = useState("");

  useEffect(() => {
    if (group) {
      setName(group.name);
    }
  }, [group?.id]);

  useEffect(() => {
    void listContainers().then(setContainers).catch(() => undefined);
    const unlisten = onContainersChanged(setContainers);
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);

  const run = (action: () => Promise<unknown>) => () => {
    void action().finally(() => void closePopup());
  };

  if (!group) {
    return (
      <PopupFrame id="tab-group-menu" ready={snapshot.loaded}>
        <div id="tab-group-menu-missing" className="px-3 py-2 text-sm text-zinc-500">
          That group is gone.
        </div>
      </PopupFrame>
    );
  }

  const saveName = () => {
    if (name.trim() !== group.name) {
      void updateGroup(group.id, { name });
    }
  };

  return (
    <PopupFrame id="tab-group-menu" ready={snapshot.loaded}>
      <div id="tab-group-menu-edit" className="flex flex-col gap-2 px-3 pb-1 pt-3">
        <input
          id="tab-group-menu-name"
          autoFocus
          value={name}
          maxLength={60}
          placeholder="Name this group"
          className="h-8 w-full rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500"
          onChange={(event) => setName(event.target.value)}
          onBlur={saveName}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              saveName();
              void closePopup();
            }
          }}
        />
        <div id="tab-group-menu-colors" role="radiogroup" aria-label="Group color" className="flex flex-wrap gap-1.5">
          {GROUP_COLORS.map((color) => (
            <button
              key={color}
              id={`tab-group-menu-color-${color}`}
              type="button"
              role="radio"
              aria-checked={group.color === color}
              title={color}
              className={cn(
                "inline-flex size-6 items-center justify-center rounded-full ring-offset-2 ring-offset-zinc-900",
                groupColorClasses(color).swatch,
                group.color === color && "ring-2 ring-zinc-200",
              )}
              onClick={() => void updateGroup(group.id, { color })}
            >
              {group.color === color ? <Check id={`tab-group-menu-color-${color}-check`} className="size-3.5 text-zinc-950" aria-hidden /> : null}
            </button>
          ))}
        </div>
      </div>
      <div id="tab-group-menu-container" className="flex flex-col gap-1.5 px-3 pb-1">
        <div id="tab-group-menu-container-label" className="text-xs text-zinc-500">
          Container
        </div>
        <div id="tab-group-menu-container-list" className="flex flex-col">
          <button
            id="tab-group-menu-container-default"
            type="button"
            className={cn(
              "h-8 rounded-md px-2 text-left text-sm hover:bg-zinc-800",
              !group.container && "bg-zinc-800",
            )}
            onClick={() => void setGroupContainer(group.id, "")}
          >
            Default profile
          </button>
          {containers.map((container) => (
            <button
              id={`tab-group-menu-container-${container.id}`}
              key={container.id}
              type="button"
              className={cn(
                "flex h-8 items-center gap-2 rounded-md px-2 text-left text-sm hover:bg-zinc-800",
                group.container === container.id && "bg-zinc-800",
              )}
              onClick={() => void setGroupContainer(group.id, container.id)}
            >
              <span
                id={`tab-group-menu-container-${container.id}-dot`}
                className={cn("size-2 rounded-full", groupColorClasses(container.color).swatch)}
              />
              <span id={`tab-group-menu-container-${container.id}-name`} className="truncate">
                {container.name}
              </span>
            </button>
          ))}
        </div>
        <div id="tab-group-menu-container-new" className="flex gap-1.5">
          <input
            id="tab-group-menu-container-name"
            value={containerName}
            maxLength={40}
            placeholder="New container"
            className="h-8 min-w-0 flex-1 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm outline-none focus:border-sky-500"
            onChange={(event) => setContainerName(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                void createContainer(containerName, "green").then((created) => {
                  setContainerName("");
                  return setGroupContainer(group.id, created.id);
                });
              }
            }}
          />
        </div>
        <p id="tab-group-menu-container-hint" className="text-[11px] text-zinc-500">
          A container keeps its own cookies and logins. Extensions stay on the default profile.
        </p>
      </div>
      <PopupMenu id="tab-group-menu-items" label="Tab group">
        <PopupMenuItem id="tab-group-menu-new-tab" icon={<Plus />} label="New tab in group" onClick={run(() => newTabInGroup(group.id))} />
        <PopupMenuItem id="tab-group-menu-ungroup" icon={<FolderMinus />} label="Ungroup" onClick={run(() => ungroup(group.id))} />
        <PopupMenuSeparator id="tab-group-menu-separator" />
        <PopupMenuItem
          id="tab-group-menu-close"
          icon={<X />}
          label={count === 1 ? "Close group" : `Close group (${count} tabs)`}
          danger
          onClick={run(() => closeGroup(group.id))}
        />
      </PopupMenu>
    </PopupFrame>
  );
}
