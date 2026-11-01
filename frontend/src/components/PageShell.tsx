import { Fragment, useEffect, type ReactNode } from "react";
import { Bookmark, Download, Folder, History, Puzzle, Settings } from "lucide-react";
import { cn } from "../lib/cn";
import { goToPage } from "../lib/route";
import { useAppearanceFromSettings } from "../lib/use-browser";

export type PageOutlineItem = {
  id: string;
  label: string;
  active?: boolean;
};

const NAV = [
  { page: "bookmarks", label: "Bookmarks", icon: Bookmark },
  { page: "downloads", label: "Downloads", icon: Download },
  { page: "files", label: "Files", icon: Folder },
  { page: "history", label: "History", icon: History },
  { page: "extensions", label: "Extensions", icon: Puzzle },
  { page: "settings", label: "Settings", icon: Settings },
] as const;

type PageShellProps = {
  id: string;
  page: (typeof NAV)[number]["page"];
  title: string;
  actions?: ReactNode;
  outline?: PageOutlineItem[];
  lockHorizontalScroll?: boolean;
  children: ReactNode;
};

export function PageShell({ id, page, title, actions, outline, lockHorizontalScroll = false, children }: PageShellProps) {
  useAppearanceFromSettings();

  useEffect(() => {
    document.title = title;
  }, [title]);

  useEffect(() => {
    if (!lockHorizontalScroll) {
      return undefined;
    }
    const targets = [document.documentElement, document.body];
    const previous = targets.map((node) => ({ overflowX: node.style.overflowX, overscrollBehaviorX: node.style.overscrollBehaviorX }));
    for (const node of targets) {
      node.style.overflowX = "hidden";
      node.style.overscrollBehaviorX = "none";
    }
    return () => {
      targets.forEach((node, index) => {
        node.style.overflowX = previous[index].overflowX;
        node.style.overscrollBehaviorX = previous[index].overscrollBehaviorX;
      });
    };
  }, [lockHorizontalScroll]);

  const scrollLock = lockHorizontalScroll && "overflow-x-hidden overscroll-x-none";

  return (
    <div id={id} className={cn("flex h-full bg-zinc-950 text-zinc-100", lockHorizontalScroll && "overflow-hidden")}>
      <nav id={`${id}-nav`} className={cn("flex w-56 shrink-0 flex-col gap-1 overflow-y-auto border-r border-zinc-800 p-3", scrollLock)}>
        <div id={`${id}-nav-brand`} className="px-3 pb-3 pt-1 text-sm font-semibold text-zinc-400">
          ce-browser
        </div>
        {NAV.map((item) => (
          <Fragment key={item.page}>
            <button
              id={`${id}-nav-${item.page}`}
              type="button"
              className={cn(
                "flex h-9 items-center gap-3 rounded-md px-3 text-left text-sm text-zinc-300 hover:bg-zinc-900 hover:text-zinc-100",
                item.page === page && "bg-sky-950 text-sky-300 hover:bg-sky-950",
              )}
              onClick={() => goToPage(item.page)}
            >
              <item.icon id={`${id}-nav-${item.page}-icon`} className="size-4" aria-hidden />
              {item.label}
            </button>
            {item.page === page && outline && outline.length > 0 ? (
              <div id={`${id}-nav-outline`} className="mb-1 ml-4 flex flex-col gap-0.5 border-l border-zinc-800 py-0.5 pl-2">
                {outline.map((entry) => (
                  <button
                    key={entry.id}
                    id={`${id}-nav-outline-${entry.id}`}
                    type="button"
                    title={entry.label}
                    aria-current={entry.active ? "location" : undefined}
                    className={cn(
                      "truncate rounded-md px-2 py-1 text-left text-xs text-zinc-400 hover:bg-zinc-900 hover:text-zinc-100",
                      entry.active && "bg-sky-950 text-sky-300 hover:bg-sky-950",
                    )}
                    onClick={() => document.getElementById(entry.id)?.scrollIntoView({ behavior: "smooth", block: "start" })}
                  >
                    {entry.label}
                  </button>
                ))}
              </div>
            ) : null}
          </Fragment>
        ))}
      </nav>
      <main id={`${id}-main`} className="flex min-w-0 flex-1 flex-col">
        <header id={`${id}-header`} className="flex h-16 shrink-0 items-center justify-between gap-4 border-b border-zinc-800 px-8">
          <h1 id={`${id}-title`} className="text-xl font-semibold">
            {title}
          </h1>
          <div id={`${id}-actions`} className="flex items-center gap-2">
            {actions}
          </div>
        </header>
        <div id={`${id}-body`} className={cn("min-h-0 flex-1 overflow-y-auto px-8 py-6", scrollLock)}>
          {children}
        </div>
      </main>
    </div>
  );
}
