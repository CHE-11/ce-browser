import { useEffect, useState } from "react";
import { PageNavLinks, pageNavClassName } from "@/components/PageShell";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";

type SettingsNavSection = {
  id: string;
  title: string;
};

const TOOL_SECTION_IDS = ["settings-adblock", "settings-ai", "settings-shopping", "settings-translate", "settings-print"] as const;

const itemClass = "truncate rounded-md px-2 py-1 text-left text-[11px] leading-4 text-zinc-400 hover:bg-zinc-900 hover:text-zinc-100";

export function SettingsNavigation({ sections, enabled }: { sections: readonly SettingsNavSection[]; enabled: boolean }) {
  const [activeId, setActiveId] = useState<string | undefined>(sections[0]?.id);
  const visibleKey = sections.map((section) => section.id).join("|");
  const currentId = sections.some((section) => section.id === activeId) ? activeId : sections[0]?.id;
  const toolIds = new Set<string>(TOOL_SECTION_IDS);
  const mainSections = sections.filter((section) => !toolIds.has(section.id));
  const toolSections = TOOL_SECTION_IDS.flatMap((id) => sections.filter((section) => section.id === id));
  const toolsActive = toolSections.some((section) => section.id === currentId);

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
    let frame = 0;
    const pickActive = () => {
      const atBottom = root.scrollTop > 8 && root.scrollTop + root.clientHeight >= root.scrollHeight - 8;
      if (atBottom) {
        setActiveId(nodes[nodes.length - 1].id);
        return;
      }
      const rootTop = root.getBoundingClientRect().top;
      let passed: { id: string; top: number } | null = null;
      let next: { id: string; top: number } | null = null;
      for (const node of nodes) {
        const top = node.getBoundingClientRect().top - rootTop;
        if (top <= 32) {
          if (passed === null || top > passed.top) {
            passed = { id: node.id, top };
          }
        } else if (next === null || top < next.top) {
          next = { id: node.id, top };
        }
      }
      const best = passed ?? next;
      if (best) {
        setActiveId(best.id);
      }
    };
    const schedule = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(pickActive);
    };
    schedule();
    const observer = new IntersectionObserver(schedule, { root, threshold: 0 });
    for (const node of nodes) {
      observer.observe(node);
    }
    root.addEventListener("scroll", schedule, { passive: true });
    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
      root.removeEventListener("scroll", schedule);
    };
  }, [enabled, visibleKey]);

  return (
    <nav id="settings-page-nav" aria-label="Settings" className={cn(pageNavClassName(true), "gap-0.5 p-2.5")}>
      <PageNavLinks id="settings-page" page="settings" compact>
        {enabled && sections.length > 0 ? (
          <div id="settings-page-nav-outline" className="mb-1 ml-3.5 flex flex-col gap-px border-l border-zinc-800 py-0.5 pl-1.5">
            {mainSections.map((section) => (
              <button
                key={section.id}
                id={`settings-page-nav-outline-${section.id}`}
                type="button"
                title={section.title}
                aria-current={section.id === currentId ? "location" : undefined}
                className={cn(itemClass, focusRing, section.id === currentId && "bg-sky-950 text-sky-300 hover:bg-sky-950")}
                onClick={() => document.getElementById(section.id)?.scrollIntoView({ behavior: "smooth", block: "start" })}
              >
                {section.title}
              </button>
            ))}
            {toolSections.length > 0 ? (
              <div id="settings-page-nav-tools" className="flex flex-col gap-px">
                <button
                  id="settings-page-nav-outline-settings-tools"
                  type="button"
                  title="Tools"
                  className={cn(itemClass, focusRing, toolsActive && "bg-sky-950 text-sky-300 hover:bg-sky-950")}
                  onClick={() => document.getElementById(toolSections[0].id)?.scrollIntoView({ behavior: "smooth", block: "start" })}
                >
                  Tools
                </button>
                <div id="settings-page-nav-tools-items" className="ml-2 flex flex-col gap-px border-l border-zinc-800 py-0.5 pl-1.5">
                  {toolSections.map((section) => (
                    <button
                      key={section.id}
                      id={`settings-page-nav-outline-${section.id}`}
                      type="button"
                      title={section.title}
                      aria-current={section.id === currentId ? "location" : undefined}
                      className={cn(itemClass, focusRing, section.id === currentId && "bg-sky-950 text-sky-300 hover:bg-sky-950")}
                      onClick={() => document.getElementById(section.id)?.scrollIntoView({ behavior: "smooth", block: "start" })}
                    >
                      {section.title}
                    </button>
                  ))}
                </div>
              </div>
            ) : null}
          </div>
        ) : null}
      </PageNavLinks>
    </nav>
  );
}
