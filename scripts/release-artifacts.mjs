import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { isDeepStrictEqual } from "node:util";

// This list must match the build and publish matrices in npm-publish.yml.
const targets = {
  "type-tune-linux-x64-gnu": "x86_64-unknown-linux-gnu",
  "type-tune-linux-x64-musl": "x86_64-unknown-linux-musl",
  "type-tune-darwin-x64": "x86_64-apple-darwin",
  "type-tune-darwin-arm64": "aarch64-apple-darwin",
  "type-tune-win32-x64-msvc": "x86_64-pc-windows-msvc",
};
const [command, directory, name, target, sha] = process.argv.slice(2);
const hash = (file) => createHash("sha256").update(fs.readFileSync(file)).digest("hex");
const readJson = (file) => JSON.parse(fs.readFileSync(file, "utf8"));
const version = readJson("package.json").version;

if (command === "create") {
  if (!directory || !targets[name] || targets[name] !== target || !/^[a-f0-9]{40}$/.test(sha ?? "")) {
    throw new Error("Expected artifact directory, supported package, matching target and commit SHA.");
  }
  fs.mkdirSync(directory, { recursive: true });
  const packed = JSON.parse(execFileSync("npm", ["pack", `./packages/${name}`, "--ignore-scripts", "--json", "--pack-destination", directory], { encoding: "utf8" }));
  const tarball = path.join(directory, packed[0].filename);
  fs.renameSync(tarball, path.join(directory, "package.tgz"));
  fs.writeFileSync(path.join(directory, "metadata.json"), JSON.stringify({ name, target, sha, version, digest: hash(path.join(directory, "package.tgz")) }) + "\n");
} else if (command === "verify") {
  // Verify all five before permitting any native publication.
  const expectedSha = name;
  if (!directory || !/^[a-f0-9]{40}$/.test(expectedSha ?? "")) throw new Error("Expected artifact directory and commit SHA.");
  for (const [packageName, expectedTarget] of Object.entries(targets)) {
    const artifact = path.join(directory, packageName);
    const metadata = readJson(path.join(artifact, "metadata.json"));
    const tarball = path.join(artifact, "package.tgz");
    if (!isDeepStrictEqual(metadata, { name: packageName, target: expectedTarget, sha: expectedSha, version, digest: hash(tarball) })) {
      throw new Error(`Invalid metadata or tarball digest for ${packageName}.`);
    }
    const temp = fs.mkdtempSync(path.join(os.tmpdir(), "type-tune-artifact-"));
    try {
      execFileSync("tar", ["-xzf", path.resolve(tarball), "-C", temp]);
      const packageDir = path.join(temp, "package");
      const actual = readJson(path.join(packageDir, "package.json"));
      const expected = readJson(path.join("packages", packageName, "package.json"));
      if (!isDeepStrictEqual(actual, expected) || actual.version !== version) {
        throw new Error(`Unexpected package metadata for ${packageName}.`);
      }
      const binaryName = actual.os.includes("win32") ? "type-tune.exe" : "type-tune";
      const binary = path.join(packageDir, "bin", binaryName);
      if (!actual.os.includes("win32") && (fs.statSync(binary).mode & 0o111) === 0) throw new Error(`${packageName} binary is not executable.`);
      if (fs.readFileSync(`${binary}.sha256`, "utf8") !== `${hash(binary)}  ${binaryName}\n`) throw new Error(`Invalid binary checksum for ${packageName}.`);
    } finally {
      fs.rmSync(temp, { recursive: true, force: true });
    }
  }
  console.log(`Verified all five native packages for ${expectedSha} and version ${version}.`);
} else {
  throw new Error("Usage: release-artifacts.mjs create <dir> <name> <target> <sha> | verify <dir> <sha>");
}
