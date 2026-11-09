function note(patch) {
  chrome.storage.local.set(patch);
}

function probe() {
  chrome.contextMenus.removeAll(function () {
    chrome.contextMenus.create(
      { id: "ce-probe", title: "CE compat probe", contexts: ["page", "selection", "link", "image"] },
      function () {
        var error = chrome.runtime.lastError;
        note({ contextMenus: error ? String(error.message) : "created" });
      },
    );
  });
  chrome.action.setBadgeText({ text: "1" }, function () {
    var error = chrome.runtime.lastError;
    note({ badgeText: error ? String(error.message) : "set" });
  });
  chrome.action.setTitle({ title: "Probe title" }, function () {
    var error = chrome.runtime.lastError;
    note({ badgeTitle: error ? String(error.message) : "set" });
  });
  try {
    var port = chrome.runtime.connectNative("com.ce.browser.compat_probe");
    port.onDisconnect.addListener(function () {
      var error = chrome.runtime.lastError;
      note({ nativeMessaging: error ? String(error.message) : "disconnected" });
    });
  } catch (error) {
    note({ nativeMessaging: String(error && error.message ? error.message : error) });
  }
  chrome.tabs.query({}, function (tabs) {
    chrome.windows.getAll({}, function (windows) {
      note({
        workerTabs: (tabs || []).length,
        workerWindows: (windows || []).length,
      });
    });
  });
}

chrome.runtime.onInstalled.addListener(probe);
chrome.runtime.onStartup.addListener(probe);
probe();

chrome.commands.onCommand.addListener(function (command) {
  note({ command: command, commandAt: Date.now() });
});

chrome.contextMenus.onClicked.addListener(function () {
  note({ contextClick: "yes" });
});

chrome.action.onClicked.addListener(function () {
  note({ actionClick: Date.now() });
});
