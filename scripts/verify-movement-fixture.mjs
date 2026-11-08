import assert from "node:assert/strict";
import { access, cp, mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));
const fixturesRoot = join(repositoryRoot, "fixtures");
const binary = join(repositoryRoot, "target", "release", "type-tune");
const typescriptBin = join(repositoryRoot, "node_modules", ".bin");
const fixtureFilter = process.env.TYPE_TUNE_FIXTURE_FILTER;

function runResult(projectRoot, command, args) {
  return spawnSync(command, args, {
    cwd: projectRoot,
    encoding: "utf8",
    env: {
      ...process.env,
      PATH: `${typescriptBin}:${process.env.PATH ?? ""}`
    }
  });
}

function run(projectRoot, command, args) {
  const result = runResult(projectRoot, command, args);
  assert.equal(
    result.status,
    0,
    `${command} ${args.join(" ")} failed:\n${result.stderr || result.stdout}`
  );
}

function runFailure(projectRoot, command, args) {
  const result = runResult(projectRoot, command, args);
  assert.notEqual(result.status, 0, `${command} ${args.join(" ")} unexpectedly succeeded`);
  return result;
}

async function verifyFixture(name, test) {
  if (fixtureFilter && fixtureFilter !== name) {
    return;
  }
  const projectRoot = await mkdtemp(join(tmpdir(), `type-tune-${name}-`));
  try {
    await cp(join(fixturesRoot, name), projectRoot, { recursive: true });
    await test(projectRoot);
  } finally {
    await rm(projectRoot, { recursive: true, force: true });
  }
}

async function fileExists(path) {
  try {
    await stat(path);
    return true;
  } catch (error) {
    if (error.code === "ENOENT") {
      return false;
    }
    throw error;
  }
}

await access(binary);

await verifyFixture("cli-movement-project", async (projectRoot) => {
  run(projectRoot, binary, [
    "extract",
    "OrderSummary",
    "--from",
    "src/features/order/schema.ts",
    "--to",
    "src/features/order/types/order.ts",
    "--project",
    "tsconfig.json"
  ]);
  run(projectRoot, "tsc", ["--noEmit", "--project", "tsconfig.json"]);

  assert.equal(
    await readFile(join(projectRoot, "src/features/order/types/order.ts"), "utf8"),
    "type Money = { cents: number };\n\nexport type OrderSummary = {\n  id: string;\n  total: Money;\n};\n"
  );
  assert.equal(
    await readFile(join(projectRoot, "src/features/order/schema.ts"), "utf8"),
    "export type { OrderSummary } from './types/order';\n"
  );
  assert.equal(
    await readFile(join(projectRoot, "src/features/order/index.ts"), "utf8"),
    "export type { OrderSummary } from \"./types/order\";\n"
  );
  assert.equal(
    await readFile(join(projectRoot, "src/pages/order-page.ts"), "utf8"),
    "import type { OrderSummary } from \"@/features/order\";\n\nexport const selected: OrderSummary = {\n  id: \"order-2\",\n  total: { cents: 3000 }\n};\n"
  );

});

await verifyFixture("cli-js-specifier-project", async (projectRoot) => {
  run(projectRoot, binary, [
    "extract",
    "Shared",
    "--from",
    "src/shared.ts",
    "--project",
    "tsconfig.json"
  ]);
  run(projectRoot, "tsc", ["--noEmit", "--project", "tsconfig.json"]);

  assert.equal(
    await readFile(join(projectRoot, "src/app.ts"), "utf8"),
    "import type { Shared } from \"./types/src.js\";\n\nexport const current: Shared = { id: \"current\" };\n"
  );
});

