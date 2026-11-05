//! What runs: finds the frameworks, languages, libraries, services and white-label products a tab's page uses.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::{tabs, webview2};

const FAILURE: &str = "Couldn't scan this page.";
const CATEGORIES: &[&str] = &[
    "platform",
    "framework",
    "language",
    "library",
    "ui",
    "cms",
    "analytics",
    "server",
    "hosting",
    "build",
    "other",
];
const MAX_TECHNOLOGIES: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedTechnology {
    pub name: String,
    pub category: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct PageTechnologies {
    pub technologies: Vec<DetectedTechnology>,
}

/// Runs in the page's main world so it can read the globals that libraries leave behind.
/// Every check is guarded: a page that breaks one check still reports the rest.
const DETECT_SCRIPT: &str = r##"(async () => {
  const found = new Map();
  const add = (name, category, version, url) => {
    const key = name.toLowerCase();
    let clean = null;
    if (typeof version === "string" || typeof version === "number") {
      clean = String(version).trim().replace(/^v(?=\d)/i, "").slice(0, 40) || null;
    }
    const existing = found.get(key);
    if (existing) {
      if (!existing.version && clean) existing.version = clean;
      return;
    }
    found.set(key, { name, category, version: clean, url: url || null });
  };
  const check = (fn) => { try { fn(); } catch {} };
  const w = window;
  const $ = (selector) => { try { return document.querySelector(selector); } catch { return null; } };

  const scripts = Array.from(document.scripts);
  const scriptUrls = scripts.map((script) => script.src).filter(Boolean);
  const styleUrls = Array.from(document.querySelectorAll("link[href]")).map((link) => link.href);
  let resourceUrls = [];
  check(() => { resourceUrls = performance.getEntriesByType("resource").map((entry) => entry.name); });
  const urls = [...scriptUrls, ...styleUrls, ...resourceUrls];
  const hasUrl = (pattern) => urls.some((url) => pattern.test(url));
  const versionFromUrl = (pattern) => {
    for (const url of urls) {
      const match = url.match(pattern);
      if (match) return match[1];
    }
    return null;
  };
  const generators = Array.from(document.querySelectorAll("meta[name='generator' i]"))
    .map((meta) => meta.content || "")
    .filter(Boolean);
  const generator = (pattern) => {
    for (const value of generators) {
      const match = value.match(pattern);
      if (match) return { version: match[1] || null };
    }
    return null;
  };
  let windowKeys = [];
  check(() => { windowKeys = Object.getOwnPropertyNames(w); });
  const hasKey = (prefix) => windowKeys.some((key) => key.startsWith(prefix));

  const sample = Array.from(document.querySelectorAll("body, body *")).slice(0, 2000);
  const classes = new Set();
  let vueScoped = false;
  for (const element of sample) {
    check(() => {
      element.classList.forEach((name) => { if (classes.size < 5000) classes.add(name); });
    });
    if (!vueScoped) {
      for (const attribute of element.attributes) {
        if (attribute.name.startsWith("data-v-")) { vueScoped = true; break; }
      }
    }
  }
  const findElement = (test) => {
    for (const element of sample) {
      try { if (test(element)) return element; } catch {}
    }
    return null;
  };

  // Front-end frameworks.
  check(() => {
    const hook = w.__REACT_DEVTOOLS_GLOBAL_HOOK__;
    let version = w.React && w.React.version;
    if (!version && hook && hook.renderers && hook.renderers.size) {
      for (const renderer of hook.renderers.values()) {
        if (renderer && renderer.version) { version = renderer.version; break; }
      }
    }
    const root = $("[data-reactroot]") || findElement((element) =>
      element._reactRootContainer ||
      Object.keys(element).some((key) => key.startsWith("__reactFiber$") || key.startsWith("__reactContainer$") || key.startsWith("__reactInternalInstance$")));
    if (root || version) add("React", "framework", version, "https://react.dev");
  });
  check(() => {
    const root = findElement((element) => element.__k && typeof element.__k === "object" && "__v" in element.__k);
    if (w.preact || root) add("Preact", "framework", w.preact && w.preact.version, "https://preactjs.com");
  });
  check(() => {
    const app = findElement((element) => element.__vue_app__);
    const legacy = findElement((element) => element.__vue__);
    let version = (app && app.__vue_app__.version) || (w.Vue && w.Vue.version);
    if (!version && legacy) {
      const root = legacy.__vue__.$root;
      version = root && root.constructor && root.constructor.version;
    }
    if (app || legacy || w.Vue || w.__VUE__ || vueScoped || $("[data-v-app]")) add("Vue.js", "framework", version, "https://vuejs.org");
  });
  check(() => {
    if (w.__NUXT__ || w.$nuxt || w.useNuxtApp || $("#__nuxt") || hasUrl(/\/_nuxt\//)) {
      add("Nuxt", "framework", w.$nuxt && w.$nuxt.$nuxt && w.$nuxt.$nuxt.version, "https://nuxt.com");
      add("Vue.js", "framework", null, "https://vuejs.org");
      add("Node.js", "language", null, "https://nodejs.org");
    }
  });
  check(() => {
    if (w.__NEXT_DATA__ || w.next || w.__next_f || $("#__next") || hasUrl(/\/_next\/static\//)) {
      add("Next.js", "framework", w.next && w.next.version, "https://nextjs.org");
      add("React", "framework", null, "https://react.dev");
      add("Node.js", "language", null, "https://nodejs.org");
    }
  });
  check(() => {
    if (w.__remixContext || w.__remixManifest) {
      add("Remix", "framework", null, "https://remix.run");
      add("React", "framework", null, "https://react.dev");
    }
    if (w.__reactRouterContext || w.__reactRouterVersion) {
      add("React Router", "framework", w.__reactRouterVersion, "https://reactrouter.com");
    }
  });
  check(() => {
    if (w.___gatsby || w.___loader || $("#___gatsby")) {
      add("Gatsby", "framework", null, "https://www.gatsbyjs.com");
      add("React", "framework", null, "https://react.dev");
    }
  });
  check(() => {
    const angular = $("[ng-version]");
    if (angular) add("Angular", "framework", angular.getAttribute("ng-version"), "https://angular.dev");
    if (w.angular && w.angular.version) add("AngularJS", "framework", w.angular.version.full, "https://angularjs.org");
    else if ($("[ng-app], [data-ng-app], .ng-scope")) add("AngularJS", "framework", null, "https://angularjs.org");
  });
  check(() => {
    const svelteClass = Array.from(classes).some((name) => /^svelte-[a-z0-9]{4,}$/.test(name));
    const svelte5 = w.__svelte && w.__svelte.v;
    if (svelteClass || svelte5) {
      add("Svelte", "framework", svelte5 && svelte5.size ? Array.from(svelte5)[0] : null, "https://svelte.dev");
    }
    if (hasKey("__sveltekit") || $("[data-sveltekit-preload-data], [data-sveltekit-hydrate]") || hasUrl(/\/_app\/immutable\//)) {
      add("SvelteKit", "framework", null, "https://svelte.dev/docs/kit");
      add("Svelte", "framework", null, "https://svelte.dev");
    }
  });
  check(() => {
    if (w._$HY || w.Solid$$) add("SolidJS", "framework", null, "https://www.solidjs.com");
  });
  check(() => {
    const qwik = $("[q\\:container]");
    if (qwik || w.qwikevents) add("Qwik", "framework", qwik && qwik.getAttribute("q:version"), "https://qwik.dev");
  });
  check(() => {
    const astro = generator(/Astro\s*v?([\d.]+)?/i);
    if (astro || $("astro-island, astro-slot")) add("Astro", "framework", astro && astro.version, "https://astro.build");
  });
  check(() => {
    if (w.Ember) add("Ember.js", "framework", w.Ember.VERSION, "https://emberjs.com");
    if (w.Backbone) add("Backbone.js", "framework", w.Backbone.VERSION, "https://backbonejs.org");
    if (w.Alpine || $("[x-data]")) add("Alpine.js", "framework", w.Alpine && w.Alpine.version, "https://alpinejs.dev");
    if (w.htmx || $("[hx-get], [hx-post], [data-hx-get], [data-hx-post]")) add("htmx", "framework", w.htmx && w.htmx.version, "https://htmx.org");
    if (w.Stimulus || $("[data-controller]") && w.Turbo) add("Hotwire", "framework", null, "https://hotwired.dev");
    if (w.Livewire) add("Livewire", "framework", null, "https://livewire.laravel.com");
    if (w.Polymer) add("Polymer", "framework", w.Polymer.version, "https://polymer-library.polymer-project.org");
    if (w.litElementVersions || w.litHtmlVersions) {
      const versions = w.litElementVersions || w.litHtmlVersions;
      add("Lit", "framework", versions && versions[0], "https://lit.dev");
    }
  });

  // JavaScript libraries.
  check(() => {
    if (w.jQuery && w.jQuery.fn) add("jQuery", "library", w.jQuery.fn.jquery, "https://jquery.com");
    if (w.jQuery && w.jQuery.ui) add("jQuery UI", "library", w.jQuery.ui.version, "https://jqueryui.com");
  });
  check(() => {
    if (w._ && w._.VERSION) {
      if (typeof w._.chunk === "function" && typeof w._.flattenDeep === "function") add("Lodash", "library", w._.VERSION, "https://lodash.com");
      else add("Underscore.js", "library", w._.VERSION, "https://underscorejs.org");
    }
  });
  check(() => {
    if (w.moment && w.moment.version) add("Moment.js", "library", w.moment.version, "https://momentjs.com");
    if (w.dayjs) add("Day.js", "library", null, "https://day.js.org");
    if (w.d3 && w.d3.version) add("D3", "library", w.d3.version, "https://d3js.org");
    if (w.THREE || w.__THREE__) add("Three.js", "library", (w.THREE && w.THREE.REVISION) || w.__THREE__, "https://threejs.org");
    if (w.gsap) add("GSAP", "library", w.gsap.version, "https://gsap.com");
    if (w.Chart && w.Chart.version) add("Chart.js", "library", w.Chart.version, "https://www.chartjs.org");
    if (w.axios) add("Axios", "library", w.axios.VERSION, "https://axios-http.com");
    if (w.Swiper) add("Swiper", "library", null, "https://swiperjs.com");
    if (w.L && w.L.version && w.L.map) add("Leaflet", "library", w.L.version, "https://leafletjs.com");
    if (w.mapboxgl) add("Mapbox GL JS", "library", w.mapboxgl.version, "https://www.mapbox.com");
    if (w.google && w.google.maps) add("Google Maps", "library", w.google.maps.version, "https://developers.google.com/maps");
    if (w.Hammer) add("Hammer.js", "library", w.Hammer.VERSION, "https://hammerjs.github.io");
    if (w.Modernizr) add("Modernizr", "library", w.Modernizr._version, "https://modernizr.com");
    if (w.Prism) add("Prism", "library", null, "https://prismjs.com");
    if (w.hljs) add("highlight.js", "library", w.hljs.versionString, "https://highlightjs.org");
    if (w.MathJax) add("MathJax", "library", w.MathJax.version, "https://www.mathjax.org");
    if (w.katex) add("KaTeX", "library", w.katex.version, "https://katex.org");
    if (w.Zone) add("Zone.js", "library", null, "https://github.com/angular/angular/tree/main/packages/zone.js");
    if (w.core && w.core.version) add("core-js", "library", w.core.version, "https://github.com/zloirock/core-js");
    if (w.Lottie || w.lottie || w.bodymovin) add("Lottie", "library", null, "https://airbnb.io/lottie");
    if (w.workbox || hasUrl(/workbox-/)) add("Workbox", "library", null, "https://developer.chrome.com/docs/workbox");
  });

  // UI kits and CSS.
  check(() => {
    let tailwind = false;
    const root = getComputedStyle(document.documentElement);
    const body = document.body ? getComputedStyle(document.body) : root;
    for (const name of ["--tw-border-spacing-x", "--tw-ring-offset-width", "--tw-translate-x", "--tw-shadow"]) {
      if (root.getPropertyValue(name) !== "" || body.getPropertyValue(name) !== "") { tailwind = true; break; }
    }
    if (!tailwind) {
      const utility = /^(?:(?:sm|md|lg|xl|2xl|hover|focus|dark|group-hover|active|disabled):)*(?:flex|grid|hidden|block|inline-flex|items-center|justify-between|justify-center|mx-auto|(?:p|px|py|pt|pb|m|mx|my|mt|mb|gap|space-[xy])-\d+(?:\.5)?|(?:text|bg|border)-(?:slate|gray|zinc|neutral|stone|red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose)-\d{2,3}|rounded(?:-(?:sm|md|lg|xl|2xl|full))?|text-(?:xs|sm|base|lg|xl|2xl|3xl)|font-(?:medium|semibold|bold)|w-full|h-full)$/;
      let hits = 0;
      let variant = false;
      for (const name of classes) {
        if (utility.test(name)) {
          hits += 1;
          if (name.includes(":")) variant = true;
        }
      }
      tailwind = hits >= 12 && variant;
    }
    if (tailwind) add("Tailwind CSS", "ui", null, "https://tailwindcss.com");
  });
  check(() => {
    const fromUrl = versionFromUrl(/bootstrap@?\/?(\d+\.\d+\.\d+)/i);
    const fromJs = (w.bootstrap && w.bootstrap.Tooltip && w.bootstrap.Tooltip.VERSION) ||
      (w.jQuery && w.jQuery.fn && w.jQuery.fn.tooltip && w.jQuery.fn.tooltip.Constructor && w.jQuery.fn.tooltip.Constructor.VERSION);
    const markup = $(".btn.btn-primary, .navbar-expand, .navbar-expand-sm, .navbar-expand-md, .navbar-expand-lg") && $(".row > [class*='col-']");
    if (fromUrl || fromJs || hasUrl(/bootstrap(\.bundle)?(\.min)?\.(css|js)/i) || markup) {
      add("Bootstrap", "ui", fromJs || fromUrl, "https://getbootstrap.com");
    }
  });
  check(() => {
    if (hasUrl(/bulma/i) || ($(".columns > .column") && $(".button.is-primary, .navbar-burger, .hero-body"))) add("Bulma", "ui", versionFromUrl(/bulma@(\d+\.\d+\.\d+)/i), "https://bulma.io");
    if (w.Foundation || hasUrl(/foundation(\.min)?\.(css|js)/i)) add("Foundation", "ui", w.Foundation && w.Foundation.version, "https://get.foundation");
    if ($("[class^='Mui'], [class*=' Mui']")) add("Material UI", "ui", null, "https://mui.com");
    if ($("[class^='chakra-'], [class*=' chakra-']")) add("Chakra UI", "ui", null, "https://chakra-ui.com");
    if ($("[class^='ant-'], [class*=' ant-']")) add("Ant Design", "ui", null, "https://ant.design");
    if ($("[class^='mantine-'], [class*=' mantine-']")) add("Mantine", "ui", null, "https://mantine.dev");
    if ($("[data-radix-popper-content-wrapper], [data-radix-collection-item], [data-radix-scroll-area-viewport]")) add("Radix UI", "ui", null, "https://www.radix-ui.com");
    if ($("[class^='v-application'], .v-application")) add("Vuetify", "ui", null, "https://vuetifyjs.com");
    if ($("[class*='mat-mdc-'], [class^='mat-']")) add("Angular Material", "ui", null, "https://material.angular.dev");
    if (w.UIkit) add("UIkit", "ui", w.UIkit.version, "https://getuikit.com");
    if (w.M && w.M.version && hasUrl(/materialize/i)) add("Materialize", "ui", w.M.version, "https://materializecss.com");
    if (hasUrl(/font-?awesome/i) || $(".fa-solid, .fa-regular, .fa-brands, .fas, .fab, .far, svg.svg-inline--fa")) add("Font Awesome", "ui", versionFromUrl(/font-?awesome[@/-](\d+\.\d+\.\d+)/i), "https://fontawesome.com");
    if (hasUrl(/fonts\.(googleapis|gstatic)\.com/)) add("Google Fonts", "ui", null, "https://fonts.google.com");
    if (hasUrl(/use\.typekit\.net|p\.typekit\.net/)) add("Adobe Fonts", "ui", null, "https://fonts.adobe.com");
    if (hasUrl(/bootstrap-icons/i)) add("Bootstrap Icons", "ui", null, "https://icons.getbootstrap.com");
    if ($("[class*='material-icons'], [class*='material-symbols']")) add("Material Icons", "ui", null, "https://fonts.google.com/icons");
  });

  // CMS, site builders and stores.
  check(() => {
    const php = () => add("PHP", "language", null, "https://www.php.net");
    const wordpress = generator(/WordPress\s*([\d.]+)?/i);
    if (wordpress || hasUrl(/\/wp-(content|includes)\//) || w.wp) {
      add("WordPress", "cms", wordpress && wordpress.version, "https://wordpress.org");
      php();
    }
    if (w.wc_add_to_cart_params || w.woocommerce_params || generator(/WooCommerce/i)) {
      add("WooCommerce", "cms", (generator(/WooCommerce\s*([\d.]+)/i) || {}).version, "https://woocommerce.com");
      php();
    }
    const drupal = generator(/Drupal\s*(\d+)?/i);
    if (drupal || w.Drupal || hasUrl(/\/sites\/(default|all)\/(files|modules|themes)\//)) {
      add("Drupal", "cms", drupal && drupal.version, "https://www.drupal.org");
      php();
    }
    const joomla = generator(/Joomla!?\s*([\d.]+)?/i);
    if (joomla || w.Joomla) {
      add("Joomla", "cms", joomla && joomla.version, "https://www.joomla.org");
      php();
    }
    if (w.mw && w.mw.config && w.mw.config.get) {
      add("MediaWiki", "cms", w.mw.config.get("wgVersion"), "https://www.mediawiki.org");
      php();
    }
    if (w.Mage || hasUrl(/\/static\/version\d+\/frontend\//)) {
      add("Magento", "cms", null, "https://business.adobe.com/products/magento/magento-commerce.html");
      php();
    }
    const typo3 = generator(/TYPO3\s*(?:CMS)?\s*([\d.]+)?/i);
    if (typo3) { add("TYPO3", "cms", typo3.version, "https://typo3.org"); php(); }
    if (w.Shopify || hasUrl(/cdn\.shopify\.com/)) add("Shopify", "platform", null, "https://www.shopify.com");
    if (w.wixBiSession || hasUrl(/static\.(wixstatic|parastorage)\.com/) || generator(/Wix\.com/i)) add("Wix", "platform", null, "https://www.wix.com");
    if ((w.Static && w.Static.SQUARESPACE_CONTEXT) || hasUrl(/squarespace(-cdn)?\.com/) || generator(/Squarespace/i)) add("Squarespace", "platform", null, "https://www.squarespace.com");
    if (w.Webflow || document.documentElement.hasAttribute("data-wf-page") || generator(/Webflow/i)) add("Webflow", "platform", null, "https://webflow.com");
    const ghost = generator(/Ghost\s*([\d.]+)?/i);
    if (ghost) add("Ghost", "platform", ghost.version, "https://ghost.org");
    const hugo = generator(/Hugo\s*([\d.]+)?/i);
    if (hugo) add("Hugo", "cms", hugo.version, "https://gohugo.io");
    const jekyll = generator(/Jekyll\s*v?([\d.]+)?/i);
    if (jekyll) add("Jekyll", "cms", jekyll.version, "https://jekyllrb.com");
    const eleventy = generator(/Eleventy\s*v?([\d.]+)?/i);
    if (eleventy) add("Eleventy", "cms", eleventy.version, "https://www.11ty.dev");
    const hexo = generator(/Hexo\s*([\d.]+)?/i);
    if (hexo) add("Hexo", "cms", hexo.version, "https://hexo.io");
    const docusaurus = generator(/Docusaurus\s*v?([\d.]+)?/i);
    if (docusaurus || w.docusaurus) {
      add("Docusaurus", "cms", docusaurus && docusaurus.version, "https://docusaurus.io");
      add("React", "framework", null, "https://react.dev");
    }
    const vitepress = generator(/VitePress\s*v?([\d.]+)?/i);
    if (vitepress || w.__VP_HASH_MAP__) add("VitePress", "cms", vitepress && vitepress.version, "https://vitepress.dev");
    const mkdocs = generator(/mkdocs-?([\d.]+)?/i);
    if (mkdocs) add("MkDocs", "cms", mkdocs.version, "https://www.mkdocs.org");
    if (generator(/Sphinx/i) || w.DOCUMENTATION_OPTIONS) add("Sphinx", "cms", (generator(/Sphinx\s*([\d.]+)/i) || {}).version, "https://www.sphinx-doc.org");
    if (generator(/Blogger/i)) add("Blogger", "cms", null, "https://www.blogger.com");
    if (w.Discourse) add("Discourse", "cms", null, "https://www.discourse.org");
    if (w.HubSpotConversations || hasUrl(/js\.hs-scripts\.com|js\.hsforms\.net/)) add("HubSpot", "analytics", null, "https://www.hubspot.com");
  });

  // White-label and hosted apps. The name is the vendor's product, not the brand on the page.
  check(() => {
    const host = location.hostname.replace(/\.$/, "").replace(/^www\./, "").toLowerCase();
    const hostIs = (domain) => host === domain || host.endsWith("." + domain);
    const vendor = (domain) => {
      if (hostIs(domain)) return true;
      const escaped = domain.replace(/\./g, "\\.");
      return hasUrl(new RegExp("(^|[./])" + escaped + "(?::\\d+)?(?:[/?#]|$)", "i"));
    };
    const gen = (pattern) => !!generator(pattern);
    const badge = (pattern) => {
      try {
        return Array.from(document.links).slice(0, 500).some((link) => {
          const href = link.href || "";
          if (!pattern.test(href)) return false;
          const image = link.querySelector("img");
          const text = (link.textContent || "") + " " + (link.getAttribute("aria-label") || "") + " " + (image ? image.alt : "");
          return pattern.test(text) || /powered|made with|built with|create a free/i.test(text);
        });
      } catch { return false; }
    };
    let cookies = "";
    try { cookies = document.cookie || ""; } catch {}
    const foundPlatform = (fn, name, url) => {
      try { if (fn()) add(name, "platform", null, url); } catch {}
    };
    const domains = [
      ["framerusercontent.com", "Framer", "https://www.framer.com"],
      ["cdn-website.com", "Duda", "https://www.duda.co"],
      ["multiscreensite.com", "Duda", "https://www.duda.co"],
      ["duda.co", "Duda", "https://www.duda.co"],
      ["dudaone.com", "Duda", "https://www.duda.co"],
      ["clickfunnels.com", "ClickFunnels", "https://www.clickfunnels.com"],
      ["myclickfunnels.com", "ClickFunnels", "https://www.clickfunnels.com"],
      ["kajabi-cdn.com", "Kajabi", "https://kajabi.com"],
      ["kajabi.com", "Kajabi", "https://kajabi.com"],
      ["kartra.com", "Kartra", "https://kartra.com"],
      ["systeme.io", "systeme.io", "https://systeme.io"],
      ["d1yei2z3i6k35z.cloudfront.net", "systeme.io", "https://systeme.io"],
      ["builderall.com", "Builderall", "https://www.builderall.com"],
      ["carrd.co", "Carrd", "https://carrd.co"],
      ["unbounce.com", "Unbounce", "https://unbounce.com"],
      ["ubembed.com", "Unbounce", "https://unbounce.com"],
      ["unbouncepages.com", "Unbounce", "https://unbounce.com"],
      ["pageserve.co", "Instapage", "https://instapage.com"],
      ["instapage.com", "Instapage", "https://instapage.com"],
      ["leadpages.net", "Leadpages", "https://www.leadpages.com"],
      ["lpages.co", "Leadpages", "https://www.leadpages.com"],
      ["tildacdn.com", "Tilda", "https://tilda.cc"],
      ["tilda.ws", "Tilda", "https://tilda.cc"],
      ["tilda.cc", "Tilda", "https://tilda.cc"],
      ["bubble.io", "Bubble", "https://bubble.io"],
      ["softr.io", "Softr", "https://www.softr.io"],
      ["softrfiles.com", "Softr", "https://www.softr.io"],
      ["jimcdn.com", "Jimdo", "https://www.jimdo.com"],
      ["jimdosite.com", "Jimdo", "https://www.jimdo.com"],
      ["webnode.com", "Webnode", "https://www.webnode.com"],
      ["webnode.page", "Webnode", "https://www.webnode.com"],
      ["strikinglycdn.com", "Strikingly", "https://www.strikingly.com"],
      ["strikingly.com", "Strikingly", "https://www.strikingly.com"],
      ["simvoly.com", "Simvoly", "https://simvoly.com"],
      ["showit.co", "Showit", "https://www.showit.com"],
      ["popmenu.com", "Popmenu", "https://www.popmenu.com"],
      ["popmenucloud.com", "Popmenu", "https://www.popmenu.com"],
      ["getbento.com", "BentoBox", "https://www.getbento.com"],
      ["pixieset.com", "Pixieset", "https://pixieset.com"],
      ["bigcommerce.com", "BigCommerce", "https://www.bigcommerce.com"],
      ["ecwid.com", "Ecwid", "https://www.ecwid.com"],
      ["shift4shop.com", "Shift4Shop", "https://www.shift4shop.com"],
      ["3dcart.com", "Shift4Shop", "https://www.shift4shop.com"],
      ["3dcartstores.com", "Shift4Shop", "https://www.shift4shop.com"],
      ["webshopapp.com", "Lightspeed", "https://www.lightspeedhq.com"],
      ["shoplightspeed.com", "Lightspeed", "https://www.lightspeedhq.com"],
      ["volusion.com", "Volusion", "https://www.volusion.com"],
      ["bigcartel.com", "Big Cartel", "https://www.bigcartel.com"],
      ["swell.is", "Swell", "https://www.swell.is"],
      ["swell.store", "Swell", "https://www.swell.is"],
      ["vtexassets.com", "VTEX", "https://vtex.com"],
      ["vteximg.com.br", "VTEX", "https://vtex.com"],
      ["vtexcommercestable.com.br", "VTEX", "https://vtex.com"],
      ["tcdn.com.br", "Tray", "https://www.tray.com.br"],
      ["tray.com.br", "Tray", "https://www.tray.com.br"],
      ["lojaintegrada.com.br", "Loja Integrada", "https://lojaintegrada.com.br"],
      ["teachablecdn.com", "Teachable", "https://teachable.com"],
      ["teachable.com", "Teachable", "https://teachable.com"],
      ["thinkific.com", "Thinkific", "https://www.thinkific.com"],
      ["podia.com", "Podia", "https://www.podia.com"],
      ["learnworlds.com", "LearnWorlds", "https://www.learnworlds.com"],
      ["gumroad.com", "Gumroad", "https://gumroad.com"],
      ["readymag.com", "Readymag", "https://readymag.com"],
      ["cargo.site", "Cargo", "https://cargo.site"],
      ["format.com", "Format", "https://www.format.com"],
      ["smugmug.com", "SmugMug", "https://www.smugmug.com"],
      ["myportfolio.com", "Adobe Portfolio", "https://portfolio.adobe.com"],
      ["carbonmade.com", "Carbonmade", "https://carbonmade.com"],
      ["zenfolio.com", "Zenfolio", "https://zenfolio.com"],
      ["photoshelter.com", "PhotoShelter", "https://www.photoshelter.com"],
      ["pixpa.com", "Pixpa", "https://www.pixpa.com"],
      ["mozello.com", "Mozello", "https://www.mozello.com"],
      ["voog.com", "Voog", "https://www.voog.com"],
      ["dorik.io", "Dorik", "https://dorik.com"],
      ["ucraft.com", "uCraft", "https://www.ucraft.com"],
      ["ucraft.net", "uCraft", "https://www.ucraft.com"],
      ["durable.co", "Durable", "https://durable.co"],
      ["typedream.com", "Typedream", "https://typedream.com"],
      ["popsy.co", "Popsy", "https://popsy.co"],
      ["circle.so", "Circle", "https://circle.so"],
      ["mightynetworks.com", "Mighty Networks", "https://www.mightynetworks.com"],
      ["skool.com", "Skool", "https://www.skool.com"],
      ["glideapps.com", "Glide", "https://www.glideapps.com"],
      ["adalo.com", "Adalo", "https://www.adalo.com"],
      ["appsmith.com", "Appsmith", "https://www.appsmith.com"],
      ["budibase.com", "Budibase", "https://budibase.com"],
      ["budibase.app", "Budibase", "https://budibase.com"],
      ["retool.com", "Retool", "https://retool.com"],
      ["retool-edge.com", "Retool", "https://retool.com"],
      ["mendix.com", "Mendix", "https://www.mendix.com"],
      ["mendixcloud.com", "Mendix", "https://www.mendix.com"],
      ["outsystems.com", "OutSystems", "https://www.outsystems.com"],
      ["outsystemsenterprise.com", "OutSystems", "https://www.outsystems.com"],
      ["caspio.com", "Caspio", "https://www.caspio.com"],
      ["builder.io", "Builder.io", "https://www.builder.io"],
      ["plasmic.app", "Plasmic", "https://www.plasmic.app"],
      ["brizy.io", "Brizy", "https://www.brizy.io"],
      ["brizy.cloud", "Brizy", "https://www.brizy.io"],
      ["weblium.com", "Weblium", "https://weblium.com"],
      ["convertri.com", "Convertri", "https://www.convertri.com"],
      ["samcart.com", "SamCart", "https://samcart.com"],
      ["landingi.com", "Landingi", "https://landingi.com"],
      ["swipepages.com", "Swipe Pages", "https://swipepages.com"],
      ["heyflow.com", "Heyflow", "https://heyflow.com"],
      ["heyflow.app", "Heyflow", "https://heyflow.com"],
      ["groovepages.com", "GroovePages", "https://groove.cm"],
      ["yolasite.com", "Yola", "https://www.yola.com"],
      ["yolacdn.net", "Yola", "https://www.yola.com"],
      ["hashnode.com", "Hashnode", "https://hashnode.com"],
      ["scorpion.co", "Scorpion", "https://www.scorpion.co"],
      ["officite.com", "Officite", "https://www.officite.com"],
      ["patientpop.com", "Tebra", "https://www.tebra.com"],
      ["tebra.com", "Tebra", "https://www.tebra.com"],
      ["lawlytics.com", "LawLytics", "https://www.lawlytics.com"],
      ["justatic.com", "Justia", "https://www.justia.com"],
      ["findlaw.com", "FindLaw", "https://www.findlaw.com"],
      ["fl-cdn.com", "FindLaw", "https://www.findlaw.com"],
      ["placester.com", "Placester", "https://placester.com"],
      ["placester.net", "Placester", "https://placester.com"],
      ["luxurypresence.com", "Luxury Presence", "https://www.luxurypresence.com"],
      ["boomtownroi.com", "BoomTown", "https://www.boomtownroi.com"],
      ["kvcore.com", "kvCORE", "https://www.kvcore.com"],
      ["lofty.com", "Lofty", "https://lofty.com"],
      ["chime.me", "Lofty", "https://lofty.com"],
      ["sierrainteractive.com", "Sierra Interactive", "https://www.sierrainteractive.com"],
      ["realgeeks.com", "Real Geeks", "https://www.realgeeks.com"],
      ["dealer.com", "Dealer.com", "https://www.dealer.com"],
      ["dealeron.com", "DealerOn", "https://www.dealeron.com"],
      ["dealerinspire.com", "Dealer Inspire", "https://www.dealerinspire.com"],
      ["dealerfire.com", "DealerFire", "https://www.dealerfire.com"],
      ["payhip.com", "Payhip", "https://payhip.com"],
      ["sellfy.com", "Sellfy", "https://sellfy.com"],
      ["perspective.co", "Perspective", "https://www.perspective.co"],
      ["super.so", "Super", "https://super.so"],
      ["zyrosite.com", "Hostinger Website Builder", "https://www.hostinger.com/website-builder"],
      ["mywebsite-editor.com", "IONOS MyWebsite", "https://www.ionos.com/websites/website-builder"],
      ["ionos.space", "IONOS MyWebsite", "https://www.ionos.com/websites/website-builder"],
      ["godaddysites.com", "GoDaddy Website Builder", "https://www.godaddy.com/websites/website-builder"],
    ];
    for (const [domain, name, url] of domains) foundPlatform(() => vendor(domain), name, url);

    foundPlatform(() => gen(/Framer/i) || !!$("[data-framer-name], [data-framer-page], [data-framer-component-type]") || (document.documentElement && (document.documentElement.hasAttribute("data-framer-html") || document.documentElement.hasAttribute("data-framer-theme"))) || badge(/\bframer\.com\b/i), "Framer", "https://www.framer.com");
    foundPlatform(() => !!$("#dmRoot, .dmBody, #d-js-dmapi") || !!(w.dmAPI || w.DM) || gen(/Duda/i), "Duda", "https://www.duda.co");
    foundPlatform(() => hostIs("highlevel.com") || hostIs("gohighlevel.com") || vendor("msgsndr.com") || vendor("filesafe.space") || vendor("apisystem.tech") || hasUrl(/(^|[./])(?:backend|stcdn|api|images|cdn|link)\.leadconnectorhq\.com(?::\d+)?(?:[/?#]|$)/i) || gen(/HighLevel|GoHighLevel|LeadConnector/i), "HighLevel", "https://www.highlevel.com");
    foundPlatform(() => !!w.ClickFunnels || gen(/ClickFunnels/i), "ClickFunnels", "https://www.clickfunnels.com");
    foundPlatform(() => hasUrl(/kajabi-storefronts/i) || !!$("[data-kajabi-app]") || gen(/Kajabi/i), "Kajabi", "https://kajabi.com");
    foundPlatform(() => gen(/Kartra/i), "Kartra", "https://kartra.com");
    foundPlatform(() => gen(/systeme\.io/i), "systeme.io", "https://systeme.io");
    foundPlatform(() => gen(/Builderall/i), "Builderall", "https://www.builderall.com");
    foundPlatform(() => gen(/Carrd/i) || badge(/carrd\.co/i), "Carrd", "https://carrd.co");
    foundPlatform(() => !!$("#lp-pom-root") || gen(/Unbounce/i), "Unbounce", "https://unbounce.com");
    foundPlatform(() => gen(/Instapage/i), "Instapage", "https://instapage.com");
    foundPlatform(() => gen(/Leadpages/i), "Leadpages", "https://www.leadpages.com");
    foundPlatform(() => !!$(".t-records") || gen(/Tilda/i), "Tilda", "https://tilda.cc");
    foundPlatform(() => !!w.appquery || gen(/^Bubble/i), "Bubble", "https://bubble.io");
    foundPlatform(() => gen(/Softr/i), "Softr", "https://www.softr.io");
    foundPlatform(() => gen(/Jimdo/i), "Jimdo", "https://www.jimdo.com");
    foundPlatform(() => gen(/Webnode/i), "Webnode", "https://www.webnode.com");
    foundPlatform(() => gen(/Strikingly/i), "Strikingly", "https://www.strikingly.com");
    foundPlatform(() => hasUrl(/s123-cdn/i) || hostIs("site123.com") || hostIs("site123.me") || gen(/Site123/i), "Site123", "https://www.site123.com");
    foundPlatform(() => !!w.showit || gen(/Showit/i), "Showit", "https://www.showit.com");
    foundPlatform(() => gen(/Popmenu/i), "Popmenu", "https://www.popmenu.com");
    foundPlatform(() => gen(/BentoBox/i), "BentoBox", "https://www.getbento.com");
    foundPlatform(() => !!w.BCData || gen(/BigCommerce/i), "BigCommerce", "https://www.bigcommerce.com");
    foundPlatform(() => !!w.Ecwid || !!$("#ecwid_html, #ecwid_body"), "Ecwid", "https://www.ecwid.com");
    foundPlatform(() => !!w.vtexjs, "VTEX", "https://vtex.com");
    foundPlatform(() => hasUrl(/\/vspfiles\//i), "Volusion", "https://www.volusion.com");
    foundPlatform(() => gen(/Teachable/i), "Teachable", "https://teachable.com");
    foundPlatform(() => gen(/Thinkific/i), "Thinkific", "https://www.thinkific.com");
    foundPlatform(() => gen(/Podia/i), "Podia", "https://www.podia.com");
    foundPlatform(() => gen(/LearnWorlds/i), "LearnWorlds", "https://www.learnworlds.com");
    foundPlatform(() => gen(/Simvoly/i), "Simvoly", "https://simvoly.com");
    foundPlatform(() => vendor("tiendanube.com") || vendor("mitiendanube.com"), "Tiendanube", "https://www.tiendanube.com");
    foundPlatform(() => (vendor("nuvemshop.com.br") || vendor("nuvemshop.com")) && !(vendor("tiendanube.com") || vendor("mitiendanube.com")), "Nuvemshop", "https://www.nuvemshop.com.br");
    foundPlatform(() => hasUrl(/demandware\.static|demandware\.net|\/on\/demandware\./i) || /(?:^|;\s*)dwsid=/.test(cookies), "Salesforce Commerce Cloud", "https://www.salesforce.com/products/commerce/");
    foundPlatform(() => hostIs("my.site.com") || hostIs("salesforce-sites.com") || hasUrl(/\/sfsites\/c\//i), "Salesforce Experience Cloud", "https://www.salesforce.com/products/experience-cloud/overview/");
    foundPlatform(() => !hostIs("my.site.com") && !hostIs("salesforce-sites.com") && !hasUrl(/\/sfsites\/c\//i) && (hostIs("force.com") || hostIs("salesforce.com") || hostIs("visualforce.com")), "Salesforce", "https://www.salesforce.com");
    foundPlatform(() => !!w._spPageContextInfo || hostIs("sharepoint.com"), "SharePoint", "https://www.microsoft.com/microsoft-365/sharepoint/collaboration");
    foundPlatform(() => hostIs("powerappsportals.com") || hostIs("powerpages.microsoft.com") || vendor("powerappsportals.com") || gen(/Power Pages/i), "Power Pages", "https://www.microsoft.com/power-platform/products/power-pages");
    foundPlatform(() => !!$("[data-hs-cos-type]") || /\.hs-sites/.test(host) || host.includes("hubspotpagebuilder"), "HubSpot CMS", "https://www.hubspot.com/products/cms");
    foundPlatform(() => gen(/Substack/i) || hostIs("substack.com") || (vendor("substackcdn.com") && !!w._preloads), "Substack", "https://substack.com");
    foundPlatform(() => gen(/beehiiv/i) || hostIs("beehiiv.com") || hasUrl(/(^|[./])beehiiv\.com\/[^?#]*\.(?:js|css)(?:[?#]|$)/i), "Beehiiv", "https://www.beehiiv.com");
    foundPlatform(() => hostIs("notion.site") || hostIs("notion.so"), "Notion", "https://www.notion.so");
    foundPlatform(() => hostIs("janeapp.com"), "Jane", "https://jane.app");
    foundPlatform(() => hostIs("vagaro.com"), "Vagaro", "https://www.vagaro.com");
    foundPlatform(() => hostIs("mindbodyonline.com"), "Mindbody", "https://www.mindbodyonline.com");
    foundPlatform(() => hostIs("ac-page.com") || hostIs("activehosted.com"), "ActiveCampaign", "https://www.activecampaign.com");
    foundPlatform(() => hostIs("infusionsoft.com") || hostIs("keap.com"), "Keap", "https://keap.com");
    foundPlatform(() => hostIs("sites.google.com") || gen(/Google Sites/i), "Google Sites", "https://sites.google.com");
    foundPlatform(() => hostIs("lovable.app") || hostIs("lovableproject.com"), "Lovable", "https://lovable.dev");
    foundPlatform(() => gen(/GoDaddy Website Builder/i) || (!!$("[data-ux]") && vendor("wsimg.com")), "GoDaddy Website Builder", "https://www.godaddy.com/websites/website-builder");
    foundPlatform(() => gen(/Hostinger Website Builder|^Zyro/i) || hostIs("zyro.com"), "Hostinger Website Builder", "https://www.hostinger.com/website-builder");
    foundPlatform(() => gen(/IONOS MyWebsite|1&1 MyWebsite|IONOS Website Builder/i), "IONOS MyWebsite", "https://www.ionos.com/websites/website-builder");
    const squareOnline = gen(/Square Online/i) || hostIs("square.site");
    foundPlatform(() => squareOnline, "Square Online", "https://squareup.com/us/en/online-store");
    foundPlatform(() => !squareOnline && (vendor("editmysite.com") || (!!w._W && !!(w._W.metaSite || w._W.configDomain)) || gen(/Weebly/i)), "Weebly", "https://www.weebly.com");
  });

  // Analytics, tags and monitoring.
  check(() => {
    if (w.gtag || w.ga || w.GoogleAnalyticsObject || hasUrl(/google-analytics\.com|googletagmanager\.com\/gtag\/js/)) add("Google Analytics", "analytics", null, "https://marketingplatform.google.com/about/analytics");
    if (w.google_tag_manager || hasUrl(/googletagmanager\.com\/gtm\.js/)) add("Google Tag Manager", "analytics", null, "https://tagmanager.google.com");
    if (w.plausible || hasUrl(/plausible\.io\/js/)) add("Plausible", "analytics", null, "https://plausible.io");
    if (w.fathom || hasUrl(/cdn\.usefathom\.com/)) add("Fathom", "analytics", null, "https://usefathom.com");
    if (w.umami || hasUrl(/umami/i)) add("Umami", "analytics", null, "https://umami.is");
    if (hasUrl(/cdn\.segment\.com/)) add("Segment", "analytics", null, "https://segment.com");
    if (w.hj || hasUrl(/static\.hotjar\.com/)) add("Hotjar", "analytics", null, "https://www.hotjar.com");
    if (w.clarity || hasUrl(/clarity\.ms/)) add("Microsoft Clarity", "analytics", null, "https://clarity.microsoft.com");
    if (w.fbq || hasUrl(/connect\.facebook\.net/)) add("Meta Pixel", "analytics", null, "https://www.facebook.com/business/tools/meta-pixel");
    if (hasUrl(/static\.cloudflareinsights\.com/)) add("Cloudflare Web Analytics", "analytics", null, "https://www.cloudflare.com/web-analytics");
    if (w.mixpanel) add("Mixpanel", "analytics", null, "https://mixpanel.com");
    if (w.amplitude) add("Amplitude", "analytics", null, "https://amplitude.com");
    if (w.posthog) add("PostHog", "analytics", w.posthog.version, "https://posthog.com");
    if (w.va || hasUrl(/\/_vercel\/insights\//)) add("Vercel Analytics", "analytics", null, "https://vercel.com/analytics");
    if (hasUrl(/\/_vercel\/speed-insights\//)) add("Vercel Speed Insights", "analytics", null, "https://vercel.com/docs/speed-insights");
    if (w._paq || hasUrl(/matomo\.js|piwik\.js/)) add("Matomo", "analytics", null, "https://matomo.org");
    if (w.Sentry || w.__SENTRY__) add("Sentry", "analytics", w.Sentry && w.Sentry.SDK_VERSION, "https://sentry.io");
    if (w.DD_RUM || w.DD_LOGS) add("Datadog", "analytics", null, "https://www.datadoghq.com");
    if (w.newrelic || w.NREUM) add("New Relic", "analytics", null, "https://newrelic.com");
    if (w.LogRocket) add("LogRocket", "analytics", null, "https://logrocket.com");
    if (w.OneTrust || w.Optanon) add("OneTrust", "analytics", null, "https://www.onetrust.com");
    if (w.Cookiebot) add("Cookiebot", "analytics", null, "https://www.cookiebot.com");
  });

  // Payments, captchas and widgets.
  check(() => {
    if (w.Stripe || hasUrl(/js\.stripe\.com/)) add("Stripe", "other", w.Stripe && w.Stripe.version, "https://stripe.com");
    if (w.paypal || hasUrl(/paypal\.com\/sdk\/js|paypalobjects\.com/)) add("PayPal", "other", null, "https://developer.paypal.com");
    if (w.grecaptcha || hasUrl(/google\.com\/recaptcha|recaptcha\.net/)) add("reCAPTCHA", "other", null, "https://developers.google.com/recaptcha");
    if (w.hcaptcha || hasUrl(/hcaptcha\.com/)) add("hCaptcha", "other", null, "https://www.hcaptcha.com");
    if (w.turnstile || hasUrl(/challenges\.cloudflare\.com\/turnstile/)) add("Cloudflare Turnstile", "other", null, "https://www.cloudflare.com/products/turnstile");
    if (w.Intercom) add("Intercom", "other", null, "https://www.intercom.com");
    if (w.zE || w.zEACLoaded) add("Zendesk", "other", null, "https://www.zendesk.com");
    if (w.$crisp) add("Crisp", "other", null, "https://crisp.chat");
    if (w.drift) add("Drift", "other", null, "https://www.salesloft.com/platform/drift");
    if (w.YT || hasUrl(/youtube\.com\/(iframe_api|embed)/)) add("YouTube embed", "other", null, "https://developers.google.com/youtube/iframe_api_reference");
    if (w.Vimeo || hasUrl(/player\.vimeo\.com/)) add("Vimeo embed", "other", null, "https://developer.vimeo.com/player");
    if ("serviceWorker" in navigator && navigator.serviceWorker.controller) add("Service worker", "other", null, "https://developer.mozilla.org/docs/Web/API/Service_Worker_API");
    if ($("link[rel='manifest']")) add("Web app manifest", "other", null, "https://developer.mozilla.org/docs/Web/Manifest");
  });

  // Build tools and bundlers.
  check(() => {
    if (hasUrl(/\/@vite\/client/) || $("style[data-vite-dev-id]") || w.__vite_plugin_react_preamble_installed__) add("Vite", "build", null, "https://vite.dev");
    if (hasKey("webpackChunk") || w.webpackJsonp || w.__webpack_require__) add("webpack", "build", null, "https://webpack.js.org");
    if (w.TURBOPACK || hasKey("TURBOPACK")) add("Turbopack", "build", null, "https://turbo.build/pack");
    if (hasKey("parcelRequire")) add("Parcel", "build", null, "https://parceljs.org");
    if (w.System && w.System.import) add("SystemJS", "build", null, "https://github.com/systemjs/systemjs");
    if (w.requirejs || w.require && w.require.config && w.define && w.define.amd) add("RequireJS", "build", w.requirejs && w.requirejs.version, "https://requirejs.org");
  });

  // Third-party CDNs and hosting.
  check(() => {
    if (hasUrl(/cdn\.jsdelivr\.net/)) add("jsDelivr", "hosting", null, "https://www.jsdelivr.com");
    if (hasUrl(/unpkg\.com/)) add("unpkg", "hosting", null, "https://unpkg.com");
    if (hasUrl(/cdnjs\.cloudflare\.com/)) add("cdnjs", "hosting", null, "https://cdnjs.com");
    if (hasUrl(/ajax\.googleapis\.com/)) add("Google Hosted Libraries", "hosting", null, "https://developers.google.com/speed/libraries");
    const host = location.hostname;
    const suffixes = [
      [".github.io", "GitHub Pages", "https://pages.github.com"],
      [".netlify.app", "Netlify", "https://www.netlify.com"],
      [".vercel.app", "Vercel", "https://vercel.com"],
      [".pages.dev", "Cloudflare Pages", "https://pages.cloudflare.com"],
      [".workers.dev", "Cloudflare Workers", "https://workers.cloudflare.com"],
      [".web.app", "Firebase Hosting", "https://firebase.google.com/products/hosting"],
      [".firebaseapp.com", "Firebase Hosting", "https://firebase.google.com/products/hosting"],
      [".herokuapp.com", "Heroku", "https://www.heroku.com"],
      [".onrender.com", "Render", "https://render.com"],
      [".fly.dev", "Fly.io", "https://fly.io"],
      [".azurewebsites.net", "Azure App Service", "https://azure.microsoft.com/products/app-service"],
      [".gitlab.io", "GitLab Pages", "https://docs.gitlab.com/ee/user/project/pages"],
      [".surge.sh", "Surge", "https://surge.sh"],
      [".up.railway.app", "Railway", "https://railway.com"],
    ];
    for (const [suffix, name, url] of suffixes) {
      if (host.endsWith(suffix)) add(name, "hosting", null, url);
    }
    if (w.firebase) add("Firebase", "other", w.firebase.SDK_VERSION, "https://firebase.google.com");
    if (hasUrl(/\.supabase\.co/)) add("Supabase", "other", null, "https://supabase.com");
  });

  // Server-side hints in the markup and cookies.
  check(() => {
    let cookies = [];
    check(() => { cookies = document.cookie.split(";").map((part) => part.split("=")[0].trim()).filter(Boolean); });
    const cookie = (pattern) => cookies.some((name) => pattern.test(name));
    if (cookie(/^PHPSESSID$/)) add("PHP", "language", null, "https://www.php.net");
    if (cookie(/^JSESSIONID$/)) add("Java", "language", null, "https://www.java.com");
    if (cookie(/^(ASP\.NET_SessionId|\.AspNetCore\.)/) || $("input[name='__VIEWSTATE'], input[name='__RequestVerificationToken']")) add("ASP.NET", "framework", null, "https://dotnet.microsoft.com/apps/aspnet");
    if (cookie(/^laravel_session$/) || w.Laravel) {
      add("Laravel", "framework", null, "https://laravel.com");
      add("PHP", "language", null, "https://www.php.net");
    }
    if (cookie(/^ci_session$/)) {
      add("CodeIgniter", "framework", null, "https://codeigniter.com");
      add("PHP", "language", null, "https://www.php.net");
    }
    if (cookie(/^csrftoken$/) || $("input[name='csrfmiddlewaretoken']")) {
      add("Django", "framework", null, "https://www.djangoproject.com");
      add("Python", "language", null, "https://www.python.org");
    }
    if ($("meta[name='csrf-param'][content='authenticity_token']")) {
      add("Ruby on Rails", "framework", null, "https://rubyonrails.org");
      add("Ruby", "language", null, "https://www.ruby-lang.org");
    }
    if (cookie(/^connect\.sid$/)) {
      add("Express", "framework", null, "https://expressjs.com");
      add("Node.js", "language", null, "https://nodejs.org");
    }
    if (w.__INERTIA__ || $("#app[data-page]")) add("Inertia.js", "framework", null, "https://inertiajs.com");
  });

  // Response headers. Same-origin, so every header but Set-Cookie is readable.
  if (/^https?:$/.test(location.protocol)) {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 4000);
    try {
      const response = await fetch(location.href, {
        method: "HEAD",
        credentials: "same-origin",
        cache: "no-store",
        redirect: "follow",
        signal: controller.signal,
      });
      const header = (name) => (response.headers.get(name) || "").trim();
      const server = header("server");
      const poweredBy = header("x-powered-by");
      const servers = [
        [/nginx\/?([\d.]+)?/i, "nginx", "server", "https://nginx.org"],
        [/openresty\/?([\d.]+)?/i, "OpenResty", "server", "https://openresty.org"],
        [/Apache\/?([\d.]+)?/i, "Apache HTTP Server", "server", "https://httpd.apache.org"],
        [/Microsoft-IIS\/?([\d.]+)?/i, "IIS", "server", "https://www.iis.net"],
        [/LiteSpeed/i, "LiteSpeed", "server", "https://www.litespeedtech.com"],
        [/Caddy/i, "Caddy", "server", "https://caddyserver.com"],
        [/Kestrel/i, "Kestrel", "server", "https://learn.microsoft.com/aspnet/core/fundamentals/servers/kestrel"],
        [/gunicorn\/?([\d.]+)?/i, "Gunicorn", "server", "https://gunicorn.org"],
        [/uvicorn/i, "Uvicorn", "server", "https://www.uvicorn.org"],
        [/Werkzeug\/?([\d.]+)?/i, "Werkzeug", "server", "https://werkzeug.palletsprojects.com"],
        [/Jetty/i, "Jetty", "server", "https://jetty.org"],
        [/Tomcat/i, "Apache Tomcat", "server", "https://tomcat.apache.org"],
        [/envoy/i, "Envoy", "server", "https://www.envoyproxy.io"],
        [/^gws$/i, "Google Web Server", "server", null],
        [/Google Frontend/i, "Google Cloud", "hosting", "https://cloud.google.com"],
        [/cloudflare/i, "Cloudflare", "hosting", "https://www.cloudflare.com"],
        [/^Vercel$/i, "Vercel", "hosting", "https://vercel.com"],
        [/Netlify/i, "Netlify", "hosting", "https://www.netlify.com"],
        [/GitHub\.com/i, "GitHub Pages", "hosting", "https://pages.github.com"],
        [/AmazonS3/i, "Amazon S3", "hosting", "https://aws.amazon.com/s3"],
        [/AkamaiGHost|AkamaiNetStorage/i, "Akamai", "hosting", "https://www.akamai.com"],
        [/Varnish/i, "Varnish", "server", "https://varnish-cache.org"],
        [/Deno\/?([\d.]+)?/i, "Deno", "language", "https://deno.com"],
      ];
      for (const [pattern, name, category, url] of servers) {
        const match = server.match(pattern);
        if (match) add(name, category, match[1], url);
      }
      if (/^gunicorn|^uvicorn|^Werkzeug/i.test(server)) add("Python", "language", null, "https://www.python.org");
      if (/^Kestrel/i.test(server)) add("ASP.NET", "framework", null, "https://dotnet.microsoft.com/apps/aspnet");
      if (/Jetty|Tomcat/i.test(server)) add("Java", "language", null, "https://www.java.com");

      const php = poweredBy.match(/PHP\/?([\d.]+)?/i);
      if (php) add("PHP", "language", php[1], "https://www.php.net");
      if (/ASP\.NET/i.test(poweredBy) || header("x-aspnet-version")) add("ASP.NET", "framework", header("x-aspnet-version") || null, "https://dotnet.microsoft.com/apps/aspnet");
      if (header("x-aspnetmvc-version")) add("ASP.NET MVC", "framework", header("x-aspnetmvc-version"), "https://dotnet.microsoft.com/apps/aspnet/mvc");
      if (/Express/i.test(poweredBy)) {
        add("Express", "framework", null, "https://expressjs.com");
        add("Node.js", "language", null, "https://nodejs.org");
      }
      const nextHeader = poweredBy.match(/Next\.js\s*([\d.]+)?/i);
      if (nextHeader) add("Next.js", "framework", nextHeader[1], "https://nextjs.org");
      if (/Nuxt/i.test(poweredBy)) add("Nuxt", "framework", null, "https://nuxt.com");
      if (/Servlet|JSP/i.test(poweredBy)) add("Java", "language", null, "https://www.java.com");
      if (/Phusion Passenger/i.test(poweredBy)) add("Phusion Passenger", "server", null, "https://www.phusionpassenger.com");
      if (/WP Engine/i.test(poweredBy)) add("WP Engine", "hosting", null, "https://wpengine.com");
      if (/Craft CMS/i.test(poweredBy)) add("Craft CMS", "cms", null, "https://craftcms.com");
      const xGenerator = header("x-generator");
      const drupalHeader = xGenerator.match(/Drupal\s*(\d+)?/i);
      if (drupalHeader || header("x-drupal-cache") || header("x-drupal-dynamic-cache")) {
        add("Drupal", "cms", drupalHeader && drupalHeader[1], "https://www.drupal.org");
        add("PHP", "language", null, "https://www.php.net");
      }
      const hosted = [
        [/Framer/i, "Framer", "https://www.framer.com"],
        [/Webflow/i, "Webflow", "https://webflow.com"],
        [/Wix\.com|Wix Studio/i, "Wix", "https://www.wix.com"],
        [/Squarespace/i, "Squarespace", "https://www.squarespace.com"],
        [/Shopify/i, "Shopify", "https://www.shopify.com"],
        [/Duda/i, "Duda", "https://www.duda.co"],
        [/Ghost/i, "Ghost", "https://ghost.org"],
      ];
      for (const [pattern, name, url] of hosted) {
        if (pattern.test(xGenerator)) add(name, "platform", null, url);
      }
      if (header("d-cache")) add("Duda", "platform", null, "https://www.duda.co");

      if (header("cf-ray")) add("Cloudflare", "hosting", null, "https://www.cloudflare.com");
      if (header("x-vercel-id") || header("x-vercel-cache")) add("Vercel", "hosting", null, "https://vercel.com");
      if (header("x-nf-request-id")) add("Netlify", "hosting", null, "https://www.netlify.com");
      if (header("x-amz-cf-id") || /cloudfront/i.test(header("via")) || /cloudfront/i.test(header("x-cache"))) add("Amazon CloudFront", "hosting", null, "https://aws.amazon.com/cloudfront");
      if (header("x-github-request-id")) add("GitHub Pages", "hosting", null, "https://pages.github.com");
      if (header("x-fastly-request-id") || /cache-[a-z]{3}\d+/i.test(header("x-served-by"))) add("Fastly", "hosting", null, "https://www.fastly.com");
      if (header("x-azure-ref")) add("Azure Front Door", "hosting", null, "https://azure.microsoft.com/products/frontdoor");
      if (header("fly-request-id")) add("Fly.io", "hosting", null, "https://fly.io");
      if (header("x-render-origin-server")) add("Render", "hosting", null, "https://render.com");
      if (header("x-railway-request-id") || header("x-railway-edge")) add("Railway", "hosting", null, "https://railway.com");
      if (header("x-shopify-stage") || header("x-shopid")) add("Shopify", "platform", null, "https://www.shopify.com");
      if (header("x-wix-request-id")) add("Wix", "platform", null, "https://www.wix.com");
      if (header("x-varnish")) add("Varnish", "server", null, "https://varnish-cache.org");
      if (header("x-litespeed-cache")) add("LiteSpeed", "server", null, "https://www.litespeedtech.com");
      if (/^1\.1 google$/i.test(header("via"))) add("Google Cloud", "hosting", null, "https://cloud.google.com");
      if (header("x-goog-generation") || header("x-guploader-uploadid")) add("Google Cloud Storage", "hosting", null, "https://cloud.google.com/storage");
      if (header("x-kinsta-cache")) add("Kinsta", "hosting", null, "https://kinsta.com");
      if (header("x-pantheon-styx-hostname")) add("Pantheon", "hosting", null, "https://pantheon.io");
      if (header("x-hcdn-request-id") || /hostinger/i.test(header("platform"))) add("Hostinger", "hosting", null, "https://www.hostinger.com");
    } catch {}
    clearTimeout(timer);
  }

  // Languages the page itself runs.
  check(() => {
    if (scripts.length) add("JavaScript", "language", null, "https://developer.mozilla.org/docs/Web/JavaScript");
    if (resourceUrls.some((url) => /\.wasm(\?|$)/i.test(url))) add("WebAssembly", "language", null, "https://webassembly.org");
  });

  return { technologies: Array.from(found.values()) };
})()"##;

pub fn detect_technologies(app: &AppHandle, id: u32) -> Result<PageTechnologies, String> {
    let webview =
        tabs::page_view(app, id).ok_or("That tab is closed or its page is not loaded.")?;
    let value = webview2::evaluate(&webview, DETECT_SCRIPT, Duration::from_secs(10), FAILURE)?;
    let page: PageTechnologies =
        serde_json::from_value(value).map_err(|error| format!("{FAILURE} {error}"))?;
    Ok(page.clean())
}

impl PageTechnologies {
    /// The script runs beside page code that can tamper with it, so its answer is not trusted.
    fn clean(self) -> Self {
        let technologies = self
            .technologies
            .into_iter()
            .filter_map(|technology| {
                let name: String = technology.name.trim().chars().take(80).collect();
                if name.is_empty() {
                    return None;
                }
                let category = if CATEGORIES.contains(&technology.category.as_str()) {
                    technology.category
                } else {
                    "other".to_string()
                };
                let version = technology
                    .version
                    .map(|version| version.trim().chars().take(40).collect::<String>())
                    .filter(|version| !version.is_empty());
                let url = technology
                    .url
                    .filter(|url| url.starts_with("https://") && url.len() <= 300);
                Some(DetectedTechnology {
                    name,
                    category,
                    version,
                    url,
                })
            })
            .take(MAX_TECHNOLOGIES)
            .collect();
        Self { technologies }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_drops_blank_names_and_unsafe_links() {
        let page = PageTechnologies {
            technologies: vec![
                DetectedTechnology {
                    name: " React ".into(),
                    category: "framework".into(),
                    version: Some(" 19.1.0 ".into()),
                    url: Some("https://react.dev".into()),
                },
                DetectedTechnology {
                    name: "Odd".into(),
                    category: "made-up".into(),
                    version: Some("  ".into()),
                    url: Some("javascript:alert(1)".into()),
                },
                DetectedTechnology {
                    name: "   ".into(),
                    category: "library".into(),
                    version: None,
                    url: None,
                },
            ],
        }
        .clean();
        assert_eq!(page.technologies.len(), 2);
        assert_eq!(page.technologies[0].name, "React");
        assert_eq!(page.technologies[0].version.as_deref(), Some("19.1.0"));
        assert_eq!(page.technologies[1].category, "other");
        assert_eq!(page.technologies[1].version, None);
        assert_eq!(page.technologies[1].url, None);
    }

    #[test]
    fn platform_category_is_kept() {
        let page = PageTechnologies {
            technologies: vec![DetectedTechnology {
                name: "HighLevel".into(),
                category: "platform".into(),
                version: None,
                url: Some("https://www.highlevel.com".into()),
            }],
        }
        .clean();
        assert_eq!(page.technologies.len(), 1);
        assert_eq!(page.technologies[0].name, "HighLevel");
        assert_eq!(page.technologies[0].category, "platform");
        assert!(CATEGORIES.contains(&"platform"));
    }
}
