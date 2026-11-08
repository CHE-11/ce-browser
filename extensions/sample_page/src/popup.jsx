import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { addClip } from "./model.js";

function Popup() {
  const [text, setText] = useState("");
  const [error, setError] = useState("");
  useEffect(() => {
    document.title = "Clips";
  }, []);
  return (
    <main id="clips-popup">
      <h1 id="clips-popup-title">Clips</h1>
      <form
        id="clips-form"
        onSubmit={(event) => {
          event.preventDefault();
          const next = text.trim();
          if (!next) return;
          setText("");
          addClip(next).then(
            () => window.ce.ui.openPage("library"),
            (reason) => setError(reason.message || String(reason)),
          );
        }}
      >
        <textarea id="clips-input" value={text} placeholder="Write a clip" onChange={(event) => setText(event.target.value)} />
        <button id="clips-save" type="submit">Save and open</button>
      </form>
      <p id="clips-error">{error}</p>
    </main>
  );
}

createRoot(document.getElementById("root")).render(<Popup />);
