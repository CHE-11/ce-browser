import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";

function Popout() {
  const [count, setCount] = useState(0);
  useEffect(() => {
    document.title = "Studio";
    const paint = () => window.ce.tabs.list().then((tabs) => setCount(tabs.length));
    paint();
    return window.ce.tabs.onChanged(paint);
  }, []);
  return (
    <main id="studio-popout">
      <h1 id="studio-popout-title">Studio window</h1>
      <p id="studio-popout-count">{count} tabs open</p>
      <button id="studio-popout-home" type="button" onClick={() => window.ce.ui.openPage("home")}>
        Open the Studio page
      </button>
    </main>
  );
}

createRoot(document.getElementById("root")).render(<Popout />);
