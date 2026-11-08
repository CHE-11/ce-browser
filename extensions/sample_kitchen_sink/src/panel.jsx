import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { explain, hostOf, studio } from "./format.js";
import { Brand, Empty, ErrorNote, SearchField, TabRow } from "./ui.jsx";

function Panel() {
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
    const load = () => {
      ce.tabs.list().then(
        (next) => {
          if (live) setTabs(Array.isArray(next) ? next : []);
        },
        (reason) => {
          if (live) setError(explain(reason));
        },
      );
    };
    load();
    const stop = ce.tabs.onChanged(load);
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
    <main id="studio-panel" className="flex h-full flex-col bg-zinc-950 text-zinc-100">
      <div id="studio-panel-header" className="flex flex-col gap-3 px-3 pt-3 pb-2">
        <Brand
          id="studio-panel-brand"
          titleId="studio-panel-title"
          title="Studio"
          detailId="studio-panel-count"
          detail={`${tabs.length} open`}
        />
        <SearchField id="studio-panel-search" label="Filter tabs" value={query} onChange={setQuery} placeholder="Filter tabs" />
      </div>
      <div id="studio-panel-list" className="min-h-0 flex-1 overflow-auto px-2 pb-3">
        {shown.length === 0 ? (
          <Empty id="studio-panel-empty">{tabs.length === 0 ? "No tabs are open." : "No tabs match that filter."}</Empty>
        ) : (
          <ul id="studio-panel-tabs" className="flex flex-col gap-1">
            {shown.map((tab, index) => (
              <TabRow
                key={tab.id}
                id={`studio-panel-tab-${index}`}
                tab={tab}
            onOpen={() => run((ce) => ce.tabs.activate(tab.id))}
            onClose={() => run((ce) => ce.tabs.close(tab.id))}
              />
            ))}
          </ul>
        )}
      </div>
      <div id="studio-panel-error-wrap" className="px-3 pb-3">
        <ErrorNote id="studio-panel-error">{error}</ErrorNote>
      </div>
    </main>
  );
}

createRoot(document.getElementById("root")).render(<Panel />);
