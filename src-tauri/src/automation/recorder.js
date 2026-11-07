  if (window !== window.top) return;
  var TOKEN = "__CE_RECORD_TOKEN__";
  var KEY = "__ceRecording_" + TOKEN;
  if (window[KEY]) return;

  function bridge() {
    var host = window.__ceHost || (window.chrome && window.chrome.webview);
    return host && host.postMessage ? host : null;
  }

  function boot() {
    if (window[KEY]) return true;
    var host = bridge();
    if (!host) return false;
    var L = __ceLocators;
    if (!L) return false;
    Object.defineProperty(window, KEY, { value: true });
    var stopped = false;
    var pending = null;
    var pendingTimer = 0;
    var pressed = null;
    var recentClick = null;

    function send(step) {
      if (stopped) return;
      try { host.postMessage({ type: "ce-record", token: TOKEN, step: step }); } catch (e) {}
    }

    function flash(el) {
      if (!el || el.nodeType !== 1) return;
      var outline = el.style.outline;
      var offset = el.style.outlineOffset;
      el.style.outline = "2px solid #fb7185";
      el.style.outlineOffset = "2px";
      setTimeout(function () {
        if (!el.isConnected) return;
        el.style.outline = outline;
        el.style.outlineOffset = offset;
      }, 700);
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

    function safeDescribe(el) {
      try { return describe(el); } catch (e) {
        var tag = el && el.tagName ? el.tagName.toLowerCase() : "element";
        return { tag: tag, role: "", name: "", text: "", testId: "", css: "", xpath: "", description: "the " + tag };
      }
    }

    function skipClick(el) {
      if (!el || el.nodeType !== 1) return true;
      if (textField(el)) return true;
      var tag = el.tagName.toLowerCase();
      if (tag === "select" || tag === "option") return true;
      if (tag === "input" && (el.getAttribute("type") || "").toLowerCase() === "file") return true;
      return false;
    }

    // Mouse clicks and keyboard clicks (Enter or Space on a control) both land here.
    // The click event often never follows a pointerup, so the pointerup is what gets recorded.
    function trackClick(el) {
      if (skipClick(el)) return;
      var now = Date.now();
      if (recentClick && recentClick.el === el && now - recentClick.at < 700) return;
      recentClick = { el: el, at: now };
      flush();
      flash(el);
      send({ kind: "click", target: safeDescribe(el) });
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
      var hostName = location.hostname.replace(/^www\./, "").replace(/[^a-z0-9.-]/gi, "");
      flash(el);
      send({
        kind: "type",
        target: safeDescribe(el),
        text: isPassword ? "{{secret:" + (hostName || "site") + "-password}}" : String(valueOf(el) || ""),
        note: isPassword ? "Save the password in Settings, under Automation, as \"" + (hostName || "site") + "-password\"." : ""
      });
    }

    document.addEventListener("pointerdown", function (event) {
      if (!event.isTrusted || stopped || event.button !== 0) return;
      pressed = { x: event.clientX, y: event.clientY, el: actionable(deep(event)) };
    }, true);

    document.addEventListener("pointerup", function (event) {
      if (!event.isTrusted || stopped || event.button !== 0) return;
      var start = pressed;
      pressed = null;
      if (!start) return;
      if (Math.abs(event.clientX - start.x) + Math.abs(event.clientY - start.y) > 12) return;
      var el = actionable(deep(event)) || start.el;
      trackClick(el);
    }, true);

    document.addEventListener("click", function (event) {
      if (!event.isTrusted || stopped || event.button !== 0) return;
      trackClick(actionable(deep(event)));
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
        flash(el);
        send({ kind: "select", target: safeDescribe(el), values: values });
      } else if (tag === "input" && (el.getAttribute("type") || "").toLowerCase() === "file") {
        flush();
        var names = Array.prototype.map.call(el.files || [], function (file) { return file.name; });
        flash(el);
        send({ kind: "upload", target: safeDescribe(el), values: names, note: "Replace these names with full file paths." });
      } else if (pending === el) {
        flush();
      }
    }, true);

    document.addEventListener("keydown", function (event) {
      if (!event.isTrusted || stopped) return;
      var el = deep(event);
      var control = el && el.nodeType === 1 ? actionable(el) : null;
      if ((event.key === "Enter" || event.key === " ") && control && !skipClick(control) && (L.interactive(control) || L.pointerOnly(control))) {
        if (event.key === " " && event.repeat) return;
        trackClick(control);
        return;
      }
      if (event.key !== "Enter" && event.key !== "Escape") return;
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
    try { host.postMessage({ type: "ce-record", token: TOKEN, ready: true }); } catch (e) {}
    return true;
  }

  if (!boot()) {
    var tries = 0;
    var wait = setInterval(function () {
      tries += 1;
      if (boot() || tries >= 40) clearInterval(wait);
    }, 100);
  }
