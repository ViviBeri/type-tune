#!/usr/bin/env node

import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
import { spawnSync } from "node:child_process";

const require = createRequire(import.meta.url);

/** Runs the platform-specific Rust CLI selected for the current Node runtime. */
function main() {
  try {
    const binaryPath = resolveNativeBinary();
    const result = spawnSync(binaryPath, process.argv.slice(2), { stdio: "inherit" });
    if (result.error) {
      throw result.error;
    }
    process.exitCode = result.status ?? 1;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(`type-tune: ${message}\n`);
    process.exitCode = 1;
  }
}

/** Resolves and verifies the Rust executable supplied by the matching npm platform package. */
function resolveNativeBinary() {
  const packageName = nativePackageName(process.platform, process.arch, usesGlibc());
  if (!packageName) {
    throw new Error(`unsupported platform ${process.platform}-${process.arch}`);
  }

  let manifestPath;
  try {
    manifestPath = require.resolve(`${packageName}/package.json`);
  } catch {
    throw new Error(
      `the native package ${packageName} is unavailable; reinstall type-tune for this platform`,
    );
  }

  const binaryName = process.platform === "win32" ? "type-tune.exe" : "type-tune";
  const binaryPath = path.join(path.dirname(manifestPath), "bin", binaryName);
  if (!fs.existsSync(binaryPath)) {
    throw new Error(`${packageName} is missing bin/${binaryName}`);
  }

  const checksumPath = `${binaryPath}.sha256`;
  if (fs.existsSync(checksumPath)) {
    const expected = fs.readFileSync(checksumPath, "utf8").trim().split(/\s+/u)[0];
    const actual = createHash("sha256").update(fs.readFileSync(binaryPath)).digest("hex");
    if (expected !== actual) {
      throw new Error(`checksum verification failed for ${packageName}`);
    }
  }

  return binaryPath;
}

/** Returns the optional npm package serving a supported platform tuple. */
function nativePackageName(platform, arch, glibc) {
  if (platform === "darwin" && arch === "arm64") return "@type-tune/darwin-arm64";
  if (platform === "darwin" && arch === "x64") return "@type-tune/darwin-x64";
  if (platform === "linux" && arch === "x64") {
    return glibc ? "@type-tune/linux-x64-gnu" : "@type-tune/linux-x64-musl";
  }
  if (platform === "win32" && arch === "x64") return "@type-tune/win32-x64-msvc";
  return undefined;
}

/** Detects whether the current Linux runtime uses glibc. */
function usesGlibc() {
  try {
    const report = process.report?.getReport();
    return Boolean(report?.header?.glibcVersionRuntime);
  } catch {
    return false;
  }
}

main();
