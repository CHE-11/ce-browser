import { useEffect, useState } from "react";
import { ShoppingCart } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { listPrices, onPricesChanged, toggleShoppingPanel } from "@/lib/browser-client";
import { siteDomain } from "@/built_in_extensions/shopping/format";
import type { ProductPrice, TabInfo } from "@type/browser";
import { useTabs } from "@/lib/use-browser";

function badgeText(count: number) {
  return count > 99 ? "99+" : String(count);
}

export function ShoppingButton({ activeTab }: { activeTab: TabInfo | undefined }) {
  const { shoppingPanel } = useTabs();
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
    <IconButton
      id="address-shopping"
      label={label}
      active={shoppingPanel}
      aria-pressed={shoppingPanel}
      className="relative"
      onClick={() => void toggleShoppingPanel()}
    >
      <ShoppingCart id="address-shopping-icon" aria-hidden />
      {count > 0 ? (
        <span
          id="address-shopping-badge"
          className="pointer-events-none absolute -top-0.5 -right-0.5 min-w-4 rounded-full bg-sky-600 px-1 text-center text-[9px] font-semibold leading-4 text-white"
          aria-hidden
        >
          {badgeText(count)}
        </span>
      ) : null}
    </IconButton>
  );
}
