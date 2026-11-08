# Contributing

Thanks for taking the time to improve type-tune.

type-tune is a Rust refactoring CLI for TypeScript projects, so changes should stay careful,
deterministic, and easy to review. The project favors a narrow, reliable workflow over
large automatic rewrites.

## Before You Start

For larger changes, please open an issue first. This helps confirm the scope before you
spend time on implementation.

Good first contributions include:

- Improving README examples or command documentation.
- Adding focused tests for scanner, planner, or rewrite behavior.
- Improving error messages.
- Tightening package or release checks.

Please avoid broad refactors mixed with behavior changes. They make rewrite safety harder
to review.

## Development Setup

Install dependencies:

```bash
pnpm install
# Optional: enable the local pre-commit checks
pnpm exec lefthook install
```

The commands below use [mise](https://mise.jdx.dev/) to provision the pinned stable Rust toolchain.
Install mise before running the Rust checks directly, or use the `pnpm` commands, which invoke it
for you.

Run the full local check:

```bash
pnpm check
```

Run the release package check:

```bash
pnpm publish:check
```

Run the end-to-end compatibility fixtures after a scan, plan, rewrite, import-resolution, or
TypeScript-validation change:

```bash
pnpm compatibility:check
```

Build the documentation site before changing Astro content, navigation, or examples:

```bash
pnpm docs:check
pnpm docs:build
```

## Project Conventions

- The CLI implementation lives in `crates/type-tune-cli` and `crates/type-tune-core`.
- Use Rust's `Path` APIs and the project's path normalization helpers for project-relative paths.
- Keep source paths project-relative and POSIX-normalized before storing, comparing,
  hashing, or printing them.
- Keep scan output deterministic.
- Document public Rust APIs when their behavior is not self-evident.
- Keep user-facing command examples aligned with `target/release/type-tune <command> --help`.
- Keep `docs/` TypeScript tooling scoped to the Astro documentation site; it is not part of the
  published CLI implementation.

## Testing

Use Rust unit tests beside the implementation. For project scanning tests, create temporary fixture
projects with a minimal `tsconfig.json`.

Useful checks:

```bash
cargo fmt --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo build --release --workspace
```

For packaging changes, stage the appropriate native binary before running `pnpm package:check` so
the packed-launcher smoke test can execute it. CI performs that staging for each release target.

## Pull Requests

Please include:

- A short summary of the change.
- The reason the change is needed.
- Tests or verification commands.
- Any known limitations or follow-up work.

For rewrite behavior, include before/after examples when possible.
