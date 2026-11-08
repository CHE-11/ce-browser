import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";

function Panel() {
  const [titles, setTitles] = useState([]);
  useEffect(() => {
    document.title = "Studio";
    const load = () => window.ce.tabs.list().then((tabs) => setTitles(tabs.map((tab) => tab.title || tab.url).slice(0, 8)));
    load();
    return window.ce.tabs.onChanged(load);
  }, []);
  return (
    <main id="studio-panel">
      <h1 id="studio-panel-title">Studio panel</h1>
      <ul id="studio-panel-tabs">
        {titles.map((title, index) => (
          <li id={`studio-panel-tab-${index}`} key={`${title}-${index}`}>{title}</li>
        ))}
      </ul>
    </main>
  );
}

createRoot(document.getElementById("root")).render(<Panel />);
