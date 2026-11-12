import { ExtensionIcon, isComponentExtension } from "@/components/ExtensionIcon";
import { IconButton } from "@/components/IconButton";
import { PinPlacement } from "@/components/PinPlacement";
import { PopupFrame } from "@/components/PopupFrame";
import { Toggle } from "@/components/Toggle";
import { BUILTIN_TOOLS } from "@/features/BuiltInExtensions/builtin";
import { ExtensionsToolbarSideControl } from "@/features/Extensions/ExtensionsToolbarSideControl";
import { RemoveExtensionDialog } from "@/features/Extensions/RemoveExtensionDialog";
import {
  checkExtensionUpdates,
  closePopup,
  createTab,
  openExtensionOptions,
  removeExtension,
  removeHostExtension,
  saveSettings,
  setExtensionEnabled,
  setHostExtensionEnabled,
  updateExtension,
} from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import { builtinToolEnabled, saveBuiltinEnabled, saveToolbarPlacement, toolbarPlacement } from "@/lib/toolbar-pin";
import { toolbarSide, toolbarSidePatch } from "@/lib/toolbar-side";
import { useExtensions, useHostExtensions, useSettingsState } from "@/lib/use-browser";
import type { BrowserSettings, ExtensionInfo, HostExtensionInfo, ToolbarPlacement, ToolbarSide } from "@type/browser";
import { ExternalLink, Puzzle, RefreshCw, Settings2, SquareArrowOutUpRight, Trash2, X } from "lucide-react";
import { useState, type ReactNode } from "react";

type Status = { tone: "info" | "error"; text: string };
type PendingRemove = { kind: "installed" | "host"; id: string; name: string };

