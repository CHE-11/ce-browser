import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { addTask, loadTasks } from "./model.js";

function Popup() {
  const [tasks, setTasks] = useState([]);
  const [title, setTitle] = useState("");
  const [error, setError] = useState("");
  useEffect(() => {
    document.title = "Tasks";
    loadTasks().then(setTasks, (reason) => setError(reason.message || String(reason)));
  }, []);
  const open = tasks.filter((task) => task.column !== "done").slice(0, 6);
  return (
    <main id="tasks-popup">
      <h1 id="tasks-popup-title">Tasks</h1>
      <form
        id="tasks-add-form"
        onSubmit={(event) => {
          event.preventDefault();
          const next = title.trim();
          if (!next) return;
          setTitle("");
          addTask(next).then(setTasks, (reason) => setError(reason.message || String(reason)));
        }}
      >
        <input id="tasks-add-input" value={title} placeholder="Add a task" onChange={(event) => setTitle(event.target.value)} />
        <button id="tasks-add-button" type="submit">Add</button>
      </form>
      <p id="error">{error}</p>
      <ul id="tasks-popup-list">
        {open.map((task) => (
          <li id={`tasks-popup-item-${task.id}`} key={task.id}>
            {task.title}
          </li>
        ))}
      </ul>
      <button id="tasks-open-board" type="button" onClick={() => window.ce.ui.openPage("board")}>
        Open board
      </button>
    </main>
  );
}

createRoot(document.getElementById("root")).render(<Popup />);
