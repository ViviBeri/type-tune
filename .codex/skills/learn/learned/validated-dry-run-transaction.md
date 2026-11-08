# Validated Dry-Run Transaction

**Captured:** 2026-08-25  
**Context:** Any dry-run or preview that temporarily performs a candidate rewrite.  
**Tags:** type-tune, typescript, cli, dry-run, apply, extract, rewrite, compile-validation, rollback, vitest

## Problem

A preview that only rewrites and rolls back can report success even when the equivalent real apply would fail its compile gate. This makes dry-run a weaker execution path than the operation it claims to preview.

Tests that start from an already broken project are insufficient: they do not prove validation ran against the temporarily rewritten filesystem state.

## Solution

Treat dry-run as the same validated transaction as apply, except that rollback is unconditional:

1. Build the rewrite action and snapshot every affected path.
2. Apply the temporary rewrite.
3. Run the shared post-rewrite compile validation.
4. Produce the preview summary only after validation succeeds.
5. Roll back in a `finally` block on both success and failure.

Pass `projectPath` and the command-specific compile failure message through the same validated-rewrite input used by real apply. A validation failure should throw the normal CLI error, suppress the success summary, and result in a non-zero exit while still restoring the original files.

```ts
try {
  await rewriteAction.apply();
  validateRewrittenProject(input.projectPath, input.compileFailureMessage);
  return summarizeDryRunChanges(rewriteAction.changedFilePaths, snapshot.originalTexts);
} finally {
  rollbackRewrite(snapshot);
}
```

For a strong regression test, use a fixture that compiles before the rewrite but fails only after a declaration moves. An inline relative `import()` type is useful because its resolution context changes with the destination path. Assert the compile error, non-zero CLI exit, absence of success output, restoration of the source, and removal of the temporary destination.

## When To Use

- When changing preview/apply compile validation or rollback behavior.
- When adding a command whose dry-run promises to validate the proposed rewrite.
- When testing whether validation observes the rewritten project rather than only the original project.

## Verification

- `pnpm check` — 250 tests passed; lint, formatting, typecheck, and build succeeded.
- `pnpm docs:check` — completed with no diagnostics.

## Related Files

- `src/rewrite/validated-rewrite.ts`
- `src/commands/apply.ts`
- `src/commands/extract.ts`
- `src/commands/apply.test.ts`
- `src/commands/extract.test.ts`
- `src/cli/index.test.ts`
- `docs/design.md`
- `docs/requirements.md`
