import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { capturePage } from "./model.js";

function Popup() {
  const [shot, setShot] = useState("");
  const [error, setError] = useState("");
  useEffect(() => {
    document.title = "Outline";
  }, []);
  return (
    <main id="lens-popup">
      <h1 id="lens-popup-title">Outline</h1>
      <div id="lens-actions">
        <button id="lens-outline" type="button" onClick={() => window.ce.ui.togglePanel()}>
          Outline
        </button>
        <button
          id="lens-shot"
          type="button"
          onClick={() => capturePage().then(setShot, (reason) => setError(reason.message || String(reason)))}
        >
          Shot
        </button>
      </div>
      <p id="lens-error">{error}</p>
      {shot ? <img id="lens-preview" src={shot} alt="Visible area of the active tab" /> : null}
    </main>
  );
}

createRoot(document.getElementById("root")).render(<Popup />);
