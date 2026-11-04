# ce-browser

A small Windows browser built on Tauri 2 and Microsoft Edge WebView2. Tabs, pinned tabs, bookmarks, downloads and unpacked Chrome extensions are built in.

## Running

```powershell
pnpm install
pnpm tauri dev
```

`pnpm build:exe` produces the installer `src-tauri/target/release/bundle/nsis/ce-browser_<version>_x64-setup.exe`. WebView2 ships with Windows 10 and 11. Extension support needs a recent WebView2 runtime (1.0.2739 or later).

## Tabs

Every tab is its own WebView2 page view, so a crashed or heavy page stays in its own tab. Tabs can be dragged to reorder, pinned, duplicated and reopened after closing (Ctrl+Shift+T). Pinned tabs sit at the left edge, show only their icon and survive "Close other tabs".

On startup the browser restores the last session, pinned tabs first. Pinned tabs load in the background one at a time right after the window opens, and never sleep. Other restored tabs load the first time you select them. Settings can switch startup to the home page instead.

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
| Ctrl+K | Command palette |
| Ctrl+Alt+1 to Ctrl+Alt+9 | Switch to workspace 1 to 9 |
| Shift+click a link | Open it in a peek |
| Esc (in a peek) | Close the peek |
| [ / ] | Slow down or speed up a video on the page |

## Vertical tabs

Settings > Tabs (or the browser menu) moves the tabs into a sidebar left of the page. The sidebar can be collapsed to icons with the button at its bottom, and dragged wider or narrower from its right edge.

## Tab groups

Right-click a tab and choose **Add to new group**. A group gets a color and an optional name. Click the group's label to collapse it into one chip. Hover a collapsed chip to see its tabs and pick one. Right-click the label to rename it, change its color, add a tab, ungroup it or close it. Links opened from a grouped tab join the group.

## Split view

The split button in the toolbar, or **Split with current tab** in a tab's right-click menu, shows two tabs side by side. Drag the divider to resize them, or double-click it to make both sides equal. Click into either side to make it the active tab. Selecting another tab leaves split view, and selecting either half again brings it back.

## Workspaces

A workspace is a named set of tabs in the same window. The workspace button at the left of the tab strip lists workspaces and creates new ones. A tab's right-click menu moves it to another workspace. Tabs in the workspace you leave stay open in the background.

## Peek

Shift+click a link, or choose **Open link in peek** from its right-click menu, to open it in a card over the current tab. Esc, the close button, or a click on the tab behind closes it. **Open as tab** and **Open in split view** keep the page. Settings > Tabs can turn Shift+click peeks off.

## Sleeping tabs and muting

Tabs that stay in the background for 30 minutes go to sleep: the page is suspended and wakes when you select it. Sleeping tabs are dimmed and show a moon. Tabs playing sound, the tabs on screen, and sites in the never-sleep list stay awake. Settings > Tabs changes the delay or turns sleep off.

A speaker icon on a tab shows it is playing sound. Click it to mute or unmute the tab. Settings > Tabs can mute every tab that is not on screen.

## Bookmarks

The star in the address bar adds or edits a bookmark. Bookmarks live in a **Bookmarks bar** folder and an **Other bookmarks** folder. `ce://bookmarks` manages folders and imports or exports the Netscape HTML format that Chrome, Edge and Firefox use.

## Media

The page-capture menu saves a PNG of the whole page or the visible area. **Region** lets you drag a rectangle. **Edit** opens `ce://capture-editor` with that image: pen, arrow, rectangle, text, and blur.

**Record tab**, **Record area**, and **Record window** write a GIF, about 8 frames a second, for up to 15 seconds. The long side stays at 1280 pixels. Record window includes the tab strip of this browser and leaves other apps out. There is no MP4 file.

**Media** on the toolbar lists audio and video in open tabs, with play, pause, and seek. **Media on this page** shows a grid; each item, and **Save all** (up to 40), downloads through the download shelf.

**Save HTML** writes one file and inlines styles the page can read. **Save Markdown** writes the title, article text, links, and image addresses. **Copy text from image** uses Windows OCR on the visible page or a dragged region and copies the words. If Windows has no OCR language pack, the menu says so.

## Downloads

Downloads go to your Downloads folder, or to a folder you choose in settings. An existing file is never overwritten: the new file gets a ` (1)` suffix. The toolbar button shows progress. Downloads can be paused, resumed, cancelled, opened and shown in Explorer. `ce://downloads` keeps the history.

**Accelerate large downloads** is on by default. A file of 8 MB or more is fetched in 4 pieces when the server accepts ranges, using the cookies of the tab that started it. Smaller files, and servers that do not accept ranges, stay on the single download. Pause, resume, and cancel work for both. An accelerated file that was still running when the browser quit can be resumed from the shelf.

