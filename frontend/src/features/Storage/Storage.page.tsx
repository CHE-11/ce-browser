import { useEffect, useState } from "react";
import { Globe } from "lucide-react";
import { EmptyState } from "@/components/EmptyState";
import { SkeletonRows } from "@/components/Skeleton";
import { deleteCookie, deleteLocalStorage, listCookies, listLocalStorage, setCookie, setLocalStorage } from "@/lib/browser-client";
import { useAppearanceFromSettings, useTabs } from "@/lib/use-browser";
import { Button } from "@/components/Button";
import type { SiteCookie, StoragePair } from "@type/browser";

type StoragePageProps = {
  params: URLSearchParams;
};

export function StoragePage({ params }: StoragePageProps) {
  const ready = useAppearanceFromSettings();
  const snapshot = useTabs();
  const requested = Number(params.get("tab"));
  const tab = snapshot.tabs.find((item) => item.id === requested) ?? snapshot.tabs.find((item) => item.id === snapshot.activeId);
  const [cookies, setCookies] = useState<SiteCookie[]>([]);
  const [items, setItems] = useState<StoragePair[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState("");
  const [cookieName, setCookieName] = useState("");
  const [cookieValue, setCookieValue] = useState("");
  const [itemKey, setItemKey] = useState("");
  const [itemValue, setItemValue] = useState("");

  useEffect(() => {
    document.title = "Site storage";
  }, []);

  useEffect(() => {
    if (!tab) {
      return;
    }
    let gone = false;
    setError("");
    setLoaded(false);
    void Promise.all([listCookies(tab.id), listLocalStorage(tab.id)])
      .then(([nextCookies, nextItems]) => {
        if (!gone) {
          setCookies(nextCookies);
          setItems(nextItems);
        }
      })
      .catch((reason: unknown) => {
        if (!gone) {
          setError(reason instanceof Error ? reason.message : "Couldn't read this site.");
        }
      })
      .finally(() => {
        if (!gone) {
          setLoaded(true);
        }
      });
    return () => {
      gone = true;
    };
  }, [tab?.id, tab?.url]);

  if (!ready) {
    return null;
  }

  const reload = () => {
    if (!tab) {
      return;
    }
    void listCookies(tab.id).then(setCookies).catch(() => undefined);
    void listLocalStorage(tab.id).then(setItems).catch(() => undefined);
  };

  return (
    <div id="storage-page" className="mx-auto flex h-full max-w-4xl flex-col gap-6 overflow-auto bg-zinc-950 p-8 text-zinc-100">
      <div id="storage-header">
        <h1 id="storage-title" className="text-xl font-semibold">
          Site storage
        </h1>
        <p id="storage-site" className="truncate text-sm text-zinc-500">
          {tab?.url || "Open a site first."}
        </p>
      </div>
      {error ? (
        <p id="storage-error" className="text-sm text-amber-300">
          {error}
        </p>
      ) : null}
      <section id="storage-cookies" className="flex flex-col gap-2">
        <h2 id="storage-cookies-title" className="text-sm font-semibold">
          Cookies
        </h2>
        <form
          id="storage-cookie-form"
          className="flex gap-2"
          onSubmit={(event) => {
            event.preventDefault();
            if (!tab || !cookieName.trim()) {
              return;
            }
            void setCookie(tab.id, cookieName.trim(), cookieValue).then(() => {
              setCookieName("");
              setCookieValue("");
              reload();
            });
          }}
        >
          <input id="storage-cookie-name" value={cookieName} placeholder="Name" className="h-8 flex-1 rounded-md border border-zinc-700 bg-zinc-900 px-2 text-sm" onChange={(event) => setCookieName(event.target.value)} />
          <input id="storage-cookie-value" value={cookieValue} placeholder="Value" className="h-8 flex-1 rounded-md border border-zinc-700 bg-zinc-900 px-2 text-sm" onChange={(event) => setCookieValue(event.target.value)} />
          <Button id="storage-cookie-add" type="submit" variant="primary" disabled={!tab}>
            Add
          </Button>
        </form>
        {tab && !loaded ? <SkeletonRows id="storage-cookie-loading" rows={3} /> : null}
        {!tab ? (
          <EmptyState id="storage-no-site" icon={Globe} title="Open a site first" hint="Cookies and storage belong to the site in the tab you opened this page from." />
        ) : loaded && cookies.length === 0 ? (
          <p id="storage-cookie-empty" className="rounded-md border border-dashed border-zinc-800 px-3 py-4 text-center text-xs text-zinc-500">
            This site has no cookies.
          </p>
        ) : null}
        <ul id="storage-cookie-list" className="flex flex-col gap-1">
          {cookies.map((cookie) => (
            <li id={`storage-cookie-${cookie.name}-${cookie.path}`} key={`${cookie.domain}-${cookie.path}-${cookie.name}`} className="flex items-center gap-2 rounded-md bg-zinc-900 px-3 py-2">
              <span id={`storage-cookie-label-${cookie.name}`} className="min-w-0 flex-1 truncate text-sm">
                {cookie.name}={cookie.value}
              </span>
              <Button
                id={`storage-cookie-delete-${cookie.name}`}
                disabled={!tab}
                onClick={() => tab && void deleteCookie(tab.id, cookie.name, cookie.domain, cookie.path).then(reload)}
              >
                Delete
              </Button>
            </li>
          ))}
        </ul>
      </section>
      <section id="storage-local" className="flex flex-col gap-2">
        <h2 id="storage-local-title" className="text-sm font-semibold">
          localStorage
        </h2>
        <form
          id="storage-local-form"
          className="flex gap-2"
          onSubmit={(event) => {
            event.preventDefault();
            if (!tab || !itemKey) {
              return;
            }
            void setLocalStorage(tab.id, itemKey, itemValue).then(() => {
              setItemKey("");
              setItemValue("");
              reload();
            });
          }}
        >
          <input id="storage-local-key" value={itemKey} placeholder="Key" className="h-8 flex-1 rounded-md border border-zinc-700 bg-zinc-900 px-2 text-sm" onChange={(event) => setItemKey(event.target.value)} />
          <input id="storage-local-value" value={itemValue} placeholder="Value" className="h-8 flex-1 rounded-md border border-zinc-700 bg-zinc-900 px-2 text-sm" onChange={(event) => setItemValue(event.target.value)} />
          <Button id="storage-local-add" type="submit" variant="primary" disabled={!tab}>
            Set
          </Button>
        </form>
        {tab && !loaded ? <SkeletonRows id="storage-local-loading" rows={3} /> : null}
        {tab && loaded && items.length === 0 ? (
          <p id="storage-local-empty" className="rounded-md border border-dashed border-zinc-800 px-3 py-4 text-center text-xs text-zinc-500">
            Nothing is saved in localStorage for this site.
          </p>
        ) : null}
        <ul id="storage-local-list" className="flex flex-col gap-1">
          {items.map((item) => (
            <li id={`storage-item-${item.key}`} key={item.key} className="flex items-center gap-2 rounded-md bg-zinc-900 px-3 py-2">
              <span id={`storage-item-label-${item.key}`} className="min-w-0 flex-1 truncate text-sm">
                {item.key}={item.value}
              </span>
              <Button id={`storage-item-delete-${item.key}`} disabled={!tab} onClick={() => tab && void deleteLocalStorage(tab.id, item.key).then(reload)}>
                Delete
              </Button>
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}
