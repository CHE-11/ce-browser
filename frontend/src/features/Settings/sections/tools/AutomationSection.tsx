import { useEffect, useState } from "react";
import { X } from "lucide-react";
import type { AgentAutonomy, AutomationSettings, McpStatus } from "@type/browser";
import { AUTONOMY_LEVELS } from "@/lib/agent-autonomy";
import { Button } from "@/components/Button";
import { IconButton } from "@/components/IconButton";
import { Toggle } from "@/components/Toggle";
import {
  automationSecretNames,
  clearAutomationSecret,
  copyText,
  mcpStatus,
  onMcpChanged,
  openAutomations,
  regenerateMcpToken,
  setAutomationSecret,
} from "@/lib/browser-client";
import { SETTINGS_SECTIONS } from "../../settings-sections";
import { errorText, inputClass, matchesQuery, Row, Section, useReportMatch, useSettingVisible, type SettingsSectionProps } from "../../SettingsControls";

const section = SETTINGS_SECTIONS.find((item) => item.id === "settings-automation")!;
const mcpSection = SETTINGS_SECTIONS.find((item) => item.id === "settings-mcp")!;

const TEXT = {
  enabled: "Allow browser automation",
  enabledHint: "The agent, recorded flows, and schedules. Off stops new runs.",
  model: "Agent model",
  modelHint: "Blank uses the model chosen under AI/Agent. Models that see images work best.",
  steps: "Most turns per run",
  stepsHint: "After this many turns the agent asks whether to keep going.",
  autonomy: "Default autonomy",
  autonomyHint: "How much new agent runs do without asking. Each run can change it in the agent panel.",
  helpers: "Helper agents",
  helpersHint: "Helpers one run may have working in background tabs at once. 0 turns helpers off.",
  memory: "Agent memory",
  memoryHint: "The agent keeps notes about you and sites across runs. Manage them in the agent panel.",
  notify: "Notify when background runs finish",
  notifyHint: "A Windows notification when a run you aren't watching finishes or needs you.",
  script: "Let the agent run scripts",
  scriptHint: "The agent may run JavaScript in pages. Runs ask first unless they're on full auto.",
  desktop: "Let the agent use the desktop",
  desktopHint: "Mouse and keyboard outside the browser. Off until you turn it on. The first action of each run still asks.",
  stream: "Show replies as they're written",
  streamHint: "Streams the model's thinking into the panel. Turn off for endpoints that don't stream.",
  confirm: "Ask before sensitive actions",
  confirmHint: "Purchases, payments, sends, deletes, and posts wait for your approval. Uploads always ask.",
  screenshots: "Send screenshots to the model",
  screenshotsHint: "Off sends only the page text, which is cheaper but less reliable.",
  allowed: "Allowed sites",
  allowedHint: "When this list has sites, opening any other site asks first.",
  blocked: "Blocked sites",
  blockedHint: "Automation never acts on these sites or their subdomains. Sites blocked for AI are included.",
  secrets: "Saved secrets",
  secretsHint: "Flows and the agent type these as {{secret:name}} without seeing them. Encrypted with Windows DPAPI and never synced.",
  scheduler: "Run scheduled flows",
  schedulerHint: "Flows with triggers run on their own while the browser is open.",
  battery: "Wait while on battery",
  batteryHint: "Scheduled runs wait until the PC is plugged in.",
  flows: "Flows and datasets",
  flowsHint: "Record, edit, and schedule flows. Export extracted data.",
};

const MCP_TEXT = {
  mcp: "Local MCP server",
  mcpHint: "Lets Cursor, Claude, and other MCP clients use this browser's tabs.",
  port: "Port",
  portHint: "Listens on 127.0.0.1 only.",
  readOnly: "Read only",
  readOnlyHint: "MCP clients can read pages but not click, type, or navigate.",
  evaluate: "Allow running scripts",
  evaluateHint: "MCP clients may run JavaScript in pages. Leave off unless you trust the client.",
  token: "Access token",
  config: "Client setup",
  configHint: "Add this to Cursor's mcp.json, or the matching file of another client.",
};

