import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { hostOf } from "./model.js";

function Panel() {
  const [tabs, setTabs] = useState([]);
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
  const groups = new Map();
  for (const tab of tabs) {
    const host = hostOf(tab.url);
    groups.set(host, [...(groups.get(host) || []), tab]);
  }
  const hosts = [...groups.keys()].sort();
  return (
    <main id="tabs-panel">
      <h1 id="tabs-panel-title">By site</h1>
      <p id="tabs-error">{error}</p>
      {hosts.map((host) => (
        <section id={`tabs-host-${host.replace(/[^a-z0-9]+/gi, "-")}`} key={host} className="host">
          <h2 id={`tabs-host-name-${host.replace(/[^a-z0-9]+/gi, "-")}`}>{host}</h2>
          {groups.get(host).map((tab) => (
            <button id={`tabs-panel-open-${tab.id}`} key={tab.id} type="button" className="title" onClick={() => window.ce.tabs.activate(tab.id)}>
              {tab.title || host}
            </button>
          ))}
        </section>
      ))}
    </main>
  );
}

createRoot(document.getElementById("root")).render(<Panel />);
