import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { LayoutGrid, SearchX } from "lucide-react";
import { Button } from "@/components/Button";
import { Dialog } from "@/components/Dialog";
import { EmptyState } from "@/components/EmptyState";
import { Skeleton } from "@/components/Skeleton";
import { copyPinImage, createTab, deleteBoard, deleteBoardFolder, deletePins, movePins, setBoardCover, showNotice } from "@/lib/browser-client";
import { useAppearanceFromSettings } from "@/lib/use-browser";
import type { Pin } from "@type/browser";
import { errorText } from "@features/BuiltInExtensions/use-page-tool";
import { BoardCards } from "./BoardCards";
import { BoardHeader } from "./BoardHeader";
import { BoardsSidebar } from "./BoardsSidebar";
import { MoveMenu } from "./MoveMenu";
import { PinDialog } from "./PinDialog";
import { PinGrid, type PinActions, type SelectMode } from "./PinGrid";
import { folderContents, pinMatches, pinsInFolder, pinsOnBoard, type BoardsScope } from "./board-tree";
import { useBoards } from "./use-boards";

type Confirm = { kind: "pins"; ids: string[] } | { kind: "board"; id: string } | { kind: "folder"; id: string };

function isTyping(target: EventTarget | null) {
  return target instanceof HTMLElement && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.tagName === "SELECT" || target.isContentEditable);
}

function sourceUrl(pin: Pin) {
  return pin.kind === "link" && pin.linkUrl ? pin.linkUrl : pin.pageUrl || pin.linkUrl;
}

