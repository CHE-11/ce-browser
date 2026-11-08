import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import * as esbuild from "esbuild";
import postcss from "postcss";
import tailwindcss from "@tailwindcss/postcss";

const dir = path.dirname(fileURLToPath(import.meta.url));
const src = path.join(dir, "src");
const out = path.join(dir, "build");
const manifest = JSON.parse(readFileSync(path.join(dir, "manifest.json"), "utf8"));
const title = manifest.name ?? "Studio";

const pages = new Set();
const addPage = (value) => {
  if (typeof value === "string" && value.endsWith(".html")) pages.add(value.slice(0, -".html".length));
};
addPage(manifest.background);
addPage(manifest.toolbar?.popup);
addPage(manifest.panel?.path);
addPage(manifest.popout?.path);
addPage(manifest.pageToolbar?.path);
addPage(manifest.bookmarksBar);
addPage(manifest.downloads);
for (const page of manifest.pages ?? []) addPage(page.path);

const contentPath = [path.join(src, "content.js"), path.join(dir, "content.js")].find((file) => existsSync(file));
const content = contentPath
  ? readFileSync(contentPath, "utf8")
  : `if (window.ce && window.ce.runtime && window.ce.runtime.sendMessage) {
  window.ce.runtime.sendMessage(${JSON.stringify(manifest.id)}, { title: document.title, href: location.href });
}
`;

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
cpSync(path.join(dir, "manifest.json"), path.join(out, "manifest.json"));

const icon = [path.join(src, "icon.svg"), path.join(dir, "icon.svg")].find((file) => existsSync(file));
if (icon) cpSync(icon, path.join(out, "icon.svg"));
writeFileSync(path.join(out, "content.js"), content);

const pageTitle = (name) => (name === "background" || name === "downloads" ? `${title} ${name}` : title);
for (const name of pages) {
  writeFileSync(
    path.join(out, `${name}.html`),
    `<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <title>${pageTitle(name)}</title>
    <link id="studio-${name}-style" rel="stylesheet" href="studio.css" />
  </head>
  <body>
    <div id="root"></div>
    <script id="studio-${name}-script" src="${name}.js"></script>
  </body>
</html>
`,
  );
}

const entryPoints = [...pages].map((name) => path.join(src, `${name}.jsx`)).filter((file) => existsSync(file));
await esbuild.build({
  entryPoints,
  outdir: out,
  outbase: src,
  bundle: true,
  format: "iife",
  platform: "browser",
  target: ["es2022"],
  jsx: "automatic",
  minify: true,
  legalComments: "none",
});

const style = path.join(src, "app.css");
if (existsSync(style)) {
  const to = path.join(out, "studio.css");
  const result = await postcss([tailwindcss()]).process(readFileSync(style, "utf8"), { from: style, to });
  writeFileSync(to, result.css);
}

console.log("built sample_kitchen_sink/build");
