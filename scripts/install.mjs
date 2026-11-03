import { spawn } from "node:child_process";
import { existsSync, readdirSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const nsisDir = join(root, "src-tauri", "target", "release", "bundle", "nsis");

if (!existsSync(nsisDir)) {
  console.error(`No NSIS installer in ${nsisDir}`);
  console.error("Run pnpm build:exe first.");
  process.exit(1);
}

const installer = readdirSync(nsisDir)
  .filter((name) => name.endsWith("-setup.exe"))
  .map((name) => {
    const path = join(nsisDir, name);
    return { path, mtime: statSync(path).mtimeMs };
  })
  .sort((left, right) => right.mtime - left.mtime)[0];

if (!installer) {
  console.error(`No NSIS installer in ${nsisDir}`);
  console.error("Run pnpm build:exe first.");
  process.exit(1);
}

console.log(`Opening ${installer.path}`);
const child = spawn(installer.path, {
  detached: true,
  stdio: "ignore",
});
child.on("error", (error) => {
  console.error(error.message);
  process.exit(1);
});
child.unref();
