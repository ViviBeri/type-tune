#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const binary = path.join(root, "target", "release", process.platform === "win32" ? "type-tune.exe" : "type-tune");
const fixturesRoot = path.join(root, "fixtures", "compatibility");
const fixtures = [
  { name: "alias-feature", symbol: "OrderSummary", strategy: "alias" },
  { name: "barrel-library", symbol: "Product", strategy: "barrel" },
  { name: "framework-layout", symbol: "OrderFilter", strategy: "relative" },
];

if (!fs.existsSync(binary)) {
  throw new Error(`Missing release binary at ${binary}. Run pnpm build first.`);
}

for (const fixture of fixtures) verifyFixture(fixture);
verifyBarrelReexports();
verifyAliasesWithoutBaseUrl();
process.stdout.write(
  `Compatibility fixture check passed for ${fixtures.length} projects and barrel re-exports.\n`,
);

/** Checks explicit and inherited paths against TypeScript without an implicit baseUrl. */
function verifyAliasesWithoutBaseUrl() {
  for (const inherited of [false, true]) {
    const temporaryRoot = fs.mkdtempSync(path.join(os.tmpdir(), "type-tune-path-alias-"));
    const project = path.join(temporaryRoot, "project");
    fs.cpSync(path.join(fixturesRoot, "alias-feature"), project, { recursive: true });
    try {
      const options = { jsx: "preserve", strict: true, noEmit: true, paths: { "~/*": ["./src/*"], "@/*": ["./src/*"] } };
      if (inherited) {
        fs.mkdirSync(path.join(project, "configs"));
        options.paths = { "~/*": ["../src/*"], "@/*": ["../src/*"] };
        fs.writeFileSync(path.join(project, "configs", "base.json"), JSON.stringify({ compilerOptions: options }));
      }
      fs.writeFileSync(path.join(project, "tsconfig.json"), JSON.stringify(inherited
        ? { extends: "./configs/base.json", include: ["src/**/*.ts", "src/**/*.tsx"] }
        : { compilerOptions: options, include: ["src/**/*.ts", "src/**/*.tsx"] }));
      const view = path.join(project, "src/features/orders/order-store.ts");
      fs.writeFileSync(view, fs.readFileSync(view, "utf8").replace('"@/', '"~/'));
      run(["plan", "--project", "tsconfig.json", "--out", ".type-tune/plan.json", "--markdown", ".type-tune/plan.md", "--default-status", "apply"], project);
      run(["apply", "--plan", ".type-tune/plan.json", "--markdown", ".type-tune/plan.md"], project);
      if (!fs.readFileSync(view, "utf8").includes('"~/features/orders/types/orders"')) {
        throw new Error(`Failed to preserve ~/ with inherited=${inherited}`);
      }
      if (!fs.readFileSync(path.join(project, "src/pages/orders.ts"), "utf8").includes('"@/features/orders/types/orders"')) {
        throw new Error(`Failed to preserve @/ with inherited=${inherited}`);
      }
      const tsc = spawnSync(process.execPath, [path.join(root, "node_modules/typescript/bin/tsc"), "--project", "tsconfig.json"], { cwd: project, encoding: "utf8" });
      if (tsc.status !== 0) throw new Error(tsc.stderr || tsc.stdout);
    } finally {
      fs.rmSync(temporaryRoot, { force: true, recursive: true });
    }
  }
}

/** Exercises public CLI rewrites and TypeScript resolution for mixed barrel exports. */
function verifyBarrelReexports() {
  const cases = [
    ["export type { A, B } from './schema';", "A", false],
    ["export { type A, type B } from './schema';", "A", false],
    ["export { type A, type B } from './schema';", "A", true],
    ["export type { A, B } from './schema';", "A", true],
    ["export type { A as PublicA, B } from './schema';", "PublicA", false],
    ["export type { A as PublicA, B } from './schema';", "PublicA", true],
    ["export * from './schema';", "A", false],
  ];
  for (const [barrel, imported, moveB] of cases) {
    const project = fs.mkdtempSync(path.join(os.tmpdir(), "type-tune-barrel-regression-"));
    try {
      fs.mkdirSync(path.join(project, "src", "feature"), { recursive: true });
      fs.writeFileSync(
        path.join(project, "tsconfig.json"),
        JSON.stringify({
          compilerOptions: { strict: true, noEmit: true },
          include: ["src/**/*.ts"],
        }),
      );
      fs.writeFileSync(
        path.join(project, "src", "feature", "schema.ts"),
        "export type A = { a: string };\nexport type B = { b: number };\nexport const label = 'kept';\n",
      );
      fs.writeFileSync(path.join(project, "src", "feature", "index.ts"), `${barrel}\n`);
      const consumer = `import type { ${imported}, B } from './feature';\nexport const a: ${imported} = { a: 'ok' };\nexport const b: B = { b: 1 };\n`;
      fs.writeFileSync(path.join(project, "src", "app.ts"), consumer);
      fs.writeFileSync(path.join(project, "src", "other.ts"), consumer);
      if (barrel.startsWith("export *")) {
        fs.writeFileSync(
          path.join(project, "src", "value.ts"),
          "import { label } from './feature';\nexport const value = label;\n",
        );
      }
      run(
        [
          "plan",
          "--project",
          "tsconfig.json",
          "--out",
          ".type-tune/plan.json",
          "--markdown",
          ".type-tune/plan.md",
        ],
        project,
      );
      const plan = JSON.parse(fs.readFileSync(path.join(project, ".type-tune/plan.json"), "utf8"));
      const selected = plan.entries.filter(
        ({ candidate }) => candidate.symbolName === "A" || (moveB && candidate.symbolName === "B"),
      );
      if (selected.length !== (moveB ? 2 : 1))
        throw new Error(`Missing barrel candidates: ${barrel}`);
      const markdownPath = path.join(project, ".type-tune/plan.md");
      let markdown = fs.readFileSync(markdownPath, "utf8");
      for (const { candidate } of selected)
        markdown = markdown.replace(`[ignore] ${candidate.id}`, `[apply] ${candidate.id}`);
      fs.writeFileSync(markdownPath, markdown);
      run(["apply", "--plan", ".type-tune/plan.json", "--markdown", ".type-tune/plan.md"], project);
      const tsc = spawnSync(
        process.execPath,
        [path.join(root, "node_modules/typescript/bin/tsc"), "--project", "tsconfig.json"],
        { cwd: project, encoding: "utf8" },
      );
      if (tsc.status !== 0) throw new Error(`${barrel}\n${tsc.stderr || tsc.stdout}`);
      const source = fs.readFileSync(path.join(project, "src", "feature", "schema.ts"), "utf8");
      if (source.includes("export type B =") === moveB)
        throw new Error(`Unexpected B declaration: ${barrel}`);
    } finally {
      fs.rmSync(project, { force: true, recursive: true });
    }
  }
}

