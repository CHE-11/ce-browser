# Extension API measurements

Unpacked fixtures in this folder were installed with passage against a dev build, then removed. WebView2 runs the extension. These are the calls that returned, and the ones the browser had to host itself.

| API | Result |
| --- | --- |
| `contextMenus.create` | Succeeded. The item **CE compat probe** appeared at the bottom of the page right-click menu on example.com, and choosing it is left to WebView2. |
| `commands` `Ctrl+Shift+9` | Pressing it on the focused page did not run `chrome.commands.onCommand`. The browser keeps its own shortcuts. Bitwarden also suggests `Ctrl+Shift+Y`, which opens the agent panel. |
| `action.onClicked` | No WebView2 call dispatches it. The toolbar button is enabled (`Action only`) and the click has nowhere to go. |
| `action.setBadgeText` / `setTitle` | The calls succeeded inside the extension. The toolbar icon does not show the badge or a new title beyond `default_title`. |
| `chrome.tabs.query` / `chrome.windows.getAll` from a popup | One browser window and the real tab strip. `query({ active: true, currentWindow: true })` and `getCurrent` return the open page. |
| Same calls from the service worker | One window per webview (16 windows for 16 views). The page script cannot patch a worker. |
| Content script on `https://example.com/` | The script ran (`data-ce-content` was set). `chrome.tabs` was missing in that script (`reading 'query'`). |
| `chrome.sidePanel.open` | From the probe popup, opened `side.html` in the browser side panel. A side-panel-only toolbar button opens that extension's `side.html`. |
| `chrome_url_overrides.newtab` | A new tab loaded `chrome-extension://…/newtab.html`. Removed after the check. |
| `chrome.bookmarks.getTree` | Only `Favorites bar` and `Other favorites`. This browser's bookmark bar is not that tree. |
| `chrome.history.search` | Returned visits. Left on the WebView2 profile. |
| `chrome.downloads.search` | Returned downloads. `SetHandled` does not drop them. Accelerated downloads are not cancelled while an extension has the `downloads` permission, so the WebView2 operation stays visible. |
| `runtime.connectNative` | Without a host: `Specified native messaging host not found.` With the host registered for Chrome and Edge: the port stayed open. |
| `declarativeNetRequest` | `https://example.com/` returned 200. `https://example.com/ce-dnr-probe` failed to fetch. Request blocking in this browser steps aside while an extension has `declarativeNetRequest` or `webRequest`. |
