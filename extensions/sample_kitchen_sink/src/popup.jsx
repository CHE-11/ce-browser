import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { explain, hostOf, studio } from "./format.js";
import { ActionButton, Brand, ErrorNote, Icon, Stat } from "./ui.jsx";

function Popup() {
  const [count, setCount] = useState(0);
  const [lastPage, setLastPage] = useState(null);
  const [error, setError] = useState("");

  useEffect(() => {
    document.title = "Studio";
    const ce = studio();
    if (!ce) {
      setError("Studio runs inside the browser.");
      return;
    }
    ce.tabs.list().then(
      (tabs) => setCount(Array.isArray(tabs) ? tabs.length : 0),
      (reason) => setError(explain(reason)),
    );
    ce.storage.get("lastPage").then((value) => setLastPage(value));
  }, []);

  const run = (action) => {
    setError("");
    const ce = studio();
    if (!ce) {
      setError("Studio runs inside the browser.");
      return;
    }
    Promise.resolve()
      .then(() => action(ce))
      .catch((reason) => setError(explain(reason)));
  };

  const lastTitle = lastPage && lastPage.title ? lastPage.title : "";
  const lastHost = lastPage && lastPage.href ? hostOf(lastPage.href) : "";

  return (
    <main id="studio-popup" className="flex flex-col gap-3 bg-zinc-950 p-3 text-zinc-100">
      <Brand id="studio-popup-brand" titleId="studio-popup-title" title="Studio" detailId="studio-popup-kicker" detail="Workspace" />
      <section id="studio-popup-stats" className="grid grid-cols-2 gap-2">
        <Stat id="studio-popup-tabs" label="Tabs open" valueId="studio-tab-count" value={count} />
        <article id="studio-popup-last-card" className="min-w-0 rounded-2xl bg-zinc-900 px-3 py-3 ring-1 ring-zinc-800">
          <p id="studio-popup-last-label" className="text-[10px] font-medium tracking-[0.16em] text-zinc-500 uppercase">
            Last page
          </p>
          <p id="studio-popup-last-host" className="mt-1 truncate text-sm font-medium text-zinc-50">
            {lastHost || "None yet"}
          </p>
        </article>
      </section>
      <p id="studio-last" className="rounded-2xl bg-zinc-900 px-3 py-2.5 text-xs leading-5 text-zinc-300 ring-1 ring-zinc-800">
        {lastTitle ? `Last example.com page: ${lastTitle}` : "Open example.com to record a page."}
      </p>
      <div id="studio-actions" className="flex flex-col gap-2">
        <ActionButton
          id="studio-open-panel"
          primary
          icon={<Icon id="studio-open-panel-glyph" name="panel" />}
          label="Open panel"
          hint="Tabs beside the page"
          onClick={() => run((ce) => ce.ui.togglePanel())}
        />
        <ActionButton
          id="studio-open-page"
          icon={<Icon id="studio-open-page-glyph" name="page" />}
          label="Open page"
          hint="Studio home"
          onClick={() => run((ce) => ce.ui.openPage("home"))}
        />
        <ActionButton
          id="studio-open-popout"
          icon={<Icon id="studio-open-popout-glyph" name="window" />}
          label="Open window"
          hint="A floating Studio window"
          onClick={() => run((ce) => ce.ui.openPopout())}
        />
      </div>
      <ErrorNote id="studio-error">{error}</ErrorNote>
    </main>
  );
}

createRoot(document.getElementById("root")).render(<Popup />);