export function AddressBarExtensionPopover() {
  const { extensions, error: loadError, loaded } = useExtensions();
  const host = useHostExtensions();
  const { settings, loaded: settingsLoaded } = useSettingsState();
  const [status, setStatus] = useState<Status | null>(null);
  const [busy, setBusy] = useState(false);
  const [checking, setChecking] = useState(false);
  const [query, setQuery] = useState("");
  const [pending, setPending] = useState<PendingRemove | null>(null);

  const needle = query.trim().toLowerCase();
  const matches = (name: string) => needle === "" || name.toLowerCase().includes(needle);
  const installed = extensions.filter((extension) => !isComponentExtension(extension.id, extension.name) && matches(extension.name));
  const hosts = host.extensions.filter((extension) => matches(extension.name));
  const builtinItems = [
    ...extensions
      .filter((extension) => isComponentExtension(extension.id, extension.name) && matches(extension.name))
      .map((extension) => ({ kind: "component" as const, name: extension.name, extension })),
    ...BUILTIN_TOOLS.filter((tool) => matches(tool.name)).map((tool) => ({ kind: "tool" as const, name: tool.name, tool })),
  ].sort((left, right) => left.name.localeCompare(right.name, undefined, { sensitivity: "base" }));
  const ready = loaded && host.loaded && settingsLoaded;

  const fail = (action: Promise<unknown>) => {
    action.catch((reason: unknown) => setStatus({ tone: "error", text: String(reason) }));
  };

  const checkUpdates = async () => {
    setBusy(true);
    setChecking(true);
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
      setChecking(false);
    }
  };

  const setPlacement = (id: string, placement: ToolbarPlacement) => {
    if (settings) {
      fail(saveToolbarPlacement(settings, id, placement));
    }
  };

  const openExtensionsPage = () => {
    void createTab("ce://extensions");
    void closePopup();
  };

  const setToolbarSide = (side: ToolbarSide) => {
    if (settings) {
      fail(saveSettings({ ...settings, ...toolbarSidePatch(side) }));
    }
  };

  const setToolbarLabels = (verticalToolbarLabels: boolean) => {
    if (settings) {
      fail(saveSettings({ ...settings, verticalToolbarLabels }));
    }
  };

  const renderInstalled = (extension: ExtensionInfo, removable = true) => (
    <InstalledRow
      key={extension.id}
      extension={extension}
      settings={settings}
      busy={busy}
      onPlacement={setPlacement}
      onEnabled={(enabled) => fail(setExtensionEnabled(extension.id, enabled))}
      onOptions={() => {
        fail(openExtensionOptions(extension.id));
        void closePopup();
      }}
      onRemove={removable ? () => setPending({ kind: "installed", id: extension.id, name: extension.name }) : undefined}
      onUpdate={() => {
        setBusy(true);
        setStatus(null);
        void updateExtension(extension.id)
          .then((next) => setStatus({ tone: "info", text: `Updated ${next.name} ${next.version}.` }))
          .catch((reason: unknown) => setStatus({ tone: "error", text: String(reason) }))
          .finally(() => setBusy(false));
      }}
    />
  );

  return (
    <PopupFrame
      id="address-extension-popover"
      width={380}
      maxHeight={520}
      ready={ready}
      role="dialog"
      labelledBy="address-extension-popover-title"
    >
      <div id="address-extension-popover-header" className="flex items-center justify-between gap-2 py-1 pr-1 pl-3">
        <div id="address-extension-popover-title-row" className="flex min-w-0 items-center gap-1">
          <h2 id="address-extension-popover-title" className="flex items-center gap-1.5 text-sm font-medium">
            <Puzzle id="address-extension-popover-title-icon" className="size-3.5 text-sky-400" aria-hidden />
            Extensions
          </h2>
          <button
            id="address-extension-popover-open-page"
            type="button"
            title="Open extensions page"
            aria-label="Open extensions page"
            className={cn(
              "inline-flex size-5 shrink-0 items-center justify-center rounded text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100",
              focusRing,
            )}
            onClick={openExtensionsPage}
          >
            <ExternalLink id="address-extension-popover-open-page-icon" className="size-3.5" aria-hidden />
          </button>
        </div>
        <IconButton id="address-extension-popover-close" label="Close" onClick={() => void closePopup()}>
          <X id="address-extension-popover-close-icon" aria-hidden />
        </IconButton>
      </div>

      <div id="address-extension-popover-search-wrap" className="px-2 pb-2">
        <input
          id="address-extension-popover-search"
          value={query}
          spellCheck={false}
          placeholder="Filter extensions"
          aria-label="Filter extensions"
          className="h-7 w-full rounded-md border border-zinc-800 bg-zinc-950 px-2 text-xs text-zinc-100 outline-none placeholder:text-zinc-500 focus:border-sky-500"
          onChange={(event) => setQuery(event.target.value)}
        />
      </div>

      {status || loadError || host.error ? (
        <p
          id="address-extension-popover-status"
          className={cn(
            "mx-2 mb-2 rounded-md border px-2 py-1.5 text-xs",
            status?.tone === "error" || (!status && (loadError || host.error))
              ? "border-red-900 bg-red-950 text-red-300"
              : "border-zinc-800 bg-zinc-950 text-zinc-300",
          )}
        >
          {status?.text ?? loadError ?? host.error}
        </p>
      ) : null}

      <div id="address-extension-popover-list" className="flex flex-col pb-1">
        <Section
          id="address-extension-popover-installed"
          title="Installed"
          action={
            <button
              id="address-extension-popover-installed-refresh"
              type="button"
              title="Check for updates"
              aria-label="Check for updates"
              disabled={busy}
              className={cn(
                "inline-flex size-5 shrink-0 items-center justify-center rounded text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100 disabled:opacity-40",
                focusRing,
              )}
              onClick={() => void checkUpdates()}
            >
              <RefreshCw
                id="address-extension-popover-installed-refresh-icon"
                className={cn("size-3", checking && "animate-spin")}
                aria-hidden
              />
            </button>
          }
        >
          {installed.length === 0 ? (
            <Empty id="address-extension-popover-installed-empty" text={needle ? "No matching extensions." : "No third-party extensions installed."} />
          ) : (
            installed.map((extension) => renderInstalled(extension))
          )}
        </Section>

        {host.extensions.length > 0 ? (
          <Section id="address-extension-popover-host" title="Host">
            {hosts.length === 0 ? (
              <Empty id="address-extension-popover-host-empty" text="No matching host extensions." />
            ) : (
              hosts.map((extension) => (
                <HostRow
                  key={extension.id}
                  extension={extension}
                  settings={settings}
                  busy={busy}
                  onPlacement={setPlacement}
                  onEnabled={(enabled) => fail(setHostExtensionEnabled(extension.id, enabled))}
                  onRemove={() => setPending({ kind: "host", id: extension.id, name: extension.name })}
                />
              ))
            )}
          </Section>
        ) : null}

        <Section id="address-extension-popover-builtin" title="Built-in">
          {builtinItems.length === 0 ? (
            <Empty id="address-extension-popover-builtin-empty" text="No matching built-in extensions." />
          ) : (
            builtinItems.map((item) =>
              item.kind === "component" ? (
                renderInstalled(item.extension, false)
              ) : (
                <BuiltinRow
                  key={item.tool.id}
                  tool={item.tool}
                  settings={settings}
                  onPlacement={setPlacement}
                  onEnabled={(enabled) => settings && fail(saveBuiltinEnabled(settings, item.tool.id, enabled))}
                />
              ),
            )
          )}
        </Section>
      </div>

      <section id="address-extension-popover-toolbar" className="flex flex-col gap-3 border-t border-zinc-800 px-3 py-2.5">
        <div id="address-extension-popover-toolbar-heading">
          <h3 id="address-extension-popover-toolbar-title" className="text-sm font-medium">
            Toolbar
          </h3>
          <p id="address-extension-popover-toolbar-hint" className="mt-0.5 text-xs text-zinc-400">
            <kbd id="address-extension-popover-toolbar-shortcut" className="font-sans text-zinc-300">
              Ctrl+Shift+T
            </kbd>{" "}
            shows or hides it.
          </p>
        </div>
        <div id="address-extension-popover-toolbar-layout" className="flex items-center justify-between gap-3">
          <div id="address-extension-popover-toolbar-layout-text" className="min-w-0">
            <p id="address-extension-popover-toolbar-layout-label" className="text-sm">
              Side
            </p>
            <p id="address-extension-popover-toolbar-layout-hint" className="text-xs text-zinc-400">
              Top sits above the bookmarks bar. Right, left and bottom sit on that edge of the page.
            </p>
          </div>
          <ExtensionsToolbarSideControl
            id="address-extension-popover-toolbar-side"
            value={toolbarSide(settings)}
            disabled={!settings}
            onChange={setToolbarSide}
          />
        </div>
        <div id="address-extension-popover-toolbar-labels" className="flex items-center justify-between gap-3">
          <div id="address-extension-popover-toolbar-labels-text" className="min-w-0">
            <p id="address-extension-popover-toolbar-labels-label" className="text-sm">
              Show labels
            </p>
            <p id="address-extension-popover-toolbar-labels-hint" className="text-xs text-zinc-400">
              A short name under each icon.
            </p>
          </div>
          <Toggle
            id="address-extension-popover-toolbar-labels-toggle"
            aria-labelledby="address-extension-popover-toolbar-labels-label"
            checked={settings?.verticalToolbarLabels ?? false}
            disabled={!settings}
            onChange={setToolbarLabels}
          />
        </div>
      </section>

      <div id="address-extension-popover-confirm-room" className={pending ? "min-h-72" : undefined} />
      <RemoveExtensionDialog
        id="address-extension-popover-remove-dialog"
        name={pending?.name ?? null}
        open={pending !== null}
        onCancel={() => setPending(null)}
        onConfirm={() => {
          if (!pending) {
            return;
          }
          const next = pending;
          setPending(null);
          fail(next.kind === "host" ? removeHostExtension(next.id) : removeExtension(next.id));
        }}
      />

      <div id="address-extension-popover-footer" className="border-t border-zinc-800">
        <button
          id="address-extension-popover-manage"
          type="button"
          className={cn("flex w-full items-center justify-between gap-3 px-3 py-2.5 text-left text-sm hover:bg-zinc-800", focusRing)}
          onClick={openExtensionsPage}
        >
          <span id="address-extension-popover-manage-label">Manage extensions</span>
          <SquareArrowOutUpRight id="address-extension-popover-manage-icon" className="size-4 shrink-0 text-zinc-400" aria-hidden />
        </button>
      </div>
    </PopupFrame>
  );
}