**File name** in Settings > Downloads is empty by default, which keeps the name the site suggested. `{site}-{date}-{name}` builds a name from the host (without `www`), the date (`YYYY-MM-DD`), and the file stem. `{ext}` is the extension. The extension is kept when the template does not mention it. "Ask where to save" offers that name in the dialog.

`ce://files` opens the download folder. You can go into a folder, go up, or open another folder. JSON is pretty-printed, CSV is a table, Markdown shows headings, lists, links, and code, and logs are lines. Search filters the table and highlights the other views. Previews stop at 2 MB. A finished JSON, CSV, Markdown, or log download opens here; other files open in their usual app.


## Page control

The sliders button in the toolbar changes how the current site behaves. A rule for a site also covers its subdomains, and it is remembered in `site-controls.json`.

- **Allow copy, paste and selection** turns those back on, including the right-click menu, on sites that block them.
- **Clean page** hides sticky bars, cookie notices and subscribe popups.
- **Force dark mode** inverts the page, with brightness and contrast sliders. Photos stay as they are.
- **Video speed** applies to every video and audio element. `[` and `]` change it while you watch.
- **User agent**, **block JavaScript**, **block images** and **block fonts** take effect on the next load of that site.
- **Pick an element to hide** outlines elements as you move the pointer. Click one to hide it on that site from then on. Esc cancels.
- **Reader mode** opens the article in the same tab, using the same cleanup as Print to PDF. Back, or Show original, returns to the site.
- **Auto-refresh** reloads that tab on an interval. **Notify when the page changes** puts a dot on the tab when the text differs.
- **Site scripts** opens `ce://scripts`, where CSS and JavaScript can be saved for one site or for every site. `styles.css` still styles the browser itself.

## Productivity

Ctrl+K opens the command palette: search open tabs, bookmarks, history and browser actions.

The address bar accepts keywords. `gh react` or `!yt cats` searches GitHub or YouTube. The built-in keywords are `gh`, `yt`, `w` and `r`. Add your own under Settings > Search. A keyword's address uses `%s` for the query.

The address bar also answers `2*(3+4)`, `10 km to mi` and `32 f to c`. `10 usd to eur` uses the day's exchange rates when the computer is online. Enter on an answer leaves it in the address bar.

`ce://shortcuts` lists every shortcut. Record captures the next key, and a second key within one second makes a chord. Reset one row, or reset all.

## Privacy

Settings > Privacy holds three switches. **Strip tracking parameters** is on by default and removes `utm_*`, `fbclid`, `gclid` and similar from addresses and from links you copy. **Fingerprint shield** adds a little noise to canvas, WebGL and audio reads; each site gets the same noise until the browser exits. **HTTPS-only** loads `http` pages over `https`. Localhost stays on `http`, and a site can be excepted from the page-controls menu or from the list in settings.

The page-controls menu also sets the **referrer** for the current site: browser default, never send, site only, or same site only.

**Network log** in that menu lists the recent requests for the tab, with status codes. It is kept in memory only.

`ce://permissions` lists camera, microphone, location and notification requests. Revoke blocks the next request from that site.

A tab group can use a **container**. The container has its own cookies, logins and cache, in `%LOCALAPPDATA%\com.ce.browser\containers\<id>`. Extensions stay on the default profile. Changing a group's container loads its pages again.

An **ephemeral tab** is InPrivate. Right-click the new-tab button, or use the tab menu or the browser menu. It is not saved and it is left out of history. Ephemeral tabs share one private session, which is dropped when the last one closes.

## Developer tools

`ce://rewrites` holds request rules. A rule matches when the request URL contains its text. It can block the request, send it to another address, or answer a GET with a local file of up to 8 MB. Rules run before the ad blocker.

Page controls can size the active page as a phone (390×844), a tablet (768×1024), or a laptop (1280×800). The page is centered. A split view is set aside until the device size is turned off. The choice lasts for that window.

The tab menu and the browser menu can open the current site in Chrome, Firefox, or Edge when that browser is installed. `ce://` pages stay here.

`ce://storage` lists the cookies and localStorage of the site in the tab. You can add and delete them. localStorage needs that site to be loaded.

Page errors, unhandled rejections, and `console.error` put a count on the tab. It clears when the tab navigates and is not kept in the session. **Color and font** in page controls follows the pointer and copies the element's color, background, and font. Esc cancels.

## Customization

Drag a pinned icon on the address bar onto another to reorder the built-in tools and extension buttons. The order is remembered. Back, forward, reload, home, the address field, Downloads, and the menu stay put. Right-click still unpins an icon.

**Site accent** in Settings > Appearance is on by default. The active tab and the address bar take a tint from the site's theme color, or from its favicon when the page has no theme color. Other tabs for that site show a thin edge in the same color.

## Tools

**Translate** replaces the article text on the page. Show original, or click the icon again, reloads the page. Settings > Translate sets the language and the provider URL. `%s` is the text and `%t` is the language. The default provider needs no key.

**Password** builds a password on this computer and copies it. The length and the character types are chosen in the popup.

