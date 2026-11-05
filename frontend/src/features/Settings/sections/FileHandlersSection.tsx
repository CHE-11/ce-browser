import { useEffect, useState } from "react";
import { RotateCcw, X } from "lucide-react";
import type { BrowserProfile, FileHandler } from "@type/browser";
import { Button } from "@/components/Button";
import { IconButton } from "@/components/IconButton";
import { listProfiles, onProfilesChanged } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { SETTINGS_SECTIONS } from "../settings-sections";
import {
  inputClass,
  matchesQuery,
  Section,
  useDefaultSettings,
  useReportMatch,
  type SettingsSectionProps,
} from "../SettingsControls";

const section = SETTINGS_SECTIONS.find((item) => item.id === "settings-file-handlers")!;

const HINT =
  "Opening a file of this type loads this page and places the file on its file input. A chosen profile always opens it. With none chosen, an open profile is used, or the default profile when none are open. Windows lists ce-browser under Open with for these types.";

const emptyDraft = { extension: "", url: "", inputId: "", profile: "" };

function cleanExtension(value: string) {
  return value.trim().replace(/^\./, "").toLowerCase();
}

function validExtension(value: string) {
  return /^[a-z0-9]{1,16}$/.test(value);
}

function validUrl(value: string) {
  return (value.startsWith("https://") || value.startsWith("http://")) && !/\s/.test(value);
}

