import { useCallback, useEffect, useMemo, useRef, useState, type DragEvent } from "react";
import { ClipboardPaste, Clock, FilePlus, FolderInput, FolderPlus, LayoutGrid, SearchX, SquareMousePointer, Trash2 } from "lucide-react";
import { Button } from "@/components/Button";
import { ContextMenu, ContextMenuItem, ContextMenuSeparator } from "@/components/ContextMenu";
import { Dialog } from "@/components/Dialog";
import { EmptyState } from "@/components/EmptyState";
import { Skeleton } from "@/components/Skeleton";
import {
  addPinsToFolder,
  cancelBoardImport,
  copyPinImage,
  copyText,
  createBoardFolder,
  createTab,
  deleteBoardFolder,
  deletePinsForever,
  duplicateBoardFolder,
  emptyBoardTrash,
  exportBoardFolder,
  exportPins,
  importBoardItems,
  importEagleLibrary,
  markPinsUsed,
  moveBoardFolder,
  movePins,
  onBoardImportProgress,
  openPinFile,
  pasteBoardClipboard,
  pickBoardFiles,
  pickBoardFolders,
  pickEagleLibrary,
  pickExportFolder,
  removePinsFromFolder,
  renameBoardFolder,
  restorePins,
  revealPinFile,
  showNotice,
  startBoardImport,
  trashPins,
  updateBoardFolder,
  updatePin,
} from "@/lib/browser-client";
import { useAppearanceFromSettings } from "@/lib/use-browser";
import type { BoardFolder, BoardImportProgress, BoardImportResult, Pin, PinSort } from "@type/browser";
import { errorText } from "@/built_in_extensions/use-page-tool";
import { FolderCards } from "../grid/FolderCards";
import { PinGrid, type PinListHandlers, type SelectMode } from "../grid/PinGrid";
import { PinTable } from "../grid/PinTable";
import { MoveMenu } from "../MoveMenu";
import { savePinFileAs } from "../file-pin";
import { EMPTY_FILTER, activeFilterCount, applyFilter, type PinFilter } from "../filters";
import { DRAG_PINS, UNCATEGORIZED, childFolders, folderContents, livePins, pinAddress, pinMatches, sortPins } from "../folder-tree";
import { importFiles, linksInText, linksToImports } from "../import-items";
import { dropEntries, walkEntries, type DroppedFile } from "../import/walk-drop";
import { sameScope, scopeExists, scopeFromParams, scopeHasOwnOrder, scopeKey, scopeLabel, scopePins, scopeSaveTarget, type BoardsScope } from "../scopes";
import { useBoards } from "../use-boards";
import { readViewPrefs, writeViewPrefs, type ViewPrefs } from "../view-prefs";
import { BoardsSidebar, type PinDropTarget } from "./BoardsSidebar";
import { BoardsToolbar } from "./BoardsToolbar";
import { FilterBar } from "./FilterBar";
import { FolderContextMenu } from "./FolderContextMenu";
import { ImportProgress } from "./ImportProgress";
import { Inspector } from "./Inspector";
import { PinContextMenu, type PinMenuActions } from "./PinContextMenu";
import { PinViewer } from "./PinViewer";
import { TagsView } from "./TagsView";

type Confirm = { kind: "forever"; ids: string[] } | { kind: "empty-trash" } | { kind: "folder"; id: string };
type MoveMenuState = { ids: string[]; x: number; y: number; mode: "add" | "move" };
type History = { stack: BoardsScope[]; index: number };

const OPEN_LIMIT = 20;
const HISTORY_LIMIT = 50;
const DONE_JOB_MS = 8000;
const USED_DELAY_MS = 800;

function isTyping(target: EventTarget | null) {
  return target instanceof HTMLElement && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.tagName === "SELECT" || target.isContentEditable);
}

/** Files or links dragged in from outside, not pins dragged within the page. */
function isOutsideDrag(event: DragEvent) {
  const types = [...event.dataTransfer.types];
  return !types.includes(DRAG_PINS) && (types.includes("Files") || types.includes("text/uri-list"));
}

function plural(count: number, one: string, many = `${one}s`) {
  return `${count} ${count === 1 ? one : many}`;
}

function savedMessage(result: Pick<BoardImportResult, "duplicates" | "failed"> & { saved: number }, where: string) {
  const parts = [result.saved > 0 ? `Saved ${plural(result.saved, "item")} to ${where}.` : "Nothing new was saved."];
  if (result.duplicates > 0) {
    parts.push(`${plural(result.duplicates, "item was", "items were")} already there.`);
  }
  if (result.failed > 0) {
    parts.push(`${plural(result.failed, "item")} couldn't be read.`);
  }
  return parts.join(" ");
}