await verifyFixture("cli-plan-apply-project", async (projectRoot) => {
  const source = join(projectRoot, "src/features/catalog/schema.ts");
  const destination = join(projectRoot, "src/features/catalog/types/catalog.ts");
  const originalSource = await readFile(source, "utf8");

  run(projectRoot, binary, [
    "plan",
    "--project",
    "tsconfig.json",
    "--out",
    ".type-tune/plan.json",
    "--markdown",
    ".type-tune/plan.md",
    "--min-referrers",
    "1",
    "--default-status",
    "apply"
  ]);
  run(projectRoot, binary, [
    "apply",
    "--plan",
    ".type-tune/plan.json",
    "--markdown",
    ".type-tune/plan.md",
    "--dry-run"
  ]);
  assert.equal(await readFile(source, "utf8"), originalSource);
  assert.equal(await fileExists(destination), false);

  run(projectRoot, binary, [
    "apply",
    "--plan",
    ".type-tune/plan.json",
    "--markdown",
    ".type-tune/plan.md"
  ]);
  run(projectRoot, "tsc", ["--noEmit", "--project", "tsconfig.json"]);

  assert.equal(
    await readFile(destination, "utf8"),
    "export interface CatalogItem {\n  sku: string;\n}\n"
  );
  assert.equal(
    await readFile(source, "utf8"),
    "export type { CatalogItem } from './types/catalog'\n"
  );
  assert.equal(
    await readFile(join(projectRoot, "src/pages/catalog-page.ts"), "utf8"),
    "import type { CatalogItem } from \"../features/catalog/types/catalog\";\n\nexport const featured: CatalogItem = { sku: \"featured\" };\n"
  );
});

await verifyFixture("cli-retained-value-project", async (projectRoot) => {
  run(projectRoot, binary, [
    "extract",
    "Shared",
    "--from",
    "src/shared.ts",
    "--project",
    "tsconfig.json"
  ]);
  run(projectRoot, "tsc", ["--noEmit", "--project", "tsconfig.json"]);

  assert.equal(
    await readFile(join(projectRoot, "src/shared.ts"), "utf8"),
    "import type { Shared } from './types/src';\n\nexport const defaultShared: Shared = { id: \"default\" };\n\nexport type { Shared } from './types/src';\n"
  );
  assert.equal(
    await readFile(join(projectRoot, "src/app.ts"), "utf8"),
    "import type { Shared } from \"./types/src\";\n\nexport const current: Shared = { id: \"current\" };\n"
  );
});

await verifyFixture("cli-validation-rollback-project", async (projectRoot) => {
  const source = join(projectRoot, "src/shared.ts");
  const originalSource = await readFile(source, "utf8");
  const result = runFailure(projectRoot, binary, [
    "extract",
    "Shared",
    "--from",
    "src/shared.ts",
    "--project",
    "tsconfig.json"
  ]);

  assert.match(result.stderr, /Compile validation failed after extract; rolled back file changes\./);
  assert.equal(await readFile(source, "utf8"), originalSource);
  assert.equal(await fileExists(join(projectRoot, "src/types/src.ts")), false);
});

await verifyFixture("cli-mixed-import-project", async (projectRoot) => {
  run(projectRoot, binary, [
    "extract",
    "Shared",
    "--from",
    "src/shared.ts",
    "--project",
    "tsconfig.json"
  ]);
  run(projectRoot, "tsc", ["--noEmit", "--project", "tsconfig.json"]);

  assert.equal(
    await readFile(join(projectRoot, "src/shared.ts"), "utf8"),
    "export const label = \"shared\";\n\nexport type { Shared } from './types/src';\n"
  );
  assert.equal(
    await readFile(join(projectRoot, "src/app.ts"), "utf8"),
    "import { label } from \"./shared\";\nimport type { Shared } from \"./types/src\";\n\nexport const current: Shared = { id: label };\n"
  );
});

await verifyFixture("cli-repo-types-project", async (projectRoot) => {
  run(projectRoot, binary, [
    "extract",
    "OrderSummary",
    "--from",
    "src/features/order/schema.ts",
    "--placement-style",
    "repo-types",
    "--project",
    "tsconfig.json"
  ]);
  run(projectRoot, "tsc", ["--noEmit", "--project", "tsconfig.json"]);

  assert.equal(
    await readFile(
      join(projectRoot, "types/src/features/order/types/order.ts"),
      "utf8"
    ),
    "export interface OrderSummary {\n  id: string;\n}\n"
  );
  assert.equal(
    await readFile(join(projectRoot, "src/pages/order-page.ts"), "utf8"),
    "import type { OrderSummary } from \"../../types/src/features/order/types/order\";\n\nexport const selected: OrderSummary = { id: \"order-2\" };\n"
  );
});

await verifyFixture("cli-namespace-dependency-project", async (projectRoot) => {
  run(projectRoot, binary, [
    "extract",
    "Shared",
    "--from",
    "src/features/schema.ts",
    "--project",
    "tsconfig.json"
  ]);
  run(projectRoot, "tsc", ["--noEmit", "--project", "tsconfig.json"]);

  assert.equal(
    await readFile(join(projectRoot, "src/features/types/features.ts"), "utf8"),
    "import type * as External from \"../../external\";\nexport type Shared = External.Metadata;\n"
  );
  assert.equal(
    await readFile(join(projectRoot, "src/app.ts"), "utf8"),
    "import type { Shared } from \"./features/types/features\";\n\nexport const current: Shared = { id: \"current\" };\n"
  );
});

