# Parallelize Fixture-Heavy Vitest Suites by File

**Captured:** 2026-08-28
**Context:** CLI integration suites that repeatedly load ts-morph projects, rescan plans, rewrite files, and run TypeScript validation.
**Tags:** type-tune, typescript, ts-morph, cli, vitest, fixtures, testing, performance, parallelism

## Problem

Vitest parallelizes test files, but tests within one file run serially by default. A large fixture-heavy
suite can therefore become the wall-clock critical path even when the rest of the suite is well
parallelized.

`src/commands/apply.test.ts` contained 44 tests in one file. Many tests created a temporary TypeScript
project, ran `plan`, loaded and rescanned the project during `apply`, performed a rewrite, and ran real
TypeScript compile validation. The file took 28.8 seconds in isolation and held the full suite near 31
seconds.

Enabling `--sequence.concurrent` was not a safe shortcut. Command tests share a mocked
`process.stdout`, so concurrent tests mixed output, broke JSON assertions, and caused timeouts. It also
did not materially improve wall-clock time.

## Solution

Keep the real plan, rewrite, and compile-validation coverage, but distribute the test groups across
multiple Vitest files so the existing file-level worker parallelism can run them independently.

1. Move the shared test definitions and fixture helpers to a non-test module.
2. Export small registration functions for balanced behavioral groups.
3. Add thin `*.test.ts` wrappers that import and invoke one registration function each.
4. Profile the split groups and divide any group that still dominates the critical path.
5. Do not enable global test concurrency while suites rely on process-global stdout spies.

Group tests by execution cost and responsibility, not only by test count. In this case the final groups
covered workflow, local dependencies, referrers, barrels, imports, and safety validation. Splitting four
initial groups into six balanced groups reduced the apply suite from 28.8 seconds to 10.5 seconds and
the full suite to 14.1 seconds, while preserving all 257 tests and real compile validation.

## Example

```ts
// apply-test-cases.ts
export function registerApplyCommandSafetyTests(): void {
  registerApplyCommandTestGroup("applyCommand safety", () => {
    it("rolls back existing and new files when compile validation fails", async () => {
      // Real fixture, rewrite, validation, and rollback assertions.
    });
  });
}

// apply-safety.test.ts
import { registerApplyCommandSafetyTests } from "./apply-test-cases.js";

registerApplyCommandSafetyTests();
```

Measure file durations with Vitest's JSON reporter instead of guessing:

```bash
pnpm exec vitest run --reporter=json --outputFile=/tmp/type-tune-vitest-profile.json
jq -r '.testResults[] | [.endTime - .startTime, .name] | @tsv' \
  /tmp/type-tune-vitest-profile.json | sort -nr
```

After restructuring, verify the focused groups and the complete project:

```bash
pnpm exec vitest run src/commands/apply-*.test.ts
pnpm check
```

## When To Use

Use this pattern when one fixture-heavy test file dominates wall-clock time and its tests are isolated
by temporary project directories. Prefer file splitting before mocking compile validation, because it
preserves end-to-end rewrite safety coverage. Consider mocking or a broader test-pyramid refactor only
if balanced file-level parallelism is still too slow.

Do not use global `sequence.concurrent` when tests share process-global spies, mutable singleton state,
or output buffers.

## Related Files

- `src/commands/apply-test-cases.ts`
- `src/commands/apply-workflow.test.ts`
- `src/commands/apply-dependencies.test.ts`
- `src/commands/apply-referrers.test.ts`
- `src/commands/apply-barrels.test.ts`
- `src/commands/apply-imports.test.ts`
- `src/commands/apply-safety.test.ts`
- `vitest.config.ts`
