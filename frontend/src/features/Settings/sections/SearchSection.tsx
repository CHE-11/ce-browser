import { useEffect, useState } from "react";
import { X } from "lucide-react";
import type { Keyword, SearchEngine } from "@type/browser";
import { Button } from "@/components/Button";
import { IconButton } from "@/components/IconButton";
import { Toggle } from "@/components/Toggle";
import { listKeywords, onKeywordsChanged, saveKeywords } from "@/lib/browser-client";
import { SETTINGS_SECTIONS } from "../settings-sections";
import {
  inputClass,
  matchesQuery,
  Row,
  Section,
  useReportMatch,
  useSectionMatches,
  useSettingVisible,
  useSettingsQuery,
  type SettingsSectionProps,
} from "../SettingsControls";

const section = SETTINGS_SECTIONS.find((item) => item.id === "settings-search")!;

const SEARCH_ENGINES: { id: SearchEngine; label: string }[] = [
  { id: "google", label: "Google" },
  { id: "bing", label: "Bing" },
  { id: "duckduckgo", label: "DuckDuckGo" },
  { id: "brave", label: "Brave Search" },
  { id: "startpage", label: "Startpage" },
  { id: "kagi", label: "Kagi (no ads)" },
  { id: "mojeek", label: "Mojeek (no ads)" },
  { id: "custom", label: "Custom" },
];

const ENGINE_HINT = "Used when the address bar text is not a URL. Kagi and Mojeek do not show ads. Kagi needs an account.";
const CUSTOM_LABEL = "Custom search URL";
const CUSTOM_HINT = "Use %s where the query goes.";
const GESTURE_LABEL = "Mouse gestures";
const GESTURE_HINT = "Right-drag left to go back, right to go forward, up to reload, down to close the tab.";
const KEYWORD_LABEL = "Keywords";
const KEYWORD_HINT = "Type gh react or !gh react in the address bar. Put %s where the query goes.";

