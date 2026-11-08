(function () {
  if (String(location.hostname || "").endsWith(".ce-ext.localhost")) return;
  var runtime = window.ce && window.ce.runtime;
  if (!runtime || !runtime.sendMessage) return;
  runtime.sendMessage("notes.example", { href: location.href });
})();
