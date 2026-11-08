#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";

const [packageDirectory] = process.argv.slice(2);
if (!packageDirectory) {
  throw new Error("Usage: node scripts/check-native-package.mjs <package-directory>");
}

const absoluteDirectory = path.resolve(packageDirectory);
const manifestPath = path.join(absoluteDirectory, "package.json");
const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
const binaryName = manifest.os?.includes("win32") ? "type-tune.exe" : "type-tune";
const binaryRelativePath = path.posix.join("bin", binaryName);
const checksumRelativePath = `${binaryRelativePath}.sha256`;
const binaryPath = path.join(absoluteDirectory, binaryRelativePath);
const checksumPath = path.join(absoluteDirectory, checksumRelativePath);

if (!manifest.name || !fs.existsSync(binaryPath) || !fs.existsSync(checksumPath)) {
  throw new Error(`${manifestPath} must declare a package name and contain its staged binary and checksum.`);
}
if (!manifest.files?.includes(binaryRelativePath) || !manifest.files.includes(checksumRelativePath)) {
  throw new Error(`${manifest.name} must package ${binaryRelativePath} and ${checksumRelativePath}.`);
}
if (!manifest.os.includes("win32") && (fs.statSync(binaryPath).mode & 0o111) === 0) {
  throw new Error(`${manifest.name} binary is not executable.`);
}

const expected = `${createHash("sha256").update(fs.readFileSync(binaryPath)).digest("hex")}  ${binaryName}\n`;
if (fs.readFileSync(checksumPath, "utf8") !== expected) {
  throw new Error(`${manifest.name} checksum does not match ${binaryRelativePath}.`);
}

const packed = spawnSync("npm", ["pack", "--dry-run", "--json", "--ignore-scripts"], {
  cwd: absoluteDirectory,
  encoding: "utf8",
});
if (packed.error) throw packed.error;
if (packed.status !== 0) throw new Error(packed.stderr || packed.stdout || "npm pack failed.");
const entries = JSON.parse(packed.stdout);
const packedPaths = new Set(entries[0]?.files?.map((file) => file.path) ?? []);
for (const filePath of ["package.json", binaryRelativePath, checksumRelativePath]) {
  if (!packedPaths.has(filePath)) throw new Error(`${manifest.name} npm pack output is missing ${filePath}.`);
}

process.stdout.write(`Native package check passed for ${manifest.name}.\n`);