await verifyFixture("cli-wildcard-barrel-project", async (projectRoot) => {
  run(projectRoot, binary, [
    "extract",
    "Shared",
    "--from",
    "src/features/schema.ts",
    "--project",
    "tsconfig.json"
  ]);
  run(projectRoot, "tsc", ["--noEmit", "--project", "tsconfig.json"]);

  assert.equal(
    await readFile(join(projectRoot, "src/features/index.ts"), "utf8"),
    "export * from \"./types/features\";\n"
  );
  assert.equal(
    await readFile(join(projectRoot, "src/app.ts"), "utf8"),
    "import type { Shared } from \"./features\";\n\nexport const current: Shared = { id: \"current\" };\n"
  );
});

await verifyFixture("cli-default-import-project", async (projectRoot) => {
  const source = join(projectRoot, "src/features/schema.ts");
  const destination = join(projectRoot, "src/features/types/features.ts");
  const originalSource = await readFile(source, "utf8");

  run(projectRoot, binary, [
    "extract",
    "Shared",
    "--from",
    "src/features/schema.ts",
    "--project",
    "tsconfig.json",
    "--dry-run"
  ]);
  assert.equal(await readFile(source, "utf8"), originalSource);
  assert.equal(await fileExists(destination), false);

  run(projectRoot, binary, [
    "extract",
    "Shared",
    "--from",
    "src/features/schema.ts",
    "--project",
    "tsconfig.json"
  ]);
  run(projectRoot, "tsc", ["--noEmit", "--project", "tsconfig.json"]);

  assert.equal(
    await readFile(destination, "utf8"),
    "import type External from \"../../external\";\nexport type Shared = External;\n"
  );
});

await verifyFixture("cli-stale-plan-project", async (projectRoot) => {
  const source = join(projectRoot, "src/features/schema.ts");
  const destination = join(projectRoot, "src/features/types/features.ts");

  run(projectRoot, binary, [
    "plan",
    "--project",
    "tsconfig.json",
    "--out",
    ".type-tune/plan.json",
    "--markdown",
    ".type-tune/plan.md",
    "--min-referrers",
    "1",
    "--default-status",
    "apply"
  ]);
  await writeFile(source, "export type Shared = { id: string; updated: boolean };\n");

  const result = runFailure(projectRoot, binary, [
    "apply",
    "--plan",
    ".type-tune/plan.json",
    "--markdown",
    ".type-tune/plan.md"
  ]);
  assert.match(result.stderr, /Stale plan entry/);
  assert.equal(
    await readFile(source, "utf8"),
    "export type Shared = { id: string; updated: boolean };\n"
  );
  assert.equal(await fileExists(destination), false);
});

await verifyFixture("cli-generated-scan-project", async (projectRoot) => {
  const scan = runResult(projectRoot, binary, [
    "scan",
    "--project",
    "tsconfig.json",
    "--format",
    "json",
    "--min-referrers",
    "1"
  ]);
  assert.equal(scan.status, 0, scan.stderr);
  assert.deepEqual(
    JSON.parse(scan.stdout).candidates.map((candidate) => candidate.symbolName),
    ["Shared"]
  );

  const includingGenerated = runResult(projectRoot, binary, [
    "scan",
    "--project",
    "tsconfig.json",
    "--format",
    "json",
    "--min-referrers",
    "1",
    "--include-generated"
  ]);
  assert.equal(includingGenerated.status, 0, includingGenerated.stderr);
  assert.deepEqual(
    JSON.parse(includingGenerated.stdout).candidates.map((candidate) => candidate.symbolName),
    ["Generated", "Shared"]
  );
});

await verifyFixture("cli-tsx-project", async (projectRoot) => {
  run(projectRoot, binary, [
    "extract",
    "CardProps",
    "--from",
    "src/features/card.tsx",
    "--project",
    "tsconfig.json"
  ]);
  run(projectRoot, "tsc", ["--noEmit", "--project", "tsconfig.json"]);

  assert.equal(
    await readFile(join(projectRoot, "src/features/types/features.ts"), "utf8"),
    "export interface CardProps {\n  title: string;\n}\n"
  );
  assert.equal(
    await readFile(join(projectRoot, "src/app.ts"), "utf8"),
    "import type { CardProps } from \"./features/types/features\";\n\nexport const current: CardProps = { title: \"current\" };\n"
  );
});

