import { spawnSync, execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const packages = ["darwin-arm64", "darwin-x64", "linux-x64-gnu", "linux-x64-musl", "win32-x64-msvc"].map((x) => `@type-tune/${x}`);
const version = "1.0.0";
const expectedSha = "89ee004f9504270ba52aa5e2f297b70e95a42744";
const fail = (message) => { throw new Error(message); };
const hash = (file) => createHash("sha256").update(fs.readFileSync(file)).digest("hex");

export function validateRequest(requested, dryRun, confirmation) {
  if (requested !== version) fail("Recovery is restricted to version 1.0.0.");
  if (dryRun !== "true" && dryRun !== "false") fail("dry_run must be true or false.");
  if (dryRun === "false" && confirmation !== "PUBLISH type-tune@1.0.0") fail("Explicit publish confirmation does not match.");
}

export function validateManifest(manifest) {
  if (manifest.name !== "type-tune" || manifest.version !== version) fail("Wrong package name or version.");
  if (manifest.bin?.["type-tune"] !== "./bin/type-tune.mjs" && manifest.bin?.["type-tune"] !== "bin/type-tune.mjs") fail("CLI bin entry is missing or invalid.");
  if (typeof manifest.license !== "string" || !manifest.license || !manifest.repository) fail("Required package metadata is missing.");
  const deps = manifest.optionalDependencies;
  if (!deps || Object.keys(deps).length !== packages.length || packages.some((name) => deps[name] !== version)) fail("Native optional dependencies do not match 1.0.0.");
}

export function validateAuthentication(result) {
  if (result.error || result.status !== 0 || !result.stdout?.trim()) fail("npm registry authentication failed; no publication is safe.");
}

export async function checkRegistry(fetcher, token) {
  if (!token) fail("NPM_TOKEN is missing.");
  async function get(name) {
    let response;
    try {
      response = await fetcher(`https://registry.npmjs.org/${encodeURIComponent(name)}/${version}`, {
        headers: { authorization: `Bearer ${token}`, accept: "application/json" },
        redirect: "error", signal: AbortSignal.timeout(15000),
      });
    } catch { fail(`Registry lookup failed for ${name}; state is unknown.`); }
    let body;
    try { body = await response.json(); } catch { fail(`Invalid registry response for ${name}.`); }
    return { status: response.status, body };
  }
  for (const name of packages) {
    const result = await get(name);
    if (result.status !== 200 || result.body?.name !== name || result.body?.version !== version) fail(`Native dependency ${name}@1.0.0 is missing or registry state is ambiguous.`);
  }
  const root = await get("type-tune");
  if (root.status === 200) {
    if (root.body?.name !== "type-tune" || root.body?.version !== version) fail("Root registry response is ambiguous.");
    fail("type-tune@1.0.0 already exists; do not publish.");
  }
  const missing = typeof root.body === "string" ? root.body : root.body?.error;
  if (root.status !== 404 || typeof missing !== "string" || !/^not found$/i.test(missing)) fail("Root package registry state is ambiguous.");
  return "Authenticated registry returned a canonical 404 for type-tune@1.0.0; five native packages are available.";
}

export function verifyTarball(tarball) {
  const absolute = path.resolve(tarball);
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "type-tune-v1-root-"));
  try {
    const entries = execFileSync("tar", ["-tzf", absolute], { encoding: "utf8" }).trim().split("\n");
    const expected = ["package/LICENSE", "package/README.md", "package/bin/type-tune.mjs", "package/package.json"];
    if (entries.length !== expected.length || new Set(entries).size !== entries.length || expected.some((x) => !entries.includes(x))) fail("Tarball contents differ from the expected package files.");
    execFileSync("tar", ["-xzf", absolute, "-C", dir]);
    const pkg = path.join(dir, "package");
    validateManifest(JSON.parse(fs.readFileSync(path.join(pkg, "package.json"), "utf8")));
    const launcher = path.join(pkg, "bin/type-tune.mjs");
    if (!fs.lstatSync(launcher).isFile() || !fs.readFileSync(launcher, "utf8").startsWith("#!/usr/bin/env node\n")) fail("Launcher is missing or invalid.");
    const mode = fs.statSync(launcher).mode & 0o777;
    if ((mode & 0o444) !== 0o444 || (mode & 0o022) !== 0) fail("Tarball launcher permissions are unsafe.");
    // npm normalizes the manifest when publishing. Check a disposable extraction, never the tarball.
    const normalized = spawnSync("npm", ["pkg", "fix", "--ignore-scripts"], { cwd: pkg, encoding: "utf8" });
    if (normalized.error || normalized.status !== 0) fail("npm manifest normalization failed.");
    validateManifest(JSON.parse(fs.readFileSync(path.join(pkg, "package.json"), "utf8")));
    const consumer = path.join(dir, "consumer");
    fs.mkdirSync(consumer);
    const install = spawnSync("npm", ["install", "--ignore-scripts", "--omit=optional", "--offline", "--no-audit", "--no-fund", "--no-package-lock", "--no-save", "--cache", path.join(dir, "cache"), absolute], { cwd: consumer, encoding: "utf8" });
    if (install.error || install.status !== 0) fail("Offline npm installation of the packed root package failed.");
    const installed = path.join(consumer, "node_modules/type-tune/bin/type-tune.mjs");
    const link = path.join(consumer, "node_modules/.bin/type-tune");
    if ((fs.statSync(installed).mode & 0o111) !== 0o111 || fs.realpathSync(link) !== fs.realpathSync(installed)) fail("Installed CLI is not executable or its link is invalid.");
    validateManifest(JSON.parse(fs.readFileSync(path.join(consumer, "node_modules/type-tune/package.json"), "utf8")));
    console.log(`Verified tarball launcher mode ${mode.toString(8)}; npm installed an executable CLI.`);
    return hash(absolute);
  } finally { fs.rmSync(dir, { recursive: true, force: true }); }
}

