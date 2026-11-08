# Fail-closed plan review

When changing plan generation, Markdown review, or apply, preserve the explicit review boundary:

- Candidates default to `ignore`; `--default-status apply` is an explicit opt-in.
- Reviewed Markdown must contain every candidate ID and reject malformed, duplicate, unknown, or
  missing entries. Reviewed destination overrides must remain separate from scanned metadata.
- Without Markdown, apply requires `--accept-unreviewed`, including for dry-run. Do not let the
  initial status bypass that requirement.

Test selection, complete Markdown parsing, and the unreviewed-plan gate together. Also retain
coverage for edited destinations and stale or tampered plan data.

Relevant code: `crates/type-tune-cli/src/options.rs`, `crates/type-tune-cli/src/commands/apply.rs`,
`crates/type-tune-core/src/plan/markdown.rs`, and `crates/type-tune-cli/tests/review_destination.rs`.
