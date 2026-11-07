import { Button } from "@/components/Button";
import { IconButton } from "@/components/IconButton";
import { extractPage, savePrintSettings, defaultDownloadDir, chooseSavePath, printHtmlToPdf, openSavedFile, printHtmlDialog, showSavedFile } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { useSettings } from "@/lib/use-browser";
import { ExtractedPage, PrintEditorSettings, PaperSize, PrintPaper } from "@type/browser";
import { CircleAlert, RotateCcw, LoaderCircle, Printer, Undo2, Redo2, ArrowUpToLine, Trash2, Copy, X, PencilLine, MousePointerClick, FileDown, FileCheck, FolderOpen } from "lucide-react";
import { useState, useRef, useEffect, useMemo, useCallback, ReactNode } from "react";
import { Segmented, OptionRow } from "../PopoverParts";
import { joinPath, fileNameFor } from "../shared";
import { errorText } from "../use-page-tool";
import { CleanupKind } from "./cleanup";
import { FrameEditor } from "./editor-dom";
import { ContentState, BuiltDocument, contentStateFrom, parsePage, buildDocument, FORMAT_STYLE_ID, formatCss, serializeForPrint, finalizeRendered } from "./print-document";
import { DEFAULT_PRINT_SETTINGS, contentBox, pdfOptions, MODE_OPTIONS, FONT_OPTIONS, IMAGE_OPTIONS, PAPER_GROUPS, PAPER_SIZES, paperOptionLabel, visiblePapers, MARGIN_OPTIONS } from "./print-options";


type PrintEditorProps = {
  tabId: number;
};

type History = {
  past: ContentState[];
  present: ContentState;
  future: ContentState[];
};

type Notice = { kind: "saved"; path: string } | { kind: "error"; text: string } | null;

type CleanupFlag = "removeAds" | "removeNavigation" | "removePopups" | "removeSocial" | "removeMedia";

const HISTORY_LIMIT = 100;

const CLEANUP_ROWS: { flag: CleanupFlag; kind: CleanupKind; label: string; hint: string }[] = [
  { flag: "removeAds", kind: "ads", label: "Remove ads", hint: "Ad slots, sponsored and promoted blocks" },
  { flag: "removePopups", kind: "popups", label: "Remove pop-ups", hint: "Cookie banners, sign-up boxes, overlays" },
  { flag: "removeNavigation", kind: "navigation", label: "Remove navigation", hint: "Menus, headers, footers, sidebars" },
  { flag: "removeSocial", kind: "social", label: "Remove social and comments", hint: "Share bars, comments, related stories" },
  { flag: "removeMedia", kind: "media", label: "Remove videos and embeds", hint: "Players, iframes, audio" },
];

function hostOf(url: string) {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
}

function editingField(target: EventTarget | null) {
  const element = target as HTMLElement | null;
  return !!element && (element.isContentEditable || ["INPUT", "SELECT", "TEXTAREA"].includes(element.tagName));
}

