import { useRef, useState, type DragEvent } from "react";
import { PanelRight, Puzzle } from "lucide-react";
import type { BrowserSettings, ExtensionInfo, TabInfo } from "@type/browser";
import { openPopup, saveSettings } from "@/lib/browser-client";
import { toolbarDropEdge, toolbarIndexAt } from "@/lib/toolbar-pin";
import { IconButton } from "@/components/IconButton";
import { ToolbarEntry } from "../Extensions/ExtensionsToolbar";

type AddressBarExtensionsProps = {
  addressExtensions: string[];
  toolbarExtensions: ExtensionInfo[];
  activeTab: TabInfo | undefined;
  settings: BrowserSettings | null;
  onMove: (from: string, dropIndex: number) => void;
};

export function AddressBarExtensions({
  addressExtensions,
  toolbarExtensions,
  activeTab,
  settings,
  onMove,
}: AddressBarExtensionsProps) {
  const draggingRef = useRef<string | null>(null);
  const [dragging, setDragging] = useState<string | null>(null);
  const [dropIndex, setDropIndex] = useState<number | null>(null);
  const fromIndex = dragging ? addressExtensions.indexOf(dragging) : -1;
  const toolsOpen = settings?.showToolsToolbar !== false;

  const toggleToolsToolbar = () => {
    if (!settings) {
      return;
    }
    void saveSettings({ ...settings, showToolsToolbar: !toolsOpen });
  };

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
      id="address-tools"
      className="flex items-center"
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
      {addressExtensions.map((id, index) => (
        <ToolbarEntry
          key={id}
          id={id}
          idPrefix="address"
          extension={toolbarExtensions.find((item) => item.id === id)}
          activeTab={activeTab}
          settings={settings}
          labeled={false}
          toolbarIndex={index}
          dragging={dragging === id}
          drop={toolbarDropEdge(index, addressExtensions.length, fromIndex, dropIndex)}
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
      <IconButton
        id="address-secondary-toolbar"
        label={
          toolsOpen
            ? "Hide secondary extensions toolbar (Ctrl+Shift+T)"
            : "Show secondary extensions toolbar (Ctrl+Shift+T)"
        }
        active={toolsOpen}
        disabled={!settings}
        onClick={toggleToolsToolbar}
      >
        <PanelRight id="address-secondary-toolbar-icon" aria-hidden />
      </IconButton>
      <IconButton
        id="address-extensions-page"
        label="Extensions"
        aria-haspopup="dialog"
        onClick={(event) => void openPopup("extensions-menu", event.currentTarget)}
      >
        <Puzzle id="address-extensions-page-icon" aria-hidden />
      </IconButton>
    </div>
  );
}
