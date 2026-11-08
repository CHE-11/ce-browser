import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { DownloadList } from "./list.jsx";

function Popup() {
  const [items, setItems] = useState([]);
  const [error, setError] = useState("");
  const load = () => window.ce.downloads.list().then(setItems, (reason) => setError(reason.message || String(reason)));
  useEffect(() => {
    document.title = "Shelf";
    load();
    return window.ce.downloads.onChanged(load);
  }, []);
  const active = items.filter((item) => item.state === "inProgress" || item.state === "paused");
  return (
    <main id="shelf-popup">
      <h1 id="shelf-popup-title">Shelf</h1>
      <p id="shelf-count">{active.length} in progress</p>
      <p id="shelf-error">{error}</p>
      <DownloadList items={items.slice(0, 4)} onChange={load} />
    </main>
  );
}

createRoot(document.getElementById("root")).render(<Popup />);
