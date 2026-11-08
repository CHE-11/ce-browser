import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { loadClips, removeClip } from "./model.js";

function Library() {
  const [clips, setClips] = useState([]);
  const [error, setError] = useState("");
  useEffect(() => {
    document.title = "Clips";
    loadClips().then(setClips, (reason) => setError(reason.message || String(reason)));
  }, []);
  return (
    <main id="clips-library">
      <h1 id="clips-library-title">Clips</h1>
      <p id="clips-error">{error}</p>
      {clips.length === 0 ? <p id="clips-empty">Nothing saved yet.</p> : null}
      {clips.map((clip) => (
        <article id={`clips-item-${clip.id}`} key={clip.id}>
          <p id={`clips-text-${clip.id}`}>{clip.text}</p>
          <button id={`clips-remove-${clip.id}`} type="button" onClick={() => removeClip(clip.id).then(setClips)}>
            Remove
          </button>
        </article>
      ))}
    </main>
  );
}

createRoot(document.getElementById("root")).render(<Library />);