export function SearchSection({ settings, query, onUpdate, onMatch }: SettingsSectionProps) {
  const [keywords, setKeywords] = useState<Keyword[]>([]);
  const [draft, setDraft] = useState({ keyword: "", title: "", url: "https://example.com/search?q=%s" });
  const [customDraft, setCustomDraft] = useState("");
  const [pendingCustom, setPendingCustom] = useState(false);

  useEffect(() => {
    setCustomDraft(settings.customSearchUrl);
  }, [settings.customSearchUrl]);

  useEffect(() => {
    void listKeywords().then(setKeywords).catch(() => undefined);
    const unlisten = onKeywordsChanged(setKeywords);
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);

  const engine = pendingCustom ? "custom" : settings.searchEngine;
  const customVisible = engine === "custom";
  const visible = matchesQuery(
    query,
    section.title,
    "Search engine",
    ENGINE_HINT,
    ...SEARCH_ENGINES.map((option) => option.label),
    customVisible ? CUSTOM_LABEL : "",
    customVisible ? CUSTOM_HINT : "",
    customVisible ? customDraft : "",
    GESTURE_LABEL,
    GESTURE_HINT,
    KEYWORD_LABEL,
    KEYWORD_HINT,
    "Add",
    ...keywords.flatMap((keyword) => [keyword.keyword, keyword.title, keyword.url]),
  );
  useReportMatch(section.id, visible, onMatch);

  const save = (next: Keyword[]) => {
    void saveKeywords(next).then(setKeywords).catch(() => undefined);
  };

  if (!visible) {
    return null;
  }

  return (
    <Section id={section.id} title={section.title}>
      <Row id="settings-search-engine" setting="searchEngine" label="Search engine" hint={ENGINE_HINT} search={SEARCH_ENGINES.map((option) => option.label)}>
        <select
          id="settings-search-engine-select"
          value={engine}
          className={inputClass}
          onChange={(event) => {
            const value = event.target.value as SearchEngine;
            if (value === "custom" && !settings.customSearchUrl) {
              setPendingCustom(true);
              return;
            }
            setPendingCustom(false);
            onUpdate({ searchEngine: value });
          }}
        >
          {SEARCH_ENGINES.map((option) => (
            <option key={option.id} id={`settings-search-engine-${option.id}`} value={option.id}>
              {option.label}
            </option>
          ))}
        </select>
      </Row>
      {customVisible ? (
        <Row id="settings-search-custom" setting="customSearchUrl" label={CUSTOM_LABEL} hint={CUSTOM_HINT} search={[customDraft]}>
          <input
            id="settings-search-custom-input"
            value={customDraft}
            spellCheck={false}
            placeholder="https://example.com/search?q=%s"
            className={inputClass}
            onChange={(event) => setCustomDraft(event.target.value)}
            onBlur={() => {
              if (customDraft.trim()) {
                setPendingCustom(false);
                onUpdate({ searchEngine: "custom", customSearchUrl: customDraft.trim() });
              }
            }}
            onKeyDown={(event) => event.key === "Enter" && event.currentTarget.blur()}
          />
        </Row>
      ) : null}
      <Row id="settings-gestures" setting="mouseGestures" label={GESTURE_LABEL} hint={GESTURE_HINT}>
        <Toggle id="settings-gestures-toggle" checked={settings.mouseGestures} onChange={(mouseGestures) => onUpdate({ mouseGestures })} />
      </Row>
      <KeywordEditor keywords={keywords} draft={draft} setDraft={setDraft} onSave={save} />
    </Section>
  );
}

function KeywordEditor({
  keywords,
  draft,
  setDraft,
  onSave,
}: {
  keywords: Keyword[];
  draft: { keyword: string; title: string; url: string };
  setDraft: (draft: { keyword: string; title: string; url: string }) => void;
  onSave: (next: Keyword[]) => void;
}) {
  const query = useSettingsQuery();
  const sectionMatches = useSectionMatches();
  const blockMatches = sectionMatches || matchesQuery(query, KEYWORD_LABEL, KEYWORD_HINT, "Add");
  const visibleKeywords = blockMatches
    ? keywords
    : keywords.filter((keyword) => matchesQuery(query, keyword.keyword, keyword.title, keyword.url));
  const visible = useSettingVisible(
    KEYWORD_LABEL,
    KEYWORD_HINT,
    "Add",
    ...keywords.flatMap((keyword) => [keyword.keyword, keyword.title, keyword.url]),
  );
  if (!visible) {
    return null;
  }
  return (
    <div id="settings-keywords" className="px-5 py-3">
      <div id="settings-keywords-label" className="text-sm text-zinc-100">
        {KEYWORD_LABEL}
      </div>
      <p id="settings-keywords-hint" className="mb-2 text-xs text-zinc-500">
        {KEYWORD_HINT}
      </p>
      <ul id="settings-keywords-list" className="mb-2 flex flex-col gap-1">
        {visibleKeywords.map((keyword) => (
          <li id={`settings-keyword-${keyword.keyword}`} key={keyword.keyword} className="flex items-center gap-2 text-sm">
            <span id={`settings-keyword-${keyword.keyword}-name`} className="w-12 shrink-0 text-zinc-300">
              {keyword.keyword}
            </span>
            <span id={`settings-keyword-${keyword.keyword}-title`} className="w-28 shrink-0 truncate text-zinc-400">
              {keyword.title}
            </span>
            <span id={`settings-keyword-${keyword.keyword}-url`} className="min-w-0 flex-1 truncate text-xs text-zinc-500">
              {keyword.url}
            </span>
            <IconButton
              id={`settings-keyword-${keyword.keyword}-remove`}
              label={`Remove ${keyword.keyword}`}
              onClick={() => onSave(keywords.filter((item) => item.keyword !== keyword.keyword))}
            >
              <X id={`settings-keyword-${keyword.keyword}-remove-icon`} aria-hidden />
            </IconButton>
          </li>
        ))}
      </ul>
      <div id="settings-keywords-add" className="flex gap-2">
        <input
          id="settings-keywords-keyword"
          value={draft.keyword}
          placeholder="gh"
          maxLength={20}
          className={`${inputClass} w-20`}
          onChange={(event) => setDraft({ ...draft, keyword: event.target.value })}
        />
        <input
          id="settings-keywords-title"
          value={draft.title}
          placeholder="GitHub"
          className={`${inputClass} w-32`}
          onChange={(event) => setDraft({ ...draft, title: event.target.value })}
        />
        <input
          id="settings-keywords-url"
          value={draft.url}
          spellCheck={false}
          className={inputClass}
          onChange={(event) => setDraft({ ...draft, url: event.target.value })}
        />
        <Button
          id="settings-keywords-add-button"
          size="sm"
          disabled={!draft.keyword.trim() || !draft.url.includes("%s")}
          onClick={() => {
            onSave([
              ...keywords.filter((item) => item.keyword !== draft.keyword.trim().toLowerCase()),
              { ...draft, keyword: draft.keyword.trim().toLowerCase() },
            ]);
            setDraft({ keyword: "", title: "", url: "https://example.com/search?q=%s" });
          }}
        >
          Add
        </Button>
      </div>
    </div>
  );
}
