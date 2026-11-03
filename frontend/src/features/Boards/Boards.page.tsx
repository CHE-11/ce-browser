import { useCallback, useEffect, useMemo, useRef, useState, type DragEvent } from "react";
import { LayoutGrid, SearchX } from "lucide-react";
import { Button } from "@/components/Button";
import { Dialog } from "@/components/Dialog";
import { EmptyState } from "@/components/EmptyState";
import { Skeleton } from "@/components/Skeleton";
import {
  copyPinImage,
  copyText,
  createTab,
  deleteBoard,
  deleteBoardFolder,
  deletePins,
  importBoardItems,
  movePins,
  openPinFile,
  pickBoardFiles,
  setBoardCover,
  showNotice,
} from "@/lib/browser-client";
import { useAppearanceFromSettings } from "@/lib/use-browser";
import type { BoardImportItem, Pin, PinKind } from "@type/browser";
import { errorText } from "@features/BuiltInExtensions/use-page-tool";
import { BoardCards } from "./BoardCards";
import { BoardHeader } from "./BoardHeader";
import { BoardsSidebar } from "./BoardsSidebar";
import { MoveMenu } from "./MoveMenu";
import { PinDialog } from "./PinDialog";
import { PinGrid, type PinActions, type SelectMode } from "./PinGrid";
import { PinTable } from "./PinTable";
import {
  DRAG_PINS,
  folderContents,
  pinAddress,
  pinMatches,
  pinsInFolder,
  pinsOnBoard,
  readPinSort,
  readPinView,
  sortPins,
  writePinSort,
  writePinView,
  type BoardsScope,
  type PinSort,
  type PinView,
} from "./board-tree";
import { savePinFileAs } from "./file-pin";
import { MAX_IMPORT, filesToImports, linksInText, linksToImports, pathsToImports } from "./import-items";
import { useBoards } from "./use-boards";

type Confirm = { kind: "pins"; ids: string[] } | { kind: "board"; id: string } | { kind: "folder"; id: string };

function isTyping(target: EventTarget | null) {
  return target instanceof HTMLElement && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.tagName === "SELECT" || target.isContentEditable);
}

const OPEN_LIMIT = 20;

