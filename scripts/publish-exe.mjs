import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const { name, version } = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
const tag = `v${version}`;
const releaseDir = join(root, "src-tauri", "target", "release");
const built = join(releaseDir, `${name}.exe`);
const assetName = `${name}_${String(version).split(".").join("_")}.exe`;
const assetPath = join(releaseDir, assetName);
const versionFiles = ["package.json", "src-tauri/tauri.conf.json", "src-tauri/Cargo.toml"];

if (!existsSync(built)) {
  console.error(`Expected build output at ${built}`);
  console.error("Run pnpm build:exe first.");
  process.exit(1);
}

const tauriVersion = JSON.parse(readFileSync(join(root, "src-tauri", "tauri.conf.json"), "utf8")).version;
const cargoVersion = /^version = "([^"]+)"/m.exec(readFileSync(join(root, "src-tauri", "Cargo.toml"), "utf8"))?.[1];
if (tauriVersion !== version || cargoVersion !== version) {
  console.error("package.json, tauri.conf.json, and Cargo.toml must use the same version.");
  process.exit(1);
}

const dirty = spawnSync("git", ["diff", "--quiet", "HEAD", "--", ...versionFiles], { cwd: root });
if (dirty.error) {
  console.error(dirty.error.message);
  process.exit(1);
}
if (dirty.status === 1) {
  console.error("Commit the version bump before publishing.");
  process.exit(1);
}
if (dirty.status !== 0) {
  process.exit(dirty.status ?? 1);
}

const sha = git(["rev-parse", "HEAD"]);
const remote = spawnSync("gh", ["api", "--silent", `repos/{owner}/{repo}/commits/${sha}`], {
  cwd: root,
  encoding: "utf8",
  stdio: ["ignore", "pipe", "pipe"],
});
if (remote.error) {
  console.error(remote.error.message);
  process.exit(1);
}
if (remote.status !== 0) {
  console.error(`Commit ${sha.slice(0, 7)} is not on GitHub. Push this branch, then run pnpm publish:exe again.`);
  process.exit(remote.status ?? 1);
}

copyFileSync(built, assetPath);
run([
  "release",
  "create",
  tag,
  assetPath,
  "--title",
  tag,
  "--target",
  sha,
  "--generate-notes",
  "--latest",
]);
console.log(`\nPublished ${tag} with ${assetName}`);

function git(args) {
  const result = spawnSync("git", args, {
    cwd: root,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });

  if (result.error) {
    console.error(result.error.message);
    process.exit(1);
  }

  if (result.status !== 0) {
    if (result.stderr) process.stderr.write(result.stderr);
    process.exit(result.status ?? 1);
  }

  return (result.stdout ?? "").trim();
}

function run(args) {
  const result = spawnSync("gh", args, {
    cwd: root,
    encoding: "utf8",
    stdio: "inherit",
  });

  if (result.error) {
    console.error(result.error.message);
    process.exit(1);
  }

  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}
