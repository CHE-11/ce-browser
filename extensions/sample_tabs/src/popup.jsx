import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { hostOf } from "./model.js";

function Popup() {
  const [tabs, setTabs] = useState([]);
  const [query, setQuery] = useState("");
  const [error, setError] = useState("");
  useEffect(() => {
    document.title = "Tab shelf";
    let alive = true;
    const load = () =>
      window.ce.tabs.list().then(
        (value) => {
          if (alive) setTabs(value);
        },
        (reason) => {
          if (alive) setError(reason.message || String(reason));
        },
      );
    load();
    const stop = window.ce.tabs.onChanged(load);
    return () => {
      alive = false;
      stop();
    };
  }, []);
  const needle = query.trim().toLowerCase();
  const shown = tabs.filter((tab) => !needle || `${tab.title} ${tab.url}`.toLowerCase().includes(needle));
  return (
    <main id="tabs-popup">
      <h1 id="tabs-popup-title">Tab shelf</h1>
      <input id="tabs-search" value={query} placeholder="Search tabs" onChange={(event) => setQuery(event.target.value)} />
      <p id="tabs-error">{error}</p>
      <div id="tabs-popup-list">
        {shown.map((tab) => (
          <article id={`tabs-popup-item-${tab.id}`} key={tab.id} className={tab.active ? "tab active" : "tab"}>
            <div id={`tabs-popup-row-${tab.id}`} className="row">
              <button id={`tabs-popup-open-${tab.id}`} type="button" className="title" onClick={() => window.ce.tabs.activate(tab.id)}>
                {tab.title || hostOf(tab.url)}
              </button>
              <button id={`tabs-popup-close-${tab.id}`} type="button" onClick={() => window.ce.tabs.close(tab.id)}>
                Close
              </button>
            </div>
            <p id={`tabs-popup-url-${tab.id}`} className="url">{tab.url}</p>
          </article>
        ))}
      </div>
      <button id="tabs-open-panel" type="button" onClick={() => window.ce.ui.togglePanel()}>
        Group by site
      </button>
    </main>
  );
}

createRoot(document.getElementById("root")).render(<Popup />);
