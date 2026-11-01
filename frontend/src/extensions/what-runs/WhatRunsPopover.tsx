import { useEffect, useState } from "react";
import { CircleAlert, ExternalLink, Layers, LoaderCircle, RotateCcw } from "lucide-react";
import type { DetectedTechnology } from "../../../../types/browser";
import { Button } from "../../components/Button";
import { PopupFrame } from "../../components/PopupFrame";
import { closePopup, createTab, detectTechnologies } from "../../lib/browser-client";
import { cn } from "../../lib/cn";
import { PopoverHeader } from "../PopoverParts";
import { readPopoverTab } from "../shared";
import { errorText } from "../use-page-tool";
import { CATEGORIES } from "./categories";

type WhatRunsPopoverProps = {
  params: URLSearchParams;
};

type Scan =
  | { state: "loading" }
  | { state: "done"; technologies: DetectedTechnology[] }
  | { state: "error"; message: string };

function hostOf(url: string) {
  try {
    return new URL(url).host || url;
  } catch {
    return url;
  }
}

export function WhatRunsPopover({ params }: WhatRunsPopoverProps) {
  const tab = readPopoverTab(params);
  const [scan, setScan] = useState<Scan>({ state: "loading" });
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let cancelled = false;
    setScan({ state: "loading" });
    detectTechnologies(tab.id)
      .then((page) => {
        if (!cancelled) {
          setScan({ state: "done", technologies: page.technologies });
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setScan({ state: "error", message: errorText(error) });
        }
      });
    return () => {
      cancelled = true;
    };
  }, [tab.id, attempt]);

  return (
    <PopupFrame id="what-runs-popover" maxHeight={480} ready={scan.state !== "loading"}>
      <PopoverHeader
        id="what-runs-header"
        icon={<Layers aria-hidden />}
        title="What runs"
        subtitle={hostOf(tab.url) || tab.title}
      />
      <div id="what-runs-body" className="flex flex-col gap-2 px-3 pb-3">
        {scan.state === "loading" ? (
          <div id="what-runs-loading" className="flex h-16 items-center justify-center gap-2 text-sm text-zinc-400">
            <LoaderCircle id="what-runs-loading-icon" className="size-4 animate-spin" aria-hidden />
            Scanning this page…
          </div>
        ) : scan.state === "error" ? (
          <div id="what-runs-error" className="rounded-lg bg-amber-500/10 p-3 text-amber-200">
            <div id="what-runs-error-title" className="flex items-center gap-2 text-sm font-medium">
              <CircleAlert id="what-runs-error-icon" className="size-4 shrink-0" aria-hidden />
              Couldn't scan this page
            </div>
            <p id="what-runs-error-detail" className="mt-1 break-words text-xs opacity-80">
              {scan.message}
            </p>
          </div>
        ) : scan.technologies.length === 0 ? (
          <p id="what-runs-empty" className="rounded-lg bg-zinc-800/60 p-3 text-center text-sm text-zinc-400">
            Nothing recognisable on this page.
          </p>
        ) : (
          <TechnologyGroups technologies={scan.technologies} />
        )}
        {scan.state !== "loading" ? (
          <Button id="what-runs-again" size="sm" onClick={() => setAttempt((count) => count + 1)}>
            <RotateCcw id="what-runs-again-icon" className="size-3.5" aria-hidden />
            Scan again
          </Button>
        ) : null}
      </div>
    </PopupFrame>
  );
}

function TechnologyGroups({ technologies }: { technologies: DetectedTechnology[] }) {
  const groups = CATEGORIES.map((category) => ({
    ...category,
    items: technologies
      .filter((technology) => technology.category === category.id)
      .sort((a, b) => a.name.localeCompare(b.name)),
  })).filter((group) => group.items.length > 0);

  return (
    <div id="what-runs-groups" className="flex flex-col gap-3">
      {groups.map((group) => {
        const Icon = group.icon;
        return (
          <section id={`what-runs-group-${group.id}`} key={group.id} className="flex flex-col gap-0.5">
            <h3
              id={`what-runs-group-${group.id}-title`}
              className="flex items-center gap-1.5 px-1 pb-0.5 text-[11px] font-semibold uppercase tracking-wide text-zinc-500"
            >
              <Icon id={`what-runs-group-${group.id}-icon`} className="size-3.5" aria-hidden />
              {group.label}
            </h3>
            {group.items.map((technology, index) => (
              <TechnologyRow
                key={technology.name}
                id={`what-runs-${group.id}-${index}`}
                technology={technology}
              />
            ))}
          </section>
        );
      })}
    </div>
  );
}

function TechnologyRow({ id, technology }: { id: string; technology: DetectedTechnology }) {
  const link = technology.url;
  const open = () => {
    if (!link) {
      return;
    }
    void createTab(link);
    void closePopup();
  };

  return (
    <button
      id={id}
      type="button"
      disabled={!link}
      title={link ?? undefined}
      className={cn(
        "group flex h-8 w-full items-center gap-2 rounded-md px-2 text-left text-sm text-zinc-100",
        link ? "hover:bg-zinc-800" : "cursor-default",
      )}
      onClick={open}
    >
      <span id={`${id}-name`} className="min-w-0 flex-1 truncate">
        {technology.name}
      </span>
      {technology.version ? (
        <span
          id={`${id}-version`}
          className="shrink-0 rounded bg-zinc-800 px-1.5 py-0.5 font-mono text-[11px] text-zinc-400 group-hover:bg-zinc-700"
        >
          {technology.version}
        </span>
      ) : null}
      {link ? (
        <ExternalLink
          id={`${id}-link-icon`}
          className="size-3.5 shrink-0 text-zinc-600 group-hover:text-zinc-300"
          aria-hidden
        />
      ) : null}
    </button>
  );
}
