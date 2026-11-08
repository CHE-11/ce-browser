import { hostOf } from "./format.js";

export function Mark({ id, small = false }) {
  return (
    <span
      id={id}
      className={
        small
          ? "grid size-3.5 shrink-0 grid-cols-2 gap-px rounded-[3px] bg-indigo-400/20 p-px"
          : "grid size-8 shrink-0 grid-cols-2 gap-0.5 rounded-xl bg-indigo-500/15 p-1.5 ring-1 ring-indigo-400/30"
      }
    >
      <span id={`${id}-a`} className="rounded-[1px] bg-indigo-100" />
      <span id={`${id}-b`} className="rounded-[1px] bg-indigo-300" />
      <span id={`${id}-c`} className="col-span-2 rounded-[1px] bg-indigo-200" />
    </span>
  );
}

export function Brand({ id, titleId, title, detailId, detail, large = false }) {
  return (
    <header id={id} className="flex min-w-0 items-center gap-3">
      <Mark id={`${id}-mark`} />
      <div id={`${id}-text`} className="min-w-0">
        <h1
          id={titleId}
          className={
            large
              ? "truncate text-2xl font-semibold tracking-tight text-zinc-50"
              : "truncate text-sm font-semibold tracking-tight text-zinc-50"
          }
        >
          {title}
        </h1>
        {detail ? (
          <p id={detailId} className="truncate text-xs text-zinc-400">
            {detail}
          </p>
        ) : null}
      </div>
    </header>
  );
}

export function Icon({ id, name }) {
  return (
    <svg
      id={id}
      viewBox="0 0 16 16"
      className="size-4"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.4"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {name === "panel" ? (
        <g id={`${id}-panel`}>
          <rect id={`${id}-frame`} x="1.5" y="2.5" width="13" height="11" rx="1.6" />
          <path id={`${id}-split`} d="M10 2.5v11" />
        </g>
      ) : null}
      {name === "page" ? (
        <g id={`${id}-page`}>
          <path id={`${id}-sheet`} d="M4 2.5h5.2L13 6.2V13.5H4z" />
          <path id={`${id}-fold`} d="M9.2 2.7V6.2H13" />
        </g>
      ) : null}
      {name === "window" ? (
        <g id={`${id}-window`}>
          <rect id={`${id}-window-frame`} x="2" y="3" width="12" height="10" rx="1.6" />
          <path id={`${id}-window-bar`} d="M2 6.2h12" />
        </g>
      ) : null}
      {name === "download" ? (
        <g id={`${id}-download`}>
          <path id={`${id}-stem`} d="M8 2.4v7" />
          <path id={`${id}-arrow`} d="M5.4 7.1 8 9.7l2.6-2.6" />
          <path id={`${id}-base`} d="M3.2 13.2h9.6" />
        </g>
      ) : null}
    </svg>
  );
}

export function ActionButton({ id, icon, label, hint, onClick, primary = false }) {
  return (
    <button
      id={id}
      type="button"
      onClick={onClick}
      className={
        primary
          ? "flex w-full items-center gap-3 rounded-2xl bg-indigo-500 px-3 py-3 text-left text-white transition hover:bg-indigo-400 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-200"
          : "flex w-full items-center gap-3 rounded-2xl bg-zinc-900 px-3 py-3 text-left text-zinc-100 ring-1 ring-zinc-800 transition hover:bg-zinc-800 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-400"
      }
    >
      <span id={`${id}-icon`} className={primary ? "text-white" : "text-indigo-300"}>
        {icon}
      </span>
      <span id={`${id}-copy`} className="min-w-0 flex-1">
        <span id={`${id}-label`} className="block text-sm font-medium">
          {label}
        </span>
        {hint ? (
          <span id={`${id}-hint`} className={primary ? "mt-0.5 block text-[11px] text-indigo-100" : "mt-0.5 block text-[11px] text-zinc-500"}>
            {hint}
          </span>
        ) : null}
      </span>
      <span id={`${id}-chevron`} className={primary ? "text-lg leading-none text-indigo-100" : "text-lg leading-none text-zinc-600"} aria-hidden="true">
        ›
      </span>
    </button>
  );
}

export function Monogram({ id, label }) {
  const letter = (label || "?").trim().charAt(0).toUpperCase() || "?";
  return (
    <span id={id} className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-zinc-800 text-xs font-semibold text-indigo-200">
      {letter}
    </span>
  );
}

export function TabRow({ id, tab, onOpen, onClose }) {
  const host = hostOf(tab.url);
  const title = tab.title || host || "New tab";
  return (
    <li id={id} className={tab.active ? "flex items-center gap-1 rounded-xl bg-indigo-500/10 pr-1 ring-1 ring-indigo-400/25" : "flex items-center gap-1 rounded-xl pr-1 hover:bg-zinc-900"}>
      <button id={`${id}-open`} type="button" onClick={onOpen} className="flex min-w-0 flex-1 items-center gap-3 rounded-xl px-2 py-2 text-left">
        <Monogram id={`${id}-mark`} label={host || title} />
        <span id={`${id}-copy`} className="min-w-0 flex-1">
          <span id={`${id}-name`} className="block truncate text-sm font-medium text-zinc-100">
            {title}
          </span>
          <span id={`${id}-host`} className="block truncate text-[11px] text-zinc-500">
            {host || "No address"}
          </span>
        </span>
        {tab.active ? <span id={`${id}-active`} className="size-1.5 shrink-0 rounded-full bg-indigo-400" /> : null}
      </button>
      <button
        id={`${id}-close`}
        type="button"
        aria-label={`Close ${title}`}
        onClick={onClose}
        className="rounded-lg px-2 py-1 text-base leading-none text-zinc-500 hover:bg-zinc-800 hover:text-zinc-100"
      >
        <span id={`${id}-close-glyph`} aria-hidden="true">
          ×
        </span>
      </button>
    </li>
  );
}

export function SearchField({ id, label, value, onChange, placeholder }) {
  return (
    <label id={`${id}-label`} className="block">
      <span id={`${id}-caption`} className="sr-only">
        {label}
      </span>
      <input
        id={id}
        type="search"
        value={value}
        placeholder={placeholder}
        onChange={(event) => onChange(event.target.value)}
        className="w-full rounded-xl bg-zinc-900 px-3 py-2 text-sm text-zinc-100 ring-1 ring-zinc-800 outline-none placeholder:text-zinc-600 focus:ring-indigo-400/70"
      />
    </label>
  );
}

export function Empty({ id, children }) {
  return (
    <p id={id} className="rounded-2xl border border-dashed border-zinc-800 px-4 py-8 text-center text-sm text-zinc-500">
      {children}
    </p>
  );
}

export function ErrorNote({ id, children }) {
  return (
    <p id={id} className={children ? "rounded-xl bg-rose-500/10 px-3 py-2 text-xs text-rose-200 ring-1 ring-rose-400/30" : "hidden"}>
      {children}
    </p>
  );
}

export function Stat({ id, label, valueId, value }) {
  return (
    <article id={id} className="min-w-0 rounded-2xl bg-zinc-900 px-3 py-3 ring-1 ring-zinc-800">
      <p id={`${id}-label`} className="text-[10px] font-medium tracking-[0.16em] text-zinc-500 uppercase">
        {label}
      </p>
      <p id={valueId} className="mt-1 text-2xl font-semibold tracking-tight text-zinc-50 tabular-nums">
        {value}
      </p>
    </article>
  );
}
