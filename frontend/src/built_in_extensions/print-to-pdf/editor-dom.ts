import { EDITOR_STYLE_ID, GUIDES_ID, ID_ATTR } from "./print-document";

export type FrameEditorEvents = {
  /** Elements were removed from the live document. */
  onDelete: (ids: string[]) => void;
  /** Text inside an element was edited. */
  onTextEdit: (id: string, html: string) => void;
  onSelectionChange: (count: number) => void;
  onHover: (label: string | null) => void;
  onUndo: () => void;
  onRedo: () => void;
  /** Ctrl+S or Ctrl+P pressed while the preview has focus. */
  onShortcut: (action: "save" | "print") => void;
};

const PROTECTED = new Set(["HTML", "BODY", "HEAD"]);

const EDITOR_CSS = `
.ce-hover { outline: 2px solid #ef4444 !important; outline-offset: -2px !important; background-color: rgba(239, 68, 68, 0.08) !important; cursor: pointer !important; }
.ce-selected { outline: 2px dashed #0ea5e9 !important; outline-offset: -2px !important; background-color: rgba(14, 165, 233, 0.1) !important; }
html.ce-editing-text .ce-hover, body.ce-editing-text .ce-hover { outline: none !important; background-color: transparent !important; cursor: text !important; }
html.ce-editing-text body, body.ce-editing-text { outline: none !important; caret-color: #0ea5e9; }
#${GUIDES_ID} {
  position: absolute !important; left: 0 !important; top: 0 !important; width: 100% !important;
  pointer-events: none !important; z-index: 2147483647 !important; margin: 0 !important; padding: 0 !important;
  background-image: repeating-linear-gradient(to bottom, transparent 0, transparent calc(var(--ce-page) - 1px), rgba(14, 165, 233, 0.55) calc(var(--ce-page) - 1px), rgba(14, 165, 233, 0.55) var(--ce-page)) !important;
}
`;

function describe(element: Element) {
  let label = element.tagName.toLowerCase();
  const id = element.getAttribute("id");
  if (id) {
    label += `#${id}`;
  }
  const classes = Array.from(element.classList).filter((name) => !name.startsWith("ce-"));
  if (classes.length > 0) {
    label += `.${classes.slice(0, 2).join(".")}`;
  }
  const text = element.textContent?.replace(/\s+/g, " ").trim() ?? "";
  return text ? `${label} — "${text.slice(0, 48)}${text.length > 48 ? "…" : ""}"` : label;
}

/** Where a node can be inserted. An iframe document can have a body before its root is exposed. */
function mountParent(doc: Document, preferBody = false): HTMLElement | null {
  if (preferBody) {
    return doc.documentElement ?? doc.body;
  }
  return doc.head ?? doc.body ?? doc.documentElement;
}

/** Inserts `node` now, or as soon as the document grows a parent. */
function appendWhenReady(doc: Document, node: Node, preferBody = false) {
  const parent = mountParent(doc, preferBody);
  if (parent) {
    parent.appendChild(node);
    return;
  }
  if (doc.readyState !== "loading") {
    return;
  }
  const place = () => {
    const next = mountParent(doc, preferBody);
    if (!next || node.parentNode) {
      return next !== null;
    }
    next.appendChild(node);
    return true;
  };
  const observer = new MutationObserver(() => {
    if (place()) {
      observer.disconnect();
    }
  });
  observer.observe(doc, { childList: true });
  doc.addEventListener(
    "DOMContentLoaded",
    () => {
      place();
      observer.disconnect();
    },
    { once: true },
  );
}

function editable(element: Element | null): Element | null {
  let current = element;
  while (current && !current.hasAttribute(ID_ATTR)) {
    current = current.parentElement;
  }
  if (!current || PROTECTED.has(current.tagName) || current.id === GUIDES_ID) {
    return null;
  }
  return current;
}

/** A selector for elements built like `element`, or null when it would match too broadly. */
export function similarSelector(element: Element) {
  const tag = element.tagName.toLowerCase();
  const classes = Array.from(element.classList).filter((name) => !name.startsWith("ce-"));
  if (classes.length > 0) {
    return `${tag}${classes.map((name) => `.${CSS.escape(name)}`).join("")}`;
  }
  const parent = element.parentElement;
  const parentClasses = parent ? Array.from(parent.classList).filter((name) => !name.startsWith("ce-")) : [];
  if (parent && !PROTECTED.has(parent.tagName) && parentClasses.length > 0) {
    return `${parent.tagName.toLowerCase()}${parentClasses.map((name) => `.${CSS.escape(name)}`).join("")} > ${tag}`;
  }
  return null;
}