function Section({ id, title, action, children }: { id: string; title: string; action?: ReactNode; children: ReactNode }) {
  return (
    <section id={id} aria-labelledby={`${id}-title`}>
      <div id={`${id}-heading`} className="flex items-center justify-between gap-2 pr-1.5">
        <h3 id={`${id}-title`} className="px-3 pt-2 pb-1 text-[10px] font-medium tracking-wide text-zinc-500 uppercase">
          {title}
        </h3>
        {action ? (
          <span id={`${id}-action`} className="pt-1">
            {action}
          </span>
        ) : null}
      </div>
      {children}
    </section>
  );
}

function Empty({ id, text }: { id: string; text: string }) {
  return (
    <p id={id} className="px-3 py-1.5 text-xs text-zinc-500">
      {text}
    </p>
  );
}

function Row({
  id,
  name,
  detail,
  enabled,
  icon,
  placement,
  placementDisabled,
  onPlacement,
  enabledDisabled,
  onEnabled,
  actions,
}: {
  id: string;
  name: string;
  detail: string;
  enabled: boolean;
  icon: ReactNode;
  placement: ToolbarPlacement | null;
  placementDisabled: boolean;
  onPlacement: (placement: ToolbarPlacement) => void;
  enabledDisabled: boolean;
  onEnabled: (enabled: boolean) => void;
  actions?: ReactNode;
}) {
  return (
    <div id={id} title={detail} className={cn("flex items-center gap-1.5 px-2 py-1 hover:bg-zinc-800/70", !enabled && "opacity-70")}>
      <span id={`${id}-icon-wrap`} className="grid size-5 shrink-0 place-items-center">
        {icon}
      </span>
      <span id={`${id}-name`} className="min-w-0 flex-1 truncate text-xs text-zinc-100">
        {name}
      </span>
      {placement ? (
        <PinPlacement id={`${id}-pin`} placement={placement} disabled={placementDisabled} onChange={onPlacement} />
      ) : (
        <span id={`${id}-pin-none`} className="text-[10px] text-zinc-600">
          —
        </span>
      )}
      {actions}
      <Toggle id={`${id}-enabled`} size="sm" checked={enabled} disabled={enabledDisabled} title={enabled ? `Disable ${name}` : `Enable ${name}`} onChange={onEnabled} />
    </div>
  );
}