await verifyFixture("cli-extends-project", async (projectRoot) => {
  run(projectRoot, binary, [
    "extract",
    "Shared",
    "--from",
    "src/shared.ts",
    "--project",
    "tsconfig.json"
  ]);
  run(projectRoot, "tsc", ["--noEmit", "--project", "tsconfig.json"]);

  assert.equal(
    await readFile(join(projectRoot, "src/types/src.ts"), "utf8"),
    "export type Shared = {\n  id: string;\n};\n"
  );
});

await verifyFixture("cli-module-suffix-project", async (projectRoot) => {
  run(projectRoot, binary, [
    "extract",
    "PlatformInfo",
    "--from",
    "src/shared.native.ts",
    "--project",
    "tsconfig.json"
  ]);
  run(projectRoot, "tsc", ["--noEmit", "--project", "tsconfig.json"]);

  assert.equal(
    await readFile(join(projectRoot, "src/types/src.ts"), "utf8"),
    "export type PlatformInfo = {\n  name: string;\n};\n"
  );
  assert.equal(
    await readFile(join(projectRoot, "src/app.ts"), "utf8"),
    "import type { PlatformInfo } from \"./types/src\";\n\nexport const platform: PlatformInfo = { name: \"native\" };\n"
  );
});

await verifyFixture("cli-root-dirs-project", async (projectRoot) => {
  run(projectRoot, binary, [
    "extract",
    "VirtualShared",
    "--from",
    "src/shared.ts",
    "--project",
    "tsconfig.json"
  ]);
  run(projectRoot, "tsc", ["--noEmit", "--project", "tsconfig.json"]);

  assert.equal(
    await readFile(join(projectRoot, "src/types/src.ts"), "utf8"),
    "export type VirtualShared = {\n  id: string;\n};\n"
  );
  assert.equal(
    await readFile(join(projectRoot, "virtual/app.ts"), "utf8"),
    "import type { VirtualShared } from \"../src/types/src\";\n\nexport const current: VirtualShared = { id: \"virtual\" };\n"
  );
});

await verifyFixture("cli-imported-source-project", async (projectRoot) => {
  run(projectRoot, binary, [
    "extract",
    "ImportedOnly",
    "--from",
    "src/shared.ts",
    "--project",
    "tsconfig.json"
  ]);
  run(projectRoot, "tsc", ["--noEmit", "--project", "tsconfig.json"]);

  assert.equal(
    await readFile(join(projectRoot, "src/types/src.ts"), "utf8"),
    "export type ImportedOnly = {\n  id: string;\n};\n"
  );
  assert.equal(
    await readFile(join(projectRoot, "src/app.ts"), "utf8"),
    "import type { ImportedOnly } from \"./types/src\";\n\nexport const current: ImportedOnly = { id: \"current\" };\n"
  );
});

await verifyFixture("cli-package-imports-project", async (projectRoot) => {
  run(projectRoot, binary, [
    "extract",
    "PackageModel",
    "--from",
    "src/model.ts",
    "--project",
    "tsconfig.json"
  ]);
  run(projectRoot, "tsc", ["--noEmit", "--project", "tsconfig.json"]);

  assert.equal(
    await readFile(join(projectRoot, "src/types/src.ts"), "utf8"),
    "export type PackageModel = {\n  id: string;\n};\n"
  );
  assert.equal(
    await readFile(join(projectRoot, "src/app.ts"), "utf8"),
    "import type { PackageModel } from \"./types/src.js\";\n\nexport const current: PackageModel = { id: \"current\" };\n"
  );
  assert.equal(
    await readFile(join(projectRoot, "src/model.ts"), "utf8"),
    "export type { PackageModel } from './types/src.js';\n"
  );
});

await verifyFixture("cli-package-imports-plan-project", async (projectRoot) => {
  run(projectRoot, binary, [
    "plan",
    "--project",
    "tsconfig.json",
    "--out",
    ".type-tune/plan.json",
    "--markdown",
    ".type-tune/plan.md",
    "--min-referrers",
    "1",
    "--default-status",
    "apply"
  ]);
  run(projectRoot, binary, [
    "apply",
    "--plan",
    ".type-tune/plan.json",
    "--markdown",
    ".type-tune/plan.md"
  ]);
  run(projectRoot, "tsc", ["--noEmit", "--project", "tsconfig.json"]);

  assert.equal(
    await readFile(join(projectRoot, "src/types/src.ts"), "utf8"),
    "export type PlannedModel = {\n  id: string;\n};\n"
  );
  assert.equal(
    await readFile(join(projectRoot, "src/app.ts"), "utf8"),
    "import type { PlannedModel } from \"./types/src.js\";\n\nexport const current: PlannedModel = { id: \"planned\" };\n"
  );
});

