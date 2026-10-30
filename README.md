# ce-browser

A small Windows browser built on Tauri 2 and Microsoft Edge WebView2. Tabs, pinned tabs, bookmarks, downloads and unpacked Chrome extensions are built in.

## Running

```powershell
pnpm install
pnpm tauri dev
```

`pnpm build:exe` produces `src-tauri/target/release/ce-browser.exe`. WebView2 ships with Windows 10 and 11. Extension support needs a recent WebView2 runtime (1.0.2739 or later).

## Tabs

Every tab is its own WebView2 page view, so a crashed or heavy page stays in its own tab. Tabs can be dragged to reorder, pinned, duplicated and reopened after closing (Ctrl+Shift+T). Pinned tabs sit at the left edge, show only their icon and survive "Close other tabs".

On startup the browser restores the last session, pinned tabs first. Restored tabs load the first time you select them. Settings can switch startup to the home page instead.

| Shortcut | Action |
| --- | --- |
| Ctrl+T / Ctrl+W | New tab / close tab |
| Ctrl+Shift+T | Reopen closed tab |
| Ctrl+Tab / Ctrl+Shift+Tab | Next / previous tab |
| Ctrl+1 to Ctrl+8, Ctrl+9 | Select tab, select last tab |
| Ctrl+L, Alt+D, F6 | Focus the address bar |
| Ctrl+R, F5 | Reload |
| Alt+Left / Alt+Right | Back / forward |
| Ctrl+D | Bookmark this page |
| Ctrl+Shift+B | Show or hide the bookmarks bar |
| Ctrl+J | Downloads |
| Ctrl+F | Find in page |
| Ctrl+= / Ctrl+- / Ctrl+0 | Zoom in / out / reset |
| F12 | Developer tools |

## Bookmarks

The star in the address bar adds or edits a bookmark. Bookmarks live in a **Bookmarks bar** folder and an **Other bookmarks** folder. `ce://bookmarks` manages folders and imports or exports the Netscape HTML format that Chrome, Edge and Firefox use.

## Downloads

Downloads go to your Downloads folder, or to a folder you choose in settings. An existing file is never overwritten: the new file gets a ` (1)` suffix. The toolbar button shows progress. Downloads can be paused, resumed, cancelled, opened and shown in Explorer. `ce://downloads` keeps the history.

## Extensions

WebView2 loads unpacked Chrome extensions but cannot install from the Chrome Web Store. To add one:

1. Download a Chromium build: a `.crx` from the Chrome Web Store (with a CRX downloader), a `.zip` from the extension's releases, or an unpacked folder that contains a `manifest.json`.
2. Open `ce://extensions` and choose **Install .zip or .crx** (or **Load unpacked** for a folder).
3. The extension icon appears next to the address bar. Click it to open the extension.

The extension's files are copied into `%APPDATA%\com.ce.browser\extensions`. WebView2 supports a subset of the Chrome extension APIs. Popups, content scripts and autofill usually work. Features that need native messaging or browser-specific APIs may not. Keyboard commands and right-click menu entries added by extensions are not available.

## Data

Settings, session, bookmarks, downloads and the extension list are JSON files in `%APPDATA%\com.ce.browser`. Cookies, site data and installed extensions live in the WebView2 profile at `%LOCALAPPDATA%\com.ce.browser\profile`.
