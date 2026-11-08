import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { checkRegistry, validateAuthentication, validateManifest, validateRequest, verifyTarball } from "./v1-root-recovery.mjs";

const names = ["darwin-arm64", "darwin-x64", "linux-x64-gnu", "linux-x64-musl", "win32-x64-msvc"].map((s) => `@type-tune/${s}`);
const manifest = () => ({ name: "type-tune", version: "1.0.0", license: "MIT", repository: "https://example.invalid/repo", bin: { "type-tune": "./bin/type-tune.mjs" }, optionalDependencies: Object.fromEntries(names.map((name) => [name, "1.0.0"])) });
const response = (status, body) => ({ status, json: async () => body });
const registry = (overrides = {}) => async (url) => {
  const name = decodeURIComponent(new URL(url).pathname.split("/")[1]);
  return overrides[name] ?? (name === "type-tune" ? response(404, "Not Found") : response(200, { name, version: "1.0.0" }));
};

test("only 1.0.0 and explicitly confirmed non-dry-run requests are accepted", () => {
  validateRequest("1.0.0", "true", "");
  validateRequest("1.0.0", "false", "PUBLISH type-tune@1.0.0");
  assert.throws(() => validateRequest("1.0.1", "false", "PUBLISH type-tune@1.0.0"));
  assert.throws(() => validateRequest("1.0.0", "false", ""));
  assert.throws(() => validateRequest("1.0.0", "invalid", ""));
});

test("registry authentication must succeed", () => {
  validateAuthentication({ status: 0, stdout: "publisher\n" });
  assert.throws(() => validateAuthentication({ status: 1, stdout: "" }));
  assert.throws(() => validateAuthentication({ error: new Error("unavailable"), stdout: "" }));
});

test("registry validation fails closed for missing dependencies, existing root and ambiguous responses", async () => {
  await checkRegistry(registry(), "test-token");
  await assert.rejects(checkRegistry(registry(), ""));
  await assert.rejects(checkRegistry(registry({ [names[0]]: response(404, "Not Found") }), "test-token"));
  await assert.rejects(checkRegistry(registry({ "type-tune": response(200, { name: "type-tune", version: "1.0.0" }) }), "test-token"));
  await assert.rejects(checkRegistry(registry({ "type-tune": response(403, { error: "Forbidden" }) }), "test-token"));
  await assert.rejects(checkRegistry(registry({ "type-tune": response(404, { error: "unauthorized" }) }), "test-token"));
  await assert.rejects(checkRegistry(async () => { throw new Error("network"); }, "test-token"));
});

test("packed CLI entry and dependency versions must be intact", () => {
  validateManifest(manifest());
  const noBin = manifest(); delete noBin.bin;
  assert.throws(() => validateManifest(noBin));
  const wrong = manifest(); wrong.optionalDependencies[names[0]] = "2.0.0";
  assert.throws(() => validateManifest(wrong));
  const extra = manifest(); extra.optionalDependencies.extra = "1.0.0";
  assert.throws(() => validateManifest(extra));
});

test("validates the actual tarball and rejects a missing launcher", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "type-tune-recovery-test-"));
  try {
    const pkg = path.join(dir, "package"); fs.mkdirSync(path.join(pkg, "bin"), { recursive: true });
    fs.writeFileSync(path.join(pkg, "package.json"), JSON.stringify(manifest()));
    fs.writeFileSync(path.join(pkg, "README.md"), "example"); fs.writeFileSync(path.join(pkg, "LICENSE"), "MIT");
    fs.writeFileSync(path.join(pkg, "bin/type-tune.mjs"), "#!/usr/bin/env node\n", { mode: 0o644 });
    const tarball = path.join(dir, "package.tgz");
    const pack = () => execFileSync("tar", ["-czf", tarball, "package/LICENSE", "package/README.md", "package/bin/type-tune.mjs", "package/package.json"], { cwd: dir });
    pack(); assert.match(verifyTarball(tarball), /^[a-f0-9]{64}$/);
    const broken = manifest(); delete broken.bin; fs.writeFileSync(path.join(pkg, "package.json"), JSON.stringify(broken));
    pack(); assert.throws(() => verifyTarball(tarball), /bin/);
    fs.unlinkSync(path.join(pkg, "bin/type-tune.mjs"));
    execFileSync("tar", ["-czf", tarball, "package/LICENSE", "package/README.md", "package/package.json"], { cwd: dir });
    assert.throws(() => verifyTarball(tarball), /contents/);
  } finally { fs.rmSync(dir, { recursive: true, force: true }); }
});

test("wrong source SHA is rejected", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "type-tune-recovery-sha-"));
  try {
    execFileSync("git", ["init", "-q"], { cwd: dir });
    execFileSync("git", ["-c", "user.name=Test", "-c", "user.email=test@example.invalid", "commit", "--allow-empty", "-qm", "fixture"], { cwd: dir });
    const result = spawnSync(process.execPath, ["scripts/v1-root-recovery.mjs", "source", dir, "1.0.0"], { encoding: "utf8" });
    assert.notEqual(result.status, 0); assert.match(result.stderr, /commit/);
  } finally { fs.rmSync(dir, { recursive: true, force: true }); }
});

test("recovery workflow has dry-run, exact-tag and confirmation publication gates", () => {
  const workflow = fs.readFileSync(".github/workflows/publish-root-recovery.yml", "utf8");
  assert.match(workflow, /workflow_dispatch:/); assert.doesNotMatch(workflow, /\n  push:/);
  assert.match(workflow, /group: npm-publish-refs\/tags\/v1\.0\.0/);
  assert.match(workflow, /dry_run:[\s\S]*?default: true/);
  assert.match(workflow, /ref: refs\/tags\/v1\.0\.0/);
  assert.match(workflow, /node control\/scripts\/v1-root-recovery\.mjs source release/);
  assert.match(workflow, /if: \$\{\{ !inputs\.dry_run && inputs\.version == '1\.0\.0' && inputs\.confirmation == 'PUBLISH type-tune@1\.0\.0' \}\}/);
  assert.match(workflow, /npm publish "\$RUNNER_TEMP\/v1-root-recovery\/type-tune-1\.0\.0\.tgz" --ignore-scripts --access public --provenance/);
  assert.doesNotMatch(workflow, /npm publish.*@type-tune\//);
});
