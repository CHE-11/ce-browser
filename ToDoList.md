# To-do list

Sizes: S is a small change, M touches a few files on both sides of the bridge, L is an architectural change.

## New features
- [ ] **Tab search and recently closed** (M) - Ctrl+Shift+A popup listing tabs across all workspaces plus the last 25 closed tabs/groups, each restorable (today only the last tab can be reopened).
- [ ] **Crash and unresponsive tab recovery** (M) - handle WebView2 `ProcessFailed` with a "this tab crashed" page and Reload; prompt to wait or end an unresponsive page.
- [ ] **Friendly error pages** (M) - offline, DNS, certificate, and timeout pages with Retry, Search instead, and "Open cached copy" via the existing Wayback tool.
- [ ] **Site info panel** (M) - click the lock in the address bar: connection/certificate, this site's permissions, cookies and storage size, ads blocked, and shortcuts into page controls, all in one place.
- [ ] **Auto-update** (M) - `tauri-plugin-updater` with a "What's new" page after an update.

## Feature enhancements
- [ ] **Omnibox ranking** (M) - "Switch to tab" matches, frecency ranking, inline autocomplete, Shift+Delete to remove a suggestion.
- [ ] **Command palette** (S) - jump to any setting, recently closed, workspaces, extension actions, fuzzy matching.
- [ ] **Ctrl+Tab in recent-use order** (S) - optional switcher overlay with thumbnails.
- [ ] **Page right-click menu** (S) - for selections: search, translate, ask assistant, copy clean link-to-text; for images: copy text (OCR), search image, save.
- [ ] **Find in page** (S) - match case, whole word, and "find in all tabs".
- [ ] **Sleeping tabs** (S) - "Sleep this tab" / "Sleep other tabs" actions and memory saved shown in the status bar.
- [ ] **Tab groups** (S) - save a group to bookmarks, reopen a bookmark folder as a group, auto-group by site.

## UI enhancements
- [ ] **Undo toasts** (S) - shared toast component for closing a group/many tabs, deleting bookmarks, clearing history, with Undo.
- [ ] **Loading feedback** (S) - thin progress bar under the address bar and a spinner on the tab favicon.
- [ ] **Address bar domain emphasis** (S) - bold the registrable domain, dim the rest, hide `https://` until focused.
- [ ] **Tab hover cards** (M) - title, URL, thumbnail, memory use, and sleep state.
- [ ] **Tab strip overflow** (S) - scroll with the mouse wheel, minimum tab width, overflow list button.
- [ ] **Motion** (S) - animate tab open/close/reorder and group collapse; honor reduced-motion.
- [ ] **Windows 11 Mica backdrop** (S) - Mica/Acrylic window effect behind the chrome, toggle in Appearance.
- [ ] **Shortcut cheat sheet** (S) - Ctrl+/ overlay listing the active shortcuts from `ce://shortcuts`.
- [ ] **Settings polish** (S) - "changed from default" dot and per-setting reset.
- [ ] **Empty and loading states** (S) - consistent empty states and skeletons on all `ce://` pages.
- [ ] **Shared UI primitives** (M) - common Button, Menu, Dialog components with visible keyboard focus rings and accessible labels.
