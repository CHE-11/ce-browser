import { useEffect } from "react";
import { createRoot } from "react-dom/client";

function Bar() {
  useEffect(() => {
    document.title = "Studio";
  }, []);
  return (
    <div id="studio-bar">
      <span id="studio-bar-label">Studio</span>
      <button id="studio-bar-open" type="button" onClick={() => window.ce.ui.openPage("home")}>Open page</button>
    </div>
  );
}

createRoot(document.getElementById("root")).render(<Bar />);
