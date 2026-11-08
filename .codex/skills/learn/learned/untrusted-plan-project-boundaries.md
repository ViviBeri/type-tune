# Untrusted plan project boundaries

Plan metadata and reviewed destinations are untrusted inputs. Schema checks alone cannot prevent
project-root escapes or a tampered candidate from reaching a rewrite.

- Reject unsafe or non-canonical project-relative TypeScript paths and check filesystem boundaries,
  including symlinks and dangling path entries, before writes.
- Re-scan with the plan's options and compare persisted candidate metadata with fresh results;
  separately validate source fingerprints. Apply path checks to effective reviewed destinations too.
- Keep direct extract and dry-run within the same filesystem safety boundary. Test traversal,
  absolute paths, symlinks, metadata tampering, destination conflicts, and stale declarations.

These checks operate relative to the selected project root; they do not by themselves establish
trust in an arbitrary caller-supplied project. Concurrent filesystem changes remain a separate risk.

Relevant code: `crates/type-tune-core/src/path.rs`,
`crates/type-tune-core/src/plan/validation.rs`, `crates/type-tune-cli/src/commands/apply.rs`, and
`crates/type-tune-cli/tests/review_destination.rs`.
