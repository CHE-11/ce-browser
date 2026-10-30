import { useEffect, type ReactNode } from "react";
import { Bookmark, Download, History, Puzzle, Settings } from "lucide-react";
import { cn } from "../lib/cn";
import { goToPage } from "../lib/route";
import { useAppearanceFromSettings } from "../lib/use-browser";

const NAV = [
  { page: "bookmarks", label: "Bookmarks", icon: Bookmark },
  { page: "downloads", label: "Downloads", icon: Download },
  { page: "history", label: "History", icon: History },
  { page: "extensions", label: "Extensions", icon: Puzzle },
  { page: "settings", label: "Settings", icon: Settings },
] as const;

type PageShellProps = {
  id: string;
  page: (typeof NAV)[number]["page"];
  title: string;
  actions?: ReactNode;
  children: ReactNode;
};

export function PageShell({ id, page, title, actions, children }: PageShellProps) {
  useAppearanceFromSettings();

  useEffect(() => {
    document.title = title;
  }, [title]);

  return (
    <div id={id} className="flex h-full bg-zinc-950 text-zinc-100">
      <nav id={`${id}-nav`} className="flex w-56 shrink-0 flex-col gap-1 border-r border-zinc-800 p-3">
        <div id={`${id}-nav-brand`} className="px-3 pb-3 pt-1 text-sm font-semibold text-zinc-400">
          ce-browser
        </div>
        {NAV.map((item) => (
          <button
            key={item.page}
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
        <div id={`${id}-body`} className="min-h-0 flex-1 overflow-y-auto px-8 py-6">
          {children}
        </div>
      </main>
    </div>
  );
}
