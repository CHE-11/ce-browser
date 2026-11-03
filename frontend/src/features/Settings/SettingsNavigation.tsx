import { useEffect, useState } from "react";
import { PageNavLinks, pageNavClassName } from "@/components/PageShell";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";

type SettingsNavSection = {
  id: string;
  title: string;
};

export function SettingsNavigation({ sections, enabled }: { sections: readonly SettingsNavSection[]; enabled: boolean }) {
  const [activeId, setActiveId] = useState<string | undefined>(sections[0]?.id);
  const visibleKey = sections.map((section) => section.id).join("|");
  const currentId = sections.some((section) => section.id === activeId) ? activeId : sections[0]?.id;

  useEffect(() => {
    if (!enabled) {
      return undefined;
    }
    const root = document.getElementById("settings-page-body");
    if (!root) {
      return undefined;
    }
    const ids = visibleKey.length > 0 ? visibleKey.split("|") : [];
    const nodes = ids
      .map((id) => document.getElementById(id))
      .filter((node): node is HTMLElement => node instanceof HTMLElement);
    if (nodes.length === 0) {
      return undefined;
    }
    const seen = new Set<string>();
    const pickActive = () => {
      const atBottom = root.scrollTop > 8 && root.scrollTop + root.clientHeight >= root.scrollHeight - 8;
      if (atBottom) {
        const last = nodes[nodes.length - 1];
        if (last) {
          setActiveId(last.id);
        }
        return;
      }
      const rootTop = root.getBoundingClientRect().top;
      let best: { id: string; top: number } | null = null;
      for (const id of seen) {
        const node = document.getElementById(id);
        if (!node) {
          continue;
        }
        const top = node.getBoundingClientRect().top - rootTop;
        if (top <= 32 && (best === null || top > best.top)) {
          best = { id, top };
        }
      }
      if (!best) {
        for (const id of seen) {
          const node = document.getElementById(id);
          if (!node) {
            continue;
          }
          const top = node.getBoundingClientRect().top - rootTop;
          if (best === null || top < best.top) {
            best = { id, top };
          }
        }
      }
      if (best) {
        setActiveId(best.id);
      }
    };
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            seen.add(entry.target.id);
          } else {
            seen.delete(entry.target.id);
          }
        }
        pickActive();
      },
      { root, threshold: 0 },
    );
    for (const node of nodes) {
      observer.observe(node);
    }
    return () => observer.disconnect();
  }, [enabled, visibleKey]);

  return (
    <nav id="settings-page-nav" aria-label="Settings" className={cn(pageNavClassName(true), "gap-0.5 p-2.5")}>
      <PageNavLinks id="settings-page" page="settings" compact>
        {enabled && sections.length > 0 ? (
          <div id="settings-page-nav-outline" className="mb-1 ml-3.5 flex flex-col gap-px border-l border-zinc-800 py-0.5 pl-1.5">
            {sections.map((section) => (
              <button
                key={section.id}
                id={`settings-page-nav-outline-${section.id}`}
                type="button"
                title={section.title}
                aria-current={section.id === currentId ? "location" : undefined}
                className={cn(
                  "truncate rounded-md px-1.5 py-0.5 text-left text-[11px] leading-4 text-zinc-400 hover:bg-zinc-900 hover:text-zinc-100",
                  focusRing,
                  section.id === currentId && "bg-sky-950 text-sky-300 hover:bg-sky-950",
                )}
                onClick={() => document.getElementById(section.id)?.scrollIntoView({ behavior: "smooth", block: "start" })}
              >
                {section.title}
              </button>
            ))}
          </div>
        ) : null}
      </PageNavLinks>
    </nav>
  );
}
