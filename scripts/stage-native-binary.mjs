#!/usr/bin/env node

import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";

const [packageDirectory, binaryPath] = process.argv.slice(2);
if (!packageDirectory || !binaryPath) {
  throw new Error("Usage: node scripts/stage-native-binary.mjs <package-directory> <binary-path>");
}

const manifestPath = path.join(packageDirectory, "package.json");
const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
const binaryName = manifest.os?.includes("win32") ? "type-tune.exe" : "type-tune";
const destinationDirectory = path.join(packageDirectory, "bin");
const destinationPath = path.join(destinationDirectory, binaryName);
const checksumPath = `${destinationPath}.sha256`;

fs.mkdirSync(destinationDirectory, { recursive: true });
fs.copyFileSync(binaryPath, destinationPath);
fs.chmodSync(destinationPath, 0o755);
fs.writeFileSync(
  checksumPath,
  `${createHash("sha256").update(fs.readFileSync(destinationPath)).digest("hex")}  ${binaryName}\n`,
);

const files = new Set(manifest.files ?? []);
files.add(path.posix.join("bin", binaryName));
files.add(path.posix.join("bin", `${binaryName}.sha256`));
manifest.files = [...files].sort((left, right) => left.localeCompare(right, "en"));
fs.writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);

process.stdout.write(`${destinationPath}\n${checksumPath}\n`);