/**
 * Wires click-to-delete, multi-select and text editing into the preview frame. The frame runs
 * no scripts of its own: these listeners belong to the editor page.
 */
export class FrameEditor {
  private hovered: Element | null = null;
  private selected = new Set<Element>();
  private editingText = false;
  private textTimer: number | null = null;
  private pendingText: Element | null = null;
  private resizeObserver: ResizeObserver | null = null;
  private readonly cleanup: (() => void)[] = [];

  constructor(
    private readonly doc: Document,
    private readonly events: FrameEditorEvents,
    pageHeight: number,
  ) {
    const style = doc.createElement("style");
    style.id = EDITOR_STYLE_ID;
    style.textContent = EDITOR_CSS;
    appendWhenReady(doc, style);
    this.installGuides(pageHeight);

    this.listen("mouseover", (event) => this.onOver(event as MouseEvent));
    this.listen("mouseout", (event) => this.onOut(event as MouseEvent));
    this.listen("click", (event) => this.onClick(event as MouseEvent), true);
    this.listen("auxclick", (event) => event.preventDefault(), true);
    this.listen("submit", (event) => event.preventDefault(), true);
    this.listen("dragstart", (event) => event.preventDefault(), true);
    this.listen("keydown", (event) => this.onKey(event as KeyboardEvent));
    this.listen("input", () => this.onInput());
  }

  private listen(type: string, handler: (event: Event) => void, capture = false) {
    this.doc.addEventListener(type, handler, capture);
    this.cleanup.push(() => this.doc.removeEventListener(type, handler, capture));
  }

  private installGuides(pageHeight: number) {
    const body = this.doc.body;
    if (!body || pageHeight <= 0) {
      return;
    }
    const guides = this.doc.createElement("div");
    guides.id = GUIDES_ID;
    guides.setAttribute("aria-hidden", "true");
    guides.style.setProperty("--ce-page", `${pageHeight}px`);
    appendWhenReady(this.doc, guides, true);
    const update = () => {
      const root = this.doc.documentElement ?? body;
      const height = Math.max(body.scrollHeight + body.offsetTop, root.clientHeight);
      guides.style.setProperty("height", `${height}px`, "important");
    };
    update();
    this.resizeObserver = new ResizeObserver(update);
    this.resizeObserver.observe(body);
  }

  setPageHeight(pageHeight: number) {
    this.doc.getElementById(GUIDES_ID)?.style.setProperty("--ce-page", `${pageHeight}px`);
  }

  setEditText(on: boolean) {
    this.editingText = on;
    this.flushText();
    this.setHovered(null);
    this.clearSelection();
    const body = this.doc.body;
    (this.doc.documentElement ?? body)?.classList.toggle("ce-editing-text", on);
    if (!body) {
      return;
    }
    if (on) {
      body.setAttribute("contenteditable", "true");
      body.setAttribute("spellcheck", "true");
      body.focus();
    } else {
      body.removeAttribute("contenteditable");
      body.removeAttribute("spellcheck");
    }
  }

  get selectionCount() {
    return this.selected.size;
  }

  private onOver(event: MouseEvent) {
    if (this.editingText) {
      return;
    }
    this.setHovered(editable(event.target as Element));
  }

  private onOut(event: MouseEvent) {
    if (!event.relatedTarget) {
      this.setHovered(null);
    }
  }

  private setHovered(element: Element | null) {
    if (this.hovered === element) {
      return;
    }
    this.hovered?.classList.remove("ce-hover");
    this.hovered = element;
    element?.classList.add("ce-hover");
    this.events.onHover(element ? describe(element) : null);
  }

