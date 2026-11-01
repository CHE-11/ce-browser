# Feature list

Windows desktop browser built on Tauri 2 and WebView2. Each tab is its own page view, so a crashed or heavy page stays in that tab.

## Tabs and windows

- New, close, duplicate, and reopen the last closed tab (Ctrl+Shift+T)
- Drag tabs to reorder them
- Pinned tabs sit at the left, show only their icon, and survive Close other tabs
- Session restore on startup, with pinned tabs first; restored tabs load the first time you select them
- Startup can open the home page instead of the last session
- Next and previous tab, and jump to a tab by number (Ctrl+1 through Ctrl+8, Ctrl+9 for the last)
- Vertical tab sidebar, collapsible to icons and resizable from its right edge
- Tab groups with a color and an optional name; collapse to a chip, hover to pick a tab, and links opened from a grouped tab join the group
- Workspaces: named sets of tabs in the same window; tabs you leave stay open in the background (Ctrl+Alt+1 through Ctrl+Alt+9)
- Split view of two tabs side by side, with a draggable divider that equalizes on double-click
- Peek: Shift+click a link, or Open link in peek, to show it in a card over the current tab; Esc closes it, and it can become a tab or a split
- Sleeping tabs suspend after a delay (default 30 minutes) and wake when selected; a moon marks them, and a never-sleep list keeps chosen sites awake
- Sound indicator on a tab, with click to mute or unmute, plus an option to mute every tab that is not on screen
- Pinned tabs can be allowed to sleep
- Find in page (Ctrl+F)
- Page zoom in, out, and reset (Ctrl+=, Ctrl+-, Ctrl+0), with a default page zoom in settings
- Developer tools (F12)

## Address bar and search

- Focus the address bar (Ctrl+L, Alt+D, F6)
- Back, forward, reload, and home
- Omnibox suggestions
- Search keywords such as `gh`, `yt`, `w`, and `r`, plus custom keywords that use `%s` for the query (`gh react`, `!yt cats`)
- Custom search URL
- Inline answers for math, unit conversion, and currency (`2*(3+4)`, `10 km to mi`, `32 f to c`, `10 usd to eur`)
- Command palette (Ctrl+K) across open tabs, bookmarks, history, and browser actions

## Bookmarks and history

- Star in the address bar to add or edit a bookmark (Ctrl+D)
- Bookmarks bar and Other bookmarks folders, with the bar shown or hidden (Ctrl+Shift+B)
- `ce://bookmarks` to manage folders, drag items, and switch between views
- Import and export Netscape HTML bookmarks (Chrome, Edge, and Firefox)
- Bookmark health check for dead links and duplicates
- `ce://history` with search, removal, and clear
- Full-text history search over page text indexed on this computer, with the index optional in Settings > Privacy

## Downloads and files

- Downloads go to the Downloads folder or a folder chosen in settings
- Existing files are never overwritten; a new file gets a ` (1)` suffix
- Toolbar progress, plus pause, resume, cancel, open, and show in Explorer
- `ce://downloads` keeps the history
- Ask where to save each file
- Accelerate files of 8 MB or more in four pieces when the server accepts ranges, using the cookies of the tab that started the download
- An accelerated download that was still running when the browser quit can be resumed
- Rename template such as `{site}-{date}-{name}`, with `{ext}` for the extension
- Windows toast when a download finishes; activating it opens the file
- `ce://files` browses the download folder and previews JSON, CSV, Markdown, and logs, with search, up to 2 MB
- A finished JSON, CSV, Markdown, or log download opens in the file viewer; other files open in their usual app

## Page control

- Per-site rules, including subdomains, remembered in `site-controls.json`
- Allow copy, paste, selection, and the right-click menu on sites that block them
- Clean page hides sticky bars, cookie notices, and subscribe popups
- Force dark mode with brightness and contrast sliders; photos stay as they are
- Video and audio speed, remembered per site, with `[` and `]` while watching
- Per-site user agent, and block JavaScript, images, or fonts on the next load
- Element zapper: click an element to hide it on that site from then on
- Reader mode in the same tab, using the same cleanup as Print to PDF
- Auto-refresh on an interval, and a tab dot when the page text changes
- Site scripts at `ce://scripts`: CSS and JavaScript for one site or every site
- `styles.css` styles the browser chrome itself
- Referrer for the current site: browser default, never send, site only, or same site only
- Device size for the active page: phone (390×844), tablet (768×1024), or laptop (1280×800)

## Media and capture

