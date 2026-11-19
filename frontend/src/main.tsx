import "./lib/console-capture";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { applyAppearance, applyFont, noteWindowsScheme, readAppearance, readFont } from "./lib/appearance";
import { getUserStyles, getWindowsColorScheme, onUserStylesChanged, onWindowsColorScheme } from "./lib/browser-client";
import { useRoute } from "./lib/route";
import { applyUserStyles } from "./lib/user-styles";
import { applyZoom, readZoom } from "./lib/zoom";
import { AgentPanelPage } from "./built_in_extensions/agent/AgentPanel.page";
import { AutomationsPage } from "./built_in_extensions/automation/Automations.page";
import { AutomationsPanelPage } from "./built_in_extensions/automation/AutomationsPanel.page";
import { CaptureEditorPage } from "./built_in_extensions/capture-editor/CaptureEditor.page";
import { PresentOverlayPage } from "./built_in_extensions/present/PresentOverlay.page";
import { PresentPalette } from "./built_in_extensions/present/PresentPalette";
import { RecordingCamera } from "./built_in_extensions/recording/RecordingCamera";
import { RecordingControls } from "./built_in_extensions/recording/RecordingControls";
import { RecordingDraw } from "./built_in_extensions/recording/RecordingDraw";
import { RecordingsPage } from "./built_in_extensions/recording/Recordings.page";
import { BrowserPage } from "./features/Browser/Browser.page";
import { DownloadsPage } from "./features/Downloads/Downloads.page";
import { FilesPage } from "./built_in_extensions/files/Files.page";
import { NetworkPage } from "./built_in_extensions/network/Network.page";
import { ExtensionsPage } from "./features/Extensions/Extensions.page";
import { PeekPage } from "./features/Peek/Peek.page";
import { PermissionsPage } from "./features/Permissions/Permissions.page";
import { PopupPage } from "./features/Popup/Popup.page";
import { PrintEditorPage } from "./built_in_extensions/print-to-pdf/PrintEditor.page";
import { ReaderPage } from "./built_in_extensions/reader/Reader.page";
import { ScriptsPage } from "./built_in_extensions/scripting/Scripts.page";
import { ConsolePanelPage } from "./built_in_extensions/console/ConsolePanel.page";
import { ScriptingPanelPage } from "./built_in_extensions/scripting/ScriptingPanel.page";
import { RewritesPage } from "./built_in_extensions/rewrites/Rewrites.page";
import { ShortcutsPage } from "./features/Shortcuts/Shortcuts.page";
import { StoragePage } from "./features/Storage/Storage.page";
import { SettingsPage } from "./features/Settings/Settings.page";
import { StylesPage } from "./features/Styles/Styles.page";
import { ShoppingPage } from "./built_in_extensions/shopping/Shopping.page";
import { WhatsNewPage } from "./features/WhatsNew/WhatsNew.page";
import "./index.css";
import { BookmarksPage } from "./features/Bookmarks/Bookmarks.page";
import { BoardsPage } from "./built_in_extensions/boards/page/Boards.page";
import { DebugHudPage } from "./built_in_extensions/debug-hud/DebugHud.page";
import { DebugOutlinePage } from "./built_in_extensions/debug-hud/DebugOutline.page";
import { HistoryPage } from "./features/History/History.page";

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
    case "recordings":
      return <RecordingsPage params={route.params} />;
    case "recording-controls":
      return <RecordingControls />;
    case "recording-camera":
      return <RecordingCamera />;
    case "recording-draw":
      return <RecordingDraw />;
    case "present":
      return <PresentOverlayPage />;
    case "present-controls":
      return <PresentPalette />;
    case "boards":
      return <BoardsPage params={route.params} />;
    case "whats-new":
      return <WhatsNewPage params={route.params} />;
    case "agent-panel":
      return <AgentPanelPage />;
    case "automations-panel":
      return <AutomationsPanelPage />;
    case "scripting-panel":
      return <ScriptingPanelPage />;
    case "console-panel":
      return <ConsolePanelPage owner={route.params.get("owner") ?? ""} />;
    case "automations":
      return <AutomationsPage params={route.params} />;
    case "debug-hud":
      return <DebugHudPage />;
    case "debug-outline":
      return <DebugOutlinePage params={route.params} />;
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
