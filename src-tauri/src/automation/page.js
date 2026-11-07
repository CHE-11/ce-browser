(function () {
  if (globalThis.__ce) return;
  var L = __ceLocators;
  var refs = new Map();
  var ids = new WeakMap();
  var next = 1;
  var lastShown = [];
  var MARK_ID = "ce-automation-marks";
  var FLASH_ID = "ce-automation-flash";

  function refOf(el) {
    var id = ids.get(el);
    if (!id) {
      id = next++;
      ids.set(el, id);
      refs.set(id, new WeakRef(el));
    }
    return id;
  }

  function el(id) {
    var held = refs.get(Number(id));
    var found = held && held.deref();
    if (!found || !found.isConnected) {
      throw new Error("Element " + id + " is no longer on the page. Take a new snapshot.");
    }
    return found;
  }

  /** Offset of a same-origin frame's viewport inside the top viewport. */
  function frameOffset(node) {
    var x = 0;
    var y = 0;
    var view = node.ownerDocument && node.ownerDocument.defaultView;
    while (view && view.frameElement) {
      var frame = view.frameElement;
      var rect = frame.getBoundingClientRect();
      var style = L.styleOf(frame);
      x += rect.left + (style ? parseFloat(style.borderLeftWidth) + parseFloat(style.paddingLeft) : 0);
      y += rect.top + (style ? parseFloat(style.borderTopWidth) + parseFloat(style.paddingTop) : 0);
      view = view.parent;
    }
    return { x: x, y: y };
  }

  function topRect(node) {
    var rect = node.getBoundingClientRect();
    var offset = frameOffset(node);
    return { x: rect.left + offset.x, y: rect.top + offset.y, width: rect.width, height: rect.height };
  }

  function inView(rect) {
    return rect.y + rect.height > 0 && rect.y < innerHeight && rect.x + rect.width > 0 && rect.x < innerWidth;
  }

  function describeValue(node) {
    var tag = node.tagName.toLowerCase();
    var parts = [];
    if (tag === "input") {
      var type = (node.getAttribute("type") || "text").toLowerCase();
      if (type !== "text") parts.push("type=" + type);
      if (type === "checkbox" || type === "radio") {
        parts.push(node.checked ? "checked" : "unchecked");
      } else if (type === "password") {
        if (node.value) parts.push("value=\"\u2022\u2022\u2022\"");
      } else if (type !== "submit" && type !== "button" && type !== "file" && node.value) {
        parts.push("value=\"" + L.clip(node.value, 60) + "\"");
      }
      var auto = node.getAttribute("autocomplete");
      if (auto && /cc-|password|one-time-code/.test(auto)) parts.push("autocomplete=" + auto);
    } else if (tag === "textarea" && node.value) {
      parts.push("value=\"" + L.clip(node.value, 60) + "\"");
    } else if (tag === "select") {
      var chosen = Array.prototype.filter.call(node.options, function (option) { return option.selected; });
      if (chosen.length) parts.push("selected=\"" + L.clip(chosen.map(function (option) { return option.text; }).join(", "), 60) + "\"");
      var options = Array.prototype.slice.call(node.options, 0, 12).map(function (option) { return L.clip(option.text, 30); });
      parts.push("options=[" + options.join(" | ") + (node.options.length > 12 ? " | \u2026" : "") + "]");
    } else if (tag === "a") {
      var href = node.getAttribute("href") || "";
      if (href && href.charAt(0) !== "#" && !/^javascript:/i.test(href)) parts.push("href=" + L.clip(href, 80));
    }
    if (node.getAttribute("aria-expanded")) parts.push("expanded=" + node.getAttribute("aria-expanded"));
    if (node.getAttribute("aria-checked")) parts.push("checked=" + node.getAttribute("aria-checked"));
    if (node.getAttribute("aria-selected") === "true") parts.push("selected");
    if (node.disabled || node.getAttribute("aria-disabled") === "true") parts.push("disabled");
    return parts.join(" ");
  }

  function deepActive() {
    var active = document.activeElement;
    while (active) {
      if (active.shadowRoot && active.shadowRoot.activeElement) {
        active = active.shadowRoot.activeElement;
        continue;
      }
      if ((active.tagName === "IFRAME" || active.tagName === "FRAME")) {
        try {
          var inner = active.contentDocument && active.contentDocument.activeElement;
          if (inner) { active = inner; continue; }
        } catch (e) {}
      }
      break;
    }
    return active;
  }

  function snapshot(options) {
    options = options || {};
    var limit = options.limit || 350;
    var lines = [];
    var shown = [];
    var offscreen = 0;
    var total = 0;
    var roots = L.allRoots(document);
    for (var r = 0; r < roots.length; r++) {
      var root = roots[r];
      var walker = (root.ownerDocument || root).createTreeWalker(root, 1);
      var node = walker.nextNode();
      var skipInside = null;
      while (node) {
        if (skipInside && !skipInside.contains(node)) skipInside = null;
        var tag = node.tagName.toLowerCase();
        if (tag === "script" || tag === "style" || tag === "noscript" || tag === "template" || node.id === MARK_ID) {
          node = nextOutside(walker, node);
          continue;
        }
        var heading = /^h[1-4]$/.test(tag);
        var dialog = node.getAttribute("role") === "dialog" || node.getAttribute("role") === "alertdialog" || tag === "dialog";
        var control = !skipInside && (L.interactive(node) || L.pointerOnly(node));
        if ((heading || dialog || control) && L.visible(node)) {
          var rect = topRect(node);
          var visibleNow = inView(rect);
          if (control) {
            total++;
            if (!visibleNow) offscreen++;
            if (shown.length < limit && (visibleNow || offscreen <= limit / 3)) {
              var id = refOf(node);
              var name = L.nameOf(node);
              var extra = describeValue(node);
              lines.push("[" + id + "] " + L.roleOf(node) + (name ? " \"" + name.replace(/"/g, "'") + "\"" : "") + (extra ? " " + extra : "") + (visibleNow ? "" : " (offscreen)"));
              shown.push({ ref: id, x: rect.x, y: rect.y, width: rect.width, height: rect.height, visible: visibleNow });
            }
            if (tag !== "label" && tag !== "summary") skipInside = node;
          } else if (heading) {
            var text = L.clip(node.innerText || node.textContent, 100);
            if (text) lines.push("#".repeat(Number(tag.charAt(1))) + " " + text);
          } else if (dialog) {
            lines.push("-- dialog \"" + L.clip(L.nameOf(node), 60) + "\" is open --");
          }
        }
        node = walker.nextNode();
      }
    }
    var framed = frames();
    if (framed.text) lines.push(framed.text);
    lastShown = shown;
    var active = deepActive();
    var focused = active && active !== document.body && ids.get(active) ? ids.get(active) : null;
    var scroller = document.scrollingElement || document.documentElement;
    return {
      url: location.href,
      title: document.title,
      width: innerWidth,
      height: innerHeight,
      scrollY: Math.round(scroller.scrollTop),
      scrollHeight: Math.round(scroller.scrollHeight),
      focused: focused,
      count: shown.length,
      total: total,
      text: lines.join("\n")
    };
  }

  function frames() {
    var lines = [];
    var boxes = [];
    function walk(doc) {
      var list;
      try { list = doc.querySelectorAll("iframe, frame"); } catch (e) { return; }
      for (var i = 0; i < list.length; i++) {
        var frame = list[i];
        if (!L.visible(frame)) continue;
        var rect = topRect(frame);
        if (rect.width < 8 || rect.height < 8) continue;
        var src = frame.getAttribute("src") || "";
        var host = "";
        try { host = new URL(src, location.href).hostname.replace(/^www\./, ""); } catch (e) {}
        var open = false;
        try { open = !!(frame.contentDocument && frame.contentDocument.documentElement); } catch (e) {}
        var box = Math.round(rect.x) + "," + Math.round(rect.y) + " " + Math.round(rect.width) + "x" + Math.round(rect.height);
        if (open) {
          lines.push("-- iframe " + (host || "this page") + " box " + box + " (elements inside are listed) --");
          walk(frame.contentDocument);
        } else {
          lines.push("-- iframe " + (host || "another site") + " box " + box + " (no access; click a point inside this box) --");
          boxes.push({ host: host, x: rect.x, y: rect.y, width: rect.width, height: rect.height });
        }
      }
    }
    walk(document);
    return { text: lines.join("\n"), boxes: boxes };
  }

  function nextOutside(walker, node) {
    var sibling = walker.nextSibling();
    if (sibling) return sibling;
    var parent = walker.parentNode();
    while (parent) {
      sibling = walker.nextSibling();
      if (sibling) return sibling;
      parent = walker.parentNode();
    }
    return null;
  }

  function targetOf(node) {
    if (node.tagName === "INPUT" && node.labels && node.labels.length) {
      var rect = node.getBoundingClientRect();
      if (rect.width < 2 || rect.height < 2 || !L.visible(node)) return node.labels[0];
    }
    return node;
  }

  function describeNode(node) {
    if (!node || !node.tagName) return "";
    return L.roleOf(node) + " \"" + L.clip(L.nameOf(node), 60) + "\"";
  }

  /** Scrolls the element into view and returns the point to click in top-viewport CSS pixels. */
  function point(id) {
    var node = targetOf(el(id));
    node.scrollIntoView({ block: "center", inline: "center", behavior: "instant" });
    var rect = node.getBoundingClientRect();
    var offset = frameOffset(node);
    var cx = rect.left + rect.width / 2;
    var cy = rect.top + rect.height / 2;
    var root = node.getRootNode();
    var hit = root.elementFromPoint ? root.elementFromPoint(cx, cy) : node.ownerDocument.elementFromPoint(cx, cy);
    var ok = !!hit && (hit === node || node.contains(hit) || hit.contains(node));
    return {
      x: offset.x + cx,
      y: offset.y + cy,
      width: rect.width,
      height: rect.height,
      ok: ok,
      cover: ok ? "" : describeNode(hit)
    };
  }

  function info(id) {
    var node = el(id);
    return infoOf(node);
  }

  function infoOf(node) {
    if (!node || !node.tagName) return null;
    var form = node.form || (node.closest && node.closest("form"));
    var submits = [];
    var payment = false;
    if (form) {
      var buttons = form.querySelectorAll("button, input[type=submit], input[type=image]");
      for (var i = 0; i < buttons.length && i < 6; i++) submits.push(L.nameOf(buttons[i]));
      payment = !!form.querySelector("[autocomplete^=cc-], input[name*=card i], input[name*=cvv i], input[name*=cvc i]");
    }
    var href = node.closest && node.closest("a[href]");
    return {
      tag: node.tagName.toLowerCase(),
      role: L.roleOf(node),
      name: L.nameOf(node),
      type: (node.getAttribute("type") || "").toLowerCase(),
      autocomplete: node.getAttribute("autocomplete") || "",
      href: href ? href.href : "",
      submits: submits,
      payment: payment,
      editable: node.isContentEditable || node.tagName === "TEXTAREA" || (node.tagName === "INPUT" && !/^(checkbox|radio|submit|button|file|image|range|color|reset)$/i.test(node.getAttribute("type") || "")),
      file: node.tagName === "INPUT" && (node.getAttribute("type") || "").toLowerCase() === "file"
    };
  }

  function focused() {
    var active = deepActive();
    if (!active || active === document.body) return null;
    var result = infoOf(active);
    if (result) result.ref = refOf(active);
    return result;
  }

  /** Focuses the field and selects what it holds, so typed text replaces it. */
  function selectContents(id) {
    var node = el(id);
    node.focus();
    if (typeof node.select === "function") {
      node.select();
      return true;
    }
    if (node.isContentEditable) {
      var range = node.ownerDocument.createRange();
      range.selectNodeContents(node);
      var selection = node.ownerDocument.defaultView.getSelection();
      selection.removeAllRanges();
      selection.addRange(range);
      return true;
    }
    return false;
  }

  function focus(id) {
    var node = el(id);
    node.focus();
    return true;
  }

  function choose(id, values) {
    var node = el(id);
    if (node.tagName !== "SELECT") {
      throw new Error("That element is not a list box. Click it, then click an option.");
    }
    var wanted = values.map(function (value) { return String(value).trim().toLowerCase(); });
    var picked = [];
    for (var i = 0; i < node.options.length; i++) {
      var option = node.options[i];
      var match = wanted.indexOf(option.value.toLowerCase()) >= 0 || wanted.indexOf(option.text.trim().toLowerCase()) >= 0;
      if (match) picked.push(option.text.trim());
      if (node.multiple) option.selected = match;
      else if (match && picked.length === 1) node.selectedIndex = i;
    }
    if (!picked.length) throw new Error("None of those options are in the list.");
    node.dispatchEvent(new Event("input", { bubbles: true }));
    node.dispatchEvent(new Event("change", { bubbles: true }));
    return picked;
  }

  function textOf(max, links) {
    max = max || 20000;
    var out = [];
    var size = 0;
    var BLOCK = /^(p|div|section|article|li|ul|ol|tr|table|h[1-6]|header|footer|main|nav|aside|br|dd|dt|blockquote|pre|form|fieldset)$/;
    function walk(node) {
      if (size > max) return;
      if (node.nodeType === 3) {
        var text = node.nodeValue.replace(/\s+/g, " ");
        if (text.trim()) { out.push(text); size += text.length; }
        return;
      }
      if (node.nodeType !== 1) return;
      var tag = node.tagName.toLowerCase();
      if (tag === "script" || tag === "style" || tag === "noscript" || tag === "svg" || node.id === MARK_ID) return;
      var style = L.styleOf(node);
      if (style && (style.display === "none" || style.visibility === "hidden")) return;
      var block = BLOCK.test(tag);
      if (block) out.push("\n");
      if (node.shadowRoot) walk(node.shadowRoot);
      var kids = node.childNodes;
      for (var i = 0; i < kids.length; i++) walk(kids[i]);
      if (links && tag === "a" && node.href && !/^javascript:/i.test(node.getAttribute("href") || "")) {
        out.push(" <" + node.href + ">");
      }
      if (tag === "img" && links && node.alt) out.push(" [image: " + L.clip(node.alt, 60) + "]");
      if (block) out.push("\n");
    }
    if (document.body) walk(document.body);
    var text = out.join("").replace(/[ \t]+\n/g, "\n").replace(/\n{3,}/g, "\n\n").trim();
    return text.length > max ? text.slice(0, max) + "\n\u2026" : text;
  }

  function find(query) {
    if (query.selector) {
      var list = L.queryAll(query.selector).filter(L.visible);
      if (!list.length) return false;
    }
    if (query.text) {
      var body = document.body ? document.body.innerText.toLowerCase() : "";
      if (body.indexOf(String(query.text).toLowerCase()) < 0) return false;
    }
    if (query.gone) {
      var gone = document.body ? document.body.innerText.toLowerCase() : "";
      if (gone.indexOf(String(query.gone).toLowerCase()) >= 0) return false;
    }
    return true;
  }

  function idle() {
    return {
      ready: document.readyState,
      resources: performance.getEntriesByType("resource").length
    };
  }

  function marks(on) {
    var old = document.getElementById(MARK_ID);
    if (old) old.remove();
    if (!on) return 0;
    var layer = document.createElement("div");
    layer.id = MARK_ID;
    layer.style.cssText = "position:fixed;inset:0;pointer-events:none;z-index:2147483647;";
    var shadow = layer.attachShadow({ mode: "closed" });
    var count = 0;
    var colors = ["#e11d48", "#2563eb", "#16a34a", "#d97706", "#7c3aed", "#0891b2"];
    for (var i = 0; i < lastShown.length; i++) {
      var item = lastShown[i];
      if (!item.visible) continue;
      var color = colors[item.ref % colors.length];
      var box = document.createElement("div");
      box.style.cssText = "position:fixed;box-sizing:border-box;border:2px solid " + color + ";border-radius:3px;left:" + item.x + "px;top:" + item.y + "px;width:" + item.width + "px;height:" + item.height + "px;";
      var tag = document.createElement("span");
      tag.textContent = String(item.ref);
      tag.style.cssText = "position:absolute;left:-2px;top:-16px;background:" + color + ";color:#fff;font:bold 11px/14px Segoe UI,Arial,sans-serif;padding:0 3px;border-radius:2px;";
      box.appendChild(tag);
      shadow.appendChild(box);
      count++;
    }
    (document.body || document.documentElement).appendChild(layer);
    return count;
  }

  function locate(target) {
    var found = L.locate(target);
    if (!found) return null;
    return { ref: refOf(found.el), by: found.by };
  }

  function locatorsOf(id) {
    return L.locatorsOf(el(id));
  }

  function rectOf(id) {
    var node = el(id);
    node.scrollIntoView({ block: "center", inline: "center", behavior: "instant" });
    return topRect(node);
  }

  function nextLink(text) {
    var words = text ? [String(text).toLowerCase()] : ["next", "next page", "\u203a", "\u00bb", ">", "more results", "load more", "show more"];
    var rel = L.queryAll("a[rel~=next], link[rel~=next]").filter(function (node) { return node.tagName === "A" && L.visible(node); });
    if (rel.length) return refOf(rel[0]);
    var all = L.queryAll("a, button, [role=button], [role=link]").filter(L.visible);
    for (var w = 0; w < words.length; w++) {
      for (var i = 0; i < all.length; i++) {
        var node = all[i];
        if (node.disabled || node.getAttribute("aria-disabled") === "true") continue;
        var name = L.nameOf(node).toLowerCase();
        var label = (node.getAttribute("aria-label") || "").toLowerCase();
        if (name === words[w] || label === words[w] || label.indexOf(words[w] + " page") === 0) return refOf(node);
      }
    }
    return null;
  }

  // Extraction

  function signature(node) {
    var classes = Array.prototype.filter.call(node.classList || [], function (name) {
      return /^[a-zA-Z_-][\w-]*$/.test(name) && !/\d{3,}/.test(name) && !/(active|selected|hover|focus|odd|even|first|last)/i.test(name);
    }).slice(0, 2);
    return node.tagName.toLowerCase() + classes.map(function (name) { return "." + (typeof CSS !== "undefined" && CSS.escape ? CSS.escape(name) : name); }).join("");
  }

  function siblingsLike(node) {
    var parent = node.parentElement;
    if (!parent) return [];
    var sig = signature(node);
    return Array.prototype.filter.call(parent.children, function (child) { return signature(child) === sig; });
  }

  function itemOf(node) {
    var current = node;
    var best = null;
    while (current && current.parentElement && current !== document.body) {
      var like = siblingsLike(current);
      if (like.length >= 3) { best = current; break; }
      current = current.parentElement;
    }
    return best || node;
  }

  function pathWithin(node, item) {
    if (node === item) return "";
    var parts = [];
    var current = node;
    while (current && current !== item) {
      var parent = current.parentElement;
      if (!parent) break;
      var tag = current.tagName.toLowerCase();
      var same = Array.prototype.filter.call(parent.children, function (child) { return child.tagName === current.tagName; });
      parts.unshift(same.length > 1 ? tag + ":nth-of-type(" + (same.indexOf(current) + 1) + ")" : tag);
      current = parent;
    }
    return ":scope > " + parts.join(" > ");
  }

  var PRICE = /([$\u20ac\u00a3\u00a5\u20b9]\s?\d[\d.,]*|\d[\d.,]*\s?(usd|eur|gbp|\u20ac))/i;

  function fieldsOf(item) {
    var fields = [];
    var used = {};
    function add(name, selector, attr) {
      var base = name;
      var n = 2;
      while (used[name]) name = base + n++;
      used[name] = true;
      fields.push({ name: name, selector: selector, attr: attr });
    }
    var heading = item.querySelector("h1, h2, h3, h4, h5, h6, [role=heading]");
    if (heading) add("title", pathWithin(heading, item), "text");
    var link = item.tagName === "A" ? item : item.querySelector("a[href]");
    if (link) {
      if (!heading) add("title", pathWithin(link, item), "text");
      add("link", pathWithin(link, item), "href");
    }
    var image = item.querySelector("img");
    if (image) add("image", pathWithin(image, item), "src");
    var walker = document.createTreeWalker(item, 1);
    var node = walker.nextNode();
    var texts = 0;
    while (node && fields.length < 10) {
      var own = Array.prototype.some.call(node.childNodes, function (child) { return child.nodeType === 3 && child.nodeValue.trim(); });
      if (own && node !== heading && (!heading || !heading.contains(node)) && (!link || node !== link) && L.visible(node)) {
        var text = L.clip(node.innerText || node.textContent, 200);
        if (text) {
          if (PRICE.test(text) && !used.price) add("price", pathWithin(node, item), "text");
          else if (texts < 5) { texts++; add("text", pathWithin(node, item), "text"); }
        }
      }
      node = walker.nextNode();
    }
    if (!fields.length) add("text", "", "text");
    return fields;
  }

  function valueOf(node, attr) {
    if (!node) return "";
    if (attr === "href") return node.href || node.getAttribute("href") || "";
    if (attr === "src") return node.currentSrc || node.src || node.getAttribute("src") || "";
    if (attr === "value") return node.value || "";
    if (attr && attr !== "text") return node.getAttribute(attr) || "";
    return L.clip(node.innerText || node.textContent || "", 500);
  }

  function extract(spec) {
    var items = L.queryAll(spec.item);
    var rows = [];
    for (var i = 0; i < items.length && rows.length < 2000; i++) {
      var row = {};
      var filled = false;
      for (var f = 0; f < spec.fields.length; f++) {
        var field = spec.fields[f];
        var node = null;
        try { node = field.selector ? items[i].querySelector(field.selector) : items[i]; } catch (e) { node = null; }
        var value = valueOf(node, field.attr);
        if (value) filled = true;
        row[field.name] = value;
      }
      if (filled) rows.push(row);
    }
    return rows;
  }

  function specFor(node) {
    var item = itemOf(node);
    var parent = item.parentElement;
    var selector = (parent ? L.cssPath(parent) + " > " : "") + signature(item);
    return { item: selector, fields: fieldsOf(item) };
  }

  var picking = null;

  /** The nearest element a click on `node` acts on, the same way the recorder picks it. */
  function actionable(node) {
    var current = node;
    while (current && current.nodeType === 1) {
      if (L.interactive(current) || L.pointerOnly(current)) return current;
      current = current.parentElement || (current.getRootNode && current.getRootNode().host) || null;
    }
    return node && node.nodeType === 1 ? node : null;
  }

  function targetFor(node) {
    var loc = L.locatorsOf(node);
    var role = loc.role || loc.tag;
    loc.description = loc.name ? "the " + loc.name + " " + role : "the " + role;
    return loc;
  }

  function pick(timeoutMs) {
    return pickWith(timeoutMs, false);
  }

  /** Lets the user click one element. Resolves with its ref and locators, or null. */
  function pickElement(timeoutMs) {
    return pickWith(timeoutMs, true);
  }

  function pickWith(timeoutMs, single) {
    if (picking) picking.cancel();
    return new Promise(function (resolve) {
      var layer = document.createElement("div");
      layer.id = MARK_ID;
      layer.style.cssText = "position:fixed;inset:0;pointer-events:none;z-index:2147483647;";
      var shadow = layer.attachShadow({ mode: "closed" });
      var box = document.createElement("div");
      box.style.cssText = "position:fixed;box-sizing:border-box;border:2px solid #0ea5e9;background:rgba(14,165,233,.12);border-radius:3px;display:none;";
      var group = document.createElement("div");
      var hint = document.createElement("div");
      hint.textContent = single ? "Click the element this step should use. Esc cancels." : "Click an item in the list to extract. Esc cancels.";
      hint.style.cssText = "position:fixed;left:50%;top:12px;transform:translateX(-50%);background:#0f172a;color:#e2e8f0;font:13px Segoe UI,Arial,sans-serif;padding:6px 10px;border-radius:6px;box-shadow:0 2px 10px rgba(0,0,0,.4);";
      shadow.appendChild(group);
      shadow.appendChild(box);
      shadow.appendChild(hint);
      (document.body || document.documentElement).appendChild(layer);
      var timer = setTimeout(function () { finish(null); }, timeoutMs || 120000);
      function outline(node) {
        if (single) node = actionable(node) || node;
        var rect = node.getBoundingClientRect();
        box.style.display = "block";
        box.style.left = rect.left + "px";
        box.style.top = rect.top + "px";
        box.style.width = rect.width + "px";
        box.style.height = rect.height + "px";
        group.textContent = "";
        if (single) return;
        var like = siblingsLike(itemOf(node));
        for (var i = 0; i < like.length && i < 200; i++) {
          var r = like[i].getBoundingClientRect();
          var ghost = document.createElement("div");
          ghost.style.cssText = "position:fixed;box-sizing:border-box;border:1px dashed #f59e0b;left:" + r.left + "px;top:" + r.top + "px;width:" + r.width + "px;height:" + r.height + "px;";
          group.appendChild(ghost);
        }
      }
      function move(event) {
        var node = event.composedPath ? event.composedPath()[0] : event.target;
        if (node && node.nodeType === 1) outline(node);
      }
      function click(event) {
        event.preventDefault();
        event.stopPropagation();
        event.stopImmediatePropagation();
        var node = event.composedPath ? event.composedPath()[0] : event.target;
        if (!node || node.nodeType !== 1) return;
        if (single) {
          var chosen = actionable(node) || node;
          finish({ ref: refOf(chosen), target: targetFor(chosen) });
          return;
        }
        var spec = specFor(node);
        finish({ spec: spec, preview: extract(spec).slice(0, 5) });
      }
      function key(event) {
        if (event.key === "Escape") {
          event.preventDefault();
          finish(null);
        }
      }
      function block(event) {
        event.preventDefault();
        event.stopPropagation();
      }
      function finish(result) {
        clearTimeout(timer);
        document.removeEventListener("mousemove", move, true);
        document.removeEventListener("click", click, true);
        document.removeEventListener("mousedown", block, true);
        document.removeEventListener("mouseup", block, true);
        document.removeEventListener("keydown", key, true);
        layer.remove();
        picking = null;
        resolve(result);
      }
      document.addEventListener("mousemove", move, true);
      document.addEventListener("click", click, true);
      document.addEventListener("mousedown", block, true);
      document.addEventListener("mouseup", block, true);
      document.addEventListener("keydown", key, true);
      picking = { cancel: function () { finish(null); } };
    });
  }

  function scrollBy(dx, dy) {
    var scroller = document.scrollingElement || document.documentElement;
    var before = scroller.scrollTop;
    window.scrollBy(dx, dy);
    return { moved: Math.abs(scroller.scrollTop - before) > 1, scrollY: scroller.scrollTop, scrollHeight: scroller.scrollHeight };
  }

  function element(id) {
    return el(id);
  }

  /** Outlines an element for a moment so the user can see what a locator matched. */
  function flash(id) {
    var node = el(id);
    node.scrollIntoView({ block: "center", inline: "center", behavior: "instant" });
    var rect = topRect(node);
    var old = document.getElementById(FLASH_ID);
    if (old) old.remove();
    var layer = document.createElement("div");
    layer.id = FLASH_ID;
    layer.style.cssText = "position:fixed;inset:0;pointer-events:none;z-index:2147483647;";
    var shadow = layer.attachShadow({ mode: "closed" });
    var box = document.createElement("div");
    box.style.cssText = "position:fixed;box-sizing:border-box;border:3px solid #22c55e;background:rgba(34,197,94,.15);border-radius:4px;transition:opacity .4s;left:" + rect.x + "px;top:" + rect.y + "px;width:" + rect.width + "px;height:" + rect.height + "px;";
    shadow.appendChild(box);
    (document.body || document.documentElement).appendChild(layer);
    setTimeout(function () { box.style.opacity = "0"; }, 1200);
    setTimeout(function () { layer.remove(); }, 1700);
    return true;
  }

  globalThis.__ce = {
    snapshot: snapshot,
    point: point,
    info: info,
    focused: focused,
    selectContents: selectContents,
    focus: focus,
    choose: choose,
    text: textOf,
    find: find,
    idle: idle,
    marks: marks,
    locate: locate,
    locatorsOf: locatorsOf,
    rectOf: rectOf,
    nextLink: nextLink,
    extract: extract,
    pick: pick,
    pickElement: pickElement,
    flash: flash,
    scrollBy: scrollBy,
    element: element,
    frames: frames
  };
})();
