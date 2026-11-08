import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const repositoryRoot = path.resolve(import.meta.dirname, "..");
const checkOnly = process.argv.slice(2).join(" ") === "--check";
const rootManifestPath = path.join(repositoryRoot, "package.json");
const rootManifest = readJson(rootManifestPath);
const version = rootManifest.version;

if (typeof version !== "string") {
  throw new Error("Root package.json must contain a string version.");
}

const changes = [];
for (const manifestPath of nativePackageManifestPaths()) {
  const manifest = readJson(manifestPath);
  if (manifest.version !== version) {
    manifest.version = version;
    changes.push(writeJsonChange(manifestPath, manifest));
  }
}

let optionalDependenciesChanged = false;
for (const [name, dependencyVersion] of Object.entries(rootManifest.optionalDependencies ?? {})) {
  if (name.startsWith("@type-tune/") && dependencyVersion !== version) {
    rootManifest.optionalDependencies[name] = version;
    optionalDependenciesChanged = true;
  }
}
if (optionalDependenciesChanged) {
  changes.push(writeJsonChange(rootManifestPath, rootManifest));
}

for (const manifestPath of cargoManifestPaths()) {
  const content = fs.readFileSync(manifestPath, "utf8");
  const next = content.replace(/^version = "[^"]+"$/m, `version = "${version}"`);
  if (next !== content) {
    changes.push({ path: manifestPath, content: next });
  }
}

const cargoLockPath = path.join(repositoryRoot, "Cargo.lock");
const cargoLock = fs.readFileSync(cargoLockPath, "utf8");
const nextCargoLock = cargoLock.replace(
  /(name = "type-tune(?:-core)?"\nversion = )"[^"]+"/g,
  `$1"${version}"`,
);
if (nextCargoLock !== cargoLock) {
  changes.push({ path: cargoLockPath, content: nextCargoLock });
}

if (checkOnly) {
  if (changes.length > 0) {
    throw new Error(
      `Native package versions do not match ${version}: ${changes.map((change) => path.relative(repositoryRoot, change.path)).join(", ")}`,
    );
  }
  process.stdout.write(`Native package versions match ${version}.\n`);
} else {
  for (const change of dedupeChanges(changes)) {
    fs.writeFileSync(change.path, change.content);
  }
  process.stdout.write(`Synchronized native package versions to ${version}.\n`);
}

function nativePackageManifestPaths() {
  const packagesPath = path.join(repositoryRoot, "packages");
  return fs
    .readdirSync(packagesPath, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && entry.name.startsWith("type-tune-"))
    .map((entry) => path.join(packagesPath, entry.name, "package.json"))
    .sort();
}

function cargoManifestPaths() {
  const cratesPath = path.join(repositoryRoot, "crates");
  return fs
    .readdirSync(cratesPath, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => path.join(cratesPath, entry.name, "Cargo.toml"))
    .filter((manifestPath) => fs.existsSync(manifestPath))
    .sort();
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function writeJsonChange(filePath, value) {
  return { path: filePath, content: `${JSON.stringify(value, null, 2)}\n` };
}

function dedupeChanges(changes) {
  return [...new Map(changes.map((change) => [change.path, change])).values()];
}