export function PrintEditor({ tabId }: PrintEditorProps) {
  const settings = useSettings();
  const [page, setPage] = useState<ExtractedPage | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);
  const [format, setFormat] = useState<PrintEditorSettings | null>(null);
  const [history, setHistory] = useState<History | null>(null);
  const [renderVersion, setRenderVersion] = useState(0);
  const [built, setBuilt] = useState<BuiltDocument | null>(null);
  const [hover, setHover] = useState<string | null>(null);
  const [selectionCount, setSelectionCount] = useState(0);
  const [editText, setEditText] = useState(false);
  const [busy, setBusy] = useState<"pdf" | "print" | null>(null);
  const [notice, setNotice] = useState<Notice>(null);
  const [overlaysRemoved, setOverlaysRemoved] = useState(0);

  const frameRef = useRef<HTMLIFrameElement>(null);
  const editorRef = useRef<FrameEditor | null>(null);
  const historyRef = useRef(history);
  historyRef.current = history;
  const formatRef = useRef(format);
  formatRef.current = format;
  const settingsRef = useRef(settings);
  settingsRef.current = settings;
  const editTextRef = useRef(editText);
  editTextRef.current = editText;
  const scrollRef = useRef(0);

  useEffect(() => {
    if (!Number.isFinite(tabId) || tabId <= 0) {
      setLoadError("This editor was opened without a page. Open it from the Print to PDF button in the toolbar.");
      return;
    }
    let cancelled = false;
    setLoadError(null);
    setPage(null);
    extractPage(tabId)
      .then((next) => {
        if (!cancelled) {
          setPage(next);
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setLoadError(errorText(error));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [tabId, attempt]);

  useEffect(() => {
    document.title = page ? `Print: ${page.title || hostOf(page.url)}` : "Print to PDF";
  }, [page]);

  useEffect(() => {
    if (settings && !formatRef.current) {
      const stored = { ...DEFAULT_PRINT_SETTINGS, ...settings.printEditor };
      const menu = visiblePapers(stored);
      const initial = menu.includes(stored.paper) ? stored : { ...stored, paper: menu[0] };
      setFormat(initial);
      setHistory({ past: [], present: contentStateFrom(initial), future: [] });
    }
  }, [settings]);

  useEffect(() => {
    const print = settings?.printEditor;
    if (!print || !formatRef.current) {
      return;
    }
    setFormat((current) => {
      if (!current) {
        return current;
      }
      const menu = visiblePapers({
        ...current,
        paperChoices: print.paperChoices,
        customWidth: print.customWidth,
        customHeight: print.customHeight,
      });
      const paper = menu.includes(current.paper) ? current.paper : menu[0];
      if (
        paper === current.paper &&
        current.paperChoices === print.paperChoices &&
        current.customWidth === print.customWidth &&
        current.customHeight === print.customHeight
      ) {
        return current;
      }
      return {
        ...current,
        paper,
        paperChoices: print.paperChoices,
        customWidth: print.customWidth,
        customHeight: print.customHeight,
      };
    });
  }, [settings]);

  const sourceDoc = useMemo(() => (page ? parsePage(page) : null), [page]);
  const ready = format !== null && history !== null;

  useEffect(() => {
    const current = historyRef.current;
    const currentFormat = formatRef.current;
    if (!sourceDoc || !page || !current || !currentFormat) {
      return;
    }
    const view = frameRef.current?.contentWindow;
    if (view) {
      scrollRef.current = view.scrollY;
    }
    editorRef.current?.flushText();
    setBuilt(buildDocument(sourceDoc, page, current.present, currentFormat));
  }, [sourceDoc, page, renderVersion, ready]);

  // Set on every build, even when the HTML is unchanged: the live frame may hold deletions
  // that an undo has to bring back.
  useEffect(() => {
    if (built && frameRef.current) {
      frameRef.current.srcdoc = built.html;
    }
  }, [built]);

  useEffect(() => () => editorRef.current?.dispose(), []);

  // Remember the choices on this profile. Named quick-clean presets stay as they were saved.
  const rememberPrintSettings = useCallback(() => {
    const currentFormat = formatRef.current;
    const currentHistory = historyRef.current;
    const current = settingsRef.current;
    if (!currentFormat || !currentHistory || !current) {
      return;
    }
    const { present } = currentHistory;
    const next: PrintEditorSettings = {
      ...currentFormat,
      mode: present.mode,
      removeAds: present.removeAds,
      removeNavigation: present.removeNavigation,
      removePopups: present.removePopups,
      removeSocial: present.removeSocial,
      removeMedia: present.removeMedia,
      paperChoices: current.printEditor.paperChoices,
      customWidth: current.printEditor.customWidth,
      customHeight: current.printEditor.customHeight,
      presets: current.printEditor.presets,
    };
    const saved = current.printEditor as Record<string, unknown>;
    if ((Object.keys(next) as (keyof PrintEditorSettings)[]).every((key) => next[key] === saved[key])) {
      return;
    }
    void savePrintSettings(next)
      .then((stored) => {
        settingsRef.current = stored;
      })
      .catch(() => undefined);
  }, []);

  useEffect(() => {
    if (!format || !history) {
      return;
    }
    const timer = window.setTimeout(rememberPrintSettings, 500);
    return () => window.clearTimeout(timer);
  }, [format, history, rememberPrintSettings]);

  useEffect(() => {
    const doc = frameRef.current?.contentDocument;
    if (!doc || !format || !built) {
      return;
    }
    const style = doc.getElementById(FORMAT_STYLE_ID);
    if (style) {
      style.textContent = formatCss(format, built.articleFound ? "article" : "full");
    }
    editorRef.current?.setPageHeight(contentBox(format).height);
  }, [format, built]);

  useEffect(() => {
    editorRef.current?.setEditText(editText);
  }, [editText]);

  const commit = useCallback((change: (present: ContentState) => ContentState, rerender: boolean) => {
    setHistory((current) =>
      current
        ? {
            past: [...current.past, current.present].slice(-HISTORY_LIMIT),
            present: change(current.present),
            future: [],
          }
        : current,
    );
    if (rerender) {
      setRenderVersion((version) => version + 1);
    }
  }, []);

  const undo = useCallback(() => {
    editorRef.current?.flushText();
    setHistory((current) => {
      if (!current || current.past.length === 0) {
        return current;
      }
      return {
        past: current.past.slice(0, -1),
        present: current.past[current.past.length - 1],
        future: [current.present, ...current.future],
      };
    });
    setRenderVersion((version) => version + 1);
  }, []);

  const redo = useCallback(() => {
    editorRef.current?.flushText();
    setHistory((current) => {
      if (!current || current.future.length === 0) {
        return current;
      }
      return {
        past: [...current.past, current.present],
        present: current.future[0],
        future: current.future.slice(1),
      };
    });
    setRenderVersion((version) => version + 1);
  }, []);

  const recordText = useCallback((id: string, html: string) => {
    setHistory((current) => {
      if (!current) {
        return current;
      }
      const ops = current.present.ops;
      const last = ops[ops.length - 1];
      if (last?.type === "text" && last.id === id) {
        return { ...current, present: { ...current.present, ops: [...ops.slice(0, -1), { type: "text", id, html }] } };
      }
      return {
        past: [...current.past, current.present].slice(-HISTORY_LIMIT),
        present: { ...current.present, ops: [...ops, { type: "text", id, html }] },
        future: [],
      };
    });
  }, []);

  const currentHtml = () => {
    editorRef.current?.flushText();
    const doc = frameRef.current?.contentDocument;
    return doc?.documentElement ? serializeForPrint(doc) : null;
  };

  const savePdf = async () => {
    if (!page || !format || busy) {
      return;
    }
    const html = currentHtml();
    if (!html) {
      return;
    }
    const folder = settingsRef.current?.downloadDir || (await defaultDownloadDir());
    const path = await chooseSavePath(joinPath(folder, fileNameFor(page.title, page.url, "pdf")), "PDF document", ["pdf"]);
    if (!path) {
      return;
    }
    rememberPrintSettings();
    setBusy("pdf");
    setNotice(null);
    try {
      await printHtmlToPdf(html, pdfOptions(format, page.title, page.url), path);
      setNotice({ kind: "saved", path });
      if (format.openAfterSave) {
        void openSavedFile(path).catch(() => undefined);
      }
    } catch (error) {
      setNotice({ kind: "error", text: errorText(error) });
    } finally {
      setBusy(null);
    }
  };

  const printPage = async () => {
    if (busy) {
      return;
    }
    const html = currentHtml();
    if (!html) {
      return;
    }
    setBusy("print");
    setNotice(null);
    try {
      await printHtmlDialog(html);
    } catch (error) {
      setNotice({ kind: "error", text: errorText(error) });
    } finally {
      setBusy(null);
    }
  };

  const actions = useRef({ undo, redo, savePdf, printPage });
  actions.current = { undo, redo, savePdf, printPage };

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const key = event.key.toLowerCase();
      if (event.ctrlKey && (key === "s" || key === "p")) {
        event.preventDefault();
        void (key === "s" ? actions.current.savePdf() : actions.current.printPage());
        return;
      }
      if (editingField(event.target)) {
        return;
      }
      if (event.ctrlKey && key === "z" && !event.shiftKey) {
        event.preventDefault();
        actions.current.undo();
      } else if (event.ctrlKey && (key === "y" || (key === "z" && event.shiftKey))) {
        event.preventDefault();
        actions.current.redo();
      } else if (event.key === "Delete") {
        editorRef.current?.deleteSelected();
      } else if (event.key === "Escape") {
        editorRef.current?.clearSelection();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const onFrameLoad = () => {
    editorRef.current?.dispose();
    editorRef.current = null;
    const frame = frameRef.current;
    const doc = frame?.contentDocument;
    const currentFormat = formatRef.current;
    const current = historyRef.current;
    if (!frame || !doc?.body || !currentFormat || !current) {
      return;
    }
    setOverlaysRemoved(finalizeRendered(doc, current.present));
    const editor = new FrameEditor(
      doc,
      {
        onDelete: (ids) => commit((present) => ({ ...present, ops: [...present.ops, { type: "delete", ids }] }), false),
        onTextEdit: recordText,
        onSelectionChange: setSelectionCount,
        onHover: setHover,
        onUndo: () => actions.current.undo(),
        onRedo: () => actions.current.redo(),
        onShortcut: (action) => void (action === "save" ? actions.current.savePdf() : actions.current.printPage()),
      },
      contentBox(currentFormat).height,
    );
    if (editTextRef.current) {
      editor.setEditText(true);
    }
    editorRef.current = editor;
    setSelectionCount(0);
    setHover(null);
    frame.contentWindow?.scrollTo(0, scrollRef.current);
  };

  const updateFormat = (change: Partial<PrintEditorSettings>, rerender = false) => {
    setFormat((current) => (current ? { ...current, ...change } : current));
    if (rerender) {
      setRenderVersion((version) => version + 1);
    }
  };

  const setContent = (change: Partial<ContentState>) => commit((present) => ({ ...present, ...change }), true);

  if (loadError) {
    return (
      <main id="print-editor" className="flex h-screen items-center justify-center bg-zinc-950 p-8 text-zinc-100">
        <div id="print-editor-error" className="flex max-w-md flex-col items-center gap-3 text-center">
          <CircleAlert id="print-editor-error-icon" className="size-8 text-amber-400" aria-hidden />
          <h1 id="print-editor-error-title" className="text-lg font-semibold">
            Couldn't read the page
          </h1>
          <p id="print-editor-error-text" className="text-sm text-zinc-400">
            {loadError}
          </p>
          <p id="print-editor-error-hint" className="text-xs text-zinc-500">
            Keep the original tab open while you edit. If it was closed, open the page again and use Print to PDF from the
            toolbar.
          </p>
          <Button id="print-editor-error-retry" variant="primary" onClick={() => setAttempt((value) => value + 1)}>
            <RotateCcw id="print-editor-error-retry-icon" className="size-4" aria-hidden />
            Try again
          </Button>
        </div>
      </main>
    );
  }

  if (!page || !format || !history || !built) {
    return (
      <main id="print-editor" className="flex h-screen items-center justify-center bg-zinc-950 text-zinc-400">
        <div id="print-editor-loading" className="flex items-center gap-3 text-sm">
          <LoaderCircle id="print-editor-loading-icon" className="size-5 animate-spin text-sky-400" aria-hidden />
          Reading the page…
        </div>
      </main>
    );
  }

  const present = history.present;
  const menuSettings = settings?.printEditor ?? format;
  const paperMenu = visiblePapers({
    ...format,
    paperChoices: menuSettings.paperChoices,
    customWidth: menuSettings.customWidth,
    customHeight: menuSettings.customHeight,
  });
  const selectedPaper = paperMenu.includes(format.paper) ? format.paper : paperMenu[0];
  const box = contentBox({ ...format, paper: selectedPaper, customWidth: menuSettings.customWidth, customHeight: menuSettings.customHeight });
  const manualDeleted = present.ops.reduce((sum, op) => sum + (op.type === "delete" ? op.ids.length : 0), 0);
  const autoRemoved =
    Object.values(built.stats).reduce((sum, count) => sum + count, 0) + overlaysRemoved;
  const showArticleNote = present.mode === "article" && !built.articleFound;

  return (
    <main id="print-editor" className="flex h-screen flex-col bg-zinc-950 text-zinc-100">
      <header
        id="print-editor-toolbar"
        className="grid h-14 shrink-0 grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] items-center border-b border-zinc-800 px-3"
      >
        <div id="print-editor-heading" className="flex min-w-0 items-center gap-2.5 justify-self-start pr-3">
          <span
            id="print-editor-heading-icon"
            className="inline-flex size-8 shrink-0 items-center justify-center rounded-lg bg-sky-500/15 text-sky-300"
          >
            <Printer id="print-editor-heading-glyph" className="size-4" aria-hidden />
          </span>
          <div id="print-editor-heading-text" className="min-w-0">
            <h1 id="print-editor-title" className="max-w-72 truncate text-sm font-semibold">
              {page.title || hostOf(page.url)}
            </h1>
            <p id="print-editor-host" className="max-w-72 truncate text-xs text-zinc-500">
              {hostOf(page.url)}
            </p>
          </div>
        </div>

        <div id="print-editor-toolbar-controls" className="flex items-center justify-center gap-2">
          <ToolbarGroup id="print-editor-history">
            <IconButton id="print-editor-undo" label="Undo (Ctrl+Z)" disabled={history.past.length === 0} onClick={undo}>
              <Undo2 id="print-editor-undo-icon" aria-hidden />
            </IconButton>
            <IconButton id="print-editor-redo" label="Redo (Ctrl+Y)" disabled={history.future.length === 0} onClick={redo}>
              <Redo2 id="print-editor-redo-icon" aria-hidden />
            </IconButton>
            <IconButton
              id="print-editor-reset"
              label="Restore everything you deleted or edited"
              disabled={present.ops.length === 0}
              onClick={() => setContent({ ops: [] })}
            >
              <RotateCcw id="print-editor-reset-icon" aria-hidden />
            </IconButton>
          </ToolbarGroup>

          <ToolbarGroup id="print-editor-selection" divided>
            <IconButton
              id="print-editor-select-parent"
              label="Select the parent of the selection"
              disabled={editText}
              onClick={() => editorRef.current?.selectParent()}
            >
              <ArrowUpToLine id="print-editor-select-parent-icon" aria-hidden />
            </IconButton>
            <IconButton
              id="print-editor-delete-selected"
              label="Delete selected (Delete)"
              disabled={selectionCount === 0 || editText}
              onClick={() => editorRef.current?.deleteSelected()}
            >
              <Trash2 id="print-editor-delete-selected-icon" aria-hidden />
            </IconButton>
            <IconButton
              id="print-editor-delete-similar"
              label="Delete everything like the selection (Alt+click)"
              disabled={selectionCount === 0 || editText}
              onClick={() => editorRef.current?.deleteSimilar()}
            >
              <Copy id="print-editor-delete-similar-icon" aria-hidden />
            </IconButton>
            <IconButton
              id="print-editor-clear-selection"
              label="Clear selection (Esc)"
              disabled={selectionCount === 0}
              onClick={() => editorRef.current?.clearSelection()}
            >
              <X id="print-editor-clear-selection-icon" aria-hidden />
            </IconButton>
          </ToolbarGroup>

          <Button
            id="print-editor-edit-text"
            variant={editText ? "primary" : "ghost"}
            size="sm"
            aria-pressed={editText}
            onClick={() => setEditText((value) => !value)}
          >
            {editText ? (
              <PencilLine id="print-editor-edit-text-icon" className="size-4" aria-hidden />
            ) : (
              <MousePointerClick id="print-editor-edit-text-icon" className="size-4" aria-hidden />
            )}
            {editText ? "Editing text" : "Edit text"}
          </Button>
        </div>

        <div id="print-editor-toolbar-actions" className="flex items-center justify-end gap-2 justify-self-end pl-3">
          <EditorNotice notice={notice} onDismiss={() => setNotice(null)} />

          <Button id="print-editor-print" variant="ghost" disabled={busy !== null} onClick={() => void printPage()}>
            {busy === "print" ? (
              <LoaderCircle id="print-editor-print-icon" className="size-4 animate-spin" aria-hidden />
            ) : (
              <Printer id="print-editor-print-icon" className="size-4" aria-hidden />
            )}
            Print…
          </Button>
          <Button id="print-editor-save" variant="primary" disabled={busy !== null} onClick={() => void savePdf()}>
            {busy === "pdf" ? (
              <LoaderCircle id="print-editor-save-icon" className="size-4 animate-spin" aria-hidden />
            ) : (
              <FileDown id="print-editor-save-icon" className="size-4" aria-hidden />
            )}
            {busy === "pdf" ? "Saving…" : "Save PDF"}
          </Button>
        </div>
      </header>

      <div id="print-editor-body" className="flex min-h-0 flex-1">
        <section id="print-editor-preview" className="flex min-w-0 flex-1 justify-center overflow-auto bg-zinc-900/60 p-6">
          <div
            id="print-editor-sheet"
            className="h-full shrink-0 rounded-sm bg-white shadow-2xl shadow-black/40 ring-1 ring-black/10"
            style={{ width: box.width + box.margin * 2, padding: box.margin }}
          >
            <iframe
              id="print-editor-frame"
              ref={frameRef}
              title="Print preview"
              className="block h-full border-0 bg-white"
              style={{ width: box.width }}
              onLoad={onFrameLoad}
            />
          </div>
        </section>

        <aside id="print-editor-sidebar" className="w-80 shrink-0 overflow-y-auto border-l border-zinc-800 p-3">
          <SidebarSection id="print-editor-content" title="Content">
            <Segmented
              id="print-editor-mode"
              label="Content"
              value={present.mode}
              options={MODE_OPTIONS}
              onChange={(mode) => setContent({ mode })}
            />
            {showArticleNote ? (
              <p id="print-editor-mode-note" className="px-1 text-xs text-amber-300">
                No article was found on this page, so the full page is shown.
              </p>
            ) : null}
            <OptionRow
              id="print-editor-header"
              label="Title, source and date"
              hint="Shown above the content"
              checked={format.showHeader}
              onChange={(showHeader) => updateFormat({ showHeader }, true)}
            />
          </SidebarSection>

          <SidebarSection id="print-editor-cleanup" title="Clean up">
            {CLEANUP_ROWS.map((row) => {
              const removed = built.stats[row.kind] + (row.kind === "popups" ? overlaysRemoved : 0);
              return (
                <OptionRow
                  key={row.flag}
                  id={`print-editor-cleanup-${row.kind}`}
                  label={row.label}
                  hint={present[row.flag] && removed > 0 ? `${removed} removed · ${row.hint}` : row.hint}
                  checked={present[row.flag]}
                  onChange={(checked) => setContent({ [row.flag]: checked })}
                />
              );
            })}
          </SidebarSection>

          <SidebarSection id="print-editor-text" title="Text">
            <label id="print-editor-text-size" className="flex flex-col gap-1.5 px-1">
              <span id="print-editor-text-size-row" className="flex items-center justify-between text-sm">
                <span id="print-editor-text-size-label">Text size</span>
                <span id="print-editor-text-size-value" className="text-xs text-zinc-400">
                  {format.textSize}%
                </span>
              </span>
              <input
                id="print-editor-text-size-input"
                type="range"
                min={70}
                max={150}
                step={5}
                value={format.textSize}
                className="w-full accent-sky-500"
                onChange={(event) => updateFormat({ textSize: Number(event.target.value) })}
              />
            </label>
            <FieldLabel id="print-editor-font-label">Font</FieldLabel>
            <Segmented
              id="print-editor-font"
              label="Font"
              value={format.font}
              options={FONT_OPTIONS}
              onChange={(font) => updateFormat({ font })}
            />
          </SidebarSection>

          <SidebarSection id="print-editor-images" title="Images and color">
            <FieldLabel id="print-editor-image-size-label">Image size</FieldLabel>
            <select
              id="print-editor-image-size"
              value={format.imageSize}
              className="h-8 w-full rounded-md border border-zinc-700 bg-zinc-800 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500"
              onChange={(event) => updateFormat({ imageSize: event.target.value as PrintEditorSettings["imageSize"] })}
            >
              {IMAGE_OPTIONS.map((option) => (
                <option id={`print-editor-image-size-${option.value}`} key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
            <OptionRow
              id="print-editor-backgrounds"
              label="Remove background colors"
              hint="Black text on white, saves ink"
              checked={format.removeBackgrounds}
              onChange={(removeBackgrounds) => updateFormat({ removeBackgrounds })}
            />
          </SidebarSection>

          <SidebarSection id="print-editor-page" title="Page">
            <FieldLabel id="print-editor-paper-label">Paper size</FieldLabel>
            <select
              id="print-editor-paper"
              value={selectedPaper}
              className="h-8 w-full rounded-md border border-zinc-700 bg-zinc-800 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500"
              onChange={(event) => updateFormat({ paper: event.target.value as PrintPaper })}
            >
              {PAPER_GROUPS.map((group) => {
                const papers = paperMenu.filter(
                  (paper): paper is PaperSize => paper !== "custom" && PAPER_SIZES[paper].group === group.id,
                );
                if (papers.length === 0) {
                  return null;
                }
                return (
                  <optgroup id={`print-editor-paper-${group.id}`} key={group.id} label={group.label}>
                    {papers.map((paper) => (
                      <option id={`print-editor-paper-${paper}`} key={paper} value={paper}>
                        {paperOptionLabel(paper)}
                      </option>
                    ))}
                  </optgroup>
                );
              })}
              {paperMenu.includes("custom") ? (
                <optgroup id="print-editor-paper-custom" label="Custom">
                  <option id="print-editor-paper-custom-option" value="custom">
                    {paperOptionLabel("custom", menuSettings)}
                  </option>
                </optgroup>
              ) : null}
            </select>
            <FieldLabel id="print-editor-orientation-label">Orientation</FieldLabel>
            <Segmented
              id="print-editor-orientation"
              label="Orientation"
              value={format.landscape ? "landscape" : "portrait"}
              options={[
                { value: "portrait", label: "Portrait" },
                { value: "landscape", label: "Landscape" },
              ]}
              onChange={(value) => updateFormat({ landscape: value === "landscape" })}
            />
            <FieldLabel id="print-editor-margins-label">Margins</FieldLabel>
            <Segmented
              id="print-editor-margins"
              label="Margins"
              value={format.margins}
              options={MARGIN_OPTIONS}
              onChange={(margins) => updateFormat({ margins })}
            />
            <OptionRow
              id="print-editor-print-backgrounds"
              label="Print backgrounds"
              hint="Colors and background images"
              checked={format.printBackgrounds}
              onChange={(printBackgrounds) => updateFormat({ printBackgrounds })}
            />
            <OptionRow
              id="print-editor-header-footer"
              label="Page headers and footers"
              hint="Title, URL and page numbers on every page"
              checked={format.headerFooter}
              onChange={(headerFooter) => updateFormat({ headerFooter })}
            />
          </SidebarSection>

          <SidebarSection id="print-editor-after" title="After saving">
            <OptionRow
              id="print-editor-open-after"
              label="Open the PDF"
              hint="In your default PDF viewer"
              checked={format.openAfterSave}
              onChange={(openAfterSave) => updateFormat({ openAfterSave })}
            />
          </SidebarSection>
        </aside>
      </div>

      <footer
        id="print-editor-status"
        className="flex h-8 shrink-0 items-center gap-4 border-t border-zinc-800 px-3 text-xs text-zinc-500"
      >
        <span id="print-editor-status-hint" className="min-w-0 flex-1 truncate">
          {editText
            ? "Click into the text to edit it. Turn off Edit text to go back to deleting elements."
            : hover ?? "Click an element to delete it · Shift+click selects several · Alt+click deletes similar · Ctrl+Z undoes"}
        </span>
        {selectionCount > 0 ? (
          <span id="print-editor-status-selection" className="shrink-0 text-sky-300">
            {selectionCount} selected
          </span>
        ) : null}
        <span id="print-editor-status-removed" className="shrink-0">
          {autoRemoved} removed automatically · {manualDeleted} deleted by you
        </span>
        <span id="print-editor-status-paper" className="shrink-0">
          {paperOptionLabel(selectedPaper, menuSettings)} {format.landscape ? "landscape" : "portrait"}
        </span>
      </footer>
    </main>
  );
}

function ToolbarGroup({ id, children, divided = false }: { id: string; children: ReactNode; divided?: boolean }) {
  return (
    <div id={id} className={cn("flex items-center gap-0.5", divided && "border-l border-zinc-800 pl-2")}>
      {children}
    </div>
  );
}

function SidebarSection({ id, title, children }: { id: string; title: string; children: ReactNode }) {
  return (
    <section id={id} className="mb-4 flex flex-col gap-2">
      <h2 id={`${id}-title`} className="px-1 text-[11px] font-semibold uppercase tracking-wider text-zinc-500">
        {title}
      </h2>
      {children}
    </section>
  );
}

function FieldLabel({ id, children }: { id: string; children: ReactNode }) {
  return (
    <span id={id} className="px-1 pt-1 text-sm text-zinc-300">
      {children}
    </span>
  );
}

function EditorNotice({ notice, onDismiss }: { notice: Notice; onDismiss: () => void }) {
  if (!notice) {
    return null;
  }
  if (notice.kind === "error") {
    return (
      <div id="print-editor-notice" className="flex min-w-0 max-w-sm items-center gap-2 text-xs text-amber-300">
        <CircleAlert id="print-editor-notice-icon" className="size-4 shrink-0" aria-hidden />
        <span id="print-editor-notice-text" className="truncate" title={notice.text}>
          {notice.text}
        </span>
        <IconButton id="print-editor-notice-dismiss" label="Dismiss" className="size-6" onClick={onDismiss}>
          <X id="print-editor-notice-dismiss-icon" aria-hidden />
        </IconButton>
      </div>
    );
  }
  return (
    <div id="print-editor-notice" className="flex min-w-0 items-center gap-1.5 text-xs text-sky-200">
      <FileCheck id="print-editor-notice-icon" className="size-4 shrink-0" aria-hidden />
      <span id="print-editor-notice-text" className="max-w-48 truncate" title={notice.path}>
        Saved
      </span>
      <Button id="print-editor-notice-open" size="xs" variant="ghost" onClick={() => void openSavedFile(notice.path)}>
        Open
      </Button>
      <Button id="print-editor-notice-folder" size="xs" variant="ghost" onClick={() => void showSavedFile(notice.path)}>
        <FolderOpen id="print-editor-notice-folder-icon" className="size-3.5" aria-hidden />
        Show
      </Button>
    </div>
  );
}
