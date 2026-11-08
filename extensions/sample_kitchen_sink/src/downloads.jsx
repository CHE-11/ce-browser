import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { downloadLabel, downloadMeta, downloadPercent, downloadTone, explain, studio } from "./format.js";
import { Brand, Empty, ErrorNote, SearchField } from "./ui.jsx";

function Downloads() {
  const [items, setItems] = useState([]);
  const [query, setQuery] = useState("");
  const [error, setError] = useState("");

  useEffect(() => {
    document.title = "Studio downloads";
    const ce = studio();
    if (!ce) {
      setError("Studio runs inside the browser.");
      return;
    }
    let live = true;
    const load = () => {
      ce.downloads.list().then(
        (next) => {
          if (live) setItems(Array.isArray(next) ? next : []);
        },
        (reason) => {
          if (live) setError(explain(reason));
        },
      );
    };
    load();
    const stop = ce.downloads.onChanged(load);
    return () => {
      live = false;
      stop();
    };
  }, []);

  const run = (action) => {
    setError("");
    const ce = studio();
    if (!ce) {
      setError("Studio runs inside the browser.");
      return;
    }
    Promise.resolve()
      .then(() => action(ce))
      .catch((reason) => setError(explain(reason)));
  };

  const needle = query.trim().toLowerCase();
  const shown = items.filter((item) => {
    if (!needle) return true;
    return `${item.fileName || ""} ${item.url || ""}`.toLowerCase().includes(needle);
  });

  return (
    <div id="studio-downloads" className="h-full overflow-auto bg-zinc-950 text-zinc-100">
      <div id="studio-downloads-body" className="mx-auto flex w-full max-w-3xl flex-col gap-4 px-5 py-8">
        <Brand
          id="studio-downloads-brand"
          titleId="studio-downloads-title"
          title="Downloads"
          detailId="studio-downloads-count"
          detail={`${items.length} downloads`}
          large
        />
        <SearchField id="studio-downloads-search" label="Filter downloads" value={query} onChange={setQuery} placeholder="Filter downloads" />
        {shown.length === 0 ? (
          <Empty id="studio-downloads-empty">{items.length === 0 ? "Nothing saved yet." : "No downloads match that filter."}</Empty>
        ) : (
          <ul id="studio-downloads-list" className="flex flex-col gap-2">
            {shown.map((item) => {
              const percent = downloadPercent(item);
              const running = item.state === "inProgress" || item.state === "paused";
              return (
                <li id={`studio-download-${item.id}`} key={item.id} className="rounded-2xl bg-zinc-900 p-4 ring-1 ring-zinc-800">
                  <div id={`studio-download-row-${item.id}`} className="flex items-start justify-between gap-3">
                    <div id={`studio-download-copy-${item.id}`} className="min-w-0">
                      <p id={`studio-download-name-${item.id}`} className="truncate text-sm font-medium text-zinc-50">
                        {item.fileName || item.url || "Download"}
                      </p>
                      <p id={`studio-download-meta-${item.id}`} className="mt-1 truncate text-xs text-zinc-500">
                        {downloadMeta(item)}
                      </p>
                    </div>
                    <span id={`studio-download-state-${item.id}`} className={`shrink-0 rounded-full px-2 py-0.5 text-[11px] font-medium ${downloadTone(item.state)}`}>
                      {downloadLabel(item.state)}
                    </span>
                  </div>
                  {percent !== null && item.state !== "completed" ? (
                    <div id={`studio-download-track-${item.id}`} className="mt-3 h-1 overflow-hidden rounded-full bg-zinc-800">
                      <div id={`studio-download-fill-${item.id}`} className="h-full rounded-full bg-indigo-400" style={{ width: `${percent}%` }} />
                    </div>
                  ) : null}
                  <div id={`studio-download-actions-${item.id}`} className="mt-3 flex flex-wrap gap-2">
                    {item.state === "inProgress" ? (
                      <button id={`studio-download-pause-${item.id}`} type="button" onClick={() => run((ce) => ce.downloads.pause(item.id))} className="rounded-lg bg-zinc-800 px-2.5 py-1 text-xs font-medium text-zinc-100 hover:bg-zinc-700">
                        Pause
                      </button>
                    ) : null}
                    {item.state === "paused" || (item.state === "interrupted" && item.canResume) ? (
                      <button id={`studio-download-resume-${item.id}`} type="button" onClick={() => run((ce) => ce.downloads.resume(item.id))} className="rounded-lg bg-indigo-500 px-2.5 py-1 text-xs font-medium text-white hover:bg-indigo-400">
                        Resume
                      </button>
                    ) : null}
                    {running ? (
                      <button id={`studio-download-cancel-${item.id}`} type="button" onClick={() => run((ce) => ce.downloads.cancel(item.id))} className="rounded-lg px-2.5 py-1 text-xs font-medium text-rose-200 hover:bg-rose-500/10">
                        Cancel
                      </button>
                    ) : null}
                    {item.state === "completed" ? (
                      <button id={`studio-download-open-${item.id}`} type="button" onClick={() => run((ce) => ce.downloads.open(item.id))} className="rounded-lg bg-indigo-500 px-2.5 py-1 text-xs font-medium text-white hover:bg-indigo-400">
                        Open
                      </button>
                    ) : null}
                    {item.state === "completed" ? (
                      <button id={`studio-download-show-${item.id}`} type="button" onClick={() => run((ce) => ce.downloads.show(item.id))} className="rounded-lg bg-zinc-800 px-2.5 py-1 text-xs font-medium text-zinc-100 hover:bg-zinc-700">
                        Show in folder
                      </button>
                    ) : null}
                  </div>
                </li>
              );
            })}
          </ul>
        )}
        <ErrorNote id="studio-downloads-error">{error}</ErrorNote>
      </div>
    </div>
  );
}

createRoot(document.getElementById("root")).render(<Downloads />);
