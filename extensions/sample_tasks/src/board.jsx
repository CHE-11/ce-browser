import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { addTask, loadTasks, moveTask } from "./model.js";

const columns = [
  ["today", "Today"],
  ["later", "Later"],
  ["done", "Done"],
];

function Board() {
  const [tasks, setTasks] = useState([]);
  const [title, setTitle] = useState("");
  const [error, setError] = useState("");
  useEffect(() => {
    document.title = "Tasks";
    loadTasks().then(setTasks, (reason) => setError(reason.message || String(reason)));
  }, []);
  return (
    <main id="tasks-board">
      <form
        id="tasks-board-form"
        style={{ gridColumn: "1 / -1" }}
        onSubmit={(event) => {
          event.preventDefault();
          const next = title.trim();
          if (!next) return;
          setTitle("");
          addTask(next).then(setTasks, (reason) => setError(reason.message || String(reason)));
        }}
      >
        <input id="tasks-board-input" value={title} placeholder="Add to today" onChange={(event) => setTitle(event.target.value)} />
        <button id="tasks-board-add" type="submit">Add</button>
      </form>
      <p id="error" style={{ gridColumn: "1 / -1" }}>{error}</p>
      {columns.map(([column, name]) => (
        <section id={`tasks-column-${column}`} key={column} className="column">
          <h2 id={`tasks-column-title-${column}`}>{name}</h2>
          {tasks
            .filter((task) => task.column === column)
            .map((task) => (
              <article id={`tasks-card-${task.id}`} key={task.id} className="card">
                <span id={`tasks-card-name-${task.id}`}>{task.title}</span>
                <div id={`tasks-card-moves-${task.id}`} className="row">
                  {columns
                    .filter(([id]) => id !== column)
                    .map(([id, label]) => (
                      <button
                        id={`tasks-card-move-${task.id}-${id}`}
                        key={id}
                        type="button"
                        className="quiet"
                        onClick={() => moveTask(task.id, id).then(setTasks, (reason) => setError(reason.message || String(reason)))}
                      >
                        {label}
                      </button>
                    ))}
                </div>
              </article>
            ))}
        </section>
      ))}
    </main>
  );
}

createRoot(document.getElementById("root")).render(<Board />);
