import { Fragment, useEffect, type ReactNode } from "react";
import { Bookmark, Download, Folder, History, LayoutGrid, Paintbrush, Puzzle, Settings } from "lucide-react";
import { cn } from "../lib/cn";
import { focusRing } from "../lib/focus-ring";
import { goToPage } from "../lib/route";
import { useAppearanceFromSettings } from "../lib/use-browser";

export const PAGE_NAV = [
  { page: "bookmarks", label: "Bookmarks", icon: Bookmark },
  { page: "boards", label: "Boards", icon: LayoutGrid },
  { page: "downloads", label: "Downloads", icon: Download },
  { page: "files", label: "Files", icon: Folder },
  { page: "history", label: "History", icon: History },
  { page: "extensions", label: "Extensions", icon: Puzzle },
  { page: "styles", label: "Styles", icon: Paintbrush },
  { page: "settings", label: "Settings", icon: Settings },
] as const;

export type BrowserPage = (typeof PAGE_NAV)[number]["page"];

export function pageNavClassName(lockHorizontalScroll = false) {
  return cn(
    "flex w-56 shrink-0 flex-col gap-1 overflow-y-auto border-r border-zinc-800 p-3",
    lockHorizontalScroll && "overflow-x-hidden overscroll-x-none",
  );
}

export function PageNavLinks({ id, page, compact = false, children }: { id: string; page: BrowserPage; compact?: boolean; children?: ReactNode }) {
  return (
    <>
      <div id={`${id}-nav-brand`} className={cn("px-3 pb-3 pt-1 text-sm font-semibold text-zinc-400", compact && "px-2.5 pb-2 pt-0.5 text-[13px]")}>
        ce-browser
      </div>
      {PAGE_NAV.map((item) => (
        <Fragment key={item.page}>
          <button
            id={`${id}-nav-${item.page}`}
            type="button"
            aria-current={item.page === page ? "page" : undefined}
            className={cn(
              "flex h-9 items-center gap-3 rounded-md px-3 text-left text-sm text-zinc-300 hover:bg-zinc-900 hover:text-zinc-100",
              compact && "h-8 gap-2.5 px-2.5 text-[13px]",
              focusRing,
              item.page === page && "bg-sky-950 text-sky-300 hover:bg-sky-950",
            )}
            onClick={() => goToPage(item.page)}
          >
            <item.icon id={`${id}-nav-${item.page}-icon`} className={cn("size-4", compact && "size-3.5")} aria-hidden />
            {item.label}
          </button>
          {item.page === page ? children : null}
        </Fragment>
      ))}
    </>
  );
}

type PageShellProps = {
  id: string;
  page: BrowserPage;
  title: string;
  actions?: ReactNode;
  nav?: ReactNode;
  lockHorizontalScroll?: boolean;
  /** The body fills the page height and does not pad or scroll on its own. */
  flush?: boolean;
  children: ReactNode;
};

export function PageShell({ id, page, title, actions, nav, lockHorizontalScroll = false, flush = false, children }: PageShellProps) {
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
      {nav ?? (
        <nav id={`${id}-nav`} className={pageNavClassName(lockHorizontalScroll)}>
          <PageNavLinks id={id} page={page} />
        </nav>
      )}
      <main id={`${id}-main`} className="flex min-w-0 flex-1 flex-col">
        <header id={`${id}-header`} className="flex h-16 shrink-0 items-center justify-between gap-4 border-b border-zinc-800 px-8">
          <h1 id={`${id}-title`} className="text-xl font-semibold">
            {title}
          </h1>
          <div id={`${id}-actions`} className="flex items-center gap-2">
            {actions}
          </div>
        </header>
        <div
          id={`${id}-body`}
          className={cn("min-h-0 flex-1", flush ? "flex flex-col overflow-hidden" : "overflow-y-auto px-8 py-6", scrollLock)}
        >
          {children}
        </div>
      </main>
    </div>
  );
}