export function BoardsPage({ params }: { params: URLSearchParams }) {
  useAppearanceFromSettings();
  const { boards, loaded } = useBoards();
  const requestedBoard = params.get("board");
  const [scope, setScope] = useState<BoardsScope>(() => (requestedBoard ? { kind: "board", id: requestedBoard } : { kind: "all" }));
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [openPinId, setOpenPinId] = useState<string | null>(null);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [moveMenu, setMoveMenu] = useState<{ ids: string[]; x: number; y: number } | null>(null);
  const [confirm, setConfirm] = useState<Confirm | null>(null);
  const [error, setError] = useState("");
  const lastPicked = useRef<string | null>(null);

  useEffect(() => {
    document.title = "Boards";
  }, []);

  useEffect(() => {
    if (requestedBoard) {
      setScope({ kind: "board", id: requestedBoard });
    }
  }, [requestedBoard]);

  useEffect(() => {
    if (!loaded) {
      return;
    }
    const exists =
      scope.kind === "all" ||
      (scope.kind === "board" ? boards.boards.some((board) => board.id === scope.id) : boards.folders.some((folder) => folder.id === scope.id));
    if (!exists) {
      setScope({ kind: "all" });
    }
  }, [loaded, boards, scope]);

  useEffect(() => {
    setSelected(new Set());
    lastPicked.current = null;
  }, [scope]);

  useEffect(() => {
    const ids = new Set(boards.pins.map((pin) => pin.id));
    setSelected((current) => {
      const kept = [...current].filter((id) => ids.has(id));
      return kept.length === current.size ? current : new Set(kept);
    });
    if (openPinId && !ids.has(openPinId)) {
      setOpenPinId(null);
    }
  }, [boards, openPinId]);

  const scopePins = useMemo(() => {
    const pins = scope.kind === "board" ? pinsOnBoard(boards, scope.id) : scope.kind === "folder" ? pinsInFolder(boards, scope.id) : boards.pins;
    return pins.filter((pin) => pinMatches(pin, query));
  }, [boards, scope, query]);

  const fail = useCallback((reason: unknown) => setError(errorText(reason)), []);

  const select = (pin: Pin, mode: SelectMode) => {
    setSelected((current) => {
      const next = new Set(current);
      const anchor = lastPicked.current ? scopePins.findIndex((item) => item.id === lastPicked.current) : -1;
      const index = scopePins.findIndex((item) => item.id === pin.id);
      if (mode === "range" && anchor >= 0 && index >= 0) {
        const [from, to] = anchor < index ? [anchor, index] : [index, anchor];
        for (const item of scopePins.slice(from, to + 1)) {
          next.add(item.id);
        }
      } else if (next.has(pin.id)) {
        next.delete(pin.id);
      } else {
        next.add(pin.id);
      }
      return next;
    });
    lastPicked.current = pin.id;
  };

  const actions: PinActions = {
    open: (pin) => setOpenPinId(pin.id),
    openSource: (pin) => {
      const url = sourceUrl(pin);
      if (url) {
        void createTab(url).catch(fail);
      }
    },
    copyImage: (pin) =>
      void copyPinImage(pin.id)
        .then(() => showNotice("Image copied"))
        .catch(fail),
    move: (ids, x, y) => setMoveMenu({ ids, x, y }),
    setCover: (pin) => void setBoardCover(pin.boardId, pin.id).catch(fail),
    remove: (ids) => setConfirm({ kind: "pins", ids }),
  };

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (openPinId || confirm || moveMenu || isTyping(event.target)) {
        return;
      }
      if (event.key === "Escape" && selected.size > 0) {
        setSelected(new Set());
      } else if (event.key === "Delete" && selected.size > 0) {
        event.preventDefault();
        setConfirm({ kind: "pins", ids: [...selected] });
      } else if (event.key.toLowerCase() === "a" && (event.ctrlKey || event.metaKey) && scopePins.length > 0) {
        event.preventDefault();
        setSelected(new Set(scopePins.map((pin) => pin.id)));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [openPinId, confirm, moveMenu, selected, scopePins]);

  const runConfirm = () => {
    if (!confirm) {
      return;
    }
    const work =
      confirm.kind === "pins" ? deletePins(confirm.ids) : confirm.kind === "board" ? deleteBoard(confirm.id) : deleteBoardFolder(confirm.id);
    if (confirm.kind === "pins") {
      setSelected((current) => new Set([...current].filter((id) => !confirm.ids.includes(id))));
      if (openPinId && confirm.ids.includes(openPinId)) {
        setOpenPinId(null);
      }
    }
    setConfirm(null);
    void work.catch(fail);
  };

  const openIndex = openPinId ? scopePins.findIndex((pin) => pin.id === openPinId) : -1;
  const openPin = openPinId ? (boards.pins.find((pin) => pin.id === openPinId) ?? null) : null;
  const moveCurrent = moveMenu && moveMenu.ids.length > 0 ? boards.pins.find((pin) => pin.id === moveMenu.ids[0])?.boardId : undefined;
  const sameBoard = moveMenu ? moveMenu.ids.every((id) => boards.pins.find((pin) => pin.id === id)?.boardId === moveCurrent) : false;

  return (
    <div id="boards-page" className="flex h-full bg-zinc-950 text-zinc-100">
      <BoardsSidebar
        boards={boards}
        scope={scope}
        onScope={setScope}
        query={query}
        onQuery={setQuery}
        renaming={renaming}
        onRenaming={setRenaming}
        onDeleteFolder={(id) => setConfirm({ kind: "folder", id })}
        onDeleteBoard={(id) => setConfirm({ kind: "board", id })}
        onError={fail}
      />
      <main id="boards-main" className="flex min-w-0 flex-1 flex-col">
        <BoardHeader
          boards={boards}
          scope={scope}
          pinCount={scopePins.length}
          selectedCount={selected.size}
          onScope={setScope}
          onMoveSelected={(x, y) => setMoveMenu({ ids: [...selected], x, y })}
          onDeleteSelected={() => setConfirm({ kind: "pins", ids: [...selected] })}
          onClearSelection={() => setSelected(new Set())}
          onError={fail}
        />
        {error ? (
          <div id="boards-error" role="alert" className="flex items-center gap-3 border-b border-amber-900/50 bg-amber-950/40 px-8 py-2 text-sm text-amber-200">
            <span id="boards-error-text" className="min-w-0 flex-1">
              {error}
            </span>
            <Button id="boards-error-dismiss" variant="ghost" size="sm" onClick={() => setError("")}>
              Dismiss
            </Button>
          </div>
        ) : null}
        <div id="boards-content" className="min-h-0 flex-1 overflow-y-auto px-8 py-6">
          {!loaded ? (
            <div id="boards-loading" className="columns-[15rem] gap-4">
              {[180, 260, 220, 300, 200, 240].map((height, index) => (
                <Skeleton key={index} id={`boards-loading-${index}`} className="mb-4 w-full rounded-xl" style={{ height }} />
              ))}
            </div>
          ) : (
            <>
              {scope.kind === "folder" && !query ? (
                <section id="boards-folder-overview" className="mb-8">
                  <BoardCards boards={boards} folderId={scope.id} onScope={setScope} />
                </section>
              ) : null}
              {scopePins.length > 0 ? (
                <>
                  {scope.kind === "folder" && !query ? (
                    <h3 id="boards-folder-pins-title" className="mb-3 text-sm font-medium text-zinc-400">
                      All pins in this folder
                    </h3>
                  ) : null}
                  <PinGrid boards={boards} pins={scopePins} selected={selected} showBoard={scope.kind !== "board"} onSelect={select} actions={actions} />
                </>
              ) : query ? (
                <EmptyState id="boards-no-match" icon={SearchX} title="No pins match" hint="Search looks at titles, notes, page text and addresses." />
              ) : scope.kind === "board" ? (
                <EmptyState
                  id="boards-board-empty"
                  icon={LayoutGrid}
                  title="This board is empty"
                  hint="Save to it with the Boards button in the address bar, or drag pins onto it in the sidebar."
                />
              ) : scope.kind === "all" ? (
                <EmptyState
                  id="boards-empty"
                  icon={LayoutGrid}
                  title="Nothing saved yet"
                  hint="Use the Boards button in the address bar to save a page, a region, a page section, a link or images. Right-click an image on any page to save it too."
                />
              ) : folderContents(boards, scope.id).boards === 0 ? (
                <EmptyState id="boards-folder-empty" icon={LayoutGrid} title="This folder is empty" hint="Add a board here, or drag boards onto the folder in the sidebar." />
              ) : null}
            </>
          )}
        </div>
      </main>

      <PinDialog
        boards={boards}
        pin={openPin}
        onClose={() => setOpenPinId(null)}
        onPrevious={openIndex > 0 ? () => setOpenPinId(scopePins[openIndex - 1].id) : undefined}
        onNext={openIndex >= 0 && openIndex < scopePins.length - 1 ? () => setOpenPinId(scopePins[openIndex + 1].id) : undefined}
        onOpenSource={actions.openSource}
        onDelete={actions.remove}
        onError={fail}
      />

      {moveMenu ? (
        <MoveMenu
          boards={boards}
          x={moveMenu.x}
          y={moveMenu.y}
          currentBoardId={sameBoard ? moveCurrent : undefined}
          onMove={(boardId) => {
            void movePins(moveMenu.ids, boardId).catch(fail);
            setSelected(new Set());
          }}
          onClose={() => setMoveMenu(null)}
        />
      ) : null}

      <ConfirmDialog boards={boards} confirm={confirm} onCancel={() => setConfirm(null)} onConfirm={runConfirm} />
    </div>
  );
}

