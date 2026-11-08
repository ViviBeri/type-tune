# Repository Instructions

## Completion Workflow

- After implementing requested changes, run the relevant checks and commit the task's changes unless the user asks otherwise.
- Leave unrelated user changes out of the commit.

## Implementation Conventions

- This project is a Rust CLI. Keep command parsing in `crates/type-tune-cli/src/main.rs` and reusable behavior in `crates/type-tune-core/`.
- The npm `bin/type-tune.mjs` file is a minimal distribution launcher only; do not add CLI behavior or fallbacks to it.
- Use tree-sitter and the existing Rust parser/model APIs for TypeScript analysis and rewrites.
- Keep source paths project-relative and POSIX-normalized before storing, comparing, hashing, or printing them.
- Generated files are excluded from project scanning by default. Treat `generated`, `__generated__`, `*.generated.ts`, and `*.generated.tsx` paths as generated unless an option explicitly includes them.

## Deterministic Scan Data

- Keep scan output deterministic. Sort declaration results by stable key using English locale comparison.
- Candidate stable keys are built from normalized `sourcePath`, `symbolName`, and `kind`.
- Candidate IDs are the first 12 hex characters of the SHA-256 hash of the candidate stable key.
- Declaration stable keys include normalized `sourcePath`, `kind`, `symbolName`, and declaration start position.
- Declaration keys are the full SHA-256 hash of the declaration stable key and are used for rewrite dedupe and conflict detection.

## Tests

- Use Rust unit tests in the relevant module or CLI test module.
- For TypeScript project scanning tests, create temporary fixture projects with `tempfile` and a minimal `tsconfig.json`.
- Add focused tests for deterministic ordering, path filtering, stable key/hash generation, and CLI-visible behavior when implementing commands.
