import { spawn } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const config = JSON.parse(readFileSync(join(root, "src-tauri", "tauri.conf.json"), "utf8"));
const installer = join(
  root,
  "src-tauri",
  "target",
  "release",
  "bundle",
  "nsis",
  `${config.productName}_${config.version}_x64-setup.exe`,
);

// The updater only trusts installers signed with the key whose public half is in tauri.conf.json.
const keyFile = "S:/_keys_/ce-browser.key";
if (!existsSync(keyFile)) {
  console.error(`No signing key at ${keyFile}`);
  process.exit(1);
}

const env = {
  ...process.env,
  TAURI_SIGNING_PRIVATE_KEY: keyFile,
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD: process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ?? "",
};

const child = spawn("pnpm tauri build --bundles nsis", {
  cwd: root,
  stdio: "inherit",
  shell: true,
  env,
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }

  if (code !== 0) {
    process.exit(code ?? 1);
  }

  for (const file of [installer, `${installer}.sig`]) {
    if (!existsSync(file)) {
      console.error(`Expected build output at ${file}`);
      process.exit(1);
    }
  }

  console.log(`\nBuilt ${installer}`);
});
