# To Do List

## Tabs and windows
- [x] Split view: two or more tab webviews side by side in one window, with a draggable divider. Each tab is already a child webview with its own bounds.
- [x] Tab stacks: group tabs into a single named, colored stack that opens out on hover.
- [x] Vertical tab sidebar as a setting, collapsible to icons.
- [x] Tab workspaces: named sets of tabs (for example Work and Personal) you can swap in one click without opening a new profile.
- [x] Tab hibernation: suspend background tabs after N minutes (WebView2 `TrySuspend`), with a per-site never-sleep list.
- [x] Peek tab: Shift+click a link to open it in a floating popup webview, close it with Esc.
- [x] Per-tab mute, and a "mute all but the active tab" mode.

## Page control (things sites normally decide)
- [x] Per-site user CSS and JS (built-in userscript manager): extends the profile `styles.css` to a per-domain style and script editor at `ce://scripts`.
- [x] Force-enable copy, paste, right-click and text selection on sites that block them.
- [x] "Clean page" action that removes sticky headers, cookie banners and subscribe overlays.
- [x] Element zapper: click any element to hide it permanently on that site, stored as a rule.
- [x] Force dark mode per site, with brightness and contrast sliders.
- [x] Reader mode that reuses the Print Editor cleanup rules.
- [x] Custom user agent per site.
- [x] Global video speed control (keyboard shortcuts for any HTML5 video), remembered per site.
- [x] Disable JavaScript, images or fonts per site from an address-bar toggle.
- [x] Auto-refresh a tab on an interval, with an optional "notify when the page content changes" mode.

## Privacy and security
- [x] Containers: isolated cookie stores per tab group (WebView2 profiles via `CreateCoreWebView2ControllerOptions` with a profile name).
- [x] Ephemeral tabs: an InPrivate tab inside the normal window that wipes itself on close.
- [x] Fingerprinting shield: randomize the canvas, WebGL and audio fingerprint per site with an injected script.
- [x] Tracking-parameter stripper: remove `utm_*`, `fbclid`, `gclid` and similar from URLs before navigation and when copying.
- [x] Referrer control per site.
- [x] Permission dashboard at `ce://permissions`: every site's camera, microphone, location and notification grant, with one-click revoke.
- [x] Network request log per tab (`WebResourceRequested`), viewable without DevTools.
- [x] HTTPS-only mode with a per-site exception list.

## Productivity
- [ ] Command palette (Ctrl+K): fuzzy search across tabs, bookmarks, history, settings and built-in actions.
- [ ] Omnibox bangs and custom keywords (for example `!gh query` or `yt query`), configurable in settings.
- [ ] Omnibox calculator, unit conversion and currency conversion inline.
- [ ] Keyboard shortcut editor at `ce://shortcuts`, including key chords.
- [ ] Mouse gestures, for example right-drag left to go back.

## Media and capture
- [ ] Full-page and region screenshot with an annotate-and-blur editor (extends Page Capture).
- [ ] Area, screen, tab recording to mp4 or GIF. 
- [ ] Media hub popup listing every playing audio or video across all tabs, with play, pause and seek.
- [ ] Download any media on a page (images and video sources) from a grid view.
- [ ] Save a page as a single self-contained HTML file or as markdown.
- [ ] OCR an image or region on the page and copy the text.

## Downloads and files
- [ ] Multi-connection download accelerator for large files.
- [ ] Rename-on-download template, for example `{site}-{date}-{name}`.
- [ ] Built-in viewers for JSON, CSV, Markdown and log files, with formatting and search.
- [ ] Local file browser at `ce://files` for quick previews.

## Developer tools
- [ ] Request rewriting and blocking rules (a local mapping, like Requestly) through `WebResourceRequested`.
- [ ] Responsive viewport toggle that sizes the tab webview to preset devices.
- [ ] "Open in another browser" that launches the URL in installed Chrome, Firefox or Edge.
- [ ] Cookie and localStorage editor per site at `ce://storage`.
- [ ] Console error badge on the tab when the page logs errors.
- [ ] Color picker and font inspector tools.

## Customization and UI
- [ ] Drag-to-arrange toolbar items.
- [ ] Per-site accent color: tint the tab and toolbar from the site's theme color or favicon.

## Built-in extensions (new toolbar tools)
- [ ] Translate page inline through a configurable provider.
- [ ] Password generator.
- [ ] Price and stock tracker for product pages.
- [ ] Wayback lookup: open the current URL in the Internet Archive, or save it there.
- [ ] Word count, reading time and readability score for the current page.
- [ ] AI assistant panel: summarize the page or ask questions about it, using a user-provided API key or a local model endpoint.

## System integration (Windows)
- [ ] Install any site as a standalone app window with its own taskbar icon.
- [ ] Global hotkey to summon the browser or a quick-search popup from anywhere in Windows.
- [ ] Native Windows toast notifications for downloads and site notifications.
- [ ] Default browser registration for `http`, `https` and `.html`.
- [ ] Tray icon with recent tabs and quick search.
- [ ] Battery saver: on battery power, throttle background tabs and cap frame rate.

## Data and sync
- [ ] Encrypted export and import of a full profile as one file.
- [ ] Sync conflict viewer for the sync folder when two machines edit at the same time.
- [ ] Full-text history search: index page text locally so history can be searched by content.
- [ ] Bookmark health check: find dead links and duplicates.
