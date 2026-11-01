import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const { version } = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
const config = JSON.parse(readFileSync(join(root, "src-tauri", "tauri.conf.json"), "utf8"));
const tag = `v${version}`;
const bundleDir = join(root, "src-tauri", "target", "release", "bundle", "nsis");
const installer = join(bundleDir, `${config.productName}_${version}_x64-setup.exe`);
const signature = `${installer}.sig`;
const manifest = join(bundleDir, "latest.json");
const versionFiles = ["package.json", "src-tauri/tauri.conf.json", "src-tauri/Cargo.toml"];

for (const file of [installer, signature]) {
  if (!existsSync(file)) {
    console.error(`Expected build output at ${file}`);
    console.error("Run pnpm build:exe first.");
    process.exit(1);
  }
}

const cargoVersion = /^version = "([^"]+)"/m.exec(readFileSync(join(root, "src-tauri", "Cargo.toml"), "utf8"))?.[1];
if (config.version !== version || cargoVersion !== version) {
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

const repository = gh(["repo", "view", "--json", "nameWithOwner", "-q", ".nameWithOwner"]);
run(["release", "create", tag, installer, signature, "--title", tag, "--target", sha, "--generate-notes", "--latest"]);

// The updater reads latest.json from the newest release. Its notes also fill ce://whats-new.
const notes = gh(["release", "view", tag, "--json", "body", "-q", ".body"]);
const update = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: {
    "windows-x86_64": {
      signature: readFileSync(signature, "utf8").trim(),
      url: `https://github.com/${repository}/releases/download/${tag}/${basename(installer)}`,
    },
  },
};
writeFileSync(manifest, `${JSON.stringify(update, null, 2)}\n`);
run(["release", "upload", tag, manifest, "--clobber"]);
console.log(`\nPublished ${tag} with ${basename(installer)} and latest.json`);

function git(args) {
  return capture("git", args);
}

function gh(args) {
  return capture("gh", args);
}

function capture(command, args) {
  const result = spawnSync(command, args, {
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