**Prices** reads a product price from the page and remembers it in `prices.json`. Check updates a tracked product when that page is open, and marks a lower price or a stock change.

**Reading** shows the word count, the minutes at 200 words a minute, and a reading-ease score.

## System

**Install site** in the tab menu adds a Start menu shortcut for that website. Opening it shows one window, with the site's name and icon, and Windows can give it a separate taskbar button.

Ctrl+Alt+Space restores the browser and focuses the address bar. Change it under Settings > System. The shortcut is read when the browser starts.

A finished download shows a Windows toast. Activating it opens the file. A site can raise a toast only after its notification permission is allow.

ce-browser registers for `http`, `https`, and HTML files. **Make default** opens Windows settings so you can confirm. A website address on the command line opens in the running window.

The tray icon lists recent tabs, Show, Quick search, and Quit. Clicking it shows the window.

**Battery saver** is on by default. On battery, background tabs sleep after a few seconds and the visible page stays near 30 frames a second. Plugged in, the normal sleep delay returns.


## Internal Pages

These addresses open inside the browser. Type one in the address bar, or open it from a menu.
- `ce://newtab` is a new tab. Ctrl+T and the browser menu open it, and the address bar takes focus.
- `ce://bookmarks` manages bookmark folders. It imports and exports the Netscape HTML format that Chrome, Edge and Firefox use.
- `ce://boards` shows saved pins in a grid. Folders hold boards. Search covers titles, notes, page text and addresses.
- `ce://history` lists pages you have opened. You can search it, remove one entry, or clear it.
- `ce://downloads` keeps the download history. Items can be paused, resumed, cancelled, opened and shown in Explorer.
- `ce://files` opens the download folder. You can go into a folder, go up, or open another folder. JSON, CSV, Markdown and logs preview in the page. Previews stop at 2 MB.
- `ce://settings` is the browser settings. A link such as `ce://settings?focus=settings-profiles` opens a section.
- `ce://extensions` installs an unpacked folder, a `.zip`, or a `.crx`, and turns extensions on or off.
- `ce://scripts` saves CSS and JavaScript for one site or for every site. `styles.css` still styles the browser itself.
- `ce://shortcuts` lists every shortcut. Record captures the next key, and a second key within one second makes a chord.
- `ce://permissions` lists camera, microphone, location and notification requests. Revoke blocks the next request from that site.
- `ce://rewrites` holds request rules. A rule can block a request, send it to another address, or answer a GET with a local file of up to 8 MB.
- `ce://storage` lists the cookies and localStorage of the site in the tab. You can add and delete them. `ce://storage?tab=<id>` opens a chosen tab's site.
- `ce://reader` shows the current article with the same cleanup as Print to PDF. Show original returns to the site.
- `ce://capture-editor` edits a page capture: pen, arrow, rectangle, text and blur.
- `ce://print-editor` prepares the current page for PDF: delete content, set the font, margins, orientation, backgrounds, and headers and footers.
- `ce://shopping` lists tracked product prices. A lower price or a stock change is marked when that page is checked.
- `ce://whats-new` shows the release notes for the version that is running. A newer version opens it once after an update.


## Data

Each profile keeps its settings, session, bookmarks, downloads, history and extension list as JSON files in its own folder. The folder is named after the profile. The first profile is named "Default". The root folder is the sync folder when one is selected, otherwise `%APPDATA%\com.ce.browser`.

- Root folder (the selected sync folder or `%APPDATA%\com.ce.browser`)
  - profiles.json
  - Default
    - extensions folder
    - bookmarks.json
    - downloads.json
    - extensions.json
    - history.json
    - session.json
    - settings.json
    - containers.json
    - permissions.json
    - site-controls.json
    - styles.css
  - Profile 2
    - extensions folder
    - bookmarks.json
    - downloads.json
    - extensions.json
    - history.json
    - session.json
    - settings.json
    - containers.json
    - permissions.json
    - site-controls.json
    - styles.css

Cookies, logins and the WebView2 profile stay in `%LOCALAPPDATA%\com.ce.browser` on each computer. Data written by an older build (default profile files loose in the root, other profiles under `profiles\<id>`, or a `ce-browser` folder in the sync folder) moves into this layout the next time the browser starts.


## Extensions

WebView2 loads unpacked Chrome extensions but cannot install from the Chrome Web Store. To add one:

1. Download a Chromium build: a `.crx` from the Chrome Web Store (with a CRX downloader), a `.zip` from the extension's releases, or an unpacked folder that contains a `manifest.json`.
2. Open `ce://extensions` and choose **Install .zip or .crx** (or **Load unpacked** for a folder).
3. The extension icon appears next to the address bar. Click it to open the extension.

The extension's files are copied into the `extensions` folder of the current profile, inside the root folder described above. WebView2 supports a subset of the Chrome extension APIs. Popups, content scripts and autofill usually work. Features that need native messaging or browser-specific APIs may not. Keyboard commands and right-click menu entries added by extensions are not available.