import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";

function Popup() {
  const [count, setCount] = useState(0);
  const [lastPage, setLastPage] = useState(null);
  const [error, setError] = useState("");
  useEffect(() => {
    document.title = "Studio";
    window.ce.tabs.list().then(
      (tabs) => setCount(tabs.length),
      (reason) => setError(reason.message || String(reason)),
    );
    window.ce.storage.get("lastPage").then((value) => setLastPage(value));
  }, []);
  return (
    <main id="studio-popup">
      <h1 id="studio-popup-title">Studio</h1>
      <p id="studio-tab-count">{count} tabs open</p>
      <p id="studio-last">{lastPage && lastPage.title ? `Last example.com page: ${lastPage.title}` : "Open example.com to record a page."}</p>
      <div id="studio-actions">
        <button id="studio-open-panel" type="button" onClick={() => window.ce.ui.togglePanel()}>Open panel</button>
        <button id="studio-open-page" type="button" onClick={() => window.ce.ui.openPage("home")}>Open page</button>
        <button id="studio-open-popout" type="button" onClick={() => window.ce.ui.openPopout()}>Open window</button>
      </div>
      <p id="studio-error">{error}</p>
    </main>
  );
}

createRoot(document.getElementById("root")).render(<Popup />);
