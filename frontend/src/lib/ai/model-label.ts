/** Short-context output price in dollars per million tokens. */
const OUTPUT_PRICE: Record<string, number> = {
  "gpt-6-astra": 50,
  "gpt-6.1-sol": 10,
  "gpt-6.1-terra": 5,
  "gpt-6-luna": 0.5,
  "gpt-6-sol": 10,
  "gpt-5.6-sol": 20,
  "gpt-5.6-terra": 12,
  "gpt-5.6-luna": 1.2,
  "gpt-5.5": 30,
  "gpt-5.5-pro": 180,
  "gpt-5.4": 15,
  "gpt-5.4-pro": 180,
  "gpt-5.4-mini": 4.5,
  "gpt-5.4-nano": 1.25,
  "gpt-5.2": 14,
  "gpt-5.2-pro": 168,
  "gpt-5.1": 10,
  "gpt-5": 10,
  "gpt-5-mini": 2,
  "gpt-5-nano": 0.4,
  "gpt-5-pro": 120,
  "gpt-4.1": 8,
  "gpt-4.1-mini": 1.6,
  "gpt-4o": 10,
  "gpt-4o-mini": 0.6,
  "o3-pro": 80,
  "o3": 8,
  "o4-mini": 4,
  "o3-mini": 4,
  "o1": 60,
  "o1-mini": 12,
  "claude-fable-5-1": 50,
  "claude-fable-5": 50,
  "claude-mythos-5-1": 50,
  "claude-mythos-5": 50,
  "claude-opus-5-5": 20,
  "claude-opus-5": 25,
  "claude-opus-4-8": 25,
  "claude-opus-4-7": 25,
  "claude-opus-4-6": 25,
  "claude-opus-4-5": 25,
  "claude-sonnet-5-5": 10,
  "claude-sonnet-5": 10,
  "claude-sonnet-4-6": 15,
  "claude-sonnet-4-5": 15,
  "claude-haiku-5-5": 0.5,
  "claude-haiku-4-5": 5,
};

/** `$` is the cheap end. `$$$$` is the expensive end. */
export function modelPriceMarks(id: string) {
  const price = outputPrice(modelFamily(id));
  if (price === null) {
    return "";
  }
  if (price < 3) {
    return "$";
  }
  if (price < 12) {
    return "$$";
  }
  if (price < 20) {
    return "$$$";
  }
  return "$$$$";
}

export function modelLabel(id: string) {
  const marks = modelPriceMarks(id);
  return marks ? `${id} ${marks}` : id;
}

/** OpenAI reasoning models and current Claude models take a thinking effort. */
export function modelAcceptsThinking(provider: string, id: string) {
  const name = modelFamily(id);
  if (!name || name.includes("chat")) {
    return false;
  }
  if (provider === "openai") {
    return openaiThinks(name);
  }
  if (provider === "claude") {
    return claudeThinks(name);
  }
  return false;
}

function modelFamily(id: string) {
  const name = id.trim().toLowerCase();
  return name.replace(/-\d{8}$/, "").replace(/-\d{4}-\d{2}-\d{2}$/, "");
}

function outputPrice(id: string) {
  const listed = OUTPUT_PRICE[id];
  if (listed !== undefined) {
    return listed;
  }
  if (id.startsWith("ft:")) {
    return outputPrice(id.slice(3).split(":")[0] ?? "");
  }
  if (/(nano|mini|luna|haiku)/.test(id)) {
    return 1;
  }
  if (/(pro|astra|fable|mythos|cyber)/.test(id)) {
    return 50;
  }
  if (id.includes("sol") || id.includes("opus")) {
    return 20;
  }
  if (id.includes("terra")) {
    return 5;
  }
  if (id.includes("sonnet") || id.startsWith("gpt-") || /^o\d/.test(id)) {
    return 10;
  }
  return null;
}

function openaiThinks(id: string) {
  if (/^o\d($|-)/.test(id)) {
    return true;
  }
  const version = id.match(/^gpt-(\d)/);
  return version !== null && version[1] !== "3" && version[1] !== "4";
}

function claudeThinks(id: string) {
  if (id.startsWith("claude-fable") || id.startsWith("claude-mythos") || id.includes("-3-7")) {
    return true;
  }
  const match = id.match(/^claude-(opus|sonnet|haiku)-(\d+)(?:-(\d+))?/);
  if (!match) {
    return false;
  }
  const major = Number(match[2]);
  const minor = Number(match[3] ?? 0);
  if (match[1] === "haiku") {
    return major > 4 || (major === 4 && minor >= 5);
  }
  return major >= 4;
}
