const KEY = "links";

export async function loadLinks() {
  const value = await window.ce.storage.get(KEY);
  return Array.isArray(value) ? value : [];
}

export async function addLink(label, url) {
  const links = await loadLinks();
  links.unshift({ id: crypto.randomUUID(), label, url });
  await window.ce.storage.set(KEY, links);
  return links;
}

export async function removeLink(id) {
  const links = (await loadLinks()).filter((link) => link.id !== id);
  await window.ce.storage.set(KEY, links);
  return links;
}
