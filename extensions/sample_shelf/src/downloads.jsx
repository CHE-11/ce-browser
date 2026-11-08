import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { DownloadList } from "./list.jsx";

function Page() {
  const [items, setItems] = useState([]);
  const [error, setError] = useState("");
  const load = () => window.ce.downloads.list().then(setItems, (reason) => setError(reason.message || String(reason)));
  useEffect(() => {
    document.title = "Shelf";
    load();
    return window.ce.downloads.onChanged(load);
  }, []);
  return (
    <main id="shelf-page">
      <h1 id="shelf-page-title">Downloads</h1>
      <p id="shelf-error">{error}</p>
      <DownloadList items={items} onChange={load} />
    </main>
  );
}

createRoot(document.getElementById("root")).render(<Page />);
