import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { outlinePage } from "./model.js";

function Panel() {
  const [page, setPage] = useState({ title: "", selection: "", headings: [] });
  const [error, setError] = useState("");
  const load = () => outlinePage().then(setPage, (reason) => setError(reason.message || String(reason)));
  useEffect(() => {
    document.title = "Outline";
    load();
  }, []);
  return (
    <main id="lens-panel">
      <h1 id="lens-panel-title">{page.title || "Outline"}</h1>
      <div id="lens-panel-actions">
        <button id="lens-refresh" type="button" onClick={load}>Read page</button>
      </div>
      <p id="lens-error">{error}</p>
      {page.selection ? <blockquote id="lens-selection">{page.selection}</blockquote> : null}
      <ol id="lens-headings">
        {(page.headings || []).map((heading, index) => (
          <li id={`lens-heading-${index}`} key={`${heading.tag}-${index}`}>
            <small id={`lens-heading-tag-${index}`}>{heading.tag}</small> {heading.text}
          </li>
        ))}
      </ol>
    </main>
  );
}

createRoot(document.getElementById("root")).render(<Panel />);