function isFileDrag(event: DragEvent) {
  const types = [...event.dataTransfer.types];
  return types.includes("Files") && !types.includes(DRAG_PINS);
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
  const [view, setView] = useState<PinView>(readPinView);
  const [sort, setSort] = useState<PinSort>(readPinSort);
  const [kind, setKind] = useState<PinKind | null>(null);
  const [linkOpen, setLinkOpen] = useState(false);
  const [linkValue, setLinkValue] = useState("");
  const [dragOver, setDragOver] = useState(false);
  const lastPicked = useRef<string | null>(null);
  const importing = useRef(false);
  const dragDepth = useRef(0);
  const fileDrag = useRef(false);
  const queryRef = useRef(query);

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

  const inScope = useMemo(() => {
    return scope.kind === "board" ? pinsOnBoard(boards, scope.id) : scope.kind === "folder" ? pinsInFolder(boards, scope.id) : boards.pins;
  }, [boards, scope]);

  const scopePins = useMemo(() => {
    const matched = inScope.filter((pin) => pinMatches(pin, query) && (kind === null || pin.kind === kind));
    return sortPins(matched, sort);
  }, [inScope, query, kind, sort]);

  useEffect(() => {
    if (queryRef.current && !query) {
      setKind(null);
    }
    queryRef.current = query;
  }, [query]);

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
      const url = pinAddress(pin);
      if (url) {
        void createTab(url).catch(fail);
      }
    },
    copyImage: (pin) =>
      void copyPinImage(pin.id)
        .then(() => showNotice("Image copied"))
        .catch(fail),
    openFile: (pin) =>
      void openPinFile(pin.id)
        .then(() => showNotice("Opened file"))
        .catch(fail),
    saveFile: (pin) =>
      void savePinFileAs(pin)
        .then((saved) => {
          if (saved) {
            return showNotice("File saved");
          }
        })
        .catch(fail),
    move: (ids, x, y) => setMoveMenu({ ids, x, y }),
    setCover: (pin) => void setBoardCover(pin.boardId, pin.id).catch(fail),
    remove: (ids) => setConfirm({ kind: "pins", ids }),
  };

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (openPinId || confirm || moveMenu || linkOpen || isTyping(event.target)) {
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
  }, [openPinId, confirm, moveMenu, linkOpen, selected, scopePins]);

  const selectedPins = boards.pins.filter((pin) => selected.has(pin.id));
  const targetBoardId = scope.kind === "board" ? scope.id : (boards.lastBoardId ?? "");
  const targetBoardName = boards.boards.find((board) => board.id === targetBoardId)?.name ?? "Unsorted";

  const runImport = useCallback(
    async (items: BoardImportItem[], skipped = 0) => {
      if (importing.current || items.length === 0) {
        return;
      }
      importing.current = true;
      try {
        const pins = await importBoardItems(targetBoardId, items);
        const saved = boards.boards.find((board) => board.id === pins[0]?.boardId);
        const count = pins.length === 1 ? "1 item" : `${pins.length} items`;
        const extra = skipped > 0 ? ` ${skipped === 1 ? "1 was" : `${skipped} were`} left out.` : "";
        await showNotice(`Saved ${count} to ${saved?.name ?? "Unsorted"}.${extra}`).catch(() => undefined);
      } catch (reason) {
        fail(reason);
      } finally {
        importing.current = false;
      }
    },
    [boards.boards, fail, targetBoardId],
  );

  const addFiles = useCallback(
    (files: File[]) => {
      void filesToImports(files)
        .then(({ items, skipped }) => runImport(items, skipped))
        .catch(fail);
    },
    [fail, runImport],
  );

  useEffect(() => {
    const onPaste = (event: ClipboardEvent) => {
      if (openPinId || confirm || moveMenu || linkOpen || isTyping(event.target)) {
        return;
      }
      const data = event.clipboardData;
      if (!data) {
        return;
      }
      const files = [...data.files];
      if (files.length === 0) {
        for (const item of data.items) {
          if (item.kind !== "file") {
            continue;
          }
          const file = item.getAsFile();
          if (file) {
            files.push(file);
          }
        }
      }
      if (files.length > 0) {
        event.preventDefault();
        addFiles(files);
        return;
      }
      const found = linksInText(data.getData("text/plain"));
      if (found.length > 0) {
        event.preventDefault();
        void runImport(linksToImports(found), Math.max(0, found.length - MAX_IMPORT));
      }
    };
    window.addEventListener("paste", onPaste);
    return () => window.removeEventListener("paste", onPaste);
  }, [openPinId, confirm, moveMenu, linkOpen, addFiles, runImport]);

  const copySelected = () => {
    const urls = selectedPins.map(pinAddress).filter(Boolean);
    if (urls.length === 0) {
      void showNotice("Those pins have no address.");
      return;
    }
    void copyText(urls.join("\n"))
      .then(() => showNotice(urls.length === 1 ? "Copied 1 address" : `Copied ${urls.length} addresses`))
      .catch(fail);
  };

  const openSelected = () => {
    const urls = selectedPins.map(pinAddress).filter(Boolean);
    if (urls.length === 0) {
      void showNotice("Those pins have no address.");
      return;
    }
    for (const url of urls.slice(0, OPEN_LIMIT)) {
      void createTab(url, true).catch(fail);
    }
    if (urls.length > OPEN_LIMIT) {
      void showNotice(`Opened ${OPEN_LIMIT} of ${urls.length}`);
    }
  };

  const pickFiles = () => {
    void pickBoardFiles()
      .then((paths) => {
        if (!paths || paths.length === 0) {
          return;
        }
        const skipped = Math.max(0, paths.length - MAX_IMPORT);
        void runImport(pathsToImports(paths), skipped);
      })
      .catch(fail);
  };

  const submitLink = () => {
    const urls = linksToImports(linksInText(linkValue));
    if (urls.length === 0) {
      setError("That link needs to start with http:// or https://.");
      return;
    }
    setLinkOpen(false);
    setLinkValue("");
    void runImport(urls);
  };

  const onDragEnter = (event: DragEvent<HTMLElement>) => {
    if (!isFileDrag(event)) {
      return;
    }
    event.preventDefault();
    fileDrag.current = true;
    dragDepth.current += 1;
    setDragOver(true);
  };

  const onDragLeave = () => {
    if (!fileDrag.current) {
      return;
    }
    dragDepth.current = Math.max(0, dragDepth.current - 1);
    if (dragDepth.current === 0) {
      fileDrag.current = false;
      setDragOver(false);
    }
  };

  const onDrop = (event: DragEvent<HTMLElement>) => {
    if (!fileDrag.current && !isFileDrag(event)) {
      return;
    }
    event.preventDefault();
    dragDepth.current = 0;
    fileDrag.current = false;
    setDragOver(false);
    addFiles([...event.dataTransfer.files]);
  };

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
      <main
        id="boards-main"
        className="relative flex min-w-0 flex-1 flex-col"
        onDragEnter={onDragEnter}
        onDragOver={(event) => {
          if (isFileDrag(event) || fileDrag.current) {
            event.preventDefault();
            event.dataTransfer.dropEffect = "copy";
          }
        }}
        onDragLeave={onDragLeave}
        onDrop={onDrop}
      >
        <BoardHeader
          boards={boards}
          scope={scope}
          pinCount={scopePins.length}
          selectedCount={selected.size}
          view={view}
          sort={sort}
          kind={kind}
          onScope={setScope}
          onView={(next) => {
            setView(next);
            writePinView(next);
          }}
          onSort={(next) => {
            setSort(next);
            writePinSort(next);
          }}
          onKind={setKind}
          onMoveSelected={(x, y) => setMoveMenu({ ids: [...selected], x, y })}
          onDeleteSelected={() => setConfirm({ kind: "pins", ids: [...selected] })}
          onClearSelection={() => setSelected(new Set())}
          onCopySelected={copySelected}
          onOpenSelected={openSelected}
          onPickFiles={pickFiles}
          onAddLink={() => {
            setLinkValue("");
            setLinkOpen(true);
          }}
          onError={fail}
        />
        {dragOver ? (
          <div id="boards-drop" className="pointer-events-none absolute inset-0 z-20 flex items-center justify-center bg-zinc-950/75">
            <p id="boards-drop-label" className="rounded-xl border border-sky-500 bg-sky-950/90 px-6 py-4 text-sm text-sky-100">
              Add to {targetBoardName}
            </p>
          </div>
        ) : null}
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
                  {view === "table" ? (
                    <PinTable
                      boards={boards}
                      pins={scopePins}
                      selected={selected}
                      showBoard={scope.kind !== "board"}
                      onSelect={select}
                      onSelectAll={() => setSelected(new Set(scopePins.map((pin) => pin.id)))}
                      onClear={() => setSelected(new Set())}
                      actions={actions}
                    />
                  ) : (
                    <PinGrid boards={boards} pins={scopePins} selected={selected} showBoard={scope.kind !== "board"} onSelect={select} actions={actions} />
                  )}
                </>
              ) : query ? (
                <EmptyState id="boards-no-match" icon={SearchX} title="No pins match" hint="Search looks at titles, notes, page text, file names and addresses." />
              ) : kind && inScope.length > 0 ? (
                <EmptyState id="boards-kind-empty" icon={SearchX} title="No pins of this kind" hint="Choose All to see every pin in this view." />
              ) : scope.kind === "board" ? (
                <EmptyState
                  id="boards-board-empty"
                  icon={LayoutGrid}
                  title="This board is empty"
                  hint="Drop or paste files, images and links here, use Add, or save from the Boards button in the address bar."
                />
              ) : scope.kind === "all" ? (
                <EmptyState
                  id="boards-empty"
                  icon={LayoutGrid}
                  title="Nothing saved yet"
                  hint="Drop or paste files, images and links, use Add, or save a page from the Boards button in the address bar."
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

      <Dialog
        id="boards-add-link-dialog"
        title="Add a link"
        open={linkOpen}
        onClose={() => setLinkOpen(false)}
        onEnter={submitLink}
        panelClassName="max-w-md"
      >
        <label id="boards-add-link-label" className="flex flex-col gap-1 text-xs text-zinc-400">
          Address
          <input
            id="boards-add-link-input"
            autoFocus
            value={linkValue}
            placeholder="https://"
            className="h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500"
            onChange={(event) => setLinkValue(event.target.value)}
          />
        </label>
        <div id="boards-add-link-actions" className="mt-4 flex justify-end gap-2">
          <Button id="boards-add-link-cancel" onClick={() => setLinkOpen(false)}>
            Cancel
          </Button>
          <Button id="boards-add-link-save" variant="primary" onClick={submitLink}>
            Add link
          </Button>
        </div>
      </Dialog>

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
