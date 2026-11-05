(() => {
  if (__CE_ENABLED__ !== true) return;
  if (location.protocol !== "http:" && location.protocol !== "https:") return;
  const paused = __CE_PAUSED__;
  const host = String(location.hostname || "")
    .trim()
    .replace(/\.+$/, "")
    .toLowerCase();
  const key = host.startsWith("www.") && host.length > 4 ? host.slice(4) : host;
  const names = [];
  let current = key;
  while (current) {
    names.push(current);
    const dot = current.indexOf(".");
    if (dot === -1) break;
    const parent = current.slice(dot + 1);
    if (!parent.includes(".")) break;
    current = parent;
  }
  if (names.some((name) => paused.includes(name))) return;

  const prefs = { ioo: () => true };
  try {
    Object.defineProperty(window, "_gaUserPrefs", {
      configurable: true,
      get: () => prefs,
      set: () => true,
    });
  } catch (e) {}

  const disable = (id) => {
    if (typeof id === "string" && id) window["ga-disable-" + id] = true;
  };
  const ga = function () {
    if (arguments[0] === "create") disable(arguments[1]);
  };
  ga.q = [];
  ga.l = Date.now();
  ga.getAll = () => [];
  ga.getByName = () => null;
  ga.create = (id) => {
    disable(id);
    return { get() {}, set() {}, send() {} };
  };
  const gtag = function () {
    const cmd = arguments[0];
    if (cmd === "config") disable(arguments[1]);
  };
  const hold = (name, value) => {
    try {
      Object.defineProperty(window, name, {
        configurable: true,
        get: () => value,
        set: () => true,
      });
    } catch (e) {}
  };
  hold("ga", ga);
  hold("gtag", gtag);

  const ads = [];
  ads.push = () => ads.length;
  hold("adsbygoogle", ads);

  const slot = {
    addService() {
      return this;
    },
    setTargeting() {
      return this;
    },
    defineSizeMapping() {
      return this;
    },
  };
  const pubads = {
    enableSingleRequest() {},
    collapseEmptyDivs() {},
    disableInitialLoad() {},
    refresh() {},
    addEventListener() {},
    getSlots() {
      return [];
    },
    setTargeting() {
      return this;
    },
    setPrivacySettings() {
      return this;
    },
  };
  const googletag = {
    cmd: [],
    defineSlot() {
      return slot;
    },
    defineOutOfPageSlot() {
      return slot;
    },
    pubads() {
      return pubads;
    },
    companionAds() {
      return pubads;
    },
    enableServices() {},
    display() {},
    destroySlots() {},
    setAdIframeTitle() {},
  };
  googletag.cmd.push = (fn) => {
    try {
      if (typeof fn === "function") fn();
    } catch (e) {}
    return googletag.cmd.length;
  };
  hold("googletag", googletag);

  const fbq = function () {};
  fbq.queue = [];
  fbq.loaded = true;
  fbq.version = "2.0";
  fbq.push = fbq;
  fbq.callMethod = function () {};
  hold("fbq", fbq);
  hold("_fbq", fbq);

  const mount = () => {
    if (document.getElementById("ce-adblock")) return true;
    const root = document.head || document.documentElement;
    if (!root) return false;
    const style = document.createElement("style");
    style.id = "ce-adblock";
    style.textContent = __CE_CSS__;
    root.appendChild(style);
    return true;
  };
  if (!mount() && document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", mount, { once: true });
  }
})();
