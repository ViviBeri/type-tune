#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const manifest = JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8"));
const failures = [];
const platformPackages = [
  "@type-tune/darwin-arm64",
  "@type-tune/darwin-x64",
  "@type-tune/linux-x64-gnu",
  "@type-tune/linux-x64-musl",
  "@type-tune/win32-x64-msvc",
];

if (manifest.bin?.["type-tune"] !== "./bin/type-tune.mjs") {
  failures.push('package.json bin.type-tune must point to "./bin/type-tune.mjs".');
}
if (!manifest.files?.includes("bin")) failures.push('package.json files must include "bin".');
if (Object.keys(manifest.dependencies ?? {}).length > 0) {
  failures.push("the Rust CLI package must not have JavaScript runtime dependencies.");
}
for (const name of platformPackages) {
  if (manifest.optionalDependencies?.[name] !== manifest.version) {
    failures.push(`${name} must be an optional dependency at the root package version.`);
  }
}

const launcherPath = path.join(root, "bin", "type-tune.mjs");
if (!fs.existsSync(launcherPath)) failures.push("bin/type-tune.mjs is missing.");
const syntax = spawnSync(process.execPath, ["--check", launcherPath], { encoding: "utf8" });
if (syntax.status !== 0) failures.push(syntax.stderr || "the npm launcher has invalid syntax.");

const packed = spawnSync("npm", ["pack", "--dry-run", "--json", "--ignore-scripts"], {
  cwd: root,
  encoding: "utf8",
});
if (packed.status !== 0) {
  failures.push(packed.stderr || packed.stdout || "npm pack --dry-run failed.");
} else {
  const entries = JSON.parse(packed.stdout);
  const packedPaths = new Set(entries[0]?.files?.map((file) => file.path) ?? []);
  for (const filePath of ["README.md", "LICENSE", "package.json", "bin/type-tune.mjs"]) {
    if (!packedPaths.has(filePath)) failures.push(`${filePath} is missing from npm pack output.`);
  }
}

try {
  verifyPackedLauncher();
} catch (error) {
  failures.push(error instanceof Error ? error.message : String(error));
}

if (failures.length > 0) {
  for (const failure of failures) process.stderr.write(`Package check failed: ${failure}\n`);
  process.exit(1);
}
process.stdout.write("Package check passed.\n");

/** Verifies a packed launcher resolves and runs the staged binary for this platform when available. */
function verifyPackedLauncher() {
  const platformPackage = currentPlatformPackage();
  if (!platformPackage) {
    process.stdout.write("Skipping packed launcher smoke test on an unsupported platform.\n");
    return;
  }

  const stagedPackagePath = path.join(root, "packages", platformPackage.directory);
  const stagedBinaryPath = path.join(stagedPackagePath, "bin", platformPackage.binaryName);
  if (!fs.existsSync(stagedBinaryPath)) {
    process.stdout.write("Skipping packed launcher smoke test because no native binary is staged.\n");
    return;
  }

  const temporaryRoot = fs.mkdtempSync(path.join(os.tmpdir(), "type-tune-package-check-"));
  try {
    const packedRoot = path.join(temporaryRoot, "pack");
    fs.mkdirSync(packedRoot);
    const packedRootResult = run("npm", [
      "pack",
      "--json",
      "--ignore-scripts",
      "--pack-destination",
      packedRoot,
    ]);
    const tarballName = JSON.parse(packedRootResult)[0]?.filename;
    if (typeof tarballName !== "string") {
      throw new Error("npm pack did not report the root package tarball.");
    }

    const consumerRoot = path.join(temporaryRoot, "consumer");
    const packageRoot = path.join(consumerRoot, "node_modules", "type-tune");
    const platformRoot = path.join(
      consumerRoot,
      "node_modules",
      "@type-tune",
      platformPackage.directory.replace("type-tune-", ""),
    );
    fs.mkdirSync(packageRoot, { recursive: true });
    run("tar", [
      "-xzf",
      path.join(packedRoot, tarballName),
      "-C",
      packageRoot,
      "--strip-components",
      "1",
    ]);
    fs.cpSync(stagedPackagePath, platformRoot, { recursive: true });

    const helpOutput = run(process.execPath, [path.join(packageRoot, "bin", "type-tune.mjs"), "--help"], {
      cwd: consumerRoot,
    });
    if (!helpOutput.includes("Usage: type-tune <command>")) {
      throw new Error("the packed npm launcher did not run the staged Rust CLI.");
    }
  } finally {
    fs.rmSync(temporaryRoot, { force: true, recursive: true });
  }
}

/** Returns the workspace platform package matching this Node runtime. */
function currentPlatformPackage() {
  if (process.platform === "darwin" && process.arch === "arm64") {
    return { binaryName: "type-tune", directory: "type-tune-darwin-arm64" };
  }
  if (process.platform === "darwin" && process.arch === "x64") {
    return { binaryName: "type-tune", directory: "type-tune-darwin-x64" };
  }
  if (process.platform === "linux" && process.arch === "x64") {
    return {
      binaryName: "type-tune",
      directory: usesGlibc() ? "type-tune-linux-x64-gnu" : "type-tune-linux-x64-musl",
    };
  }
  if (process.platform === "win32" && process.arch === "x64") {
    return { binaryName: "type-tune.exe", directory: "type-tune-win32-x64-msvc" };
  }
  return undefined;
}

/** Detects whether this Linux runtime uses glibc. */
function usesGlibc() {
  try {
    return Boolean(process.report?.getReport()?.header?.glibcVersionRuntime);
  } catch {
    return false;
  }
}

/** Runs a process and returns stdout or throws its diagnostic output. */
function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd ?? root,
    encoding: "utf8",
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(result.stderr || result.stdout || `${command} failed.`);
  }
  return result.stdout;
}
