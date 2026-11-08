const KEY = "clips";

export async function loadClips() {
  const value = await window.ce.storage.get(KEY);
  return Array.isArray(value) ? value : [];
}

export async function addClip(text) {
  const clips = await loadClips();
  clips.unshift({ id: crypto.randomUUID(), text, createdAt: Date.now() });
  await window.ce.storage.set(KEY, clips);
  return clips;
}

export async function removeClip(id) {
  const clips = (await loadClips()).filter((clip) => clip.id !== id);
  await window.ce.storage.set(KEY, clips);
  return clips;
}