  private onClick(event: MouseEvent) {
    const link = (event.target as Element | null)?.closest?.("a, button, input, select, label, summary");
    if (this.editingText) {
      if (link) {
        event.preventDefault();
      }
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    const target = editable(event.target as Element);
    if (!target) {
      return;
    }
    if (event.shiftKey || event.ctrlKey) {
      this.toggleSelected(target);
      return;
    }
    if (event.altKey) {
      this.deleteSimilarTo([target]);
      return;
    }
    this.remove([target]);
  }

  private onKey(event: KeyboardEvent) {
    const key = event.key.toLowerCase();
    if ((event.ctrlKey || event.metaKey) && (key === "s" || key === "p")) {
      event.preventDefault();
      this.flushText();
      this.events.onShortcut(key === "s" ? "save" : "print");
      return;
    }
    if ((event.ctrlKey || event.metaKey) && !this.editingText) {
      if (key === "z" && !event.shiftKey) {
        event.preventDefault();
        this.events.onUndo();
        return;
      }
      if (key === "y" || (key === "z" && event.shiftKey)) {
        event.preventDefault();
        this.events.onRedo();
        return;
      }
    }
    if (this.editingText) {
      if (event.key === "Escape") {
        (this.doc.activeElement as HTMLElement | null)?.blur();
      }
      return;
    }
    if (event.key === "Delete" || event.key === "Backspace") {
      event.preventDefault();
      this.deleteSelected();
    } else if (event.key === "Escape") {
      this.clearSelection();
    }
  }

  private onInput() {
    if (!this.editingText) {
      return;
    }
    const anchor = this.doc.getSelection()?.anchorNode ?? null;
    const element = anchor instanceof Element ? anchor : (anchor?.parentElement ?? null);
    const target = editable(element);
    if (!target) {
      return;
    }
    if (this.pendingText && this.pendingText !== target) {
      this.flushText();
    }
    this.pendingText = target;
    if (this.textTimer !== null) {
      window.clearTimeout(this.textTimer);
    }
    this.textTimer = window.setTimeout(() => this.flushText(), 500);
  }

  /** Records a pending text edit now, for example before undo or a rebuild. */
  flushText() {
    if (this.textTimer !== null) {
      window.clearTimeout(this.textTimer);
      this.textTimer = null;
    }
    const target = this.pendingText;
    this.pendingText = null;
    const id = target?.getAttribute(ID_ATTR);
    if (target && id && target.isConnected) {
      const clone = target.cloneNode(true) as Element;
      clone.querySelectorAll(".ce-hover, .ce-selected").forEach((element) => element.classList.remove("ce-hover", "ce-selected"));
      this.events.onTextEdit(id, clone.innerHTML);
    }
  }

  private toggleSelected(element: Element) {
    if (this.selected.has(element)) {
      this.selected.delete(element);
      element.classList.remove("ce-selected");
    } else {
      // Selecting a parent or a child of a selected element replaces it.
      for (const other of Array.from(this.selected)) {
        if (other.contains(element) || element.contains(other)) {
          this.selected.delete(other);
          other.classList.remove("ce-selected");
        }
      }
      this.selected.add(element);
      element.classList.add("ce-selected");
    }
    this.events.onSelectionChange(this.selected.size);
  }

  clearSelection() {
    for (const element of this.selected) {
      element.classList.remove("ce-selected");
    }
    this.selected.clear();
    this.events.onSelectionChange(0);
  }

  /** Widens each selected element (or the hovered one) to its parent. */
  selectParent() {
    const current = this.selected.size > 0 ? Array.from(this.selected) : this.hovered ? [this.hovered] : [];
    this.clearSelection();
    for (const element of current) {
      const parent = editable(element.parentElement) ?? element;
      if (!this.selected.has(parent)) {
        this.toggleSelected(parent);
      }
    }
  }

  deleteSelected() {
    const targets = Array.from(this.selected);
    this.clearSelection();
    this.remove(targets);
  }

  deleteSimilar() {
    const targets = this.selected.size > 0 ? Array.from(this.selected) : this.hovered ? [this.hovered] : [];
    this.clearSelection();
    this.deleteSimilarTo(targets);
  }

  private deleteSimilarTo(targets: Element[]) {
    const matches = new Set<Element>(targets);
    for (const target of targets) {
      const selector = similarSelector(target);
      if (!selector) {
        continue;
      }
      try {
        this.doc.body?.querySelectorAll(selector).forEach((element) => {
          const owned = editable(element);
          if (owned === element) {
            matches.add(element);
          }
        });
      } catch {
        // An unusual class name the selector engine rejects.
      }
    }
    this.remove(Array.from(matches));
  }

  private remove(targets: Element[]) {
    const roots = targets.filter(
      (element) => element.isConnected && !targets.some((other) => other !== element && other.contains(element)),
    );
    const ids: string[] = [];
    for (const element of roots) {
      const id = element.getAttribute(ID_ATTR);
      if (!id || PROTECTED.has(element.tagName)) {
        continue;
      }
      ids.push(id);
      if (this.hovered && element.contains(this.hovered)) {
        this.setHovered(null);
      }
      element.remove();
    }
    if (ids.length > 0) {
      this.events.onDelete(ids);
    }
  }

  dispose() {
    this.flushText();
    this.resizeObserver?.disconnect();
    for (const stop of this.cleanup) {
      stop();
    }
    this.cleanup.length = 0;
  }
}