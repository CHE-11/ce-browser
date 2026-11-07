import { useState } from "react";
import { ChevronRight, SendHorizontal } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { cn } from "@/lib/cn";
import { inputClassName } from "./agent-styles";

type Props = {
  /** `start` begins a run in the active tab. `reply` sends a message to the running agent. */
  mode: "start" | "reply";
  disabled?: boolean;
  suggestions?: string[];
  onSubmit: (text: string) => void;
};

export function AgentComposer({ mode, disabled, suggestions = [], onSubmit }: Props) {
  const [text, setText] = useState("");
  const [suggestionsOpen, setSuggestionsOpen] = useState(false);
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
      {mode === "start" && !text && suggestions.length > 0 ? (
        <div id="agent-composer-examples">
          <button
            id="agent-composer-examples-toggle"
            type="button"
            className="flex items-center gap-1 rounded-md px-1 py-0.5 text-xs text-zinc-400 hover:text-zinc-100"
            aria-expanded={suggestionsOpen}
            aria-controls="agent-composer-examples-list"
            onClick={() => setSuggestionsOpen((open) => !open)}
          >
            <ChevronRight
              id="agent-composer-examples-toggle-icon"
              className={cn("size-3.5 transition-transform", suggestionsOpen && "rotate-90")}
              aria-hidden
            />
            Suggestions
          </button>
          {suggestionsOpen ? (
            <ul id="agent-composer-examples-list" className="mt-1 flex flex-col gap-1">
              {suggestions.map((example, index) => (
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
        </div>
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