export function AutomationSection({ settings, query, onUpdate, onError, onMatch }: SettingsSectionProps) {
  const automation = settings.automation;
  const aiBlockedSites = settings.aiBlockedSites ?? [];
  const visible = matchesQuery(
    query,
    section.title,
    "agent ai rpa",
    ...Object.values(TEXT),
    ...automation.allowedSites,
    ...automation.blockedSites,
    ...aiBlockedSites,
  );
  useReportMatch(section.id, visible, onMatch);
  const update = (patch: Partial<AutomationSettings>) => onUpdate({ automation: { ...automation, ...patch } });

  if (!visible) {
    return null;
  }

  return (
    <Section id={section.id} title={section.title}>
      <Row id="settings-automation-enabled" setting="automation.enabled" label={TEXT.enabled} hint={TEXT.enabledHint}>
        <Toggle id="settings-automation-enabled-toggle" checked={automation.enabled} onChange={(enabled) => update({ enabled })} />
      </Row>
      <Row id="settings-automation-model" setting="automation.agentModel" label={TEXT.model} hint={TEXT.modelHint}>
        <input
          id="settings-automation-model-input"
          className={inputClass}
          value={automation.agentModel}
          placeholder={settings.aiProviders[settings.aiProvider]?.model || "Model from AI/Agent settings"}
          spellCheck={false}
          onChange={(event) => update({ agentModel: event.target.value })}
        />
      </Row>
      <Row id="settings-automation-steps" setting="automation.maxSteps" label={TEXT.steps} hint={TEXT.stepsHint}>
        <input
          id="settings-automation-steps-input"
          className={inputClass}
          type="number"
          min={5}
          max={500}
          value={automation.maxSteps}
          onChange={(event) => update({ maxSteps: Math.max(5, Math.min(500, Number(event.target.value) || 30)) })}
        />
      </Row>
      <Row id="settings-automation-autonomy" setting="automation.defaultAutonomy" label={TEXT.autonomy} hint={TEXT.autonomyHint}>
        <select
          id="settings-automation-autonomy-select"
          className={inputClass}
          value={automation.defaultAutonomy ?? "ask"}
          title={AUTONOMY_LEVELS.find((level) => level.value === automation.defaultAutonomy)?.hint}
          onChange={(event) => update({ defaultAutonomy: event.target.value as AgentAutonomy })}
        >
          {AUTONOMY_LEVELS.map((level) => (
            <option id={`settings-automation-autonomy-${level.value}`} key={level.value} value={level.value}>
              {level.label}
            </option>
          ))}
        </select>
      </Row>
      <Row id="settings-automation-helpers" setting="automation.maxParallelAgents" label={TEXT.helpers} hint={TEXT.helpersHint}>
        <input
          id="settings-automation-helpers-input"
          className={inputClass}
          type="number"
          min={0}
          max={6}
          value={automation.maxParallelAgents ?? 3}
          onChange={(event) => update({ maxParallelAgents: Math.max(0, Math.min(6, Math.round(Number(event.target.value) || 0))) })}
        />
      </Row>
      <Row id="settings-automation-memory" setting="automation.agentMemory" label={TEXT.memory} hint={TEXT.memoryHint}>
        <Toggle id="settings-automation-memory-toggle" checked={automation.agentMemory ?? true} onChange={(agentMemory) => update({ agentMemory })} />
      </Row>
      <Row id="settings-automation-notify" setting="automation.agentNotify" label={TEXT.notify} hint={TEXT.notifyHint}>
        <Toggle id="settings-automation-notify-toggle" checked={automation.agentNotify ?? true} onChange={(agentNotify) => update({ agentNotify })} />
      </Row>
      <Row id="settings-automation-script" setting="automation.agentAllowScript" label={TEXT.script} hint={TEXT.scriptHint}>
        <Toggle id="settings-automation-script-toggle" checked={automation.agentAllowScript ?? false} onChange={(agentAllowScript) => update({ agentAllowScript })} />
      </Row>
      <Row id="settings-automation-desktop" setting="automation.desktopControl" label={TEXT.desktop} hint={TEXT.desktopHint}>
        <Toggle id="settings-automation-desktop-toggle" checked={automation.desktopControl ?? false} onChange={(desktopControl) => update({ desktopControl })} />
      </Row>
      <Row id="settings-automation-stream" setting="automation.streamReplies" label={TEXT.stream} hint={TEXT.streamHint}>
        <Toggle id="settings-automation-stream-toggle" checked={automation.streamReplies ?? true} onChange={(streamReplies) => update({ streamReplies })} />
      </Row>
      <Row id="settings-automation-confirm" setting="automation.confirmSensitive" label={TEXT.confirm} hint={TEXT.confirmHint}>
        <Toggle id="settings-automation-confirm-toggle" checked={automation.confirmSensitive} onChange={(confirmSensitive) => update({ confirmSensitive })} />
      </Row>
      <Row id="settings-automation-screenshots" setting="automation.screenshotsToModel" label={TEXT.screenshots} hint={TEXT.screenshotsHint}>
        <Toggle id="settings-automation-screenshots-toggle" checked={automation.screenshotsToModel} onChange={(screenshotsToModel) => update({ screenshotsToModel })} />
      </Row>
      <SiteList
        id="settings-automation-allowed"
        label={TEXT.allowed}
        hint={TEXT.allowedHint}
        empty="Every site is allowed."
        sites={automation.allowedSites}
        onChange={(allowedSites) => update({ allowedSites })}
      />
      <SiteList
        id="settings-automation-blocked"
        label={TEXT.blocked}
        hint={TEXT.blockedHint}
        empty="No blocked sites."
        sites={automation.blockedSites}
        included={aiBlockedSites.filter((site) => !automation.blockedSites.includes(site))}
        includedNote="Blocked for AI"
        onChange={(blockedSites) => update({ blockedSites })}
      />
      <Secrets onError={onError} />
      <Row id="settings-automation-scheduler" setting="automation.schedulerEnabled" label={TEXT.scheduler} hint={TEXT.schedulerHint}>
        <Toggle id="settings-automation-scheduler-toggle" checked={automation.schedulerEnabled} onChange={(schedulerEnabled) => update({ schedulerEnabled })} />
      </Row>
      <Row id="settings-automation-battery" setting="automation.pauseOnBattery" label={TEXT.battery} hint={TEXT.batteryHint}>
        <Toggle id="settings-automation-battery-toggle" checked={automation.pauseOnBattery} onChange={(pauseOnBattery) => update({ pauseOnBattery })} />
      </Row>
      <Row id="settings-automation-flows" label={TEXT.flows} hint={TEXT.flowsHint}>
        <Button id="settings-automation-flows-open" size="sm" variant="outline" onClick={() => void openAutomations().catch((reason: unknown) => onError(errorText(reason)))}>
          Open automations
        </Button>
      </Row>
    </Section>
  );
}