- Save a PNG of the whole page, the visible area, or a dragged region
- Screenshot editor (`ce://capture-editor`) with pen, arrow, rectangle, text, and blur
- Record tab, record area, and record window as a GIF, about 8 frames a second, up to 15 seconds, long side at 1280 pixels
- Record window includes this browser's tab strip and leaves other apps out
- Media hub lists audio and video in open tabs, with play, pause, and seek
- Media on this page shows a grid and can download items, or up to 40 with Save all
- Save HTML as one file with inlined styles the page can read
- Save Markdown with the title, article text, links, and image addresses
- Copy text from an image with Windows OCR on the visible page or a dragged region
- Clear cached files for the site in the current tab

## Print

- Print to PDF from the toolbar
- Quick clean PDF, save the page as it is, or open the print editor
- Print editor to delete content, remove similar items, set font, margins, orientation, backgrounds, and headers and footers

## Privacy

- Strip tracking parameters (`utm_*`, `fbclid`, `gclid`, and similar) from addresses and copied links
- Fingerprint shield adds stable per-site noise to canvas, WebGL, and audio reads until the browser exits
- HTTPS-only mode, with localhost left on HTTP and a per-site exception list
- Network log of recent requests for the tab, kept in memory only
- `ce://permissions` for camera, microphone, location, and notification grants, with revoke
- A site can raise a Windows toast only after its notification permission is allow
- Containers on a tab group: separate cookies, logins, and cache; extensions stay on the default profile
- Ephemeral (InPrivate) tabs share one private session, are left out of history, and are dropped when the last one closes

## Developer tools

- Request rules at `ce://rewrites`: block a URL, send it elsewhere, or answer a GET with a local file of up to 8 MB; rules run before the ad blocker
- Open the current site in Chrome, Firefox, or Edge when that browser is installed; `ce://` pages stay here
- Cookie and localStorage editor at `ce://storage`
- Badge on the tab when the page logs errors, unhandled rejections, or `console.error`; it clears on navigation
- Color and font inspector that follows the pointer and copies the element's color, background, and font
- What runs lists frameworks, languages, UI, libraries, CMS, analytics, server, hosting, and build tools on the page

## Productivity

- Keyboard shortcut editor at `ce://shortcuts`, including chords, with per-row and reset-all
- Mouse gestures: right-drag left for back, right for forward, up to reload, down to close the tab
- Ad blocker for ads and trackers, with a pause list per site
- Translate replaces article text on the page through a configurable provider; the default needs no key
- Password generator on this computer, with length and character types, copied to the clipboard
- Price tracker reads a product price, stores it in `prices.json`, and marks a lower price or a stock change
- Wayback opens the current site in the Internet Archive or asks the archive to save it
- Reading stats: word count, minutes at 200 words a minute, and a reading-ease score
- Assistant summarizes the page or answers a question about it through an OpenAI-compatible endpoint or a local model; the API key stays on this computer

## Customization

- Drag pinned address-bar icons to reorder built-in tools and extension buttons
- Right-click to unpin an icon; back, forward, reload, home, the address field, Downloads, and the menu stay put
- Light and dark mode, and a choice of themes
- Site accent tints the active tab and address bar from the page theme color or favicon
- Separate zoom for the browser UI and for web pages
- Bookmarks bar visibility

## System

- Install a site as a Start menu shortcut with its own window, name, icon, and taskbar button
- Global summon shortcut (Ctrl+Alt+Space by default) restores the browser and focuses the address bar
- Register as the handler for `http`, `https`, and HTML files, with Make default opening Windows settings
- A website address on the command line opens in the running window
- Tray icon with recent tabs, Show, Quick search, and Quit
- Battery saver: on battery, background tabs sleep after a few seconds and the visible page stays near 30 frames a second

## Profiles, data, and sync

- Multiple profiles, each with its own bookmarks, history, downloads, extensions, settings, containers, permissions, and site controls
- Profile icon, Start menu item, and taskbar pin
- Encrypted export and import of a profile as one file; cookies stay on this computer
- Optional sync folder for profile JSON; cookies, logins, and the WebView2 profile stay in local app data
- Sync conflict viewer to keep this computer's copy, keep the other copy, or merge when both machines edited bookmarks, history, or settings
- Older data layouts migrate into the current profile folders on startup

## Extensions

- Load unpacked Chrome extensions, or install a `.zip` or `.crx`, from `ce://extensions`
- Extension files are copied into the current profile
- Popups, content scripts, and autofill usually work
- Native messaging, browser-specific APIs, extension keyboard commands, and extension right-click menus are not available
- WebView2 cannot install from the Chrome Web Store directly

## Internal pages

- `ce://newtab`
- `ce://bookmarks`
- `ce://history`
- `ce://downloads`
- `ce://files`
- `ce://settings`
- `ce://extensions`
- `ce://scripts`
- `ce://shortcuts`
- `ce://permissions`
- `ce://rewrites`
- `ce://storage`
- `ce://reader`
- `ce://capture-editor`
- `ce://print-editor`
