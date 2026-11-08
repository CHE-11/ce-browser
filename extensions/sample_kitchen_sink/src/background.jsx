import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";

function Background() {
  const [last, setLast] = useState("");
  useEffect(() => {
    document.title = "Studio background";
    return window.ce.runtime.onMessage((message) => {
      window.ce.storage.set("lastPage", message);
      setLast(JSON.stringify(message));
    });
  }, []);
  return (
    <main id="studio-background">
      <p id="studio-background-last">{last}</p>
    </main>
  );
}

createRoot(document.getElementById("root")).render(<Background />);
