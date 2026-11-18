import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

const rootDir = path.dirname(fileURLToPath(import.meta.url));
const host = process.env.TAURI_DEV_HOST;

export default defineConfig(() => ({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: [
      { find: "@components", replacement: path.resolve(rootDir, "src/components") },
      { find: "@features", replacement: path.resolve(rootDir, "src/features") },
      { find: "@lib", replacement: path.resolve(rootDir, "src/lib") },
      { find: "@type", replacement: path.resolve(rootDir, "../types") },
      { find: "@", replacement: path.resolve(rootDir, "src") },
      { find: "monaco-editor.css", replacement: path.resolve(rootDir, "node_modules/monaco-editor/min/vs/editor/editor.main.css") },
    ],
  },
  clearScreen: false,
  // Prebundling these pulls Monaco features in without their services, and breaks Biome's wasm URL.
  optimizeDeps: {
    exclude: ["monaco-editor", "@biomejs/wasm-web"],
  },
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));
