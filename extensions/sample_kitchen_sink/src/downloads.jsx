import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";

function Downloads() {
  const [items, setItems] = useState([]);
  useEffect(() => {
    document.title = "Studio downloads";
    const load = () => window.ce.downloads.list().then(setItems);
    load();
    return window.ce.downloads.onChanged(load);
  }, []);
  return (
    <main id="studio-downloads">
      <h1 id="studio-downloads-title">Studio downloads</h1>
      <p id="studio-downloads-count">{items.length} downloads</p>
      <ul id="studio-downloads-list">
        {items.slice(0, 20).map((item) => (
          <li id={`studio-download-${item.id}`} key={item.id}>{item.fileName || item.url}</li>
        ))}
      </ul>
    </main>
  );
}

createRoot(document.getElementById("root")).render(<Downloads />);
