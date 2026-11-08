import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";

function Home() {
  const [count, setCount] = useState(0);
  useEffect(() => {
    document.title = "Studio";
    window.ce.bookmarks.list().then((tree) => {
      const walk = (node) => (node && node.url ? 1 : 0) + ((node && node.children) || []).reduce((sum, child) => sum + walk(child), 0);
      setCount(walk(tree.bar) + walk(tree.other));
    });
  }, []);
  return (
    <main id="studio-home">
      <h1 id="studio-home-title">Studio</h1>
      <p id="studio-home-body">This page, the toolbar popup, the side panel, the bookmarks bar, and the downloads page all belong to one host extension.</p>
      <p id="studio-bookmark-count">{count} bookmarks</p>
    </main>
  );
}

createRoot(document.getElementById("root")).render(<Home />);
