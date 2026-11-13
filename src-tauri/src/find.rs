use std::time::Duration;

use serde::Serialize;
use tauri::AppHandle;

use crate::{tabs, webview2};

const FAILURE: &str = "Couldn't search this page.";
const TIMEOUT: Duration = Duration::from_secs(3);

/// One match for the results panel: the words around it and where it sits on the page.
#[derive(Debug, Clone, Default, Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FindHit {
    pub index: u32,
    pub before: String,
    pub text: String,
    pub after: String,
    /// A heading, a line in an editor, or how far down the page the match is.
    pub location: String,
}

/// Text matches on one page. `index` is the highlighted match, or -1.
#[derive(Debug, Clone, Default, Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FindResult {
    pub count: u32,
    pub index: i32,
    /// Filled when the results panel is open. Empty for a count or a plain find.
    pub hits: Vec<FindHit>,
}

/// Matches in one tab for "Find in all tabs".
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FindTabResult {
    pub id: u32,
    pub title: String,
    pub url: String,
    /// `None` when the tab was not searched.
    pub count: Option<u32>,
    /// Why the tab was not searched, such as `Not loaded`.
    pub skipped: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Options {
    case_sensitive: bool,
    whole_word: bool,
    backwards: bool,
    /// Move to the next or previous match. Typing searches again from the top of the view.
    step: bool,
    /// Count only. The page is not highlighted or scrolled.
    count: bool,
    clear: bool,
    /// Jump to this match instead of stepping. Absent keeps the usual movement.
    #[serde(skip_serializing_if = "Option::is_none")]
    select: Option<i32>,
    /// Include `hits` so the results panel can list them.
    list: bool,
}

/// Searches text in the page, its open shadow trees, and same-origin frames. Monaco, CodeMirror,
/// and Ace keep the real document in the editor, so those matches come from the editor and the
/// editor scrolls to them. When the editors fill most of the window, the page around them is left
/// out. The current match is kept between calls so Enter moves through the list.
const FINDER_JS: &str = r#"(function (query, options) {
  var LIMIT = 2000;
  var EDITOR_SHARE = 0.4;
  var WORD_SEPARATORS = "`~!@#$%^&*()-=+[{]}\\|;:'\",.<>/?";
  var STYLE = "::highlight(ce-find){background-color:rgba(250,204,21,.45);color:inherit}::highlight(ce-find-current){background-color:#f97316;color:#111}.ce-find-inline{background-color:rgba(250,204,21,.45)!important}.ce-find-current-inline{background-color:#f97316!important;color:#111!important}.ace_marker-layer .ce-find-inline,.ace_marker-layer .ce-find-current-inline{position:absolute}";
  var state = window.__ceFind;
  if (!state) state = window.__ceFind = {};
  if (!state.roots) {
    state.matches = [];
    state.index = -1;
    state.key = "";
    state.dirty = true;
    state.mark = null;
    state.cleanups = [];
    state.styled = [];
    state.roots = new WeakSet();
    state.hostSeq = 0;
  }
  var word = /[\p{L}\p{N}_]/u;
  var needle = options.caseSensitive ? query : String(query).toLowerCase();
  var shownCache = new WeakMap();
  var handledCache = new WeakMap();

  function clearPaint() {
    var docs = state.styled ? state.styled.slice() : [];
    if (docs.indexOf(document) < 0) docs.push(document);
    for (var i = 0; i < docs.length; i++) {
      var css = docs[i].defaultView && docs[i].defaultView.CSS;
      if (css && css.highlights) {
        css.highlights.delete("ce-find");
        css.highlights.delete("ce-find-current");
      }
    }
    var jobs = state.cleanups || [];
    state.cleanups = [];
    for (var j = 0; j < jobs.length; j++) {
      try { jobs[j](); } catch (e) {}
    }
  }
  if (options.clear) {
    clearPaint();
    state.matches = [];
    state.index = -1;
    state.key = "";
    state.mark = null;
    state.dirty = true;
    return { count: 0, index: -1 };
  }

  function watch(root) {
    var target = root.nodeType === 9 ? root.documentElement : root;
    if (!target || state.roots.has(target)) return;
    state.roots.add(target);
    try {
      new MutationObserver(function () { state.dirty = true; }).observe(target, { subtree: true, childList: true, characterData: true });
    } catch (e) {}
  }
  function watchEditor(target, subscribe) {
    if (!target || state.roots.has(target)) return;
    state.roots.add(target);
    try { subscribe(function () { state.dirty = true; }); } catch (e) {}
  }
  function rememberDoc(doc) {
    if (state.styled.indexOf(doc) < 0) state.styled.push(doc);
  }
  function ensureStyle(doc) {
    if (!doc || !doc.documentElement || doc.getElementById("ce-find-style")) return;
    var style = doc.createElement("style");
    style.id = "ce-find-style";
    style.textContent = STYLE;
    (doc.head || doc.documentElement).appendChild(style);
  }
  function hostId(node, prefix) {
    if (!node.__ceFindId) {
      state.hostSeq += 1;
      node.__ceFindId = prefix + state.hostSeq;
    }
    return node.__ceFindId;
  }
  function shown(element) {
    if (shownCache.has(element)) return shownCache.get(element);
    var ok = true;
    try {
      if (element.checkVisibility) ok = element.checkVisibility({ contentVisibilityAuto: false, checkOpacity: false, checkVisibilityCSS: true });
    } catch (e) {}
    shownCache.set(element, ok);
    return ok;
  }
  function present(node) {
    if (!node || !node.isConnected || node.offsetWidth <= 0 || node.offsetHeight <= 0) return false;
    return shown(node);
  }
  function nodeContainsFocus(node) {
    if (!node) return false;
    var active = null;
    try { active = node.ownerDocument.activeElement; } catch (e) { return false; }
    if (!active || active === node.ownerDocument.body || active === node.ownerDocument.documentElement) return false;
    try { return node.contains(active); } catch (e) { return false; }
  }
  function insideHandled(element, handled) {
    if (handledCache.has(element)) return handledCache.get(element);
    var node = element;
    var chain = [];
    var inside = false;
    while (node && !handledCache.has(node)) {
      chain.push(node);
      if (handled.has(node)) { inside = true; break; }
      var root = node.getRootNode && node.getRootNode();
      node = node.parentElement || (root && root.host) || null;
    }
    if (node && handledCache.has(node)) inside = handledCache.get(node);
    for (var i = 0; i < chain.length; i++) handledCache.set(chain[i], inside);
    return inside;
  }
  function boundary(text, at, length) {
    if (!options.wholeWord) return true;
    var before = at > 0 ? text.charAt(at - 1) : "";
    var after = text.charAt(at + length);
    return (!before || !word.test(before)) && (!after || !word.test(after));
  }
  function metrics(node) {
    var rect = { left: 0, top: 0, right: 0, bottom: 0 };
    try { rect = node.getBoundingClientRect(); } catch (e) {}
    var x = rect.left;
    var y = rect.top;
    var right = rect.right;
    var bottom = rect.bottom;
    var win = null;
    try { win = node.ownerDocument.defaultView; } catch (e) {}
    var guard = 0;
    while (win && win !== window && guard < 8) {
      guard += 1;
      var frame = null;
      try { frame = win.frameElement; } catch (e) { break; }
      if (!frame) break;
      var frameRect = null;
      try { frameRect = frame.getBoundingClientRect(); } catch (e) { break; }
      x += frameRect.left;
      right += frameRect.left;
      y += frameRect.top;
      bottom += frameRect.top;
      try { win = frame.ownerDocument.defaultView; } catch (e) { break; }
    }
    var viewport = Math.max(0, window.innerWidth) * Math.max(0, window.innerHeight);
    var width = Math.max(0, Math.min(right, window.innerWidth) - Math.max(x, 0));
    var height = Math.max(0, Math.min(bottom, window.innerHeight) - Math.max(y, 0));
    return { area: width * height, viewport: viewport, top: y };
  }
  function trees(doc, visit) {
    var seen = new WeakSet();
    function walk(root) {
      if (!root || seen.has(root)) return;
      seen.add(root);
      watch(root);
      visit(root);
      var start = root.nodeType === 9 ? (root.body || root.documentElement) : root;
      if (!start) return;
      var walker = null;
      try { walker = doc.createTreeWalker(start, NodeFilter.SHOW_ELEMENT); } catch (e) { return; }
      if (start.shadowRoot) walk(start.shadowRoot);
      var node = walker.nextNode();
      while (node) {
        if (node.shadowRoot) walk(node.shadowRoot);
        node = walker.nextNode();
      }
    }
    walk(doc);
  }
  function documents() {
    var docs = [];
    var seen = new WeakSet();
    function add(doc) {
      if (!doc || seen.has(doc)) return;
      seen.add(doc);
      docs.push(doc);
      var frames = [];
      trees(doc, function (root) {
        if (!root.querySelectorAll) return;
        var nodes = null;
        try { nodes = root.querySelectorAll("iframe,frame"); } catch (e) { return; }
        for (var i = 0; i < nodes.length; i++) frames.push(nodes[i]);
      });
      for (var f = 0; f < frames.length; f++) {
        try { add(frames[f].contentDocument); } catch (e) {}
      }
    }
    add(document);
    return docs;
  }
  function pushTextMatches(into, text, meta) {
    var hay = options.caseSensitive ? text : text.toLowerCase();
    var at = hay.indexOf(needle);
    while (at !== -1 && into.length < LIMIT) {
      if (boundary(text, at, needle.length)) {
        var end = at + needle.length;
        into.push({
          kind: meta.kind,
          editorId: meta.editorId,
          key: meta.id + ":" + at,
          offset: at,
          end: end,
          top: meta.top,
          focused: meta.focused,
          fromCursor: !!meta.focused && (at >= meta.cursor || (at < meta.cursor && end > meta.cursor)),
          cm: meta.cm || null,
          view: meta.view || null,
          ace: meta.ace || null,
          session: meta.session || null
        });
      }
      at = hay.indexOf(needle, at + Math.max(1, needle.length));
    }
  }
  function addMonaco(win, into, metas, handled) {
    var monaco = win.monaco;
    if (!monaco || !monaco.editor || typeof monaco.editor.getEditors !== "function") return;
    var editors = null;
    try { editors = monaco.editor.getEditors() || []; } catch (e) { return; }
    var groups = [];
    for (var i = 0; i < editors.length; i++) takeMonaco(editors[i], groups);
    for (var g = 0; g < groups.length; g++) addMonacoGroup(groups[g], into, metas, handled);
  }
  function takeMonaco(editor, groups) {
    var model = null;
    var node = null;
    try {
      model = editor.getModel && editor.getModel();
      node = editor.getDomNode && editor.getDomNode();
    } catch (e) { return; }
    if (!model || typeof model.findMatches !== "function") return;
    var focused = false;
    try { focused = !!(editor.hasTextFocus && editor.hasTextFocus()); } catch (e) {}
    if (!focused) focused = nodeContainsFocus(node);
    if (node && !present(node) && !focused) return;
    var group = null;
    for (var g = 0; g < groups.length; g++) if (groups[g].model === model) group = groups[g];
    if (!group) {
      group = { model: model, editors: [], node: node, focused: false, reveal: editor };
      groups.push(group);
    }
    group.editors.push(editor);
    if (focused) {
      group.focused = true;
      group.reveal = editor;
      if (node) group.node = node;
    }
  }
  function addMonacoGroup(group, into, metas, handled) {
    var found = null;
    try {
      found = group.model.findMatches(query, false, false, !!options.caseSensitive, options.wholeWord ? WORD_SEPARATORS : null, false, LIMIT);
    } catch (e) { return; }
    if (!found || typeof found.length !== "number") return;
    var box = { area: 0, viewport: Math.max(0, window.innerWidth) * Math.max(0, window.innerHeight), top: 0 };
    var placed = false;
    for (var i = 0; i < group.editors.length; i++) {
      var dom = null;
      try { dom = group.editors[i].getDomNode && group.editors[i].getDomNode(); } catch (e) {}
      if (!dom) continue;
      handled.add(dom);
      var part = metrics(dom);
      box.area += part.area;
      if (part.viewport) box.viewport = part.viewport;
      if (!placed || dom === group.node) box.top = part.top;
      placed = true;
    }
    if (group.node) handled.add(group.node);
    var id = "monaco:" + monacoModelId(group.model);
    metas.push({ focused: group.focused, area: box.area, viewport: box.viewport });
    watchEditor(group.model, function (mark) {
      if (group.model.onDidChangeContent) group.model.onDidChangeContent(mark);
    });
    var cursor = 0;
    try {
      var position = group.reveal.getPosition && group.reveal.getPosition();
      if (position && group.model.getOffsetAt) cursor = group.model.getOffsetAt(position);
    } catch (e) {}
    for (var r = 0; r < found.length && into.length < LIMIT; r++) {
      var range = found[r].range;
      if (!range) continue;
      var start = r;
      var end = r + 1;
      try {
        if (group.model.getOffsetAt) {
          start = group.model.getOffsetAt({ lineNumber: range.startLineNumber, column: range.startColumn });
          end = group.model.getOffsetAt({ lineNumber: range.endLineNumber, column: range.endColumn });
        }
      } catch (e) {}
      into.push({
        kind: "monaco",
        editorId: id,
        key: id + ":" + start,
        offset: start,
        end: end,
        top: box.top,
        focused: group.focused,
        fromCursor: group.focused && (start >= cursor || (start < cursor && end > cursor)),
        editor: group.reveal,
        editors: group.editors,
        range: range
      });
    }
  }
  function monacoModelId(model) {
    try { if (model.uri && model.uri.toString) return model.uri.toString(); } catch (e) {}
    try { if (model.id != null) return String(model.id); } catch (e) {}
    return "model";
  }
  function addCm5(root, into, metas, handled) {
    if (!root.querySelectorAll) return;
    var nodes = null;
    try { nodes = root.querySelectorAll(".CodeMirror"); } catch (e) { return; }
    for (var i = 0; i < nodes.length; i++) addOneCm5(nodes[i], into, metas, handled);
  }
  function addOneCm5(node, into, metas, handled) {
    if (handled.has(node)) return;
    var cm = node.CodeMirror;
    if (!cm || typeof cm.getValue !== "function" || typeof cm.posFromIndex !== "function") return;
    var focused = nodeContainsFocus(node);
    if (!present(node) && !focused) return;
    var text = null;
    try { text = cm.getValue(); } catch (e) { return; }
    if (typeof text !== "string") return;
    handled.add(node);
    var box = metrics(node);
    var id = hostId(node, "cm5:");
    metas.push({ focused: focused, area: box.area, viewport: box.viewport });
    watchEditor(cm, function (mark) { if (cm.on) cm.on("change", mark); });
    var cursor = 0;
    try { if (cm.indexFromPos && cm.getCursor) cursor = cm.indexFromPos(cm.getCursor()); } catch (e) {}
    pushTextMatches(into, text, { kind: "cm5", editorId: id, id: id, top: box.top, focused: focused, cursor: cursor, cm: cm });
  }
  function addCm6(root, into, metas, handled) {
    if (!root.querySelectorAll) return;
    var nodes = null;
    try { nodes = root.querySelectorAll(".cm-editor"); } catch (e) { return; }
    for (var i = 0; i < nodes.length; i++) addOneCm6(nodes[i], into, metas, handled);
  }
  function addOneCm6(node, into, metas, handled) {
    if (handled.has(node)) return;
    var view = cm6View(node);
    if (!view) return;
    var focused = nodeContainsFocus(node);
    if (!present(node) && !focused) return;
    var text = null;
    try { text = view.state.doc.toString(); } catch (e) { return; }
    if (typeof text !== "string") return;
    handled.add(node);
    var box = metrics(node);
    var id = hostId(node, "cm6:");
    metas.push({ focused: focused, area: box.area, viewport: box.viewport });
    var cursor = 0;
    try {
      var selection = view.state.selection;
      var main = selection && (selection.main || (selection.ranges && selection.ranges[0]));
      if (main && typeof main.head === "number") cursor = main.head;
    } catch (e) {}
    pushTextMatches(into, text, { kind: "cm6", editorId: id, id: id, top: box.top, focused: focused, cursor: cursor, view: view });
  }
  function cm6View(host) {
    var content = host.classList && host.classList.contains("cm-content") ? host : (host.querySelector && host.querySelector(".cm-content"));
    var seeds = [];
    if (content && content.cmView) seeds.push(content.cmView);
    if (host.cmView) seeds.push(host.cmView);
    var scroller = host.querySelector && host.querySelector(".cm-scroller");
    if (scroller && scroller.cmView) seeds.push(scroller.cmView);
    for (var i = 0; i < seeds.length; i++) {
      var view = unwrapCm6(seeds[i]);
      if (view) return view;
    }
    return null;
  }
  function unwrapCm6(value) {
    var current = value;
    var guard = 0;
    while (current && guard < 6) {
      guard += 1;
      if (current.state && current.state.doc && typeof current.dispatch === "function" && typeof current.state.doc.toString === "function") return current;
      current = current.view || current.editorView || null;
    }
    return null;
  }
  function addAce(win, root, into, metas, handled) {
    if (!root.querySelectorAll) return;
    var nodes = null;
    try { nodes = root.querySelectorAll(".ace_editor"); } catch (e) { return; }
    for (var i = 0; i < nodes.length; i++) addOneAce(win, nodes[i], into, metas, handled);
  }
  function addOneAce(win, node, into, metas, handled) {
    if (handled.has(node)) return;
    var editor = node.env && node.env.editor;
    if (!editor) return;
    var session = editor.session || (editor.getSession && editor.getSession());
    if (!session || typeof session.getValue !== "function") return;
    var focused = nodeContainsFocus(node);
    try { if (!focused && editor.isFocused) focused = !!editor.isFocused(); } catch (e) {}
    if (!present(node) && !focused) return;
    var text = null;
    try { text = session.getValue(); } catch (e) { return; }
    if (typeof text !== "string") return;
    handled.add(node);
    var box = metrics(node);
    var id = hostId(node, "ace:");
    metas.push({ focused: focused, area: box.area, viewport: box.viewport });
    watchEditor(session, function (mark) { if (session.on) session.on("change", mark); });
    var cursor = 0;
    try {
      var doc = session.getDocument ? session.getDocument() : session.doc;
      var lead = editor.getCursorPosition && editor.getCursorPosition();
      if (doc && lead && doc.positionToIndex) cursor = doc.positionToIndex(lead);
    } catch (e) {}
    pushTextMatches(into, text, { kind: "ace", editorId: id, id: id, top: box.top, focused: focused, cursor: cursor, ace: editor, session: session, win: win });
  }
  function collectText(doc, root, into, handled) {
    if (!query || into.length >= LIMIT) return;
    var start = root.nodeType === 9 ? (root.body || root.documentElement) : root;
    if (!start) return;
    var walker = null;
    try {
      walker = doc.createTreeWalker(start, NodeFilter.SHOW_TEXT, {
        acceptNode: function (node) {
          var parent = node.parentElement;
          if (!parent) return NodeFilter.FILTER_REJECT;
          var tag = parent.tagName;
          if (tag === "SCRIPT" || tag === "STYLE" || tag === "NOSCRIPT" || tag === "TEMPLATE") return NodeFilter.FILTER_REJECT;
          if (insideHandled(parent, handled)) return NodeFilter.FILTER_REJECT;
          return shown(parent) ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT;
        }
      });
    } catch (e) { return; }
    var node = walker.nextNode();
    while (node && into.length < LIMIT) {
      var text = node.nodeValue || "";
      var hay = options.caseSensitive ? text : text.toLowerCase();
      var at = hay.indexOf(needle);
      while (at !== -1 && into.length < LIMIT) {
        if (boundary(text, at, needle.length)) {
          try {
            var range = doc.createRange();
            range.setStart(node, at);
            range.setEnd(node, at + needle.length);
            into.push({ kind: "dom", node: node, offset: at, range: range, top: 0, focused: false, fromCursor: false });
          } catch (e) {}
        }
        at = hay.indexOf(needle, at + Math.max(1, needle.length));
      }
      node = walker.nextNode();
    }
  }
  function editorsFill(metas) {
    var area = 0;
    var viewport = 0;
    var focusedArea = 0;
    for (var i = 0; i < metas.length; i++) {
      area += metas[i].area;
      if (metas[i].viewport > viewport) viewport = metas[i].viewport;
      if (metas[i].focused) focusedArea += metas[i].area;
    }
    if (viewport <= 0) return false;
    // The editors are the page, or the focused one is more than a small field.
    return area >= viewport * EDITOR_SHARE || focusedArea >= viewport * 0.2;
  }
  function collect() {
    if (!query) return { matches: [], editorOnly: false };
    var docs = documents();
    var editorMatches = [];
    var metas = [];
    var handled = new Set();
    for (var i = 0; i < docs.length; i++) {
      var doc = docs[i];
      var win = doc.defaultView;
      if (!win) continue;
      try { addMonaco(win, editorMatches, metas, handled); } catch (e) {}
      trees(doc, function (root) {
        var frameWin = doc.defaultView;
        try { addCm5(root, editorMatches, metas, handled); } catch (e) {}
        try { addCm6(root, editorMatches, metas, handled); } catch (e) {}
        try { addAce(frameWin, root, editorMatches, metas, handled); } catch (e) {}
      });
    }
    var editorOnly = editorsFill(metas);
    var domMatches = [];
    if (!editorOnly) {
      for (var d = 0; d < docs.length; d++) {
        trees(docs[d], function (root) { collectText(docs[d], root, domMatches, handled); });
      }
    }
    return { matches: orderMatches(editorMatches, domMatches, editorOnly), editorOnly: editorOnly };
  }
  function orderMatches(editorMatches, domMatches, editorOnly) {
    var focused = [];
    var rest = [];
    for (var i = 0; i < editorMatches.length; i++) {
      if (editorMatches[i].focused) focused.push(editorMatches[i]);
      else rest.push(editorMatches[i]);
    }
    focused.sort(function (a, b) { return a.offset - b.offset; });
    rest.sort(function (a, b) { return a.top - b.top || a.offset - b.offset; });
    var matches = editorOnly ? focused.concat(rest) : focused.concat(domMatches, rest);
    return matches.length > LIMIT ? matches.slice(0, LIMIT) : matches;
  }
  function initialIndex(matches) {
    for (var i = 0; i < matches.length; i++) if (matches[i].fromCursor) return i;
    for (var f = 0; f < matches.length; f++) if (matches[f].focused) return f;
    for (var j = 0; j < matches.length; j++) {
      if (matches[j].kind !== "dom") continue;
      try {
        if (matches[j].range.getBoundingClientRect().bottom >= 0) return j;
      } catch (e) {}
    }
    return 0;
  }
  function sameMark(match, mark) {
    if (!mark || match.kind !== mark.kind) return false;
    if (match.kind === "dom") return match.node === mark.node && match.offset === mark.offset;
    return match.key === mark.key;
  }
  function markOf(match) {
    if (match.kind === "dom") return { kind: "dom", node: match.node, offset: match.offset };
    return { kind: match.kind, key: match.key };
  }
  function revealElement(element) {
    if (!element || !element.scrollIntoView) return;
    var saved = [];
    var node = element;
    var guard = 0;
    while (node && guard < 80) {
      guard += 1;
      if (node.style) {
        saved.push(node, node.style.scrollBehavior);
        node.style.scrollBehavior = "auto";
      }
      var next = node.parentElement;
      if (!next && node.getRootNode) {
        var root = node.getRootNode();
        if (root && root.host) next = root.host;
      }
      node = next;
    }
    try { element.scrollIntoView({ block: "center", inline: "nearest", behavior: "instant" }); } catch (e) {}
    var doc = element.ownerDocument;
    var frames = 0;
    while (doc && doc !== document && frames < 8) {
      frames += 1;
      var frame = null;
      try { frame = doc.defaultView && doc.defaultView.frameElement; } catch (e) { break; }
      if (!frame) break;
      try { frame.scrollIntoView({ block: "center", inline: "nearest", behavior: "instant" }); } catch (e) {}
      doc = frame.ownerDocument;
    }
    for (var n = 0; n < saved.length; n += 2) saved[n].style.scrollBehavior = saved[n + 1];
  }
  function revealMatch(match) {
    try {
      if (match.kind === "dom") revealElement(match.node.parentElement || match.node.parentNode);
      else if (match.kind === "monaco") {
        try { match.editor.setSelection(match.range); } catch (e) {}
        try { match.editor.revealRangeInCenter(match.range, 1); } catch (e) {
          try { match.editor.revealRangeInCenter(match.range); } catch (err) {}
        }
      } else if (match.kind === "cm5") {
        var from = match.cm.posFromIndex(match.offset);
        var to = match.cm.posFromIndex(match.end);
        if (match.cm.setSelection) match.cm.setSelection(from, to);
        if (match.cm.scrollIntoView) match.cm.scrollIntoView({ from: from, to: to }, 80);
      } else if (match.kind === "cm6") {
        match.view.dispatch({ selection: { anchor: match.offset, head: match.end }, scrollIntoView: true });
      } else if (match.kind === "ace") {
        var placed = aceEnds(match);
        if (!placed) return;
        if (placed.range && match.ace.selection && match.ace.selection.setRange) match.ace.selection.setRange(placed.range);
        if (placed.range && match.ace.revealRange) match.ace.revealRange(placed.range);
        else if (match.ace.scrollToLine) match.ace.scrollToLine(placed.start.row, true, true);
      }
    } catch (e) {}
  }
  function aceEnds(match) {
    var session = match.session;
    var doc = session && (session.getDocument ? session.getDocument() : session.doc);
    if (!doc || !doc.indexToPosition) return null;
    var start = doc.indexToPosition(match.offset);
    var end = doc.indexToPosition(match.end);
    var win = match.ace.container && match.ace.container.ownerDocument.defaultView;
    return { start: start, end: end, range: aceRange(win, start, end) };
  }
  function aceRange(win, start, end) {
    var Ctor = null;
    try {
      if (win && win.ace && typeof win.ace.require === "function") Ctor = win.ace.require("ace/range").Range;
    } catch (e) {}
    if (!Ctor && win && win.ace && win.ace.Range) Ctor = win.ace.Range;
    if (!Ctor) return null;
    return new Ctor(start.row, start.column, end.row, end.column);
  }
  function paint(matches, current) {
    var groups = [];
    function rangesFor(doc) {
      for (var i = 0; i < groups.length; i++) if (groups[i].doc === doc) return groups[i];
      var created = { doc: doc, ranges: [], current: null };
      groups.push(created);
      return created;
    }
    for (var i = 0; i < matches.length; i++) {
      if (matches[i].kind !== "dom") continue;
      var owner = matches[i].node.ownerDocument;
      if (!owner) continue;
      var group = rangesFor(owner);
      group.ranges.push(matches[i].range);
      if (matches[i] === current) group.current = matches[i].range;
    }
    var editorIds = [];
    var editorGroups = [];
    for (var m = 0; m < matches.length; m++) {
      if (matches[m].kind === "dom") continue;
      var index = editorIds.indexOf(matches[m].editorId);
      if (index < 0) {
        editorIds.push(matches[m].editorId);
        editorGroups.push([matches[m]]);
      } else editorGroups[index].push(matches[m]);
    }
    for (var g = 0; g < editorGroups.length; g++) paintEditor(editorGroups[g], current, rangesFor);
    for (var n = 0; n < groups.length; n++) flushHighlights(groups[n]);
  }
  function paintEditor(list, current, rangesFor) {
    try {
      var kind = list[0].kind;
      if (kind === "monaco") paintMonaco(list, current);
      else if (kind === "cm5") paintCm5(list, current);
      else if (kind === "cm6") paintCm6(list, current, rangesFor);
      else if (kind === "ace") paintAce(list, current);
    } catch (e) {}
  }
  function decorateMonaco(editor, specs) {
    if (typeof editor.createDecorationsCollection === "function") {
      var collection = editor.createDecorationsCollection(specs);
      state.cleanups.push(function () { collection.clear(); });
      return;
    }
    if (typeof editor.deltaDecorations !== "function") return;
    var ids = editor.deltaDecorations(editor.__ceFindIds || [], specs);
    editor.__ceFindIds = ids;
    state.cleanups.push(function () {
      editor.__ceFindIds = editor.deltaDecorations(editor.__ceFindIds || [], []);
    });
  }
  function paintMonaco(list, current) {
    var editors = list[0].editors || [list[0].editor];
    var specs = [];
    for (var i = 0; i < list.length; i++) {
      specs.push({
        range: list[i].range,
        options: { inlineClassName: list[i] === current ? "ce-find-current-inline" : "ce-find-inline" }
      });
    }
    for (var e = 0; e < editors.length; e++) {
      var editor = editors[e];
      try {
        var node = editor.getDomNode && editor.getDomNode();
        if (node) ensureStyle(node.ownerDocument);
      } catch (err) {}
      try { decorateMonaco(editor, specs); } catch (err) {}
    }
  }
  function rememberMark(mark) {
    state.cleanups.push(function () { mark.clear(); });
  }
  function paintCm5(list, current) {
    var cm = list[0].cm;
    if (!cm || !cm.markText) return;
    var wrapper = cm.getWrapperElement && cm.getWrapperElement();
    if (wrapper) ensureStyle(wrapper.ownerDocument);
    for (var i = 0; i < list.length; i++) {
      var from = cm.posFromIndex(list[i].offset);
      var to = cm.posFromIndex(list[i].end);
      rememberMark(cm.markText(from, to, { className: list[i] === current ? "ce-find-current-inline" : "ce-find-inline" }));
    }
  }
  function paintCm6(list, current, rangesFor) {
    var view = list[0].view;
    if (!view || !view.domAtPos) return;
    var doc = view.dom.ownerDocument;
    var viewport = view.viewport || { from: 0, to: 0 };
    var group = rangesFor(doc);
    for (var i = 0; i < list.length; i++) {
      if (list[i].end < viewport.from || list[i].offset > viewport.to) continue;
      var range = cm6Range(view, doc, list[i].offset, list[i].end);
      if (!range) continue;
      group.ranges.push(range);
      if (list[i] === current) group.current = range;
    }
  }
  function cm6Range(view, doc, from, to) {
    try {
      var start = view.domAtPos(from);
      var end = view.domAtPos(to);
      if (!start || !end || !start.node || !end.node) return null;
      var range = doc.createRange();
      range.setStart(start.node, start.offset);
      range.setEnd(end.node, end.offset);
      return range;
    } catch (e) { return null; }
  }
  function paintAce(list, current) {
    for (var i = 0; i < list.length; i++) {
      var placed = aceEnds(list[i]);
      if (!placed || !placed.range || !list[i].session.addMarker) continue;
      var container = list[i].ace.container;
      if (container) ensureStyle(container.ownerDocument);
      rememberAce(list[i].session, placed.range, list[i] === current);
    }
  }
  function rememberAce(session, range, current) {
    var id = session.addMarker(range, current ? "ce-find-current-inline" : "ce-find-inline", "text");
    state.cleanups.push(function () { session.removeMarker(id); });
  }
  function flushHighlights(group) {
    var view = group.doc.defaultView;
    var Highlight = view && view.Highlight;
    var css = view && view.CSS;
    if (!Highlight || !css || !css.highlights || !group.ranges.length) return;
    ensureStyle(group.doc);
    rememberDoc(group.doc);
    var all = new Highlight();
    if (typeof all.add === "function") {
      for (var i = 0; i < group.ranges.length; i++) all.add(group.ranges[i]);
    } else all = new Highlight(...group.ranges);
    css.highlights.set("ce-find", all);
    if (group.current) {
      var one = new Highlight();
      if (typeof one.add === "function") one.add(group.current);
      else one = new Highlight(group.current);
      css.highlights.set("ce-find-current", one);
    } else css.highlights.delete("ce-find-current");
  }

  function clip(value, max) {
    var text = String(value || "").replace(/\s+/g, " ").trim();
    if (text.length <= max) return text;
    return text.slice(0, max - 1) + "…";
  }
  function joinPlace(first, second, third) {
    var parts = [];
    if (first) parts.push(first);
    if (second) parts.push(second);
    if (third) parts.push(third);
    return parts.join(" · ");
  }
  function windowText(text, at, length) {
    var raw = String(text || "");
    var start = Math.max(0, Math.min(at, raw.length));
    var end = Math.min(raw.length, start + Math.max(0, length));
    var from = Math.max(0, start - 48);
    var to = Math.min(raw.length, end + 48);
    var before = raw.slice(from, start).replace(/\s+/g, " ");
    var mid = raw.slice(start, end).replace(/\s+/g, " ");
    var after = raw.slice(end, to).replace(/\s+/g, " ");
    if (from > 0) before = "…" + before.replace(/^\s+/, "");
    if (to < raw.length) after = after.replace(/\s+$/, "") + "…";
    return { before: before, text: mid || query, after: after };
  }
  function fileTail(uri) {
    var text = String(uri || "");
    var cut = Math.max(text.lastIndexOf("/"), text.lastIndexOf("\\"));
    return cut >= 0 ? text.slice(cut + 1) : text;
  }
  function domSnippet(match) {
    var node = match.node;
    var own = node.nodeValue || "";
    var before = "";
    var after = "";
    var prev = node.previousSibling;
    if (prev) before = prev.nodeType === 3 ? (prev.nodeValue || "") : (prev.textContent || "");
    var next = node.nextSibling;
    if (next) after = next.nodeType === 3 ? (next.nodeValue || "") : (next.textContent || "");
    if (!before && node.parentElement && node.parentElement.previousElementSibling) before = node.parentElement.previousElementSibling.textContent || "";
    if (!after && node.parentElement && node.parentElement.nextElementSibling) after = node.parentElement.nextElementSibling.textContent || "";
    if (before && !/\s$/.test(before)) before += " ";
    if (after && !/^\s/.test(after)) after = " " + after;
    var length = match.range ? match.range.toString().length : query.length;
    return windowText(before + own + after, before.length + match.offset, length);
  }
  function domPlace(match) {
    var element = match.node.parentElement;
    if (!element) return "On this page";
    var doc = element.ownerDocument;
    var heading = "";
    var node = element;
    var guard = 0;
    while (node && guard < 40) {
      guard += 1;
      if (node.tagName && /^H[1-6]$/.test(node.tagName)) {
        heading = clip(node.textContent, 80);
        break;
      }
      var root = node.getRootNode && node.getRootNode();
      node = node.parentElement || (root && root.host) || null;
    }
    if (!heading && doc && doc.querySelectorAll) {
      var headings = [];
      try { headings = doc.querySelectorAll("h1,h2,h3,h4,h5,h6"); } catch (e) { headings = []; }
      for (var i = 0; i < headings.length; i++) {
        if (!headings[i].compareDocumentPosition) break;
        var pos = headings[i].compareDocumentPosition(element);
        if ((pos & Node.DOCUMENT_POSITION_FOLLOWING) || headings[i].contains(element)) {
          var label = clip(headings[i].textContent, 80);
          if (label) heading = label;
        }
      }
    }
    var percent = "";
    try {
      var view = doc.defaultView;
      var rect = element.getBoundingClientRect();
      var top = rect.top + ((view && (view.pageYOffset || view.scrollY)) || (doc.documentElement && doc.documentElement.scrollTop) || 0);
      var height = Math.max((doc.documentElement && doc.documentElement.scrollHeight) || 0, (view && view.innerHeight) || 1);
      percent = Math.max(0, Math.min(100, Math.round((top / height) * 100))) + "% down the page";
    } catch (e) {}
    var frame = "";
    try {
      var frameEl = doc.defaultView && doc.defaultView.frameElement;
      if (frameEl && doc !== document) frame = clip(frameEl.getAttribute("title") || frameEl.getAttribute("aria-label") || frameEl.id, 40) || "Embedded frame";
    } catch (e) { frame = "Embedded frame"; }
    return joinPlace(frame, heading, percent) || "On this page";
  }
  function editorLine(text, offset, length) {
    var raw = String(text || "");
    var line = raw.slice(0, offset).split("\n").length;
    var lineStart = raw.lastIndexOf("\n", Math.max(0, offset - 1)) + 1;
    var lineEnd = raw.indexOf("\n", offset);
    if (lineEnd < 0 || lineEnd < offset) lineEnd = raw.length;
    return { line: line, snippet: windowText(raw.slice(lineStart, lineEnd), offset - lineStart, length) };
  }
  function describeHit(match) {
    if (match.kind === "dom") return Object.assign(domSnippet(match), { location: domPlace(match) });
    if (match.kind === "monaco") {
      var model = match.editor && match.editor.getModel && match.editor.getModel();
      var line = match.range ? match.range.startLineNumber : 1;
      var lineText = "";
      var column = match.range ? match.range.startColumn - 1 : 0;
      var length = query.length;
      if (model && match.range && match.range.startLineNumber !== match.range.endLineNumber && model.getValueInRange) {
        lineText = model.getValueInRange(match.range);
        column = 0;
        length = lineText.length;
      } else if (model && model.getLineContent) {
        lineText = model.getLineContent(line);
        length = match.range ? match.range.endColumn - match.range.startColumn : query.length;
      }
      var name = "";
      try { if (model && model.uri && model.uri.toString) name = fileTail(model.uri.toString()); } catch (e) {}
      return Object.assign(windowText(lineText, column, length), { location: joinPlace(name, "Line " + line, "") || ("Line " + line) });
    }
    if (match.kind === "cm5") {
      var from = match.cm.posFromIndex(match.offset);
      var cmLine = match.cm.getLine(from.line);
      return Object.assign(windowText(cmLine, from.ch, Math.max(0, match.end - match.offset)), { location: "Line " + (from.line + 1) });
    }
    if (match.kind === "cm6") {
      var info = editorLine(match.view.state.doc.toString(), match.offset, Math.max(0, match.end - match.offset));
      return Object.assign(info.snippet, { location: "Line " + info.line });
    }
    if (match.kind === "ace") {
      var session = match.session;
      var aceDoc = session && (session.getDocument ? session.getDocument() : session.doc);
      var acePos = aceDoc && aceDoc.indexToPosition ? aceDoc.indexToPosition(match.offset) : { row: 0, column: 0 };
      var aceLine = session && session.getLine ? session.getLine(acePos.row) : "";
      return Object.assign(windowText(aceLine, acePos.column, Math.max(0, match.end - match.offset)), { location: "Line " + (acePos.row + 1) });
    }
    return { before: "", text: query, after: "", location: "On this page" };
  }
  function listHits(matches, index) {
    var span = 250;
    var start = 0;
    if (matches.length > span) start = Math.max(0, Math.min(index - 30, matches.length - span));
    var end = Math.min(matches.length, start + span);
    var hits = [];
    for (var i = start; i < end; i++) {
      var hit = { before: "", text: query, after: "", location: "On this page" };
      try { hit = describeHit(matches[i]); } catch (e) {}
      hit.index = i;
      hits.push(hit);
    }
    return hits;
  }

  if (options.count) return { count: collect().matches.length, index: -1 };
  var key = JSON.stringify([query, !!options.caseSensitive, !!options.wholeWord]);
  var sameQuery = key === state.key;
  if (!sameQuery || state.dirty) {
    var collected = collect();
    state.matches = collected.matches;
    state.dirty = false;
    state.key = key;
    var kept = -1;
    if (sameQuery && state.mark) {
      for (var k = 0; k < state.matches.length; k++) {
        if (sameMark(state.matches[k], state.mark)) { kept = k; break; }
      }
    }
    state.index = kept;
  }
  var count = state.matches.length;
  if (count === 0) {
    clearPaint();
    state.index = -1;
    state.mark = null;
    var empty = { count: 0, index: -1 };
    if (options.list) empty.hits = [];
    return empty;
  }
  if (typeof options.select === "number" && options.select >= 0) {
    state.index = Math.min(Math.floor(options.select), count - 1);
  } else if (state.index < 0 || !sameQuery) {
    state.index = initialIndex(state.matches);
    if (options.step && options.backwards) state.index = (state.index - 1 + count) % count;
  } else if (options.step) {
    state.index = (state.index + (options.backwards ? -1 : 1) + count) % count;
  }
  var current = state.matches[state.index];
  state.mark = markOf(current);
  clearPaint();
  revealMatch(current);
  paint(state.matches, current);
  var payload = { count: count, index: state.index };
  if (options.list) payload.hits = listHits(state.matches, state.index);
  return payload;
})"#;

