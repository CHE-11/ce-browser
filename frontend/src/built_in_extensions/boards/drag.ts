import type { DragEvent } from "react";
import type { BoardsSnapshot } from "@type/browser";
import { DRAG_FOLDER, DRAG_PINS, folderPath } from "./folder-tree";

export function dragHas(event: DragEvent, type: string) {
  return event.dataTransfer.types.includes(type);
}

/** Pins or a folder dragged inside Boards. */
export function isBoardsDrag(event: DragEvent) {
  return dragHas(event, DRAG_PINS) || dragHas(event, DRAG_FOLDER);
}

/** Ctrl adds pins to the target and leaves them where they are. */
export function pinDropEffect(event: DragEvent): "copy" | "move" {
  return event.ctrlKey ? "copy" : "move";
}

/** Ids of the dragged pins. Only readable on drop. */
export function draggedPins(event: DragEvent): string[] {
  try {
    const ids: unknown = JSON.parse(event.dataTransfer.getData(DRAG_PINS) || "[]");
    return Array.isArray(ids) ? ids.filter((id): id is string => typeof id === "string") : [];
  } catch {
    return [];
  }
}

/** Id of the dragged folder. Only readable on drop. */
export function draggedFolder(event: DragEvent) {
  return event.dataTransfer.getData(DRAG_FOLDER);
}

/** A folder can go into `target` (null is the top level) unless that is itself, inside it, or where it already is. */
export function canMoveFolder(boards: BoardsSnapshot, id: string, target: string | null) {
  const folder = boards.folders.find((item) => item.id === id);
  if (!folder || folder.parentId === target) {
    return false;
  }
  return target === null || !folderPath(boards, target).some((item) => item.id === id);
}
