import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { applyAppearance, readAppearance } from "./lib/appearance";
import { getUserStyles, onUserStylesChanged } from "./lib/browser-client";
import { useRoute } from "./lib/route";
import { applyUserStyles } from "./lib/user-styles";
import { applyZoom, readZoom } from "./lib/zoom";
import { BookmarksPage } from "./features/Bookmarks/Bookmarks.page";
import { CaptureEditorPage } from "./features/CaptureEditor/CaptureEditor.page";
import { BrowserPage } from "./features/Browser/Browser.page";
import { DownloadsPage } from "./features/Downloads/Downloads.page";
import { FilesPage } from "./features/Files/Files.page";
import { HistoryPage } from "./features/History/History.page";
import { ExtensionsPage } from "./features/Extensions/Extensions.page";
import { NewTabPage } from "./features/NewTab/NewTab.page";
import { PeekPage } from "./features/Peek/Peek.page";
import { PermissionsPage } from "./features/Permissions/Permissions.page";
import { PopupPage } from "./features/Popup/Popup.page";
import { PrintEditorPage } from "./features/PrintEditor/PrintEditor.page";
import { ReaderPage } from "./features/Reader/Reader.page";
import { ScriptsPage } from "./features/Scripts/Scripts.page";
import { ShortcutsPage } from "./features/Shortcuts/Shortcuts.page";
import { SettingsPage } from "./features/Settings/Settings.page";
import "./index.css";

const appearance = readAppearance();
applyAppearance(appearance.theme, appearance.colorMode);
applyZoom(readZoom());

void getUserStyles()
  .then((styles) => applyUserStyles(styles.css))
  .catch(() => undefined);
void onUserStylesChanged((styles) => applyUserStyles(styles.css));

function App() {
  const route = useRoute();
  if (route.kind === "browser") {
    return <BrowserPage />;
  }
  if (route.kind === "popup") {
    return <PopupPage popup={route.popup} params={route.params} />;
  }
  switch (route.page) {
    case "bookmarks":
      return <BookmarksPage />;
    case "downloads":
      return <DownloadsPage />;
    case "files":
      return <FilesPage params={route.params} />;
    case "history":
      return <HistoryPage />;
    case "settings":
      return <SettingsPage />;
    case "extensions":
      return <ExtensionsPage />;
    case "print-editor":
      return <PrintEditorPage params={route.params} />;
    case "peek":
      return <PeekPage />;
    case "reader":
      return <ReaderPage />;
    case "permissions":
      return <PermissionsPage />;
    case "scripts":
      return <ScriptsPage params={route.params} />;
    case "shortcuts":
      return <ShortcutsPage />;
    case "capture-editor":
      return <CaptureEditorPage />;
    default:
      return <NewTabPage />;
  }
}

const root = document.getElementById("root");
if (!root) {
  throw new Error("Root element is missing");
}

// A website could frame the app origin to reach the browser commands. Framed copies render nothing.
if (window.top === window.self) {
  createRoot(root).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}
