import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { npmInvocation } from "./npm-invocation.mjs";

test("invokes npm without a shell on POSIX and uses Node for Windows npm-cli.js", () => {
  assert.deepEqual(npmInvocation({ platform: "linux" }), { command: "npm", args: [] });
  const node = "C:\\Program Files\\nodejs\\node.exe";
  const cli = "C:\\Program Files\\nodejs\\node_modules\\npm\\bin\\npm-cli.js";
  assert.deepEqual(npmInvocation({ platform: "win32", node, exists: (p) => p === cli }), {
    command: node, args: [cli],
  });
  assert.throws(() => npmInvocation({ platform: "win32", node, exists: () => false }), /npm-cli\.js/);
});

test("packing a checkout without dev dependencies does not execute prepare", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "type-tune-no-hooks-"));
  try {
    for (const name of ["package.json", "README.md", "LICENSE", "bin"]) {
      fs.cpSync(path.resolve(name), path.join(root, name), { recursive: true });
    }
    const manifest = JSON.parse(fs.readFileSync(path.join(root, "package.json")));
    assert.equal(manifest.scripts?.prepare, undefined);
    const { command, args } = npmInvocation();
    const packed = JSON.parse(execFileSync(command, [...args, "pack", "--dry-run", "--json", "--ignore-scripts"], { cwd: root, encoding: "utf8" }));
    const files = packed[0].files.map((file) => file.path);
    assert.ok(files.includes("bin/type-tune.mjs"));
  } finally { fs.rmSync(root, { recursive: true, force: true }); }
});

test("native workflows install the same toolchain and target that cargo uses", () => {
  for (const file of ["native-packages.yml", "npm-publish.yml"]) {
    const workflow = fs.readFileSync(`.github/workflows/${file}`, "utf8");
    assert.match(workflow, /toolchain: \$\{\{ steps\.rust-version\.outputs\.version \}\}/);
    assert.match(workflow, /targets: \$\{\{ matrix\.target \}\}/);
    assert.match(workflow, /node scripts\/ci-rust-toolchain\.mjs/);
    assert.match(workflow, /node-version: "24"/);
  }
});


test("CI toolchain output matches the pinned project channel", () => {
  const output = path.join(fs.mkdtempSync(path.join(os.tmpdir(), "type-tune-rust-version-")), "output");
  try {
    execFileSync(process.execPath, ["scripts/ci-rust-toolchain.mjs"], { env: { ...process.env, GITHUB_OUTPUT: output } });
    const channel = fs.readFileSync("rust-toolchain.toml", "utf8").match(/^channel\s*=\s*"([^"]+)"/m)?.[1];
    assert.equal(fs.readFileSync(output, "utf8"), `version=${channel}\n`);
  } finally { fs.rmSync(path.dirname(output), { recursive: true, force: true }); }
});
