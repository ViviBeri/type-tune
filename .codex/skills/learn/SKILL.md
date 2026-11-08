---
name: learn
description: Extract reusable patterns from non-trivial work in the type-tune-base repository and store them as project knowledge. Use after meaningful ts-morph analysis, deterministic scan data, plan/apply workflow, rewrite, CLI, testing, or compatibility discoveries.
---

# /learn - Capture Reusable type-tune-base Patterns

Analyze the current session and capture reusable knowledge for this TypeScript CLI that finds and extracts shared TypeScript types through a reviewable plan.

## Auto-Activation Criteria

Consider auto-activating this skill when:

1. A non-trivial `ts-morph` analysis, symbol/reference lookup, or AST rewrite pattern was implemented or debugged.
2. A deterministic scan, candidate ID, declaration key, sorting, path normalization, or fingerprinting rule was added or corrected.
3. A `scan -> plan -> apply` workflow decision was made and validated with tests.
4. An import strategy, destination resolution, local dependency collection, or rewrite conflict rule was discovered.
5. The session produced reusable guidance about CLI behavior, Vitest fixtures, TypeDoc comments, ESM imports, or release safety for this repository.

Do not activate for:

- trivial typo fixes
- one-line doc wording changes
- obvious refactors with no reusable lesson
- patterns already documented in `.codex/skills/learn/learned/`

## Manual Trigger

Run `/learn` after finishing a meaningful implementation, refactor, or debugging session in `type-tune-base`.

## What To Extract

### 1. Project Analysis Patterns

- What `ts-morph` API or project-loading behavior mattered?
- Which source files were included or excluded, and why?
- How were TypeScript / TSX / declaration / generated files handled?
- What future scanner or rewrite code should preserve?

Project-relevant examples:

- Loading a `Project` from `tsconfig.json` and deriving `projectRoot`.
- Keeping paths project-relative and POSIX-normalized before hashing or printing.
- Excluding `.d.ts`, `generated`, `__generated__`, `*.generated.ts`, and `*.generated.tsx` by default.

### 2. Deterministic Data Patterns

- What stable key, ID, hash, sort order, or fingerprint rule was involved?
- What inputs must be normalized before comparison or hashing?
- What test locks the behavior down?

Project-relevant examples:

- Candidate stable keys from `sourcePath`, `symbolName`, and `kind`.
- Candidate IDs as the first 12 hex characters of SHA-256.
- Declaration keys as full SHA-256 hashes of declaration stable keys.
- Sorting scan output by stable key with English locale comparison.

### 3. Plan and Rewrite Workflow Patterns

- How does the pattern support `scan -> plan -> apply`?
- Which data belongs in `plan.json`, `plan.md`, or rewrite actions?
- How are review state, stale plans, dedupe, or conflicts handled?
- What minimal rewrite behavior should future changes preserve?

Project-relevant examples:

- Treating `plan.md` checklist state as the human review source.
- Keeping `plan.json` as structured apply input with complete candidate metadata.
- Dedupe and conflict detection by declaration key before writing to disk.
- Preserving existing import style: barrel, alias, or relative when possible.

### 4. CLI and Module Patterns

- What command shape, option parsing, output format, or error behavior was chosen?
- How does the command fit into `src/cli/index.ts` and `src/commands/`?
- What ESM import or package/build constraint mattered?

Project-relevant examples:

- Command modules export functions returning `commander` `Command` instances.
- Relative imports in `src/**/*.ts` use `.js` extensions.
- Machine-readable output is concentrated in `scan --format json` rather than a separate `report` command.

### 5. Verification Patterns

- What test mix caught or prevented the issue?
- What fixture setup is reusable?
- What commands should be run after similar changes?
- What docs or TypeDoc comments needed updating?

Project-relevant examples:

- Vitest fixture projects created with `fs.mkdtempSync(path.join(os.tmpdir(), "..."))`.
- Focused tests for deterministic ordering, generated file filtering, hash values, and CLI-visible output.
- Running `pnpm lint`, `pnpm format:check`, `pnpm typecheck`, `pnpm test`, `pnpm build`, or `pnpm check` depending on change scope.
- Adding TypeDoc comments to exported types and every exported object type field.

## Output Format

Create one file per pattern at:

`.codex/skills/learn/learned/[pattern-name].md`

Template:

````markdown
# [Descriptive Pattern Name]

**Captured:** YYYY-MM-DD
**Context:** [When this pattern applies]
**Tags:** type-tune, typescript, ts-morph, cli, scan, plan, rewrite, deterministic, vitest, etc.

## Problem

[Specific recurring problem this pattern solves]

## Solution

[Reusable approach, key decisions, constraints, and tradeoffs]

## Example

```ts
// Minimal real example from this codebase
```

## When To Use

[Trigger conditions for applying this pattern]

## Related Files

- `src/project/load-project.ts`
- `src/scan/exported-declarations.ts`
- `src/models/candidate.ts`
- `docs/design.md`
- `docs/development-plan.md`
````

## Process

1. Review the session for candidate learnings.
2. Select the highest-value reusable pattern(s).
3. Draft the pattern file.
4. Ask for user confirmation before saving unless the user explicitly requested `/learn` to save directly.
5. Save to `.codex/skills/learn/learned/`.
6. Update `.codex/skills/learn/LEARNED_INDEX.md` with a one-line entry.

Index format:

`- **[pattern-name](learned/pattern-name.md)** - One-line summary.`

## Common Pattern Categories For This Repository

- `ts-morph` project loading and AST traversal
- Exported type/interface candidate scanning
- Deterministic IDs, keys, fingerprints, sorting, and path normalization
- Local type dependency collection
- Destination and import strategy resolution
- Reviewable `plan.json` / `plan.md` workflows
- Minimal rewrite actions, dedupe, and conflict detection
- Commander CLI design and ESM package constraints
- Vitest fixture projects and CLI-visible regression tests
- TypeDoc coverage for exported models

## Notes

- Capture only reusable, non-trivial lessons.
- Prefer patterns that explain the constraint, the implementation shape, and the validation that protects it.
- Include exact commands and tests that validated the pattern when relevant.
- Keep entries concrete, searchable, and specific to `type-tune-base`.
