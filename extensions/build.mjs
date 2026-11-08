import { readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import * as esbuild from "esbuild";

const root = path.dirname(fileURLToPath(import.meta.url));
const samples = readdirSync(root, { withFileTypes: true })
  .filter((entry) => entry.isDirectory() && entry.name.startsWith("sample_") && entry.name !== "sample_simple")
  .map((entry) => entry.name)
  .filter((name) => {
    try {
      return readdirSync(path.join(root, name, "src")).some((file) => file.endsWith(".jsx"));
    } catch {
      return false;
    }
  });

for (const name of samples) {
  const dir = path.join(root, name);
  const src = path.join(dir, "src");
  const pages = new Set(
    readdirSync(dir)
      .filter((file) => file.endsWith(".html"))
      .map((file) => file.slice(0, -".html".length)),
  );
  const entryPoints = readdirSync(src)
    .filter((file) => file.endsWith(".jsx") && pages.has(file.slice(0, -".jsx".length)))
    .map((file) => path.join(src, file));
  await esbuild.build({
    entryPoints,
    outdir: dir,
    outbase: src,
    bundle: true,
    format: "iife",
    platform: "browser",
    target: ["es2022"],
    jsx: "automatic",
    minify: true,
    legalComments: "none",
  });
  console.log(`built ${name}`);
}
