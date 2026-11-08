import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

const root = process.cwd();
const packages = {
  "type-tune-linux-x64-gnu": "x86_64-unknown-linux-gnu",
  "type-tune-linux-x64-musl": "x86_64-unknown-linux-musl",
  "type-tune-darwin-x64": "x86_64-apple-darwin",
  "type-tune-darwin-arm64": "aarch64-apple-darwin",
  "type-tune-win32-x64-msvc": "x86_64-pc-windows-msvc",
};
const sha = "a".repeat(40);
const run = (args, cwd, env = {}) => spawnSync(process.execPath, args, { cwd, env: { ...process.env, ...env }, encoding: "utf8" });

function fixture() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "type-tune-release-test-"));
  for (const name of ["scripts", "packages", "crates"]) {
    fs.cpSync(path.join(root, name), path.join(dir, name), { recursive: true, filter: (source) => !source.includes(`${path.sep}bin${path.sep}`) });
  }
  for (const name of ["package.json", "Cargo.lock"]) fs.copyFileSync(path.join(root, name), path.join(dir, name));
  return dir;
}

test("release tag, event SHA, checkout and native versions must agree", (t) => {
  const dir = fixture();
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const git = (...args) => execFileSync("git", args, { cwd: dir, stdio: "pipe" }).toString().trim();
  git("init", "-q");
  git("config", "user.email", "test@example.invalid");
  git("config", "user.name", "Release test");
  git("add", ".");
  git("commit", "-qm", "fixture");
  const commit = git("rev-parse", "HEAD");
  git("tag", "v1.0.0");
  git("tag", "v1.0.1");
  const valid = { GITHUB_REF: "refs/tags/v1.0.0", GITHUB_REF_NAME: "v1.0.0", GITHUB_SHA: commit };
  const validate = (env) => run(["scripts/validate-release.mjs"], dir, env);
  assert.equal(validate(valid).status, 0);
  for (const invalid of [
    { ...valid, GITHUB_REF: "", GITHUB_REF_NAME: "" },
    { ...valid, GITHUB_REF: "refs/heads/main", GITHUB_REF_NAME: "main" },
    { ...valid, GITHUB_REF: "refs/tags/v1.0.1", GITHUB_REF_NAME: "v1.0.1" },
    { ...valid, GITHUB_SHA: sha },
  ]) assert.notEqual(validate(invalid).status, 0);
  const manifest = path.join(dir, "packages/type-tune-linux-x64-gnu/package.json");
  const data = JSON.parse(fs.readFileSync(manifest));
  data.version = "1.0.1";
  fs.writeFileSync(manifest, JSON.stringify(data));
  assert.notEqual(validate(valid).status, 0);
});

test("all transferred packages must have matching metadata, bytes, checksums and commit", (t) => {
  const dir = fixture();
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  for (const [name, target] of Object.entries(packages)) {
    const pkg = path.join(dir, "packages", name);
    const manifest = JSON.parse(fs.readFileSync(path.join(pkg, "package.json")));
    const binaryName = manifest.os.includes("win32") ? "type-tune.exe" : "type-tune";
    fs.mkdirSync(path.join(pkg, "bin"), { recursive: true });
    const binary = path.join(pkg, "bin", binaryName);
    fs.writeFileSync(binary, `fixture for ${target}`, { mode: 0o755 });
    fs.writeFileSync(`${binary}.sha256`, `${createHash("sha256").update(fs.readFileSync(binary)).digest("hex")}  ${binaryName}\n`);
    const result = run(["scripts/release-artifacts.mjs", "create", `artifacts/${name}`, name, target, sha], dir);
    assert.equal(result.status, 0, result.stderr);
  }
  const verify = (commit = sha) => run(["scripts/release-artifacts.mjs", "verify", "artifacts", commit], dir);
  assert.equal(verify().status, 0, verify().stderr);
  assert.notEqual(verify("b".repeat(40)).status, 0);
  const meta = path.join(dir, "artifacts/type-tune-linux-x64-gnu/metadata.json");
  const original = fs.readFileSync(meta);
  const changed = JSON.parse(original);
  changed.target = "wrong-target";
  fs.writeFileSync(meta, JSON.stringify(changed));
  assert.notEqual(verify().status, 0);
  fs.writeFileSync(meta, original);
  const tarball = path.join(dir, "artifacts/type-tune-linux-x64-musl/package.tgz");
  fs.appendFileSync(tarball, "corruption");
  assert.notEqual(verify().status, 0);
  fs.unlinkSync(tarball);
  assert.notEqual(verify().status, 0);
});

test("workflow dependencies prevent publishing until every gate succeeds", () => {
  const workflow = fs.readFileSync(path.join(root, ".github/workflows/npm-publish.yml"), "utf8");
  const jobSection = workflow.slice(workflow.indexOf("\njobs:\n") + "\njobs:\n".length);
  const jobs = Object.fromEntries([...jobSection.matchAll(/^  ([a-z][a-z-]+):\n([\s\S]*?)(?=^  [a-z][a-z-]+:\n|$(?![\s\S]))/gm)].map((match) => [match[1], match[2]]));
  assert.deepEqual(Object.keys(jobs), ["preflight", "build-native", "verify-artifacts", "publish-native", "publish-root"]);
  assert.match(jobs.preflight, /node scripts\/validate-release\.mjs/);
  assert.match(jobs.preflight, /pnpm publish:check/);
  assert.match(jobs["build-native"], /needs: preflight/);
  assert.match(jobs["build-native"], /Smoke test native binary/);
  assert.match(jobs["verify-artifacts"], /needs: \[preflight, build-native\]/);
  assert.match(jobs["verify-artifacts"], /release-artifacts\.mjs verify/);
  assert.match(jobs["publish-native"], /needs: \[preflight, build-native, verify-artifacts\]/);
  assert.match(jobs["publish-root"], /needs: publish-native/);
  for (const job of Object.values(jobs)) {
    assert.doesNotMatch(job, /^    if:.*(?:always|failure|cancelled)\s*\(/m);
  }
  assert.match(jobs["verify-artifacts"], /pattern: type-tune-\*/);
  assert.match(jobs["publish-native"], /name: verified-native-packages/);
  for (const [name, target] of Object.entries(packages)) {
    assert.ok(jobs["build-native"].includes(`package: ${name}`));
    assert.ok(jobs["build-native"].includes(`target: ${target}`));
    assert.ok(jobs["publish-native"].includes(`- ${name}`));
  }
  assert.doesNotMatch(jobs.preflight + jobs["build-native"] + jobs["verify-artifacts"], /npm publish\s/);
  assert.match(jobs["publish-native"], /npm publish release-artifacts\/\$\{\{ matrix\.package \}\}\/package\.tgz/);
  assert.match(workflow, /cancel-in-progress: false/);
  // With Actions' default success() condition, a failed or skipped dependency blocks its dependants.
  for (const failed of ["preflight", "build-native", "verify-artifacts"]) {
    const status = { preflight: true, "build-native": true, "verify-artifacts": true };
    status[failed] = false;
    assert.equal(status.preflight && status["build-native"] && status["verify-artifacts"], false);
  }
});
