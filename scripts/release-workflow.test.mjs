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

function nextPatch(version) {
  const match = String(version).match(/^(\d+)\.(\d+)\.(\d+)$/);
  if (!match) throw new Error(`Expected a numeric major.minor.patch version, received ${version}.`);
  return `${match[1]}.${match[2]}.${Number(match[3]) + 1}`;
}

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
  const version = JSON.parse(fs.readFileSync(path.join(dir, "package.json"), "utf8")).version;
  const other = nextPatch(version);
  git("tag", `v${version}`);
  git("tag", `v${other}`);
  const valid = { GITHUB_REF: `refs/tags/v${version}`, GITHUB_REF_NAME: `v${version}`, GITHUB_SHA: commit };
  const validate = (env) => run(["scripts/validate-release.mjs"], dir, env);
  assert.equal(validate(valid).status, 0);
  for (const invalid of [
    { ...valid, GITHUB_REF: "", GITHUB_REF_NAME: "" },
    { ...valid, GITHUB_REF: "refs/heads/main", GITHUB_REF_NAME: "main" },
    { ...valid, GITHUB_REF: `refs/tags/v${other}`, GITHUB_REF_NAME: `v${other}` },
    { ...valid, GITHUB_SHA: sha },
  ]) assert.notEqual(validate(invalid).status, 0);
  const manifest = path.join(dir, "packages/type-tune-linux-x64-gnu/package.json");
  const data = JSON.parse(fs.readFileSync(manifest));
  data.version = other;
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
  assert.match(jobs["publish-native"], /release-artifacts\.mjs verify/);
  assert.match(jobs["publish-root"], /npm publish --ignore-scripts --access public --provenance/);
  assert.match(jobs["publish-native"], /npm publish release-artifacts\/\$\{\{ matrix\.package \}\}\/package\.tgz --access public --provenance/);
  assert.match(workflow, /^permissions:\n {2}contents: read\n {2}id-token: write\n/m);
  assert.match(jobs["build-native"], /Stage and verify platform package/);
  assert.match(jobs["build-native"], /node scripts\/check-native-package\.mjs/);
  assert.match(jobs["build-native"], /matrix\.target != 'x86_64-unknown-linux-musl'/);
  for (const name of ["publish-native", "publish-root"]) {
    const job = jobs[name];
    assert.match(job, /runs-on: ubuntu-latest/);
    assert.match(job, /environment: npm/);
    assert.match(job, /contents: read/);
    assert.match(job, /id-token: write/);
    assert.match(job, /uses: actions\/setup-node@v5/);
    assert.match(job, /node-version: "24"/);
    assert.match(job, /package-manager-cache: false/);
    assert.match(job, /^ {8}run: node scripts\/require-npm-version\.mjs$/m);
    assert.doesNotMatch(job, /NPM_TOKEN|NODE_AUTH_TOKEN|secrets\./);
    assert.doesNotMatch(job, /^\s+registry-url:/m);
  }
  assert.doesNotMatch(workflow, /NPM_TOKEN|NODE_AUTH_TOKEN/);
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
  assert.match(workflow, /cancel-in-progress: false/);
  // With Actions' default success() condition, a failed or skipped dependency blocks its dependants.
  for (const failed of ["preflight", "build-native", "verify-artifacts"]) {
    const status = { preflight: true, "build-native": true, "verify-artifacts": true };
    status[failed] = false;
    assert.equal(status.preflight && status["build-native"] && status["verify-artifacts"], false);
  }
});

test("trusted publishing rejects npm CLI versions before 11.5.1", () => {
  const check = (version) => run(["scripts/require-npm-version.mjs", version], root);
  for (const version of ["11.5.1", "11.5.2", "11.6.0", "12.0.0"]) {
    const result = check(version);
    assert.equal(result.status, 0, result.stderr);
  }
  for (const version of ["11.5.0", "11.4.9", "10.9.2", "11.5", "11.5.1-canary", "not-a-version"]) {
    const result = check(version);
    assert.notEqual(result.status, 0, version);
    assert.match(result.stderr, /npm >= 11\.5\.1/);
  }
});

