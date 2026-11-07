var __ceLocators = (function () {
  var INTERACTIVE_ROLES = {
    button: 1, link: 1, checkbox: 1, radio: 1, tab: 1, menuitem: 1, menuitemcheckbox: 1,
    menuitemradio: 1, option: 1, switch: 1, combobox: 1, textbox: 1, searchbox: 1, slider: 1,
    spinbutton: 1, treeitem: 1, gridcell: 1, listbox: 1
  };
  var TEST_ATTRS = ["data-testid", "data-test", "data-test-id", "data-qa", "data-cy"];

  function clip(text, max) {
    text = String(text == null ? "" : text).replace(/\s+/g, " ").trim();
    return text.length > max ? text.slice(0, max - 1) + "\u2026" : text;
  }

  function styleOf(el) {
    var view = (el.ownerDocument && el.ownerDocument.defaultView) || window;
    try { return view.getComputedStyle(el); } catch (e) { return null; }
  }

  function roleOf(el) {
    var explicit = el.getAttribute && el.getAttribute("role");
    if (explicit) return explicit.split(/\s+/)[0].toLowerCase();
    var tag = el.tagName ? el.tagName.toLowerCase() : "";
    if (tag === "a" && el.hasAttribute("href")) return "link";
    if (tag === "button" || tag === "summary") return "button";
    if (tag === "select") return el.multiple ? "listbox" : "combobox";
    if (tag === "textarea") return "textbox";
    if (tag === "option") return "option";
    if (tag === "img") return "img";
    if (/^h[1-6]$/.test(tag)) return "heading";
    if (tag === "input") {
      var type = (el.getAttribute("type") || "text").toLowerCase();
      if (type === "checkbox") return "checkbox";
      if (type === "radio") return "radio";
      if (type === "range") return "slider";
      if (type === "number") return "spinbutton";
      if (type === "search") return "searchbox";
      if (type === "button" || type === "submit" || type === "reset" || type === "image") return "button";
      if (type === "file") return "button";
      return "textbox";
    }
    if (el.isContentEditable) return "textbox";
    return tag;
  }

  function labelText(el) {
    var doc = el.ownerDocument;
    var by = el.getAttribute && el.getAttribute("aria-labelledby");
    if (by && doc) {
      var parts = by.split(/\s+/).map(function (id) {
        var node = doc.getElementById(id);
        return node ? node.textContent : "";
      });
      var joined = clip(parts.join(" "), 120);
      if (joined) return joined;
    }
    var aria = el.getAttribute && el.getAttribute("aria-label");
    if (aria && aria.trim()) return clip(aria, 120);
    if (el.labels && el.labels.length) {
      var label = clip(el.labels[0].innerText || el.labels[0].textContent, 120);
      if (label) return label;
    }
    return "";
  }

  function nameOf(el) {
    var label = labelText(el);
    if (label) return label;
    var tag = el.tagName ? el.tagName.toLowerCase() : "";
    if (tag === "img") return clip(el.getAttribute("alt") || el.getAttribute("title") || "", 120);
    if (tag === "input") {
      var type = (el.getAttribute("type") || "text").toLowerCase();
      if (type === "submit" || type === "button" || type === "reset") return clip(el.value || type, 120);
      if (type === "image") return clip(el.getAttribute("alt") || "image", 120);
      return clip(el.getAttribute("placeholder") || el.getAttribute("title") || el.getAttribute("name") || "", 120);
    }
    if (tag === "textarea" || tag === "select") {
      return clip(el.getAttribute("placeholder") || el.getAttribute("title") || el.getAttribute("name") || "", 120);
    }
    var text = clip(el.innerText || el.textContent || "", 120);
    if (text) return text;
    var title = el.getAttribute && el.getAttribute("title");
    if (title) return clip(title, 120);
    var img = el.querySelector && el.querySelector("img[alt]");
    if (img) return clip(img.getAttribute("alt"), 120);
    var svgTitle = el.querySelector && el.querySelector("svg title");
    if (svgTitle) return clip(svgTitle.textContent, 120);
    return "";
  }

  function visible(el) {
    if (!el || !el.getBoundingClientRect) return false;
    var rect = el.getBoundingClientRect();
    if (rect.width < 1 || rect.height < 1) return false;
    var style = styleOf(el);
    if (!style) return true;
    if (style.visibility === "hidden" || style.display === "none") return false;
    if (Number(style.opacity) === 0) return false;
    return true;
  }

  function interactive(el) {
    var tag = el.tagName ? el.tagName.toLowerCase() : "";
    if (tag === "a" && el.hasAttribute("href")) return true;
    if (tag === "button" || tag === "select" || tag === "textarea" || tag === "summary") return true;
    if (tag === "input") return (el.getAttribute("type") || "").toLowerCase() !== "hidden";
    if (el.isContentEditable && (!el.parentElement || !el.parentElement.isContentEditable)) return true;
    var role = el.getAttribute("role");
    if (role && INTERACTIVE_ROLES[role.toLowerCase()]) return true;
    if (el.hasAttribute("onclick")) return true;
    var tabindex = el.getAttribute("tabindex");
    if (tabindex !== null && Number(tabindex) >= 0 && tag !== "div" && tag !== "span") return true;
    if (tabindex !== null && Number(tabindex) >= 0) {
      var style = styleOf(el);
      return !!style && style.cursor === "pointer";
    }
    return false;
  }

  function pointerOnly(el) {
    var tag = el.tagName ? el.tagName.toLowerCase() : "";
    if (tag !== "div" && tag !== "span" && tag !== "li" && tag !== "img" && tag !== "svg" && tag !== "label" && tag !== "td") return false;
    var style = styleOf(el);
    if (!style || style.cursor !== "pointer") return false;
    var parent = el.parentElement;
    var parentStyle = parent && styleOf(parent);
    return !parentStyle || parentStyle.cursor !== "pointer";
  }

  function cssEscape(value) {
    if (typeof CSS !== "undefined" && CSS.escape) return CSS.escape(value);
    return String(value).replace(/[^a-zA-Z0-9_-]/g, function (c) { return "\\" + c; });
  }

  function stableId(id) {
    return id && !/\d{3,}/.test(id) && !/^[0-9]/.test(id) && !/[:]/.test(id) && id.length < 60;
  }

  function cssPath(el) {
    var parts = [];
    var node = el;
    while (node && node.nodeType === 1 && parts.length < 12) {
      var tag = node.tagName.toLowerCase();
      if (node.id && stableId(node.id)) {
        parts.unshift("#" + cssEscape(node.id));
        break;
      }
      var parent = node.parentElement;
      if (!parent) {
        parts.unshift(tag);
        break;
      }
      var same = Array.prototype.filter.call(parent.children, function (child) { return child.tagName === node.tagName; });
      parts.unshift(same.length > 1 ? tag + ":nth-of-type(" + (same.indexOf(node) + 1) + ")" : tag);
      node = parent;
    }
    return parts.join(" > ");
  }

  function xpathOf(el) {
    var parts = [];
    var node = el;
    while (node && node.nodeType === 1) {
      var index = 1;
      var sibling = node.previousElementSibling;
      while (sibling) {
        if (sibling.tagName === node.tagName) index++;
        sibling = sibling.previousElementSibling;
      }
      parts.unshift(node.tagName.toLowerCase() + "[" + index + "]");
      node = node.parentElement;
    }
    return "/" + parts.join("/");
  }

  function testIdOf(el) {
    for (var i = 0; i < TEST_ATTRS.length; i++) {
      var value = el.getAttribute && el.getAttribute(TEST_ATTRS[i]);
      if (value) return TEST_ATTRS[i] + "=" + value;
    }
    return "";
  }

  function textOf(el) {
    try {
      return el.innerText || el.textContent || "";
    } catch (e) {
      return el.textContent || "";
    }
  }

  function locatorsOf(el) {
    var tag = el.tagName ? el.tagName.toLowerCase() : "";
    var css = "";
    if (el.id && stableId(el.id)) css = "#" + cssEscape(el.id);
    else if (el.getAttribute && el.getAttribute("name") && (tag === "input" || tag === "select" || tag === "textarea")) {
      css = tag + "[name=\"" + el.getAttribute("name").replace(/"/g, "\\\"") + "\"]";
    }
    return {
      role: roleOf(el),
      name: nameOf(el),
      text: clip(textOf(el), 80),
      testId: testIdOf(el),
      css: css || cssPath(el),
      xpath: xpathOf(el),
      tag: tag,
      frame: frameOf(el)
    };
  }

  function frameOf(el) {
    try {
      var view = el.ownerDocument && el.ownerDocument.defaultView;
      if (!view || view === view.top) return "";
      return view.location.origin || "";
    } catch (e) {
      return "";
    }
  }

  function allRoots(doc) {
    var roots = [doc];
    var stack = [doc];
    while (stack.length) {
      var root = stack.pop();
      var walker = (root.ownerDocument || root).createTreeWalker(root, 1);
      var node = walker.currentNode;
      while (node) {
        if (node.shadowRoot) {
          roots.push(node.shadowRoot);
          stack.push(node.shadowRoot);
        }
        if (node.tagName === "IFRAME" || node.tagName === "FRAME") {
          try {
            var inner = node.contentDocument;
            if (inner && inner.documentElement) {
              roots.push(inner);
              stack.push(inner);
            }
          } catch (e) {}
        }
        node = walker.nextNode();
      }
    }
    return roots;
  }

  function queryAll(selector) {
    var found = [];
    var roots = allRoots(document);
    for (var i = 0; i < roots.length; i++) {
      try {
        var list = roots[i].querySelectorAll(selector);
        for (var j = 0; j < list.length; j++) found.push(list[j]);
      } catch (e) {}
    }
    return found;
  }

  function byXpath(path) {
    try {
      var result = document.evaluate(path, document, null, XPathResult.ORDERED_NODE_SNAPSHOT_TYPE, null);
      var out = [];
      for (var i = 0; i < result.snapshotLength; i++) out.push(result.snapshotItem(i));
      return out;
    } catch (e) {
      return [];
    }
  }

  function same(a, b) {
    return clip(a, 80).toLowerCase() === clip(b, 80).toLowerCase();
  }

  /** The first locator that names exactly one visible element. */
  function locate(target) {
    var tries = [];
    if (target.testId) {
      var at = target.testId.indexOf("=");
      if (at > 0) {
        tries.push(["testId", function () {
          return queryAll("[" + target.testId.slice(0, at) + "=\"" + target.testId.slice(at + 1).replace(/"/g, "\\\"") + "\"]");
        }]);
      }
    }
    if (target.css && target.css.charAt(0) === "#") tries.push(["css", function () { return queryAll(target.css); }]);
    if (target.role && target.name) {
      tries.push(["role", function () {
        return candidates().filter(function (el) { return roleOf(el) === target.role && same(nameOf(el), target.name); });
      }]);
    }
    if (target.css && target.css.charAt(0) !== "#") tries.push(["css", function () { return queryAll(target.css); }]);
    if (target.text) {
      tries.push(["text", function () {
        return candidates().filter(function (el) { return same(el.innerText || el.textContent || "", target.text); });
      }]);
    }
    if (target.xpath) tries.push(["xpath", function () { return byXpath(target.xpath); }]);
    for (var i = 0; i < tries.length; i++) {
      var list = tries[i][1]().filter(visible);
      if (target.tag) {
        var tagged = list.filter(function (el) { return el.tagName.toLowerCase() === target.tag; });
        if (tagged.length) list = tagged;
      }
      if (target.frame) {
        list = list.filter(function (el) { return frameOf(el) === target.frame; });
      } else {
        var topOnly = list.filter(function (el) { return !frameOf(el); });
        if (topOnly.length) list = topOnly;
      }
      if (list.length === 1) return { el: list[0], by: tries[i][0] };
      if (list.length > 1 && tries[i][0] === "role") {
        return { el: list[0], by: "role" };
      }
    }
    return null;
  }

  function candidates() {
    var out = [];
    var roots = allRoots(document);
    for (var i = 0; i < roots.length; i++) {
      var root = roots[i];
      var walker = (root.ownerDocument || root).createTreeWalker(root, 1);
      var node = walker.nextNode();
      while (node) {
        if (interactive(node) || pointerOnly(node)) out.push(node);
        node = walker.nextNode();
      }
    }
    return out;
  }

  return {
    clip: clip,
    roleOf: roleOf,
    nameOf: nameOf,
    visible: visible,
    interactive: interactive,
    pointerOnly: pointerOnly,
    locatorsOf: locatorsOf,
    locate: locate,
    allRoots: allRoots,
    queryAll: queryAll,
    cssPath: cssPath,
    styleOf: styleOf
  };
})();
