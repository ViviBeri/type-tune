---
name: learn
description: Capture reusable discoveries about type-tune's Rust scan, plan, rewrite, transaction, or compatibility behavior after substantive work. Do not use for routine edits or already documented facts.
---

# Learn from type-tune work

Capture a non-obvious, reusable lesson supported by the current implementation or a reproducible
result. Follow the user's instructions and `AGENTS.md`; do not add knowledge for routine changes or
repeat existing guidance.

1. Review [LEARNED_INDEX.md](LEARNED_INDEX.md) and the relevant existing notes. Update an existing
   note if the discovery refines it; create a short `learned/<kebab-case>.md` only for a distinct lesson.
2. Verify the behavior against the current code and tests. Explain the problem, the invariant or
   tradeoff, when it applies, and the evidence or checks that support it. Distinguish confirmed
   behavior from a recommendation or an untested limitation.
3. Reference existing repository-relative paths and keep the index in sync. Remove obsolete commands,
   historical implementation assumptions, and broken references rather than preserving them as rules.
4. Run checks appropriate to the changed guidance and report what was verified. Do not include private
   project data, credentials, or unverified safety guarantees in public notes.

## Project context

- `crates/type-tune-cli/src/main.rs` and its command modules implement the CLI;
  `crates/type-tune-core/src/` contains the parser, scan, plan, rewrite, and transaction logic.
- TypeScript analysis uses tree-sitter and the Rust parser/model APIs. Source paths and stable IDs
  follow `AGENTS.md`; `bin/type-tune.mjs` is only the npm distribution launcher.
- Rust unit and CLI integration tests use temporary projects; `fixtures/` and
  `scripts/compatibility-fixtures.mjs` exercise end-to-end compatibility.
- Use `pnpm check` for the workspace fmt, Clippy, tests, release build, and compatibility fixtures.
  Run `pnpm docs:check` and `pnpm docs:build` when documentation changes warrant them;
  `pnpm publish:check` adds version synchronization and package checks before release.
