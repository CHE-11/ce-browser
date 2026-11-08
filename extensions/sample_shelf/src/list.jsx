import { percent } from "./model.js";

export function DownloadList({ items, onChange }) {
  const act = (method, id) => window.ce.downloads[method](id).then(onChange);
  return (
    <div id="shelf-list">
      {items.length === 0 ? <p id="shelf-empty">No downloads yet.</p> : null}
      {items.map((item) => (
        <article id={`shelf-item-${item.id}`} key={item.id} className="item">
          <p id={`shelf-name-${item.id}`} className="name">{item.fileName || item.url}</p>
          <div id={`shelf-meter-${item.id}`} className="meter">
            <span id={`shelf-meter-fill-${item.id}`} style={{ width: `${percent(item)}%` }} />
          </div>
          <div id={`shelf-actions-${item.id}`} className="row">
            {item.state === "inProgress" ? (
              <button id={`shelf-pause-${item.id}`} type="button" onClick={() => act("pause", item.id)}>Pause</button>
            ) : null}
            {item.canResume && item.state !== "inProgress" ? (
              <button id={`shelf-resume-${item.id}`} type="button" onClick={() => act("resume", item.id)}>Resume</button>
            ) : null}
            {item.state !== "completed" && item.state !== "cancelled" ? (
              <button id={`shelf-cancel-${item.id}`} type="button" onClick={() => act("cancel", item.id)}>Cancel</button>
            ) : null}
            {item.state === "completed" ? (
              <button id={`shelf-open-${item.id}`} type="button" onClick={() => act("open", item.id)}>Open</button>
            ) : null}
            <button id={`shelf-show-${item.id}`} type="button" onClick={() => act("show", item.id)}>Show</button>
          </div>
        </article>
      ))}
    </div>
  );
}
