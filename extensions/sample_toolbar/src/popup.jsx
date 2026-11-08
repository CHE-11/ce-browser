import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { addLink, loadLinks, removeLink } from "./model.js";

function Popup() {
  const [links, setLinks] = useState([]);
  const [label, setLabel] = useState("");
  const [url, setUrl] = useState("");
  const [error, setError] = useState("");
  useEffect(() => {
    document.title = "Launch";
    loadLinks().then(setLinks, (reason) => setError(reason.message || String(reason)));
  }, []);
  return (
    <main id="launch-popup">
      <h1 id="launch-title">Launch</h1>
      <form
        id="launch-form"
        onSubmit={(event) => {
          event.preventDefault();
          const nextLabel = label.trim();
          const nextUrl = url.trim();
          if (!nextLabel || !nextUrl) return;
          setLabel("");
          setUrl("");
          addLink(nextLabel, nextUrl).then(setLinks, (reason) => setError(reason.message || String(reason)));
        }}
      >
        <input id="launch-label" value={label} placeholder="Name" onChange={(event) => setLabel(event.target.value)} />
        <input id="launch-url" value={url} placeholder="https://" onChange={(event) => setUrl(event.target.value)} />
        <button id="launch-add" type="submit">Save link</button>
      </form>
      <p id="launch-error">{error}</p>
      <div id="launch-list">
        {links.map((link) => (
          <div id={`launch-row-${link.id}`} key={link.id} className="link">
            <button id={`launch-open-${link.id}`} type="button" onClick={() => window.ce.tabs.create(link.url)}>
              {link.label}
            </button>
            <button id={`launch-remove-${link.id}`} type="button" onClick={() => removeLink(link.id).then(setLinks)}>
              Remove
            </button>
          </div>
        ))}
      </div>
    </main>
  );
}

createRoot(document.getElementById("root")).render(<Popup />);
