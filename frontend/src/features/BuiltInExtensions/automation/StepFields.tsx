import type { AgentAutonomy, FlowStep } from "@type/browser";
import { AUTONOMY_LEVELS } from "@/lib/agent-autonomy";
import { cn } from "@/lib/cn";
import { inputClassName } from "@/features/BuiltInExtensions/agent/agent-styles";
import { TARGET_KINDS } from "./flow-model";
import { SpecPicker } from "./SpecPicker";
import { Field } from "./StepField";
import { TargetEditor } from "./TargetEditor";

type StepFieldsProps = {
  id: string;
  step: FlowStep;
  /** Tab to pick elements and lists in. */
  tabId: number | null;
  set: (change: Partial<FlowStep>) => void;
};

/** The inputs one step kind needs. */
export function StepFields({ id, step, tabId, set }: StepFieldsProps) {
  return (
    <>
      {TARGET_KINDS.includes(step.kind) && step.kind !== "drag" ? <TargetEditor id={`${id}-target`} target={step.target} tabId={tabId} onChange={(target) => set({ target })} /> : null}

      {step.kind === "navigate" ? (
        <Field id={`${id}-url`} label="Address">
          <input id={`${id}-url-input`} className={inputClassName} value={step.url} placeholder="https://example.com/{{page}}" onChange={(event) => set({ url: event.target.value })} />
        </Field>
      ) : null}

      {step.kind === "click" ? (
        <div id={`${id}-click`} className="flex flex-col gap-3">
          <div id={`${id}-click-row`} className="flex flex-wrap items-end gap-3">
            <Field id={`${id}-button`} label="Button" className="w-40">
              <select id={`${id}-button-input`} className={inputClassName} value={step.keys || "left"} onChange={(event) => set({ keys: event.target.value === "left" ? "" : event.target.value })}>
                <option id={`${id}-button-left`} value="left">
                  Left
                </option>
                <option id={`${id}-button-right`} value="right">
                  Right
                </option>
                <option id={`${id}-button-middle`} value="middle">
                  Middle
                </option>
              </select>
            </Field>
            <label id={`${id}-double`} className="flex items-center gap-2 pb-2 text-xs text-zinc-300">
              <input
                id={`${id}-double-input`}
                type="checkbox"
                className="size-3.5 accent-sky-500"
                checked={step.clicks >= 2}
                onChange={(event) => set({ clicks: event.target.checked ? 2 : 1 })}
              />
              Double-click
            </label>
            <label id={`${id}-follow`} className="flex items-center gap-2 pb-2 text-xs text-zinc-300">
              <input
                id={`${id}-follow-input`}
                type="checkbox"
                className="size-3.5 accent-sky-500"
                checked={step.follow}
                onChange={(event) => set({ follow: event.target.checked })}
              />
              Follow the new tab
            </label>
          </div>
          <PointFields id={id} x={step.x} y={step.y} onChange={(x, y) => set({ x, y })} />
        </div>
      ) : null}

      {step.kind === "drag" ? (
        <div id={`${id}-drag`} className="flex flex-col gap-3">
          <p id={`${id}-drag-start-label`} className="text-xs text-zinc-500">
            Start
          </p>
          <TargetEditor id={`${id}-from`} target={step.target} tabId={tabId} onChange={(target) => set({ target })} />
          <PointFields id={`${id}-from-point`} x={step.x} y={step.y} onChange={(x, y) => set({ x, y })} />
          <p id={`${id}-drag-end-label`} className="text-xs text-zinc-500">
            End
          </p>
          <TargetEditor id={`${id}-to`} target={step.to} tabId={tabId} onChange={(to) => set({ to })} />
          <PointFields id={`${id}-to-point`} x={step.toX} y={step.toY} onChange={(toX, toY) => set({ toX, toY })} />
        </div>
      ) : null}

      {step.kind === "dialog" ? (
        <div id={`${id}-dialog`} className="flex gap-2">
          <Field id={`${id}-dialog-action`} label="Answer" className="w-40">
            <select id={`${id}-dialog-action-input`} className={inputClassName} value={step.keys === "dismiss" ? "dismiss" : "accept"} onChange={(event) => set({ keys: event.target.value === "dismiss" ? "dismiss" : "" })}>
              <option id={`${id}-dialog-accept`} value="accept">
                Accept
              </option>
              <option id={`${id}-dialog-dismiss`} value="dismiss">
                Dismiss
              </option>
            </select>
          </Field>
          <Field id={`${id}-dialog-text`} label="Prompt text" className="flex-1">
            <input id={`${id}-dialog-text-input`} className={inputClassName} value={step.text} placeholder="Only for a prompt" onChange={(event) => set({ text: event.target.value })} />
          </Field>
        </div>
      ) : null}

      {step.kind === "desktopClick" ? <PointFields id={id} x={step.x} y={step.y} placeholder="Screen px" onChange={(x, y) => set({ x, y })} /> : null}

      {step.kind === "desktopDrag" ? (
        <div id={`${id}-desktop-drag`} className="flex flex-col gap-3">
          <PointFields id={`${id}-desktop-from`} x={step.x} y={step.y} placeholder="Screen px" onChange={(x, y) => set({ x, y })} />
          <PointFields id={`${id}-desktop-to`} x={step.toX} y={step.toY} placeholder="Screen px" onChange={(toX, toY) => set({ toX, toY })} />
        </div>
      ) : null}

      {step.kind === "desktopType" ? (
        <Field id={`${id}-desktop-text`} label="Text">
          <textarea id={`${id}-desktop-text-input`} rows={2} className={inputClassName} value={step.text} onChange={(event) => set({ text: event.target.value })} />
        </Field>
      ) : null}

      {step.kind === "type" ? (
        <Field id={`${id}-text`} label="Text">
          <textarea id={`${id}-text-input`} rows={2} className={inputClassName} value={step.text} onChange={(event) => set({ text: event.target.value })} />
        </Field>
      ) : null}

      {step.kind === "press" ? (
        <Field id={`${id}-keys`} label="Keys" className="w-64">
          <input id={`${id}-keys-input`} className={inputClassName} value={step.keys} placeholder="Enter, Escape, Control+A" onChange={(event) => set({ keys: event.target.value })} />
        </Field>
      ) : null}

      {step.kind === "select" ? (
        <Field id={`${id}-values`} label="Options, separated by commas">
          <input
            id={`${id}-values-input`}
            className={inputClassName}
            value={step.values.join(", ")}
            onChange={(event) =>
              set({
                values: event.target.value
                  .split(",")
                  .map((value) => value.trim())
                  .filter(Boolean),
              })
            }
          />
        </Field>
      ) : null}

      {step.kind === "upload" ? (
        <Field id={`${id}-files`} label="Files, one full path per line">
          <textarea
            id={`${id}-files-input`}
            rows={2}
            className={cn(inputClassName, "font-mono text-xs")}
            value={step.values.join("\n")}
            onChange={(event) =>
              set({
                values: event.target.value
                  .split("\n")
                  .map((value) => value.trim())
                  .filter(Boolean),
              })
            }
          />
        </Field>
      ) : null}

      {step.kind === "scroll" ? (
        <div id={`${id}-scroll`} className="flex gap-2">
          <Field id={`${id}-direction`} label="Direction" className="w-40">
            <select id={`${id}-direction-input`} className={inputClassName} value={step.text || "down"} onChange={(event) => set({ text: event.target.value })}>
              <option id={`${id}-direction-down`} value="down">
                Down
              </option>
              <option id={`${id}-direction-up`} value="up">
                Up
              </option>
              <option id={`${id}-direction-left`} value="left">
                Left
              </option>
              <option id={`${id}-direction-right`} value="right">
                Right
              </option>
            </select>
          </Field>
          <Field id={`${id}-screens`} label="Screens" className="w-28">
            <input
              id={`${id}-screens-input`}
              type="number"
              min={0.1}
              step={0.5}
              className={inputClassName}
              value={step.seconds || 1}
              onChange={(event) => set({ seconds: Number(event.target.value) })}
            />
          </Field>
        </div>
      ) : null}

      {step.kind === "wait" ? (
        <Field id={`${id}-seconds`} label="Seconds" className="w-28">
          <input
            id={`${id}-seconds-input`}
            type="number"
            min={0.1}
            step={0.5}
            className={inputClassName}
            value={step.seconds}
            onChange={(event) => set({ seconds: Number(event.target.value) })}
          />
        </Field>
      ) : null}

      {step.kind === "waitFor" || step.kind === "assert" ? (
        <div id={`${id}-wait`} className="flex gap-2">
          <Field id={`${id}-wait-text`} label="Text on the page" className="flex-1">
            <input id={`${id}-wait-text-input`} className={inputClassName} value={step.text} onChange={(event) => set({ text: event.target.value })} />
          </Field>
          {step.kind === "waitFor" ? (
            <Field id={`${id}-wait-mode`} label="Until it" className="w-36">
              <select
                id={`${id}-wait-mode-input`}
                className={inputClassName}
                value={step.keys === "gone" ? "gone" : "appears"}
                onChange={(event) => set({ keys: event.target.value === "gone" ? "gone" : "" })}
              >
                <option id={`${id}-wait-mode-appears`} value="appears">
                  Appears
                </option>
                <option id={`${id}-wait-mode-gone`} value="gone">
                  Goes away
                </option>
              </select>
            </Field>
          ) : null}
          <Field id={`${id}-wait-timeout`} label="Give up after (s)" className="w-32">
            <input
              id={`${id}-wait-timeout-input`}
              type="number"
              min={0}
              className={inputClassName}
              value={step.seconds || ""}
              placeholder={step.kind === "assert" ? "5" : "15"}
              onChange={(event) => set({ seconds: Number(event.target.value) })}
            />
          </Field>
        </div>
      ) : null}

      {step.kind === "extract" ? (
        <div id={`${id}-extract`} className="flex flex-col gap-3">
          <SpecPicker id={`${id}-spec`} spec={step.spec} tabId={tabId} onPick={(spec) => set({ spec })} />
          <div id={`${id}-extract-paging`} className="flex gap-2">
            <Field id={`${id}-pages`} label="Pages to follow" className="w-32">
              <input
                id={`${id}-pages-input`}
                type="number"
                min={1}
                max={100}
                className={inputClassName}
                value={step.pages || 1}
                onChange={(event) => set({ pages: Number(event.target.value) })}
              />
            </Field>
            <Field id={`${id}-next`} label="Next link text (blank finds it)" className="flex-1">
              <input id={`${id}-next-input`} className={inputClassName} value={step.text} placeholder="Next" onChange={(event) => set({ text: event.target.value })} />
            </Field>
          </div>
          <Field id={`${id}-extract-prompt`} label="Or describe the data for AI extraction">
            <input
              id={`${id}-extract-prompt-input`}
              className={inputClassName}
              value={step.prompt}
              placeholder="Every product with its name, price and rating"
              onChange={(event) => set({ prompt: event.target.value })}
            />
          </Field>
        </div>
      ) : null}

      {step.kind === "ai" ? (
        <div id={`${id}-ai`} className="flex flex-col gap-3">
          <Field id={`${id}-prompt`} label="What the agent should do here">
            <textarea
              id={`${id}-prompt-input`}
              rows={3}
              className={inputClassName}
              value={step.prompt}
              placeholder="Find the cheapest shipping option and choose it"
              onChange={(event) => set({ prompt: event.target.value })}
            />
          </Field>
          <Field id={`${id}-autonomy`} label="How much it may do without asking" className="w-56">
            <select
              id={`${id}-autonomy-input`}
              className={inputClassName}
              value={step.autonomy ?? ""}
              title={AUTONOMY_LEVELS.find((level) => level.value === step.autonomy)?.hint ?? "Uses the default from Settings."}
              onChange={(event) => set({ autonomy: event.target.value as AgentAutonomy | "" })}
            >
              <option id={`${id}-autonomy-default`} value="">
                Default from Settings
              </option>
              {AUTONOMY_LEVELS.map((level) => (
                <option id={`${id}-autonomy-${level.value}`} key={level.value} value={level.value}>
                  {level.label}
                </option>
              ))}
            </select>
          </Field>
        </div>
      ) : null}

      {step.kind === "notify" ? (
        <Field id={`${id}-message`} label="Message. {{output}} is the last AI answer, {{rows}} the rows extracted.">
          <input id={`${id}-message-input`} className={inputClassName} value={step.text} onChange={(event) => set({ text: event.target.value })} />
        </Field>
      ) : null}
    </>
  );
}

function PointFields({
  id,
  x,
  y,
  onChange,
  placeholder = "CSS px",
}: {
  id: string;
  x: number | null;
  y: number | null;
  onChange: (x: number | null, y: number | null) => void;
  placeholder?: string;
}) {
  const change = (axis: "x" | "y", raw: string) => {
    const parsed = raw.trim() === "" ? null : Number(raw);
    const next = parsed != null && Number.isFinite(parsed) ? parsed : null;
    if (axis === "x") {
      onChange(next, y);
    } else {
      onChange(x, next);
    }
  };
  return (
    <div id={`${id}-point`} className="flex gap-2">
      <Field id={`${id}-x`} label="X" className="w-28">
        <input id={`${id}-x-input`} type="number" className={inputClassName} value={x ?? ""} placeholder={placeholder} onChange={(event) => change("x", event.target.value)} />
      </Field>
      <Field id={`${id}-y`} label="Y" className="w-28">
        <input id={`${id}-y-input`} type="number" className={inputClassName} value={y ?? ""} placeholder={placeholder} onChange={(event) => change("y", event.target.value)} />
      </Field>
    </div>
  );
}
