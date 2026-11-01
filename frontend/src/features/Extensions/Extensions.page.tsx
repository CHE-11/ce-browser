import { Button } from "@/components/Button";
import { EmptyState } from "@/components/EmptyState";
import { ExtensionIcon } from "@/components/ExtensionIcon";
import { PageShell } from "@/components/PageShell";
import { Skeleton } from "@/components/Skeleton";
import { Toggle } from "@/components/Toggle";
import { installExtensionZip, installExtensionFolder, setExtensionEnabled, openExtensionOptions, removeExtension, saveSettings } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { saveToolbarPin, toolbarPinned } from "@/lib/toolbar-pin";
import { useExtensions, useSettings } from "@/lib/use-browser";
import { ExtensionInfo } from "@type/browser";
import { FileArchive, FolderOpen, Puzzle, PinOff, Pin, Settings2, Trash2 } from "lucide-react";
import { useState } from "react";
import { BUILTIN_TOOLS } from "../BuiltInExtensions/builtin";


export function ExtensionsPage() {
  const { extensions, error: loadError, loaded } = useExtensions();
  const settings = useSettings();
  const [status, setStatus] = useState<{ tone: "info" | "error"; text: string } | null>(null);
  const [busy, setBusy] = useState(false);

  const install = async (installer: () => Promise<ExtensionInfo | null>) => {
    setBusy(true);
    setStatus(null);
    try {
      const extension = await installer();
      if (extension) {
        setStatus({ tone: "info", text: `Installed ${extension.name} ${extension.version}.` });
      }
    } catch (reason) {
      setStatus({ tone: "error", text: String(reason) });
    } finally {
      setBusy(false);
    }
  };

  const act = (action: Promise<unknown>) => {
    action.catch((reason: unknown) => setStatus({ tone: "error", text: String(reason) }));
  };

  const setPinned = (id: string, pinned: boolean) => {
    if (settings) {
      act(saveToolbarPin(settings, id, pinned));
    }
  };

  const extensionBar = settings?.verticalToolbarExtensions ? "vertical toolbar" : "address bar";

  const setToolbarOption = (patch: { verticalToolbarExtensions?: boolean; verticalToolbarLabels?: boolean }) => {
    if (settings) {
      act(saveSettings({ ...settings, ...patch }));
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
      <div id="extensions-page-content" className="mx-auto flex max-w-4xl flex-col gap-6 pb-10">
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

        {!loaded ? (
          <div id="extensions-page-loading" className="grid grid-cols-1 gap-3 md:grid-cols-2">
            {[0, 1, 2, 3].map((index) => (
              <Skeleton id={`extensions-page-loading-${index}`} key={index} className="h-32 rounded-lg" />
            ))}
          </div>
        ) : extensions.length === 0 ? (
          <EmptyState
            id="extensions-page-empty"
            icon={Puzzle}
            title="No extensions installed yet"
            hint="Install a Chrome extension from a .zip or .crx file, or load an unpacked folder."
            className="rounded-lg border border-dashed border-zinc-700"
            action={
              <Button id="extensions-page-empty-install" size="sm" disabled={busy} onClick={() => void install(installExtensionZip)}>
                <FileArchive id="extensions-page-empty-install-icon" className="size-4" aria-hidden />
                Install .zip or .crx
              </Button>
            }
          />
        ) : (
          <div id="extensions-page-grid" className="grid grid-cols-1 gap-3 md:grid-cols-2">
            {extensions.map((extension) => {
              const domId = `extension-${extension.id}`;
              return (
                <article
                  key={extension.id}
                  id={domId}
                  className={cn(
                    "flex flex-col gap-3 rounded-lg border border-zinc-800 bg-zinc-900 p-4",
                    !extension.enabled && "opacity-70",
                  )}
                >
                  <div id={`${domId}-header`} className="flex items-start gap-3">
                    <ExtensionIcon
                      id={`${domId}-icon`}
                      extensionId={extension.id}
                      name={extension.name}
                      src={extension.icon}
                      className="size-10"
                    />
                    <div id={`${domId}-text`} className="min-w-0 flex-1">
                      <h2 id={`${domId}-name`} className="truncate text-sm font-semibold">
                        {extension.name}
                        <span id={`${domId}-version`} className="ml-2 text-xs font-normal text-zinc-500">
                          {extension.version}
                        </span>
                      </h2>
                      <p id={`${domId}-description`} className="line-clamp-2 text-xs text-zinc-400">
                        {extension.description || "No description."}
                      </p>
                      <p id={`${domId}-id`} className="mt-1 truncate font-mono text-[11px] text-zinc-600">
                        {extension.id}
                      </p>
                    </div>
                    <Toggle
                      id={`${domId}-toggle`}
                      checked={extension.enabled}
                      title={extension.enabled ? "Disable" : "Enable"}
                      onChange={(enabled) => act(setExtensionEnabled(extension.id, enabled))}
                    />
                  </div>
                  <div id={`${domId}-actions`} className="flex justify-end gap-2">
                    <Button
                      id={`${domId}-pin`}
                      size="sm"
                      variant="ghost"
                      disabled={!settings}
                      title={toolbarPinned(settings, extension.id) ? `Hide from the ${extensionBar}` : `Show on the ${extensionBar}`}
                      onClick={() => setPinned(extension.id, !toolbarPinned(settings, extension.id))}
                    >
                      {toolbarPinned(settings, extension.id) ? (
                        <PinOff id={`${domId}-pin-icon`} className="size-3.5" aria-hidden />
                      ) : (
                        <Pin id={`${domId}-pin-icon`} className="size-3.5" aria-hidden />
                      )}
                      {toolbarPinned(settings, extension.id) ? "Unpin" : "Pin"}
                    </Button>
                    {extension.optionsPage ? (
                      <Button id={`${domId}-options`} size="sm" variant="ghost" onClick={() => act(openExtensionOptions(extension.id))}>
                        <Settings2 id={`${domId}-options-icon`} className="size-3.5" aria-hidden />
                        Options
                      </Button>
                    ) : null}
                    <Button id={`${domId}-remove`} size="sm" variant="ghost" onClick={() => act(removeExtension(extension.id))}>
                      <Trash2 id={`${domId}-remove-icon`} className="size-3.5" aria-hidden />
                      Remove
                    </Button>
                  </div>
                </article>
              );
            })}
          </div>
        )}

        <section id="extensions-page-builtin" className="flex flex-col gap-3">
          <div id="extensions-page-toolbar" className="flex flex-col gap-4 rounded-lg border border-zinc-800 bg-zinc-900 p-4">
            <h2 id="extensions-page-toolbar-title" className="text-sm font-semibold">
              Vertical toolbar
            </h2>
            <div id="extensions-page-toolbar-extensions" className="flex items-center justify-between gap-4">
              <div id="extensions-page-toolbar-extensions-text" className="min-w-0">
                <p id="extensions-page-toolbar-extensions-label" className="text-sm">
                  Use the vertical toolbar
                </p>
                <p id="extensions-page-toolbar-extensions-hint" className="text-xs text-zinc-400">
                  Pinned extensions move to the right side of the page.
                </p>
              </div>
              <Toggle
                id="extensions-page-toolbar-extensions-toggle"
                aria-labelledby="extensions-page-toolbar-extensions-label"
                checked={settings?.verticalToolbarExtensions ?? false}
                disabled={!settings}
                onChange={(verticalToolbarExtensions) => setToolbarOption({ verticalToolbarExtensions })}
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
                onChange={(verticalToolbarLabels) => setToolbarOption({ verticalToolbarLabels })}
              />
            </div>
          </div>
          <h2 id="extensions-page-builtin-title" className="text-sm font-semibold">
            Built-in tools
          </h2>
          <div id="extensions-page-builtin-grid" className="grid grid-cols-1 gap-3 md:grid-cols-2">
            {BUILTIN_TOOLS.map((tool) => {
              const domId = `builtin-${tool.id}`;
              const pinned = toolbarPinned(settings, tool.id);
              const Icon = tool.icon;
              return (
                <article
                  key={tool.id}
                  id={domId}
                  className="flex items-center gap-3 rounded-lg border border-zinc-800 bg-zinc-900 p-4"
                >
                  <Icon id={`${domId}-icon`} className="size-8 shrink-0 text-zinc-300" aria-hidden />
                  <div id={`${domId}-text`} className="min-w-0 flex-1">
                    <h3 id={`${domId}-name`} className="truncate text-sm font-semibold">
                      {tool.name}
                    </h3>
                    <p id={`${domId}-description`} className="line-clamp-2 text-xs text-zinc-400">
                      {tool.description}
                    </p>
                  </div>
                  <Button
                    id={`${domId}-pin`}
                    size="sm"
                    variant="ghost"
                    disabled={!settings}
                    title={pinned ? "Hide from the toolbar" : "Show on the toolbar"}
                    onClick={() => setPinned(tool.id, !pinned)}
                  >
                    {pinned ? (
                      <PinOff id={`${domId}-pin-icon`} className="size-3.5" aria-hidden />
                    ) : (
                      <Pin id={`${domId}-pin-icon`} className="size-3.5" aria-hidden />
                    )}
                    {pinned ? "Unpin" : "Pin"}
                  </Button>
                </article>
              );
            })}
          </div>
        </section>

        <section id="extensions-page-install" className="rounded-lg border border-zinc-800 bg-zinc-900/60 p-5">
          <h2 id="extensions-page-install-title" className="flex items-center gap-2 text-sm font-semibold">
            <Puzzle id="extensions-page-install-icon" className="size-4 text-sky-400" aria-hidden />
            Installing an extension
          </h2>
          <ol id="extensions-page-install-steps" className="mt-3 list-decimal space-y-1.5 pl-5 text-sm text-zinc-300">
            <li id="extensions-page-install-step-1">
              Download a Chromium build of the extension: a <code>.crx</code> from the Chrome Web Store (with a CRX
              downloader), a <code>.zip</code> from the extension releases, or an unpacked folder that contains a <code>manifest.json</code>.
            </li>
            <li id="extensions-page-install-step-2">
              Click <strong>Install .zip or .crx</strong> and pick the file. An unzipped folder works with <strong>Load unpacked</strong>.
            </li>
            <li id="extensions-page-install-step-3">
              The extension icon appears next to the address bar. Turn on the vertical toolbar to move pinned extensions to the right side of the page. Unpin an icon on this page, or right-click it, to take it off the toolbar.
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