function InstalledRow({
  extension,
  settings,
  busy,
  onPlacement,
  onEnabled,
  onOptions,
  onRemove,
  onUpdate,
}: {
  extension: ExtensionInfo;
  settings: BrowserSettings | null;
  busy: boolean;
  onPlacement: (id: string, placement: ToolbarPlacement) => void;
  onEnabled: (enabled: boolean) => void;
  onOptions: () => void;
  onRemove?: () => void;
  onUpdate: () => void;
}) {
  const id = `address-extension-popover-ext-${extension.id}`;
  return (
    <Row
      id={id}
      name={extension.name}
      detail={extension.description || extension.id}
      enabled={extension.enabled}
      icon={
        <ExtensionIcon id={`${id}-icon`} extensionId={extension.id} name={extension.name} src={extension.icon} className="size-4" />
      }
      placement={toolbarPlacement(settings, extension.id)}
      placementDisabled={!settings}
      onPlacement={(placement) => onPlacement(extension.id, placement)}
      enabledDisabled={false}
      onEnabled={onEnabled}
      actions={
        <>
          {extension.updateVersion ? (
            <IconAction id={`${id}-update`} label={`Update to ${extension.updateVersion}`} disabled={busy} onClick={onUpdate}>
              <RefreshCw id={`${id}-update-icon`} aria-hidden />
            </IconAction>
          ) : null}
          {extension.optionsPage ? (
            <IconAction id={`${id}-options`} label="Options" onClick={onOptions}>
              <Settings2 id={`${id}-options-icon`} aria-hidden />
            </IconAction>
          ) : null}
          {onRemove ? (
            <IconAction id={`${id}-remove`} label="Remove" onClick={onRemove}>
              <Trash2 id={`${id}-remove-icon`} aria-hidden />
            </IconAction>
          ) : null}
        </>
      }
    />
  );
}

