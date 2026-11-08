import { Button } from "@/components/Button";
import { PageShell } from "@/components/PageShell";
import { Toggle } from "@/components/Toggle";
import { installExtensionZip, installExtensionFolder, installExtensionFromStore, checkExtensionUpdates, updateExtension, setExtensionEnabled, openExtensionOptions, removeExtension, saveSettings } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { saveBuiltinEnabled, saveToolbarPlacement } from "@/lib/toolbar-pin";
import { toolbarSide, toolbarSidePatch } from "@/lib/toolbar-side";
import { useExtensions, useHostExtensions, useSettings } from "@/lib/use-browser";
import { HostExtensionsSection } from "../HostExtensions/HostExtensionsSection";
import { ExtensionInfo, ToolbarPlacement, ToolbarSide } from "@type/browser";
import { FileArchive, FolderOpen, Puzzle } from "lucide-react";
import { useState } from "react";
import { builtinTool } from "../BuiltInExtensions/builtin";
import { ExtensionsTable } from "./ExtensionsTable";
import { ExtensionsToolbarSideControl } from "../ExtensionsToolbar/ExtensionsToolbarSideControl";


export function ExtensionsPage() {
  const { extensions, error: loadError, loaded } = useExtensions();
  const host = useHostExtensions();
  const settings = useSettings();
  const [status, setStatus] = useState<{ tone: "info" | "error"; text: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const [storeUrl, setStoreUrl] = useState("");

  const install = async (installer: () => Promise<ExtensionInfo | null>, verb: "Installed" | "Updated" = "Installed") => {
    setBusy(true);
    setStatus(null);
    try {
      const extension = await installer();
      if (extension) {
        setStatus({ tone: "info", text: `${verb} ${extension.name} ${extension.version}.` });
      }
    } catch (reason) {
      setStatus({ tone: "error", text: String(reason) });
    } finally {
      setBusy(false);
    }
  };

  const installFromStore = () => {
    const url = storeUrl.trim();
    if (!url) {
      return;
    }
    void install(async () => {
      const extension = await installExtensionFromStore(url);
      setStoreUrl("");
      return extension;
    });
  };

  const checkUpdates = async () => {
    setBusy(true);
    setStatus(null);
    try {
      const list = await checkExtensionUpdates();
      const waiting = list.filter((extension) => extension.updateVersion).length;
      const watched = list.filter((extension) => extension.store).length;
      const text = waiting
        ? `${waiting} update${waiting === 1 ? "" : "s"} available.`
        : watched
          ? "Extensions are up to date."
          : "No store extensions to check.";
      setStatus({ tone: "info", text });
    } catch (reason) {
      setStatus({ tone: "error", text: String(reason) });
    } finally {
      setBusy(false);
    }
  };

  const act = (action: Promise<unknown>) => {
    action.catch((reason: unknown) => setStatus({ tone: "error", text: String(reason) }));
  };

  const setPlacement = (id: string, placement: ToolbarPlacement) => {
    if (settings) {
      act(saveToolbarPlacement(settings, id, placement));
    }
  };

  const setToolbarLabels = (verticalToolbarLabels: boolean) => {
    if (settings) {
      act(saveSettings({ ...settings, verticalToolbarLabels }));
    }
  };

  const setToolbarSide = (side: ToolbarSide) => {
    if (settings) {
      act(saveSettings({ ...settings, ...toolbarSidePatch(side) }));
    }
  };

  return (
    <PageShell
      id="extensions-page"
      page="extensions"
      title="Extensions"
      actions={
        <>
          <Button id="extensions-page-install-zip" variant="primary" disabled={busy} onClick={() => void install(installExtensionZip)}>
            <FileArchive id="extensions-page-install-zip-icon" className="size-4" aria-hidden />
            Install .zip or .crx
          </Button>
          <Button id="extensions-page-install-folder" variant="outline" disabled={busy} onClick={() => void install(installExtensionFolder)}>
            <FolderOpen id="extensions-page-install-folder-icon" className="size-4" aria-hidden />
            Load unpacked
          </Button>
        </>
      }
    >
      <div id="extensions-page-content" className="mx-auto flex max-w-6xl flex-col gap-6 pb-10">
        <section id="extensions-page-toolbar" className="flex flex-col gap-4 rounded-lg border border-zinc-800 bg-zinc-900 p-4">
          <div id="extensions-page-toolbar-heading">
            <h2 id="extensions-page-toolbar-title" className="text-sm font-semibold">
              Toolbar
            </h2>
            <p id="extensions-page-toolbar-hint" className="mt-1 text-xs text-zinc-400">
              <kbd id="extensions-page-toolbar-shortcut" className="font-sans text-zinc-300">
                Ctrl+Shift+T
              </kbd>{" "}
              shows or hides it.
            </p>
          </div>
          <div id="extensions-page-toolbar-layout" className="flex items-center justify-between gap-4">
            <div id="extensions-page-toolbar-layout-text" className="min-w-0">
              <p id="extensions-page-toolbar-layout-label" className="text-sm">
                Side
              </p>
              <p id="extensions-page-toolbar-layout-hint" className="text-xs text-zinc-400">
                Top sits above the bookmarks bar. Right, left and bottom sit on that edge of the page.
              </p>
            </div>
            <ExtensionsToolbarSideControl
              id="extensions-page-toolbar-side"
              value={toolbarSide(settings)}
              disabled={!settings}
              onChange={setToolbarSide}
            />
          </div>
          <div id="extensions-page-toolbar-labels" className="flex items-center justify-between gap-4">
            <div id="extensions-page-toolbar-labels-text" className="min-w-0">
              <p id="extensions-page-toolbar-labels-label" className="text-sm">
                Show labels
              </p>
              <p id="extensions-page-toolbar-labels-hint" className="text-xs text-zinc-400">
                A short name under each icon.
              </p>
            </div>
            <Toggle
              id="extensions-page-toolbar-labels-toggle"
              aria-labelledby="extensions-page-toolbar-labels-label"
              checked={settings?.verticalToolbarLabels ?? false}
              disabled={!settings}
              onChange={setToolbarLabels}
            />
          </div>
        </section>

        <section id="extensions-page-store" className="flex flex-col gap-3 rounded-lg border border-zinc-800 bg-zinc-900 p-4">
          <h2 id="extensions-page-store-title" className="text-sm font-semibold">
            Install from a store
          </h2>
          <p id="extensions-page-store-hint" className="text-xs text-zinc-400">
            Paste a Chrome Web Store or Edge Add-ons link, or an extension id.
          </p>
          <form
            id="extensions-page-store-form"
            className="flex flex-col gap-2 sm:flex-row"
            onSubmit={(event) => {
              event.preventDefault();
              installFromStore();
            }}
          >
            <input
              id="extensions-page-store-url"
              value={storeUrl}
              spellCheck={false}
              placeholder="https://chromewebstore.google.com/detail/…"
              className="h-8 min-w-0 flex-1 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500"
              onChange={(event) => setStoreUrl(event.target.value)}
            />
            <Button id="extensions-page-store-install" type="submit" variant="primary" disabled={busy || storeUrl.trim() === ""}>
              Install from store
            </Button>
            <Button id="extensions-page-store-check" variant="outline" disabled={busy} onClick={() => void checkUpdates()}>
              Check for updates
            </Button>
          </form>
        </section>
        {status || loadError ? (
          <div
            id="extensions-page-status"
            className={cn(
              "rounded-md border px-3 py-2 text-sm",
              status?.tone === "info" ? "border-zinc-800 bg-zinc-900 text-zinc-300" : "border-red-900 bg-red-950 text-red-300",
            )}
          >
            {status?.text ?? loadError}
          </div>
        ) : null}

        {loaded && extensions.length === 0 ? (
          <p id="extensions-page-empty" className="text-xs text-zinc-400">
            No third-party extensions installed yet. Paste a store link, or install a .zip, .crx, or unpacked folder.
          </p>
        ) : null}
        <HostExtensionsSection
          extensions={host.extensions}
          loaded={host.loaded}
          busy={busy}
          onStatus={setStatus}
          onBusy={setBusy}
        />
        <ExtensionsTable
          extensions={extensions}
          loaded={loaded}
          settings={settings}
          busy={busy}
          onPlacement={setPlacement}
          onEnabled={(id, enabled) => {
            if (builtinTool(id)) {
              if (settings) {
                act(saveBuiltinEnabled(settings, id, enabled));
              }
              return;
            }
            act(setExtensionEnabled(id, enabled));
          }}
          onOptions={(id) => act(openExtensionOptions(id))}
          onRemove={(id) => act(removeExtension(id))}
          onUpdate={(id) => void install(() => updateExtension(id), "Updated")}
        />

        <section id="extensions-page-install" className="rounded-lg border border-zinc-800 bg-zinc-900/60 p-5">
          <h2 id="extensions-page-install-title" className="flex items-center gap-2 text-sm font-semibold">
            <Puzzle id="extensions-page-install-icon" className="size-4 text-sky-400" aria-hidden />
            Installing an extension
          </h2>
          <ol id="extensions-page-install-steps" className="mt-3 list-decimal space-y-1.5 pl-5 text-sm text-zinc-300">
            <li id="extensions-page-install-step-1">
              Paste a Chrome Web Store or Edge Add-ons link above. A <code>.crx</code>, a <code>.zip</code>, or an unpacked
              folder with a <code>manifest.json</code> still works from the buttons at the top of the page.
            </li>
            <li id="extensions-page-install-step-2">
              Click <strong>Install .zip or .crx</strong> and pick the file. An unzipped folder works with <strong>Load unpacked</strong>.
            </li>
            <li id="extensions-page-install-step-3">
              Choose Primary to show it next to the address bar, Secondary to show it on the tools toolbar, or both. Unpinned hides it from both. Right-click an icon to change the same choice.
            </li>
          </ol>
          <p id="extensions-page-install-note" className="mt-3 text-xs text-zinc-500">
            Extensions run in Microsoft Edge WebView2, which supports a subset of the Chrome extension APIs. Popups, content
            scripts and autofill usually work. Features that need native messaging or browser-specific APIs may not.
          </p>
        </section>
      </div>
    </PageShell>
  );
}
