chrome.action.onClicked.addListener(function () {
  chrome.storage.local.set({ clicked: Date.now() });
});
