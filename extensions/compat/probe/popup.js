function finish(value, error) {
  if (error) return Promise.resolve({ error: String(error.message || error) });
  return Promise.resolve(value);
}

function call(fn) {
  return new Promise(function (resolve) {
    try {
      var result = fn(function (value) {
        resolve(finish(value, chrome.runtime.lastError));
      });
      if (result && typeof result.then === "function") {
        result.then(
          function (value) { resolve(finish(value, null)); },
          function (error) { resolve(finish(null, error)); },
        );
      }
    } catch (error) {
      resolve(finish(null, error));
    }
  });
}

var report = {};

chrome.storage.local.get(null).then(function (stored) {
  report.stored = stored || {};
  return call(function (done) { chrome.tabs.query({}, done); });
}).then(function (tabs) {
  report.popupTabs = Array.isArray(tabs) ? tabs.map(function (tab) {
    return { id: tab.id, windowId: tab.windowId, active: tab.active, url: tab.url, title: tab.title };
  }) : tabs;
  return call(function (done) { chrome.windows.getAll({ populate: false }, done); });
}).then(function (windows) {
  report.popupWindows = Array.isArray(windows) ? windows.map(function (item) {
    return { id: item.id, focused: item.focused, type: item.type };
  }) : windows;
  return call(function (done) { chrome.bookmarks.getTree(done); });
}).then(function (tree) {
  report.bookmarks = Array.isArray(tree) ? "tree" : tree;
  return call(function (done) { chrome.history.search({ text: "", maxResults: 5 }, done); });
}).then(function (history) {
  report.history = Array.isArray(history) ? history.length : history;
  return call(function (done) { chrome.downloads.search({ limit: 5 }, done); });
}).then(function (downloads) {
  report.downloads = Array.isArray(downloads) ? downloads.length : downloads;
  report.sidePanel = !!(chrome.sidePanel && chrome.sidePanel.open);
  return fetch("https://example.com/ce-dnr-probe").then(
    function (response) { return "status " + response.status; },
    function (error) { return String(error && error.message ? error.message : error); },
  );
}).then(function (dnr) {
  report.dnr = dnr;
  document.getElementById("probe-report").textContent = JSON.stringify(report, null, 2);
}).catch(function (error) {
  report.failed = String(error && error.message ? error.message : error);
  document.getElementById("probe-report").textContent = JSON.stringify(report, null, 2);
});