await verifyFixture("cli-renamed-barrel-project", async (projectRoot) => {
  run(projectRoot, binary, [
    "extract",
    "InternalModel",
    "--from",
    "src/shared.ts",
    "--project",
    "tsconfig.json"
  ]);
  run(projectRoot, "tsc", ["--noEmit", "--project", "tsconfig.json"]);

  assert.equal(
    await readFile(join(projectRoot, "src/types/src.ts"), "utf8"),
    "export type InternalModel = {\n  id: string;\n};\n"
  );
  assert.equal(
    await readFile(join(projectRoot, "src/index.ts"), "utf8"),
    "export type { InternalModel as PublicModel } from \"./types/src\";\n"
  );
  assert.equal(
    await readFile(join(projectRoot, "src/app.ts"), "utf8"),
    "import type { PublicModel } from \"./index\";\n\nexport const current: PublicModel = { id: \"current\" };\n"
  );
});

await verifyFixture("cli-type-export-barrel-project", async (projectRoot) => {
  run(projectRoot, binary, [
    "extract",
    "TypedBarrelModel",
    "--from",
    "src/shared.ts",
    "--project",
    "tsconfig.json"
  ]);
  run(projectRoot, "tsc", ["--noEmit", "--project", "tsconfig.json"]);

  assert.equal(
    await readFile(join(projectRoot, "src/types/src.ts"), "utf8"),
    "export interface TypedBarrelModel {\n  id: string;\n}\n"
  );
  assert.equal(
    await readFile(join(projectRoot, "src/index.ts"), "utf8"),
    "export type { TypedBarrelModel } from \"./types/src\";\n"
  );
});

await verifyFixture("cli-declaration-source-project", async (projectRoot) => {
  const source = join(projectRoot, "src/contracts.d.ts");
  const originalSource = await readFile(source, "utf8");
  const result = runFailure(projectRoot, binary, [
    "extract",
    "DeclaredContract",
    "--from",
    "src/contracts.d.ts",
    "--project",
    "tsconfig.json"
  ]);

  assert.match(result.stderr, /No extractable candidate found/);
  assert.equal(await readFile(source, "utf8"), originalSource);
  assert.equal(await fileExists(join(projectRoot, "src/types/src.d.ts")), false);
  run(projectRoot, "tsc", ["--noEmit", "--project", "tsconfig.json"]);
});

await verifyFixture("cli-nested-package-imports-project", async (projectRoot) => {
  run(projectRoot, binary, [
    "extract",
    "NestedModel",
    "--from",
    "packages/app/src/model.ts",
    "--project",
    "tsconfig.json"
  ]);
  run(projectRoot, "tsc", ["--noEmit", "--project", "tsconfig.json"]);

  assert.equal(
    await readFile(join(projectRoot, "packages/app/src/types/src.ts"), "utf8"),
    "export type NestedModel = {\n  id: string;\n};\n"
  );
  assert.equal(
    await readFile(join(projectRoot, "packages/app/src/app.ts"), "utf8"),
    "import type { NestedModel } from \"./types/src.js\";\n\nexport const current: NestedModel = { id: \"nested\" };\n"
  );
});

await verifyFixture("cli-base-url-project", async (projectRoot) => {
  run(projectRoot, binary, [
    "extract",
    "BaseUrlModel",
    "--from",
    "src/models/shared.ts",
    "--project",
    "tsconfig.json"
  ]);
  run(projectRoot, "tsc", ["--noEmit", "--project", "tsconfig.json"]);

  assert.equal(
    await readFile(join(projectRoot, "src/models/types/models.ts"), "utf8"),
    "export type BaseUrlModel = {\n  id: string;\n};\n"
  );
  assert.equal(
    await readFile(join(projectRoot, "src/app.ts"), "utf8"),
    "import type { BaseUrlModel } from \"./models/types/models\";\n\nexport const current: BaseUrlModel = { id: \"base-url\" };\n"
  );
});

console.log("CLI movement fixtures passed.");
