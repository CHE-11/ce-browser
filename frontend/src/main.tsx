import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { applyAppearance, applyFont, noteWindowsScheme, readAppearance, readFont } from "./lib/appearance";
import { getUserStyles, getWindowsColorScheme, onUserStylesChanged, onWindowsColorScheme } from "./lib/browser-client";
import { useRoute } from "./lib/route";
import { applyUserStyles } from "./lib/user-styles";
import { applyZoom, readZoom } from "./lib/zoom";
import { AgentPanelPage } from "./features/Agent/AgentPanel.page";
import { AutomationsPage } from "./features/Automations/Automations.page";
import { BoardsPage } from "./features/Boards/Boards.page";
import { BookmarksPage } from "./features/Bookmarks/Bookmarks.page";
import { CaptureEditorPage } from "./features/CaptureEditor/CaptureEditor.page";
import { BrowserPage } from "./features/Browser/Browser.page";
import { DownloadsPage } from "./features/Downloads/Downloads.page";
import { FilesPage } from "./features/Files/Files.page";
import { HistoryPage } from "./features/History/History.page";
import { NetworkPage } from "./features/Network/Network.page";
import { ExtensionsPage } from "./features/Extensions/Extensions.page";
import { PeekPage } from "./features/Peek/Peek.page";
import { PermissionsPage } from "./features/Permissions/Permissions.page";
import { PopupPage } from "./features/Popup/Popup.page";
import { PrintEditorPage } from "./features/PrintEditor/PrintEditor.page";
import { ReaderPage } from "./features/Reader/Reader.page";
import { ScriptsPage } from "./features/Scripts/Scripts.page";
import { RewritesPage } from "./features/Rewrites/Rewrites.page";
import { ShortcutsPage } from "./features/Shortcuts/Shortcuts.page";
import { StoragePage } from "./features/Storage/Storage.page";
import { SettingsPage } from "./features/Settings/Settings.page";
import { StylesPage } from "./features/Styles/Styles.page";
import { ShoppingPage } from "./features/Shopping/Shopping.page";
import { WhatsNewPage } from "./features/WhatsNew/WhatsNew.page";
import "./index.css";

const appearance = readAppearance();
applyAppearance(appearance.theme, appearance.colorMode);
applyFont(readFont());
applyZoom(readZoom());

void getWindowsColorScheme()
  .then((scheme) => noteWindowsScheme(scheme))
  .catch(() => undefined);
void onWindowsColorScheme((scheme) => noteWindowsScheme(scheme));

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
    case "network":
      return <NetworkPage tabId={Number(route.params.get("id"))} />;
    case "settings":
      return <SettingsPage />;
    case "styles":
      return <StylesPage />;
    case "shopping":
      return <ShoppingPage params={route.params} />;
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
    case "rewrites":
      return <RewritesPage />;
    case "storage":
      return <StoragePage params={route.params} />;
    case "capture-editor":
      return <CaptureEditorPage />;
    case "boards":
      return <BoardsPage params={route.params} />;
    case "whats-new":
      return <WhatsNewPage params={route.params} />;
    case "agent-panel":
      return <AgentPanelPage />;
    case "automations":
      return <AutomationsPage params={route.params} />;
    default:
      return <BookmarksPage />;
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
