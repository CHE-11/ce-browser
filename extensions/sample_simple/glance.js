const surface = document.body.dataset.surface;

function label(tab) {
  return tab.title || tab.url || "New tab";
}

async function paint() {
  const tabs = await window.ce.tabs.list();
  const list = document.getElementById("glance-list");
  document.getElementById("glance-count").textContent = `${tabs.length} open`;
  list.replaceChildren();
  for (const tab of tabs) {
    const row = document.createElement("div");
    row.id = `glance-row-${tab.id}`;
    row.className = tab.active ? "row active" : "row";
    const open = document.createElement("button");
    open.id = `glance-open-${tab.id}`;
    open.type = "button";
    open.className = "open";
    open.textContent = label(tab);
    open.onclick = () => window.ce.tabs.activate(tab.id);
    row.append(open);
    if (surface !== "popup") {
      const close = document.createElement("button");
      close.id = `glance-close-${tab.id}`;
      close.type = "button";
      close.className = "close";
      close.textContent = "Close";
      close.onclick = () => window.ce.tabs.close(tab.id);
      row.append(close);
    }
    list.append(row);
  }
}

document.getElementById("glance-new").onclick = () => window.ce.tabs.create("");
const popout = document.getElementById("glance-popout");
if (popout) popout.onclick = () => window.ce.ui.openPopout();
document.getElementById("glance-reload").onclick = async () => {
  const tabs = await window.ce.tabs.list();
  const active = tabs.find((tab) => tab.active);
  if (active) await window.ce.tabs.reload(active.id);
};

window.ce.tabs.onChanged(() => {
  paint().catch(showError);
});
paint().catch(showError);

function showError(error) {
  document.getElementById("glance-error").textContent = error && error.message ? error.message : String(error);
}