export function FileHandlersSection({ settings, query, onUpdate, onMatch }: SettingsSectionProps) {
  const handlers = settings.fileHandlers ?? [];
  const [draft, setDraft] = useState(emptyDraft);
  const [profiles, setProfiles] = useState<BrowserProfile[]>([]);
  const defaults = useDefaultSettings();
  const builtin = defaults?.fileHandlers.find((handler) => handler.extension === "pdf") ?? null;
  const visible = matchesQuery(
    query,
    section.title,
    HINT,
    "pdf",
    "open with",
    "extension",
    "file input",
    "profile",
    "Add",
    ...handlers.flatMap((handler) => [handler.extension, handler.url, handler.inputId, handler.profile]),
    ...profiles.map((profile) => profile.name),
  );
  useReportMatch(section.id, visible, onMatch);

  useEffect(() => {
    let disposed = false;
    const apply = (next: BrowserProfile[]) => {
      if (!disposed) {
        setProfiles(next);
      }
    };
    void listProfiles().then(apply).catch(() => undefined);
    const listen = onProfilesChanged(apply);
    return () => {
      disposed = true;
      void listen.then((stop) => stop());
    };
  }, []);

  if (!visible) {
    return null;
  }

  const save = (next: FileHandler[]) => onUpdate({ fileHandlers: next });
  const pdf = handlers.find((handler) => handler.extension === "pdf");
  const pdfDiffers =
    builtin !== null &&
    pdf !== undefined &&
    (pdf.url !== builtin.url || pdf.inputId !== builtin.inputId || (pdf.profile || "") !== (builtin.profile || ""));

  const commit = (index: number, next: FileHandler) => {
    const extension = cleanExtension(next.extension);
    const url = next.url.trim();
    const inputId = next.inputId.trim();
    const current = handlers[index];
    if (!validExtension(extension) || !validUrl(url)) {
      return false;
    }
    if (handlers.some((item, itemIndex) => itemIndex !== index && item.extension === extension)) {
      return false;
    }
    if (current.extension === extension && current.url === url && current.inputId === inputId) {
      return true;
    }
    const copy = handlers.slice();
    copy[index] = { extension, url, inputId, profile: next.profile || "" };
    save(copy);
    return true;
  };

  const add = () => {
    const extension = cleanExtension(draft.extension);
    const url = draft.url.trim();
    if (!validExtension(extension) || !validUrl(url)) {
      return;
    }
    save([
      ...handlers.filter((item) => item.extension !== extension),
      { extension, url, inputId: draft.inputId.trim(), profile: "" },
    ]);
    setDraft(emptyDraft);
  };

  const restorePdf = () => {
    if (!builtin) {
      return;
    }
    save([builtin, ...handlers.filter((item) => item.extension !== "pdf")]);
  };

  return (
    <Section id={section.id} title={section.title}>
      <div id="settings-file-handlers-list" className="flex flex-col gap-3 px-5 py-3">
        <p id="settings-file-handlers-hint" className="text-xs text-zinc-500">
          {HINT}
        </p>
        <ul id="settings-file-handlers-rows" className="flex flex-col gap-2">
          {handlers.map((handler, index) => (
            <li id={`settings-file-handler-${handler.extension}`} key={handler.extension} className="flex items-center gap-2">
              <input
                id={`settings-file-handler-${handler.extension}-extension`}
                defaultValue={handler.extension}
                key={`${handler.extension}-extension`}
                spellCheck={false}
                maxLength={16}
                aria-label={`${handler.extension} extension`}
                className={`${inputClass} w-20`}
                onBlur={(event) => {
                  if (!commit(index, { ...handler, extension: event.target.value })) {
                    event.target.value = handler.extension;
                  }
                }}
              />
              <select
                id={`settings-file-handler-${handler.extension}-profile`}
                value={handler.profile || ""}
                aria-label={`${handler.extension} profile`}
                className={cn(inputClass, "w-36 shrink-0")}
                onChange={(event) => {
                  const profile = event.target.value;
                  save(handlers.map((item) => (item.extension === handler.extension ? { ...item, profile } : item)));
                }}
              >
                <option id={`settings-file-handler-${handler.extension}-profile-any`} value="">
                  Any open profile
                </option>
                {handler.profile && !profiles.some((profile) => profile.id === handler.profile) ? (
                  <option id={`settings-file-handler-${handler.extension}-profile-missing`} value={handler.profile}>
                    Unknown profile
                  </option>
                ) : null}
                {profiles.map((profile) => (
                  <option id={`settings-file-handler-${handler.extension}-profile-${profile.id}`} key={profile.id} value={profile.id}>
                    {profile.name}
                  </option>
                ))}
              </select>
              <input
                id={`settings-file-handler-${handler.extension}-url`}
                defaultValue={handler.url}
                key={`${handler.extension}-url-${handler.url}`}
                spellCheck={false}
                aria-label={`${handler.extension} page`}
                className={inputClass}
                onBlur={(event) => {
                  if (!commit(index, { ...handler, url: event.target.value })) {
                    event.target.value = handler.url;
                  }
                }}
              />
              <input
                id={`settings-file-handler-${handler.extension}-input`}
                defaultValue={handler.inputId}
                key={`${handler.extension}-input-${handler.inputId}`}
                spellCheck={false}
                placeholder="File input id"
                aria-label={`${handler.extension} file input`}
                className={`${inputClass} w-40`}
                onBlur={(event) => {
                  if (!commit(index, { ...handler, inputId: event.target.value })) {
                    event.target.value = handler.inputId;
                  }
                }}
              />
              {handler.extension === "pdf" && pdfDiffers ? (
                <IconButton id="settings-file-handler-pdf-reset" label="Restore the PDF editor" onClick={restorePdf}>
                  <RotateCcw id="settings-file-handler-pdf-reset-icon" aria-hidden />
                </IconButton>
              ) : null}
              <IconButton
                id={`settings-file-handler-${handler.extension}-remove`}
                label={`Remove ${handler.extension}`}
                onClick={() => save(handlers.filter((item) => item.extension !== handler.extension))}
              >
                <X id={`settings-file-handler-${handler.extension}-remove-icon`} aria-hidden />
              </IconButton>
            </li>
          ))}
        </ul>
        {builtin && !pdf ? (
          <Button id="settings-file-handlers-restore-pdf" size="sm" variant="ghost" onClick={restorePdf}>
            <RotateCcw id="settings-file-handlers-restore-pdf-icon" className="size-3.5" aria-hidden />
            Restore PDF
          </Button>
        ) : null}
        <div id="settings-file-handlers-add" className="flex items-center gap-2">
          <input
            id="settings-file-handlers-extension"
            value={draft.extension}
            placeholder="png"
            spellCheck={false}
            maxLength={16}
            aria-label="Extension"
            className={`${inputClass} w-20`}
            onChange={(event) => setDraft({ ...draft, extension: event.target.value })}
          />
          <input
            id="settings-file-handlers-url"
            value={draft.url}
            placeholder="https://example.com/view"
            spellCheck={false}
            aria-label="Page"
            className={inputClass}
            onChange={(event) => setDraft({ ...draft, url: event.target.value })}
          />
          <input
            id="settings-file-handlers-input"
            value={draft.inputId}
            placeholder="File input id"
            spellCheck={false}
            aria-label="File input id"
            className={`${inputClass} w-40`}
            onChange={(event) => setDraft({ ...draft, inputId: event.target.value })}
          />
          <Button
            id="settings-file-handlers-add-button"
            size="sm"
            disabled={!validExtension(cleanExtension(draft.extension)) || !validUrl(draft.url.trim())}
            onClick={add}
          >
            Add
          </Button>
        </div>
      </div>
    </Section>
  );
}
