const KEY = "tasks";

export async function loadTasks() {
  const value = await window.ce.storage.get(KEY);
  return Array.isArray(value) ? value : [];
}

async function write(tasks) {
  await window.ce.storage.set(KEY, tasks);
  return tasks;
}

export async function addTask(title) {
  const tasks = await loadTasks();
  tasks.unshift({ id: crypto.randomUUID(), title, column: "today" });
  return write(tasks);
}

export async function moveTask(id, column) {
  const tasks = await loadTasks();
  return write(tasks.map((task) => (task.id === id ? { ...task, column } : task)));
}