fn run(app: &AppHandle, id: u32, query: &str, options: &Options) -> Result<FindResult, String> {
    let webview = tabs::page_view(app, id).ok_or("That tab has no page loaded.")?;
    let query = serde_json::to_string(query).map_err(|error| error.to_string())?;
    let options = serde_json::to_string(options).map_err(|error| error.to_string())?;
    let value = webview2::evaluate(
        &webview,
        &format!("{FINDER_JS}({query}, {options})"),
        TIMEOUT,
        FAILURE,
    )?;
    Ok(serde_json::from_value(value).unwrap_or_default())
}

pub fn find(
    app: &AppHandle,
    id: u32,
    query: &str,
    backwards: bool,
    case_sensitive: bool,
    whole_word: bool,
    step: bool,
    select: Option<i32>,
    list: bool,
) -> Result<FindResult, String> {
    let options = Options {
        case_sensitive,
        whole_word,
        backwards,
        step,
        count: false,
        clear: query.is_empty(),
        select,
        list,
    };
    run(app, id, query, &options)
}

pub fn clear(app: &AppHandle, id: u32) {
    let options = Options {
        case_sensitive: false,
        whole_word: false,
        backwards: false,
        step: false,
        count: false,
        clear: true,
        select: None,
        list: false,
    };
    let _ = run(app, id, "", &options);
}

/// Counts matches in every tab of the window's current workspace. Tabs that have not loaded or
/// are asleep are listed as skipped, so searching does not wake them.
pub fn find_in_tabs(
    app: &AppHandle,
    window: &str,
    query: &str,
    case_sensitive: bool,
    whole_word: bool,
) -> Vec<FindTabResult> {
    let options = Options {
        case_sensitive,
        whole_word,
        backwards: false,
        step: false,
        count: true,
        clear: false,
        select: None,
        list: false,
    };
    tabs::strip_tabs(app, window)
        .into_iter()
        .map(|tab| {
            let skipped = if !tab.loaded {
                "Not loaded"
            } else if tab.sleeping {
                "Asleep"
            } else if tab.failed {
                "Didn't load"
            } else {
                ""
            };
            let count = if skipped.is_empty() {
                run(app, tab.id, query, &options)
                    .ok()
                    .map(|result| result.count)
            } else {
                None
            };
            FindTabResult {
                id: tab.id,
                title: tab.title,
                url: tab.url,
                skipped: if skipped.is_empty() && count.is_none() {
                    "Couldn't search".into()
                } else {
                    skipped.into()
                },
                count,
            }
        })
        .collect()
}
