import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { addTask, loadTasks, moveTask } from "./model.js";

const columns = [
  ["today", "Today"],
  ["later", "Later"],
  ["done", "Done"],
];

function Panel() {
  const [tasks, setTasks] = useState([]);
  const [title, setTitle] = useState("");
  const [filter, setFilter] = useState("today");
  const [error, setError] = useState("");
  useEffect(() => {
    document.title = "Tasks";
    loadTasks().then(setTasks, (reason) => setError(reason.message || String(reason)));
  }, []);
  const visible = tasks.filter((task) => task.column === filter);
  return (
    <main id="tasks-panel">
      <h1 id="tasks-panel-title">Tasks</h1>
      <form
        id="tasks-panel-form"
        onSubmit={(event) => {
          event.preventDefault();
          const next = title.trim();
          if (!next) return;
          setTitle("");
          addTask(next).then(setTasks, (reason) => setError(reason.message || String(reason)));
        }}
      >
        <input id="tasks-panel-input" value={title} placeholder="Add a task" onChange={(event) => setTitle(event.target.value)} />
        <button id="tasks-panel-add" type="submit">Add</button>
      </form>
      <div id="tasks-panel-filters" className="row">
        {columns.map(([id, name]) => (
          <button id={`tasks-filter-${id}`} key={id} type="button" className={filter === id ? "" : "quiet"} onClick={() => setFilter(id)}>
            {name}
          </button>
        ))}
      </div>
      <p id="error">{error}</p>
      <ul id="tasks-panel-list">
        {visible.map((task) => (
          <li id={`tasks-panel-item-${task.id}`} key={task.id}>
            <span id={`tasks-panel-name-${task.id}`}>{task.title}</span>
            <div id={`tasks-panel-moves-${task.id}`} className="row">
              {columns.map(([id, name]) => (
                <button
                  id={`tasks-move-${task.id}-${id}`}
                  key={id}
                  type="button"
                  className="quiet"
                  onClick={() => moveTask(task.id, id).then(setTasks, (reason) => setError(reason.message || String(reason)))}
                >
                  {name}
                </button>
              ))}
            </div>
          </li>
        ))}
      </ul>
    </main>
  );
}

createRoot(document.getElementById("root")).render(<Panel />);