export function BoardsPage({ params }: { params: URLSearchParams }) {
  useAppearanceFromSettings();
  const { boards, loaded } = useBoards();
  const requestedFolder = params.get("folder") ?? params.get("board");
  const [history, setHistory] = useState<History>(() => ({ stack: [scopeFromParams(params)], index: 0 }));
  const scope = history.stack[history.index];
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<PinFilter>(EMPTY_FILTER);
  const [filterOpen, setFilterOpen] = useState(false);
  const [prefs, setPrefs] = useState<ViewPrefs>(readViewPrefs);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [anchorId, setAnchorId] = useState<string | null>(null);
  const [viewerId, setViewerId] = useState<string | null>(null);
  const [pinMenu, setPinMenu] = useState<{ ids: string[]; x: number; y: number } | null>(null);
  const [folderMenu, setFolderMenu] = useState<{ folder: BoardFolder; x: number; y: number } | null>(null);
  const [backgroundMenu, setBackgroundMenu] = useState<{ x: number; y: number } | null>(null);
  const [moveMenu, setMoveMenu] = useState<MoveMenuState | null>(null);
  const [confirm, setConfirm] = useState<Confirm | null>(null);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [renameFolder, setRenameFolder] = useState<{ id: string; name: string } | null>(null);
  const [linkOpen, setLinkOpen] = useState(false);
  const [linkValue, setLinkValue] = useState("");
  const [error, setError] = useState("");
  const [dragOver, setDragOver] = useState(false);
  const [jobs, setJobs] = useState<BoardImportProgress[]>([]);
  const dragDepth = useRef(0);
  const cancelledDrops = useRef(new Set<string>());
  const jobScopes = useRef(new Map<string, string>());
  const scopeRef = useRef(scope);
  scopeRef.current = scope;

  const fail = useCallback((reason: unknown) => setError(errorText(reason)), []);

  useEffect(() => {
    document.title = "Boards";
  }, []);

  const go = useCallback((next: BoardsScope) => {
    setHistory((current) => {
      const here = current.stack[current.index];
      if (next.kind === "random" && here.kind === "random") {
        const stack = [...current.stack];
        stack[current.index] = next;
        return { stack, index: current.index };
      }
      if (sameScope(here, next)) {
        return current;
      }
      const stack = [...current.stack.slice(0, current.index + 1), next].slice(-HISTORY_LIMIT);
      return { stack, index: stack.length - 1 };
    });
  }, []);

  useEffect(() => {
    if (requestedFolder) {
      go({ kind: "folder", id: requestedFolder });
    }
  }, [requestedFolder, go]);

  useEffect(() => {
    if (loaded && !scopeExists(boards, scope)) {
      setHistory((current) => {
        const stack = [...current.stack];
        stack[current.index] = { kind: "all" };
        return { stack, index: current.index };
      });
    }
  }, [loaded, boards, scope]);

  const currentKey = scopeKey(scope);
  useEffect(() => {
    setSelected(new Set());
    setAnchorId(null);
    setViewerId(null);
  }, [currentKey]);

  const updatePrefs = useCallback((patch: Partial<ViewPrefs>) => {
    setPrefs((current) => {
      const next = { ...current, ...patch };
      writeViewPrefs(next);
      return next;
    });
  }, []);

  const library = useMemo(() => livePins(boards), [boards]);
  const base = useMemo(() => scopePins(boards, scope), [boards, scope]);
  const folder = scope.kind === "folder" ? boards.folders.find((item) => item.id === scope.id) : undefined;
  const folderSort: PinSort | null = folder?.sort ? folder.sort : null;
  const sort: PinSort | null = scopeHasOwnOrder(scope) ? null : (folderSort ?? prefs.sort);
  const searched = useMemo(() => (query.trim() ? base.filter((pin) => pinMatches(pin, query)) : base), [base, query]);
  const shown = useMemo(() => {
    const filtered = applyFilter(searched, filter, library);
    return sort ? sortPins(filtered, sort) : filtered;
  }, [searched, filter, library, sort]);
  const subfolders = useMemo(() => (scope.kind === "folder" && !query.trim() ? childFolders(boards, scope.id) : []), [boards, scope, query]);
  const filterCount = activeFilterCount(filter);
  const inTrash = scope.kind === "trash";
  const saveTarget = scopeSaveTarget(scope);
  const saveTargetName = saveTarget === UNCATEGORIZED ? "Uncategorized" : scopeLabel(boards, scope);

  useEffect(() => {
    const ids = new Set(boards.pins.map((pin) => pin.id));
    setSelected((current) => {
      const kept = [...current].filter((id) => ids.has(id));
      return kept.length === current.size ? current : new Set(kept);
    });
  }, [boards]);

  const pinsById = useMemo(() => new Map(boards.pins.map((pin) => [pin.id, pin])), [boards]);
  const selectedPins = useMemo(() => shown.filter((pin) => selected.has(pin.id)), [shown, selected]);
  const anchor = anchorId ? shown.findIndex((pin) => pin.id === anchorId) : -1;
  const viewerIndex = viewerId ? shown.findIndex((pin) => pin.id === viewerId) : -1;

  useEffect(() => {
    if (viewerId && viewerIndex < 0) {
      setViewerId(null);
    }
  }, [viewerId, viewerIndex]);

  useEffect(() => {
    if (!viewerId) {
      return;
    }
    const timer = window.setTimeout(() => void markPinsUsed([viewerId]).catch(() => undefined), USED_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [viewerId]);

  const overlayOpen = Boolean(viewerId || pinMenu || folderMenu || backgroundMenu || moveMenu || confirm || linkOpen || renameFolder);

  // Import jobs

  const upsertJob = useCallback((progress: BoardImportProgress) => {
    setJobs((current) => {
      const index = current.findIndex((job) => job.jobId === progress.jobId);
      if (index < 0) {
        return [...current, progress];
      }
      const next = [...current];
      next[index] = progress;
      return next;
    });
    if (progress.finished && !progress.error && progress.failed === 0) {
      window.setTimeout(() => setJobs((current) => current.filter((job) => job.jobId !== progress.jobId)), DONE_JOB_MS);
    }
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void onBoardImportProgress((progress) => {
      upsertJob(progress);
      if (progress.finished && !progress.cancelled && progress.folderId) {
        const started = jobScopes.current.get(progress.jobId);
        jobScopes.current.delete(progress.jobId);
        if (started && started === scopeKey(scopeRef.current)) {
          go({ kind: "folder", id: progress.folderId });
        }
      }
    }).then((stop) => {
      if (disposed) {
        stop();
      } else {
        unlisten = stop;
      }
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [upsertJob, go]);

  const trackJob = useCallback(
    (jobId: string) => {
      jobScopes.current.set(jobId, scopeKey(scopeRef.current));
      setJobs((current) =>
        current.some((job) => job.jobId === jobId)
          ? current
          : [
              ...current,
              { jobId, total: 0, done: 0, imported: 0, duplicates: 0, failed: 0, current: "", finished: false, cancelled: false, error: "", folderId: null },
            ],
      );
    },
    [],
  );

  const importPaths = useCallback(
    (paths: string[]) => {
      if (paths.length === 0) {
        return;
      }
      void startBoardImport(paths, saveTarget).then(trackJob).catch(fail);
    },
    [saveTarget, trackJob, fail],
  );

  const importDropped = useCallback(
    (files: DroppedFile[], target: string = saveTarget) => {
      if (files.length === 0) {
        return;
      }
      const jobId = `drop-${Date.now()}`;
      void importFiles(files, target, jobId, upsertJob, () => cancelledDrops.current.has(jobId)).finally(() => cancelledDrops.current.delete(jobId));
    },
    [saveTarget, upsertJob],
  );

  const importEntries = useCallback(
    (entries: FileSystemEntry[], target: string) =>
      void walkEntries(entries)
        .then((files) => importDropped(files, target))
        .catch(fail),
    [importDropped, fail],
  );

  const importLinks = useCallback(
    (urls: string[]) => {
      if (urls.length === 0) {
        return;
      }
      void importBoardItems(saveTarget, linksToImports(urls))
        .then((result) => showNotice(savedMessage({ ...result, saved: result.pins.length }, saveTargetName)))
        .catch(fail);
    },
    [saveTarget, saveTargetName, fail],
  );

  const pasteNative = useCallback(() => {
    void pasteBoardClipboard(saveTarget)
      .then((result) => showNotice(savedMessage({ ...result, saved: result.pins.length }, saveTargetName)))
      .catch(fail);
  }, [saveTarget, saveTargetName, fail]);

  const cancelJob = (jobId: string) => {
    if (jobId.startsWith("drop-")) {
      cancelledDrops.current.add(jobId);
    } else {
      void cancelBoardImport(jobId).catch(fail);
    }
  };

  const pickFiles = () => void pickBoardFiles().then((paths) => importPaths(paths ?? [])).catch(fail);
  const pickFolders = () => void pickBoardFolders().then((paths) => importPaths(paths ?? [])).catch(fail);
  const pickEagle = () =>
    void pickEagleLibrary()
      .then((path) => (path ? importEagleLibrary(path, saveTarget === UNCATEGORIZED ? "" : saveTarget).then(trackJob) : undefined))
      .catch(fail);

  // Selection

  const select = useCallback(
    (pin: Pin, mode: SelectMode) => {
      const index = shown.findIndex((item) => item.id === pin.id);
      setSelected((current) => {
        if (mode === "range" && anchor >= 0 && index >= 0) {
          const [from, to] = anchor < index ? [anchor, index] : [index, anchor];
          return new Set(shown.slice(from, to + 1).map((item) => item.id));
        }
        if (mode === "toggle") {
          const next = new Set(current);
          if (next.has(pin.id)) {
            next.delete(pin.id);
          } else {
            next.add(pin.id);
          }
          return next;
        }
        return new Set([pin.id]);
      });
      if (mode !== "range" || anchor < 0) {
        setAnchorId(pin.id);
      }
    },
    [shown, anchor],
  );

  const handlers: PinListHandlers = {
    onSelect: select,
    onMarquee: (ids) => {
      setSelected(new Set(ids));
      if (ids.length === 0) {
        setAnchorId(null);
      }
    },
    onOpen: (pin) => setViewerId(pin.id),
    onMenu: (pin, x, y) => {
      const ids = selected.has(pin.id) ? [...selected] : [pin.id];
      setPinMenu({ ids, x, y });
    },
    onNavigate: (index, extend) => {
      const pin = shown[index];
      if (!pin) {
        return;
      }
      if (extend && anchor >= 0) {
        setSelected((current) => new Set([...current, pin.id]));
      } else {
        setSelected(new Set([pin.id]));
      }
      setAnchorId(pin.id);
    },
  };

  // Pin actions

  const dropFromSelection = (ids: string[]) => setSelected((current) => new Set([...current].filter((id) => !ids.includes(id))));

  const exportTo = (work: (dir: string) => Promise<string | number>, message: (result: string | number) => string) =>
    void pickExportFolder()
      .then(async (dir) => {
        if (dir) {
          await showNotice(message(await work(dir)));
        }
      })
      .catch(fail);

  const actions: PinMenuActions = {
    open: (pin) => setViewerId(pin.id),
    openSource: (pins) => {
      const urls = pins.map(pinAddress).filter(Boolean);
      if (urls.length === 0) {
        void showNotice(pins.length === 1 ? "This item has no address." : "These items have no address.");
        return;
      }
      urls.slice(0, OPEN_LIMIT).forEach((url, index) => void createTab(url, urls.length > 1 || index > 0).catch(fail));
      if (urls.length > OPEN_LIMIT) {
        void showNotice(`Opened ${OPEN_LIMIT} of ${urls.length}`);
      }
    },
    openFile: (pin) => void openPinFile(pin.id).catch(fail),
    reveal: (pin) => void revealPinFile(pin.id).catch(fail),
    copyImage: (pin) =>
      void copyPinImage(pin.id)
        .then(() => {
          void markPinsUsed([pin.id]).catch(() => undefined);
          return showNotice("Image copied");
        })
        .catch(fail),
    copyAddress: (pins) => {
      const urls = pins.map(pinAddress).filter(Boolean);
      if (urls.length === 0) {
        void showNotice("No address to copy.");
        return;
      }
      void copyText(urls.join("\n"))
        .then(() => showNotice(urls.length === 1 ? "Address copied" : `Copied ${urls.length} addresses`))
        .catch(fail);
    },
    saveAs: (pin) =>
      void savePinFileAs(pin)
        .then((saved) => (saved ? showNotice("Saved") : undefined))
        .catch(fail),
    exportPins: (pins) =>
      exportTo(
        (dir) => exportPins(pins.map((pin) => pin.id), dir),
        (count) => `Exported ${plural(Number(count), "file")}`,
      ),
    addToFolder: (pins, x, y) => setMoveMenu({ ids: pins.map((pin) => pin.id), x, y, mode: "add" }),
    moveToFolder: (pins, x, y) => setMoveMenu({ ids: pins.map((pin) => pin.id), x, y, mode: "move" }),
    removeFromFolder: (pins) => {
      if (scope.kind === "folder") {
        const ids = pins.map((pin) => pin.id);
        void removePinsFromFolder(ids, scope.id).catch(fail);
        dropFromSelection(ids);
      }
    },
    setCover: (pin) => {
      if (scope.kind === "folder") {
        void updateBoardFolder(scope.id, { coverPinId: pin.id })
          .then(() => showNotice("Cover set"))
          .catch(fail);
      }
    },
    rate: (pins, rating) => void Promise.all(pins.map((pin) => updatePin(pin.id, { rating }))).catch(fail),
    trash: (pins) => {
      const ids = pins.map((pin) => pin.id);
      if (ids.length === 0) {
        return;
      }
      void trashPins(ids)
        .then(() => showNotice(`Moved ${plural(ids.length, "item")} to Trash`))
        .catch(fail);
      dropFromSelection(ids);
    },
    restore: (pins) => {
      const ids = pins.map((pin) => pin.id);
      void restorePins(ids)
        .then(() => showNotice(`Restored ${plural(ids.length, "item")}`))
        .catch(fail);
      dropFromSelection(ids);
    },
    deleteForever: (pins) => setConfirm({ kind: "forever", ids: pins.map((pin) => pin.id) }),
  };

  const exportFolder = (id: string) =>
    exportTo(
      (dir) => exportBoardFolder(id, dir),
      (path) => `Exported to ${String(path)}`,
    );

  const dropPins = (ids: string[], target: PinDropTarget, move: boolean) => {
    if (ids.length === 0) {
      return;
    }
    const from = scope.kind === "folder" ? scope.id : null;
    let work: Promise<void>;
    if (target.kind === "trash") {
      actions.trash(ids.map((id) => pinsById.get(id)).filter((pin): pin is Pin => Boolean(pin)));
      return;
    } else if (target.kind === "uncategorized") {
      work = movePins(ids, null, UNCATEGORIZED);
    } else if (move) {
      work = movePins(ids, from, target.id);
    } else {
      work = addPinsToFolder(ids, target.id);
    }
    void work.catch(fail);
  };

  // Keyboard, paste and drop

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (overlayOpen || isTyping(event.target)) {
        return;
      }
      const ctrl = event.ctrlKey || event.metaKey;
      if (event.key === "Escape" && selected.size > 0) {
        setSelected(new Set());
        setAnchorId(null);
      } else if (event.key === "Delete" && selectedPins.length > 0) {
        event.preventDefault();
        if (inTrash) {
          actions.deleteForever(selectedPins);
        } else {
          actions.trash(selectedPins);
        }
      } else if (ctrl && event.key.toLowerCase() === "a" && shown.length > 0) {
        event.preventDefault();
        setSelected(new Set(shown.map((pin) => pin.id)));
      } else if (ctrl && event.key.toLowerCase() === "f") {
        event.preventDefault();
        document.getElementById("boards-search-input")?.focus();
      } else if (ctrl && event.key.toLowerCase() === "i") {
        event.preventDefault();
        updatePrefs({ inspector: !prefs.inspector });
      } else if (ctrl && event.key.toLowerCase() === "c" && selectedPins.length === 1 && window.getSelection()?.isCollapsed !== false) {
        const pin = selectedPins[0];
        if (pin.hasImage || pin.kind === "image") {
          event.preventDefault();
          actions.copyImage(pin);
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  useEffect(() => {
    const onPaste = (event: ClipboardEvent) => {
      if (overlayOpen || isTyping(event.target) || inTrash) {
        return;
      }
      const data = event.clipboardData;
      if (!data) {
        return;
      }
      event.preventDefault();
      const files: File[] = [...data.files];
      if (files.length === 0) {
        for (const item of data.items) {
          const file = item.kind === "file" ? item.getAsFile() : null;
          if (file) {
            files.push(file);
          }
        }
      }
      if (files.length > 0) {
        importDropped(files.map((file) => ({ file, relativeDir: "" })));
        return;
      }
      const found = linksInText(data.getData("text/plain"));
      if (found.length > 0) {
        importLinks(found);
        return;
      }
      pasteNative();
    };
    window.addEventListener("paste", onPaste);
    return () => window.removeEventListener("paste", onPaste);
  }, [overlayOpen, inTrash, importDropped, importLinks, pasteNative]);

  const onDragEnter = (event: DragEvent<HTMLElement>) => {
    if (!isOutsideDrag(event) || inTrash) {
      return;
    }
    event.preventDefault();
    dragDepth.current += 1;
    setDragOver(true);
  };

  const onDragLeave = (event: DragEvent<HTMLElement>) => {
    if (!isOutsideDrag(event)) {
      return;
    }
    dragDepth.current = Math.max(0, dragDepth.current - 1);
    if (dragDepth.current === 0) {
      setDragOver(false);
    }
  };

  const onDrop = (event: DragEvent<HTMLElement>) => {
    if (!isOutsideDrag(event) || inTrash) {
      return;
    }
    event.preventDefault();
    dragDepth.current = 0;
    setDragOver(false);
    const entries = dropEntries(event.dataTransfer);
    if (entries.length > 0) {
      importEntries(entries, saveTarget);
      return;
    }
    const files = [...event.dataTransfer.files];
    if (files.length > 0) {
      importDropped(files.map((file) => ({ file, relativeDir: "" })));
      return;
    }
    importLinks(linksInText(event.dataTransfer.getData("text/uri-list") || event.dataTransfer.getData("text/plain")));
  };

  // Folders

  const newFolder = (parentId: string | null) =>
    void createBoardFolder("New folder", parentId)
      .then((created) => setRenameFolder({ id: created.id, name: created.name }))
      .catch(fail);

  const submitRenameFolder = () => {
    if (renameFolder && renameFolder.name.trim()) {
      void renameBoardFolder(renameFolder.id, renameFolder.name.trim()).catch(fail);
    }
    setRenameFolder(null);
  };

  const submitLink = () => {
    const urls = linksInText(linkValue);
    if (urls.length === 0) {
      setError("That link needs to start with http:// or https://.");
      return;
    }
    setLinkOpen(false);
    setLinkValue("");
    importLinks(urls);
  };

  const runConfirm = () => {
    if (!confirm) {
      return;
    }
    if (confirm.kind === "forever") {
      void deletePinsForever(confirm.ids).catch(fail);
      dropFromSelection(confirm.ids);
    } else if (confirm.kind === "empty-trash") {
      void emptyBoardTrash()
        .then((count) => showNotice(`Deleted ${plural(count, "item")} for good`))
        .catch(fail);
      setSelected(new Set());
    } else {
      const id = confirm.id;
      void deleteBoardFolder(id)
        .then((trashed) => (trashed > 0 ? showNotice(`Moved ${plural(trashed, "item")} to Trash`) : undefined))
        .catch(fail);
    }
    setConfirm(null);
  };

  // Rendering

  const empty = (() => {
    if (subfolders.length > 0) {
      return null;
    }
    if (base.length > 0 || query.trim()) {
      return (
        <EmptyState
          id="boards-no-match"
          icon={SearchX}
          title="Nothing matches"
          hint={filterCount > 0 ? "Clear a filter or change the search." : "Search looks at names, notes, tags, page text, file names and addresses."}
        />
      );
    }
    switch (scope.kind) {
      case "trash":
        return <EmptyState id="boards-trash-empty" icon={Trash2} title="Trash is empty" hint="Deleted items wait here until you empty the trash." />;
      case "recent":
        return <EmptyState id="boards-recent-empty" icon={Clock} title="Nothing used yet" hint="Items you open, copy or view show up here." />;
      case "folder":
        return (
          <EmptyState
            id="boards-folder-empty"
            icon={LayoutGrid}
            title="This folder is empty"
            hint="Drop files or folders here, paste, or drag items onto the folder in the sidebar."
          />
        );
      default:
        return (
          <EmptyState
            id="boards-empty"
            icon={LayoutGrid}
            title="Nothing here yet"
            hint="Drop or paste files, folders, images and links, use Add, or save a page from the Boards button in the address bar."
          />
        );
    }
  })();

  const header =
    subfolders.length > 0 ? (
      <FolderCards
        boards={boards}
        folders={subfolders}
        size={prefs.size}
        onOpen={(item) => go({ kind: "folder", id: item.id })}
        onMenu={(item, x, y) => setFolderMenu({ folder: item, x, y })}
        onDropPins={(ids, item, move) => dropPins(ids, { kind: "folder", id: item.id }, move)}
      />
    ) : null;

  const menuPins = pinMenu ? pinMenu.ids.map((id) => pinsById.get(id)).filter((pin): pin is Pin => Boolean(pin)) : [];
  const movePinsList = moveMenu ? moveMenu.ids.map((id) => pinsById.get(id)).filter((pin): pin is Pin => Boolean(pin)) : [];
  const moveCurrent = movePinsList.length > 0 ? movePinsList[0].folderIds.filter((id) => movePinsList.every((pin) => pin.folderIds.includes(id))) : [];

  return (
    <div id="boards-page" className="flex h-full bg-zinc-950 text-zinc-100">
      <BoardsSidebar
        boards={boards}
        scope={scope}
        onScope={go}
        renaming={renaming}
        onRenaming={setRenaming}
        onDeleteFolder={(id) => setConfirm({ kind: "folder", id })}
        onExportFolder={exportFolder}
        onDropPins={dropPins}
        onDropFiles={importEntries}
        onError={fail}
      />
      <main
        id="boards-main"
        className="relative flex min-w-0 flex-1 flex-col"
        onDragEnter={onDragEnter}
        onDragOver={(event) => {
          if (isOutsideDrag(event) && !inTrash) {
            event.preventDefault();
            event.dataTransfer.dropEffect = "copy";
          }
        }}
        onDragLeave={onDragLeave}
        onDrop={onDrop}
      >
        <BoardsToolbar
          boards={boards}
          scope={scope}
          count={scope.kind === "tags" ? 0 : shown.length}
          selectedCount={selected.size}
          canBack={history.index > 0}
          canForward={history.index < history.stack.length - 1}
          onBack={() => setHistory((current) => ({ ...current, index: Math.max(0, current.index - 1) }))}
          onForward={() => setHistory((current) => ({ ...current, index: Math.min(current.stack.length - 1, current.index + 1) }))}
          onScope={go}
          query={query}
          onQuery={setQuery}
          prefs={prefs}
          onPrefs={updatePrefs}
          sort={sort}
          folderSort={Boolean(folderSort)}
          onSort={(next) => {
            if (folderSort && scope.kind === "folder") {
              void updateBoardFolder(scope.id, { sort: next }).catch(fail);
            } else {
              updatePrefs({ sort: next });
            }
          }}
          filterOpen={filterOpen}
          filterCount={filterCount}
          onToggleFilter={() => setFilterOpen((current) => !current)}
          onAddFiles={pickFiles}
          onImportFolders={pickFolders}
          onAddLink={() => {
            setLinkValue("");
            setLinkOpen(true);
          }}
          onPaste={pasteNative}
          onImportEagle={pickEagle}
          onReshuffle={() => go({ kind: "random", seed: Date.now() })}
          onEmptyTrash={() => setConfirm({ kind: "empty-trash" })}
          onClearSelection={() => setSelected(new Set())}
        />
        {filterOpen && scope.kind !== "tags" ? <FilterBar filter={filter} onFilter={setFilter} pins={searched} /> : null}
        {error ? (
          <div id="boards-error" role="alert" className="flex items-center gap-3 border-b border-amber-900/50 bg-amber-950/40 px-6 py-2 text-sm text-amber-200">
            <span id="boards-error-text" className="min-w-0 flex-1">
              {error}
            </span>
            <Button id="boards-error-dismiss" variant="ghost" size="sm" onClick={() => setError("")}>
              Dismiss
            </Button>
          </div>
        ) : null}

        {!loaded ? (
          <div id="boards-loading" className="flex flex-wrap gap-3 px-6 py-4">
            {[260, 180, 220, 300, 200, 240, 280, 190].map((width, index) => (
              <Skeleton key={index} id={`boards-loading-${index}`} className="h-48 rounded-md" style={{ width }} />
            ))}
          </div>
        ) : scope.kind === "tags" ? (
          <TagsView boards={boards} query={query} onScope={go} onError={fail} />
        ) : prefs.layout === "list" ? (
          <PinTable
            id="boards-list"
            boards={boards}
            pins={shown}
            selected={selected}
            anchor={anchor}
            keyboard={!overlayOpen}
            header={header}
            empty={empty}
            {...handlers}
          />
        ) : (
          <PinGrid
            id="boards-grid"
            pins={shown}
            layout={prefs.layout}
            size={prefs.size}
            captions={prefs.captions}
            selected={selected}
            anchor={anchor}
            keyboard={!overlayOpen}
            header={header}
            empty={empty}
            onBackgroundMenu={(x, y) => setBackgroundMenu({ x, y })}
            {...handlers}
          />
        )}

        {dragOver ? (
          <div id="boards-drop" className="pointer-events-none absolute inset-0 z-20 flex items-center justify-center bg-zinc-950/75">
            <p id="boards-drop-label" className="rounded-xl border border-sky-500 bg-sky-950/90 px-6 py-4 text-sm text-sky-100">
              Add to {saveTargetName}. Folders keep their structure.
            </p>
          </div>
        ) : null}

        {viewerId && viewerIndex >= 0 ? (
          <PinViewer pins={shown} index={viewerIndex} onIndex={(index) => setViewerId(shown[index]?.id ?? null)} onClose={() => setViewerId(null)} actions={actions} />
        ) : null}

        <ImportProgress
          jobs={jobs}
          onCancel={cancelJob}
          onDismiss={(jobId) => setJobs((current) => current.filter((job) => job.jobId !== jobId))}
          onOpenFolder={(id) => go({ kind: "folder", id })}
        />
      </main>

      {prefs.inspector ? (
        <Inspector
          boards={boards}
          scope={scope}
          selected={selectedPins}
          shown={shown}
          actions={actions}
          onScope={go}
          onColor={(color) => {
            setFilter((current) => ({ ...current, color }));
            setFilterOpen(true);
          }}
          onExportFolder={exportFolder}
          onError={fail}
        />
      ) : null}

      {pinMenu && menuPins.length > 0 ? (
        <PinContextMenu
          pins={menuPins}
          x={pinMenu.x}
          y={pinMenu.y}
          folderId={scope.kind === "folder" ? scope.id : null}
          inTrash={inTrash}
          actions={actions}
          onClose={() => setPinMenu(null)}
        />
      ) : null}

      {backgroundMenu ? (
        <ContextMenu id="boards-background-menu" label="Boards" x={backgroundMenu.x} y={backgroundMenu.y} onClose={() => setBackgroundMenu(null)}>
          {inTrash ? (
            <ContextMenuItem
              id="boards-background-menu-empty-trash"
              icon={<Trash2 />}
              label="Empty Trash…"
              danger
              onClick={() => {
                setBackgroundMenu(null);
                setConfirm({ kind: "empty-trash" });
              }}
            />
          ) : (
            <>
              <ContextMenuItem
                id="boards-background-menu-paste"
                icon={<ClipboardPaste />}
                label="Paste"
                onClick={() => {
                  setBackgroundMenu(null);
                  pasteNative();
                }}
              />
              <ContextMenuItem
                id="boards-background-menu-files"
                icon={<FilePlus />}
                label="Add files…"
                onClick={() => {
                  setBackgroundMenu(null);
                  pickFiles();
                }}
              />
              <ContextMenuItem
                id="boards-background-menu-folders"
                icon={<FolderInput />}
                label="Import folders…"
                onClick={() => {
                  setBackgroundMenu(null);
                  pickFolders();
                }}
              />
              <ContextMenuSeparator id="boards-background-menu-separator" />
              <ContextMenuItem
                id="boards-background-menu-new-folder"
                icon={<FolderPlus />}
                label={scope.kind === "folder" ? "New subfolder" : "New folder"}
                onClick={() => {
                  setBackgroundMenu(null);
                  newFolder(scope.kind === "folder" ? scope.id : null);
                }}
              />
            </>
          )}
          {shown.length > 0 ? (
            <ContextMenuItem
              id="boards-background-menu-select-all"
              icon={<SquareMousePointer />}
              label="Select all"
              trailing="Ctrl+A"
              onClick={() => {
                setBackgroundMenu(null);
                setSelected(new Set(shown.map((pin) => pin.id)));
              }}
            />
          ) : null}
        </ContextMenu>
      ) : null}

      {folderMenu ? (
        <FolderContextMenu
          folder={folderMenu.folder}
          x={folderMenu.x}
          y={folderMenu.y}
          onClose={() => setFolderMenu(null)}
          onNewSubfolder={() => newFolder(folderMenu.folder.id)}
          onRename={() => setRenameFolder({ id: folderMenu.folder.id, name: folderMenu.folder.name })}
          onUpdate={(patch) => void updateBoardFolder(folderMenu.folder.id, patch).catch(fail)}
          onDuplicate={() => void duplicateBoardFolder(folderMenu.folder.id).catch(fail)}
          onExport={() => exportFolder(folderMenu.folder.id)}
          onMoveToTop={() => void moveBoardFolder(folderMenu.folder.id, null).catch(fail)}
          onDelete={() => setConfirm({ kind: "folder", id: folderMenu.folder.id })}
        />
      ) : null}

      {moveMenu ? (
        <MoveMenu
          boards={boards}
          x={moveMenu.x}
          y={moveMenu.y}
          label={moveMenu.mode === "add" ? "Add to folder" : "Move to folder"}
          current={moveCurrent}
          allowUncategorized={moveMenu.mode === "move"}
          onPick={(folderId) => {
            const ids = moveMenu.ids;
            const work = moveMenu.mode === "add" ? addPinsToFolder(ids, folderId) : movePins(ids, scope.kind === "folder" ? scope.id : null, folderId);
            void work.catch(fail);
            if (moveMenu.mode === "move" && scope.kind === "folder") {
              dropFromSelection(ids);
            }
          }}
          onClose={() => setMoveMenu(null)}
          onError={fail}
        />
      ) : null}

      <Dialog id="boards-add-link-dialog" title="Add a link" open={linkOpen} onClose={() => setLinkOpen(false)} onEnter={submitLink} panelClassName="max-w-md">
        <label id="boards-add-link-label" className="flex flex-col gap-1 text-xs text-zinc-400">
          Address
          <textarea
            id="boards-add-link-input"
            autoFocus
            rows={3}
            value={linkValue}
            placeholder="https://"
            className="rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-sm text-zinc-100 outline-none focus:border-sky-500"
            onChange={(event) => setLinkValue(event.target.value)}
          />
          <span id="boards-add-link-hint" className="text-zinc-500">
            Paste one or more links. Each becomes its own item in {saveTargetName}.
          </span>
        </label>
        <div id="boards-add-link-actions" className="mt-4 flex justify-end gap-2">
          <Button id="boards-add-link-cancel" onClick={() => setLinkOpen(false)}>
            Cancel
          </Button>
          <Button id="boards-add-link-save" variant="primary" onClick={submitLink}>
            Add
          </Button>
        </div>
      </Dialog>

      <Dialog id="boards-rename-folder" title="Folder name" open={renameFolder !== null} onClose={() => setRenameFolder(null)} onEnter={submitRenameFolder} panelClassName="max-w-sm">
        <input
          id="boards-rename-folder-input"
          autoFocus
          value={renameFolder?.name ?? ""}
          className="h-8 w-full rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500"
          onFocus={(event) => event.currentTarget.select()}
          onChange={(event) => setRenameFolder((current) => (current ? { ...current, name: event.target.value } : current))}
        />
        <div id="boards-rename-folder-actions" className="mt-4 flex justify-end gap-2">
          <Button id="boards-rename-folder-cancel" size="sm" variant="ghost" onClick={() => setRenameFolder(null)}>
            Cancel
          </Button>
          <Button id="boards-rename-folder-save" size="sm" variant="primary" onClick={submitRenameFolder}>
            Save
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
  let action = "Delete";
  if (confirm?.kind === "forever") {
    title = confirm.ids.length === 1 ? "Delete this item for good?" : `Delete ${confirm.ids.length} items for good?`;
    detail = "Their saved files are removed too. This can't be undone.";
  } else if (confirm?.kind === "empty-trash") {
    const count = boards.pins.filter((pin) => pin.deletedAt !== null).length;
    title = "Empty Trash?";
    detail = count === 0 ? "The trash is already empty." : `${plural(count, "item")} and their saved files will be deleted. This can't be undone.`;
    action = "Empty Trash";
  } else if (confirm?.kind === "folder") {
    const folder = boards.folders.find((item) => item.id === confirm.id);
    const contents = folderContents(boards, confirm.id);
    title = `Delete "${folder?.name ?? "folder"}"?`;
    const parts = [contents.folders > 0 ? `its ${plural(contents.folders, "subfolder")} are deleted too` : ""];
    if (contents.trashed > 0) {
      parts.push(`${plural(contents.trashed, "item")} that ${contents.trashed === 1 ? "is" : "are"} in no other folder will move to the Trash`);
    }
    const kept = contents.pins - contents.trashed;
    if (kept > 0) {
      parts.push(`${plural(kept, "item")} that ${kept === 1 ? "is" : "are"} also in other folders will stay there`);
    }
    const said = parts.filter(Boolean);
    detail = said.length === 0 ? "The folder is empty." : `${said.join("; ")}.`.replace(/^./, (first) => first.toUpperCase());
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
          {action}
        </Button>
      </div>
    </Dialog>
  );
}