function HostRow({
  extension,
  settings,
  busy,
  onPlacement,
  onEnabled,
  onRemove,
}: {
  extension: HostExtensionInfo;
  settings: BrowserSettings | null;
  busy: boolean;
  onPlacement: (id: string, placement: ToolbarPlacement) => void;
  onEnabled: (enabled: boolean) => void;
  onRemove: () => void;
}) {
  const id = `address-extension-popover-host-${extension.id}`;
  const pinId = `host:${extension.id}`;
  return (
    <Row
      id={id}
      name={extension.name}
      detail={extension.description || extension.permissions.join(", ") || extension.id}
      enabled={extension.enabled}
      icon={<ExtensionIcon id={`${id}-icon`} extensionId={extension.id} name={extension.name} src={extension.icon} className="size-4" />}
      placement={extension.surfaces.toolbar ? toolbarPlacement(settings, pinId) : null}
      placementDisabled={!settings}
      onPlacement={(placement) => onPlacement(pinId, placement)}
      enabledDisabled={busy}
      onEnabled={onEnabled}
      actions={
        <IconAction id={`${id}-remove`} label="Remove" disabled={busy} onClick={onRemove}>
          <Trash2 id={`${id}-remove-icon`} aria-hidden />
        </IconAction>
      }
    />
  );
}

function BuiltinRow({
  tool,
  settings,
  onPlacement,
  onEnabled,
}: {
  tool: (typeof BUILTIN_TOOLS)[number];
  settings: BrowserSettings | null;
  onPlacement: (id: string, placement: ToolbarPlacement) => void;
  onEnabled: (enabled: boolean) => void;
}) {
  const id = `address-extension-popover-builtin-${tool.id}`;
  const Icon = tool.icon;
  const enabled = builtinToolEnabled(settings, tool.id);
  return (
    <Row
      id={id}
      name={tool.name}
      detail={tool.description}
      enabled={enabled}
      icon={<Icon id={`${id}-icon`} className="size-4 text-zinc-300" aria-hidden />}
      placement={toolbarPlacement(settings, tool.id)}
      placementDisabled={!settings}
      onPlacement={(placement) => onPlacement(tool.id, placement)}
      enabledDisabled={!settings}
      onEnabled={onEnabled}
    />
  );
}

function IconAction({
  id,
  label,
  disabled,
  onClick,
  children,
}: {
  id: string;
  label: string;
  disabled?: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      id={id}
      type="button"
      title={label}
      aria-label={label}
      disabled={disabled}
      className={cn(
        "inline-flex size-6 shrink-0 items-center justify-center rounded text-zinc-400 hover:bg-zinc-700 hover:text-zinc-100 disabled:opacity-40 [&_svg]:size-3.5",
        focusRing,
      )}
      onClick={onClick}
    >
      {children}
    </button>
  );
}