/** Runs the public scan, plan, dry-run, and apply workflow against one isolated project copy. */
function verifyFixture(fixture) {
  const source = path.join(fixturesRoot, fixture.name);
  const temporaryRoot = fs.mkdtempSync(path.join(os.tmpdir(), `type-tune-${fixture.name}-`));
  const project = path.join(temporaryRoot, "project");
  fs.cpSync(source, project, { recursive: true });

  try {
    const scan = JSON.parse(run(["scan", "--project", "tsconfig.json", "--format", "json"], project));
    const candidate = scan.candidates.find((entry) => entry.symbolName === fixture.symbol);
    if (!candidate) throw new Error(`${fixture.name} did not scan ${fixture.symbol}.`);
    if (!candidate.referrerStrategies.some((entry) => entry.strategy.kind === fixture.strategy)) {
      throw new Error(`${fixture.name} did not select the expected ${fixture.strategy} import strategy.`);
    }

    run(
      [
        "plan",
        "--project",
        "tsconfig.json",
        "--out",
        ".type-tune/plan.json",
        "--markdown",
        ".type-tune/plan.md",
        "--default-status",
        "apply",
      ],
      project,
    );
    const plan = JSON.parse(fs.readFileSync(path.join(project, ".type-tune", "plan.json"), "utf8"));
    const planEntry = plan.entries.find((entry) => entry.candidate.symbolName === fixture.symbol);
    if (!planEntry || planEntry.status !== "apply") {
      throw new Error(`${fixture.name} did not create an approved plan entry for ${fixture.symbol}.`);
    }

    const beforeDryRun = sourceSnapshot(project);
    const dryRun = run(
      ["apply", "--plan", ".type-tune/plan.json", "--markdown", ".type-tune/plan.md", "--dry-run"],
      project,
    );
    if (!dryRun.includes("Dry run:")) throw new Error(`${fixture.name} did not report an apply dry run.`);
    if (sourceSnapshot(project) !== beforeDryRun) {
      throw new Error(`${fixture.name} dry run changed project source files.`);
    }

    const apply = run(["apply", "--plan", ".type-tune/plan.json", "--markdown", ".type-tune/plan.md"], project);
    if (!apply.includes("Applied")) throw new Error(`${fixture.name} did not report a successful apply.`);
    if (!fs.existsSync(path.join(project, planEntry.candidate.destinationPath))) {
      throw new Error(`${fixture.name} did not create ${planEntry.candidate.destinationPath}.`);
    }
    const sourceText = fs.readFileSync(path.join(project, planEntry.candidate.sourcePath), "utf8");
    if (!sourceText.includes(`export type { ${fixture.symbol} }`)) {
      throw new Error(`${fixture.name} did not retain a compatibility re-export for ${fixture.symbol}.`);
    }
  } finally {
    fs.rmSync(temporaryRoot, { force: true, recursive: true });
  }
}

/** Returns a stable snapshot of source files so dry-run behavior is checked as a black-box contract. */
function sourceSnapshot(project) {
  const source = path.join(project, "src");
  const files = [];
  visit(source, files);
  return files
    .sort((left, right) => left.localeCompare(right, "en"))
    .map((file) => `${path.relative(project, file).split(path.sep).join("/")}\0${fs.readFileSync(file, "utf8")}`)
    .join("\0");
}

/** Recursively records regular files below one fixture source directory. */
function visit(directory, files) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) visit(entryPath, files);
    else if (entry.isFile()) files.push(entryPath);
  }
}

/** Runs the release binary and returns stdout, surfacing actionable diagnostics on failure. */
function run(args, cwd) {
  const result = spawnSync(binary, args, { cwd, encoding: "utf8" });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(result.stderr || result.stdout || `${path.basename(binary)} ${args[0]} failed.`);
  }
  return result.stdout;
}
