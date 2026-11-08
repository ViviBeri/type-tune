# CLI and core refactoring status

This document tracks internal organization work. Refactoring must preserve command syntax, output
formats, package delivery, deterministic scan data, plan schemas, rewrite behavior, and the apply
safety contract. The Astro documentation site remains independent of the Rust runtime.

## Completed

- The CLI command-module extraction is complete. `main.rs` handles process entry and
  dispatch; `scan`, `plan`, `apply`, and `extract` live under `commands`, and `review` has its own
  module. Shared modules own option parsing, help, project loading, previews, and transaction work.
  `run_plan` now lives in `commands/plan.rs` with the other plan helpers.
- Command options accepted by `validate_options` are the `CommandOptions` constants in
  `options.rs`. Help text is exposed as strings, and
  `documented_options_match_accepted_options_and_help` fails when those constants, the help text,
  or `docs/src/shared/config/cli-options.ts` disagree. That is the drift guard for documented
  command options.
- The repository cleanup branch further shares scan candidate loading, rewrite planning and
  transaction setup, and standardizes transaction errors.
- Core scan responsibilities are separated into source discovery, filtering, placement,
  declaration analysis, barrel resolution, and referrer analysis. Scan tests have their own module.
- Core rewrite helpers are separated into options, import handling and formatting, resolution,
  validation, destination rendering, compatibility exports, re-exports, and referrer planning.
  `plan_extraction_rewrite_with_options` delegates to `rewrite/planning.rs`, which keeps the
  phases in order: prepare inputs, record merged referrer imports, plan source moves, render
  destinations, plan remaining referrer edits, and apply edits. Source cleanup and referrer edits
  remain separate planning phases. Rewrite tests have their own module.
- AI request construction and redaction, response validation, provider transport, and review cache
  have been separated from the main AI review module. AI review tests have their own module.
- TypeScript configuration JSONC normalization, inheritance loading, and package alias discovery
  have their own modules.
- Parser traversal for declarations, static and inline type imports, and re-exports has separate
  modules.

These are behavior-preserving refactors, not new CLI features.

## Real project checks

Rewrite planning was run against neighboring repositories before and after the phase split.

- `changelog-bot`: both shared type aliases planned together, and each source group on its own.
  The batch fingerprint and both group fingerprints matched before the import-marker fix below.
  `apply --dry-run` first failed TypeScript validation: a multiline `import type` lost one
  dependency and was rewritten as `import type { type ... }`. Source import cleanup now keeps a
  single type marker. A second dry-run validated both candidates with the project's TypeScript
  and rolled the files back.
- `is-kit`: `SinglePropertyKey` depends on the private helper `IsUnion`, which
  `KeyPresences` still uses. The rewrite keeps `IsUnion` in `src/core/key-internals.ts`, copies it
  into the destination, and imports `SinglePropertyKey` back. `extract --dry-run` validates that
  move with the project's TypeScript and rolls the files back.
- Real-world application checks exercised semicolon-free imports, private helper dependencies,
  overlapping edits, module-specifier extensions, comments, import splitting, aliases, line wrapping,
  and compatibility re-exports. Existing project diagnostics limited some full-project checks;
  passing a scoped dry-run does not establish full-project compatibility.

## Follow-up candidates

These are possible small, behavior-preserving changes, not release blockers or a requirement to
split files solely by line count. Assess each separately and retain existing public APIs and tests.

Each change should compile and run its relevant tests. The full final gate for a refactoring branch
is:

```bash
pnpm check
pnpm docs:check
```
