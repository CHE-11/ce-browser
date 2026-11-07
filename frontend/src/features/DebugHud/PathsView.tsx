import { useCallback, useEffect, useState } from "react";
import { FolderOpen, RefreshCw } from "lucide-react";
import type { DebugPaths } from "@type/browser";
import { debugPaths, debugRevealPath } from "@/lib/browser-client";
import { formatBytes } from "@/lib/format";
import { formatWhen } from "@/lib/network-log";
import { HudButton, HudEmpty, HudToolbar } from "./HudControls";

function failureText(reason: unknown, fallback: string) {
  return typeof reason === "string" && reason.trim() ? reason : fallback;
}

export function PathsView() {
  const [paths, setPaths] = useState<DebugPaths | null>(null);
  const [error, setError] = useState("");

  const load = useCallback(() => {
    void debugPaths()
      .then((next) => {
        setPaths(next);
        setError("");
      })
      .catch((reason: unknown) => setError(failureText(reason, "The data folders could not be read.")));
  }, []);

  useEffect(() => load(), [load]);

  const reveal = (path: string) => {
    void debugRevealPath(path).catch((reason: unknown) => setError(failureText(reason, "Explorer did not open.")));
  };

  const folders = paths
    ? [
        { key: "data-dir", label: `Profile data (${paths.profile})`, path: paths.dataDir },
        { key: "data-root", label: "Data root", path: paths.dataRoot },
        { key: "root-data-dir", label: "App data", path: paths.rootDataDir },
        { key: "profile-dir", label: "WebView2 profile", path: paths.profileDir },
      ]
    : [];

  return (
    <div id="debug-paths" className="flex min-h-0 flex-1 flex-col">
      <HudToolbar id="debug-paths-toolbar">
        <span id="debug-paths-title" className="text-[11px] text-zinc-400">
          Folders and store files of the active profile
        </span>
        <HudButton id="debug-paths-refresh" className="ml-auto" onClick={load}>
          <RefreshCw id="debug-paths-refresh-icon" aria-hidden className="size-3" />
          Refresh
        </HudButton>
      </HudToolbar>
      {error ? (
        <p id="debug-paths-error" className="px-3 py-1 text-[11px] text-red-300">
          {error}
        </p>
      ) : null}
      <div id="debug-paths-scroller" className="min-h-0 flex-1 overflow-auto px-2 py-1">
        {paths === null ? (
          <HudEmpty id="debug-paths-loading">Loading folders…</HudEmpty>
        ) : (
          <>
            <ul id="debug-paths-folders" className="flex flex-col gap-1">
              {folders.map((folder) => (
                <li id={`debug-paths-${folder.key}`} key={folder.key} className="flex items-center gap-2 rounded border border-zinc-800 px-2 py-1">
                  <div id={`debug-paths-${folder.key}-text`} className="min-w-0 flex-1">
                    <p id={`debug-paths-${folder.key}-label`} className="text-[10px] uppercase tracking-wide text-zinc-500">
                      {folder.label}
                    </p>
                    <p id={`debug-paths-${folder.key}-path`} className="truncate font-mono text-[11px] text-zinc-200" title={folder.path}>
                      {folder.path}
                    </p>
                  </div>
                  <HudButton id={`debug-paths-${folder.key}-open`} title="Open in Explorer" onClick={() => reveal(folder.path)}>
                    <FolderOpen id={`debug-paths-${folder.key}-open-icon`} aria-hidden className="size-3.5" />
                  </HudButton>
                </li>
              ))}
            </ul>
            <h3 id="debug-paths-files-title" className="mt-3 px-1 text-[10px] uppercase tracking-wide text-zinc-500">
              Files in profile data ({paths.files.length})
            </h3>
            <ul id="debug-paths-files" className="mt-1">
              {paths.files.map((file) => (
                <li id={`debug-paths-file-${file.name}`} key={file.path} className="flex items-center gap-2 border-t border-zinc-800/70 px-1 py-0.5">
                  <span id={`debug-paths-file-${file.name}-name`} className="min-w-0 flex-1 truncate font-mono text-[11px] text-zinc-200">
                    {file.name}
                  </span>
                  <span id={`debug-paths-file-${file.name}-size`} className="w-16 shrink-0 text-right text-[10px] tabular-nums text-zinc-400">
                    {formatBytes(file.bytes)}
                  </span>
                  <span id={`debug-paths-file-${file.name}-modified`} className="w-32 shrink-0 text-right text-[10px] tabular-nums text-zinc-500">
                    {file.modified ? formatWhen(file.modified) : "–"}
                  </span>
                  <HudButton id={`debug-paths-file-${file.name}-reveal`} title="Show in Explorer" onClick={() => reveal(file.path)}>
                    <FolderOpen id={`debug-paths-file-${file.name}-reveal-icon`} aria-hidden className="size-3" />
                  </HudButton>
                </li>
              ))}
            </ul>
          </>
        )}
      </div>
    </div>
  );
}
