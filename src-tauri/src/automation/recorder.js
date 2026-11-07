  if (window !== window.top) return;
  var TOKEN = "__CE_RECORD_TOKEN__";
  var KEY = "__ceRecording_" + TOKEN;
  if (window[KEY]) return;
  var bridge = window.chrome && window.chrome.webview;
  if (!bridge || !bridge.postMessage) return;
  var post = bridge.postMessage.bind(bridge);
  Object.defineProperty(window, KEY, { value: true });
  var L = __ceLocators;
  var stopped = false;
  var pending = null;
  var pendingTimer = 0;

  function send(step) {
    if (stopped) return;
    try { post({ type: "ce-record", token: TOKEN, step: step }); } catch (e) {}
  }

  function deep(event) {
    var path = event.composedPath ? event.composedPath() : [];
    return path.length && path[0].nodeType === 1 ? path[0] : event.target;
  }

  function actionable(node) {
    var el = node;
    while (el && el.nodeType === 1) {
      if (L.interactive(el) || L.pointerOnly(el)) return el;
      el = el.parentElement || (el.getRootNode && el.getRootNode().host) || null;
    }
    return node && node.nodeType === 1 ? node : null;
  }

  function textField(el) {
    var tag = el.tagName.toLowerCase();
    if (tag === "textarea") return true;
    if (el.isContentEditable) return true;
    if (tag !== "input") return false;
    var type = (el.getAttribute("type") || "text").toLowerCase();
    return ["text", "search", "email", "url", "tel", "password", "number", "date", "datetime-local", "month", "time", "week", ""].indexOf(type) >= 0;
  }

  function describe(el) {
    var loc = L.locatorsOf(el);
    var role = loc.role || loc.tag;
    loc.description = loc.name ? "the " + loc.name + " " + role : "the " + role;
    return loc;
  }

  function valueOf(el) {
    return el.isContentEditable ? el.innerText : el.value;
  }

  function flush() {
    clearTimeout(pendingTimer);
    if (!pending) return;
    var el = pending;
    pending = null;
    var isPassword = el.tagName.toLowerCase() === "input" && (el.getAttribute("type") || "").toLowerCase() === "password";
    var host = location.hostname.replace(/^www\./, "").replace(/[^a-z0-9.-]/gi, "");
    send({
      kind: "type",
      target: describe(el),
      text: isPassword ? "{{secret:" + (host || "site") + "-password}}" : String(valueOf(el) || ""),
      note: isPassword ? "Save the password in Settings, under Automation, as \"" + (host || "site") + "-password\"." : ""
    });
  }

  document.addEventListener("click", function (event) {
    if (!event.isTrusted || stopped) return;
    var el = actionable(deep(event));
    if (!el) return;
    if (textField(el)) return;
    var tag = el.tagName.toLowerCase();
    if (tag === "select" || tag === "option") return;
    if (tag === "input" && (el.getAttribute("type") || "").toLowerCase() === "file") return;
    flush();
    send({ kind: "click", target: describe(el) });
  }, true);

  document.addEventListener("input", function (event) {
    if (!event.isTrusted || stopped) return;
    var el = deep(event);
    if (!el || el.nodeType !== 1) return;
    var field = el.isContentEditable ? el.closest("[contenteditable]") || el : el;
    if (!textField(field)) return;
    if (pending && pending !== field) flush();
    pending = field;
    clearTimeout(pendingTimer);
    pendingTimer = setTimeout(flush, 1200);
  }, true);

  document.addEventListener("change", function (event) {
    if (!event.isTrusted || stopped) return;
    var el = deep(event);
    if (!el || el.nodeType !== 1) return;
    var tag = el.tagName.toLowerCase();
    if (tag === "select") {
      flush();
      var values = Array.prototype.filter.call(el.options, function (option) { return option.selected; })
        .map(function (option) { return (option.text || option.value).trim(); });
      send({ kind: "select", target: describe(el), values: values });
    } else if (tag === "input" && (el.getAttribute("type") || "").toLowerCase() === "file") {
      flush();
      var names = Array.prototype.map.call(el.files || [], function (file) { return file.name; });
      send({ kind: "upload", target: describe(el), values: names, note: "Replace these names with full file paths." });
    } else if (pending === el) {
      flush();
    }
  }, true);

  document.addEventListener("keydown", function (event) {
    if (!event.isTrusted || stopped) return;
    if (event.key !== "Enter" && event.key !== "Escape") return;
    var el = deep(event);
    if (event.key === "Enter" && el && el.tagName && el.tagName.toLowerCase() === "textarea" && !event.ctrlKey) return;
    flush();
    var keys = (event.ctrlKey ? "Control+" : "") + (event.shiftKey ? "Shift+" : "") + (event.altKey ? "Alt+" : "") + event.key;
    send({ kind: "press", keys: keys });
  }, true);

  window.addEventListener("pagehide", flush, true);
  document.addEventListener("visibilitychange", function () { if (document.hidden) flush(); }, true);
  window.addEventListener("ce-record-stop-" + TOKEN, function () {
    flush();
    stopped = true;
  });
