import { useEffect, useState } from "react";
import type { ModelPrice } from "@type/browser";
import { aiModelPrices, listAiModels } from "./browser-client";

type ModelCatalog = { models: string[]; prices: Record<string, ModelPrice> };

let catalogRequest: Promise<ModelCatalog> | null = null;
const extraPrices = new Map<string, Promise<ModelPrice | null>>();

function loadCatalog() {
  catalogRequest ??= listAiModels()
    .catch(() => [] as string[])
    .then(async (models) => ({ models, prices: await aiModelPrices(models).catch(() => ({})) }))
    .catch(() => {
      catalogRequest = null;
      return { models: [], prices: {} };
    });
  return catalogRequest;
}

/** The active provider's models with their prices. Loads once per panel. */
export function useModelCatalog() {
  const [catalog, setCatalog] = useState<ModelCatalog | null>(null);

  useEffect(() => {
    let live = true;
    void loadCatalog().then((next) => live && setCatalog(next));
    return () => {
      live = false;
    };
  }, []);

  return { models: catalog?.models ?? [], prices: catalog?.prices ?? {}, loading: catalog === null };
}

/** The price of one model, such as the default from Settings, which may not be in the list. */
export function useModelPrice(model: string) {
  const { prices, loading } = useModelCatalog();
  const [extra, setExtra] = useState<{ model: string; price: ModelPrice | null } | null>(null);
  const listed = model ? prices[model] : undefined;

  useEffect(() => {
    if (!model || loading || listed) {
      return;
    }
    let live = true;
    let request = extraPrices.get(model);
    if (!request) {
      request = aiModelPrices([model])
        .then((found) => found[model] ?? null)
        .catch(() => null);
      extraPrices.set(model, request);
    }
    void request.then((price) => live && setExtra({ model, price }));
    return () => {
      live = false;
    };
  }, [model, loading, listed]);

  return listed ?? (extra?.model === model ? extra.price : null);
}

function dollars(amount: number) {
  if (amount === 0) {
    return "$0";
  }
  if (amount < 0.1) {
    return `$${amount.toFixed(3).replace(/0+$/, "")}`;
  }
  if (amount < 10) {
    return `$${amount.toFixed(2).replace(/\.?0+$/, "")}`;
  }
  return `$${Math.round(amount)}`;
}

/** `$2 / $10` per million input and output tokens, or `Free` for local models. */
export function formatPrice(price: ModelPrice | null | undefined) {
  if (!price) {
    return "";
  }
  if (price.local) {
    return "Free";
  }
  return `${dollars(price.input)} / ${dollars(price.output)}`;
}

/** What a run cost so far, in US dollars, or null when the price is unknown. */
export function runCost(price: ModelPrice | null | undefined, tokensIn: number, tokensOut: number) {
  if (!price) {
    return null;
  }
  if (price.local) {
    return 0;
  }
  return (tokensIn * price.input + tokensOut * price.output) / 1_000_000;
}

export function formatCost(cost: number) {
  if (cost === 0) {
    return "$0";
  }
  return cost < 0.01 ? "<$0.01" : `$${cost.toFixed(2)}`;
}
