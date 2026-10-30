import { spawn } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const { name } = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
const output = join(root, "src-tauri", "target", "release", `${name}.exe`);

const child = spawn("pnpm tauri build --no-bundle", {
  cwd: root,
  stdio: "inherit",
  shell: true,
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }

  if (code !== 0) {
    process.exit(code ?? 1);
  }

  if (!existsSync(output)) {
    console.error(`Expected build output at ${output}`);
    process.exit(1);
  }

  console.log(`\nBuilt ${output}`);
});
