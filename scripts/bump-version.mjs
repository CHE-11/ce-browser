import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const packagePath = join(root, "package.json");
const levels = ["major", "minor", "patch"];
const level = (process.argv[2] ?? "patch").toLowerCase();

if (!levels.includes(level)) {
  console.error(`Usage: node scripts/bump-version.mjs [${levels.join("|")}]`);
  process.exit(1);
}

const source = readFileSync(packagePath, "utf8");
const pkg = JSON.parse(source);
const current = String(pkg.version);
const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(current);
if (!match) {
  console.error(`package.json version must be major.minor.patch, found ${current}`);
  process.exit(1);
}

let major = Number(match[1]);
let minor = Number(match[2]);
let patch = Number(match[3]);
if (level === "major") {
  major += 1;
  minor = 0;
  patch = 0;
} else if (level === "minor") {
  minor += 1;
  patch = 0;
} else {
  patch += 1;
}

const next = `${major}.${minor}.${patch}`;
pkg.version = next;
const newline = source.endsWith("\n") ? "\n" : "";
writeFileSync(packagePath, `${JSON.stringify(pkg, null, 2)}${newline}`);
setJsonVersion(join(root, "src-tauri", "tauri.conf.json"), next);
setCargoPackageVersion(join(root, "src-tauri", "Cargo.toml"), next);
console.log(`${pkg.name} ${current} → ${next}`);

function setJsonVersion(path, version) {
  const source = readFileSync(path, "utf8");
  const updated = source.replace(/("version"\s*:\s*")[^"]+(")/, `$1${version}$2`);
  if (updated === source) {
    console.error(`Could not update version in ${path}`);
    process.exit(1);
  }
  writeFileSync(path, updated);
}

function setCargoPackageVersion(path, version) {
  const source = readFileSync(path, "utf8");
  const updated = source.replace(/^version = "[^"]+"/m, `version = "${version}"`);
  if (updated === source) {
    console.error(`Could not update version in ${path}`);
    process.exit(1);
  }
  writeFileSync(path, updated);
}