test("published package metadata stays synchronized for trusted publishing", () => {
  const rootManifest = JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8"));
  assert.equal(rootManifest.name, "type-tune");
  assert.equal(rootManifest.repository.type, "git");
  assert.equal(rootManifest.repository.url, "git+https://github.com/necoz-oss/type-tune.git");
  const platforms = {
    "type-tune-darwin-arm64": { name: "@type-tune/darwin-arm64", os: ["darwin"], cpu: ["arm64"], binary: "type-tune" },
    "type-tune-darwin-x64": { name: "@type-tune/darwin-x64", os: ["darwin"], cpu: ["x64"], binary: "type-tune" },
    "type-tune-linux-x64-gnu": { name: "@type-tune/linux-x64-gnu", os: ["linux"], cpu: ["x64"], libc: ["glibc"], binary: "type-tune" },
    "type-tune-linux-x64-musl": { name: "@type-tune/linux-x64-musl", os: ["linux"], cpu: ["x64"], libc: ["musl"], binary: "type-tune" },
    "type-tune-win32-x64-msvc": { name: "@type-tune/win32-x64-msvc", os: ["win32"], cpu: ["x64"], binary: "type-tune.exe" },
  };
  assert.deepEqual(Object.keys(rootManifest.optionalDependencies).sort(), Object.values(platforms).map((platform) => platform.name).sort());
  for (const [directory, platform] of Object.entries(platforms)) {
    const manifest = JSON.parse(fs.readFileSync(path.join(root, "packages", directory, "package.json"), "utf8"));
    assert.equal(manifest.name, platform.name);
    assert.equal(manifest.version, rootManifest.version);
    assert.equal(rootManifest.optionalDependencies[platform.name], rootManifest.version);
    assert.equal(manifest.repository, "https://github.com/necoz-oss/type-tune");
    assert.deepEqual(manifest.os, platform.os);
    assert.deepEqual(manifest.cpu, platform.cpu);
    assert.deepEqual(manifest.libc, platform.libc);
    assert.deepEqual(manifest.files, [`bin/${platform.binary}`, `bin/${platform.binary}.sha256`]);
  }
});

test("version bump installs mise before publish:check", () => {
  const workflow = fs.readFileSync(path.join(root, ".github/workflows/version-bump.yaml"), "utf8");
  const jobs = workflowJobs(workflow);
  assert.deepEqual(Object.keys(jobs), ["bump-version"]);
  const job = jobs["bump-version"];
  const mise = job.indexOf("uses: jdx/mise-action@v2");
  const installRust = job.indexOf("mise install rust@stable");
  const releaseCheck = job.indexOf("pnpm publish:check");
  assert.ok(mise !== -1 && installRust !== -1 && releaseCheck !== -1);
  assert.ok(mise < installRust && installRust < releaseCheck);
  assert.match(job.slice(mise, installRust), /install: false/);
  assert.match(job, /uses: dtolnay\/rust-toolchain@stable/);
  assert.match(job, /pnpm version \$\{\{ inputs\.release_type \}\} --no-git-tag-version/);
  assert.match(job, /node scripts\/sync-native-versions\.mjs/);
  assert.match(job, /pnpm install --lockfile-only/);
  assert.match(job, /cargo fmt --all/);
  assert.match(job, /git switch -c "release\/\$\{\{ env\.new_version \}\}"/);
  assert.match(job, /chore\(release\): bump version to \$\{\{ env\.new_version \}\}/);
  assert.match(job, /gh pr create/);
});

test("every publish:check job installs mise before the check", () => {
  const workflowDir = path.join(root, ".github/workflows");
  const checked = [];
  for (const file of fs.readdirSync(workflowDir).sort()) {
    const workflow = fs.readFileSync(path.join(workflowDir, file), "utf8");
    for (const [name, job] of Object.entries(workflowJobs(workflow))) {
      const releaseCheck = job.indexOf("pnpm publish:check");
      if (releaseCheck === -1) continue;
      checked.push(`${file}:${name}`);
      const mise = job.indexOf("uses: jdx/mise-action@v2");
      const installRust = job.indexOf("mise install rust@stable");
      assert.ok(mise !== -1 && mise < releaseCheck, `${file} job ${name} runs publish:check without mise`);
      assert.ok(installRust !== -1 && installRust < releaseCheck, `${file} job ${name} runs publish:check before mise install rust@stable`);
      assert.match(job.slice(mise, installRust), /install: false/);
    }
  }
  assert.deepEqual(checked, [
    "npm-publish.yml:preflight",
    "publish-check.yml:publish-check",
    "version-bump.yaml:bump-version",
  ]);
});

function workflowJobs(workflow) {
  const marker = "\njobs:\n";
  const start = workflow.indexOf(marker);
  assert.notEqual(start, -1);
  const jobSection = workflow.slice(start + marker.length);
  return Object.fromEntries([...jobSection.matchAll(/^  ([a-z][a-z0-9-]+):\n([\s\S]*?)(?=^  [a-z][a-z0-9-]+:\n|$(?![\s\S]))/gm)].map((match) => [match[1], match[2]]));
}
