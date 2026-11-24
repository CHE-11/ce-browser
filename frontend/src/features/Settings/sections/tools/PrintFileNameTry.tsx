import { useEffect, useState } from "react";
import type { PrintPageFacts, TabInfo } from "@type/browser";
import { fileNameFromTemplate, type FileNameSource } from "@/built_in_extensions/print-to-pdf/file-name";
import { extractPage, listTabs, printPageFacts } from "@/lib/browser-client";
import { looksLikeUrl } from "@/lib/omnibox";
import { inputClass, errorText } from "../../SettingsControls";

export const TRY_LABEL = "Try an address";
export const TRY_HINT =
  "Paste a page address to see the file name this preset would save. An open tab is read as it is. Otherwise the page is downloaded for its title, author and date.";

const TRY_READING = "Reading this page…";
const TRY_SPARSE = "This page didn't include a title, author or date, so those parts of the name are left out.";
const TRY_ADDRESS_ONLY = "The name uses the address only.";
const TRY_INVALID = "Enter an address, such as https://example.com/story.";

type Loaded = {
  requested: string;
  facts: PrintPageFacts;
};

type LoadedError = {
  requested: string;
  message: string;
};

function usableAddress(text: string) {
  if (!text || !looksLikeUrl(text)) {
    return "";
  }
  const url = withScheme(text);
  try {
    const parsed = new URL(url);
    if ((parsed.protocol === "http:" || parsed.protocol === "https:") && !parsed.host) {
      return "";
    }
    return url;
  } catch {
    return "";
  }
}

function withScheme(text: string) {
  const value = text.trim();
  if (/^[a-z][a-z0-9+.-]*:/i.test(value)) {
    return value;
  }
  const host = value.split(/[/?#]/)[0] ?? value;
  const local = /^localhost(?::\d+)?$/i.test(host);
  return `${local ? "http" : "https"}://${value}`;
}

function pagePath(value: string) {
  return value.replace(/\/+$/, "") || "/";
}

function sameAddress(left: string, right: string) {
  try {
    const a = new URL(left);
    const b = new URL(right);
    return (
      a.protocol === b.protocol &&
      a.host.toLowerCase() === b.host.toLowerCase() &&
      pagePath(a.pathname) === pagePath(b.pathname) &&
      a.search === b.search
    );
  } catch {
    return left === right;
  }
}

function matchingTab(tabs: TabInfo[], activeId: number | null, url: string) {
  const open = tabs.filter((tab) => tab.loaded && sameAddress(tab.url, url));
  return open.find((tab) => tab.id === activeId) ?? open.find((tab) => !tab.sleeping) ?? open[0];
}

function emptyFacts(url: string): PrintPageFacts {
  return { title: "", url, lang: "", byline: "", siteName: "", published: "" };
}

async function pageForAddress(url: string): Promise<PrintPageFacts> {
  try {
    const snapshot = await listTabs();
    const match = matchingTab(snapshot.tabs, snapshot.activeId, url);
    if (match) {
      const page = await extractPage(match.id);
      return {
        title: page.title,
        url: page.url || url,
        lang: page.lang,
        byline: page.byline,
        siteName: page.siteName,
        published: page.published,
      };
    }
  } catch (error) {
    if (!/^https?:/i.test(url)) {
      throw error;
    }
  }
  if (!/^https?:/i.test(url)) {
    return emptyFacts(url);
  }
  return printPageFacts(url);
}

function sourceFrom(facts: PrintPageFacts, presetName: string): FileNameSource {
  return {
    title: facts.title,
    url: facts.url,
    author: facts.byline,
    site: facts.siteName,
    published: facts.published,
    lang: facts.lang,
    preset: presetName,
  };
}

export function PrintFileNameTry({ id, template, presetName }: { id: string; template: string; presetName: string }) {
  const [address, setAddress] = useState("");
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  const [pendingFor, setPendingFor] = useState("");
  const [failure, setFailure] = useState<LoadedError | null>(null);

  const text = address.trim();
  const requested = usableAddress(text);
  const ready = loaded !== null && loaded.requested === requested;
  const facts = ready ? loaded.facts : null;
  const pending = pendingFor !== "" && pendingFor === requested;
  const failed = failure !== null && failure.requested === requested ? failure.message : "";

  useEffect(() => {
    if (!requested) {
      return;
    }
    let cancel = false;
    const timer = window.setTimeout(() => {
      setPendingFor(requested);
      void pageForAddress(requested).then(
        (page) => {
          if (cancel) {
            return;
          }
          setLoaded({ requested, facts: page });
          setFailure(null);
          setPendingFor("");
        },
        (error: unknown) => {
          if (cancel) {
            return;
          }
          setLoaded(null);
          setFailure({ requested, message: errorText(error) });
          setPendingFor("");
        },
      );
    }, 400);
    return () => {
      cancel = true;
      window.clearTimeout(timer);
    };
  }, [requested]);

  const previewSource: FileNameSource | null = facts
    ? sourceFrom(facts, presetName)
    : failed
      ? { title: "", url: requested, author: "", site: "", published: "", lang: "", preset: presetName }
      : null;
  const name = previewSource ? fileNameFromTemplate(template, previewSource) : "";
  const sparse = !!facts && !facts.title && !facts.byline && !facts.published;
  const note = failed ? `${failed} ${TRY_ADDRESS_ONLY}` : sparse ? TRY_SPARSE : "";

  return (
    <div id={id} className="flex flex-col gap-1.5">
      <label id={`${id}-label`} className="flex flex-col gap-1">
        <span id={`${id}-caption`} className="text-xs text-zinc-400">
          {TRY_LABEL}
        </span>
        <input
          id={`${id}-input`}
          className={inputClass}
          value={address}
          inputMode="url"
          spellCheck={false}
          autoCapitalize="off"
          autoCorrect="off"
          placeholder="https://example.com/story"
          onChange={(event) => setAddress(event.target.value)}
        />
      </label>
      <p id={`${id}-hint`} className="text-xs text-zinc-500">
        {TRY_HINT}
      </p>
      {text && !requested ? (
        <p id={`${id}-invalid`} className="text-xs text-zinc-500">
          {TRY_INVALID}
        </p>
      ) : null}
      {pending && !name ? (
        <p id={`${id}-reading`} className="text-xs text-zinc-500">
          {TRY_READING}
        </p>
      ) : null}
      {name ? (
        <p id={`${id}-result`} className="text-xs text-zinc-400">
          <span id={`${id}-result-label`}>Would save as </span>
          <span id={`${id}-result-name`} className="font-mono text-zinc-300">
            {name}
          </span>
        </p>
      ) : null}
      {note && !pending ? (
        <p id={`${id}-note`} className="text-xs text-zinc-500">
          {note}
        </p>
      ) : null}
    </div>
  );
}