export function McpSection({ settings, query, onUpdate, onError, onMatch }: SettingsSectionProps) {
  const automation = settings.automation;
  const [status, setStatus] = useState<McpStatus | null>(null);
  const [copied, setCopied] = useState("");
  const visible = matchesQuery(query, mcpSection.title, "cursor claude model context protocol", ...Object.values(MCP_TEXT));
  useReportMatch(mcpSection.id, visible, onMatch);

  useEffect(() => {
    let active = true;
    void mcpStatus()
      .then((next) => active && setStatus(next))
      .catch(() => active && setStatus(null));
    const unlisten = onMcpChanged((next) => setStatus(next));
    return () => {
      active = false;
      void unlisten.then((stop) => stop());
    };
  }, []);

  if (!visible) {
    return null;
  }

  const update = (patch: Partial<AutomationSettings>) => onUpdate({ automation: { ...automation, ...patch } });
  const copy = (what: string, text: string) => {
    void copyText(text)
      .then(() => {
        setCopied(what);
        window.setTimeout(() => setCopied(""), 1500);
      })
      .catch((reason: unknown) => onError(errorText(reason)));
  };
  const statusText = !automation.mcpEnabled
    ? "Off."
    : status?.running
      ? `Listening on ${status.url} · ${status.calls} calls served.`
      : status?.error || "Starting…";

  return (
    <Section id={mcpSection.id} title={mcpSection.title}>
      <Row id="settings-mcp-enabled" setting="automation.mcpEnabled" label={MCP_TEXT.mcp} hint={`${MCP_TEXT.mcpHint} ${statusText}`}>
        <Toggle id="settings-mcp-enabled-toggle" checked={automation.mcpEnabled} onChange={(mcpEnabled) => update({ mcpEnabled })} />
      </Row>
      <Row id="settings-mcp-port" setting="automation.mcpPort" label={MCP_TEXT.port} hint={MCP_TEXT.portHint}>
        <input
          id="settings-mcp-port-input"
          className={inputClass}
          type="number"
          min={1024}
          max={65535}
          value={automation.mcpPort}
          onChange={(event) => update({ mcpPort: Math.max(1024, Math.min(65535, Number(event.target.value) || 17345)) })}
        />
      </Row>
      <Row id="settings-mcp-read-only" setting="automation.mcpReadOnly" label={MCP_TEXT.readOnly} hint={MCP_TEXT.readOnlyHint}>
        <Toggle id="settings-mcp-read-only-toggle" checked={automation.mcpReadOnly} onChange={(mcpReadOnly) => update({ mcpReadOnly })} />
      </Row>
      <Row id="settings-mcp-evaluate" setting="automation.mcpAllowEvaluate" label={MCP_TEXT.evaluate} hint={MCP_TEXT.evaluateHint}>
        <Toggle id="settings-mcp-evaluate-toggle" checked={automation.mcpAllowEvaluate} onChange={(mcpAllowEvaluate) => update({ mcpAllowEvaluate })} />
      </Row>
      <Row id="settings-mcp-token" label={MCP_TEXT.token} hint="Clients send this as a Bearer token. A new token disconnects existing clients.">
        <div id="settings-mcp-token-actions" className="flex flex-wrap justify-end gap-2">
          <Button id="settings-mcp-token-copy" size="sm" variant="outline" disabled={!status?.token} onClick={() => status && copy("token", status.token)}>
            {copied === "token" ? "Copied" : "Copy token"}
          </Button>
          <Button
            id="settings-mcp-token-new"
            size="sm"
            variant="ghost"
            onClick={() => void regenerateMcpToken().then(setStatus).catch((reason: unknown) => onError(errorText(reason)))}
          >
            New token
          </Button>
        </div>
      </Row>
      {status?.config ? (
        <div id="settings-mcp-config" className="px-5 py-3">
          <div id="settings-mcp-config-label" className="flex items-center justify-between text-sm text-zinc-100">
            {MCP_TEXT.config}
            <Button id="settings-mcp-config-copy" size="sm" variant="outline" onClick={() => copy("config", status.config)}>
              {copied === "config" ? "Copied" : "Copy"}
            </Button>
          </div>
          <div id="settings-mcp-config-hint" className="mb-2 text-xs text-zinc-500">
            {MCP_TEXT.configHint}
          </div>
          <pre id="settings-mcp-config-code" className="max-h-48 overflow-auto rounded-md bg-zinc-950 p-3 text-xs text-zinc-300">
            {status.config}
          </pre>
        </div>
      ) : null}
    </Section>
  );
}