function ConfirmDialog({
  boards,
  confirm,
  onCancel,
  onConfirm,
}: {
  boards: ReturnType<typeof useBoards>["boards"];
  confirm: Confirm | null;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  let title = "";
  let detail = "";
  if (confirm?.kind === "pins") {
    title = confirm.ids.length === 1 ? "Delete this pin?" : `Delete ${confirm.ids.length} pins?`;
    detail = "Saved images are removed too. This can't be undone.";
  } else if (confirm?.kind === "board") {
    const board = boards.boards.find((item) => item.id === confirm.id);
    const count = pinsOnBoard(boards, confirm.id).length;
    title = `Delete "${board?.name ?? "board"}"?`;
    detail = count === 0 ? "The board is empty." : `Its ${count === 1 ? "1 pin" : `${count} pins`} will be deleted too. This can't be undone.`;
  } else if (confirm?.kind === "folder") {
    const folder = boards.folders.find((item) => item.id === confirm.id);
    const contents = folderContents(boards, confirm.id);
    title = `Delete "${folder?.name ?? "folder"}"?`;
    const parts = [
      contents.folders ? `${contents.folders} ${contents.folders === 1 ? "folder" : "folders"}` : "",
      contents.boards ? `${contents.boards} ${contents.boards === 1 ? "board" : "boards"}` : "",
      contents.pins ? `${contents.pins} ${contents.pins === 1 ? "pin" : "pins"}` : "",
    ].filter(Boolean);
    detail = parts.length === 0 ? "The folder is empty." : `Everything inside it will be deleted: ${parts.join(", ")}. This can't be undone.`;
  }

  return (
    <Dialog id="boards-confirm" title={title} open={confirm !== null} onClose={onCancel} onEnter={onConfirm} panelClassName="max-w-md">
      <p id="boards-confirm-detail" className="text-sm text-zinc-300">
        {detail}
      </p>
      <div id="boards-confirm-actions" className="mt-4 flex justify-end gap-2">
        <Button id="boards-confirm-cancel" onClick={onCancel}>
          Cancel
        </Button>
        <Button id="boards-confirm-delete" variant="destructive" onClick={onConfirm}>
          Delete
        </Button>
      </div>
    </Dialog>
  );
}
