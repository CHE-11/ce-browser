import { useRef, useState, type DragEvent } from "react";
import type { BrowserSettings, ExtensionInfo, TabInfo } from "@type/browser";
import { toolbarDropEdge, toolbarIndexAt } from "@/lib/toolbar-pin";
import { ToolbarEntry } from "../Extensions/ExtensionsToolbar";

type AddressBarFieldExtensionsProps = {
  fieldExtensions: string[];
  toolbarExtensions: ExtensionInfo[];
  activeTab: TabInfo | undefined;
  settings: BrowserSettings | null;
  onMove: (from: string, dropIndex: number) => void;
};

/** Icons pinned inside the address field, to the left of the bookmark star. */
export function AddressBarFieldExtensions({
  fieldExtensions,
  toolbarExtensions,
  activeTab,
  settings,
  onMove,
}: AddressBarFieldExtensionsProps) {
  const draggingRef = useRef<string | null>(null);
  const [dragging, setDragging] = useState<string | null>(null);
  const [dropIndex, setDropIndex] = useState<number | null>(null);
  const fromIndex = dragging ? fieldExtensions.indexOf(dragging) : -1;

  if (fieldExtensions.length === 0) {
    return null;
  }

  const rememberDrop = (index: number) => {
    setDropIndex((current) => (current === index ? current : index));
  };

  const trackDrag = (event: DragEvent<HTMLElement>) => {
    if (!draggingRef.current) {
      return;
    }
    event.preventDefault();
    event.dataTransfer.dropEffect = "move";
    rememberDrop(toolbarIndexAt(event.currentTarget, event.clientX, "x"));
  };

  const clearDrag = () => {
    draggingRef.current = null;
    setDragging(null);
    setDropIndex(null);
  };

  const finishDrop = (event: DragEvent<HTMLElement>) => {
    const from = draggingRef.current;
    if (!from) {
      return;
    }
    event.preventDefault();
    onMove(from, toolbarIndexAt(event.currentTarget, event.clientX, "x"));
    clearDrag();
  };

  return (
    <div
      id="address-field-extensions"
      className="flex shrink-0 items-center"
      onDragEnter={trackDrag}
      onDragOver={trackDrag}
      onDrop={finishDrop}
      onDragLeave={(event) => {
        const next = event.relatedTarget;
        if (!(next instanceof Node) || event.currentTarget.contains(next)) {
          return;
        }
        setDropIndex(null);
      }}
    >
      {fieldExtensions.map((id, index) => (
        <ToolbarEntry
          key={id}
          id={id}
          idPrefix="address-field"
          extension={toolbarExtensions.find((item) => item.id === id)}
          activeTab={activeTab}
          settings={settings}
          labeled={false}
          toolbarIndex={index}
          dragging={dragging === id}
          drop={toolbarDropEdge(index, fieldExtensions.length, fromIndex, dropIndex)}
          onDragStart={(event) => {
            event.dataTransfer.effectAllowed = "move";
            event.dataTransfer.setData("text/plain", id);
            draggingRef.current = id;
            setDragging(id);
            setDropIndex(null);
          }}
          onDragEnd={clearDrag}
        />
      ))}
    </div>
  );
}
