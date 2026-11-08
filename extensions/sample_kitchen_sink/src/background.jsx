import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { studio } from "./format.js";
import { Mark } from "./ui.jsx";

function Background() {
  const [last, setLast] = useState("");
  useEffect(() => {
    document.title = "Studio background";
    const ce = studio();
    if (!ce) return;
    return ce.runtime.onMessage((message) => {
      ce.storage.set("lastPage", message);
      setLast(JSON.stringify(message, null, 2));
    });
  }, []);
  return (
    <main id="studio-background" className="min-h-full bg-zinc-950 p-4 text-zinc-100">
      <div id="studio-background-brand" className="flex items-center gap-2">
        <Mark id="studio-background-mark" small />
        <p id="studio-background-status" className="text-xs font-medium text-zinc-400">
          Listening for example.com
        </p>
      </div>
      <pre id="studio-background-last" className="mt-3 overflow-auto rounded-xl bg-zinc-900 p-3 font-mono text-xs leading-5 text-zinc-300 ring-1 ring-zinc-800">
        {last || "Waiting for a page."}
      </pre>
    </main>
  );
}

createRoot(document.getElementById("root")).render(<Background />);
