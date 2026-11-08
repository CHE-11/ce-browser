import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { explain, hostOf, studio } from "./format.js";
import { Brand, Empty, ErrorNote, SearchField, TabRow } from "./ui.jsx";

function Popout() {
  const [tabs, setTabs] = useState([]);
  const [query, setQuery] = useState("");
  const [error, setError] = useState("");

  useEffect(() => {
    document.title = "Studio";
    const ce = studio();
    if (!ce) {
      setError("Studio runs inside the browser.");
      return;
    }
    let live = true;
    const paint = () => {
      ce.tabs.list().then(
        (next) => {
          if (live) setTabs(Array.isArray(next) ? next : []);
        },
        (reason) => {
          if (live) setError(explain(reason));
        },
      );
    };
    paint();
    const stop = ce.tabs.onChanged(paint);
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
  const shown = tabs.filter((tab) => {
    if (!needle) return true;
    return `${tab.title || ""} ${hostOf(tab.url)} ${tab.url || ""}`.toLowerCase().includes(needle);
  });

  return (
    <main id="studio-popout" className="flex h-full flex-col bg-zinc-950 text-zinc-100">
      <div id="studio-popout-header" className="flex flex-col gap-3 px-4 pt-4 pb-3">
        <Brand id="studio-popout-brand" titleId="studio-popout-title" title="Studio" detailId="studio-popout-count" detail={`${tabs.length} tabs open`} />
        <SearchField id="studio-popout-search" label="Filter tabs" value={query} onChange={setQuery} placeholder="Filter tabs" />
      </div>
      <div id="studio-popout-list" className="min-h-0 flex-1 overflow-auto px-3 pb-3">
        {shown.length === 0 ? (
          <Empty id="studio-popout-empty">{tabs.length === 0 ? "No tabs are open." : "No tabs match that filter."}</Empty>
        ) : (
          <ul id="studio-popout-tabs" className="flex flex-col gap-1">
            {shown.map((tab) => (
              <TabRow
                key={tab.id}
                id={`studio-popout-tab-${tab.id}`}
                tab={tab}
                onOpen={() => run((ce) => ce.tabs.activate(tab.id))}
                onClose={() => run((ce) => ce.tabs.close(tab.id))}
              />
            ))}
          </ul>
        )}
      </div>
      <footer id="studio-popout-footer" className="flex flex-col gap-2 border-t border-zinc-800 p-4">
        <button
          id="studio-popout-home"
          type="button"
          onClick={() => run((ce) => ce.ui.openPage("home"))}
          className="rounded-xl bg-indigo-500 px-3 py-2.5 text-sm font-medium text-white hover:bg-indigo-400"
        >
          Open the Studio page
        </button>
        <button
          id="studio-popout-close"
          type="button"
          onClick={() => run((ce) => ce.ui.closePopout())}
          className="rounded-xl bg-zinc-900 px-3 py-2.5 text-sm font-medium text-zinc-200 ring-1 ring-zinc-800 hover:bg-zinc-800"
        >
          Close window
        </button>
        <ErrorNote id="studio-popout-error">{error}</ErrorNote>
      </footer>
    </main>
  );
}

createRoot(document.getElementById("root")).render(<Popout />);