export function SiteList({
  id,
  label,
  hint,
  empty,
  sites,
  included = [],
  includedNote,
  onChange,
}: {
  id: string;
  label: string;
  hint: string;
  empty: string;
  sites: string[];
  /** Sites kept on this list from somewhere else. They can't be removed here. */
  included?: string[];
  includedNote?: string;
  onChange: (sites: string[]) => void;
}) {
  const [draft, setDraft] = useState("");
  const visible = useSettingVisible(label, hint, empty, includedNote, ...sites, ...included);
  if (!visible) {
    return null;
  }
  const add = () => {
    const site = draft.trim();
    if (!site) {
      return;
    }
    if (!sites.includes(site)) {
      onChange([...sites, site]);
    }
    setDraft("");
  };
  return (
    <div id={id} className="px-5 py-3">
      <div id={`${id}-label`} className="text-sm text-zinc-100">
        {label}
      </div>
      <div id={`${id}-hint`} className="mb-2 text-xs text-zinc-500">
        {hint}
      </div>
      <div id={`${id}-add`} className="mb-2 flex gap-2">
        <input
          id={`${id}-input`}
          value={draft}
          placeholder="example.com"
          className={inputClass}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              add();
            }
          }}
        />
        <Button id={`${id}-add-button`} size="sm" disabled={!draft.trim()} onClick={add}>
          Add
        </Button>
      </div>
      {sites.length === 0 && included.length === 0 ? (
        <div id={`${id}-empty`} className="text-xs text-zinc-500">
          {empty}
        </div>
      ) : (
        <ul id={`${id}-list`} className="flex flex-col gap-1">
          {sites.map((site) => (
            <li id={`${id}-${site}`} key={site} className="flex h-8 items-center justify-between rounded-md bg-zinc-950 pl-3 pr-1 text-sm text-zinc-200">
              <span id={`${id}-${site}-host`} className="truncate">
                {site}
              </span>
              <IconButton id={`${id}-${site}-remove`} label={`Remove ${site}`} onClick={() => onChange(sites.filter((item) => item !== site))}>
                <X id={`${id}-${site}-remove-icon`} aria-hidden />
              </IconButton>
            </li>
          ))}
          {included.map((site) => (
            <li id={`${id}-included-${site}`} key={site} className="flex h-8 items-center justify-between gap-2 rounded-md bg-zinc-950 pl-3 pr-3 text-sm text-zinc-200">
              <span id={`${id}-included-${site}-host`} className="truncate">
                {site}
              </span>
              {includedNote ? (
                <span id={`${id}-included-${site}-note`} className="shrink-0 text-xs text-zinc-500">
                  {includedNote}
                </span>
              ) : null}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function Secrets({ onError }: { onError: SettingsSectionProps["onError"] }) {
  const [names, setNames] = useState<string[]>([]);
  const [name, setName] = useState("");
  const [value, setValue] = useState("");
  const visible = useSettingVisible(TEXT.secrets, TEXT.secretsHint, "password vault", ...names);

  useEffect(() => {
    void automationSecretNames()
      .then(setNames)
      .catch(() => setNames([]));
  }, []);

  if (!visible) {
    return null;
  }

  const save = () => {
    void setAutomationSecret(name.trim(), value)
      .then((next) => {
        setNames(next);
        setName("");
        setValue("");
        onError(null);
      })
      .catch((reason: unknown) => onError(errorText(reason)));
  };

  return (
    <div id="settings-automation-secrets" className="px-5 py-3">
      <div id="settings-automation-secrets-label" className="text-sm text-zinc-100">
        {TEXT.secrets}
      </div>
      <div id="settings-automation-secrets-hint" className="mb-2 text-xs text-zinc-500">
        {TEXT.secretsHint}
      </div>
      <div id="settings-automation-secrets-add" className="mb-2 flex gap-2">
        <input
          id="settings-automation-secrets-name"
          className={inputClass}
          value={name}
          placeholder="name, like shop.password"
          spellCheck={false}
          onChange={(event) => setName(event.target.value.toLowerCase())}
        />
        <input
          id="settings-automation-secrets-value"
          className={inputClass}
          type="password"
          autoComplete="off"
          value={value}
          placeholder="value"
          onChange={(event) => setValue(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && name.trim() && value) {
              event.preventDefault();
              save();
            }
          }}
        />
        <Button id="settings-automation-secrets-save" size="sm" disabled={!name.trim() || !value} onClick={save}>
          Save
        </Button>
      </div>
      {names.length === 0 ? (
        <div id="settings-automation-secrets-empty" className="text-xs text-zinc-500">
          No saved secrets.
        </div>
      ) : (
        <ul id="settings-automation-secrets-list" className="flex flex-col gap-1">
          {names.map((item) => (
            <li id={`settings-automation-secret-${item}`} key={item} className="flex h-8 items-center justify-between rounded-md bg-zinc-950 pl-3 pr-1 text-sm text-zinc-200">
              <code id={`settings-automation-secret-${item}-name`} className="truncate text-xs">
                {`{{secret:${item}}}`}
              </code>
              <IconButton
                id={`settings-automation-secret-${item}-remove`}
                label={`Delete secret ${item}`}
                onClick={() => void clearAutomationSecret(item).then(setNames).catch((reason: unknown) => onError(errorText(reason)))}
              >
                <X id={`settings-automation-secret-${item}-remove-icon`} aria-hidden />
              </IconButton>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