async function main() {
  const [command, ...args] = process.argv.slice(2);
  if (command === "request") validateRequest(...args);
  else if (command === "source") {
    const [dir, requested] = args;
    if (requested !== version) fail("Recovery is restricted to version 1.0.0.");
    const sha = execFileSync("git", ["-C", dir, "rev-parse", "HEAD^{commit}"], { encoding: "utf8" }).trim();
    if (sha !== expectedSha) fail("Checked-out commit does not match the original v1.0.0 SHA.");
    validateManifest(JSON.parse(fs.readFileSync(path.join(dir, "package.json"), "utf8")));
    console.log(`Verified original v1.0.0 source ${sha}.`);
  } else if (command === "registry") {
    const auth = spawnSync("npm", ["whoami", "--registry=https://registry.npmjs.org/"], { encoding: "utf8" });
    validateAuthentication(auth);
    console.log(await checkRegistry(fetch, process.env.NODE_AUTH_TOKEN));
  } else if (command === "report") {
    try {
      const response = await fetch(`https://registry.npmjs.org/type-tune/${version}`, {
        headers: { authorization: `Bearer ${process.env.NODE_AUTH_TOKEN ?? ""}`, accept: "application/json" },
        redirect: "error", signal: AbortSignal.timeout(15000),
      });
      const body = await response.json();
      if (response.status === 200 && body?.name === "type-tune" && body?.version === version) console.log("Registry state: type-tune@1.0.0 exists.");
      else if (response.status === 404 && (body === "Not Found" || (typeof body?.error === "string" && /^not found$/i.test(body.error)))) console.log("Registry state: canonical 404 for type-tune@1.0.0.");
      else console.log(`Registry state is ambiguous (HTTP ${response.status}).`);
    } catch { console.log("Registry state is unknown: lookup failed."); }
  } else if (command === "tarball") {
    const digest = verifyTarball(args[0]);
    if (!process.env.GITHUB_OUTPUT) fail("GITHUB_OUTPUT is required.");
    fs.appendFileSync(process.env.GITHUB_OUTPUT, `sha256=${digest}\n`);
    console.log(`Verified tarball SHA-256: ${digest}`);
  } else if (command === "digest") {
    if (!/^[a-f0-9]{64}$/.test(args[1] ?? "") || hash(args[0]) !== args[1]) fail("Verified tarball digest changed.");
  } else fail("Unknown recovery validation command.");
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((error) => { console.error(error.message); process.exitCode = 1; });
}
