# Untrusted Plan Project Boundaries

**Captured:** 2026-08-24
**Context:** Reading plan candidate metadata and preparing any apply, dry-run, or extract rewrite.
**Tags:** type-tune, typescript, cli, plan, apply, rewrite, security, paths, symlink, vitest

## Problem

Candidate paths persisted in `plan.json` can reach `path.join` and filesystem writes. Schema type
checks and rollback do not make those paths safe: traversal can write outside the project before
rollback, and a symlink can redirect a lexically contained destination. Treating persisted candidate
metadata as authoritative also lets a modified plan select paths or import strategies that scan never
produced.

Checking only `fs.existsSync` is insufficient. It follows symlinks and reports a dangling symlink as
missing, even though writing through a dangling file symlink can create its external target.

## Solution

Protect the rewrite boundary in three independent layers:

1. Parse plan project-relative paths as canonical POSIX TypeScript paths. Reject empty or absolute
   paths, Windows separators or drive prefixes, `.` / `..`, normalization changes, NUL, declaration
   files, and unsupported extensions.
2. Immediately before building a rewrite action, validate every candidate path and every computed
   changed path against the selected project root. Resolve the nearest path entry with `lstatSync`,
   then use `realpathSync` to reject ordinary and dangling symlink escapes before snapshots or writes.
3. During apply, re-scan with the persisted scan options and compare each complete candidate object
   with the fresh candidate for that ID. Keep source fingerprint validation as a separate content
   freshness check.

Put the filesystem boundary check in the shared rewrite path so direct extract and dry-run receive
the same defense. Apply should additionally validate every plan entry, including ignored entries,
before selecting rewrite candidates.

The selected project root is a separate trust boundary. These checks prove that candidate paths stay
inside that root; fully hostile plans also require binding the root to caller-controlled input rather
than trusting top-level `plan.projectPath`.

## Example

```ts
validateCandidatePathBoundaries(input.projectRoot, input.candidates);

const changedFilePaths = getCandidateChangedFilePathsForAll(
  input.projectRoot,
  input.candidates,
  input.sourceFiles,
);

validateChangedFilePathBoundaries(input.projectRoot, changedFilePaths);
```

For missing destinations, walk upward with `lstatSync` rather than `existsSync`. A dangling symlink is
an existing path entry whose target cannot be safely resolved and must be rejected.

## When To Use

Apply this pattern whenever a persisted plan field, CLI destination, scan-derived path, or newly
computed changed-file path can influence reads, writes, directory creation, deletion, snapshots, or
rollback. Add regression tests for lexical traversal, absolute paths, NUL, invalid extensions,
ordinary directory symlinks, dangling file symlinks, and in-boundary metadata tampering.

Validated in the originating change with:

- `pnpm check` — 247 tests passed, including lint, typecheck, full tests, and build
- `pnpm docs:check` — no diagnostics

## Related Files

- `src/utils/project-paths.ts`
- `src/plan/read-plan-json.ts`
- `src/plan/validate-plan.ts`
- `src/rewrite/validate-candidate-paths.ts`
- `src/rewrite/apply-single-candidate.ts`
- `src/commands/apply.ts`
- `src/plan/read-plan-json.test.ts`
- `src/commands/apply.test.ts`
- `docs/design.md`
