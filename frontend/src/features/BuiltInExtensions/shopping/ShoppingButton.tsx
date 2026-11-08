import { useEffect, useState } from "react";
import { ShoppingCart } from "lucide-react";
import { listPrices, onPricesChanged } from "@/lib/browser-client";
import { siteDomain } from "@/features/BuiltInExtensions/shopping/format";
import type { ProductPrice, TabInfo } from "@type/browser";
import { ToolButton } from "../ToolButton";

export function ShoppingButton({ activeTab }: { activeTab: TabInfo | undefined }) {
  const [items, setItems] = useState<ProductPrice[]>([]);
  const domain = siteDomain(activeTab?.url ?? "");
  const count = domain ? items.filter((item) => siteDomain(item.url) === domain).length : 0;

  useEffect(() => {
    void listPrices().then(setItems).catch(() => undefined);
    const listen = onPricesChanged(setItems);
    return () => {
      void listen.then((unlisten) => unlisten());
    };
  }, []);

  const label = count === 0 ? "Shopping" : `Shopping, ${count} tracked on this site`;

  return (
    <ToolButton
      id="shopping"
      label={label}
      kind="shopping"
      icon={<ShoppingCart id="address-shopping-icon" aria-hidden />}
      activeTab={activeTab}
      needsPage={false}
      badge={count}
    />
  );
}
