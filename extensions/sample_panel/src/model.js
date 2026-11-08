const OUTLINE = `(() => {
  const selection = String(getSelection() || "").trim().slice(0, 600);
  const headings = [...document.querySelectorAll("h1,h2,h3")].slice(0, 16).map((node) => ({
    tag: node.tagName.toLowerCase(),
    text: (node.textContent || "").trim().slice(0, 160)
  })).filter((heading) => heading.text);
  return { title: document.title, selection, headings };
})()`;

export async function activeTab() {
  const tabs = await window.ce.tabs.list();
  return tabs.find((tab) => tab.active) || null;
}

export async function outlinePage() {
  const tab = await activeTab();
  if (!tab) return { title: "", selection: "", headings: [] };
  return window.ce.page.executeScript(tab.id, OUTLINE);
}

export async function capturePage() {
  const tab = await activeTab();
  if (!tab) return "";
  const shot = await window.ce.page.capture(tab.id);
  return shot.dataUrl || "";
}
