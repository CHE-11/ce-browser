import { useState } from "react";
import { SendHorizontal } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { cn } from "@/lib/cn";
import { inputClassName } from "./agent-styles";

const EXAMPLES = [
  "Summarize this page and list the key facts",
  "Find the cheapest option on this page and add it to the cart",
  "Fill in this form with my details, then stop before submitting",
  "Go through the next 3 pages and collect every product name and price",
];

type Props = {
  /** `start` begins a run in the active tab. `reply` sends a message to the running agent. */
  mode: "start" | "reply";
  disabled?: boolean;
  onSubmit: (text: string) => void;
};

export function AgentComposer({ mode, disabled, onSubmit }: Props) {
  const [text, setText] = useState("");
  const send = () => {
    const value = text.trim();
    if (!value || disabled) {
      return;
    }
    onSubmit(value);
    setText("");
  };

  return (
    <div id="agent-composer" className="flex flex-col gap-2">
      {mode === "start" && !text ? (
        <ul id="agent-composer-examples" className="flex flex-col gap-1">
          {EXAMPLES.map((example, index) => (
            <li id={`agent-composer-example-${index}`} key={example}>
              <button
                id={`agent-composer-example-${index}-button`}
                type="button"
                className="w-full rounded-md px-2 py-1 text-left text-xs text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100"
                onClick={() => setText(example)}
              >
                {example}
              </button>
            </li>
          ))}
        </ul>
      ) : null}
      <div id="agent-composer-box" className="relative">
        <textarea
          id="agent-composer-input"
          className={cn(inputClassName, "min-h-16 resize-none pr-10")}
          rows={3}
          value={text}
          disabled={disabled}
          placeholder={mode === "start" ? "What should the agent do in this tab?" : "Tell the agent something…"}
          onChange={(event) => setText(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              send();
            }
          }}
        />
        <IconButton
          id="agent-composer-send"
          label={mode === "start" ? "Start (Enter)" : "Send (Enter)"}
          className="absolute bottom-1.5 right-1.5 size-7"
          disabled={disabled || !text.trim()}
          onClick={send}
        >
          <SendHorizontal id="agent-composer-send-icon" aria-hidden />
        </IconButton>
      </div>
    </div>
  );
}
